use crate::{overview::DesktopState, services::actions::DesktopActionResponse};
use anyhow::Result;
use nexushub_core::{
    config::{patch_probe_config_toml, Config},
    probe::{redact_probe_event_for_output, ProbeLogsDbMaintenanceResult, ProbeRuntime},
    services::{
        cleanup::{
            self as cleanup_service, ArchiveDeletePlan, ArchiveDeleteResult,
            HiddenThreadDeletePlan, HiddenThreadDeleteResult,
        },
        jobs as job_service, probe as probe_service,
        settings::{self as settings_service, ProbeSettingsSaveRequest},
        uploads as upload_service,
    },
    uploads,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const PROBE_LOGS_DB_LAST_MAINTAIN_SETTING: &str = "probe_logs_db_last_maintain";

pub(crate) type DesktopCleanupExecuteRequest = cleanup_service::CleanupExecuteRequest;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopProbeSettings {
    pub codex: Value,
    pub probe: nexushub_core::config::ProbeConfig,
    pub notifications: Value,
    pub logs_db: nexushub_core::config::ProbeLogsDbConfig,
}

impl From<settings_service::SettingsView> for DesktopProbeSettings {
    fn from(view: settings_service::SettingsView) -> Self {
        Self {
            codex: serde_json::to_value(view.codex).unwrap_or_else(|_| json!({})),
            probe: view.probe,
            notifications: serde_json::to_value(view.notifications).unwrap_or_else(|_| json!({})),
            logs_db: view.logs_db,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopProbeEventsResponse {
    pub events: Vec<nexushub_core::db::ProbeEvent>,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopDeleteUploadResponse {
    pub ok: bool,
    pub deleted: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopUploadFile {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopProbeEventsRequest {
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopDeleteUploadRequest {
    pub id: String,
}

pub(crate) fn store_uploads_with_state(
    state: &DesktopState,
    files: Vec<DesktopUploadFile>,
) -> Result<uploads::UploadOutcome> {
    let root = uploads::upload_root(&state.resolved_codex_paths().home);
    let facade = upload_service::plan_store_uploads_with_capability(
        state.platform(),
        files
            .into_iter()
            .map(|file| upload_service::UploadBatchItem {
                name: file.name,
                mime: Some(file.mime),
                bytes: file.bytes,
            })
            .collect(),
    )?;
    upload_service::store_upload_plan(&root, facade.plan)
}

pub(crate) fn probe_settings_with_state(state: &DesktopState) -> Result<DesktopProbeSettings> {
    let config = state.config();
    let secret_state = settings_service::ProbeSecretState::from_secret_bytes(
        state
            .db
            .get_secret_setting_bytes(settings_service::PROBE_BARK_DEVICE_KEY_SETTING)?
            .as_deref(),
    );
    let plan = settings_service::probe_settings_view_with_capability(
        &config,
        state.platform(),
        secret_state,
    )?;
    Ok(DesktopProbeSettings::from(plan.settings))
}

pub(crate) fn probe_save_settings_with_state(
    state: &DesktopState,
    request: ProbeSettingsSaveRequest,
) -> Result<DesktopProbeSettings> {
    let plan = settings_service::plan_probe_settings_save(state.platform(), request)?;
    let config_path = state.platform().config_file.clone();
    if !config_path.exists() {
        anyhow::bail!("config file not found: {}", config_path.display());
    }
    let text = std::fs::read_to_string(&config_path)?;
    let updated = patch_probe_config_toml(&text, &plan.config_patch)?;
    std::fs::write(&config_path, updated)?;
    let response_config = Config::load(&config_path)?;
    for secret_write in plan.secret_writes {
        state.db.set_secret_setting_bytes(
            &secret_write.setting_key,
            secret_write.secret_value.as_bytes(),
        )?;
    }
    state.replace_config(response_config);
    probe_settings_with_state(state)
}

#[cfg(test)]
pub(crate) fn test_probe_save_settings_with_state(
    state: &DesktopState,
    request: ProbeSettingsSaveRequest,
) -> Result<DesktopProbeSettings> {
    probe_save_settings_with_state(state, request)
}

pub(crate) fn probe_action_with_state(
    state: &DesktopState,
    action: probe_service::ProbeAction,
) -> Result<DesktopActionResponse> {
    let device_key_configured = state
        .db
        .get_secret_setting_bytes(settings_service::PROBE_BARK_DEVICE_KEY_SETTING)?
        .is_some_and(|value| !value.is_empty());
    let plan = probe_service::plan_probe_action_with_device_key(
        &state.config(),
        state.platform(),
        action,
        device_key_configured,
    )?;
    match plan.execution {
        probe_service::ProbeExecutionKind::FixedShellJob => {
            probe_fixed_shell_job_with_state(state, action, plan)
        }
        probe_service::ProbeExecutionKind::LogsDbMaintenance => {
            probe_logs_db_maintain_with_state(state, action, plan)
        }
    }
}

pub(crate) fn probe_logs_db_status_with_state(
    state: &DesktopState,
) -> nexushub_core::probe::ProbeLogsDbStatus {
    ProbeRuntime::new(state.config(), state.platform().clone()).logs_db_status()
}

pub(crate) fn probe_logs_db_maintain_with_state(
    state: &DesktopState,
    action: probe_service::ProbeAction,
    plan: probe_service::ProbeActionPlan,
) -> Result<DesktopActionResponse> {
    let maintenance = plan
        .maintenance
        .ok_or_else(|| anyhow::anyhow!("Probe logs DB action is missing maintenance metadata"))?;
    let job = plan
        .job
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Probe logs DB action is missing job metadata"))?;
    let dry_run = maintenance.dry_run;
    let compact = maintenance.compact;
    let job_id = format!(
        "desktop-probe-logs-db-{}",
        chrono::Utc::now().timestamp_micros()
    );
    state.db.create_job(&job_id, &job.kind, &job.title)?;

    let run = (|| -> Result<ProbeLogsDbMaintenanceResult> {
        let result = ProbeRuntime::new(state.config(), state.platform().clone())
            .maintain_logs_db_with_compaction(dry_run, compact && !dry_run)?;
        state.db.set_setting(
            PROBE_LOGS_DB_LAST_MAINTAIN_SETTING,
            &serde_json::to_string(&result)?,
        )?;
        Ok(result)
    })();

    match run {
        Ok(result) => {
            state.db.append_job_output(
                &job_id,
                &format!("{}\n", serde_json::to_string_pretty(&result)?),
            )?;
            state.db.finish_job(&job_id, "succeeded", Some(0), None)?;
            Ok(ok_action(
                action.as_desktop_command(),
                "Probe logs-db maintenance completed",
                None,
                Some(job_id),
                Some(serde_json::to_value(result)?),
            ))
        }
        Err(err) => {
            let message = err.to_string();
            let _ = state
                .db
                .append_job_output(&job_id, &format!("error: {message}\n"));
            let _ = state.db.finish_job(&job_id, "failed", None, Some(&message));
            Err(err)
        }
    }
}

pub(crate) fn probe_fixed_shell_job_with_state(
    state: &DesktopState,
    action: probe_service::ProbeAction,
    plan: probe_service::ProbeActionPlan,
) -> Result<DesktopActionResponse> {
    let command = action.as_desktop_command();
    let diagnostic_plan = plan.diagnostic_plan.clone();
    let job = plan
        .job
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Probe action is missing job metadata"))?;
    let binary = state.platform().daemon_binary();
    if !binary.is_file() {
        return Ok(unavailable_action(
            command,
            &format!(
                "Probe action requires local nexushubd binary; plan is available but job cannot start: {}",
                binary.display()
            ),
        )
        .with_data(serde_json::to_value(diagnostic_plan)?));
    }
    let job_id = if let Some(group) = job.exclusive_group.as_deref() {
        state
            .jobs
            .start_exclusive_shell_job(&job.kind, &job.title, job.command.clone(), group)?
    } else {
        state
            .jobs
            .start_shell_job(&job.kind, &job.title, job.command.clone())?
    };
    Ok(ok_action(
        command,
        "started local Probe job",
        None,
        Some(job_id),
        Some(serde_json::to_value(diagnostic_plan)?),
    ))
}

pub(crate) fn archive_delete_dry_run_with_state(state: &DesktopState) -> Result<ArchiveDeletePlan> {
    cleanup_service::dry_run_archived_with_capability(state.platform(), &state.codex_paths())
}

pub(crate) fn archive_delete_execute_with_state(
    state: &DesktopState,
    request: DesktopCleanupExecuteRequest,
) -> Result<ArchiveDeleteResult> {
    let paths = state.codex_paths();
    let plan = cleanup_service::plan_cleanup_execute_operation(
        state.platform(),
        cleanup_service::CleanupTarget::Archived,
        request,
    )?;
    let dry_run = cleanup_service::dry_run_archived_with_capability(state.platform(), &paths)?;
    cleanup_service::validate_cleanup_expected_count(&plan, dry_run.archived_threads)?;
    cleanup_service::execute_archived_with_capability(state.platform(), &paths)
}

pub(crate) fn hidden_delete_dry_run_with_state(
    state: &DesktopState,
) -> Result<HiddenThreadDeletePlan> {
    cleanup_service::dry_run_hidden_with_capability(state.platform(), &state.codex_paths())
}

pub(crate) fn hidden_delete_execute_with_state(
    state: &DesktopState,
    request: DesktopCleanupExecuteRequest,
) -> Result<HiddenThreadDeleteResult> {
    let paths = state.codex_paths();
    let plan = cleanup_service::plan_cleanup_execute_operation(
        state.platform(),
        cleanup_service::CleanupTarget::Hidden,
        request,
    )?;
    let dry_run = cleanup_service::dry_run_hidden_with_capability(state.platform(), &paths)?;
    cleanup_service::validate_cleanup_expected_count(&plan, dry_run.hidden_threads)?;
    cleanup_service::execute_hidden_with_capability(state.platform(), &paths)
}

pub(crate) fn probe_events_with_state(
    state: &DesktopState,
    request: DesktopProbeEventsRequest,
) -> Result<DesktopProbeEventsResponse> {
    let limit = request
        .limit
        .unwrap_or(state.config().probe.recent_limit as u32)
        .clamp(1, 500);
    let events = state
        .db
        .list_probe_events(limit)?
        .into_iter()
        .map(redact_probe_event_for_output)
        .collect();
    Ok(DesktopProbeEventsResponse { events, limit })
}

pub(crate) fn delete_upload_with_state(
    state: &DesktopState,
    request: DesktopDeleteUploadRequest,
) -> Result<DesktopDeleteUploadResponse> {
    let root = uploads::upload_root(&state.resolved_codex_paths().home);
    let plan = upload_service::plan_delete_upload_with_capability(state.platform(), request.id)?;
    let deleted = uploads::delete_upload(&root, &plan.id)?;
    Ok(DesktopDeleteUploadResponse { ok: true, deleted })
}

#[cfg(test)]
pub(crate) fn test_delete_upload_with_state(
    state: &DesktopState,
    request: DesktopDeleteUploadRequest,
) -> Result<DesktopDeleteUploadResponse> {
    delete_upload_with_state(state, request)
}

fn ok_action(
    command: &str,
    message: &str,
    thread_id: Option<String>,
    job_id: Option<String>,
    data: Option<Value>,
) -> DesktopActionResponse {
    job_service::action_ok(command, message, thread_id, job_id, data).into()
}

fn unavailable_action(command: &str, message: &str) -> DesktopActionResponse {
    job_service::action_unavailable(command, message).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexushub_core::{
        crypto::SecretBox,
        db::PanelDb,
        platform::{PlatformKind, PlatformPaths},
        services::settings::{ProbeNotificationsSavePatch, ProbeSettingsSavePatch},
    };

    fn command_test_state(kind: PlatformKind) -> (tempfile::TempDir, DesktopState, PlatformPaths) {
        let temp = tempfile::tempdir().unwrap();
        let mut config = Config::for_platform_kind_with_home(kind, temp.path());
        config.paths.data_dir = temp.path().join("data");
        config.paths.db_path = temp.path().join("panel.sqlite");
        config.paths.log_dir = temp.path().join("logs");
        config.codex.home = temp.path().join("codex-home");
        config.codex.workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&config.paths.data_dir).unwrap();
        std::fs::create_dir_all(&config.paths.log_dir).unwrap();
        std::fs::create_dir_all(&config.codex.home).unwrap();
        std::fs::create_dir_all(config.codex.home.join("sessions")).unwrap();
        std::fs::write(config.codex.home.join("session_index.jsonl"), "").unwrap();
        let status = std::process::Command::new("sqlite3")
            .arg(config.codex.home.join("state_5.sqlite"))
            .arg(
                "CREATE TABLE threads(
                    id TEXT PRIMARY KEY,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    title TEXT NOT NULL,
                    archived INTEGER NOT NULL DEFAULT 0,
                    archived_at INTEGER
                );
                CREATE TABLE thread_dynamic_tools(thread_id TEXT NOT NULL);
                CREATE TABLE thread_spawn_edges(parent_thread_id TEXT NOT NULL, child_thread_id TEXT NOT NULL);
                CREATE TABLE agent_job_items(id TEXT PRIMARY KEY, assigned_thread_id TEXT, status TEXT NOT NULL);",
            )
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::create_dir_all(&config.codex.workspace).unwrap();
        let mut platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, temp.path());
        platform.kind = kind;
        std::fs::create_dir_all(platform.config_file.parent().unwrap()).unwrap();
        std::fs::write(&platform.config_file, toml::to_string(&config).unwrap()).unwrap();
        let db =
            PanelDb::open_with_secret_box(&config.paths.db_path, SecretBox::deterministic_dev())
                .unwrap();
        let state = DesktopState::new(config, db, platform.clone());
        (temp, state, platform)
    }

    #[test]
    fn probe_settings_save_command_uses_core_plan_for_normalization_and_secret_write() {
        let (_temp, state, _platform) = command_test_state(PlatformKind::Macos);

        let saved = test_probe_save_settings_with_state(
            &state,
            ProbeSettingsSaveRequest {
                probe: Some(ProbeSettingsSavePatch {
                    poll_seconds: Some(1),
                    recent_limit: Some(999),
                    notifications: Some(ProbeNotificationsSavePatch {
                        server_url: Some(" https://api.day.app ".to_string()),
                        group: Some("  ".to_string()),
                        device_key: Some(" nested-key ".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                notifications: Some(ProbeNotificationsSavePatch {
                    device_key: Some(" top-key ".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(saved.probe.poll_seconds, 5);
        assert_eq!(saved.probe.recent_limit, 500);
        assert_eq!(saved.probe.notifications.server_url, "https://api.day.app");
        assert_eq!(saved.probe.notifications.group, "NexusHub");
        assert_eq!(
            state
                .db
                .get_secret_setting_bytes(settings_service::PROBE_BARK_DEVICE_KEY_SETTING)
                .unwrap()
                .as_deref(),
            Some(b"top-key".as_slice())
        );
    }

    #[test]
    fn probe_settings_save_command_honors_core_capability_gate() {
        let (_temp, state, _platform) = command_test_state(PlatformKind::Windows);

        let err = test_probe_save_settings_with_state(&state, ProbeSettingsSaveRequest::default())
            .unwrap_err()
            .to_string();

        assert!(err.contains("settings is unavailable on windows"), "{err}");
    }

    #[test]
    fn probe_fixed_shell_job_consumes_core_job_command_without_rebuilding_it() {
        let (_temp, state, platform) = command_test_state(PlatformKind::Macos);
        std::fs::create_dir_all(platform.daemon_binary().parent().unwrap()).unwrap();
        std::fs::write(
            platform.daemon_binary(),
            "#!/bin/sh\necho core-command-marker\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(platform.daemon_binary())
                .unwrap()
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(platform.daemon_binary(), permissions).unwrap();
        }
        let core_plan = probe_service::plan_probe_action_with_device_key(
            &state.config(),
            state.platform(),
            probe_service::ProbeAction::BarkTest,
            false,
        )
        .unwrap();
        let expected_command = core_plan.job.as_ref().unwrap().command.clone();

        let response = probe_action_with_state(&state, probe_service::ProbeAction::BarkTest)
            .expect("Probe action should start a fixed job");
        let job_id = response.job_id.expect("job id");
        let job = state.db.job(&job_id).unwrap().expect("job record");

        assert!(response.available);
        assert_eq!(job.kind, core_plan.job.as_ref().unwrap().kind);
        assert_eq!(job.title, core_plan.job.as_ref().unwrap().title);
        assert!(expected_command.contains("--config"));
        assert!(expected_command.contains("probe bark-test"));
    }

    #[test]
    fn settings_command_source_does_not_duplicate_probe_save_normalization() {
        let source = settings_source_before_test_module();

        assert!(
            source.contains("settings_service::plan_probe_settings_save"),
            "probe.settings.save must use the shared core save plan"
        );
        assert!(
            !source.contains("normalize_probe_config_file_patch"),
            "Tauri adapter must not duplicate core Probe settings normalization"
        );
        assert!(
            !source.contains("merge_probe_notification_patch"),
            "Tauri adapter must not merge Probe notification patches itself"
        );
    }

    #[test]
    fn probe_action_source_uses_core_plan_job_command() {
        let source = settings_source_before_test_module();

        assert!(
            source.contains("job.command.clone()"),
            "Probe action execution must pass the core plan job.command into JobRunner"
        );
        assert!(
            !source.contains("fixed_probe_shell_command"),
            "Tauri adapter must not rebuild Probe shell commands"
        );
    }

    #[test]
    fn cleanup_archive_execute_requires_confirmation_before_deleting() {
        let (_temp, state, _platform) = command_test_state(PlatformKind::Macos);

        let err =
            archive_delete_execute_with_state(&state, DesktopCleanupExecuteRequest::default())
                .expect_err("archive cleanup execute must reject unconfirmed requests")
                .to_string();

        assert!(err.contains("confirmed"), "{err}");
        let plan = archive_delete_dry_run_with_state(&state).unwrap();
        assert_eq!(plan.total_threads, 0);
    }

    #[test]
    fn cleanup_hidden_execute_requires_confirmation_before_deleting() {
        let (_temp, state, _platform) = command_test_state(PlatformKind::Macos);

        let err = hidden_delete_execute_with_state(&state, DesktopCleanupExecuteRequest::default())
            .expect_err("hidden cleanup execute must reject unconfirmed requests")
            .to_string();

        assert!(err.contains("confirmed"), "{err}");
        let plan = hidden_delete_dry_run_with_state(&state).unwrap();
        assert_eq!(plan.total_threads, 0);
    }

    #[test]
    fn cleanup_archive_execute_accepts_confirmed_matching_count() {
        let (_temp, state, _platform) = command_test_state(PlatformKind::Macos);

        let result = archive_delete_execute_with_state(
            &state,
            DesktopCleanupExecuteRequest {
                confirmed: true,
                expected_count: Some(0),
            },
        )
        .expect("confirmed archive cleanup with matching count should execute");

        assert_eq!(result.before.archived_threads, 0);
        assert_eq!(result.after_archived_threads, 0);
    }

    #[test]
    fn cleanup_hidden_execute_rejects_stale_expected_count() {
        let (_temp, state, _platform) = command_test_state(PlatformKind::Macos);

        let err = hidden_delete_execute_with_state(
            &state,
            DesktopCleanupExecuteRequest {
                confirmed: true,
                expected_count: Some(1),
            },
        )
        .expect_err("hidden cleanup execute must reject stale dry-run counts")
        .to_string();

        assert!(err.contains("expectedCount mismatch"), "{err}");
    }

    fn settings_source_before_test_module() -> &'static str {
        include_str!("settings.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("settings source before test module")
    }
}
