use nexushub_core::{
    config::Config,
    local::local_plugin_catalog,
    platform::{PlatformKind, PlatformPaths},
    probe::{ProbeActionPlanKind, ProbeEventInput, ProbeEventOutcome, ProbeRuntime},
    providers::{AgentProviderId, ProviderRegistry},
    system::system_status_with_paths,
};
use rusqlite::{params, Connection};
use serde_json::json;
use std::{fs, path::PathBuf, time::SystemTime};

#[test]
fn default_config_uses_nexushub_runtime_names() {
    let config = Config::default();

    let platform = PlatformPaths::current();
    assert_eq!(config.paths.data_dir, platform.data_dir);
    assert_eq!(
        config.paths.db_path,
        platform.data_dir.join("nexushub.sqlite")
    );
    assert_eq!(config.paths.webui_dir, platform.webui_dir);
    assert_eq!(config.paths.log_dir, platform.log_dir);
    assert_eq!(
        config.update.panel_update_command,
        "/usr/local/bin/nexushub-webd-update --repo lich13/nexushub --version latest"
    );
}

#[test]
fn linux_default_config_values_stay_cloud_compatible() {
    let config = Config::for_platform_kind(PlatformKind::Linux);

    assert_eq!(
        config.paths.data_dir,
        PathBuf::from("/var/lib/nexushub-webd")
    );
    assert_eq!(
        config.paths.db_path,
        PathBuf::from("/var/lib/nexushub-webd/nexushub.sqlite")
    );
    assert_eq!(
        config.paths.webui_dir,
        PathBuf::from("/usr/share/nexushub-webd/webui")
    );
    assert_eq!(
        config.paths.log_dir,
        PathBuf::from("/var/log/nexushub-webd")
    );
    assert_eq!(
        config.codex.workspace,
        PathBuf::from("/home/ubuntu/codex-workspace")
    );
    assert_eq!(
        config.update.doctor_command,
        "/home/ubuntu/codex-admin/bin/codex-cloud-doctor"
    );
    assert_eq!(
        config.update.panel_precheck_command,
        "/usr/local/bin/nexushub-webd-update --precheck"
    );
}

#[test]
fn macos_default_config_uses_application_support_and_tauri_app_paths() {
    let home = temp_dir("nexushub-macos-default-home");
    fs::create_dir_all(&home).unwrap();
    let config = Config::for_platform_kind_with_home(PlatformKind::Macos, &home);

    assert_eq!(
        config.paths.data_dir,
        home.join("Library/Application Support/NexusHub")
    );
    assert_eq!(
        config.paths.db_path,
        home.join("Library/Application Support/NexusHub/nexushub.sqlite")
    );
    assert_eq!(
        config.paths.webui_dir,
        home.join("Library/Application Support/NexusHub/desktop-assets")
    );
    assert_eq!(config.paths.log_dir, home.join("Library/Logs/NexusHub"));
    assert_eq!(config.codex.home.to_string_lossy(), "auto");
    assert!(config.codex.workspace.starts_with(&home));
    assert!(!config
        .codex
        .workspace
        .to_string_lossy()
        .contains("/home/ubuntu"));
    assert!(!config.update.doctor_command.contains("/home/ubuntu"));
    assert!(config
        .update
        .panel_precheck_command
        .contains("Library/Application Support/NexusHub"));
    assert!(config.update.panel_precheck_command.contains("test -d"));
    assert!(!config.update.panel_precheck_command.contains("launchctl"));
    assert!(!config
        .update
        .panel_precheck_command
        .contains("http://127.0.0.1:15742"));
    assert!(!config.update.panel_precheck_command.contains("systemctl"));
    assert!(!toml::to_string(&config).unwrap().contains("/opt/nexushub"));
    assert!(!toml::to_string(&config).unwrap().contains("/home/ubuntu"));

    fs::remove_dir_all(home).unwrap();
}

