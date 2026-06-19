use nexushub_core::{
    config::{
        Config, ProbeNotificationsConfigPatch, ProbeObservabilityConfigPatch, ProbeSettingsPatch,
    },
    platform::{PlatformKind, PlatformPaths},
    services::{
        settings::{
            build_settings_view, merge_probe_notification_patch, normalize_bark_device_key,
            normalize_probe_settings_patch, ProbeSecretState, ProbeSettingsSaveRequest,
        },
        system::system_capabilities,
    },
};

#[test]
fn linux_capabilities_expose_web_host_only_features() {
    let config = Config::for_platform_kind(PlatformKind::Linux);
    let capabilities = system_capabilities(&config, &PlatformPaths::for_kind(PlatformKind::Linux));

    assert!(capabilities.threads);
    assert!(capabilities.jobs);
    assert!(capabilities.probe);
    assert!(capabilities.status);
    assert!(capabilities.settings);
    assert!(capabilities.job_history);
    assert!(capabilities.app_updater);
    assert!(capabilities.web_auth);
    assert!(capabilities.security_settings);
    assert!(capabilities.turnstile);
    assert!(capabilities.systemd);
    assert!(capabilities.nginx);
    assert!(capabilities.public_endpoint);
    assert!(capabilities.admin_password);
    assert!(capabilities.linux_update_job);
    assert!(capabilities.prune_backups);
}

