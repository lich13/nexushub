#![allow(non_snake_case)]

use crate::{overview::DesktopState, services::desktop_webui as desktop_webui_service};
use nexushub_core::services::desktop_webui::{
    DesktopWebuiPasswordReset, DesktopWebuiSettingsPatch, DesktopWebuiSettingsView,
    DesktopWebuiStatus,
};

#[tauri::command(rename = "desktopWebUi.settings.get")]
pub fn getDesktopWebUiSettings(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopWebuiSettingsView, String> {
    desktop_webui_service::settings(&state).map_err(|err| err.to_string())
}

#[tauri::command(rename = "desktopWebUi.settings.save")]
pub fn saveDesktopWebUiSettings(
    state: tauri::State<'_, DesktopState>,
    settings: DesktopWebuiSettingsPatch,
) -> Result<DesktopWebuiSettingsView, String> {
    desktop_webui_service::save_settings(&state, settings).map_err(|err| err.to_string())
}

#[tauri::command(rename = "desktopWebUi.status")]
pub fn getDesktopWebUiStatus(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopWebuiStatus, String> {
    desktop_webui_service::status(&state).map_err(|err| err.to_string())
}

#[tauri::command(rename = "desktopWebUi.start")]
pub fn startDesktopWebUi(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopWebuiStatus, String> {
    desktop_webui_service::start(&state).map_err(|err| err.to_string())
}

#[tauri::command(rename = "desktopWebUi.stop")]
pub fn stopDesktopWebUi(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopWebuiStatus, String> {
    desktop_webui_service::stop(&state).map_err(|err| err.to_string())
}

#[tauri::command(rename = "desktopWebUi.password.reset")]
pub fn resetDesktopWebUiPassword(
    state: tauri::State<'_, DesktopState>,
    request: DesktopWebuiPasswordReset,
) -> Result<DesktopWebuiSettingsView, String> {
    desktop_webui_service::reset_password(&state, request).map_err(|err| err.to_string())
}
