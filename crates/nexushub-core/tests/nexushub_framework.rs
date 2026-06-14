use nexushub_core::{
    claude_code::{
        claude_maintenance_commands, claude_overview, discover_claude_projects, ClaudePaths,
    },
    config::Config,
    platform::{PlatformKind, PlatformPaths},
    probe::{ProbeCommandKind, ProbeCommandProfile, ProbeFlavor, ProbeStatusAvailability},
    providers::{AgentProviderId, ProviderRegistry},
};
use serde_json::json;
use std::{fs, path::PathBuf, time::SystemTime};

#[test]
fn default_config_uses_nexushub_runtime_names() {
    let config = Config::default();

    assert_eq!(config.paths.data_dir.to_string_lossy(), "/opt/nexushub");
    assert_eq!(
        config.paths.db_path.to_string_lossy(),
        "/opt/nexushub/nexushub.sqlite"
    );
    assert_eq!(
        config.paths.webui_dir.to_string_lossy(),
        "/opt/nexushub/webui"
    );
    assert_eq!(config.paths.log_dir.to_string_lossy(), "/opt/nexushub/logs");
    assert_eq!(
        config.update.panel_update_command,
        "/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest"
    );
}

#[test]
fn platform_paths_cover_linux_macos_and_windows() {
    assert_eq!(
        PlatformPaths::for_kind(PlatformKind::Linux).data_dir,
        PathBuf::from("/opt/nexushub")
    );
    assert_eq!(
        PlatformPaths::for_kind(PlatformKind::Macos).data_dir,
        PathBuf::from("~/Library/Application Support/NexusHub")
    );
    assert_eq!(
        PlatformPaths::for_kind(PlatformKind::Windows).data_dir,
        PathBuf::from(r"%ProgramData%\NexusHub")
    );
}

#[test]
fn provider_registry_exposes_codex_and_claude_preview() {
    let registry = ProviderRegistry::default();
    let providers = registry.list();

    let codex = providers
        .iter()
        .find(|provider| provider.id == AgentProviderId::Codex)
        .unwrap();
    assert_eq!(codex.status, "ready");
    assert!(codex
        .capabilities
        .iter()
        .any(|capability| capability == "ready"));

    let claude = providers
        .iter()
        .find(|provider| provider.id == AgentProviderId::ClaudeCode)
        .unwrap();
    assert_eq!(claude.status, "preview");
    assert!(claude
        .capabilities
        .iter()
        .any(|capability| capability == "readonly"));
    assert!(claude
        .capabilities
        .iter()
        .any(|capability| capability == "fixed_maintenance_commands"));
    assert!(claude.safety.contains("read-only"));

    assert!(providers
        .iter()
        .any(|provider| provider.id == AgentProviderId::Cursor));
    assert!(providers
        .iter()
        .any(|provider| provider.id == AgentProviderId::Gemini));
}

#[test]
fn probe_status_is_canonical_and_unavailable_without_cli() {
    let status = nexushub_core::probe::probe_status(
        &PlatformPaths::for_kind(PlatformKind::Linux),
        ProbeCommandProfile::unavailable(),
    );

    assert_eq!(status.label, "Probe");
    assert_eq!(status.availability, ProbeStatusAvailability::Unavailable);
    assert_eq!(status.hook_status, "unknown");
    assert_eq!(status.logs_db_status, "unknown");
    assert_eq!(status.flavor, ProbeFlavor::Unavailable);
}

#[test]
fn probe_server_profile_builds_only_fixed_server_commands() {
    let profile = ProbeCommandProfile::server("/usr/local/bin/codex-sentinel-server");

    assert_eq!(profile.flavor, ProbeFlavor::Server);
    assert_eq!(
        profile
            .command(ProbeCommandKind::InstallHooksRootRestartAppServer)
            .unwrap()
            .args,
        vec!["install-hooks-root", "--restart-app-server"]
    );
    assert_eq!(
        profile.command(ProbeCommandKind::TestBark).unwrap().args,
        vec!["test-bark"]
    );
    assert_eq!(
        profile
            .command(ProbeCommandKind::LogsDbMaintainDryRun)
            .unwrap()
            .args,
        vec!["logs-db-maintain", "--dry-run"]
    );
}

