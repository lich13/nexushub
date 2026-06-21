mod commands;
// 业务命令入口按领域放在 commands/*；overview 仅保留桌面状态、首页汇总和启动初始化。
#[allow(dead_code)]
mod overview;

use std::path::Path;
use tauri::Manager;

pub use commands::{
    probe::desktop_probe_status_with_state, updates::desktop_update_status_with_state,
};
pub use overview::{nexus_paths_for_home, DesktopState, NexusPaths};

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

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::system::getDesktopOverview,
            commands::system::getDesktopHome,
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
            commands::updates::updatesPrune,
            commands::updates::checkUpdate,
            commands::updates::installUpdateAndRestart,
            commands::settings::getProbeSettings,
            commands::settings::saveProbeSettings,
            commands::settings::getProbeLogsDbStatus,
            commands::settings::getProbeEvents,
            commands::settings::probeBarkTest,
            commands::settings::probeInstallHooks,
            commands::settings::probeLogsDbDryRun,
            commands::settings::probeLogsDbExecute,
            commands::settings::startProbeBarkTest,
            commands::settings::startProbeHooksInstall,
            commands::settings::startProbeLogsDbDryRun,
            commands::settings::startProbeLogsDbExecute,
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
            commands::jobs::getJob,
            commands::system::getDesktopPlatformStatus,
            commands::system::getDesktopClaudeCodeOverview
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
        ] {
            assert!(
                !commands.contains(&retired),
                "string action compatibility command must not be registered: {retired}"
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
            command_path("system", "getDesktopOverview"),
            command_path("system", "getDesktopHome"),
            command_path("system", "getDesktopPlatformStatus"),
            command_path("system", "getDesktopClaudeCodeOverview"),
            command_path("settings", "probeBarkTest"),
            command_path("settings", "probeInstallHooks"),
            command_path("settings", "probeLogsDbDryRun"),
            command_path("settings", "probeLogsDbExecute"),
            command_path("updates", "updatesCheck"),
            command_path("updates", "updatesInstall"),
            command_path("updates", "updatesPrune"),
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
    fn tauri_command_guard_does_not_embed_retired_compat_tokens_in_tests() {
        let test_source = include_str!("lib.rs")
            .split("\n#[cfg(test)]")
            .nth(1)
            .expect("lib source must include test section");
        for retired in [
            command_path("settings", "startProbeJob"),
            command_path("updates", "runUpdateAction"),
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
    fn tauri_invoke_handler_excludes_linux_web_host_command_surfaces() {
        let commands = registered_invoke_command_paths();
        for parts in [
            &["get", "Security"][..],
            &["save", "Security"][..],
            &["change", "Password"][..],
            &["security", "_status"][..],
            &["log", "in"][..],
            &["log", "out"][..],
            &["cs", "rf"][..],
            &["turn", "stile"][..],
            &["admin", "_password"][..],
            &["system", "d"][..],
            &["ngi", "nx"][..],
            &["system_update", "_prune"][..],
            &["desktop_update", "_prune"][..],
            &["prune", "_backups"][..],
        ] {
            let forbidden = concat_token(parts);
            assert!(
                commands.iter().all(|command| !command.contains(&forbidden)),
                "macOS desktop invoke handler must not register Linux Web host command surface: {forbidden}"
            );
        }
    }
}
