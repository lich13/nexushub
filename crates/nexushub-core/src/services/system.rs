use crate::{
    config::Config,
    platform::{PlatformKind, PlatformPaths},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemCapabilities {
    pub threads: bool,
    pub jobs: bool,
    pub probe: bool,
    pub status: bool,
    pub settings: bool,
    pub job_history: bool,
    pub app_updater: bool,
    pub web_auth: bool,
    pub security_settings: bool,
    pub turnstile: bool,
    pub systemd: bool,
    pub nginx: bool,
    pub public_endpoint: bool,
    pub admin_password: bool,
    pub linux_update_job: bool,
    pub prune_backups: bool,
}

pub fn system_capabilities(_config: &Config, platform: &PlatformPaths) -> SystemCapabilities {
    let shared_core = matches!(platform.kind, PlatformKind::Linux | PlatformKind::Macos);
    let linux_web_host = matches!(platform.kind, PlatformKind::Linux);
    SystemCapabilities {
        threads: shared_core,
        jobs: shared_core,
        probe: shared_core,
        status: shared_core,
        settings: shared_core,
        job_history: shared_core,
        app_updater: shared_core,
        web_auth: linux_web_host,
        security_settings: linux_web_host,
        turnstile: linux_web_host,
        systemd: linux_web_host,
        nginx: linux_web_host,
        public_endpoint: linux_web_host,
        admin_password: linux_web_host,
        linux_update_job: linux_web_host,
        prune_backups: linux_web_host,
    }
}