#[test]
fn probe_local_profile_builds_fixed_observation_commands_with_limits() {
    let profile = ProbeCommandProfile::local(
        "/Applications/Codex Sentinel Lite.app/Contents/MacOS/codex-sentinel-lite",
    );

    assert_eq!(profile.flavor, ProbeFlavor::Local);
    assert_eq!(
        profile
            .command(ProbeCommandKind::Running { limit: 7 })
            .unwrap()
            .args,
        vec!["running", "7"]
    );
    assert_eq!(
        profile
            .command(ProbeCommandKind::ReplyNeeded { limit: 8 })
            .unwrap()
            .args,
        vec!["reply-needed", "8"]
    );
    assert_eq!(
        profile
            .command(ProbeCommandKind::Recoverable { limit: 9 })
            .unwrap()
            .args,
        vec!["recoverable", "9"]
    );
    assert_eq!(
        profile
            .command(ProbeCommandKind::DebugAppServerThread {
                thread_id: "thread-abc".to_string(),
            })
            .unwrap()
            .args,
        vec!["debug-app-server-thread", "thread-abc"]
    );
}

#[test]
fn probe_rejects_unsafe_thread_probe_ids() {
    let profile = ProbeCommandProfile::local("/tmp/codex-sentinel-lite");

    assert!(profile
        .command(ProbeCommandKind::DebugAppServerThread {
            thread_id: "thread one".to_string(),
        })
        .is_none());
    assert!(profile
        .command(ProbeCommandKind::DebugAppServerThread {
            thread_id: "../thread".to_string(),
        })
        .is_none());
}

