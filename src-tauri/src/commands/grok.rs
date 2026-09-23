#![allow(non_snake_case)]

use nexushub_core::{
    grok::{GrokDeletePreview, GrokDeleteRequest, GrokDeleteResult},
    platform::PlatformPaths,
    services::use_cases::NexusHubUseCases,
};

#[tauri::command(rename = "grok.list")]
pub fn listGrokSessions(
    q: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<nexushub_core::grok::GrokSessionSummary>, String> {
    NexusHubUseCases::new(&PlatformPaths::desktop_current())
        .grok()
        .list(limit.unwrap_or(100), q.as_deref())
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "grok.detail")]
pub fn getGrokSession(id: String) -> Result<nexushub_core::grok::GrokSessionDetail, String> {
    NexusHubUseCases::new(&PlatformPaths::desktop_current())
        .grok()
        .detail(&id)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "grok.rename")]
pub async fn renameGrokSession(
    id: String,
    title: String,
) -> Result<nexushub_core::grok::GrokSessionSummary, String> {
    NexusHubUseCases::new(&PlatformPaths::desktop_current())
        .grok()
        .rename(&id, &title)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "grok.deletePreview")]
pub fn previewGrokSessionDelete(id: String) -> Result<GrokDeletePreview, String> {
    NexusHubUseCases::new(&PlatformPaths::desktop_current())
        .grok()
        .delete_preview(&id)
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "grok.deleteExecute")]
pub fn deleteGrokSession(request: GrokDeleteRequest) -> Result<GrokDeleteResult, String> {
    NexusHubUseCases::new(&PlatformPaths::desktop_current())
        .grok()
        .delete_execute(request)
        .map_err(|err| err.to_string())
}
