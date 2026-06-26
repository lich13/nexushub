mod commands;
// 业务命令入口按领域放在 commands/*；overview 仅保留桌面状态、首页汇总和启动初始化。
#[allow(dead_code)]
mod overview;
mod services;

use std::{
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{
    Manager, PhysicalPosition, PhysicalSize, Runtime, Size, WebviewWindow, WebviewWindowBuilder,
};

pub use overview::{nexus_paths_for_home, DesktopState, NexusPaths};
pub use services::probe::desktop_probe_status_with_state;
pub use services::updates::desktop_update_status_with_state;

const NEXUSHUBD_RESOURCE_NAME: &str = "nexushubd";
const NEXUSHUBD_HELPER_PLACEHOLDER: &[u8] = b"NEXUSHUB_HELPER_PLACEHOLDER";
const WEBUI_RESOURCE_NAME: &str = "webui";
const MAIN_WINDOW_LABEL: &str = "main";
const DESKTOP_RUNTIME_MARKER_SCRIPT: &str = r#"
window.__NEXUSHUB_DESKTOP_RUNTIME__ = true;
"#;
const DESKTOP_BOOT_PROBE_SCRIPT: &str = r#"
(function () {
  var root = document.getElementById("root");
  var bodyText = (document.body && document.body.innerText ? document.body.innerText : "").replace(/\s+/g, " ").trim();
  return {
    readyState: document.readyState,
    title: document.title,
    desktopRuntime: window.__NEXUSHUB_DESKTOP_RUNTIME__ === true,
    bootMounted: Boolean(window.__NEXUSHUB_BOOT__ && window.__NEXUSHUB_BOOT__.mounted),
    rootChildren: root ? root.children.length : -1,
    rootClass: root && root.firstElementChild ? root.firstElementChild.className : "",
    bodyTextLength: bodyText.length,
    hasMainShell: Boolean(document.querySelector(".app-shell")),
    hasDesktopNav: bodyText.indexOf("Codex") >= 0 && bodyText.indexOf("探针") >= 0 && bodyText.indexOf("运维") >= 0,
    hasWebLoginGate: Boolean(document.querySelector(".login-shell")),
    hasVisibleLinuxHostCopy: Boolean(document.querySelector(".security-workspace, .turnstile-box"))
  };
})()
"#;

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

fn reveal_main_window<R: Runtime>(window: &WebviewWindow<R>) {
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.maximize();
    fit_main_window_to_work_area(window);
    let _ = window.set_focus();
}

fn fit_main_window_to_work_area<R: Runtime>(window: &WebviewWindow<R>) {
    let monitor = match window.current_monitor() {
        Ok(Some(monitor)) => Some(monitor),
        _ => window.primary_monitor().ok().flatten(),
    };
    let Some(monitor) = monitor else {
        return;
    };
    let work_area = monitor.work_area();
    if work_area.size.width == 0 || work_area.size.height == 0 {
        return;
    }
    let _ = window.set_position(PhysicalPosition::new(
        work_area.position.x,
        work_area.position.y,
    ));
    let _ = window.set_size(Size::Physical(PhysicalSize::new(
        work_area.size.width,
        work_area.size.height,
    )));
}

fn schedule_delayed_main_window_reveal<R: Runtime>(window: &WebviewWindow<R>) {
    let delayed_window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(250));
        let main_thread_window = delayed_window.clone();
        let _ = delayed_window.run_on_main_thread(move || {
            reveal_main_window(&main_thread_window);
        });
    });
}

fn append_desktop_app_log(message: &str) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let paths = nexus_paths_for_home(home);
    if std::fs::create_dir_all(&paths.log_dir).is_err() {
        return;
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.app_log_file)
    else {
        return;
    };
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let _ = writeln!(file, "{timestamp} {message}");
}

fn schedule_desktop_boot_probe<R: tauri::Runtime>(window: &WebviewWindow<R>) {
    let probe_window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(2));
        if let Err(err) = probe_window.eval_with_callback(DESKTOP_BOOT_PROBE_SCRIPT, |payload| {
            append_desktop_app_log(&format!("desktop_boot_probe {payload}"));
        }) {
            append_desktop_app_log(&format!("desktop_boot_probe_error {err}"));
        }
    });
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