#[tokio::test]
async fn probe_command_runner_parses_json_and_redacts_output() {
    let root = temp_dir("nexushub-probe-runner");
    fs::create_dir_all(&root).unwrap();
    let binary = root.join("codex-sentinel-server");
    fs::write(
        &binary,
        "#!/bin/sh\nprintf '{\"ok\":true,\"items\":[1,2]}'\nprintf 'TOKEN=secret\\n' >&2\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&binary).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary, permissions).unwrap();
    }

    let profile = ProbeCommandProfile::server(&binary);
    let output = nexushub_core::probe::run_probe_command(&profile, ProbeCommandKind::Status).await;

    assert!(output.success);
    assert_eq!(output.json.unwrap()["ok"], true);
    assert_eq!(output.stderr, "[redacted sensitive line]");

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn claude_code_discovery_reads_project_sessions_from_claude_home() {
    let root = temp_dir("nexushub-claude-discovery");
    let projects_dir = root.join("projects").join("-Users-gosu-demo");
    fs::create_dir_all(&projects_dir).unwrap();
    fs::write(
        projects_dir.join("session-a.jsonl"),
        format!(
            "{}\n{}\n",
            json!({
                "type": "summary",
                "summary": "Implement provider abstraction",
                "timestamp": "2026-06-13T04:00:00Z"
            }),
            json!({
                "type": "user",
                "message": {"content": "hello"},
                "timestamp": "2026-06-13T04:01:00Z"
            })
        ),
    )
    .unwrap();
    fs::write(
        root.join("settings.json"),
        r#"{"permissions":{"allow":["Read"],"deny":["Write"]}}"#,
    )
    .unwrap();

    let projects = discover_claude_projects(&ClaudePaths::new(&root)).unwrap();

    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].id, "-Users-gosu-demo");
    assert_eq!(projects[0].display_name, "/Users/gosu/demo");
    assert_eq!(projects[0].sessions.len(), 1);
    assert_eq!(projects[0].sessions[0].id, "session-a");
    assert_eq!(
        projects[0].sessions[0].title.as_deref(),
        Some("Implement provider abstraction")
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn claude_code_overview_redacts_settings_and_summarizes_mcp() {
    let root = temp_dir("nexushub-claude-overview");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("settings.json"),
        r#"{
            "apiKey": "sk-ant-secret",
            "permissions": {"allow": ["Read"]},
            "mcpServers": {
                "github": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-github"],
                    "env": {"GITHUB_TOKEN": "ghp_secret", "SAFE_VALUE": "visible"}
                }
            }
        }"#,
    )
    .unwrap();

    let overview = claude_overview(&ClaudePaths::new(&root)).unwrap();
    let settings = overview.settings_preview.unwrap();

    assert_eq!(settings["apiKey"], "[redacted]");
    assert_eq!(
        settings["mcpServers"]["github"]["env"]["GITHUB_TOKEN"],
        "[redacted]"
    );
    assert_eq!(
        settings["mcpServers"]["github"]["env"]["SAFE_VALUE"],
        "visible"
    );
    assert_eq!(overview.mcp.server_count, 1);
    assert_eq!(overview.mcp.servers[0].name, "github");
    assert_eq!(overview.mcp.servers[0].command.as_deref(), Some("npx"));
    assert!(overview.mcp.servers[0]
        .env_keys
        .contains(&"GITHUB_TOKEN".to_string()));
    assert!(overview.mcp.servers[0].has_sensitive_env);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn claude_code_overview_reports_recent_sessions_install_and_cache_status() {
    let root = temp_dir("nexushub-claude-status");
    let project_dir = root.join("projects").join("-Users-gosu-demo");
    fs::create_dir_all(&project_dir).unwrap();
    fs::create_dir_all(root.join("logs")).unwrap();
    fs::create_dir_all(root.join("cache")).unwrap();
    fs::write(root.join("logs").join("claude.log"), "log").unwrap();
    fs::write(root.join("cache").join("index.json"), "{}").unwrap();
    fs::write(
        project_dir.join("older.jsonl"),
        format!(
            "{}\n",
            json!({"summary":"Older session","timestamp":"2026-06-13T01:00:00Z"})
        ),
    )
    .unwrap();
    fs::write(
        project_dir.join("newer.jsonl"),
        format!(
            "{}\n{}\n",
            json!({"summary":"Newer session","timestamp":"2026-06-13T02:00:00Z"}),
            json!({"type":"assistant","message":{"content":[{"type":"text","text":"Done"}]},"timestamp":"2026-06-13T02:02:00Z"})
        ),
    )
    .unwrap();

    let overview = claude_overview(&ClaudePaths::new(&root)).unwrap();

    assert_eq!(overview.recent_sessions.len(), 2);
    assert_eq!(overview.recent_sessions[0].id, "newer");
    assert_eq!(overview.recent_sessions[0].project_id, "-Users-gosu-demo");
    assert_eq!(
        overview.recent_sessions[0].last_message_preview.as_deref(),
        Some("Done")
    );
    assert_eq!(overview.installation.claude_home, root);
    assert!(overview
        .installation
        .health_hints
        .contains(&"settings_missing".to_string()));
    assert!(overview.cache_status.cache_exists);
    assert!(overview.cache_status.log_exists);
    assert_eq!(overview.cache_status.cache_file_count, 1);
    assert_eq!(overview.cache_status.log_file_count, 1);

    fs::remove_dir_all(overview.installation.claude_home).unwrap();
}

#[test]
fn claude_maintenance_commands_are_fixed_shell_commands() {
    let commands = claude_maintenance_commands();

    assert_eq!(commands.version_check.name, "version_check");
    assert_eq!(commands.version_check.command, "claude --version");
    assert!(commands
        .update_precheck
        .command
        .contains("command -v claude"));
    assert!(commands
        .update_start
        .command
        .contains("npm install -g @anthropic-ai/claude-code"));
    assert!(commands.smoke.command.contains("claude -p"));
    assert!(commands.cache_log_status.command.contains("$HOME/.claude"));
    assert!(!commands.update_start.command.contains("{}"));
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{label}-{unique}"))
}
