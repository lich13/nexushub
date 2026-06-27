use crate::{
    config::Config,
    platform::{PlatformKind, PlatformPaths},
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HostSurface {
    LinuxServerWebui,
    DesktopEmbeddedTauri,
    DesktopLanWebui,
}

impl HostSurface {
    pub const ALL: &'static [HostSurface] = &[
        HostSurface::LinuxServerWebui,
        HostSurface::DesktopEmbeddedTauri,
        HostSurface::DesktopLanWebui,
    ];

    pub fn default_for_platform(platform: &PlatformPaths) -> Self {
        match platform.kind {
            PlatformKind::Linux => Self::LinuxServerWebui,
            PlatformKind::Macos | PlatformKind::Windows => Self::DesktopEmbeddedTauri,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::LinuxServerWebui => "linux_server_webui",
            Self::DesktopEmbeddedTauri => "desktop_embedded_tauri",
            Self::DesktopLanWebui => "desktop_lan_webui",
        }
    }
}

impl fmt::Display for HostSurface {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for HostSurface {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value.trim().replace('-', "_").as_str() {
            "linux_server_webui" => Ok(Self::LinuxServerWebui),
            "desktop_embedded_tauri" => Ok(Self::DesktopEmbeddedTauri),
            "desktop_lan_webui" => Ok(Self::DesktopLanWebui),
            other => Err(format!("unsupported host surface: {other}")),
        }
    }
}

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
    DesktopWebuiControl,
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
        Capability::DesktopWebuiControl,
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
            Self::DesktopWebuiControl => "desktop_webui_control",
        }
    }

    pub fn is_supported_on(self, platform: &PlatformPaths) -> bool {
        self.is_supported_on_surface(platform, HostSurface::default_for_platform(platform))
    }

    pub fn is_supported_on_surface(self, platform: &PlatformPaths, surface: HostSurface) -> bool {
        let shared_core = matches!(platform.kind, PlatformKind::Linux | PlatformKind::Macos);
        let linux_server_webui = surface == HostSurface::LinuxServerWebui
            && matches!(platform.kind, PlatformKind::Linux);
        let desktop_embedded = surface == HostSurface::DesktopEmbeddedTauri && shared_core;
        let desktop_lan_webui = surface == HostSurface::DesktopLanWebui && shared_core;
        match self {
            Self::Threads
            | Self::Jobs
            | Self::Probe
            | Self::Status
            | Self::Settings
            | Self::JobHistory
            | Self::ThreadCleanup
            | Self::ProbeLogMaintenance
            | Self::ThreadArchiveActions => {
                linux_server_webui || desktop_embedded || desktop_lan_webui
            }
            Self::AppUpdater => linux_server_webui || desktop_embedded,
            Self::WebAuth | Self::Csrf => linux_server_webui || desktop_lan_webui,
            Self::SecuritySettings
            | Self::Turnstile
            | Self::Systemd
            | Self::Nginx
            | Self::PublicEndpoint
            | Self::AdminPassword
            | Self::LinuxUpdateJob
            | Self::PruneBackups => linux_server_webui,
            Self::DesktopWebuiControl => desktop_embedded,
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
    pub desktop_webui_control: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityGatePlan {
    pub capability: Capability,
    pub platform: PlatformKind,
    pub host_surface: HostSurface,
    pub supported: bool,
    pub error: Option<String>,
}

pub fn capability_gate_plan(
    platform: &PlatformPaths,
    capability: Capability,
) -> CapabilityGatePlan {
    capability_gate_plan_for_surface(
        platform,
        HostSurface::default_for_platform(platform),
        capability,
    )
}

pub fn capability_gate_plan_for_surface(
    platform: &PlatformPaths,
    host_surface: HostSurface,
    capability: Capability,
) -> CapabilityGatePlan {
    let supported = capability.is_supported_on_surface(platform, host_surface);
    CapabilityGatePlan {
        capability,
        platform: platform.kind,
        host_surface,
        supported,
        error: (!supported).then(|| {
            format!(
                "{} is unavailable on {} {}",
                capability.as_str(),
                platform_kind_label(platform.kind),
                host_surface.as_str()
            )
        }),
    }
}

pub fn require_capability(platform: &PlatformPaths, capability: Capability) -> Result<()> {
    require_capability_for_surface(
        platform,
        HostSurface::default_for_platform(platform),
        capability,
    )
}

pub fn require_capability_for_surface(
    platform: &PlatformPaths,
    host_surface: HostSurface,
    capability: Capability,
) -> Result<()> {
    if capability.is_supported_on_surface(platform, host_surface) {
        return Ok(());
    }
    bail!(
        "{} is unavailable on {} {}",
        capability.as_str(),
        platform_kind_label(platform.kind),
        host_surface.as_str()
    )
}

pub fn system_capabilities(config: &Config, platform: &PlatformPaths) -> SystemCapabilities {
    system_capabilities_for_surface(
        config,
        platform,
        HostSurface::default_for_platform(platform),
    )
}

pub fn system_capabilities_for_surface(
    _config: &Config,
    platform: &PlatformPaths,
    host_surface: HostSurface,
) -> SystemCapabilities {
    SystemCapabilities {
        threads: Capability::Threads.is_supported_on_surface(platform, host_surface),
        jobs: Capability::Jobs.is_supported_on_surface(platform, host_surface),
        probe: Capability::Probe.is_supported_on_surface(platform, host_surface),
        status: Capability::Status.is_supported_on_surface(platform, host_surface),
        settings: Capability::Settings.is_supported_on_surface(platform, host_surface),
        job_history: Capability::JobHistory.is_supported_on_surface(platform, host_surface),
        app_updater: Capability::AppUpdater.is_supported_on_surface(platform, host_surface),
        thread_cleanup: Capability::ThreadCleanup.is_supported_on_surface(platform, host_surface),
        probe_log_maintenance: Capability::ProbeLogMaintenance
            .is_supported_on_surface(platform, host_surface),
        thread_archive_actions: Capability::ThreadArchiveActions
            .is_supported_on_surface(platform, host_surface),
        web_auth: Capability::WebAuth.is_supported_on_surface(platform, host_surface),
        csrf: Capability::Csrf.is_supported_on_surface(platform, host_surface),
        security_settings: Capability::SecuritySettings
            .is_supported_on_surface(platform, host_surface),
        turnstile: Capability::Turnstile.is_supported_on_surface(platform, host_surface),
        systemd: Capability::Systemd.is_supported_on_surface(platform, host_surface),
        nginx: Capability::Nginx.is_supported_on_surface(platform, host_surface),
        public_endpoint: Capability::PublicEndpoint.is_supported_on_surface(platform, host_surface),
        admin_password: Capability::AdminPassword.is_supported_on_surface(platform, host_surface),
        linux_update_job: Capability::LinuxUpdateJob
            .is_supported_on_surface(platform, host_surface),
        prune_backups: Capability::PruneBackups.is_supported_on_surface(platform, host_surface),
        desktop_webui_control: Capability::DesktopWebuiControl
            .is_supported_on_surface(platform, host_surface),
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
    use super::{
        capability_gate_plan, capability_gate_plan_for_surface, require_capability,
        require_capability_for_surface, system_capabilities, system_capabilities_for_surface,
        Capability, HostSurface,
    };
    use crate::{config::Config, platform::PlatformPaths};

    #[test]
    fn linux_server_webui_keeps_existing_linux_web_capabilities() {
        let config = Config::for_platform_kind(crate::platform::PlatformKind::Linux);
        let linux = PlatformPaths::for_kind(crate::platform::PlatformKind::Linux);
        let matrix =
            system_capabilities_for_surface(&config, &linux, HostSurface::LinuxServerWebui);

        assert!(matrix.web_auth);
        assert!(matrix.csrf);
        assert!(matrix.security_settings);
        assert!(matrix.turnstile);
        assert!(matrix.systemd);
        assert!(matrix.nginx);
        assert!(matrix.public_endpoint);
        assert!(matrix.admin_password);
        assert!(matrix.linux_update_job);
        assert!(matrix.prune_backups);
        assert!(!matrix.desktop_webui_control);
    }

    #[test]
    fn desktop_embedded_tauri_hides_web_host_surfaces_but_can_control_lan_webui() {
        for kind in [
            crate::platform::PlatformKind::Linux,
            crate::platform::PlatformKind::Macos,
        ] {
            let config = Config::for_platform_kind(kind);
            let platform = PlatformPaths::for_kind(kind);
            let matrix = system_capabilities_for_surface(
                &config,
                &platform,
                HostSurface::DesktopEmbeddedTauri,
            );

            assert!(matrix.threads);
            assert!(matrix.probe);
            assert!(matrix.app_updater);
            assert!(matrix.thread_cleanup);
            assert!(matrix.desktop_webui_control);
            assert!(!matrix.web_auth);
            assert!(!matrix.security_settings);
            assert!(!matrix.turnstile);
            assert!(!matrix.systemd);
            assert!(!matrix.nginx);
            assert!(!matrix.public_endpoint);
            assert!(!matrix.admin_password);
            assert!(!matrix.linux_update_job);
            assert!(!matrix.prune_backups);
        }
    }

    #[test]
    fn desktop_lan_webui_uses_auth_without_linux_host_admin_surfaces() {
        let config = Config::for_platform_kind(crate::platform::PlatformKind::Macos);
        let macos = PlatformPaths::for_kind(crate::platform::PlatformKind::Macos);
        let matrix = system_capabilities_for_surface(&config, &macos, HostSurface::DesktopLanWebui);

        assert!(matrix.threads);
        assert!(matrix.jobs);
        assert!(matrix.probe);
        assert!(matrix.web_auth);
        assert!(matrix.csrf);
        assert!(matrix.thread_cleanup);
        assert!(!matrix.app_updater);
        assert!(!matrix.desktop_webui_control);
        assert!(!matrix.security_settings);
        assert!(!matrix.turnstile);
        assert!(!matrix.systemd);
        assert!(!matrix.nginx);
        assert!(!matrix.public_endpoint);
        assert!(!matrix.admin_password);
        assert!(!matrix.linux_update_job);
        assert!(!matrix.prune_backups);
    }

    #[test]
    fn linux_only_capabilities_are_allowed_only_on_linux_server_webui() {
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

        let tauri_linux_error = require_capability_for_surface(
            &linux,
            HostSurface::DesktopEmbeddedTauri,
            Capability::LinuxUpdateJob,
        )
        .expect_err("Linux Tauri must not allow server update jobs");
        assert!(tauri_linux_error
            .to_string()
            .contains("desktop_embedded_tauri"));
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

        let plan = capability_gate_plan_for_surface(
            &linux,
            HostSurface::DesktopEmbeddedTauri,
            Capability::LinuxUpdateJob,
        );
        assert!(!plan.supported);
        assert_eq!(plan.host_surface, HostSurface::DesktopEmbeddedTauri);
    }
}