pub fn run() {
    tauri::Builder::default()
        .append_invoke_initialization_script(DESKTOP_RUNTIME_MARKER_SCRIPT)
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
            commands::jobs::listJobs,
            commands::jobs::getJob
        ])
        .setup(|app| {
            if let Ok(resource_dir) = app.path().resource_dir() {
                sync_nexushubd_helper_from_resource(&resource_dir)?;
                prepare_macos_webui_assets_from_resource(&resource_dir)?;
            }
            let state = DesktopState::current().map_err(|err| err.to_string())?;
            app.manage(state);
            let main_window_config = app
                .config()
                .app
                .windows
                .iter()
                .find(|window| window.label == MAIN_WINDOW_LABEL)
                .ok_or_else(|| "main Tauri window config not found".to_string())?;
            let window = WebviewWindowBuilder::from_config(app.handle(), main_window_config)
                .map_err(|err| err.to_string())?
                .build()
                .map_err(|err| err.to_string())?;
            reveal_main_window(&window);
            schedule_delayed_main_window_reveal(&window);
            schedule_desktop_boot_probe(&window);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build NexusHub desktop app")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Ready) {
                if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                    reveal_main_window(&window);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn production_lib_source() -> &'static str {
        include_str!("lib.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("lib source must include production section")
    }

    fn registered_invoke_command_paths() -> Vec<String> {
        let production_source = production_lib_source();
        let marker = ".invoke_handler(tauri::generate_handler![";
        let start = production_source
            .find(marker)
            .expect("lib source must include tauri generate_handler")
            + marker.len();
        let body = production_source[start..]
            .split("\n        ])")
            .next()
            .expect("generate_handler block must close");
        body.lines()
            .map(str::trim)
            .filter(|line| line.starts_with("commands::"))
            .map(|line| line.trim_end_matches(',').to_string())
            .collect()
    }

    fn command_path(module: &str, name: &str) -> String {
        format!("commands::{module}::{name}")
    }

    fn retired_compat_path(module: &str, stem: &str) -> String {
        command_path(module, &format!("{stem}_{}", "command"))
    }

    fn concat_token(parts: &[&str]) -> String {
        parts.concat()
    }

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

    #[test]
    fn tauri_commands_stay_in_domain_modules() {
        let lib_source = include_str!("lib.rs");
        for domain in ["threads", "jobs", "settings", "system", "probe", "updates"] {
            assert!(
                lib_source.contains(&format!("commands::{domain}::")),
                "Tauri invoke handler must register {domain} commands through commands/{domain}.rs"
            );
        }
        for forbidden in [
            "\nfn desktop_",
            "\nasync fn desktop_",
            "\npub fn desktop_",
            "\npub async fn desktop_",
        ] {
            assert!(
                !lib_source.contains(forbidden),
                "desktop command wrappers must live in src-tauri/src/commands/*, not lib.rs"
            );
        }
    }

    #[test]
    fn tauri_invoke_handler_excludes_retired_desktop_command_compat_wrappers() {
        let commands = registered_invoke_command_paths();
        for command in &commands {
            let Some(name) = command.rsplit("::").next() else {
                continue;
            };
            assert!(
                !(name.starts_with("desktop_") && name.ends_with("_command")),
                "desktop_*_command compatibility command must not be registered: {command}"
            );
        }
        for retired in [
            command_path("settings", "startProbeJob"),
            command_path("updates", "runUpdateAction"),
            command_path("updates", "updatesPrune"),
            command_path("system", "getDesktopOverview"),
            command_path("system", "getDesktopHome"),
            command_path("system", "getDesktopPlatformStatus"),
            command_path("system", "getDesktopClaudeCodeOverview"),
        ] {
            assert!(
                !commands.contains(&retired),
                "retired or Linux-only update command must not be registered: {retired}"
            );
        }
        for (module, stem) in [
            ("threads", "desktop_threads"),
            ("threads", "desktop_thread_detail"),
            ("probe", "desktop_probe_status"),
            ("settings", "desktop_archive_plan"),
            ("settings", "desktop_hidden_plan"),
            ("settings", "desktop_open_config_dir"),
            ("settings", "desktop_open_log_dir"),
            ("settings", "desktop_save_goal"),
            ("settings", "desktop_clear_goal"),
            ("settings", "desktop_pause_goal"),
            ("settings", "desktop_resume_goal"),
            ("settings", "desktop_upload_files"),
        ] {
            let retired = retired_compat_path(module, stem);
            assert!(
                !commands.contains(&retired),
                "unused desktop compatibility command must not be registered: {retired}"
            );
        }
    }

    #[test]
    fn tauri_invoke_handler_keeps_desktop_compat_out_of_frontend_workflows() {
        let commands = registered_invoke_command_paths();
        for typed in [
            command_path("system", "getSystemStatus"),
            command_path("system", "getSystemVersion"),
            command_path("system", "listProviders"),
            command_path("system", "getClaudeCodeOverview"),
            command_path("system", "getPlatformOverview"),
            command_path("system", "listPlugins"),
            command_path("system", "listModels"),
            command_path("system", "listPermissionProfiles"),
            command_path("system", "getCodexConfig"),
            command_path("threads", "listThreads"),
            command_path("threads", "getThread"),
            command_path("threads", "getThreadBlocks"),
            command_path("threads", "createThread"),
            command_path("threads", "sendMessage"),
            command_path("threads", "steerThread"),
            command_path("threads", "listFollowUps"),
            command_path("threads", "enqueueFollowUp"),
            command_path("threads", "cancelFollowUp"),
            command_path("threads", "stopThread"),
            command_path("threads", "archiveThread"),
            command_path("threads", "restoreThread"),
            command_path("threads", "renameThread"),
            command_path("threads", "forkThread"),
            command_path("threads", "answerElicitation"),
            command_path("threads", "acceptPlan"),
            command_path("threads", "revisePlan"),
            command_path("threads", "answerApproval"),
            command_path("probe", "getProbeStatus"),
            command_path("updates", "getUpdateStatus"),
            command_path("updates", "updatesCheck"),
            command_path("updates", "updatesInstall"),
            command_path("settings", "getProbeSettings"),
            command_path("settings", "saveProbeSettings"),
            command_path("settings", "getProbeLogsDbStatus"),
            command_path("settings", "getProbeEvents"),
            command_path("settings", "probeBarkTest"),
            command_path("settings", "probeInstallHooks"),
            command_path("settings", "probeLogsDbDryRun"),
            command_path("settings", "probeLogsDbExecute"),
            command_path("settings", "dryRunArchiveDelete"),
            command_path("settings", "startArchiveDelete"),
            command_path("settings", "dryRunHiddenThreadDelete"),
            command_path("settings", "startHiddenThreadDelete"),
            command_path("settings", "deleteUpload"),
            command_path("settings", "uploadFiles"),
            command_path("settings", "getCodexGoal"),
            command_path("settings", "saveCodexGoal"),
            command_path("settings", "clearCodexGoal"),
            command_path("settings", "pauseCodexGoal"),
            command_path("settings", "resumeCodexGoal"),
            command_path("jobs", "listJobs"),
            command_path("jobs", "getJob"),
            command_path("updates", "updatesCheck"),
            command_path("updates", "updatesInstall"),
        ] {
            assert!(
                commands.contains(&typed),
                "typed desktop command must be registered: {typed}"
            );
        }

        for command in &commands {
            let Some(name) = command.rsplit("::").next() else {
                continue;
            };
            assert!(
                !name.starts_with("desktop_"),
                "frontend workflow must use typed command registration instead of desktop_* compat: {command}"
            );
        }
    }

    #[test]
    fn tauri_invoke_handler_registers_only_typed_probe_and_update_commands() {
        let commands = registered_invoke_command_paths();
        for legacy in [
            command_path("updates", "checkUpdate"),
            command_path("updates", "installUpdateAndRestart"),
            command_path("settings", "startProbeBarkTest"),
            command_path("settings", "startProbeHooksInstall"),
            command_path("settings", "startProbeLogsDbDryRun"),
            command_path("settings", "startProbeLogsDbExecute"),
            command_path("system", "getDesktopOverview"),
            command_path("system", "getDesktopHome"),
            command_path("system", "getDesktopPlatformStatus"),
            command_path("system", "getDesktopClaudeCodeOverview"),
        ] {
            assert!(
                !commands.contains(&legacy),
                "legacy WebUI compatibility command must not be registered in Tauri: {legacy}"
            );
        }
    }

    #[test]
    fn tauri_command_modules_do_not_define_legacy_probe_or_update_wrappers() {
        for (source, legacy) in [
            (
                include_str!("commands/updates.rs"),
                "pub async fn checkUpdate",
            ),
            (
                include_str!("commands/updates.rs"),
                "pub async fn installUpdateAndRestart",
            ),
            (
                include_str!("commands/settings.rs"),
                "pub fn startProbeBarkTest",
            ),
            (
                include_str!("commands/settings.rs"),
                "pub fn startProbeHooksInstall",
            ),
            (
                include_str!("commands/settings.rs"),
                "pub fn startProbeLogsDbDryRun",
            ),
            (
                include_str!("commands/settings.rs"),
                "pub fn startProbeLogsDbExecute",
            ),
            (
                include_str!("commands/system.rs"),
                "pub fn getDesktopOverview",
            ),
            (
                include_str!("commands/system.rs"),
                "pub async fn getDesktopHome",
            ),
            (
                include_str!("commands/system.rs"),
                "pub async fn getDesktopPlatformStatus",
            ),
            (
                include_str!("commands/system.rs"),
                "pub fn getDesktopClaudeCodeOverview",
            ),
        ] {
            assert!(
                !source.contains(legacy),
                "legacy Tauri command wrapper must not be defined: {legacy}"
            );
        }
    }

    #[test]
    fn tauri_update_commands_do_not_plan_linux_prune_actions() {
        let source = include_str!("commands/updates.rs");
        assert!(
            !source.contains("UpdateAction::Prune"),
            "macOS Tauri update commands must not expose Linux update prune"
        );
    }

    #[test]
    fn tauri_cleanup_execute_commands_require_confirmation_payload() {
        let source = include_str!("commands/settings.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("settings command source must include production section");

        for command in ["startArchiveDelete", "startHiddenThreadDelete"] {
            let start = source
                .find(&format!("pub fn {command}("))
                .unwrap_or_else(|| panic!("cleanup execute command must exist: {command}"));
            let body = &source[start..];
            let signature = body.split(") ->").next().unwrap_or_else(|| {
                panic!("cleanup execute command signature must close: {command}")
            });
            assert!(
                signature.contains("request:") || signature.contains("payload:"),
                "cleanup execute command must accept a confirmation payload: {command}"
            );
            assert!(
                signature.contains("DesktopCleanupExecuteRequest"),
                "cleanup execute command must use the typed cleanup confirmation payload: {command}"
            );
        }
    }

    #[test]
    fn tauri_cleanup_service_is_native_effect_executor_only() {
        let source = include_str!("services/settings.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("settings service source must include production section");

        assert!(
            source.contains(
                "type DesktopCleanupExecuteRequest = cleanup_service::CleanupExecuteRequest"
            ),
            "Tauri cleanup execute payload must reuse the shared core confirmation request"
        );
        assert!(
            source.contains("NexusHubUseCases::new(state.platform()).cleanup()")
                && source.contains(".execute_confirmed(")
                && source.contains("cleanup_service::validate_cleanup_expected_count"),
            "Tauri cleanup service must consume the core cleanup use-case facade before native delete effects"
        );
        for forbidden in [
            "cleanup_service::plan_cleanup_execute_operation",
            "ARCHIVE_DELETE_CONFIRMATION_MESSAGE",
            "HIDDEN_DELETE_CONFIRMATION_MESSAGE",
            "CLEANUP_EXPECTED_COUNT_REQUIRED_MESSAGE",
            "archive deletion must be confirmed",
            "hidden thread deletion must be confirmed",
            "expectedCount mismatch",
            "fn ensure_cleanup_expected_count",
        ] {
            assert!(
                !source.contains(forbidden),
                "Tauri cleanup service must not define cleanup business semantic token: {forbidden}"
            );
        }
    }

    #[test]
    fn overview_only_keeps_desktop_state_home_and_startup_types() {
        let overview_source = include_str!("overview.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("overview source must include production section");

        for forbidden in [
            "pub struct DesktopActionResponse",
            "pub struct DesktopThreadBlockPage",
            "pub struct DesktopProbeSettings",
            "pub struct DesktopJobResponse",
            "pub struct DesktopProbeEventsResponse",
            "pub struct DesktopDeleteUploadResponse",
            "pub struct DesktopUploadFile",
            "pub struct ThreadListRequest",
            "pub struct ThreadDetailRequest",
            "pub struct ThreadBlocksRequest",
            "pub struct DesktopSendMessageRequest",
            "pub struct DesktopStopRequest",
            "pub struct DesktopThreadIdRequest",
            "pub struct DesktopRenameThreadRequest",
            "pub struct DesktopPlanAcceptRequest",
            "pub struct DesktopPlanReviseRequest",
            "pub struct DesktopElicitationAnswerRequest",
            "pub struct DesktopJobsRequest",
            "pub struct DesktopJobDetailRequest",
            "pub struct DesktopDeleteUploadRequest",
            "pub struct DesktopFollowupRequest",
            "pub struct DesktopCancelFollowupRequest",
            "DesktopGoal",
            "ProbeRuntime",
            "ProbeStatus",
            "ProbeLogsDbStatus",
            "SystemStatus",
            "ArchiveDeletePlan",
            "HiddenThreadDeletePlan",
            "first_thread_goal",
            "ThreadSummary",
            "home_thread_summaries",
        ] {
            assert!(
                !overview_source.contains(forbidden),
                "overview.rs must not define command adapter DTO: {forbidden}"
            );
        }
    }

    #[test]
    fn tauri_thread_commands_use_core_thread_query_and_detail_plans() {
        let thread_commands_source = include_str!("commands/threads.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or(include_str!("commands/threads.rs"));
        let threads_source = include_str!("services/threads.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or(include_str!("services/threads.rs"));

        for required in [
            "thread_summaries_with_query(",
            "plan_thread_detail_read",
            "plan_thread_blocks_read",
            "thread_detail_read_model",
            "window_thread_detail_for_plan",
            "thread_blocks_page_for_plan",
            "plan_thread_send_job_execution",
            "plan_thread_command_job_execution",
            "plan_thread_steer_with_capability",
        ] {
            assert!(
                threads_source.contains(required),
                "Tauri thread adapter must consume shared core plan: {required}"
            );
        }

        for forbidden in [
            "fn thread_list_with_jobs(",
            "window_thread_detail(",
            "detail_block_limit(",
            "block_page_limit(",
            "thread_service::normalize_thread_detail_block_limit",
            "thread_service::normalize_thread_block_limit",
        ] {
            assert!(
                !threads_source.contains(forbidden),
                "Tauri thread adapter must not duplicate core thread paging logic: {forbidden}"
            );
        }

        assert!(
            thread_commands_source.contains("thread_service::threads_with_state")
                && thread_commands_source.contains("thread_service::send_message_with_state")
                && !thread_commands_source.contains("state.db.")
                && !thread_commands_source.contains("state.jobs."),
            "Tauri thread commands must stay thin and delegate to services/threads.rs"
        );
        assert!(
            threads_source.contains("thread_service::plan_thread_list_read")
                && threads_source.contains("thread_service::thread_list_read_model")
                && threads_source.contains("thread_service::thread_detail_read_model")
                && !threads_source.contains("thread_service::build_threads_overview")
                && !threads_source.contains("thread_service::apply_running_job_to_summary"),
            "desktop thread service must consume shared core read-model plans"
        );
    }

    #[test]
    fn tauri_thread_job_submission_uses_core_execution_plans() {
        let threads_source = include_str!("services/threads.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .unwrap_or(include_str!("services/threads.rs"));

        for required in [
            "job_service::plan_thread_command_job_execution",
            "job_service::plan_thread_send_job_execution",
            "job_service::ThreadCommandExecutionPlan",
            "plan.submitted_response(&job_id)",
        ] {
            assert!(
                threads_source.contains(required),
                "Tauri thread service must consume shared core job execution plan: {required}"
            );
        }

        for forbidden in [
            ".command\n        .action",
            "thread send plan is missing Codex job action",
            "build_codex_job_spec(&action",
        ] {
            assert!(
                !threads_source.contains(forbidden),
                "Tauri thread service must not rebuild core job execution semantics: {forbidden}"
            );
        }
    }

    #[test]
    fn tauri_settings_commands_use_core_settings_view_and_secret_write_plans() {
        let settings_commands_source = include_str!("commands/settings.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("settings command source must include production section");
        let settings_source = include_str!("services/settings.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("settings service source must include production section");

        for required in [
            "settings_service::probe_settings_view_with_capability",
            "for secret_write in plan.secret_writes",
            "state.db.create_job(&job_id, &job.kind, &job.title)",
        ] {
            assert!(
                settings_source.contains(required),
                "Tauri settings adapter must consume shared core settings facade: {required}"
            );
        }

        assert!(
            settings_commands_source.contains("settings_service::probe_settings_with_state")
                && settings_commands_source.contains("goal_service::save_goal_with_state")
                && !settings_commands_source.contains("state.db.")
                && !settings_commands_source.contains("plan_probe_settings_save"),
            "Tauri settings commands must stay thin and delegate to native services"
        );
        assert!(
            !settings_source.contains("if let Some(device_key) = plan.bark_device_key"),
            "Tauri settings adapter must not special-case Probe secret writes outside the core plan"
        );
    }

    #[test]
    fn tauri_settings_service_does_not_define_goal_state_semantics() {
        let settings_source = include_str!("services/settings.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("settings service source must include production section");
        let goals_source = include_str!("services/goals.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("goals service source must include production section");

        for forbidden in [
            "pub(crate) struct DesktopGoal",
            "pub(crate) struct DesktopGoalRequest",
            "fn desktop_goal_from_view",
            "fn unavailable_desktop_goal",
            "status: \"unavailable\"",
        ] {
            assert!(
                !settings_source.contains(forbidden),
                "Tauri settings service must not define Goal business state semantics: {forbidden}"
            );
            assert!(
                !goals_source.contains(forbidden),
                "Tauri goals service must not define Goal business state semantics: {forbidden}"
            );
        }
        assert!(
            goals_source.contains("type DesktopGoalView = goal_service::GoalView"),
            "Tauri goals service should expose the shared core Goal DTO instead of mapping a desktop DTO"
        );
    }

    #[test]
    fn tauri_probe_status_uses_core_probe_use_case_facade() {
        let probe_source = include_str!("services/probe.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .unwrap_or(include_str!("services/probe.rs"));

        assert!(
            probe_source.contains("NexusHubUseCases::with_config")
                && probe_source.contains(".probe()?")
                && probe_source.contains(".status()?")
                && probe_source.contains("probe_service::probe_status_with_runtime_read_model"),
            "Tauri probe status must derive read-model buckets through the shared core Probe use-case facade"
        );
        for forbidden in [
            "status.running_threads =",
            "status.reply_needed_threads =",
            "status.recoverable_threads =",
            "status.running_count =",
            "status.reply_needed_count =",
            "status.recoverable_count =",
        ] {
            assert!(
                !probe_source.contains(forbidden),
                "Tauri probe status must not assign read-model fields outside core helper: {forbidden}"
            );
        }
    }

    #[test]
    fn tauri_system_commands_delegate_to_native_service_layer() {
        let system_commands_source = include_str!("commands/system.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("system command source must include production section");

        assert!(
            system_commands_source.contains("services::system"),
            "commands/system.rs must delegate native work to services/system.rs"
        );
        for forbidden in [
            "nexushub_core::system::system_status_with_paths",
            "nexushub_core::local::local_plugin_catalog",
            "nexushub_core::local::default_codex_models",
            "nexushub_core::local::default_permission_profiles",
            "nexushub_core::local::local_codex_config",
            "nexushub_core::claude_code::claude_overview",
        ] {
            assert!(
                !system_commands_source.contains(forbidden),
                "commands/system.rs must stay thin and not execute native system logic: {forbidden}"
            );
        }
    }

    #[test]
    fn tauri_commands_do_not_reimplement_migrated_goal_or_followup_transactions() {
        let settings_commands_source = include_str!("commands/settings.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("settings command source must include production section");
        let settings_source = include_str!("services/settings.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("settings service source must include production section");
        let goals_source = include_str!("services/goals.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("goals service source must include production section");
        let thread_commands_source = include_str!("commands/threads.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .unwrap_or(include_str!("commands/threads.rs"));
        let threads_source = include_str!("services/threads.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .unwrap_or(include_str!("services/threads.rs"));

        for required in [
            "upload_service::plan_store_uploads_with_capability",
            "upload_service::plan_delete_upload_with_capability",
            "cleanup_service::dry_run_archived_with_capability",
            "cleanup_service::dry_run_hidden_with_capability",
            "let cleanup = NexusHubUseCases::new(state.platform()).cleanup()",
            ".execute_confirmed(",
        ] {
            assert!(
                settings_source.contains(required),
                "Tauri settings service must call the shared core facade/plan: {required}"
            );
        }
        for required in [
            "goal_service::goal_get_response_with_capability",
            "goal_service::save_goal_with_capability",
            "goal_service::clear_goal_with_capability",
            "goal_service::pause_goal_with_capability",
            "goal_service::resume_goal_with_capability",
        ] {
            assert!(
                goals_source.contains(required),
                "Tauri goals service must call the shared core facade/plan: {required}"
            );
        }
        for required in [
            "NexusHubUseCases::new(state.platform()).threads()",
            ".list_followups(",
            ".apply_enqueue_followup(",
            ".apply_cancel_followup(",
            "job_service::plan_thread_archive_with_capability",
            "job_service::plan_thread_restore_with_capability",
            "job_service::plan_thread_rename_with_capability",
            "job_service::thread_state_action_response",
            "job_service::plan_thread_stop_with_capability",
            "job_service::resolve_thread_stop_job",
            "job_service::thread_stop_response",
        ] {
            assert!(
                threads_source.contains(required),
                "Tauri thread commands must call the shared core facade/plan: {required}"
            );
        }

        assert!(
            settings_commands_source.contains("goal_service::save_goal_with_state")
                && !settings_commands_source.contains("goal_get_response_with_capability")
                && !settings_commands_source.contains("save_goal_with_capability")
                && !settings_commands_source.contains("clear_goal_with_capability")
                && !settings_commands_source.contains("pause_goal_with_capability")
                && !settings_commands_source.contains("resume_goal_with_capability"),
            "Tauri settings commands must delegate migrated goal transactions to services/goals.rs"
        );
        assert!(
            thread_commands_source.contains("thread_service::enqueue_followup_with_state")
                && !thread_commands_source.contains("job_service::"),
            "Tauri thread commands must delegate migrated follow-up transactions to services/threads.rs"
        );

        for forbidden in [
            "open_panel_db(config)",
            ".get_thread_goal(",
            ".upsert_thread_goal(",
            ".delete_thread_goal(",
            ".update_thread_goal_status(",
            "upload_service::plan_desktop_batch_uploads(",
            "uploads::delete_upload(&root, &request.id)",
            "plan_delete_archived(",
            "execute_delete_archived(",
            "plan_delete_hidden(",
            "execute_delete_hidden(",
        ] {
            assert!(
                !settings_source.contains(forbidden),
                "Tauri settings commands must not reimplement migrated goal transactions: {forbidden}"
            );
        }
        for forbidden in [
            ".enqueue_followup(",
            ".cancel_followup(",
            "state.db.list_followups(",
            "state.db.enqueue_followup(",
            "state.db.cancel_followup(",
            "job_service::list_followups_with_capability(",
            "job_service::enqueue_followup_with_capability(",
            "job_service::cancel_followup_with_capability(",
            "job_service::claim_next_followup_with_capability(",
            "job_service::mark_followup_submitted_with_capability(",
            "job_service::mark_followup_error_with_capability(",
            "request.name.trim()",
            "job_service::archive_thread_response(",
            "job_service::rename_thread_response(",
            "command: \"stopThread\"",
            "\"stopThread\"",
            "\"cancelFollowUp\"",
        ] {
            assert!(
                !threads_source.contains(forbidden),
                "Tauri thread commands must not reimplement migrated follow-up transactions: {forbidden}"
            );
        }
        assert!(
            threads_source.contains("NexusHubUseCases::new(state.platform()).threads()"),
            "Tauri thread services must reach follow-up DB effects through core use-case facade"
        );
    }

    #[test]
    fn tauri_thread_approval_unavailable_semantics_stay_out_of_commands() {
        let thread_commands_source = include_str!("commands/threads.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or(include_str!("commands/threads.rs"));

        assert!(
            thread_commands_source.contains("thread_service::answer_approval_with_state")
                && !thread_commands_source.contains("action_unavailable")
                && !thread_commands_source.contains("approval actions are unavailable"),
            "commands/threads.rs must keep approval answer as typed args + service + map_err only"
        );
        for forbidden in [
            "THREADS_APPROVAL_ANSWER",
            "action_unavailable",
            "approval actions are unavailable",
            "ok: false",
        ] {
            assert!(
                !thread_commands_source.contains(forbidden),
                "commands/threads.rs must not build approval business responses: {forbidden}"
            );
        }
    }

    #[test]
    fn overview_does_not_export_desktop_business_helper_functions() {
        let overview_source = include_str!("overview.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("overview source must include production section");

        for forbidden in [
            "desktop_threads",
            "desktop_thread_detail",
            "desktop_thread_blocks",
            "desktop_send_message",
            "desktop_continue_thread",
            "desktop_stop_thread",
            "desktop_plan_accept",
            "desktop_plan_revise",
            "desktop_answer_elicitation",
            "desktop_archive_thread",
            "desktop_restore_thread",
            "desktop_rename_thread",
            "desktop_fork_thread",
            "desktop_probe_status",
            "desktop_probe_settings",
            "desktop_probe_save_settings",
            "desktop_probe_bark_test",
            "desktop_probe_hooks_install",
            "desktop_probe_logs_db_maintain",
            "desktop_probe_events",
            "desktop_archive_plan",
            "desktop_hidden_plan",
            "desktop_archive_delete",
            "desktop_hidden_delete",
            "desktop_delete_upload",
            "desktop_store_uploads",
            "desktop_jobs",
            "desktop_job_detail",
            "desktop_list_followups",
            "desktop_enqueue_followup",
            "desktop_cancel_followup",
            "desktop_codex_job_spec",
        ] {
            assert!(
                !overview_source.contains(forbidden),
                "overview.rs must not retain desktop business helper: {forbidden}"
            );
        }
    }

    #[test]
    fn overview_does_not_depend_on_command_modules() {
        let overview_source = include_str!("overview.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("overview source must include production section");

        for forbidden in [
            "use crate::commands::",
            "crate::commands::",
            "commands::settings::DesktopGoal",
            "commands::threads::threads_for_home",
            "commands::settings::first_thread_goal",
        ] {
            assert!(
                !overview_source.contains(forbidden),
                "overview.rs must not depend on command adapters: {forbidden}"
            );
        }
    }

    #[test]
    fn tauri_command_modules_remain_thin_typed_adapters() {
        for (module, source) in [
            ("threads", include_str!("commands/threads.rs")),
            ("settings", include_str!("commands/settings.rs")),
            ("updates", include_str!("commands/updates.rs")),
            ("jobs", include_str!("commands/jobs.rs")),
            ("probe", include_str!("commands/probe.rs")),
        ] {
            for forbidden in [
                "state.db.",
                "state.jobs.",
                "set_thread_archived",
                "set_thread_title",
                "thread_detail(",
                "patch_probe_config_toml",
                "std::fs::write",
                "updater_builder",
                "create_job(",
                "append_job_output(",
                "finish_job(",
                "running_job_for_thread",
                "ok_or_else",
                "approval actions are unavailable",
            ] {
                assert!(
                    !source.contains(forbidden),
                    "commands/{module}.rs must stay a thin typed adapter and not contain {forbidden}"
                );
            }
        }
    }

    #[test]
    fn tauri_goal_commands_accept_core_dtos_without_alias_fallback_assembly() {
        let settings_commands_source = include_str!("commands/settings.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("settings command source must include production section");
        let goals_source = include_str!("services/goals.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("goals service source must include production section");

        for required in [
            "GoalGetRequest",
            "GoalUpdateRequest",
            "goal_service::get_goal_with_state(&state, request)",
            "goal_service::save_goal_with_state(&state, request)",
        ] {
            assert!(
                settings_commands_source.contains(required),
                "Goal commands must pass typed core DTOs into the service: {required}"
            );
        }
        {
            let forbidden = "save_goal_from_parts_with_state";
            assert!(
                !settings_commands_source.contains(forbidden),
                "commands/settings.rs must not assemble Goal compatibility payloads: {forbidden}"
            );
            assert!(
                !goals_source.contains(forbidden),
                "services/goals.rs must not assemble Goal compatibility payloads: {forbidden}"
            );
        }
        for service_forbidden in [
            "threadId.or(thread_id)",
            "tokenBudget.or(token_budget)",
            "threadId: Option<String>",
            "thread_id: Option<String>",
            "tokenBudget: Option<u64>",
            "token_budget: Option<u64>",
            "GoalUpdateRequest {",
        ] {
            assert!(
                !goals_source.contains(service_forbidden),
                "services/goals.rs must keep v0.1.128 Goal ABI compatibility out of the service layer: {service_forbidden}"
            );
        }
    }

    #[test]
    fn tauri_command_guard_does_not_embed_retired_compat_tokens_in_tests() {
        let test_source = include_str!("lib.rs")
            .split("\n#[cfg(test)]")
            .nth(1)
            .expect("lib source must include test section");
        for retired in [
            command_path("settings", "startProbeJob"),
            command_path("updates", "runUpdateAction"),
            command_path("updates", "updatesPrune"),
            command_path("updates", "pruneBackups"),
        ] {
            assert!(
                !test_source.contains(&retired),
                "tests must not embed retired string action command token: {retired}"
            );
        }
        for (module, stem) in [
            ("threads", "desktop_threads"),
            ("threads", "desktop_thread_detail"),
            ("probe", "desktop_probe_status"),
            ("settings", "desktop_archive_plan"),
            ("settings", "desktop_hidden_plan"),
            ("settings", "desktop_open_config_dir"),
            ("settings", "desktop_open_log_dir"),
            ("settings", "desktop_save_goal"),
            ("settings", "desktop_clear_goal"),
            ("settings", "desktop_pause_goal"),
            ("settings", "desktop_resume_goal"),
            ("settings", "desktop_upload_files"),
        ] {
            let retired = retired_compat_path(module, stem);
            assert!(
                !test_source.contains(&retired),
                "tests must not embed retired compatibility command token: {retired}"
            );
        }
    }

    #[test]
    fn macos_tauri_sources_do_not_suggest_linux_host_repair_steps() {
        for (label, source) in [
            ("commands/updates.rs", include_str!("commands/updates.rs")),
            ("services/updates.rs", include_str!("services/updates.rs")),
            ("commands/settings.rs", include_str!("commands/settings.rs")),
            ("services/settings.rs", include_str!("services/settings.rs")),
        ] {
            for forbidden in [
                "systemctl",
                "systemd",
                "Nginx",
                "nginx",
                "sudo ",
                "/opt/nexushub",
            ] {
                assert!(
                    !source.contains(forbidden),
                    "macOS Tauri source must not suggest Linux host repair step {forbidden} in {label}"
                );
            }
        }
    }

    #[test]
    fn tauri_invoke_handler_excludes_linux_web_host_command_surfaces() {
        let commands = registered_invoke_command_paths();
        for parts in [
            &["get", "Security"][..],
            &["save", "Security"][..],
            &["security", "Status"][..],
            &["change", "Password"][..],
            &["security", "_status"][..],
            &["auth", "Status"][..],
            &["log", "in"][..],
            &["log", "out"][..],
            &["cs", "rf"][..],
            &["turn", "stile"][..],
            &["admin", "_password"][..],
            &["system", "d"][..],
            &["System", "d"][..],
            &["ngi", "nx"][..],
            &["Nginx"][..],
            &["web", "Auth"][..],
            &["web", "auth"][..],
            &["system_update", "_prune"][..],
            &["desktop_update", "_prune"][..],
            &["prune", "_backups"][..],
            &["Probe", "Job"][..],
            &["run", "UpdateAction"][..],
        ] {
            let forbidden = concat_token(parts);
            assert!(
                commands.iter().all(|command| !command.contains(&forbidden)),
                "macOS desktop invoke handler must not register Linux Web host command surface: {forbidden}"
            );
        }
    }

    #[test]
    fn tauri_shell_injects_desktop_runtime_marker_before_webui_bootstrap() {
        let source = production_lib_source();

        assert!(
            source.contains("__NEXUSHUB_DESKTOP_RUNTIME__"),
            "Tauri must inject a desktop runtime marker before the WebUI bootstraps so macOS does not render the Web login gate"
        );
        assert!(
            source.contains(".append_invoke_initialization_script("),
            "Tauri must register the marker through an initialization script that runs before the bundled WebUI"
        );
    }

    #[test]
    fn macos_shell_creates_and_reveals_main_window_explicitly() {
        let source = production_lib_source();
        let config = include_str!("../tauri.conf.json");

        for required in [
            r#""width": 1280"#,
            r#""height": 820"#,
            r#""minWidth": 1000"#,
            r#""minHeight": 680"#,
            r#""maximized": true"#,
            r#""fullscreen": false"#,
        ] {
            assert!(
                config.contains(required),
                "main Tauri window config must preserve the v0.1.128 default maximized window contract: {required}"
            );
        }
        assert!(
            config.contains(r#""create": false"#),
            "Tauri must not rely on implicit tauri.conf window creation for the macOS shell"
        );
        assert!(
            source.contains("WebviewWindowBuilder::from_config"),
            "Tauri must explicitly build the main WebView window after desktop resources are prepared"
        );
        assert!(
            source.contains("RunEvent::Ready"),
            "Tauri must re-show and focus the main window once the event loop is ready"
        );
        let show_index = source
            .find("window.show()")
            .expect("reveal_main_window must show the main window");
        let unminimize_index = source
            .find("window.unminimize()")
            .expect("reveal_main_window must unminimize the main window before maximizing it");
        let maximize_index = source
            .find("window.maximize()")
            .expect("reveal_main_window must maximize the main window");
        let focus_index = source
            .find("window.set_focus()")
            .expect("reveal_main_window must focus the main window");
        assert!(
            show_index < unminimize_index
                && unminimize_index < maximize_index
                && maximize_index < focus_index,
            "reveal_main_window must preserve show -> unminimize -> maximize -> set_focus startup order"
        );
        for required in [
            "fn fit_main_window_to_work_area",
            "window.current_monitor()",
            "window.primary_monitor()",
            "monitor.work_area()",
            "window.set_position(PhysicalPosition::new(",
            "window.set_size(Size::Physical(PhysicalSize::new(",
        ] {
            assert!(
                source.contains(required),
                "explicit macOS window creation must fall back to the monitor work area when native maximize does not resize the window: {required}"
            );
        }
        for required in [
            "fn schedule_delayed_main_window_reveal",
            "std::time::Duration::from_millis",
            "run_on_main_thread",
            "schedule_delayed_main_window_reveal(&window)",
        ] {
            assert!(
                source.contains(required),
                "explicit macOS window creation must replay reveal after the event loop has settled: {required}"
            );
        }
        assert!(
            source.contains("desktop_boot_probe"),
            "Tauri must leave a low-detail boot probe for macOS App acceptance"
        );
        assert!(
            !source.contains("bodyTextSample"),
            "desktop boot probe must not log visible thread or workspace text"
        );
        assert!(
            !source.contains(r#"bodyText.indexOf("Turnstile")"#),
            "desktop boot probe must not classify session text as Web login UI"
        );
    }

    #[test]
    fn tauri_goal_commands_preserve_v0128_flat_invoke_abi() {
        let settings_commands_source = include_str!("commands/settings.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("settings command source must include production section");
        let goals_source = include_str!("services/goals.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("goals service source must include production section");

        for required in [
            "threadId: Option<String>",
            "thread_id: Option<String>",
            "objective: Option<String>",
            "tokenBudget: Option<u64>",
            "token_budget: Option<u64>",
            "GoalUpdateRequest {",
        ] {
            assert!(
                settings_commands_source.contains(required),
                "Tauri Goal commands must keep the v0.1.128 flat invoke ABI shim: {required}"
            );
        }
        assert!(
            settings_commands_source
                .contains("goal_service::save_goal_with_state(&state, request)")
                && goals_source.contains("goal_service::save_goal_with_capability"),
            "Goal compatibility shim must hand off a core GoalUpdateRequest to the service layer"
        );
    }
}
