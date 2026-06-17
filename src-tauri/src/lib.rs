mod overview;

use tauri::Manager;

pub use overview::{
    build_desktop_home, build_desktop_overview, desktop_archive_plan, desktop_clear_goal,
    desktop_hidden_plan, desktop_open_config_dir, desktop_open_log_dir, desktop_pause_goal,
    desktop_probe_status, desktop_resume_goal, desktop_save_goal, desktop_thread_detail,
    desktop_threads, nexus_paths_for_home, DesktopGoal, DesktopGoalRequest, DesktopHome,
    DesktopOverview, NexusPaths, ThreadListRequest,
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
fn desktop_threads_command(
    request: ThreadListRequest,
) -> Result<Vec<nexushub_core::codex::ThreadSummary>, String> {
    desktop_threads(request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_thread_detail_command(
    id: String,
) -> Result<Option<nexushub_core::codex::ThreadDetail>, String> {
    desktop_thread_detail(&id).map_err(|err| err.to_string())
}

#[tauri::command]
async fn desktop_probe_status_command() -> Result<nexushub_core::probe::ProbeStatus, String> {
    desktop_probe_status().await.map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_archive_plan_command() -> Result<nexushub_core::archive::ArchiveDeletePlan, String> {
    desktop_archive_plan().map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_hidden_plan_command() -> Result<nexushub_core::archive::HiddenThreadDeletePlan, String> {
    desktop_hidden_plan().map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_save_goal_command(request: DesktopGoalRequest) -> Result<DesktopGoal, String> {
    desktop_save_goal(request).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_clear_goal_command(thread_id: String) -> Result<DesktopGoal, String> {
    desktop_clear_goal(&thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_pause_goal_command(thread_id: String) -> Result<DesktopGoal, String> {
    desktop_pause_goal(&thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_resume_goal_command(thread_id: String) -> Result<DesktopGoal, String> {
    desktop_resume_goal(&thread_id).map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_open_config_dir_command() -> Result<(), String> {
    desktop_open_config_dir().map_err(|err| err.to_string())
}

#[tauri::command]
fn desktop_open_log_dir_command() -> Result<(), String> {
    desktop_open_log_dir().map_err(|err| err.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            desktop_overview,
            desktop_home,
            desktop_threads_command,
            desktop_thread_detail_command,
            desktop_probe_status_command,
            desktop_archive_plan_command,
            desktop_hidden_plan_command,
            desktop_save_goal_command,
            desktop_clear_goal_command,
            desktop_pause_goal_command,
            desktop_resume_goal_command,
            desktop_open_config_dir_command,
            desktop_open_log_dir_command
        ])
        .setup(|app| {
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