#[test]
fn platform_paths_cover_linux_macos_and_windows() {
    let mac_home = temp_dir("nexushub-platform-macos-home");
    fs::create_dir_all(&mac_home).unwrap();

    assert_eq!(
        PlatformPaths::for_kind(PlatformKind::Linux).data_dir,
        PathBuf::from("/var/lib/nexushub-webd")
    );
    let linux = PlatformPaths::for_kind(PlatformKind::Linux);
    assert_eq!(
        linux.config_file,
        PathBuf::from("/etc/nexushub-webd/config.toml")
    );
    assert_eq!(
        linux.webui_dir,
        PathBuf::from("/usr/share/nexushub-webd/webui")
    );
    assert_eq!(linux.log_dir, PathBuf::from("/var/log/nexushub-webd"));
    assert_eq!(linux.service_name, "nexushub-webd");
    assert_eq!(linux.service_kind, "systemd");
    assert_eq!(
        linux.service_file,
        Some(PathBuf::from("/etc/systemd/system/nexushub-webd.service"))
    );
    assert_eq!(
        linux.daemon_binary(),
        PathBuf::from("/usr/local/bin/nexushub-webd")
    );
    let macos = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &mac_home);
    assert_eq!(
        macos.data_dir,
        mac_home.join("Library/Application Support/NexusHub")
    );
    assert_eq!(
        macos.config_file,
        mac_home.join("Library/Application Support/NexusHub/config.toml")
    );
    assert_eq!(
        macos.webui_dir,
        mac_home.join("Library/Application Support/NexusHub/desktop-assets")
    );
    assert_eq!(macos.log_dir, mac_home.join("Library/Logs/NexusHub"));
    assert_eq!(macos.service_file, None);
    assert_eq!(macos.service_name, "NexusHub.app");
    assert_eq!(macos.service_kind, "tauri");
    assert_eq!(
        macos.daemon_binary(),
        mac_home.join("Library/Application Support/NexusHub/bin/nexushub-webd")
    );
    assert_eq!(
        PlatformPaths::for_kind(PlatformKind::Windows).data_dir,
        PathBuf::from(r"%ProgramData%\NexusHub")
    );
    let windows = PlatformPaths::for_kind(PlatformKind::Windows);
    assert_eq!(
        windows.daemon_binary(),
        windows.data_dir.join("bin").join("nexushub-webd.exe")
    );
    fs::remove_dir_all(mac_home).unwrap();
}

#[test]
fn provider_registry_exposes_codex_and_grok_build() {
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

    let grok = providers
        .iter()
        .find(|provider| provider.id == AgentProviderId::GrokBuild)
        .unwrap();
    assert_eq!(grok.status, "ready");
    assert!(grok
        .capabilities
        .iter()
        .any(|capability| capability == "readonly"));
    assert!(grok
        .capabilities
        .iter()
        .any(|capability| capability == "rename"));
    assert!(grok.safety.contains("native rename"));

    assert!(providers
        .iter()
        .any(|provider| provider.id == AgentProviderId::Cursor));
    assert!(providers
        .iter()
        .any(|provider| provider.id == AgentProviderId::Gemini));
}

#[test]
fn local_plugin_catalog_matches_existing_builtin_surface() {
    let plugins = local_plugin_catalog();
    let plugin_json = serde_json::to_value(&plugins).unwrap();

    assert_eq!(plugin_json.as_array().unwrap().len(), 4);
    assert_eq!(plugin_json[0]["id"], "codex");
    assert_eq!(plugin_json[0]["status"], "ready");
    assert_eq!(plugin_json[0]["kind"], "builtin");
    assert_eq!(plugin_json[0]["invocation_template"], "@Codex ");
    assert_eq!(plugin_json[1]["id"], "probe");
    assert_eq!(plugin_json[2]["id"], "grok_build");
    assert_eq!(plugin_json[2]["status"], "ready");
    assert_eq!(plugin_json[2]["invocation_template"], "@Grok Build ");
    assert_eq!(plugin_json[3]["id"], "system_ops");
}

