mod commands;
// Probe status typed command 已迁移到 commands::probe；overview 仍保留旧兼容 helper。
#[allow(dead_code)]
mod overview;

use std::path::Path;
use tauri::Manager;

pub use commands::{
    probe::desktop_probe_status_with_state, updates::desktop_update_status_with_state,
};
pub use overview::{
    build_desktop_home, build_desktop_home_with_state, build_desktop_overview,
    desktop_answer_elicitation_with_state, desktop_archive_delete_dry_run_with_state,
    desktop_archive_delete_execute_with_state, desktop_archive_plan_with_state,
    desktop_archive_thread_with_state, desktop_cancel_followup_with_state,
    desktop_clear_goal_with_state, desktop_continue_thread_with_state,
    desktop_delete_upload_with_state, desktop_enqueue_followup_with_state,
    desktop_fork_thread_with_state, desktop_hidden_delete_dry_run_with_state,
    desktop_hidden_delete_execute_with_state, desktop_hidden_plan_with_state,
    desktop_job_detail_with_state, desktop_jobs_with_state, desktop_list_followups_with_state,
    desktop_open_config_dir, desktop_open_log_dir, desktop_pause_goal_with_state,
    desktop_plan_accept_with_state, desktop_plan_revise_with_state,
    desktop_platform_status_with_state, desktop_probe_bark_test_with_state,
    desktop_probe_events_with_state, desktop_probe_hooks_install_with_state,
    desktop_probe_logs_db_maintain_with_state, desktop_probe_save_settings_with_state,
    desktop_probe_settings_with_state, desktop_rename_thread_with_state,
    desktop_restore_thread_with_state, desktop_resume_goal_with_state,
    desktop_save_goal_with_state, desktop_security_status_with_state,
    desktop_send_message_with_state, desktop_stop_thread_with_state,
    desktop_store_uploads_with_state, desktop_thread_blocks_with_state,
    desktop_thread_detail_with_state, desktop_threads_with_state, nexus_paths_for_home,
    DesktopActionResponse, DesktopCancelFollowupRequest, DesktopDeleteUploadRequest,
    DesktopDeleteUploadResponse, DesktopElicitationAnswerRequest, DesktopFollowupRequest,
    DesktopGoal, DesktopGoalRequest, DesktopHome, DesktopJobDetailRequest, DesktopJobResponse,
    DesktopJobsRequest, DesktopLogsDbMaintainRequest, DesktopOverview, DesktopPlanAcceptRequest,
    DesktopPlanReviseRequest, DesktopProbeEventsRequest, DesktopProbeEventsResponse,
    DesktopProbeSettings, DesktopProbeSettingsRequest, DesktopRenameThreadRequest,
    DesktopSecurityStatus, DesktopSendMessageRequest, DesktopState, DesktopStopRequest,
    DesktopThreadBlockPage, DesktopThreadIdRequest, DesktopUploadFile, NexusPaths,
    ThreadBlocksRequest, ThreadDetailRequest, ThreadListRequest,
};

const NEXUSHUBD_RESOURCE_NAME: &str = "nexushubd";
const NEXUSHUBD_HELPER_PLACEHOLDER: &[u8] = b"NEXUSHUB_HELPER_PLACEHOLDER";
const WEBUI_RESOURCE_NAME: &str = "webui";

fn sync_nexushubd_helper_from_resource(resource_dir: &Path) -> Result<(), String> {
    let source = resource_dir.join(NEXUSHUBD_RESOURCE_NAME);
    if !source.is_file() {
        return Ok(());
    }
    if is_nexushubd_helper_placeholder(&source).map_err(|err| err.to_string())? {
        return Ok(());
    }
    let platform = nexushub_core::platform::PlatformPaths::current();
    let target = platform.daemon_binary();
    sync_nexushubd_helper_file(&source, &target).map_err(|err| err.to_string())
}

fn prepare_macos_webui_assets_from_resource(resource_dir: &Path) -> Result<(), String> {
    let source = resource_dir.join(WEBUI_RESOURCE_NAME);
    if !source.join("index.html").is_file() {
        return Ok(());
    }

    let platform = nexushub_core::platform::PlatformPaths::current();
    sync_directory(&source, &platform.webui_dir).map_err(|err| err.to_string())?;
    remove_legacy_webui_dir(&platform).map_err(|err| err.to_string())?;
    migrate_macos_webui_dir_config(&platform).map_err(|err| err.to_string())
}

