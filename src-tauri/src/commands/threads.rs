#![allow(non_snake_case)]

use crate::{
    overview::DesktopState,
    services::{
        actions::DesktopActionResponse,
        threads::{
            self as thread_service, DesktopRenameThreadRequest, DesktopThreadIdRequest,
            ThreadBlocksRequest, ThreadDetailRequest, ThreadListRequest,
        },
    },
};

use anyhow::Result;
use nexushub_core::services::threads::ThreadBlocksPage;
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDetailOptions {
    pub limit: Option<usize>,
    pub before: Option<String>,
    pub full: Option<bool>,
}

fn thread_id_request(thread_id: String) -> DesktopThreadIdRequest {
    DesktopThreadIdRequest { thread_id }
}

#[tauri::command(rename = "threads.list")]
pub fn listThreads(
    state: tauri::State<'_, DesktopState>,
    status: Option<String>,
    q: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<nexushub_core::codex::ThreadSummary>, String> {
    thread_service::threads_with_state(
        &state,
        ThreadListRequest {
            status,
            query: q,
            limit,
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.detail")]
pub fn getThread(
    state: tauri::State<'_, DesktopState>,
    id: String,
    options: Option<ThreadDetailOptions>,
) -> Result<Option<nexushub_core::codex::ThreadDetail>, String> {
    let options = options.unwrap_or_default();
    thread_service::thread_detail_with_state(
        &state,
        ThreadDetailRequest {
            id,
            limit: options.limit,
            before: options.before,
            full: options.full,
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.blocks")]
pub fn getThreadBlocks(
    state: tauri::State<'_, DesktopState>,
    id: String,
    options: Option<ThreadDetailOptions>,
) -> Result<Option<ThreadBlocksPage>, String> {
    let options = options.unwrap_or_default();
    thread_service::thread_blocks_with_state(
        &state,
        ThreadBlocksRequest {
            id,
            limit: options.limit,
            before: options.before,
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.archive")]
pub fn archiveThread(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
) -> Result<DesktopActionResponse, String> {
    thread_service::archive_thread_with_state(&state, thread_id_request(threadId))
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.restore")]
pub fn restoreThread(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
) -> Result<DesktopActionResponse, String> {
    thread_service::restore_thread_with_state(&state, thread_id_request(threadId))
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.rename")]
pub async fn renameThread(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    name: String,
) -> Result<DesktopActionResponse, String> {
    thread_service::rename_thread_with_state(
        &state,
        DesktopRenameThreadRequest {
            thread_id: threadId,
            name,
        },
    )
    .await
    .map_err(|err| err.to_string())
}
