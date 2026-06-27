mod commands;
mod desktop_boot;
// 业务命令入口按领域放在 commands/*；overview 仅保留桌面状态、首页汇总和启动初始化。
#[allow(dead_code)]
mod overview;
mod resources;
mod services;

use tauri::{Manager, WebviewWindowBuilder};

pub use overview::{nexus_paths_for_home, DesktopState, NexusPaths};
pub use services::probe::desktop_probe_status_with_state;
pub use services::updates::desktop_update_status_with_state;

pub fn run() {
    tauri::Builder::default()
        .append_invoke_initialization_script(desktop_boot::DESKTOP_RUNTIME_MARKER_SCRIPT)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::system::getSystemStatus,
            commands::system::getSystemVersion,
            commands::system::listProviders,
            commands::system::getClaudeCodeOverview,
            commands::system::getPlatformOverview,
            commands::system::listPlugins,
            commands::system::listModels,
            commands::system::listPermissionProfiles,
            commands::system::getCodexConfig,
            commands::threads::listThreads,
            commands::threads::getThread,
            commands::threads::getThreadBlocks,
            commands::threads::createThread,
            commands::threads::sendMessage,
            commands::threads::steerThread,
            commands::threads::listFollowUps,
            commands::threads::enqueueFollowUp,
            commands::threads::cancelFollowUp,
            commands::threads::stopThread,
            commands::threads::archiveThread,
            commands::threads::restoreThread,
            commands::threads::renameThread,
            commands::threads::forkThread,
            commands::threads::answerElicitation,
            commands::threads::acceptPlan,
            commands::threads::revisePlan,
            commands::threads::answerApproval,
            commands::probe::getProbeStatus,
            commands::updates::getUpdateStatus,
            commands::updates::updatesCheck,
            commands::updates::updatesInstall,
            commands::settings::getProbeSettings,
            commands::settings::saveProbeSettings,
            commands::settings::getProbeLogsDbStatus,
            commands::settings::getProbeEvents,
            commands::settings::probeBarkTest,
            commands::settings::probeInstallHooks,
            commands::settings::probeLogsDbDryRun,
            commands::settings::probeLogsDbExecute,
            commands::settings::dryRunArchiveDelete,
            commands::settings::startArchiveDelete,
            commands::settings::dryRunHiddenThreadDelete,
            commands::settings::startHiddenThreadDelete,
            commands::settings::deleteUpload,
            commands::settings::uploadFiles,
            commands::settings::getCodexGoal,
            commands::settings::saveCodexGoal,
            commands::settings::clearCodexGoal,
            commands::settings::pauseCodexGoal,
            commands::settings::resumeCodexGoal,
            commands::desktop_webui::getDesktopWebUiSettings,
            commands::desktop_webui::saveDesktopWebUiSettings,
            commands::desktop_webui::getDesktopWebUiStatus,
            commands::desktop_webui::startDesktopWebUi,
            commands::desktop_webui::stopDesktopWebUi,
            commands::desktop_webui::resetDesktopWebUiPassword,
            commands::jobs::listJobs,
            commands::jobs::getJob
        ])
        .setup(|app| {
            if let Ok(resource_dir) = app.path().resource_dir() {
                resources::sync_nexushub_webd_helper_from_resource(&resource_dir)?;
                resources::prepare_desktop_webui_assets_from_resource(&resource_dir)?;
            }
            let state = DesktopState::current().map_err(|err| err.to_string())?;
            app.manage(state);
            let main_window_config = app
                .config()
                .app
                .windows
                .iter()
                .find(|window| window.label == desktop_boot::MAIN_WINDOW_LABEL)
                .ok_or_else(|| "main Tauri window config not found".to_string())?;
            let window = WebviewWindowBuilder::from_config(app.handle(), main_window_config)
                .map_err(|err| err.to_string())?
                .build()
                .map_err(|err| err.to_string())?;
            desktop_boot::reveal_main_window(&window);
            desktop_boot::schedule_delayed_main_window_reveal(&window);
            desktop_boot::schedule_desktop_boot_probe(&window);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build NexusHub desktop app")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Ready) {
                if let Some(window) = app.get_webview_window(desktop_boot::MAIN_WINDOW_LABEL) {
                    desktop_boot::reveal_main_window(&window);
                }
            }
        });
}

#[cfg(test)]
mod entry_guard_tests;
