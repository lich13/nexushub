#![allow(non_snake_case)]

use crate::overview::{
    self, desktop_archive_delete_dry_run_with_state, desktop_archive_delete_execute_with_state,
    desktop_archive_plan_with_state, desktop_clear_goal_with_state,
    desktop_delete_upload_with_state, desktop_hidden_delete_dry_run_with_state,
    desktop_hidden_delete_execute_with_state, desktop_hidden_plan_with_state,
    desktop_pause_goal_with_state, desktop_probe_bark_test_with_state,
    desktop_probe_events_with_state, desktop_probe_hooks_install_with_state,
    desktop_probe_logs_db_maintain_with_state, desktop_probe_save_settings_with_state,
    desktop_probe_settings_with_state, desktop_resume_goal_with_state,
    desktop_save_goal_with_state, desktop_store_uploads_with_state, DesktopActionResponse,
    DesktopDeleteUploadRequest, DesktopDeleteUploadResponse, DesktopGoal, DesktopGoalRequest,
    DesktopLogsDbMaintainRequest, DesktopProbeEventsRequest, DesktopProbeEventsResponse,
    DesktopProbeNotificationsRequest, DesktopProbeSettings, DesktopProbeSettingsPatch,
    DesktopProbeSettingsRequest, DesktopState, DesktopUploadFile,
};
use nexushub_core::services::{goals as goal_service, settings::ProbeSettingsSaveRequest};

