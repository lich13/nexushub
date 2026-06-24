mod commands;
// 业务命令入口按领域放在 commands/*；overview 仅保留桌面状态、首页汇总和启动初始化。
#[allow(dead_code)]
mod overview;
mod services;

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
        ] {
            assert!(
                !overview_source.contains(forbidden),
                "overview.rs must not define command adapter DTO: {forbidden}"
            );
        }
    }

    #[test]
    fn tauri_thread_commands_use_core_thread_query_and_detail_plans() {
        let threads_source = include_str!("commands/threads.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or(include_str!("commands/threads.rs"));
        let thread_service_source = include_str!("services/threads.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or(include_str!("services/threads.rs"));

        for required in [
            "thread_summaries_with_query(",
            "thread_service::plan_thread_detail_request",
            "thread_service::plan_thread_blocks_request",
            "thread_service::window_thread_detail_for_plan",
            "thread_service::thread_blocks_page_for_plan",
            "job_service::plan_thread_send_with_capability",
            "job_service::plan_thread_steer_with_capability",
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
            thread_service_source.contains("thread_service::plan_threads_list_request")
                && thread_service_source.contains("thread_service::build_threads_overview"),
            "desktop thread service must consume shared core thread list plans"
        );
    }

    #[test]
    fn tauri_settings_commands_use_core_settings_view_and_secret_write_plans() {
        let settings_source = include_str!("commands/settings.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("settings source must include production section");

        for required in [
            "settings_service::probe_settings_view_with_capability",
            "for secret_write in plan.secret_writes",
        ] {
            assert!(
                settings_source.contains(required),
                "Tauri settings adapter must consume shared core settings facade: {required}"
            );
        }

        assert!(
            !settings_source.contains("if let Some(device_key) = plan.bark_device_key"),
            "Tauri settings adapter must not special-case Probe secret writes outside the core plan"
        );
    }

    #[test]
    fn tauri_commands_do_not_reimplement_migrated_goal_or_followup_transactions() {
        let settings_source = include_str!("commands/settings.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .expect("settings source must include production section");
        let threads_source = include_str!("commands/threads.rs")
            .split("\n#[cfg(test)]\nmod tests")
            .next()
            .unwrap_or(include_str!("commands/threads.rs"));

        for required in [
            "goal_service::goal_get_response_with_capability",
            "goal_service::save_goal_with_capability",
            "goal_service::clear_goal_with_capability",
            "goal_service::pause_goal_with_capability",
            "goal_service::resume_goal_with_capability",
            "upload_service::plan_store_uploads_with_capability",
            "upload_service::plan_delete_upload_with_capability",
            "cleanup_service::dry_run_archived_with_capability",
            "cleanup_service::execute_archived_with_capability",
            "cleanup_service::dry_run_hidden_with_capability",
            "cleanup_service::execute_hidden_with_capability",
        ] {
            assert!(
                settings_source.contains(required),
                "Tauri settings commands must call the shared core facade/plan: {required}"
            );
        }
        for required in [
            "job_service::list_followups_with_capability",
            "job_service::enqueue_followup_with_capability",
            "job_service::cancel_followup_with_capability",
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
            ".list_followups(",
            ".enqueue_followup(",
            ".cancel_followup(",
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
}
