#![allow(non_snake_case)]

use crate::overview::{
    build_desktop_home_with_state, build_desktop_overview, DesktopHome, DesktopOverview,
    DesktopState,
};
use serde::Serialize;

#[tauri::command]
pub fn getDesktopOverview() -> Result<DesktopOverview, String> {
    build_desktop_overview().map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn getDesktopHome(state: tauri::State<'_, DesktopState>) -> Result<DesktopHome, String> {
    build_desktop_home_with_state(&state)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn getSystemStatus(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::system::SystemStatus, String> {
    let config = state.config();
    nexushub_core::system::system_status_with_paths(&config, state.platform())
        .await
        .map_err(|err| err.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct DesktopSystemVersion {
    pub panel_current: String,
    pub panel_latest: Option<String>,
    pub panel_update_available: Option<bool>,
    pub codex_current: Option<String>,
    pub codex_latest: Option<String>,
    pub codex_update_available: Option<bool>,
}

#[tauri::command]
pub fn getSystemVersion() -> Result<DesktopSystemVersion, String> {
    Ok(DesktopSystemVersion {
        panel_current: env!("CARGO_PKG_VERSION").to_string(),
        panel_latest: None,
        panel_update_available: Some(false),
        codex_current: None,
        codex_latest: None,
        codex_update_available: None,
    })
}

#[tauri::command]
pub async fn getDesktopPlatformStatus(
    state: tauri::State<'_, DesktopState>,
) -> Result<
    (
        nexushub_core::platform::PlatformPaths,
        Option<nexushub_core::system::SystemStatus>,
    ),
    String,
> {
    platform_status_with_state(&state)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn getDesktopClaudeCodeOverview() -> Result<nexushub_core::claude_code::ClaudeOverview, String>
{
    claude_code_overview().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn listProviders() -> Result<Vec<nexushub_core::local::LocalPluginInfo>, String> {
    Ok(nexushub_core::local::local_plugin_catalog())
}

#[tauri::command]
pub fn getClaudeCodeOverview() -> Result<nexushub_core::claude_code::ClaudeOverview, String> {
    claude_code_overview().map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn getPlatformOverview(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::platform::PlatformPaths, String> {
    Ok(state.platform().clone())
}

#[tauri::command]
pub fn listPlugins() -> Result<Vec<nexushub_core::local::LocalPluginInfo>, String> {
    Ok(nexushub_core::local::local_plugin_catalog())
}

#[tauri::command]
pub fn listModels() -> Result<Vec<nexushub_core::local::CodexModelInfo>, String> {
    Ok(nexushub_core::local::default_codex_models())
}

#[tauri::command]
pub fn listPermissionProfiles() -> Result<Vec<nexushub_core::local::CodexPermissionProfile>, String>
{
    Ok(nexushub_core::local::default_permission_profiles())
}

#[tauri::command]
pub fn getCodexConfig(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::local::LocalCodexConfig, String> {
    let config = state.config();
    Ok(nexushub_core::local::local_codex_config(&config, None))
}

async fn platform_status_with_state(
    state: &DesktopState,
) -> anyhow::Result<(
    nexushub_core::platform::PlatformPaths,
    Option<nexushub_core::system::SystemStatus>,
)> {
    let config = state.config();
    let system = nexushub_core::system::system_status_with_paths(&config, state.platform())
        .await
        .ok();
    Ok((state.platform().clone(), system))
}

fn claude_code_overview() -> anyhow::Result<nexushub_core::claude_code::ClaudeOverview> {
    let paths = std::env::var_os("NEXUSHUB_CLAUDE_HOME")
        .map(nexushub_core::claude_code::ClaudePaths::new)
        .unwrap_or_else(nexushub_core::claude_code::ClaudePaths::default_for_user);
    nexushub_core::claude_code::claude_overview(&paths)
}
