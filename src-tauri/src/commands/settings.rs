#![allow(non_snake_case)]

use crate::{
    overview::DesktopState,
    services::{
        actions::DesktopActionResponse,
        goals::{self as goal_service},
        settings::{
            self as settings_service, DesktopCleanupExecuteRequest, DesktopDeleteUploadRequest,
            DesktopDeleteUploadResponse, DesktopProbeEventsRequest, DesktopProbeEventsResponse,
            DesktopProbeSettings, DesktopUploadFile,
        },
    },
};
use anyhow::Result;
use nexushub_core::services::{
    goals::{GoalGetRequest, GoalUpdateRequest},
    probe as probe_service,
    settings::ProbeSettingsSaveRequest,
};

#[tauri::command(rename = "probe.settings.get")]
pub fn getProbeSettings(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopProbeSettings, String> {
    settings_service::probe_settings_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.settings.save")]
pub fn saveProbeSettings(
    state: tauri::State<'_, DesktopState>,
    settings: ProbeSettingsSaveRequest,
) -> Result<DesktopProbeSettings, String> {
    settings_service::probe_save_settings_with_state(&state, settings)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.barkTest")]
pub fn probeBarkTest(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopActionResponse, String> {
    settings_service::probe_action_with_state(&state, probe_service::ProbeAction::BarkTest)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.installHooks")]
pub fn probeInstallHooks(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopActionResponse, String> {
    settings_service::probe_action_with_state(&state, probe_service::ProbeAction::InstallHooks)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.logsDbDryRun")]
pub fn probeLogsDbDryRun(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopActionResponse, String> {
    settings_service::probe_action_with_state(&state, probe_service::ProbeAction::LogsDbDryRun)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.logsDbExecute")]
pub fn probeLogsDbExecute(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopActionResponse, String> {
    settings_service::probe_action_with_state(&state, probe_service::ProbeAction::LogsDbExecute)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.logsDb.status")]
pub fn getProbeLogsDbStatus(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::probe::ProbeLogsDbStatus, String> {
    Ok(settings_service::probe_logs_db_status_with_state(&state))
}

#[tauri::command(rename = "probe.events")]
pub fn getProbeEvents(
    state: tauri::State<'_, DesktopState>,
    limit: Option<u32>,
) -> Result<DesktopProbeEventsResponse, String> {
    settings_service::probe_events_with_state(&state, DesktopProbeEventsRequest { limit })
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "cleanup.archiveDryRun")]
pub fn dryRunArchiveDelete(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::ArchiveDeletePlan, String> {
    settings_service::archive_delete_dry_run_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command(rename = "cleanup.archiveExecute")]
pub fn startArchiveDelete(
    state: tauri::State<'_, DesktopState>,
    request: DesktopCleanupExecuteRequest,
) -> Result<nexushub_core::archive::ArchiveDeleteResult, String> {
    settings_service::archive_delete_execute_with_state(&state, request)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "cleanup.hiddenDryRun")]
pub fn dryRunHiddenThreadDelete(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::HiddenThreadDeletePlan, String> {
    settings_service::hidden_delete_dry_run_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command(rename = "cleanup.hiddenExecute")]
pub fn startHiddenThreadDelete(
    state: tauri::State<'_, DesktopState>,
    request: DesktopCleanupExecuteRequest,
) -> Result<nexushub_core::archive::HiddenThreadDeleteResult, String> {
    settings_service::hidden_delete_execute_with_state(&state, request)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "uploads.delete")]
pub fn deleteUpload(
    state: tauri::State<'_, DesktopState>,
    id: String,
) -> Result<DesktopDeleteUploadResponse, String> {
    settings_service::delete_upload_with_state(&state, DesktopDeleteUploadRequest { id })
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "uploadFiles")]
pub fn uploadFiles(
    state: tauri::State<'_, DesktopState>,
    files: Vec<DesktopUploadFile>,
) -> Result<nexushub_core::uploads::UploadOutcome, String> {
    settings_service::store_uploads_with_state(&state, files).map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.goal.get")]
pub fn getCodexGoal(
    state: tauri::State<'_, DesktopState>,
    request: GoalGetRequest,
) -> Result<goal_service::DesktopGoalView, String> {
    goal_service::get_goal_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.goal.save")]
pub fn saveCodexGoal(
    state: tauri::State<'_, DesktopState>,
    request: GoalUpdateRequest,
) -> Result<goal_service::DesktopGoalView, String> {
    goal_service::save_goal_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.goal.clear")]
pub fn clearCodexGoal(
    state: tauri::State<'_, DesktopState>,
    request: GoalGetRequest,
) -> Result<goal_service::DesktopGoalView, String> {
    goal_service::clear_goal_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.goal.pause")]
pub fn pauseCodexGoal(
    state: tauri::State<'_, DesktopState>,
    request: GoalGetRequest,
) -> Result<goal_service::DesktopGoalView, String> {
    goal_service::pause_goal_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.goal.resume")]
pub fn resumeCodexGoal(
    state: tauri::State<'_, DesktopState>,
    request: GoalGetRequest,
) -> Result<goal_service::DesktopGoalView, String> {
    goal_service::resume_goal_with_state(&state, request).map_err(|err| err.to_string())
}
