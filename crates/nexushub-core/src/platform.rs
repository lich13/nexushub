use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlatformKind {
    Linux,
    Macos,
    Windows,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformPaths {
    pub kind: PlatformKind,
    pub data_dir: PathBuf,
    pub config_file: PathBuf,
    pub webui_dir: PathBuf,
    pub log_dir: PathBuf,
    pub service_name: String,
    pub service_kind: String,
}

impl PlatformPaths {
    pub fn current() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::for_kind(PlatformKind::Macos)
        }
        #[cfg(target_os = "windows")]
        {
            Self::for_kind(PlatformKind::Windows)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Self::for_kind(PlatformKind::Linux)
        }
    }

    pub fn for_kind(kind: PlatformKind) -> Self {
        match kind {
            PlatformKind::Linux => Self {
                kind,
                data_dir: PathBuf::from("/opt/nexushub"),
                config_file: PathBuf::from("/opt/nexushub/config.toml"),
                webui_dir: PathBuf::from("/opt/nexushub/webui"),
                log_dir: PathBuf::from("/opt/nexushub/logs"),
                service_name: "nexushub".to_string(),
                service_kind: "systemd".to_string(),
            },
            PlatformKind::Macos => Self {
                kind,
                data_dir: PathBuf::from("~/Library/Application Support/NexusHub"),
                config_file: PathBuf::from("~/Library/Application Support/NexusHub/config.toml"),
                webui_dir: PathBuf::from("~/Library/Application Support/NexusHub/webui"),
                log_dir: PathBuf::from("~/Library/Logs/NexusHub"),
                service_name: "local.nexushub".to_string(),
                service_kind: "launchd".to_string(),
            },
            PlatformKind::Windows => Self {
                kind,
                data_dir: PathBuf::from(r"%ProgramData%\NexusHub"),
                config_file: PathBuf::from(r"%ProgramData%\NexusHub\config.toml"),
                webui_dir: PathBuf::from(r"%ProgramData%\NexusHub\webui"),
                log_dir: PathBuf::from(r"%ProgramData%\NexusHub\logs"),
                service_name: "NexusHub".to_string(),
                service_kind: "windows_service".to_string(),
            },
        }
    }
}
