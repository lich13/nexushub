#![allow(non_snake_case)]

use crate::overview::{
    self, build_desktop_home, build_desktop_home_with_state, build_desktop_overview,
    desktop_platform_status_with_state, DesktopHome, DesktopOverview, DesktopState,
};
use serde::Serialize;

#[tauri::command]
pub fn desktop_overview() -> Result<DesktopOverview, String> {
    build_desktop_overview().map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn desktop_home() -> Result<DesktopHome, String> {
    build_desktop_home().await.map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn desktop_home_native(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopHome, String> {
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
pub async fn desktop_platform_status(
    state: tauri::State<'_, DesktopState>,
) -> Result<
    (
        nexushub_core::platform::PlatformPaths,
        Option<nexushub_core::system::SystemStatus>,
    ),
    String,
> {
    desktop_platform_status_with_state(&state)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_claude_code_overview() -> Result<nexushub_core::claude_code::ClaudeOverview, String>
{
    overview::desktop_claude_code_overview().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn listProviders() -> Result<Vec<nexushub_core::local::LocalPluginInfo>, String> {
    Ok(nexushub_core::local::local_plugin_catalog())
}

#[tauri::command]
pub fn getClaudeCodeOverview() -> Result<nexushub_core::claude_code::ClaudeOverview, String> {
    desktop_claude_code_overview()
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
