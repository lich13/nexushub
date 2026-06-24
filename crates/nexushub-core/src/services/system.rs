use crate::{
    config::Config,
    platform::{PlatformKind, PlatformPaths},
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Threads,
    Jobs,
    Probe,
    Status,
    Settings,
    JobHistory,
    AppUpdater,
    ThreadCleanup,
    ProbeLogMaintenance,
    ThreadArchiveActions,
    WebAuth,
    Csrf,
    SecuritySettings,
    Turnstile,
    Systemd,
    Nginx,
    PublicEndpoint,
    AdminPassword,
    LinuxUpdateJob,
    PruneBackups,
}

impl Capability {
    pub const ALL: &'static [Capability] = &[
        Capability::Threads,
        Capability::Jobs,
        Capability::Probe,
        Capability::Status,
        Capability::Settings,
        Capability::JobHistory,
        Capability::AppUpdater,
        Capability::ThreadCleanup,
        Capability::ProbeLogMaintenance,
        Capability::ThreadArchiveActions,
        Capability::WebAuth,
        Capability::Csrf,
        Capability::SecuritySettings,
        Capability::Turnstile,
        Capability::Systemd,
        Capability::Nginx,
        Capability::PublicEndpoint,
        Capability::AdminPassword,
        Capability::LinuxUpdateJob,
        Capability::PruneBackups,
    ];

    pub fn all() -> &'static [Capability] {
        Self::ALL
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Threads => "threads",
            Self::Jobs => "jobs",
            Self::Probe => "probe",
            Self::Status => "status",
            Self::Settings => "settings",
            Self::JobHistory => "job_history",
            Self::AppUpdater => "app_updater",
            Self::ThreadCleanup => "thread_cleanup",
            Self::ProbeLogMaintenance => "probe_log_maintenance",
            Self::ThreadArchiveActions => "thread_archive_actions",
            Self::WebAuth => "web_auth",
            Self::Csrf => "csrf",
            Self::SecuritySettings => "security_settings",
            Self::Turnstile => "turnstile",
            Self::Systemd => "systemd",
            Self::Nginx => "nginx",
            Self::PublicEndpoint => "public_endpoint",
            Self::AdminPassword => "admin_password",
            Self::LinuxUpdateJob => "linux_update_job",
            Self::PruneBackups => "prune_backups",
        }
    }

    pub fn is_supported_on(self, platform: &PlatformPaths) -> bool {
        let shared_core = matches!(platform.kind, PlatformKind::Linux | PlatformKind::Macos);
        let linux_web_host = matches!(platform.kind, PlatformKind::Linux);
        match self {
            Self::Threads
            | Self::Jobs
            | Self::Probe
            | Self::Status
            | Self::Settings
            | Self::JobHistory
            | Self::AppUpdater
            | Self::ThreadCleanup
            | Self::ProbeLogMaintenance
            | Self::ThreadArchiveActions => shared_core,
            Self::WebAuth
            | Self::Csrf
            | Self::SecuritySettings
            | Self::Turnstile
            | Self::Systemd
            | Self::Nginx
            | Self::PublicEndpoint
            | Self::AdminPassword
            | Self::LinuxUpdateJob
            | Self::PruneBackups => linux_web_host,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemCapabilities {
    pub threads: bool,
    pub jobs: bool,
    pub probe: bool,
    pub status: bool,
    pub settings: bool,
    pub job_history: bool,
    pub app_updater: bool,
    pub thread_cleanup: bool,
    pub probe_log_maintenance: bool,
    pub thread_archive_actions: bool,
    pub web_auth: bool,
    pub csrf: bool,
    pub security_settings: bool,
    pub turnstile: bool,
    pub systemd: bool,
    pub nginx: bool,
    pub public_endpoint: bool,
    pub admin_password: bool,
    pub linux_update_job: bool,
    pub prune_backups: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityGatePlan {
    pub capability: Capability,
    pub platform: PlatformKind,
    pub supported: bool,
    pub error: Option<String>,
}

pub fn capability_gate_plan(
    platform: &PlatformPaths,
    capability: Capability,
) -> CapabilityGatePlan {
    let supported = capability.is_supported_on(platform);
    CapabilityGatePlan {
        capability,
        platform: platform.kind,
        supported,
        error: (!supported).then(|| {
            format!(
                "{} is unavailable on {}",
                capability.as_str(),
                platform_kind_label(platform.kind)
            )
        }),
    }
}

pub fn require_capability(platform: &PlatformPaths, capability: Capability) -> Result<()> {
    if capability.is_supported_on(platform) {
        return Ok(());
    }
    bail!(
        "{} is unavailable on {}",
        capability.as_str(),
        platform_kind_label(platform.kind)
    )
}

pub fn system_capabilities(_config: &Config, platform: &PlatformPaths) -> SystemCapabilities {
    SystemCapabilities {
        threads: Capability::Threads.is_supported_on(platform),
        jobs: Capability::Jobs.is_supported_on(platform),
        probe: Capability::Probe.is_supported_on(platform),
        status: Capability::Status.is_supported_on(platform),
        settings: Capability::Settings.is_supported_on(platform),
        job_history: Capability::JobHistory.is_supported_on(platform),
        app_updater: Capability::AppUpdater.is_supported_on(platform),
        thread_cleanup: Capability::ThreadCleanup.is_supported_on(platform),
        probe_log_maintenance: Capability::ProbeLogMaintenance.is_supported_on(platform),
        thread_archive_actions: Capability::ThreadArchiveActions.is_supported_on(platform),
        web_auth: Capability::WebAuth.is_supported_on(platform),
        csrf: Capability::Csrf.is_supported_on(platform),
        security_settings: Capability::SecuritySettings.is_supported_on(platform),
        turnstile: Capability::Turnstile.is_supported_on(platform),
        systemd: Capability::Systemd.is_supported_on(platform),
        nginx: Capability::Nginx.is_supported_on(platform),
        public_endpoint: Capability::PublicEndpoint.is_supported_on(platform),
        admin_password: Capability::AdminPassword.is_supported_on(platform),
        linux_update_job: Capability::LinuxUpdateJob.is_supported_on(platform),
        prune_backups: Capability::PruneBackups.is_supported_on(platform),
    }
}

fn platform_kind_label(kind: PlatformKind) -> &'static str {
    match kind {
        PlatformKind::Linux => "linux",
        PlatformKind::Macos => "macos",
        PlatformKind::Windows => "windows",
    }
}

#[cfg(test)]
mod tests {
    use super::{capability_gate_plan, require_capability, system_capabilities, Capability};
    use crate::{config::Config, platform::PlatformPaths};

    #[test]
    fn linux_only_capabilities_are_allowed_only_on_linux() {
        let linux = PlatformPaths::for_kind(crate::platform::PlatformKind::Linux);
        let macos = PlatformPaths::for_kind(crate::platform::PlatformKind::Macos);

        assert!(require_capability(&linux, Capability::SecuritySettings).is_ok());
        assert!(require_capability(&linux, Capability::LinuxUpdateJob).is_ok());
        assert!(require_capability(&linux, Capability::WebAuth).is_ok());
        assert!(require_capability(&linux, Capability::Turnstile).is_ok());
        assert!(require_capability(&linux, Capability::Systemd).is_ok());
        assert!(require_capability(&linux, Capability::Nginx).is_ok());
        assert!(require_capability(&linux, Capability::PublicEndpoint).is_ok());
        assert!(require_capability(&linux, Capability::AdminPassword).is_ok());
        assert!(require_capability(&linux, Capability::PruneBackups).is_ok());

        let security_error = require_capability(&macos, Capability::SecuritySettings)
            .expect_err("macOS must not allow Linux web-host security settings");
        assert!(security_error
            .to_string()
            .contains("security_settings is unavailable on macos"));

        let update_error = require_capability(&macos, Capability::LinuxUpdateJob)
            .expect_err("macOS must not allow Linux update jobs");
        assert!(update_error
            .to_string()
            .contains("linux_update_job is unavailable on macos"));
    }

    #[test]
    fn local_maintenance_capabilities_are_shared_by_linux_and_macos_only() {
        let linux = PlatformPaths::for_kind(crate::platform::PlatformKind::Linux);
        let macos = PlatformPaths::for_kind(crate::platform::PlatformKind::Macos);
        let windows = PlatformPaths::for_kind(crate::platform::PlatformKind::Windows);

        for capability in [
            Capability::ThreadCleanup,
            Capability::ProbeLogMaintenance,
            Capability::ThreadArchiveActions,
        ] {
            assert!(require_capability(&linux, capability).is_ok());
            assert!(require_capability(&macos, capability).is_ok());
            assert!(require_capability(&windows, capability).is_err());
        }

        assert!(require_capability(&macos, Capability::LinuxUpdateJob).is_err());
        assert!(require_capability(&windows, Capability::ThreadCleanup).is_err());
        assert!(require_capability(&windows, Capability::ProbeLogMaintenance).is_err());
        assert!(require_capability(&windows, Capability::ThreadArchiveActions).is_err());
    }

    #[test]
    fn capability_matrix_matches_neutral_capability_gate() {
        let config = Config::for_platform_kind(crate::platform::PlatformKind::Windows);
        let platform = PlatformPaths::for_kind(crate::platform::PlatformKind::Windows);
        let matrix = system_capabilities(&config, &platform);

        assert!(!matrix.settings);
        assert!(require_capability(&platform, Capability::Settings).is_err());
        assert!(!matrix.thread_cleanup);
        assert!(!matrix.probe_log_maintenance);
        assert!(!matrix.thread_archive_actions);
        assert!(!matrix.security_settings);
        assert!(require_capability(&platform, Capability::SecuritySettings).is_err());
    }

    #[test]
    fn capability_gate_plan_matches_require_capability_without_host_specific_advice() {
        let linux = PlatformPaths::for_kind(crate::platform::PlatformKind::Linux);
        let macos = PlatformPaths::for_kind(crate::platform::PlatformKind::Macos);
        let windows = PlatformPaths::for_kind(crate::platform::PlatformKind::Windows);

        for platform in [&linux, &macos, &windows] {
            for capability in Capability::all() {
                let plan = capability_gate_plan(platform, *capability);
                assert_eq!(plan.capability, *capability);
                assert_eq!(plan.platform, platform.kind);
                assert_eq!(
                    plan.supported,
                    require_capability(platform, *capability).is_ok(),
                    "{capability:?} on {:?}",
                    platform.kind
                );
                if !plan.supported {
                    let message = plan
                        .error
                        .as_deref()
                        .expect("unsupported capability should include a neutral error");
                    assert!(message.contains(capability.as_str()));
                    assert!(!message.contains("systemctl"));
                    assert!(!message.contains("Nginx"));
                    assert!(!message.contains("sudo"));
                    assert!(!message.contains("/opt/nexushub"));
                }
            }
        }
    }
}
