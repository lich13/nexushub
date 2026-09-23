#![allow(non_snake_case)]

use nexushub_core::{
    pi::{PiDeletePreview, PiDeleteRequest, PiDeleteResult},
    platform::PlatformPaths,
    services::use_cases::NexusHubUseCases,
};

#[tauri::command(rename = "pi.list")]
pub fn listPiSessions(
    q: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<nexushub_core::pi::PiSessionSummary>, String> {
    NexusHubUseCases::new(&PlatformPaths::desktop_current())
        .pi()
        .list(limit.unwrap_or(100), q.as_deref())
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "pi.detail")]
pub fn getPiSession(session_key: String) -> Result<nexushub_core::pi::PiSessionDetail, String> {
    NexusHubUseCases::new(&PlatformPaths::desktop_current())
        .pi()
        .detail(&session_key)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "pi.rename")]
pub async fn renamePiSession(
    session_key: String,
    title: String,
) -> Result<nexushub_core::pi::PiSessionSummary, String> {
    NexusHubUseCases::new(&PlatformPaths::desktop_current())
        .pi()
        .rename(&session_key, &title)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "pi.deletePreview")]
pub fn previewPiSessionDelete(session_key: String) -> Result<PiDeletePreview, String> {
    NexusHubUseCases::new(&PlatformPaths::desktop_current())
        .pi()
        .delete_preview(&session_key)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "pi.deleteExecute")]
pub fn deletePiSession(request: PiDeleteRequest) -> Result<PiDeleteResult, String> {
    NexusHubUseCases::new(&PlatformPaths::desktop_current())
        .pi()
        .delete_execute(request)
        .map_err(|err| err.to_string())
}