fn remove_legacy_webui_dir(
    platform: &nexushub_core::platform::PlatformPaths,
) -> std::io::Result<()> {
    let legacy = platform.data_dir.join("webui");
    if legacy != platform.webui_dir && legacy.is_dir() {
        std::fs::remove_dir_all(legacy)?;
    }
    Ok(())
}

fn migrate_macos_webui_dir_config(
    platform: &nexushub_core::platform::PlatformPaths,
) -> anyhow::Result<()> {
    let config_path = &platform.config_file;
    if !config_path.is_file() {
        return Ok(());
    }
    let text = std::fs::read_to_string(config_path)?;
    let mut value = text.parse::<toml::Value>()?;
    let Some(paths) = value.get_mut("paths").and_then(toml::Value::as_table_mut) else {
        return Ok(());
    };
    let data_dir = paths
        .get("data_dir")
        .and_then(toml::Value::as_str)
        .map(Path::new);
    if data_dir != Some(platform.data_dir.as_path()) {
        return Ok(());
    }
    let webui_dir = paths
        .get("webui_dir")
        .and_then(toml::Value::as_str)
        .map(Path::new);
    if webui_dir == Some(platform.webui_dir.as_path()) {
        return Ok(());
    }
    paths.insert(
        "webui_dir".to_string(),
        toml::Value::String(platform.webui_dir.display().to_string()),
    );
    std::fs::write(config_path, toml::to_string_pretty(&value)?)?;
    Ok(())
}

fn is_nexushubd_helper_placeholder(path: &Path) -> std::io::Result<bool> {
    let bytes = std::fs::read(path)?;
    Ok(bytes.starts_with(NEXUSHUBD_HELPER_PLACEHOLDER))
}

fn sync_nexushubd_helper_file(source: &Path, target: &Path) -> std::io::Result<()> {
    let should_copy = match (std::fs::metadata(source), std::fs::metadata(target)) {
        (Ok(source_meta), Ok(target_meta)) => {
            source_meta.len() != target_meta.len()
                || source_meta.modified().ok() != target_meta.modified().ok()
        }
        (Ok(_), Err(_)) => true,
        (Err(err), _) => return Err(err),
    };
    if !should_copy {
        ensure_executable(target)?;
        return Ok(());
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source, target)?;
    ensure_executable(target)
}

fn sync_directory(source: &Path, target: &Path) -> std::io::Result<()> {
    if target.exists() {
        std::fs::remove_dir_all(target)?;
    }
    copy_directory_recursive(source, target)
}

