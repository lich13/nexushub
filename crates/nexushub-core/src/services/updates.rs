use crate::{
    config::Config,
    platform::{PlatformKind, PlatformPaths},
    update::{self, analyze_job_failure, JobFailureCategory},
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateExecutionMethod {
    LinuxSystemdJob,
    MacosTauriUpdater,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateState {
    Idle,
    Checking,
    Ready,
    Installing,
    Succeeded,
    Failed,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateAction {
    Check,
    Install,
    Prune,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdateFailureCategory {
    ReleaseMissing,
    DownloadSha256Mismatch,
    SystemdFailure,
    NginxFailure,
    PermissionDeniedSudo,
    CodexAuthFailure,
    SqliteIntegrityFailure,
    ReadOnlyFileSystem,
    NetworkTlsEof,
    CodexLocalStateUnavailable,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateStatus {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: Option<bool>,
    pub channel: String,
    pub method: UpdateExecutionMethod,
    pub state: UpdateState,
    pub failure_category: Option<UpdateFailureCategory>,
    pub recommended_action: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateJobSpec {
    pub kind: String,
    pub title: String,
    pub command: String,
    pub exclusive_group: Option<String>,
}

pub fn update_status(
    config: &Config,
    platform: &PlatformPaths,
    latest_version: Option<&str>,
    last_error: Option<&str>,
) -> UpdateStatus {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let normalized_latest = latest_version.and_then(non_empty_string);
    let update_available =
        normalized_latest.map(|latest| version_tag_value(latest) != current_version);
    let failure_category = last_error.and_then(|error| {
        analyze_job_failure("nexushub_update", "", Some(error), Some(1))
            .map(|analysis| analysis.category.into())
    });
    let state = if failure_category.is_some() {
        UpdateState::Failed
    } else {
        UpdateState::Idle
    };
    let method = execution_method(platform.kind);
    let capabilities = capabilities_for(platform.kind);
    let recommended_action = recommended_action(config, platform.kind, method, update_available);

    UpdateStatus {
        current_version,
        latest_version: normalized_latest.map(ToString::to_string),
        update_available,
        channel: "stable".to_string(),
        method,
        state,
        failure_category,
        recommended_action,
        capabilities,
    }
}

pub fn update_action_job_spec(
    config: &Config,
    platform: &PlatformPaths,
    action: UpdateAction,
) -> Result<UpdateJobSpec> {
    match (platform.kind, action) {
        (PlatformKind::Linux, UpdateAction::Check) => Ok(UpdateJobSpec {
            kind: "nexushub_update_check".to_string(),
            title: "NexusHub update precheck".to_string(),
            command: config.update.panel_precheck_command.clone(),
            exclusive_group: Some("nexushub-update".to_string()),
        }),
        (PlatformKind::Linux, UpdateAction::Install) => Ok(UpdateJobSpec {
            kind: "nexushub_update_install".to_string(),
            title: "NexusHub update install".to_string(),
            command: update::panel_update_command(&config.update.panel_update_command),
            exclusive_group: Some("nexushub-update".to_string()),
        }),
        (PlatformKind::Linux, UpdateAction::Prune) => Ok(UpdateJobSpec {
            kind: "nexushub_update_prune".to_string(),
            title: "NexusHub update backup prune".to_string(),
            command: update::panel_prune_command(),
            exclusive_group: Some("nexushub-update".to_string()),
        }),
        (PlatformKind::Macos, _) => Err(anyhow!(
            "macOS updates must run through the signed Tauri updater, not shell jobs"
        )),
        (PlatformKind::Windows, _) => {
            Err(anyhow!("Windows updates are not supported in this release"))
        }
    }
}

fn non_empty_string(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

fn version_tag_value(value: &str) -> &str {
    value.trim_start_matches('v')
}

fn execution_method(platform: PlatformKind) -> UpdateExecutionMethod {
    match platform {
        PlatformKind::Linux => UpdateExecutionMethod::LinuxSystemdJob,
        PlatformKind::Macos => UpdateExecutionMethod::MacosTauriUpdater,
        PlatformKind::Windows => UpdateExecutionMethod::Unsupported,
    }
}

fn capabilities_for(platform: PlatformKind) -> Vec<String> {
    match platform {
        PlatformKind::Linux => [
            "check",
            "confirm_install",
            "job_history",
            "sha256_verification",
            "systemd_health_check",
            "rollback",
            "prune_backups",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
        PlatformKind::Macos => [
            "check",
            "confirm_install",
            "job_history",
            "signature_verification",
            "restart_after_install",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
        PlatformKind::Windows => Vec::new(),
    }
}

fn recommended_action(
    config: &Config,
    platform: PlatformKind,
    method: UpdateExecutionMethod,
    update_available: Option<bool>,
) -> String {
    match (platform, method, update_available) {
        (PlatformKind::Linux, UpdateExecutionMethod::LinuxSystemdJob, Some(true)) => {
            update::panel_update_command(&config.update.panel_update_command)
        }
        (PlatformKind::Linux, UpdateExecutionMethod::LinuxSystemdJob, _) => {
            "Run the fixed Linux update precheck before installing a new release.".to_string()
        }
        (PlatformKind::Macos, UpdateExecutionMethod::MacosTauriUpdater, Some(true)) => {
            "Confirm install in the Tauri updater after signature verification.".to_string()
        }
        (PlatformKind::Macos, UpdateExecutionMethod::MacosTauriUpdater, _) => {
            "Use the Tauri updater to check signed app releases.".to_string()
        }
        _ => "Updates are unavailable on this platform in the current release.".to_string(),
    }
}

impl From<JobFailureCategory> for UpdateFailureCategory {
    fn from(value: JobFailureCategory) -> Self {
        match value {
            JobFailureCategory::ReleaseMissing => Self::ReleaseMissing,
            JobFailureCategory::DownloadSha256Mismatch => Self::DownloadSha256Mismatch,
            JobFailureCategory::SystemdFailure => Self::SystemdFailure,
            JobFailureCategory::NginxFailure => Self::NginxFailure,
            JobFailureCategory::PermissionDeniedSudo => Self::PermissionDeniedSudo,
            JobFailureCategory::CodexAuthFailure => Self::CodexAuthFailure,
            JobFailureCategory::SqliteIntegrityFailure => Self::SqliteIntegrityFailure,
            JobFailureCategory::ReadOnlyFileSystem => Self::ReadOnlyFileSystem,
            JobFailureCategory::NetworkTlsEof => Self::NetworkTlsEof,
            JobFailureCategory::CodexLocalStateUnavailable => Self::CodexLocalStateUnavailable,
            JobFailureCategory::Unknown => Self::Unknown,
        }
    }
}
