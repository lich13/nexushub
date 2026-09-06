use crate::overview::DesktopState;
use anyhow::Result;
use nexushub_core::{
    local::{local_plugin_catalog, LocalPluginInfo},
    platform::PlatformPaths,
    system::{system_status_with_surface, SystemStatus},
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct DesktopSystemVersion {
    pub panel_current: String,
    pub panel_latest: Option<String>,
    pub panel_update_available: Option<bool>,
    pub codex_current: Option<String>,
    pub codex_latest: Option<String>,
    pub codex_update_available: Option<bool>,
}

pub(crate) async fn system_status_with_state(state: &DesktopState) -> Result<SystemStatus> {
    let config = state.config();
    system_status_with_surface(&config, state.platform(), state.host_surface()).await
}

pub(crate) fn system_version() -> DesktopSystemVersion {
    DesktopSystemVersion {
        panel_current: env!("CARGO_PKG_VERSION").to_string(),
        panel_latest: None,
        panel_update_available: None,
        codex_current: None,
        codex_latest: None,
        codex_update_available: None,
    }
}

pub(crate) fn providers() -> Vec<LocalPluginInfo> {
    local_plugin_catalog()
}

pub(crate) fn platform_overview(state: &DesktopState) -> PlatformPaths {
    state.platform().clone()
}