fn copy_directory_recursive(source: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_directory_recursive(&source_path, &target_path)?;
        } else if file_type.is_file() {
            std::fs::copy(source_path, target_path)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn ensure_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::metadata(path)?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(permissions.mode() | 0o755);
    std::fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn ensure_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

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
    commands::probe::desktop_probe_status()
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
async fn desktop_probe_status(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::probe::ProbeStatus, String> {
    commands::probe::desktop_probe_status_with_state(&state)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_update_status(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::services::updates::UpdateStatus, String> {
    commands::updates::desktop_update_status_with_state(&state, None, None)
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
fn desktop_probe_hooks_install(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopActionResponse, String> {
    desktop_probe_hooks_install_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_probe_logs_db_maintain(
    state: tauri::State<'_, DesktopState>,
    request: DesktopLogsDbMaintainRequest,
) -> Result<DesktopActionResponse, String> {
    desktop_probe_logs_db_maintain_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_probe_events(
    state: tauri::State<'_, DesktopState>,
    request: DesktopProbeEventsRequest,
) -> Result<DesktopProbeEventsResponse, String> {
    desktop_probe_events_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_archive_delete_dry_run(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::ArchiveDeletePlan, String> {
    desktop_archive_delete_dry_run_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_archive_delete_execute(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::ArchiveDeleteResult, String> {
    desktop_archive_delete_execute_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_hidden_delete_dry_run(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::HiddenThreadDeletePlan, String> {
    desktop_hidden_delete_dry_run_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_hidden_delete_execute(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::HiddenThreadDeleteResult, String> {
    desktop_hidden_delete_execute_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_delete_upload(
    state: tauri::State<'_, DesktopState>,
    request: DesktopDeleteUploadRequest,
) -> Result<DesktopDeleteUploadResponse, String> {
    desktop_delete_upload_with_state(&state, request).map_err(|err| err.to_string())
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
fn desktop_upload_files_command(
    state: tauri::State<'_, DesktopState>,
    files: Vec<DesktopUploadFile>,
) -> Result<nexushub_core::uploads::UploadOutcome, String> {
    desktop_store_uploads_with_state(&state, files).map_err(|err| err.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            desktop_update_status,
            commands::updates::check_update_status,
            commands::updates::install_update_and_restart,
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
            desktop_probe_hooks_install,
            desktop_probe_logs_db_maintain,
            desktop_probe_events,
            desktop_archive_delete_dry_run,
            desktop_archive_delete_execute,
            desktop_hidden_delete_dry_run,
            desktop_hidden_delete_execute,
            desktop_delete_upload,
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
            desktop_upload_files_command
        ])
        .setup(|app| {
            if let Ok(resource_dir) = app.path().resource_dir() {
                sync_nexushubd_helper_from_resource(&resource_dir)?;
                prepare_macos_webui_assets_from_resource(&resource_dir)?;
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_nexushubd_helper_file_copies_and_marks_executable() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("nexushubd");
        let target = temp
            .path()
            .join("Application Support/NexusHub/bin/nexushubd");
        std::fs::write(&source, b"#!/bin/sh\nexit 0\n").unwrap();

        sync_nexushubd_helper_file(&source, &target).unwrap();

        assert_eq!(std::fs::read(&target).unwrap(), b"#!/bin/sh\nexit 0\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&target).unwrap().permissions().mode();
            assert_ne!(mode & 0o111, 0, "helper must be executable");
        }
    }

    #[test]
    fn helper_placeholder_detection_prevents_dev_resource_sync() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("nexushubd");
        std::fs::write(&source, b"NEXUSHUB_HELPER_PLACEHOLDER\nnot a binary\n").unwrap();

        assert!(is_nexushubd_helper_placeholder(&source).unwrap());
    }

    #[test]
    fn sync_directory_replaces_stale_webui_assets() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("resource-webui");
        let target = temp.path().join("desktop-assets");
        std::fs::create_dir_all(source.join("assets")).unwrap();
        std::fs::create_dir_all(target.join("assets")).unwrap();
        std::fs::write(
            source.join("index.html"),
            "<script src=\"/assets/new.js\"></script>",
        )
        .unwrap();
        std::fs::write(source.join("assets/new.js"), "new").unwrap();
        std::fs::write(target.join("assets/old.js"), "old").unwrap();

        sync_directory(&source, &target).unwrap();

        assert_eq!(
            std::fs::read_to_string(target.join("index.html")).unwrap(),
            "<script src=\"/assets/new.js\"></script>"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("assets/new.js")).unwrap(),
            "new"
        );
        assert!(!target.join("assets/old.js").exists());
    }

    #[test]
    fn migrate_macos_webui_dir_config_moves_legacy_webui_path() {
        let temp = tempfile::tempdir().unwrap();
        let platform = nexushub_core::platform::PlatformPaths::for_kind_with_home(
            nexushub_core::platform::PlatformKind::Macos,
            temp.path(),
        );
        std::fs::create_dir_all(&platform.data_dir).unwrap();
        let legacy_webui = platform.data_dir.join("webui");
        let config = format!(
            r#"
[paths]
data_dir = "{}"
db_path = "{}"
webui_dir = "{}"
log_dir = "{}"
"#,
            platform.data_dir.display(),
            platform.data_dir.join("nexushub.sqlite").display(),
            legacy_webui.display(),
            platform.log_dir.display()
        );
        std::fs::write(&platform.config_file, config).unwrap();

        migrate_macos_webui_dir_config(&platform).unwrap();

        let migrated = std::fs::read_to_string(&platform.config_file).unwrap();
        assert!(migrated.contains(&format!("webui_dir = \"{}\"", platform.webui_dir.display())));
        assert!(!migrated.contains(&format!("webui_dir = \"{}\"", legacy_webui.display())));
    }
}
