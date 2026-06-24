#![allow(non_snake_case)]

use crate::{overview::DesktopState, services::system as system_service};

#[tauri::command(rename = "system.status")]
pub async fn getSystemStatus(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::system::SystemStatus, String> {
    system_service::system_status_with_state(&state)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "system.version")]
pub fn getSystemVersion() -> Result<system_service::DesktopSystemVersion, String> {
    Ok(system_service::system_version())
}

#[tauri::command(rename = "system.providers")]
pub fn listProviders() -> Result<Vec<nexushub_core::local::LocalPluginInfo>, String> {
    Ok(system_service::providers())
}

#[tauri::command(rename = "system.claudeCodeOverview")]
pub fn getClaudeCodeOverview() -> Result<nexushub_core::claude_code::ClaudeOverview, String> {
    system_service::claude_code_overview().map_err(|err| err.to_string())
}

#[tauri::command(rename = "system.platform")]
pub async fn getPlatformOverview(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::platform::PlatformPaths, String> {
    Ok(system_service::platform_overview(&state))
}

#[tauri::command(rename = "system.plugins")]
pub fn listPlugins() -> Result<Vec<nexushub_core::local::LocalPluginInfo>, String> {
    Ok(system_service::plugins())
}

#[tauri::command(rename = "system.models")]
pub fn listModels() -> Result<Vec<nexushub_core::local::CodexModelInfo>, String> {
    Ok(system_service::models())
}

#[tauri::command(rename = "system.permissionProfiles")]
pub fn listPermissionProfiles() -> Result<Vec<nexushub_core::local::CodexPermissionProfile>, String>
{
    Ok(system_service::permission_profiles())
}

#[tauri::command(rename = "system.codexConfig")]
pub fn getCodexConfig(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::local::LocalCodexConfig, String> {
    Ok(system_service::codex_config(&state))
}
