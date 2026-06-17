mod desktop_api;
mod overview;

use tauri::Manager;

use desktop_api::{DesktopApiRequest, DesktopApiState, DesktopApiUpload};
pub use overview::{
    build_desktop_home, build_desktop_home_with_state, build_desktop_overview,
    desktop_answer_elicitation_with_state, desktop_archive_delete_dry_run_with_state,
    desktop_archive_plan_with_state, desktop_archive_thread_with_state,
    desktop_cancel_followup_with_state, desktop_clear_goal_with_state,
    desktop_continue_thread_with_state, desktop_enqueue_followup_with_state,
    desktop_fork_thread_with_state, desktop_hidden_delete_dry_run_with_state,
    desktop_hidden_plan_with_state, desktop_job_detail_with_state, desktop_jobs_with_state,
    desktop_list_followups_with_state, desktop_open_config_dir, desktop_open_log_dir,
    desktop_pause_goal_with_state, desktop_plan_accept_with_state, desktop_plan_revise_with_state,
    desktop_platform_status_with_state, desktop_probe_bark_test_with_state,
    desktop_probe_logs_db_maintain_with_state, desktop_probe_save_settings_with_state,
    desktop_probe_settings_with_state, desktop_probe_status_with_state,
    desktop_rename_thread_with_state, desktop_restore_thread_with_state,
    desktop_resume_goal_with_state, desktop_save_goal_with_state,
    desktop_security_status_with_state, desktop_send_message_with_state,
    desktop_stop_thread_with_state, desktop_thread_blocks_with_state,
    desktop_thread_detail_with_state, desktop_threads_with_state, nexus_paths_for_home,
    DesktopActionResponse, DesktopCancelFollowupRequest, DesktopElicitationAnswerRequest,
    DesktopFollowupRequest, DesktopGoal, DesktopGoalRequest, DesktopHome, DesktopJobDetailRequest,
    DesktopJobResponse, DesktopJobsRequest, DesktopLogsDbMaintainRequest, DesktopOverview,
    DesktopPlanAcceptRequest, DesktopPlanReviseRequest, DesktopProbeSettings,
    DesktopProbeSettingsRequest, DesktopRenameThreadRequest, DesktopSecurityStatus,
    DesktopSendMessageRequest, DesktopState, DesktopStopRequest, DesktopThreadBlockPage,
    DesktopThreadIdRequest, NexusPaths, ThreadBlocksRequest, ThreadDetailRequest,
    ThreadListRequest,
};

#[tauri::command]
fn desktop_overview() -> Result<DesktopOverview, String> {
    build_desktop_overview().map_err(|err| err.to_string())
}

#[tauri::command]
async fn desktop_home() -> Result<DesktopHome, String> {
    build_desktop_home().await.map_err(|err| err.to_string())
}

