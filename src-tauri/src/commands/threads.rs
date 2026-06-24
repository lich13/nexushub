#![allow(non_snake_case)]

use crate::{
    overview::DesktopState,
    services::{
        actions::DesktopActionResponse,
        threads::{
            self as thread_service, DesktopCancelFollowupRequest, DesktopElicitationAnswerRequest,
            DesktopFollowupRequest, DesktopPlanAcceptRequest, DesktopPlanReviseRequest,
            DesktopRenameThreadRequest, DesktopSendMessageRequest, DesktopStopRequest,
            DesktopThreadIdRequest, ThreadBlocksRequest, ThreadDetailRequest, ThreadListRequest,
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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopStopPayload {
    #[serde(alias = "turn_id")]
    pub turn_id: Option<String>,
    #[serde(alias = "job_id")]
    pub job_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanActionPayload {
    #[serde(alias = "turn_id")]
    pub turn_id: Option<String>,
    #[serde(alias = "item_id")]
    pub item_id: Option<String>,
    pub instructions: Option<String>,
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

#[tauri::command(rename = "threads.create")]
pub fn createThread(
    state: tauri::State<'_, DesktopState>,
    payload: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    thread_service::send_message_with_state(&state, payload.without_thread_id())
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.send")]
pub fn sendMessage(
    state: tauri::State<'_, DesktopState>,
    threadId: Option<String>,
    payload: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    thread_service::send_message_with_state(&state, payload.with_thread_id_fallback(threadId))
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.steer")]
pub fn steerThread(
    state: tauri::State<'_, DesktopState>,
    threadId: Option<String>,
    payload: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    thread_service::steer_thread_with_state(&state, payload.with_thread_id_fallback(threadId))
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.followups.list")]
pub fn listFollowUps(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
) -> Result<Vec<nexushub_core::db::ThreadFollowUp>, String> {
    thread_service::list_followups_with_state(
        &state,
        DesktopFollowupRequest {
            thread_id: threadId,
            limit: Some(20),
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.followups.enqueue")]
pub fn enqueueFollowUp(
    state: tauri::State<'_, DesktopState>,
    threadId: Option<String>,
    payload: DesktopSendMessageRequest,
) -> Result<nexushub_core::db::ThreadFollowUp, String> {
    thread_service::enqueue_followup_with_state(&state, payload.with_thread_id_fallback(threadId))
        .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.followups.cancel")]
pub fn cancelFollowUp(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    followUpId: String,
) -> Result<DesktopActionResponse, String> {
    thread_service::cancel_followup_with_state(
        &state,
        DesktopCancelFollowupRequest {
            thread_id: threadId,
            followup_id: followUpId,
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.stop")]
pub fn stopThread(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    payload: Option<DesktopStopPayload>,
) -> Result<DesktopActionResponse, String> {
    let payload = payload.unwrap_or_default();
    thread_service::stop_thread_with_state(
        &state,
        DesktopStopRequest {
            thread_id: threadId,
            turn_id: payload.turn_id,
            job_id: payload.job_id,
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
pub fn renameThread(
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
    .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.fork")]
pub fn forkThread(threadId: String) -> DesktopActionResponse {
    thread_service::fork_thread_unavailable(thread_id_request(threadId))
}

#[tauri::command(rename = "threads.elicitation.answer")]
pub fn answerElicitation(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    answers: std::collections::HashMap<String, Vec<String>>,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    thread_service::answer_elicitation_with_state(
        &state,
        DesktopElicitationAnswerRequest {
            thread_id: threadId,
            answers,
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.plan.accept")]
pub fn acceptPlan(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    payload: PlanActionPayload,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    thread_service::accept_plan_with_state(
        &state,
        DesktopPlanAcceptRequest {
            thread_id: threadId,
            turn_id: payload.turn_id,
            item_id: payload.item_id,
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.plan.revise")]
pub fn revisePlan(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    payload: PlanActionPayload,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    thread_service::revise_plan_with_state(
        &state,
        DesktopPlanReviseRequest {
            thread_id: threadId,
            turn_id: payload.turn_id,
            item_id: payload.item_id,
            instructions: payload.instructions.unwrap_or_default(),
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.approval.answer")]
pub fn answerApproval(threadId: String) -> DesktopActionResponse {
    let mut response = thread_service::unavailable_action(
        "answerApproval",
        "approval actions are unavailable in the local Codex read model",
    );
    response.thread_id = Some(threadId);
    response
}
