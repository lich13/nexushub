use crate::overview::DesktopState;
use anyhow::Result;
use nexushub_core::{
    claude_code::{claude_overview, ClaudeOverview, ClaudePaths},
    local::{
        default_codex_models, default_permission_profiles, local_codex_config,
        local_plugin_catalog, CodexModelInfo, CodexPermissionProfile, LocalCodexConfig,
        LocalPluginInfo,
    },
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

pub(crate) fn claude_code_overview() -> Result<ClaudeOverview> {
    let paths = std::env::var_os("NEXUSHUB_CLAUDE_HOME")
        .map(ClaudePaths::new)
        .unwrap_or_else(ClaudePaths::default_for_user);
    claude_overview(&paths)
}

pub(crate) fn platform_overview(state: &DesktopState) -> PlatformPaths {
    state.platform().clone()
}

pub(crate) fn plugins() -> Vec<LocalPluginInfo> {
    local_plugin_catalog()
}

pub(crate) fn models() -> Vec<CodexModelInfo> {
    default_codex_models()
}

pub(crate) fn permission_profiles() -> Vec<CodexPermissionProfile> {
    default_permission_profiles()
}

pub(crate) fn codex_config(state: &DesktopState) -> LocalCodexConfig {
    let config = state.config();
    local_codex_config(&config, None)
}