#[tauri::command]
async fn desktop_home_native(state: tauri::State<'_, DesktopState>) -> Result<DesktopHome, String> {
    build_desktop_home_with_state(&state)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_threads_command(
    request: ThreadListRequest,
) -> Result<Vec<nexushub_core::codex::ThreadSummary>, String> {
    overview::desktop_threads(request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_threads(
    state: tauri::State<'_, DesktopState>,
    request: ThreadListRequest,
) -> Result<Vec<nexushub_core::codex::ThreadSummary>, String> {
    desktop_threads_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_thread_detail_command(
    id: String,
) -> Result<Option<nexushub_core::codex::ThreadDetail>, String> {
    overview::desktop_thread_detail(&id).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_thread_detail(
    state: tauri::State<'_, DesktopState>,
    request: ThreadDetailRequest,
) -> Result<Option<nexushub_core::codex::ThreadDetail>, String> {
    desktop_thread_detail_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_thread_blocks(
    state: tauri::State<'_, DesktopState>,
    request: ThreadBlocksRequest,
) -> Result<Option<DesktopThreadBlockPage>, String> {
    desktop_thread_blocks_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_send_message(
    state: tauri::State<'_, DesktopState>,
    request: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    desktop_send_message_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_continue_thread(
    state: tauri::State<'_, DesktopState>,
    request: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    desktop_continue_thread_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_stop_thread(
    state: tauri::State<'_, DesktopState>,
    request: DesktopStopRequest,
) -> Result<DesktopActionResponse, String> {
    desktop_stop_thread_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_plan_accept(
    state: tauri::State<'_, DesktopState>,
    request: DesktopPlanAcceptRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    desktop_plan_accept_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_plan_revise(
    state: tauri::State<'_, DesktopState>,
    request: DesktopPlanReviseRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    desktop_plan_revise_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_answer_elicitation(
    state: tauri::State<'_, DesktopState>,
    request: DesktopElicitationAnswerRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    desktop_answer_elicitation_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
async fn desktop_probe_status_command() -> Result<nexushub_core::probe::ProbeStatus, String> {
    overview::desktop_probe_status()
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn desktop_probe_status(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::probe::ProbeStatus, String> {
    desktop_probe_status_with_state(&state)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_archive_plan_command() -> Result<nexushub_core::archive::ArchiveDeletePlan, String> {
    overview::desktop_archive_plan().map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_hidden_plan_command() -> Result<nexushub_core::archive::HiddenThreadDeletePlan, String> {
    overview::desktop_hidden_plan().map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_archive_plan(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::ArchiveDeletePlan, String> {
    desktop_archive_plan_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_hidden_plan(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::HiddenThreadDeletePlan, String> {
    desktop_hidden_plan_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_save_goal_command(request: DesktopGoalRequest) -> Result<DesktopGoal, String> {
    overview::desktop_save_goal(request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_save_goal(
    state: tauri::State<'_, DesktopState>,
    request: DesktopGoalRequest,
) -> Result<DesktopGoal, String> {
    desktop_save_goal_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_clear_goal_command(thread_id: String) -> Result<DesktopGoal, String> {
    overview::desktop_clear_goal(&thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_clear_goal(
    state: tauri::State<'_, DesktopState>,
    thread_id: String,
) -> Result<DesktopGoal, String> {
    desktop_clear_goal_with_state(&state, &thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_pause_goal_command(thread_id: String) -> Result<DesktopGoal, String> {
    overview::desktop_pause_goal(&thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_pause_goal(
    state: tauri::State<'_, DesktopState>,
    thread_id: String,
) -> Result<DesktopGoal, String> {
    desktop_pause_goal_with_state(&state, &thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_resume_goal_command(thread_id: String) -> Result<DesktopGoal, String> {
    overview::desktop_resume_goal(&thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_resume_goal(
    state: tauri::State<'_, DesktopState>,
    thread_id: String,
) -> Result<DesktopGoal, String> {
    desktop_resume_goal_with_state(&state, &thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_archive_thread(
    state: tauri::State<'_, DesktopState>,
    request: DesktopThreadIdRequest,
) -> Result<DesktopActionResponse, String> {
    desktop_archive_thread_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_restore_thread(
    state: tauri::State<'_, DesktopState>,
    request: DesktopThreadIdRequest,
) -> Result<DesktopActionResponse, String> {
    desktop_restore_thread_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_rename_thread(
    state: tauri::State<'_, DesktopState>,
    request: DesktopRenameThreadRequest,
) -> Result<DesktopActionResponse, String> {
    desktop_rename_thread_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_fork_thread(request: DesktopThreadIdRequest) -> DesktopActionResponse {
    desktop_fork_thread_with_state(request)
}

#[tauri::command]
fn desktop_probe_settings(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopProbeSettings, String> {
    desktop_probe_settings_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_probe_save_settings(
    state: tauri::State<'_, DesktopState>,
    request: DesktopProbeSettingsRequest,
) -> Result<DesktopProbeSettings, String> {
    desktop_probe_save_settings_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_probe_bark_test(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopActionResponse, String> {
    desktop_probe_bark_test_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_probe_logs_db_maintain(
    state: tauri::State<'_, DesktopState>,
    request: DesktopLogsDbMaintainRequest,
) -> Result<nexushub_core::probe::ProbeLogsDbMaintenanceResult, String> {
    desktop_probe_logs_db_maintain_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_archive_delete_dry_run(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::ArchiveDeletePlan, String> {
    desktop_archive_delete_dry_run_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_hidden_delete_dry_run(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::HiddenThreadDeletePlan, String> {
    desktop_hidden_delete_dry_run_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_jobs(
    state: tauri::State<'_, DesktopState>,
    request: DesktopJobsRequest,
) -> Result<Vec<DesktopJobResponse>, String> {
    desktop_jobs_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_job_detail(
    state: tauri::State<'_, DesktopState>,
    request: DesktopJobDetailRequest,
) -> Result<Option<DesktopJobResponse>, String> {
    desktop_job_detail_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_list_followups(
    state: tauri::State<'_, DesktopState>,
    request: DesktopFollowupRequest,
) -> Result<Vec<nexushub_core::db::ThreadFollowUp>, String> {
    desktop_list_followups_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_enqueue_followup(
    state: tauri::State<'_, DesktopState>,
    request: DesktopSendMessageRequest,
) -> Result<nexushub_core::db::ThreadFollowUp, String> {
    desktop_enqueue_followup_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_cancel_followup(
    state: tauri::State<'_, DesktopState>,
    request: DesktopCancelFollowupRequest,
) -> Result<DesktopActionResponse, String> {
    desktop_cancel_followup_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_security_status(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopSecurityStatus, String> {
    desktop_security_status_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
async fn desktop_platform_status(
    state: tauri::State<'_, DesktopState>,
) -> Result<
    (
        nexushub_core::platform::PlatformPaths,
        Option<nexushub_core::system::SystemStatus>,
    ),
    String,
> {
    desktop_platform_status_with_state(&state)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_claude_code_overview() -> Result<nexushub_core::claude_code::ClaudeOverview, String> {
    overview::desktop_claude_code_overview().map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_open_config_dir_command() -> Result<(), String> {
    desktop_open_config_dir().map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_open_log_dir_command() -> Result<(), String> {
    desktop_open_log_dir().map_err(|err| err.to_string())
}

#[tauri::command]
async fn desktop_api_command(
    state: tauri::State<'_, DesktopApiState>,
    request: DesktopApiRequest,
) -> Result<serde_json::Value, String> {
    desktop_api::handle_desktop_api(&state, request)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_upload_files_command(
    state: tauri::State<'_, DesktopApiState>,
    files: Vec<DesktopApiUpload>,
) -> Result<nexushub_core::uploads::UploadOutcome, String> {
    desktop_api::store_desktop_uploads(&state, files).map_err(|err| err.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            desktop_overview,
            desktop_home,
            desktop_home_native,
            desktop_threads_command,
            desktop_threads,
            desktop_thread_detail_command,
            desktop_thread_detail,
            desktop_thread_blocks,
            desktop_send_message,
            desktop_continue_thread,
            desktop_stop_thread,
            desktop_plan_accept,
            desktop_plan_revise,
            desktop_answer_elicitation,
            desktop_probe_status_command,
            desktop_probe_status,
            desktop_archive_plan_command,
            desktop_archive_plan,
            desktop_hidden_plan_command,
            desktop_hidden_plan,
            desktop_save_goal_command,
            desktop_save_goal,
            desktop_clear_goal_command,
            desktop_clear_goal,
            desktop_pause_goal_command,
            desktop_pause_goal,
            desktop_resume_goal_command,
            desktop_resume_goal,
            desktop_archive_thread,
            desktop_restore_thread,
            desktop_rename_thread,
            desktop_fork_thread,
            desktop_probe_settings,
            desktop_probe_save_settings,
            desktop_probe_bark_test,
            desktop_probe_logs_db_maintain,
            desktop_archive_delete_dry_run,
            desktop_hidden_delete_dry_run,
            desktop_jobs,
            desktop_job_detail,
            desktop_list_followups,
            desktop_enqueue_followup,
            desktop_cancel_followup,
            desktop_security_status,
            desktop_platform_status,
            desktop_claude_code_overview,
            desktop_open_config_dir_command,
            desktop_open_log_dir_command,
            desktop_api_command,
            desktop_upload_files_command
        ])
        .setup(|app| {
            let state = DesktopApiState::new().map_err(|err| err.to_string())?;
            app.manage(state);
            let state = DesktopState::current().map_err(|err| err.to_string())?;
            app.manage(state);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.maximize();
                let _ = window.show();
                let _ = window.set_focus();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run NexusHub desktop app");
}
