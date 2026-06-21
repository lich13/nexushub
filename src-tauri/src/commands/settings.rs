#![allow(non_snake_case)]

use crate::overview::{
    DesktopActionResponse, DesktopDeleteUploadRequest, DesktopDeleteUploadResponse, DesktopGoal,
    DesktopGoalRequest, DesktopProbeEventsRequest, DesktopProbeEventsResponse,
    DesktopProbeSettings, DesktopState, DesktopUploadFile,
};
use anyhow::Result;
use nexushub_core::{
    archive::{
        execute_delete_archived, execute_delete_hidden, plan_delete_archived, plan_delete_hidden,
        ArchiveDeletePlan, ArchiveDeleteResult, HiddenThreadDeletePlan, HiddenThreadDeleteResult,
    },
    codex::ThreadSummary,
    config::{patch_probe_config_toml, Config},
    probe::{redact_probe_event_for_output, ProbeLogsDbMaintenanceResult, ProbeRuntime},
    services::{
        goals as goal_service, jobs as job_service, probe as probe_service,
        settings::{self as settings_service, ProbeSettingsSaveRequest},
        uploads as upload_service,
    },
    uploads,
};
use serde_json::Value;

const PROBE_LOGS_DB_LAST_MAINTAIN_SETTING: &str = "probe_logs_db_last_maintain";

