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

#[tauri::command(rename = "system.platform")]
pub async fn getPlatformOverview(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::platform::PlatformPaths, String> {
    Ok(system_service::platform_overview(&state))
}
