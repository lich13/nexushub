use crate::{
    config::Config,
    platform::{PlatformKind, PlatformPaths},
    system::{compare_semver, extract_semver},
    update::{analyze_job_failure, JobFailureCategory},
};
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateJobPlan {
    pub action: UpdateAction,
    pub method: UpdateExecutionMethod,
    pub platform: PlatformKind,
    pub exclusive: bool,
}

pub fn update_status(
    _config: &Config,
    platform: &PlatformPaths,
    latest_version: Option<&str>,
    last_error: Option<&str>,
) -> UpdateStatus {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let normalized_latest = latest_version.and_then(non_empty_string);
    let update_available = normalized_latest
        .and_then(|latest| update_available_for_versions(&current_version, latest));
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
    let recommended_action = recommended_action(platform.kind, method, update_available);

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

pub fn update_action_plan(platform: &PlatformPaths, action: UpdateAction) -> UpdateJobPlan {
    UpdateJobPlan {
        action,
        method: execution_method(platform.kind),
        platform: platform.kind,
        exclusive: platform.kind == PlatformKind::Linux,
    }
}

fn non_empty_string(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then_some(trimmed)
}

pub fn update_available_for_versions(current_version: &str, latest_version: &str) -> Option<bool> {
    Some(
        compare_semver(
            extract_semver(latest_version)?.as_str(),
            extract_semver(current_version)?.as_str(),
        )?
        .is_gt(),
    )
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
    platform: PlatformKind,
    method: UpdateExecutionMethod,
    update_available: Option<bool>,
) -> String {
    match (platform, method, update_available) {
        (PlatformKind::Linux, UpdateExecutionMethod::LinuxSystemdJob, Some(true)) => {
            "Confirm install to start the Linux server update job after precheck.".to_string()
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
