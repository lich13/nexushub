use nexushub_core::{
    config::Config,
    platform::{PlatformKind, PlatformPaths},
    services::updates::{
        linux_update_job_spec, update_action_plan, update_status, UpdateAction,
        UpdateExecutionMethod, UpdateFailureCategory, UpdateState,
    },
};

#[test]
fn linux_update_status_uses_fixed_panel_job_executor() {
    let config = Config::for_platform_kind(PlatformKind::Linux);
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);

    let status = update_status(&config, &platform, Some("v999.0.0"), None);

    assert_eq!(status.current_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(status.latest_version.as_deref(), Some("v999.0.0"));
    assert_eq!(status.update_available, Some(true));
    assert_eq!(status.channel, "stable");
    assert_eq!(status.method, UpdateExecutionMethod::LinuxSystemdJob);
    assert_eq!(status.state, UpdateState::Idle);
    assert_eq!(status.failure_category, None);
    assert!(status
        .recommended_action
        .contains("Linux server update job"));
    assert!(status
        .capabilities
        .iter()
        .any(|capability| capability == "job_history"));
    assert!(status
        .capabilities
        .iter()
        .any(|capability| capability == "rollback"));
}

#[test]
fn update_status_does_not_downgrade_or_reinstall_same_version() {
    let config = Config::for_platform_kind(PlatformKind::Macos);
    let platform = PlatformPaths::for_kind(PlatformKind::Macos);

    let older = update_status(&config, &platform, Some("v0.1.105"), None);
    let same = update_status(&config, &platform, Some(env!("CARGO_PKG_VERSION")), None);

    assert_eq!(older.update_available, Some(false));
    assert_eq!(same.update_available, Some(false));
    assert!(older.recommended_action.contains("Tauri updater"));
}

#[test]
fn macos_update_status_uses_tauri_updater_without_linux_leakage() {
    let home = temp_dir("nexushub-updates-macos-status");
    std::fs::create_dir_all(&home).unwrap();
    let config = Config::for_platform_kind_with_home(PlatformKind::Macos, &home);
    let platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &home);

    let status = update_status(
        &config,
        &platform,
        Some("v0.1.103"),
        Some("network timeout"),
    );
    let serialized = serde_json::to_string(&status).unwrap();

    assert_eq!(status.method, UpdateExecutionMethod::MacosTauriUpdater);
    assert_eq!(status.state, UpdateState::Failed);
    assert_eq!(
        status.failure_category,
        Some(UpdateFailureCategory::NetworkTlsEof)
    );
    assert!(status.recommended_action.contains("Tauri updater"));
    assert!(status
        .capabilities
        .iter()
        .any(|capability| capability == "signature_verification"));
    assert!(!serialized.contains("systemctl"));
    assert!(!serialized.contains("nginx"));
    assert!(!serialized.contains("/opt/nexushub"));
    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn update_action_plans_are_shared_and_platform_scoped_without_shell_commands() {
    let linux_platform = PlatformPaths::for_kind(PlatformKind::Linux);

    let start = update_action_plan(&linux_platform, UpdateAction::Install);
    assert_eq!(start.action, UpdateAction::Install);
    assert_eq!(start.method, UpdateExecutionMethod::LinuxSystemdJob);
    assert!(start.exclusive);

    let mac_home = temp_dir("nexushub-updates-macos-job");
    std::fs::create_dir_all(&mac_home).unwrap();
    let mac_platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &mac_home);

    let mac_plan = update_action_plan(&mac_platform, UpdateAction::Install);
    assert_eq!(mac_plan.method, UpdateExecutionMethod::MacosTauriUpdater);
    assert!(!mac_plan.exclusive);
    std::fs::remove_dir_all(mac_home).unwrap();
}

#[test]
fn linux_update_job_specs_are_planned_in_core_service() {
    let mut config = Config::for_platform_kind(PlatformKind::Linux);
    config.update.panel_precheck_command = "nexushub-update --precheck".to_string();
    config.update.panel_update_command = "nexushub-update --install".to_string();
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);

    let precheck =
        linux_update_job_spec(&config, update_action_plan(&platform, UpdateAction::Check)).unwrap();
    assert_eq!(precheck.kind, "nexushub_update_check");
    assert_eq!(precheck.title, "NexusHub update precheck");
    assert_eq!(precheck.command, "nexushub-update --precheck");
    assert_eq!(precheck.exclusive_group.as_deref(), Some("nexushub-update"));

    let install = linux_update_job_spec(
        &config,
        update_action_plan(&platform, UpdateAction::Install),
    )
    .unwrap();
    assert_eq!(install.kind, "nexushub_update_install");
    assert_eq!(install.command, "nexushub-update --install");

    let prune =
        linux_update_job_spec(&config, update_action_plan(&platform, UpdateAction::Prune)).unwrap();
    assert_eq!(prune.kind, "nexushub_update_prune");
    assert!(prune.command.contains("release update backups"));

    let mac = PlatformPaths::for_kind(PlatformKind::Macos);
    assert!(
        linux_update_job_spec(&config, update_action_plan(&mac, UpdateAction::Check),)
            .unwrap_err()
            .to_string()
            .contains("only Linux WebUI")
    );
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    let unique = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    std::env::temp_dir().join(format!("{label}-{unique}"))
}