#[tauri::command(rename = "probe.settings.get")]
pub fn getProbeSettings(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopProbeSettings, String> {
    probe_settings_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.settings.save")]
pub fn saveProbeSettings(
    state: tauri::State<'_, DesktopState>,
    settings: ProbeSettingsSaveRequest,
) -> Result<DesktopProbeSettings, String> {
    probe_save_settings_with_state(&state, settings).map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.barkTest")]
pub fn probeBarkTest(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopActionResponse, String> {
    probe_action_with_state(&state, probe_service::ProbeAction::BarkTest)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.installHooks")]
pub fn probeInstallHooks(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopActionResponse, String> {
    probe_action_with_state(&state, probe_service::ProbeAction::InstallHooks)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.logsDbDryRun")]
pub fn probeLogsDbDryRun(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopActionResponse, String> {
    probe_action_with_state(&state, probe_service::ProbeAction::LogsDbDryRun)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.logsDbExecute")]
pub fn probeLogsDbExecute(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopActionResponse, String> {
    probe_action_with_state(&state, probe_service::ProbeAction::LogsDbExecute)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.logsDb.status")]
pub fn getProbeLogsDbStatus(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::probe::ProbeLogsDbStatus, String> {
    Ok(ProbeRuntime::new(state.config(), state.platform().clone()).logs_db_status())
}

#[tauri::command(rename = "probe.events")]
pub fn getProbeEvents(
    state: tauri::State<'_, DesktopState>,
    limit: Option<u32>,
) -> Result<DesktopProbeEventsResponse, String> {
    probe_events_with_state(&state, DesktopProbeEventsRequest { limit })
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "cleanup.archiveDryRun")]
pub fn dryRunArchiveDelete(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::ArchiveDeletePlan, String> {
    archive_delete_dry_run_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command(rename = "cleanup.archiveExecute")]
pub fn startArchiveDelete(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::ArchiveDeleteResult, String> {
    archive_delete_execute_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command(rename = "cleanup.hiddenDryRun")]
pub fn dryRunHiddenThreadDelete(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::HiddenThreadDeletePlan, String> {
    hidden_delete_dry_run_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command(rename = "cleanup.hiddenExecute")]
pub fn startHiddenThreadDelete(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::HiddenThreadDeleteResult, String> {
    hidden_delete_execute_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command(rename = "uploads.delete")]
pub fn deleteUpload(
    state: tauri::State<'_, DesktopState>,
    id: String,
) -> Result<DesktopDeleteUploadResponse, String> {
    delete_upload_with_state(&state, DesktopDeleteUploadRequest { id })
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "uploadFiles")]
pub fn uploadFiles(
    state: tauri::State<'_, DesktopState>,
    files: Vec<DesktopUploadFile>,
) -> Result<nexushub_core::uploads::UploadOutcome, String> {
    store_uploads_with_state(&state, files).map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.goal.get")]
pub fn getCodexGoal(
    state: tauri::State<'_, DesktopState>,
    threadId: Option<String>,
    thread_id: Option<String>,
) -> Result<DesktopGoal, String> {
    let thread_id = threadId
        .or(thread_id)
        .ok_or_else(|| "threadId is required".to_string())?;
    let view = match state
        .db
        .get_thread_goal(&thread_id)
        .map_err(|err| err.to_string())?
    {
        Some(goal) => goal_service::goal_response(Some(&goal)),
        None => goal_service::goal_empty("idle"),
    };
    Ok(goal_with_thread_id(view, Some(thread_id)))
}

#[tauri::command(rename = "threads.goal.save")]
pub fn saveCodexGoal(
    state: tauri::State<'_, DesktopState>,
    threadId: Option<String>,
    thread_id: Option<String>,
    objective: Option<String>,
    tokenBudget: Option<u64>,
    token_budget: Option<u64>,
) -> Result<DesktopGoal, String> {
    let thread_id = threadId
        .or(thread_id)
        .ok_or_else(|| "threadId is required".to_string())?;
    save_goal_with_state(
        &state,
        DesktopGoalRequest {
            thread_id,
            objective,
            token_budget: tokenBudget.or(token_budget),
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.goal.clear")]
pub fn clearCodexGoal(
    state: tauri::State<'_, DesktopState>,
    threadId: Option<String>,
    thread_id: Option<String>,
) -> Result<DesktopGoal, String> {
    let thread_id = threadId
        .or(thread_id)
        .ok_or_else(|| "threadId is required".to_string())?;
    clear_goal_with_state(&state, &thread_id).map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.goal.pause")]
pub fn pauseCodexGoal(
    state: tauri::State<'_, DesktopState>,
    threadId: Option<String>,
    thread_id: Option<String>,
) -> Result<DesktopGoal, String> {
    let thread_id = threadId
        .or(thread_id)
        .ok_or_else(|| "threadId is required".to_string())?;
    pause_goal_with_state(&state, &thread_id).map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.goal.resume")]
pub fn resumeCodexGoal(
    state: tauri::State<'_, DesktopState>,
    threadId: Option<String>,
    thread_id: Option<String>,
) -> Result<DesktopGoal, String> {
    let thread_id = threadId
        .or(thread_id)
        .ok_or_else(|| "threadId is required".to_string())?;
    resume_goal_with_state(&state, &thread_id).map_err(|err| err.to_string())
}

pub(crate) fn first_thread_goal(
    config: &Config,
    first_thread: Option<&ThreadSummary>,
) -> DesktopGoal {
    let Some(thread) = first_thread else {
        return goal_from_view(goal_service::goal_empty("missing_thread"));
    };
    get_goal(config, &thread.id).unwrap_or_else(|err| DesktopGoal {
        available: false,
        enabled: false,
        thread_id: Some(thread.id.clone()),
        objective: None,
        token_budget: None,
        status: "unavailable".to_string(),
        completed_at: None,
        blocked_reason: Some(err.to_string()),
    })
}

pub(crate) fn store_uploads_with_state(
    state: &DesktopState,
    files: Vec<DesktopUploadFile>,
) -> Result<uploads::UploadOutcome> {
    let root = uploads::upload_root(&state.resolved_codex_paths().home);
    let plan = upload_service::plan_desktop_batch_uploads(
        files
            .into_iter()
            .map(|file| upload_service::UploadBatchItem {
                name: file.name,
                mime: Some(file.mime),
                bytes: file.bytes,
            })
            .collect(),
    )?;
    upload_service::store_upload_plan(&root, plan)
}

fn save_goal_with_state(state: &DesktopState, request: DesktopGoalRequest) -> Result<DesktopGoal> {
    let plan = goal_service::plan_save_goal(goal_service::GoalUpdateRequest {
        thread_id: Some(request.thread_id),
        objective: request.objective,
        token_budget: request.token_budget,
        status: None,
        enabled: None,
    })?;
    upsert_goal_with_state(state, plan.as_thread_goal_update())
}

fn clear_goal_with_state(state: &DesktopState, thread_id: &str) -> Result<DesktopGoal> {
    let plan = goal_service::plan_clear_goal(thread_id)?;
    upsert_goal_with_state(state, plan.as_thread_goal_update())
}

fn pause_goal_with_state(state: &DesktopState, thread_id: &str) -> Result<DesktopGoal> {
    update_existing_goal_status_with_state(state, thread_id, "paused")
}

fn resume_goal_with_state(state: &DesktopState, thread_id: &str) -> Result<DesktopGoal> {
    update_existing_goal_status_with_state(state, thread_id, "active")
}

fn probe_settings_with_state(state: &DesktopState) -> Result<DesktopProbeSettings> {
    let config = state.config();
    let secret_state = settings_service::ProbeSecretState::from_secret_bytes(
        state
            .db
            .get_secret_setting_bytes(settings_service::PROBE_BARK_DEVICE_KEY_SETTING)?
            .as_deref(),
    );
    Ok(DesktopProbeSettings::from(
        settings_service::build_settings_view(&config, secret_state),
    ))
}

fn probe_save_settings_with_state(
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
    if let Some(device_key) = plan.bark_device_key {
        state.db.set_secret_setting_bytes(
            settings_service::PROBE_BARK_DEVICE_KEY_SETTING,
            device_key.as_bytes(),
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

fn probe_action_with_state(
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

fn probe_logs_db_maintain_with_state(
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

fn probe_fixed_shell_job_with_state(
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

fn archive_delete_dry_run_with_state(state: &DesktopState) -> Result<ArchiveDeletePlan> {
    plan_delete_archived(&state.codex_paths())
}

fn archive_delete_execute_with_state(state: &DesktopState) -> Result<ArchiveDeleteResult> {
    execute_delete_archived(&state.codex_paths())
}

fn hidden_delete_dry_run_with_state(state: &DesktopState) -> Result<HiddenThreadDeletePlan> {
    plan_delete_hidden(&state.codex_paths())
}

fn hidden_delete_execute_with_state(state: &DesktopState) -> Result<HiddenThreadDeleteResult> {
    execute_delete_hidden(&state.codex_paths())
}

fn probe_events_with_state(
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

fn delete_upload_with_state(
    state: &DesktopState,
    request: DesktopDeleteUploadRequest,
) -> Result<DesktopDeleteUploadResponse> {
    let root = uploads::upload_root(&state.resolved_codex_paths().home);
    let deleted = uploads::delete_upload(&root, &request.id)?;
    Ok(DesktopDeleteUploadResponse { ok: true, deleted })
}

#[cfg(test)]
pub(crate) fn test_delete_upload_with_state(
    state: &DesktopState,
    request: DesktopDeleteUploadRequest,
) -> Result<DesktopDeleteUploadResponse> {
    delete_upload_with_state(state, request)
}

fn get_goal(config: &Config, thread_id: &str) -> Result<DesktopGoal> {
    let db = crate::overview::open_panel_db(config)?;
    let Some(goal) = db.get_thread_goal(thread_id)? else {
        return Ok(goal_with_thread_id(
            goal_service::goal_empty("idle"),
            Some(thread_id.to_string()),
        ));
    };
    Ok(goal_response(&goal))
}

fn upsert_goal_with_state(
    state: &DesktopState,
    update: nexushub_core::db::ThreadGoalUpdate<'_>,
) -> Result<DesktopGoal> {
    let goal = state.db.upsert_thread_goal(update)?;
    Ok(goal_response(&goal))
}

fn update_existing_goal_status_with_state(
    state: &DesktopState,
    thread_id: &str,
    status: &'static str,
) -> Result<DesktopGoal> {
    let existing = state.db.get_thread_goal(thread_id)?;
    let plan = goal_service::plan_goal_status_for_thread(thread_id, existing.as_ref(), status)?;
    let goal = state.db.upsert_thread_goal(plan.as_thread_goal_update())?;
    Ok(goal_response(&goal))
}

pub(crate) fn goal_response(goal: &nexushub_core::db::ThreadGoal) -> DesktopGoal {
    goal_from_view(goal_service::goal_response(Some(goal)))
}

fn goal_from_view(view: goal_service::GoalView) -> DesktopGoal {
    goal_with_thread_id(view, None)
}

fn goal_with_thread_id(view: goal_service::GoalView, thread_id: Option<String>) -> DesktopGoal {
    DesktopGoal {
        available: view.available,
        enabled: view.enabled,
        thread_id: view.thread_id.or(thread_id),
        objective: view.objective,
        token_budget: view.token_budget,
        status: view.status,
        completed_at: view.completed_at,
        blocked_reason: view.blocked_reason,
    }
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

    fn settings_source_before_test_module() -> &'static str {
        include_str!("settings.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("settings source before test module")
    }
}