#[tauri::command]
pub fn desktop_archive_plan(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::ArchiveDeletePlan, String> {
    desktop_archive_plan_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_hidden_plan(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::HiddenThreadDeletePlan, String> {
    desktop_hidden_plan_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_save_goal_command(request: DesktopGoalRequest) -> Result<DesktopGoal, String> {
    overview::desktop_save_goal(request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_save_goal(
    state: tauri::State<'_, DesktopState>,
    request: DesktopGoalRequest,
) -> Result<DesktopGoal, String> {
    desktop_save_goal_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_clear_goal_command(thread_id: String) -> Result<DesktopGoal, String> {
    overview::desktop_clear_goal(&thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_clear_goal(
    state: tauri::State<'_, DesktopState>,
    thread_id: String,
) -> Result<DesktopGoal, String> {
    desktop_clear_goal_with_state(&state, &thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_pause_goal_command(thread_id: String) -> Result<DesktopGoal, String> {
    overview::desktop_pause_goal(&thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_pause_goal(
    state: tauri::State<'_, DesktopState>,
    thread_id: String,
) -> Result<DesktopGoal, String> {
    desktop_pause_goal_with_state(&state, &thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_resume_goal_command(thread_id: String) -> Result<DesktopGoal, String> {
    overview::desktop_resume_goal(&thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_resume_goal(
    state: tauri::State<'_, DesktopState>,
    thread_id: String,
) -> Result<DesktopGoal, String> {
    desktop_resume_goal_with_state(&state, &thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_probe_settings(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopProbeSettings, String> {
    desktop_probe_settings_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_probe_save_settings(
    state: tauri::State<'_, DesktopState>,
    request: DesktopProbeSettingsRequest,
) -> Result<DesktopProbeSettings, String> {
    desktop_probe_save_settings_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn getProbeSettings(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopProbeSettings, String> {
    desktop_probe_settings_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn saveProbeSettings(
    state: tauri::State<'_, DesktopState>,
    settings: ProbeSettingsSaveRequest,
) -> Result<DesktopProbeSettings, String> {
    let normalized = settings.normalize().map_err(|err| err.to_string())?;
    let request = DesktopProbeSettingsRequest {
        codex: normalized.config_patch.codex,
        probe: normalized
            .config_patch
            .probe
            .map(DesktopProbeSettingsPatch::from),
        notifications: normalized.bark_device_key.map(|device_key| {
            DesktopProbeNotificationsRequest {
                device_key: Some(device_key),
                patch: Default::default(),
            }
        }),
    };
    desktop_probe_save_settings_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_probe_bark_test(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopActionResponse, String> {
    desktop_probe_bark_test_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_probe_hooks_install(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopActionResponse, String> {
    desktop_probe_hooks_install_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_probe_logs_db_maintain(
    state: tauri::State<'_, DesktopState>,
    request: DesktopLogsDbMaintainRequest,
) -> Result<DesktopActionResponse, String> {
    desktop_probe_logs_db_maintain_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn getProbeLogsDbStatus(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::probe::ProbeLogsDbStatus, String> {
    Ok(
        nexushub_core::probe::ProbeRuntime::new(state.config(), state.platform().clone())
            .logs_db_status(),
    )
}

#[tauri::command]
pub fn getProbeEvents(
    state: tauri::State<'_, DesktopState>,
    limit: Option<u32>,
) -> Result<DesktopProbeEventsResponse, String> {
    desktop_probe_events_with_state(&state, DesktopProbeEventsRequest { limit })
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn startProbeJob(
    state: tauri::State<'_, DesktopState>,
    action: String,
) -> Result<DesktopActionResponse, String> {
    match action.as_str() {
        "bark-test" => desktop_probe_bark_test_with_state(&state).map_err(|err| err.to_string()),
        "hooks-install" => {
            desktop_probe_hooks_install_with_state(&state).map_err(|err| err.to_string())
        }
        "logs-db-dry-run" => desktop_probe_logs_db_maintain_with_state(
            &state,
            DesktopLogsDbMaintainRequest {
                dry_run: Some(true),
                compact: Some(false),
            },
        )
        .map_err(|err| err.to_string()),
        "logs-db-execute" => desktop_probe_logs_db_maintain_with_state(
            &state,
            DesktopLogsDbMaintainRequest {
                dry_run: Some(false),
                compact: Some(false),
            },
        )
        .map_err(|err| err.to_string()),
        _ => Err(format!("unknown probe action: {action}")),
    }
}

#[tauri::command]
pub fn desktop_probe_events(
    state: tauri::State<'_, DesktopState>,
    request: DesktopProbeEventsRequest,
) -> Result<DesktopProbeEventsResponse, String> {
    desktop_probe_events_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_archive_delete_dry_run(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::ArchiveDeletePlan, String> {
    desktop_archive_delete_dry_run_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_archive_delete_execute(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::ArchiveDeleteResult, String> {
    desktop_archive_delete_execute_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_hidden_delete_dry_run(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::HiddenThreadDeletePlan, String> {
    desktop_hidden_delete_dry_run_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_hidden_delete_execute(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::HiddenThreadDeleteResult, String> {
    desktop_hidden_delete_execute_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_delete_upload(
    state: tauri::State<'_, DesktopState>,
    request: DesktopDeleteUploadRequest,
) -> Result<DesktopDeleteUploadResponse, String> {
    desktop_delete_upload_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn dryRunArchiveDelete(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::ArchiveDeletePlan, String> {
    desktop_archive_delete_dry_run_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn startArchiveDelete(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::ArchiveDeleteResult, String> {
    desktop_archive_delete_execute_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn dryRunHiddenThreadDelete(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::HiddenThreadDeletePlan, String> {
    desktop_hidden_delete_dry_run_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn startHiddenThreadDelete(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::HiddenThreadDeleteResult, String> {
    desktop_hidden_delete_execute_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn deleteUpload(
    state: tauri::State<'_, DesktopState>,
    id: String,
) -> Result<DesktopDeleteUploadResponse, String> {
    desktop_delete_upload_with_state(&state, DesktopDeleteUploadRequest { id })
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_upload_files_command(
    state: tauri::State<'_, DesktopState>,
    files: Vec<DesktopUploadFile>,
) -> Result<nexushub_core::uploads::UploadOutcome, String> {
    desktop_store_uploads_with_state(&state, files).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn uploadFiles(
    state: tauri::State<'_, DesktopState>,
    files: Vec<DesktopUploadFile>,
) -> Result<nexushub_core::uploads::UploadOutcome, String> {
    desktop_store_uploads_with_state(&state, files).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn getCodexGoal(
    state: tauri::State<'_, DesktopState>,
    thread_id: String,
) -> Result<DesktopGoal, String> {
    let view = match state
        .db
        .get_thread_goal(&thread_id)
        .map_err(|err| err.to_string())?
    {
        Some(goal) => goal_service::goal_response(Some(&goal)),
        None => goal_service::goal_empty("idle"),
    };
    Ok(DesktopGoal {
        available: view.available,
        enabled: view.enabled,
        thread_id: view.thread_id.or(Some(thread_id)),
        objective: view.objective,
        token_budget: view.token_budget,
        status: view.status,
        completed_at: view.completed_at,
        blocked_reason: view.blocked_reason,
    })
}

#[tauri::command]
pub fn saveCodexGoal(
    state: tauri::State<'_, DesktopState>,
    thread_id: String,
    objective: Option<String>,
    token_budget: Option<u64>,
) -> Result<DesktopGoal, String> {
    desktop_save_goal_with_state(
        &state,
        DesktopGoalRequest {
            thread_id,
            objective,
            token_budget,
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn clearCodexGoal(
    state: tauri::State<'_, DesktopState>,
    thread_id: String,
) -> Result<DesktopGoal, String> {
    desktop_clear_goal_with_state(&state, &thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn pauseCodexGoal(
    state: tauri::State<'_, DesktopState>,
    thread_id: String,
) -> Result<DesktopGoal, String> {
    desktop_pause_goal_with_state(&state, &thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn resumeCodexGoal(
    state: tauri::State<'_, DesktopState>,
    thread_id: String,
) -> Result<DesktopGoal, String> {
    desktop_resume_goal_with_state(&state, &thread_id).map_err(|err| err.to_string())
}

impl From<nexushub_core::config::ProbeSettingsPatch> for DesktopProbeSettingsPatch {
    fn from(value: nexushub_core::config::ProbeSettingsPatch) -> Self {
        Self {
            enabled: value.enabled,
            poll_seconds: value.poll_seconds,
            recent_limit: value.recent_limit,
            hooks: value.hooks,
            notifications: value
                .notifications
                .map(|patch| DesktopProbeNotificationsRequest {
                    device_key: None,
                    patch,
                }),
            observability: value.observability,
            logs_db: value.logs_db,
        }
    }
}
