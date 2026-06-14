use nexushub_core::{
    claude_code::{
        claude_maintenance_commands, claude_overview, discover_claude_projects, ClaudePaths,
    },
    config::Config,
    platform::{PlatformKind, PlatformPaths},
    probe::{ProbeActionPlanKind, ProbeEventInput, ProbeEventOutcome, ProbeRuntime},
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

#[tokio::test]
async fn probe_status_is_builtin_and_does_not_require_legacy_cli() {
    let config = Config::default();
    let status = ProbeRuntime::new(config, PlatformPaths::for_kind(PlatformKind::Linux))
        .status()
        .await
        .unwrap();

    assert_eq!(status.label, "Probe");
    assert!(status.enabled);
    assert!(status.available);
    assert_eq!(status.flavor, "builtin");
    assert_eq!(status.service_name, "nexushub");
    assert!(status.binary_path.is_none());
    assert_eq!(status.lifecycle_status, "managed");
    assert_eq!(status.doctor_status, "ready");
    assert_eq!(
        status.config_path,
        PathBuf::from("/opt/nexushub/config.toml")
    );
    assert_eq!(status.recent_event_count, 0);
}

#[test]
fn probe_rejects_unsafe_thread_probe_ids() {
    assert!(nexushub_core::probe::safe_thread_probe_id("thread-abc"));
    assert!(!nexushub_core::probe::safe_thread_probe_id("thread one"));
    assert!(!nexushub_core::probe::safe_thread_probe_id("../thread"));
}

#[test]
fn probe_action_plans_use_nexushub_commands_and_confirmation_ids() {
    let config = Config::default();
    let runtime = ProbeRuntime::new(config, PlatformPaths::for_kind(PlatformKind::Linux));

    let hook_plan = runtime
        .plan_action(ProbeActionPlanKind::InstallHooks)
        .unwrap();
    assert!(hook_plan.plan_id.starts_with("probe-hooks-"));
    let hook_command_step = hook_plan
        .steps
        .iter()
        .find(|step| step.contains("probe hook-stop"))
        .expect("hook command step");
    assert!(
        hook_command_step.contains("nexushubd --config /opt/nexushub/config.toml probe hook-stop")
    );
    assert!(!hook_plan
        .steps
        .iter()
        .any(|step| step.contains("codex-sentinel")));

    let logs_plan = runtime
        .plan_action(ProbeActionPlanKind::LogsDbMaintain)
        .unwrap();
    assert!(logs_plan.plan_id.starts_with("probe-logs-db-"));
    assert!(logs_plan.payload["retention_days"].as_u64().unwrap() >= 1);
}

#[test]
fn probe_diagnostics_lifecycle_and_hook_status_expose_builtin_runtime_boundaries() {
    let mut config = Config::default();
    config.probe.notifications.enabled = true;
    let runtime = ProbeRuntime::new(config, PlatformPaths::for_kind(PlatformKind::Linux));

    let lifecycle = runtime.lifecycle_status();
    assert_eq!(lifecycle.status, "managed");
    assert_eq!(lifecycle.service_name, "nexushub");
    assert_eq!(lifecycle.service_kind, "systemd");
    assert!(lifecycle.hooks_enabled);
    assert!(lifecycle.notifications_enabled);
    assert!(lifecycle.logs_db_enabled);
    assert_eq!(
        lifecycle.next_actions,
        vec![
            "probe-hook-ready".to_string(),
            "logs-db-maintenance-ready".to_string()
        ]
    );

    let hook = runtime.hook_status();
    assert_eq!(hook.status, "managed");
    assert!(hook.hook_command.contains("/opt/nexushub/bin/nexushubd"));
    assert!(hook.supported_events.contains(&"hook-stop".to_string()));
    assert!(hook
        .supported_events
        .contains(&"notify-completion".to_string()));
    assert_eq!(hook.dedupe_namespace, "probe_event");

    let diagnostics = runtime.diagnostics();
    assert_eq!(diagnostics.doctor_status, "ready");
    assert_eq!(diagnostics.lifecycle_status, "managed");
    assert!(diagnostics
        .managed_boundaries
        .iter()
        .any(|boundary| boundary.contains("不执行自动回复")));
    assert_eq!(
        diagnostics.effective_constants["legacy_sentinel_cli_runtime"],
        false
    );
    assert_eq!(
        diagnostics.effective_constants["hidden_desktop_control"],
        false
    );
}

#[test]
fn probe_event_model_dedupes_hook_stop_and_completion_without_desktop_control() {
    let runtime = ProbeRuntime::new(
        Config::default(),
        PlatformPaths::for_kind(PlatformKind::Linux),
    );

    let hook = runtime.build_event(ProbeEventInput::hook_stop(
        Some("thread-a"),
        Some("turn-1"),
        "hook-stop",
    ));
    assert_eq!(hook.kind, "hook-stop");
    assert_eq!(hook.thread_id.as_deref(), Some("thread-a"));
    assert_eq!(hook.dedupe_namespace, "probe_event");
    assert_eq!(hook.dedupe_key, "hook-stop:thread-a:turn-1");
    assert_eq!(hook.ttl_seconds, 300);
    assert_eq!(hook.payload["notify_completion"], false);
    assert_eq!(hook.payload["auto_reply"], false);
    assert_eq!(hook.payload["hidden_desktop_control"], false);

    let duplicate_hook = runtime.build_event(ProbeEventInput::hook_stop(
        Some("thread-a"),
        Some("turn-1"),
        "hook-stop",
    ));
    assert_eq!(hook.dedupe_key, duplicate_hook.dedupe_key);

    let completion = runtime.build_event(ProbeEventInput::notify_completion(
        Some("thread-a"),
        Some("turn-1"),
    ));
    assert_eq!(completion.kind, "completion");
    assert_eq!(completion.dedupe_key, "completion:thread-a:turn-1");
    assert_eq!(completion.payload["notify_completion"], true);
    assert_eq!(completion.payload["auto_reply"], false);

    let recorded = ProbeEventOutcome::from_claim(&completion, true);
    assert!(recorded.recorded);
    assert_eq!(recorded.dedupe_key, completion.dedupe_key);
    let duplicate = ProbeEventOutcome::from_claim(&completion, false);
    assert!(!duplicate.recorded);
}

#[test]
fn probe_logs_db_plan_includes_deletion_vacuum_and_skip_reason() {
    let mut disabled = Config::default();
    disabled.probe.logs_db.enabled = false;
    let disabled_plan = ProbeRuntime::new(disabled, PlatformPaths::for_kind(PlatformKind::Linux))
        .plan_action(ProbeActionPlanKind::LogsDbMaintain)
        .unwrap();

    assert_eq!(disabled_plan.payload["deletion"]["enabled"], false);
    assert_eq!(disabled_plan.payload["vacuum"]["enabled"], false);
    assert_eq!(disabled_plan.payload["skip_reason"], "logs_db_disabled");
    assert_eq!(
        disabled_plan.payload["would_call_legacy_sentinel_cli"],
        false
    );
    assert!(disabled_plan
        .steps
        .iter()
        .any(|step| step.contains("跳过删除")));

    let mut enabled = Config::default();
    enabled.probe.logs_db.auto_compact_when_codex_closed = true;
    enabled.probe.logs_db.retention_days = 21;
    enabled.probe.logs_db.delete_chunk_rows = 500;
    let enabled_plan = ProbeRuntime::new(enabled, PlatformPaths::for_kind(PlatformKind::Linux))
        .plan_action(ProbeActionPlanKind::LogsDbMaintain)
        .unwrap();

    assert_eq!(enabled_plan.payload["deletion"]["enabled"], true);
    assert_eq!(enabled_plan.payload["deletion"]["retention_days"], 21);
    assert_eq!(enabled_plan.payload["deletion"]["chunk_rows"], 500);
    assert_eq!(enabled_plan.payload["vacuum"]["enabled"], true);
    assert_eq!(enabled_plan.payload["skip_reason"], serde_json::Value::Null);
}

#[test]
fn probe_bark_test_plan_redacts_device_key_and_keeps_payload_minimal() {
    let mut config = Config::default();
    config.probe.notifications.enabled = true;
    config.probe.notifications.group = "NexusHub Ops".to_string();
    config.probe.notifications.sound = Some("alarm".to_string());
    config.probe.notifications.url = Some("https://example.com/click".to_string());
    let runtime = ProbeRuntime::new(config, PlatformPaths::for_kind(PlatformKind::Linux));

    let plan = runtime.bark_test_plan(true);
    let rendered = serde_json::to_string(&plan).unwrap();

    assert_eq!(plan.kind, "bark-test");
    assert_eq!(plan.payload["configured"], true);
    assert_eq!(plan.payload["device_key"], "[configured]");
    assert_eq!(plan.payload["bark_payload"]["title"], "NexusHub Probe test");
    assert_eq!(
        plan.payload["bark_payload"]["body"],
        "Probe notification route is configured."
    );
    assert!(plan.payload["bark_payload"].get("device_key").is_none());
    assert!(plan.payload["bark_payload"].get("sound").is_none());
    assert!(plan.payload["bark_payload"].get("group").is_none());
    assert!(plan.payload["bark_payload"].get("url").is_none());
    assert!(!rendered.contains("secret"));
    assert!(!rendered.contains("alarm"));
    assert!(!rendered.contains("example.com"));
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
