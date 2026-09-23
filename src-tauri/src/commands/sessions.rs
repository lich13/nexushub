#![allow(non_snake_case)]
use crate::overview::DesktopState;
use nexushub_core::services::{
    sessions::{
        SessionBatchExecuteRequest, SessionBatchPreview, SessionBatchRequest, SessionBatchResult,
    },
    use_cases::NexusHubUseCases,
};

#[tauri::command(rename = "sessions.bulkPreview")]
pub async fn previewSessionBatch(
    state: tauri::State<'_, DesktopState>,
    request: SessionBatchRequest,
) -> Result<SessionBatchPreview, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        NexusHubUseCases::new(state.platform())
            .sessions(state.codex_paths())
            .bulk_preview(request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command(rename = "sessions.bulkExecute")]
pub async fn executeSessionBatch(
    state: tauri::State<'_, DesktopState>,
    request: SessionBatchExecuteRequest,
) -> Result<SessionBatchResult, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        NexusHubUseCases::new(state.platform())
            .sessions(state.codex_paths())
            .bulk_execute(request)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}
