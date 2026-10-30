#![allow(non_snake_case)]

use crate::{
    overview::DesktopState,
    services::{
        actions::DesktopActionResponse,
        settings::{
            self as settings_service, DesktopCleanupExecuteRequest, DesktopProbeEventsRequest,
            DesktopProbeEventsResponse, DesktopProbeSettings,
        },
    },
};
use anyhow::Result;
use nexushub_core::services::{probe as probe_service, settings::ProbeSettingsSaveRequest};

#[tauri::command(rename = "probe.settings.get")]
pub fn getProbeSettings(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopProbeSettings, String> {
    settings_service::probe_settings_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.settings.save")]
pub fn saveProbeSettings(
    state: tauri::State<'_, DesktopState>,
    settings: ProbeSettingsSaveRequest,
) -> Result<DesktopProbeSettings, String> {
    settings_service::probe_save_settings_with_state(&state, settings)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.barkTest")]
pub fn probeBarkTest(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopActionResponse, String> {
    settings_service::probe_action_with_state(&state, probe_service::ProbeAction::BarkTest)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.installHooks")]
pub fn probeInstallHooks(
    state: tauri::State<'_, DesktopState>,
) -> Result<DesktopActionResponse, String> {
    settings_service::probe_action_with_state(&state, probe_service::ProbeAction::InstallHooks)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "probe.events")]
pub fn getProbeEvents(
    state: tauri::State<'_, DesktopState>,
    limit: Option<u32>,
) -> Result<DesktopProbeEventsResponse, String> {
    settings_service::probe_events_with_state(&state, DesktopProbeEventsRequest { limit })
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "cleanup.archiveDryRun")]
pub fn dryRunArchiveDelete(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::archive::ArchiveDeletePlan, String> {
    settings_service::archive_delete_dry_run_with_state(&state).map_err(|err| err.to_string())
}

#[tauri::command(rename = "cleanup.archiveExecute")]
pub fn startArchiveDelete(
    state: tauri::State<'_, DesktopState>,
    request: DesktopCleanupExecuteRequest,
) -> Result<nexushub_core::archive::ArchiveDeleteResult, String> {
    settings_service::archive_delete_execute_with_state(&state, request)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "cleanup.hiddenDryRun")]
pub async fn dryRunHiddenThreadDelete(
    state: tauri::State<'_, DesktopState>,
    request: Option<nexushub_core::archive::hidden_scan::HiddenScanRequest>,
) -> Result<nexushub_core::archive::HiddenThreadDeletePlan, String> {
    nexushub_core::services::use_cases::NexusHubUseCases::new(state.platform())
        .cleanup()
        .scan_hidden(
            state.codex_paths(),
            "desktop".into(),
            request.unwrap_or_default(),
        )
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "cleanup.hiddenExecute")]
pub async fn startHiddenThreadDelete(
    state: tauri::State<'_, DesktopState>,
    request: DesktopCleanupExecuteRequest,
) -> Result<nexushub_core::archive::HiddenThreadDeleteResult, String> {
    let state = state.inner().clone();
    nexushub_core::read_tasks::spawn_blocking(move || {
        settings_service::hidden_delete_execute_with_state(&state, request)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|err| err.to_string())?
}

#[tauri::command(rename = "claude.hooks")]
pub async fn claudeHooks(
    state: tauri::State<'_, DesktopState>,
    request: nexushub_core::claude::hooks::ClaudeHooksRequest,
) -> Result<nexushub_core::claude::hooks::ClaudeHooksStatus, String> {
    let state = state.inner().clone();
    nexushub_core::read_tasks::spawn_blocking(move || {
        let config = state.config();
        nexushub_core::services::use_cases::NexusHubUseCases::with_config(&config, state.platform())
            .probe()?
            .claude_hooks(request)
    })
    .await
    .map_err(|_| "Claude 设置任务未完成".to_string())?
    .map_err(|error| error.to_string())
}

#[tauri::command(rename = "cleanup.dshArchiveDryRun")]
pub async fn dryRunDshArchiveDelete(
    state: tauri::State<'_, DesktopState>,
) -> Result<nexushub_core::dsh::DshArchiveDeletePlan, String> {
    let state = state.inner().clone();
    nexushub_core::read_tasks::spawn_blocking(move || {
        nexushub_core::services::use_cases::NexusHubUseCases::new(state.platform())
            .cleanup()
            .dsh_archive_dry_run(&nexushub_core::dsh::DshPaths::default_for_user())
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|_| "DSH 归档预览任务未完成".to_string())?
}

#[tauri::command(rename = "cleanup.dshArchiveExecute")]
pub async fn startDshArchiveDelete(
    state: tauri::State<'_, DesktopState>,
    request: nexushub_core::dsh::DshArchiveDeleteRequest,
) -> Result<nexushub_core::dsh::DshArchiveDeleteResult, String> {
    let state = state.inner().clone();
    nexushub_core::read_tasks::spawn_blocking(move || {
        nexushub_core::services::use_cases::NexusHubUseCases::new(state.platform())
            .cleanup()
            .dsh_archive_execute(&nexushub_core::dsh::DshPaths::default_for_user(), request)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|_| "DSH 归档清理任务未完成".to_string())?
}

#[tauri::command(rename = "cleanup.unifiedDryRun")]
pub async fn dryRunUnifiedCleanup(
    state: tauri::State<'_, DesktopState>,
    request: Option<nexushub_core::services::cleanup::UnifiedCleanupDryRunRequest>,
) -> Result<nexushub_core::services::cleanup::UnifiedCleanupPlan, String> {
    let state = state.inner().clone();
    nexushub_core::read_tasks::spawn_blocking(move || {
        let paths = nexushub_core::services::cleanup::UnifiedCleanupPaths::default_for_codex(
            state.codex_paths(),
        );
        nexushub_core::services::use_cases::NexusHubUseCases::new(state.platform())
            .cleanup()
            .unified_dry_run(&paths, request.unwrap_or_default())
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|_| "全平台清理预览任务未完成".to_string())?
}

#[tauri::command(rename = "cleanup.unifiedExecute")]
pub async fn startUnifiedCleanup(
    state: tauri::State<'_, DesktopState>,
    request: nexushub_core::services::cleanup::UnifiedCleanupExecuteRequest,
) -> Result<nexushub_core::services::cleanup::UnifiedCleanupResult, String> {
    let state = state.inner().clone();
    nexushub_core::read_tasks::spawn_blocking(move || {
        let paths = nexushub_core::services::cleanup::UnifiedCleanupPaths::default_for_codex(
            state.codex_paths(),
        );
        nexushub_core::services::use_cases::NexusHubUseCases::new(state.platform())
            .cleanup()
            .unified_execute(&paths, request)
            .map_err(|err| err.to_string())
    })
    .await
    .map_err(|_| "全平台清理任务未完成".to_string())?
}