#[test]
fn macos_capabilities_keep_shared_core_but_disable_linux_web_host_features() {
    let home = temp_dir("nexushub-capabilities-macos");
    std::fs::create_dir_all(&home).unwrap();
    let config = Config::for_platform_kind_with_home(PlatformKind::Macos, &home);
    let capabilities = system_capabilities(
        &config,
        &PlatformPaths::for_kind_with_home(PlatformKind::Macos, &home),
    );

    assert!(capabilities.threads);
    assert!(capabilities.jobs);
    assert!(capabilities.probe);
    assert!(capabilities.status);
    assert!(capabilities.settings);
    assert!(capabilities.job_history);
    assert!(capabilities.app_updater);
    assert!(!capabilities.web_auth);
    assert!(!capabilities.security_settings);
    assert!(!capabilities.turnstile);
    assert!(!capabilities.systemd);
    assert!(!capabilities.nginx);
    assert!(!capabilities.public_endpoint);
    assert!(!capabilities.admin_password);
    assert!(!capabilities.linux_update_job);
    assert!(!capabilities.prune_backups);

    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn bark_device_key_is_trimmed_and_empty_values_are_ignored() {
    assert_eq!(
        normalize_bark_device_key(Some("  bark-key-123  ".to_string())),
        Some("bark-key-123".to_string())
    );
    assert_eq!(normalize_bark_device_key(Some(" \n\t ".to_string())), None);
    assert_eq!(normalize_bark_device_key(None), None);
}

#[test]
fn settings_view_reports_secret_state_without_returning_secret() {
    let mut config = Config::for_platform_kind(PlatformKind::Linux);
    config.probe.notifications.enabled = true;
    config.probe.notifications.server_url = "https://bark.example.com".to_string();

    let view = build_settings_view(&config, ProbeSecretState::Configured);
    let serialized = serde_json::to_string(&view).unwrap();

    assert_eq!(
        view.probe.notifications.server_url,
        "https://bark.example.com"
    );
    assert!(view.notifications.device_key_configured);
    assert!(view.notifications.device_key.is_none());
    assert!(!serialized.contains("bark-key-123"));
    assert!(!serialized.contains("device_key\":\""));
}

#[test]
fn probe_settings_patch_validation_rejects_bad_url_and_clamps_numeric_ranges() {
    let invalid_url = ProbeSettingsPatch {
        notifications: Some(ProbeNotificationsConfigPatch {
            server_url: Some("http://example.com".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert!(normalize_probe_settings_patch(invalid_url)
        .unwrap_err()
        .to_string()
        .contains("server_url"));

    let patch = ProbeSettingsPatch {
        poll_seconds: Some(1),
        recent_limit: Some(5_000),
        notifications: Some(ProbeNotificationsConfigPatch {
            server_url: Some("  http://127.0.0.1:8080  ".to_string()),
            group: Some("   ".to_string()),
            ..Default::default()
        }),
        observability: Some(ProbeObservabilityConfigPatch {
            hook_event_max_lines: Some(1),
            hook_cooldown_max_lines: Some(50_000),
            log_max_bytes: Some(1),
        }),
        ..Default::default()
    };

    let normalized = normalize_probe_settings_patch(patch).unwrap();
    assert_eq!(normalized.poll_seconds, Some(5));
    assert_eq!(normalized.recent_limit, Some(500));
    let notifications = normalized.notifications.unwrap();
    assert_eq!(
        notifications.server_url.as_deref(),
        Some("http://127.0.0.1:8080")
    );
    assert_eq!(notifications.group.as_deref(), Some("NexusHub"));
    let observability = normalized.observability.unwrap();
    assert_eq!(observability.hook_event_max_lines, Some(10));
    assert_eq!(observability.hook_cooldown_max_lines, Some(10_000));
    assert_eq!(observability.log_max_bytes, Some(4_096));
}

#[test]
fn probe_settings_patch_clamps_log_max_bytes_at_shared_frontend_boundary() {
    let over_max = normalize_probe_settings_patch(ProbeSettingsPatch {
        poll_seconds: Some(3_601),
        recent_limit: Some(0),
        observability: Some(ProbeObservabilityConfigPatch {
            log_max_bytes: Some(8_388_609),
            ..Default::default()
        }),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(over_max.poll_seconds, Some(3_600));
    assert_eq!(over_max.recent_limit, Some(1));
    assert_eq!(
        over_max.observability.unwrap().log_max_bytes,
        Some(8_388_608)
    );

    let at_max = normalize_probe_settings_patch(ProbeSettingsPatch {
        observability: Some(ProbeObservabilityConfigPatch {
            log_max_bytes: Some(8_388_608),
            ..Default::default()
        }),
        ..Default::default()
    })
    .unwrap();

    assert_eq!(at_max.observability.unwrap().log_max_bytes, Some(8_388_608));
}

#[test]
fn notification_patch_merge_preserves_existing_fields_when_source_omits_them() {
    let mut target = ProbeNotificationsConfigPatch {
        enabled: Some(true),
        server_url: Some("https://api.day.app".to_string()),
        group: Some("Ops".to_string()),
        ..Default::default()
    };

    merge_probe_notification_patch(
        &mut target,
        ProbeNotificationsConfigPatch {
            sound: Some(Some("alarm".to_string())),
            group: Some("  ".to_string()),
            ..Default::default()
        },
    );
    let normalized = normalize_probe_settings_patch(ProbeSettingsPatch {
        notifications: Some(target),
        ..Default::default()
    })
    .unwrap()
    .notifications
    .unwrap();

    assert_eq!(normalized.enabled, Some(true));
    assert_eq!(
        normalized.server_url.as_deref(),
        Some("https://api.day.app")
    );
    assert_eq!(normalized.sound, Some(Some("alarm".to_string())));
    assert_eq!(normalized.group.as_deref(), Some("NexusHub"));
}

#[test]
fn probe_settings_save_request_normalizes_bark_key_and_merges_notification_patches() {
    let request: ProbeSettingsSaveRequest = serde_json::from_value(serde_json::json!({
        "probe": {
            "notifications": {
                "enabled": true,
                "server_url": "  https://bark.example.com  ",
                "device_key": " nested-key ",
                "notify_completion": true
            }
        },
        "notifications": {
            "group": " Ops ",
            "device_key": " top-key "
        }
    }))
    .unwrap();

    let normalized = request.normalize().unwrap();

    assert_eq!(normalized.bark_device_key.as_deref(), Some("top-key"));
    let notifications = normalized
        .config_patch
        .probe
        .unwrap()
        .notifications
        .unwrap();
    assert_eq!(notifications.enabled, Some(true));
    assert_eq!(
        notifications.server_url.as_deref(),
        Some("https://bark.example.com")
    );
    assert_eq!(notifications.group.as_deref(), Some("Ops"));
    assert_eq!(notifications.notify_completion, Some(true));
}

#[test]
fn probe_settings_save_request_keeps_nested_bark_key_when_top_level_is_blank() {
    let request: ProbeSettingsSaveRequest = serde_json::from_value(serde_json::json!({
        "probe": {
            "notifications": {
                "device_key": " nested-key "
            }
        },
        "notifications": {
            "device_key": "  "
        }
    }))
    .unwrap();

    let normalized = request.normalize().unwrap();

    assert_eq!(normalized.bark_device_key.as_deref(), Some("nested-key"));
    assert!(normalized.config_patch.probe.is_none());
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
