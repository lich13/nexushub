use nexushub_core::{
    claude_code::{
        claude_maintenance_commands, claude_overview, discover_claude_projects, ClaudePaths,
    },
    config::Config,
    platform::{PlatformKind, PlatformPaths},
    probe::{ProbeActionPlanKind, ProbeEventInput, ProbeEventOutcome, ProbeRuntime},
    providers::{AgentProviderId, ProviderRegistry},
};
use rusqlite::{params, Connection};
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
    let root = temp_dir("nexushub-probe-status");
    let codex_home = root.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    seed_codex_logs_db(&codex_home.join("logs_2.sqlite"), &[]);
    let mut config = Config::default();
    config.codex.home = codex_home;
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
    assert_eq!(status.logs_db_status, "ok");
    assert_eq!(
        status.config_path,
        PathBuf::from("/opt/nexushub/config.toml")
    );
    assert_eq!(status.recent_event_count, 0);
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn probe_status_exposes_hook_and_bark_config_without_device_key() {
    let root = temp_dir("nexushub-probe-status-bark-config");
    let codex_home = root.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    seed_codex_logs_db(&codex_home.join("logs_2.sqlite"), &[]);
    let mut config = Config::default();
    config.codex.home = codex_home;
    config.probe.hooks.manage_stop_hook = true;
    config.probe.notifications.enabled = true;
    config.probe.notifications.notify_completion = true;
    config.probe.notifications.notify_reply_needed = false;
    config.probe.notifications.notify_recoverable = true;
    config.probe.notifications.server_url = "https://api.day.app/custom".to_string();
    let status = ProbeRuntime::new(config, PlatformPaths::for_kind(PlatformKind::Linux))
        .status()
        .await
        .unwrap();
    let status_json = serde_json::to_value(&status).unwrap();

    assert_eq!(status_json["hook_stop_enabled"], true);
    assert_eq!(status_json["hooks_enabled"], true);
    assert_eq!(status_json["bark_status"], "configured");
    assert_eq!(status_json["bark_enabled"], true);
    assert_eq!(status_json["bark_server_url"], "https://api.day.app/custom");
    assert_eq!(status_json["bark_notify_completion"], true);
    assert_eq!(status_json["bark_notify_reply_needed"], false);
    assert_eq!(status_json["bark_notify_recoverable"], true);
    assert!(status_json.get("device_key").is_none());
    assert!(serde_json::to_string(&status_json)
        .unwrap()
        .contains("api.day.app"));
    assert!(!serde_json::to_string(&status_json)
        .unwrap()
        .contains("secret"));
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn probe_status_surfaces_missing_codex_logs_db() {
    let root = temp_dir("nexushub-probe-status-missing-logs");
    let codex_home = root.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    fs::write(codex_home.join("state_5.sqlite"), b"").unwrap();
    let mut config = Config::default();
    config.codex.home = codex_home.clone();
    let status = ProbeRuntime::new(config, PlatformPaths::for_kind(PlatformKind::Linux))
        .status()
        .await
        .unwrap();

    assert_eq!(status.logs_db_status, "missing_db");
    assert_eq!(status.codex_home, codex_home);
    assert_eq!(status.codex_home_source, "configured");
    fs::remove_dir_all(root).unwrap();
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
    assert!(hook
        .dedupe_key
        .starts_with("hook-stop:thread-a:turn-1:hook_stop:"));
    assert!(hook.payload["bark"].get("body").is_none());
    assert_eq!(
        hook.payload["bark"]["body_sha256"],
        hook.payload["body_sha256"]
    );
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
    assert!(completion
        .dedupe_key
        .starts_with("completion:thread-a:turn-1:completion:"));
    assert!(completion.payload["bark"].get("body").is_none());
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
fn probe_logs_db_status_reads_codex_logs_2_sqlite_counts_and_file_metadata() {
    let root = temp_dir("nexushub-codex-logs-status");
    let codex_home = root.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    let logs_path = codex_home.join("logs_2.sqlite");
    let now = chrono::Utc::now().timestamp();
    seed_codex_logs_db(&logs_path, &[now - 300_000, now - 200_000, now - 60]);
    fs::write(logs_path.with_extension("sqlite-wal"), b"wal").unwrap();
    fs::write(logs_path.with_extension("sqlite-shm"), b"shm-data").unwrap();

    let mut config = Config::default();
    config.codex.home = codex_home.clone();
    config.probe.logs_db.retention_days = 2;
    let status =
        ProbeRuntime::new(config, PlatformPaths::for_kind(PlatformKind::Linux)).logs_db_status();

    assert_eq!(status.target, "codex_logs_2");
    assert_eq!(status.path, logs_path);
    assert_eq!(status.status, "ok");
    assert_eq!(status.logs_db_status, "ok");
    assert_eq!(status.total_rows, 3);
    assert_eq!(status.old_rows, 2);
    assert_eq!(status.retained_rows, 1);
    assert!(status.database_size > 0);
    assert_eq!(status.wal_size, 3);
    assert!(status.shm_size >= 8);
    assert!(status.page_count > 0);
    assert_eq!(status.freelist_count, 0);
    assert_eq!(status.retention_days, 2);
    assert!(status.deletion.skip_reason.is_none());
}

#[test]
fn probe_logs_db_status_does_not_count_panel_probe_tables() {
    let root = temp_dir("nexushub-codex-logs-empty");
    let codex_home = root.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    let logs_path = codex_home.join("logs_2.sqlite");
    seed_codex_logs_db(&logs_path, &[]);

    let mut config = Config::default();
    config.codex.home = codex_home;
    let status =
        ProbeRuntime::new(config, PlatformPaths::for_kind(PlatformKind::Linux)).logs_db_status();

    assert_eq!(status.target, "codex_logs_2");
    assert_eq!(status.total_rows, 0);
    assert_eq!(status.old_rows, 0);
    assert_eq!(status.retained_rows, 0);
}

#[test]
fn probe_logs_db_maintenance_deletes_old_codex_logs_in_chunks() {
    let root = temp_dir("nexushub-codex-logs-maintain");
    let codex_home = root.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    let logs_path = codex_home.join("logs_2.sqlite");
    let now = chrono::Utc::now().timestamp();
    seed_codex_logs_db(
        &logs_path,
        &[now - 500_000, now - 400_000, now - 300_000, now - 100],
    );

    let mut config = Config::default();
    config.codex.home = codex_home;
    config.probe.logs_db.retention_days = 2;
    config.probe.logs_db.delete_chunk_rows = 2;
    config.probe.logs_db.max_delete_rows_per_run = 3;
    let runtime = ProbeRuntime::new(config, PlatformPaths::for_kind(PlatformKind::Linux));

    let result = runtime.maintain_logs_db(false).unwrap();
    assert_eq!(result.status, "ok");
    assert_eq!(result.target, "codex_logs_2");
    assert_eq!(result.deleted_rows, 3);
    assert_eq!(result.old_rows_before, 3);
    assert_eq!(result.remaining_old_rows, 0);

    let status = runtime.logs_db_status();
    assert_eq!(status.total_rows, 1);
    assert_eq!(status.old_rows, 0);
    assert_eq!(status.retained_rows, 1);
}

#[test]
fn probe_logs_db_compaction_vacuums_only_after_quick_check_and_size_gates() {
    let root = temp_dir("nexushub-codex-logs-compact");
    let codex_home = root.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    let logs_path = codex_home.join("logs_2.sqlite");
    let now = chrono::Utc::now().timestamp();
    seed_codex_logs_db(&logs_path, &[now - 300_000, now - 250_000, now - 100]);
    {
        let conn = Connection::open(&logs_path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE bulky_payloads(body BLOB NOT NULL);
            INSERT INTO bulky_payloads(body) VALUES(zeroblob(1048576));
            INSERT INTO bulky_payloads(body) VALUES(zeroblob(1048576));
            DROP TABLE bulky_payloads;
            "#,
        )
        .unwrap();
    }

    let mut config = Config::default();
    config.codex.home = codex_home;
    config.probe.logs_db.retention_days = 2;
    config.probe.logs_db.delete_chunk_rows = 10;
    config.probe.logs_db.max_delete_rows_per_run = 10;
    config.probe.logs_db.compact_min_freelist_mb = 0;
    config.probe.logs_db.compact_min_freelist_ratio_percent = 0;
    config.probe.logs_db.minimum_free_space_mb = 0;
    let runtime = ProbeRuntime::new(config, PlatformPaths::for_kind(PlatformKind::Linux));

    let result = runtime
        .maintain_logs_db_with_compaction(false, true)
        .unwrap();

    assert!(result.ok);
    assert_eq!(result.deleted_rows, 2);
    assert_eq!(result.remaining_old_rows, 0);
    assert_eq!(result.quick_check_before_vacuum.as_deref(), Some("ok"));
    assert!(result.vacuumed);
    assert!(result.skip_reason.is_none());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn probe_logs_db_maintenance_reports_invalid_codex_logs_schema_as_result() {
    let root = temp_dir("nexushub-codex-logs-invalid-maintain");
    let codex_home = root.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    let logs_path = codex_home.join("logs_2.sqlite");
    let conn = Connection::open(&logs_path).unwrap();
    conn.execute_batch("CREATE TABLE not_logs(id INTEGER PRIMARY KEY);")
        .unwrap();

    let mut config = Config::default();
    config.codex.home = codex_home;
    let result = ProbeRuntime::new(config, PlatformPaths::for_kind(PlatformKind::Linux))
        .maintain_logs_db(false)
        .unwrap();

    assert!(!result.ok);
    assert_eq!(result.target, "codex_logs_2");
    assert_eq!(result.status, "missing_logs_table");
    assert_eq!(result.deleted_rows, 0);
    assert!(result
        .error
        .as_deref()
        .is_some_and(|value| !value.is_empty()));
    fs::remove_dir_all(root).unwrap();
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
    assert_eq!(plan.payload["bark_payload"]["title"], "Codex Sentinel Lite");
    assert_eq!(plan.payload["bark_payload"]["body"], "Bark 推送通道正常。");
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

fn seed_codex_logs_db(path: &std::path::Path, timestamps: &[i64]) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL,
            ts_nanos INTEGER NOT NULL,
            level TEXT NOT NULL,
            target TEXT NOT NULL,
            feedback_log_body TEXT,
            module_path TEXT,
            file TEXT,
            line INTEGER,
            thread_id TEXT,
            process_uuid TEXT,
            estimated_bytes INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX idx_logs_ts ON logs(ts DESC, ts_nanos DESC, id DESC);
        "#,
    )
    .unwrap();
    for ts in timestamps {
        conn.execute(
            "INSERT INTO logs(ts, ts_nanos, level, target, estimated_bytes) VALUES(?1, 0, 'INFO', 'test', 1)",
            params![ts],
        )
        .unwrap();
    }
}
