use crate::{
    codex::resolve_codex_paths,
    config::{
        valid_probe_notification_server_url, CodexProbeConfigPatch, Config, ProbeConfig,
        ProbeConfigFilePatch, ProbeHooksConfigPatch, ProbeLogsDbConfig, ProbeNotificationsConfig,
        ProbeNotificationsConfigPatch, ProbeObservabilityConfigPatch, ProbeSettingsPatch,
    },
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const PROBE_BARK_DEVICE_KEY_SETTING: &str = "probe_bark_device_key";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeSecretState {
    Configured,
    Missing,
}

impl ProbeSecretState {
    pub fn from_secret_bytes(value: Option<&[u8]>) -> Self {
        match value {
            Some(value) if !value.is_empty() => Self::Configured,
            _ => Self::Missing,
        }
    }

    pub fn is_configured(self) -> bool {
        matches!(self, Self::Configured)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettingsView {
    pub codex: CodexSettingsView,
    pub probe: ProbeConfig,
    pub notifications: ProbeNotificationsSettingsView,
    pub logs_db: ProbeLogsDbConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexSettingsView {
    pub home: Option<String>,
    pub configured_codex_home: Option<String>,
    pub resolved_codex_home: String,
    pub codex_home_source: String,
    pub logs_db_source: String,
    pub discovery_warnings: Vec<String>,
    pub workspace: String,
    pub host_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeNotificationsSettingsView {
    pub device_key_configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_key: Option<String>,
    pub server_url: String,
    pub enabled: bool,
    pub sound: Option<String>,
    pub group: String,
    pub url: Option<String>,
    pub notify_completion: bool,
    pub notify_reply_needed: bool,
    pub notify_recoverable: bool,
}

pub fn build_settings_view(config: &Config, bark_device_key: ProbeSecretState) -> SettingsView {
    let resolved = resolve_codex_paths(&config.codex.home);
    SettingsView {
        codex: CodexSettingsView {
            home: resolved.configured_codex_home.clone(),
            configured_codex_home: resolved.configured_codex_home,
            resolved_codex_home: resolved.home.display().to_string(),
            codex_home_source: resolved.codex_home_source,
            logs_db_source: resolved.logs_db_source,
            discovery_warnings: resolved.discovery_warnings,
            workspace: config.codex.workspace.display().to_string(),
            host_label: config.codex.host_label.clone(),
        },
        probe: config.probe.clone(),
        notifications: probe_notifications_settings_view(
            &config.probe.notifications,
            bark_device_key,
        ),
        logs_db: config.probe.logs_db.clone(),
    }
}

pub fn probe_notifications_settings_view(
    notifications: &ProbeNotificationsConfig,
    bark_device_key: ProbeSecretState,
) -> ProbeNotificationsSettingsView {
    ProbeNotificationsSettingsView {
        device_key_configured: bark_device_key.is_configured(),
        device_key: None,
        server_url: notifications.server_url.clone(),
        enabled: notifications.enabled,
        sound: notifications.sound.clone(),
        group: notifications.group.clone(),
        url: notifications.url.clone(),
        notify_completion: notifications.notify_completion,
        notify_reply_needed: notifications.notify_reply_needed,
        notify_recoverable: notifications.notify_recoverable,
    }
}

pub fn normalize_bark_device_key(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

pub fn merge_probe_notification_patch(
    target: &mut ProbeNotificationsConfigPatch,
    source: ProbeNotificationsConfigPatch,
) {
    if source.enabled.is_some() {
        target.enabled = source.enabled;
    }
    if source.server_url.is_some() {
        target.server_url = source.server_url;
    }
    if source.sound.is_some() {
        target.sound = source.sound;
    }
    if source.group.is_some() {
        target.group = source.group;
    }
    if source.url.is_some() {
        target.url = source.url;
    }
    if source.notify_completion.is_some() {
        target.notify_completion = source.notify_completion;
    }
    if source.notify_reply_needed.is_some() {
        target.notify_reply_needed = source.notify_reply_needed;
    }
    if source.notify_recoverable.is_some() {
        target.notify_recoverable = source.notify_recoverable;
    }
}

pub fn normalize_probe_config_file_patch(
    patch: ProbeConfigFilePatch,
) -> Result<ProbeConfigFilePatch> {
    Ok(ProbeConfigFilePatch {
        codex: patch.codex.map(normalize_codex_patch),
        probe: patch
            .probe
            .map(normalize_probe_settings_patch)
            .transpose()?,
    })
}

pub fn normalize_probe_settings_patch(mut patch: ProbeSettingsPatch) -> Result<ProbeSettingsPatch> {
    patch.poll_seconds = patch.poll_seconds.map(|value| value.clamp(5, 3_600));
    patch.recent_limit = patch.recent_limit.map(|value| value.clamp(1, 500));
    patch.hooks = patch.hooks.map(normalize_probe_hooks_patch);
    patch.notifications = patch
        .notifications
        .map(normalize_probe_notifications_patch)
        .transpose()?;
    patch.observability = patch.observability.map(normalize_probe_observability_patch);
    patch.logs_db = patch.logs_db.map(normalize_probe_logs_db_patch);
    Ok(patch)
}

fn normalize_codex_patch(mut patch: CodexProbeConfigPatch) -> CodexProbeConfigPatch {
    patch.workspace = normalize_optional_string(patch.workspace);
    patch.host_label = normalize_optional_string(patch.host_label);
    patch
}

fn normalize_probe_hooks_patch(patch: ProbeHooksConfigPatch) -> ProbeHooksConfigPatch {
    ProbeHooksConfigPatch {
        manage_stop_hook: patch.manage_stop_hook,
        reload_app_server_after_install: patch.reload_app_server_after_install,
    }
}

fn normalize_probe_notifications_patch(
    mut patch: ProbeNotificationsConfigPatch,
) -> Result<ProbeNotificationsConfigPatch> {
    if let Some(server_url) = patch.server_url.take() {
        let server_url = server_url.trim().to_string();
        if !valid_probe_notification_server_url(&server_url) {
            bail!("probe notifications server_url must use HTTPS except localhost HTTP");
        }
        patch.server_url = Some(server_url);
    }
    if let Some(group) = patch.group.take() {
        let group = group.trim();
        patch.group = Some(if group.is_empty() {
            "NexusHub".to_string()
        } else {
            group.to_string()
        });
    }
    patch.sound = normalize_optional_nullable_string(patch.sound);
    patch.url = normalize_optional_nullable_string(patch.url);
    Ok(patch)
}

fn normalize_probe_observability_patch(
    mut patch: ProbeObservabilityConfigPatch,
) -> ProbeObservabilityConfigPatch {
    patch.hook_event_max_lines = patch
        .hook_event_max_lines
        .map(|value| value.clamp(10, 10_000));
    patch.hook_cooldown_max_lines = patch
        .hook_cooldown_max_lines
        .map(|value| value.clamp(10, 10_000));
    patch.log_max_bytes = patch
        .log_max_bytes
        .map(|value| value.clamp(4_096, 8_388_608));
    patch
}

fn normalize_probe_logs_db_patch(
    mut patch: crate::config::ProbeLogsDbConfigPatch,
) -> crate::config::ProbeLogsDbConfigPatch {
    patch.retention_days = patch.retention_days.map(|value| value.clamp(1, 3_650));
    patch.maintenance_interval_hours = patch
        .maintenance_interval_hours
        .map(|value| value.clamp(1, 24 * 365));
    patch.codex_exit_grace_seconds = patch
        .codex_exit_grace_seconds
        .map(|value| value.clamp(0, 3_600));
    patch.codex_exit_max_wait_seconds = patch
        .codex_exit_max_wait_seconds
        .map(|value| value.clamp(1, 86_400));
    patch.delete_chunk_rows = patch
        .delete_chunk_rows
        .map(|value| value.clamp(1, 1_000_000));
    patch.max_delete_rows_per_run = patch
        .max_delete_rows_per_run
        .map(|value| value.clamp(1, 10_000_000));
    patch.busy_timeout_ms = patch.busy_timeout_ms.map(|value| value.clamp(100, 120_000));
    patch.compact_interval_hours = patch
        .compact_interval_hours
        .map(|value| value.clamp(1, 24 * 365));
    patch.compact_min_freelist_ratio_percent = patch
        .compact_min_freelist_ratio_percent
        .map(|value| value.clamp(1, 100));
    patch
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn normalize_optional_nullable_string(value: Option<Option<String>>) -> Option<Option<String>> {
    value.map(|inner| {
        inner.and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
    })
}
