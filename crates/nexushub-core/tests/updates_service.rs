use nexushub_core::{
    config::Config,
    platform::{PlatformKind, PlatformPaths},
    services::updates::{
        update_action_job_spec, update_status, UpdateAction, UpdateExecutionMethod,
        UpdateFailureCategory, UpdateState,
    },
};

#[test]
fn linux_update_status_uses_fixed_panel_job_executor() {
    let config = Config::for_platform_kind(PlatformKind::Linux);
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);

    let status = update_status(&config, &platform, Some("v0.1.102"), None);

    assert_eq!(status.current_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(status.latest_version.as_deref(), Some("v0.1.102"));
    assert_eq!(status.update_available, Some(true));
    assert_eq!(status.channel, "stable");
    assert_eq!(status.method, UpdateExecutionMethod::LinuxSystemdJob);
    assert_eq!(status.state, UpdateState::Idle);
    assert_eq!(status.failure_category, None);
    assert!(status
        .recommended_action
        .contains("/usr/local/bin/nexushub-update"));
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
fn macos_update_status_uses_tauri_updater_without_linux_leakage() {
    let home = temp_dir("nexushub-updates-macos-status");
    std::fs::create_dir_all(&home).unwrap();
    let config = Config::for_platform_kind_with_home(PlatformKind::Macos, &home);
    let platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &home);

    let status = update_status(
        &config,
        &platform,
        Some("v0.1.101"),
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
fn update_action_job_specs_are_shared_and_platform_scoped() {
    let linux_config = Config::for_platform_kind(PlatformKind::Linux);
    let linux_platform = PlatformPaths::for_kind(PlatformKind::Linux);

    let start = update_action_job_spec(&linux_config, &linux_platform, UpdateAction::Install)
        .expect("linux update install job");
    assert_eq!(start.kind, "nexushub_update_install");
    assert_eq!(start.title, "NexusHub update install");
    assert!(start.command.contains("nexushub-update"));
    assert!(start.exclusive_group.as_deref() == Some("nexushub-update"));

    let prune = update_action_job_spec(&linux_config, &linux_platform, UpdateAction::Prune)
        .expect("linux update prune job");
    assert_eq!(
        prune.exclusive_group.as_deref(),
        start.exclusive_group.as_deref(),
        "backup pruning must not run while install may need rollback backups",
    );

    let mac_home = temp_dir("nexushub-updates-macos-job");
    std::fs::create_dir_all(&mac_home).unwrap();
    let mac_config = Config::for_platform_kind_with_home(PlatformKind::Macos, &mac_home);
    let mac_platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &mac_home);

    let err = update_action_job_spec(&mac_config, &mac_platform, UpdateAction::Install)
        .expect_err("macOS updater must run through Tauri, not shell job");
    assert!(err.to_string().contains("Tauri updater"));
    std::fs::remove_dir_all(mac_home).unwrap();
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
