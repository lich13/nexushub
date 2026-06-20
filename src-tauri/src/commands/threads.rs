#![allow(non_snake_case)]

use crate::overview::{
    desktop_answer_elicitation_with_state, desktop_archive_thread_with_state,
    desktop_cancel_followup_with_state, desktop_continue_thread_with_state,
    desktop_enqueue_followup_with_state, desktop_fork_thread_with_state,
    desktop_list_followups_with_state, desktop_plan_accept_with_state,
    desktop_plan_revise_with_state, desktop_rename_thread_with_state,
    desktop_restore_thread_with_state, desktop_send_message_with_state,
    desktop_stop_thread_with_state, desktop_thread_blocks_with_state,
    desktop_thread_detail_with_state, desktop_threads_with_state, DesktopActionResponse,
    DesktopCancelFollowupRequest, DesktopElicitationAnswerRequest, DesktopFollowupRequest,
    DesktopPlanAcceptRequest, DesktopPlanReviseRequest, DesktopRenameThreadRequest,
    DesktopSendMessageRequest, DesktopState, DesktopStopRequest, DesktopThreadBlockPage,
    DesktopThreadIdRequest, ThreadBlocksRequest, ThreadDetailRequest, ThreadListRequest,
};

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

#[tauri::command]
pub fn listThreads(
    state: tauri::State<'_, DesktopState>,
    status: Option<String>,
    q: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<nexushub_core::codex::ThreadSummary>, String> {
    desktop_threads_with_state(
        &state,
        ThreadListRequest {
            status,
            query: q,
            limit,
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn getThread(
    state: tauri::State<'_, DesktopState>,
    id: String,
    options: Option<ThreadDetailOptions>,
) -> Result<Option<nexushub_core::codex::ThreadDetail>, String> {
    let options = options.unwrap_or_default();
    desktop_thread_detail_with_state(
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

#[tauri::command]
pub fn getThreadBlocks(
    state: tauri::State<'_, DesktopState>,
    id: String,
    options: Option<ThreadDetailOptions>,
) -> Result<Option<DesktopThreadBlockPage>, String> {
    let options = options.unwrap_or_default();
    desktop_thread_blocks_with_state(
        &state,
        ThreadBlocksRequest {
            id,
            limit: options.limit,
            before: options.before,
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn createThread(
    state: tauri::State<'_, DesktopState>,
    mut payload: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    payload.thread_id = None;
    desktop_send_message_with_state(&state, payload).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn sendMessage(
    state: tauri::State<'_, DesktopState>,
    threadId: Option<String>,
    mut payload: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    if payload.thread_id.is_none() {
        payload.thread_id = threadId;
    }
    desktop_send_message_with_state(&state, payload).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn steerThread(
    state: tauri::State<'_, DesktopState>,
    threadId: Option<String>,
    mut payload: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    if payload.thread_id.is_none() {
        payload.thread_id = threadId;
    }
    desktop_continue_thread_with_state(&state, payload).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn listFollowUps(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
) -> Result<Vec<nexushub_core::db::ThreadFollowUp>, String> {
    desktop_list_followups_with_state(
        &state,
        DesktopFollowupRequest {
            thread_id: threadId,
            limit: Some(20),
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn enqueueFollowUp(
    state: tauri::State<'_, DesktopState>,
    threadId: Option<String>,
    mut payload: DesktopSendMessageRequest,
) -> Result<nexushub_core::db::ThreadFollowUp, String> {
    if payload.thread_id.is_none() {
        payload.thread_id = threadId;
    }
    desktop_enqueue_followup_with_state(&state, payload).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn cancelFollowUp(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    followUpId: String,
) -> Result<DesktopActionResponse, String> {
    desktop_cancel_followup_with_state(
        &state,
        DesktopCancelFollowupRequest {
            thread_id: threadId,
            followup_id: followUpId,
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn stopThread(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    payload: Option<DesktopStopPayload>,
) -> Result<DesktopActionResponse, String> {
    let payload = payload.unwrap_or_default();
    desktop_stop_thread_with_state(
        &state,
        DesktopStopRequest {
            thread_id: threadId,
            turn_id: payload.turn_id,
            job_id: payload.job_id,
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn archiveThread(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
) -> Result<DesktopActionResponse, String> {
    desktop_archive_thread_with_state(&state, thread_id_request(threadId))
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn restoreThread(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
) -> Result<DesktopActionResponse, String> {
    desktop_restore_thread_with_state(&state, thread_id_request(threadId))
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn renameThread(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    name: String,
) -> Result<DesktopActionResponse, String> {
    desktop_rename_thread_with_state(&state, DesktopRenameThreadRequest { thread_id: threadId, name })
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn forkThread(threadId: String) -> DesktopActionResponse {
    desktop_fork_thread_with_state(thread_id_request(threadId))
}

#[tauri::command]
pub fn answerElicitation(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    answers: std::collections::HashMap<String, Vec<String>>,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    desktop_answer_elicitation_with_state(
        &state,
        DesktopElicitationAnswerRequest { thread_id: threadId, answers },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn acceptPlan(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    payload: PlanActionPayload,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    desktop_plan_accept_with_state(
        &state,
        DesktopPlanAcceptRequest {
            thread_id: threadId,
            turn_id: payload.turn_id,
            item_id: payload.item_id,
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn revisePlan(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    payload: PlanActionPayload,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    desktop_plan_revise_with_state(
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

#[tauri::command]
pub fn answerApproval(threadId: String) -> DesktopActionResponse {
    let mut response = crate::overview::unavailable_action(
        "answerApproval",
        "approval actions are unavailable in the local Codex read model",
    );
    response.thread_id = Some(threadId);
    response
}

#[tauri::command]
pub fn desktop_threads(
    state: tauri::State<'_, DesktopState>,
    request: ThreadListRequest,
) -> Result<Vec<nexushub_core::codex::ThreadSummary>, String> {
    desktop_threads_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_thread_detail(
    state: tauri::State<'_, DesktopState>,
    request: ThreadDetailRequest,
) -> Result<Option<nexushub_core::codex::ThreadDetail>, String> {
    desktop_thread_detail_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_thread_blocks(
    state: tauri::State<'_, DesktopState>,
    request: ThreadBlocksRequest,
) -> Result<Option<DesktopThreadBlockPage>, String> {
    desktop_thread_blocks_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_send_message(
    state: tauri::State<'_, DesktopState>,
    request: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    desktop_send_message_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_continue_thread(
    state: tauri::State<'_, DesktopState>,
    request: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    desktop_continue_thread_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_stop_thread(
    state: tauri::State<'_, DesktopState>,
    request: DesktopStopRequest,
) -> Result<DesktopActionResponse, String> {
    desktop_stop_thread_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_plan_accept(
    state: tauri::State<'_, DesktopState>,
    request: DesktopPlanAcceptRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    desktop_plan_accept_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_plan_revise(
    state: tauri::State<'_, DesktopState>,
    request: DesktopPlanReviseRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    desktop_plan_revise_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_answer_elicitation(
    state: tauri::State<'_, DesktopState>,
    request: DesktopElicitationAnswerRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    desktop_answer_elicitation_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_archive_thread(
    state: tauri::State<'_, DesktopState>,
    request: DesktopThreadIdRequest,
) -> Result<DesktopActionResponse, String> {
    desktop_archive_thread_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_restore_thread(
    state: tauri::State<'_, DesktopState>,
    request: DesktopThreadIdRequest,
) -> Result<DesktopActionResponse, String> {
    desktop_restore_thread_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_rename_thread(
    state: tauri::State<'_, DesktopState>,
    request: DesktopRenameThreadRequest,
) -> Result<DesktopActionResponse, String> {
    desktop_rename_thread_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_fork_thread(request: DesktopThreadIdRequest) -> DesktopActionResponse {
    desktop_fork_thread_with_state(request)
}

#[tauri::command]
pub fn desktop_list_followups(
    state: tauri::State<'_, DesktopState>,
    request: DesktopFollowupRequest,
) -> Result<Vec<nexushub_core::db::ThreadFollowUp>, String> {
    desktop_list_followups_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_enqueue_followup(
    state: tauri::State<'_, DesktopState>,
    request: DesktopSendMessageRequest,
) -> Result<nexushub_core::db::ThreadFollowUp, String> {
    desktop_enqueue_followup_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_cancel_followup(
    state: tauri::State<'_, DesktopState>,
    request: DesktopCancelFollowupRequest,
) -> Result<DesktopActionResponse, String> {
    desktop_cancel_followup_with_state(&state, request).map_err(|err| err.to_string())
}
