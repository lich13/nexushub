#![allow(non_snake_case)]

use crate::overview::DesktopState;
use anyhow::Result;
use nexushub_core::{
    codex::CodexPaths,
    config::Config,
    db::PanelDb,
    platform::PlatformPaths,
    probe::{ProbeRuntime, ProbeStatus},
};

pub async fn desktop_probe_status_current() -> Result<nexushub_core::probe::ProbeStatus> {
    let state = DesktopState::current()?;
    desktop_probe_status_with_state(&state).await
}

#[tauri::command]
pub async fn desktop_probe_status_command() -> std::result::Result<ProbeStatus, String> {
    desktop_probe_status_current()
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn desktop_probe_status(
    state: tauri::State<'_, DesktopState>,
) -> std::result::Result<ProbeStatus, String> {
    desktop_probe_status_with_state(&state)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn getProbeStatus(
    state: tauri::State<'_, DesktopState>,
) -> std::result::Result<ProbeStatus, String> {
    desktop_probe_status_with_state(&state)
        .await
        .map_err(|err| err.to_string())
}

pub async fn desktop_probe_status_with_state(state: &DesktopState) -> Result<ProbeStatus> {
    desktop_probe_status_from_parts(
        state.config(),
        &state.db,
        state.platform().clone(),
        state.codex_paths(),
    )
    .await
}

pub(crate) async fn desktop_probe_status_from_parts(
    config: Config,
    db: &PanelDb,
    platform: PlatformPaths,
    _codex_paths: CodexPaths,
) -> Result<ProbeStatus> {
    let limit = config.probe.recent_limit.clamp(1, 200);
    let mut status = ProbeRuntime::new(config, platform).status().await?;
    if let Ok(events) = db.list_probe_events(limit as u32) {
        status.recent_event_count = events.len();
    }
    Ok(status)
}
