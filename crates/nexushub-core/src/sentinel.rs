use crate::platform::{PlatformKind, PlatformPaths};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SentinelConfig {
    pub enabled: bool,
    pub bark_enabled: bool,
    pub hook_management_enabled: bool,
    pub logs_maintenance_enabled: bool,
    pub notify_reply_needed: bool,
    pub notify_recoverable: bool,
}

impl Default for SentinelConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            bark_enabled: false,
            hook_management_enabled: true,
            logs_maintenance_enabled: true,
            notify_reply_needed: true,
            notify_recoverable: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SentinelStatus {
    pub enabled: bool,
    pub platform: PlatformKind,
    pub service_kind: String,
    pub service_name: String,
    pub hook_status: String,
    pub bark_status: String,
    pub logs_db_status: String,
    pub recent_event_count: usize,
    pub reply_needed_count: usize,
    pub recoverable_count: usize,
    pub config_path: PathBuf,
}

pub fn sentinel_status(paths: &PlatformPaths, config: &SentinelConfig) -> SentinelStatus {
    SentinelStatus {
        enabled: config.enabled,
        platform: paths.kind,
        service_kind: paths.service_kind.clone(),
        service_name: paths.service_name.clone(),
        hook_status: if config.hook_management_enabled {
            "managed"
        } else {
            "disabled"
        }
        .to_string(),
        bark_status: if config.bark_enabled {
            "configured"
        } else {
            "not_configured"
        }
        .to_string(),
        logs_db_status: if config.logs_maintenance_enabled {
            "maintenance_ready"
        } else {
            "disabled"
        }
        .to_string(),
        recent_event_count: 0,
        reply_needed_count: 0,
        recoverable_count: 0,
        config_path: paths.config_file.clone(),
    }
}