#[tokio::test]
async fn probe_status_is_builtin_and_does_not_require_legacy_cli() {
    let root = temp_dir("nexushub-probe-status");
    let codex_home = root.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    seed_codex_logs_db(&codex_home.join("logs_2.sqlite"), &[]);
    let mut config = Config::default();
    config.codex.home = codex_home;
    config.paths.db_path = root.join("nexushub.sqlite");
    let status = ProbeRuntime::new(config, PlatformPaths::for_kind(PlatformKind::Linux))
        .status()
        .await
        .unwrap();

    assert_eq!(status.label, "Probe");
    assert!(status.enabled);
    assert!(status.available);
    assert_eq!(status.flavor, "builtin");
    assert_eq!(status.service_name, "nexushub-webd");
    assert!(status.binary_path.is_none());
    assert_eq!(status.lifecycle_status, "managed");
    assert_eq!(status.doctor_status, "ready");
    assert_eq!(status.logs_db_status, "ok");
    assert_eq!(
        status.config_path,
        PathBuf::from("/etc/nexushub-webd/config.toml")
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
    assert!(hook_command_step
        .contains("nexushub-webd --config /etc/nexushub-webd/config.toml probe hook-stop"));
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
    let root = temp_dir("nexushub-probe-hook-managed");
    let codex_home = root.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    fs::write(
        codex_home.join("hooks.json"),
        json!({
            "hooks": {
                "Stop": [{
                    "matcher": "*",
                    "hooks": [{
                        "type": "command",
                        "command": "/usr/local/bin/nexushub-webd --config /etc/nexushub-webd/config.toml probe hook-stop"
                    }]
                }],
                "PreToolUse": [{
                    "matcher": "^request_user_input$",
                    "hooks": [{
                        "type": "command",
                        "command": "/usr/local/bin/nexushub-webd --config /etc/nexushub-webd/config.toml probe hook-request-user-input",
                        "timeout": 5
                    }]
                }]
            }
        })
        .to_string(),
    )
    .unwrap();
    let mut config = Config::default();
    config.codex.home = codex_home;
    config.probe.notifications.enabled = true;
    let runtime = ProbeRuntime::new(config, PlatformPaths::for_kind(PlatformKind::Linux));

    let lifecycle = runtime.lifecycle_status();
    assert_eq!(lifecycle.status, "managed");
    assert_eq!(lifecycle.service_name, "nexushub-webd");
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
    assert!(hook.hook_command.contains("/usr/local/bin/nexushub-webd"));
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
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn macos_probe_hook_command_uses_launchd_paths_and_quotes_application_support() {
    let home = temp_dir("nexushub-macos-hook-home");
    fs::create_dir_all(&home).unwrap();
    let config = Config::for_platform_kind_with_home(PlatformKind::Macos, &home);
    let paths = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &home);
    let runtime = ProbeRuntime::new(config, paths);

    let hook = runtime.hook_status();

    assert!(hook.hook_command.contains("probe hook-stop"));
    assert!(hook.hook_command.contains("'"));
    assert!(hook.hook_command.contains(
        &home
            .join("Library/Application Support/NexusHub/config.toml")
            .display()
            .to_string()
    ));
    assert!(!hook.hook_command.contains("/opt/nexushub"));
    assert!(!hook.hook_command.contains("systemctl"));

    fs::remove_dir_all(home).unwrap();
}

#[tokio::test]
async fn macos_system_status_exposes_tauri_app_overview() {
    let home = temp_dir("nexushub-macos-system-home");
    let codex_home = home.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    let mut config = Config::for_platform_kind_with_home(PlatformKind::Macos, &home);
    config.codex.home = codex_home;
    let paths = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &home);

    let status = system_status_with_paths(&config, &paths).await.unwrap();

    assert_eq!(status.platform, PlatformKind::Macos);
    assert_eq!(status.service_kind, "tauri");
    assert_eq!(status.service_name, "NexusHub.app");
    assert_eq!(
        status.config_path,
        home.join("Library/Application Support/NexusHub/config.toml")
            .display()
            .to_string()
    );
    assert_eq!(
        status.webui_dir,
        home.join("Library/Application Support/NexusHub/desktop-assets")
            .display()
            .to_string()
    );
    assert_eq!(status.service_file.as_deref(), None);

    fs::remove_dir_all(home).unwrap();
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
