#![allow(non_snake_case)]

use crate::overview::{
    DesktopActionResponse, DesktopCancelFollowupRequest, DesktopElicitationAnswerRequest,
    DesktopFollowupRequest, DesktopPlanAcceptRequest, DesktopPlanReviseRequest,
    DesktopRenameThreadRequest, DesktopSendMessageRequest, DesktopState, DesktopStopRequest,
    DesktopThreadBlockPage, DesktopThreadIdRequest, ThreadBlocksRequest, ThreadDetailRequest,
    ThreadListRequest,
};

use anyhow::Result;
use nexushub_core::{
    codex::{
        archived_thread_ids, hidden_thread_ids, list_threads, set_thread_archived,
        set_thread_title, thread_detail, window_thread_detail, ThreadDetail, ThreadSummary,
    },
    db::ThreadFollowUp,
    services::{
        jobs as job_service,
        threads::{self as thread_service, ThreadsQuery},
        uploads as upload_service,
    },
    uploads,
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

#[tauri::command(rename = "threads.list")]
pub fn listThreads(
    state: tauri::State<'_, DesktopState>,
    status: Option<String>,
    q: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<nexushub_core::codex::ThreadSummary>, String> {
    threads_with_state(
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
    thread_detail_with_state(
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
) -> Result<Option<DesktopThreadBlockPage>, String> {
    let options = options.unwrap_or_default();
    thread_blocks_with_state(
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
    mut payload: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    payload.thread_id = None;
    send_message_with_state(&state, payload).map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.send")]
pub fn sendMessage(
    state: tauri::State<'_, DesktopState>,
    threadId: Option<String>,
    mut payload: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    if payload.thread_id.is_none() {
        payload.thread_id = threadId;
    }
    send_message_with_state(&state, payload).map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.steer")]
pub fn steerThread(
    state: tauri::State<'_, DesktopState>,
    threadId: Option<String>,
    mut payload: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    if payload.thread_id.is_none() {
        payload.thread_id = threadId;
    }
    steer_thread_with_state(&state, payload).map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.followups.list")]
pub fn listFollowUps(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
) -> Result<Vec<nexushub_core::db::ThreadFollowUp>, String> {
    list_followups_with_state(
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
    mut payload: DesktopSendMessageRequest,
) -> Result<nexushub_core::db::ThreadFollowUp, String> {
    if payload.thread_id.is_none() {
        payload.thread_id = threadId;
    }
    enqueue_followup_with_state(&state, payload).map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.followups.cancel")]
pub fn cancelFollowUp(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    followUpId: String,
) -> Result<DesktopActionResponse, String> {
    cancel_followup_with_state(
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
    stop_thread_with_state(
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
    archive_thread_with_state(&state, thread_id_request(threadId)).map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.restore")]
pub fn restoreThread(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
) -> Result<DesktopActionResponse, String> {
    restore_thread_with_state(&state, thread_id_request(threadId)).map_err(|err| err.to_string())
}

#[tauri::command(rename = "threads.rename")]
pub fn renameThread(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    name: String,
) -> Result<DesktopActionResponse, String> {
    rename_thread_with_state(
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
    fork_thread_unavailable(thread_id_request(threadId))
}

#[tauri::command(rename = "threads.elicitation.answer")]
pub fn answerElicitation(
    state: tauri::State<'_, DesktopState>,
    threadId: String,
    answers: std::collections::HashMap<String, Vec<String>>,
) -> Result<nexushub_core::jobs::CodexActionResult, String> {
    answer_elicitation_with_state(
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
    accept_plan_with_state(
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
    revise_plan_with_state(
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
    let mut response = unavailable_action(
        "answerApproval",
        "approval actions are unavailable in the local Codex read model",
    );
    response.thread_id = Some(threadId);
    response
}

pub(crate) fn threads_for_home(state: &DesktopState) -> Result<Vec<ThreadSummary>> {
    thread_list_with_jobs(
        state,
        ThreadsQuery {
            status: None,
            q: None,
            limit: Some(40),
        },
    )
}

fn threads_with_state(
    state: &DesktopState,
    request: ThreadListRequest,
) -> Result<Vec<ThreadSummary>> {
    thread_list_with_jobs(
        state,
        ThreadsQuery {
            status: request.status,
            q: request.query,
            limit: request.limit,
        },
    )
}

fn thread_detail_with_state(
    state: &DesktopState,
    request: ThreadDetailRequest,
) -> Result<Option<ThreadDetail>> {
    let paths = state.codex_paths();
    let detail = thread_detail(&paths, &request.id)?;
    let Some(mut detail) = detail else {
        return Ok(None);
    };
    apply_running_job_to_detail(state, &mut detail)?;
    Ok(Some(window_thread_detail(
        detail,
        detail_block_limit(request.limit, request.full),
        request.before.as_deref(),
    )))
}

fn thread_blocks_with_state(
    state: &DesktopState,
    request: ThreadBlocksRequest,
) -> Result<Option<DesktopThreadBlockPage>> {
    let Some(detail) = thread_detail_with_state(
        state,
        ThreadDetailRequest {
            id: request.id.clone(),
            limit: Some(block_page_limit(request.limit)),
            full: Some(false),
            before: request.before,
        },
    )?
    else {
        return Ok(None);
    };
    Ok(Some(DesktopThreadBlockPage {
        thread_id: request.id,
        blocks: detail.blocks,
        total_blocks: detail.total_blocks,
        has_more_blocks: detail.has_more_blocks,
        before_cursor: detail.before_cursor,
    }))
}

fn send_message_with_state(
    state: &DesktopState,
    mut request: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    let Some(thread_id) = request
        .thread_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return start_codex_new_thread_job(state, request);
    };
    request.thread_id = Some(thread_id.to_string());
    start_codex_job_from_request(state, request, job_service::CodexActionKind::Resume)
}

fn steer_thread_with_state(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    let attachments = prepare_request_attachments(state, &request.attachments)?;
    let followup = job_service::enqueue_followup_with_capability(
        &state.db,
        state.platform(),
        job_service::ThreadSteerRequest {
        thread_id: request.thread_id.clone(),
        message: request.into_thread_message(attachments),
        },
    )?;
    Ok(job_service::codex_action_submitted(
        Some(followup.thread_id),
        None,
    ))
}

fn stop_thread_with_state(
    state: &DesktopState,
    request: DesktopStopRequest,
) -> Result<DesktopActionResponse> {
    let plan = job_service::plan_thread_stop_with_capability(
        state.platform(),
        job_service::ThreadStopRequest {
            thread_id: request.thread_id,
            turn_id: request.turn_id,
            job_id: request.job_id,
        },
    )?;
    let active_job_id = plan
        .requires_active_job_lookup
        .then(|| derive_active_job_id(state, &plan.thread_id))
        .flatten();
    let Ok(stop) = job_service::resolve_thread_stop_job(&plan, active_job_id) else {
        return Ok(unavailable_action(
            nexushub_core::services::commands::THREADS_STOP,
            "stop requires a running local fallback job; Codex app-server stop is not available in the native read model",
        ));
    };
    let cancelled = state.jobs.cancel_job(&stop.job_id)?;
    Ok(job_service::thread_stop_response(&stop, cancelled).into())
}

fn accept_plan_with_state(
    state: &DesktopState,
    request: DesktopPlanAcceptRequest,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    let _ = (request.turn_id, request.item_id);
    start_codex_resume_job(
        state,
        &request.thread_id,
        job_service::plan_accept_resume_message(),
    )
}

fn revise_plan_with_state(
    state: &DesktopState,
    request: DesktopPlanReviseRequest,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    let _ = (request.turn_id, request.item_id);
    let instructions = request.instructions.trim();
    if instructions.is_empty() {
        anyhow::bail!("revision instructions cannot be empty");
    }
    start_codex_resume_job(
        state,
        &request.thread_id,
        job_service::plan_revise_resume_message(instructions),
    )
}

fn answer_elicitation_with_state(
    state: &DesktopState,
    request: DesktopElicitationAnswerRequest,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    let message = job_service::elicitation_answer_resume_message(&request.answers);
    if message.trim().is_empty() {
        anyhow::bail!("answers cannot be empty");
    }
    start_codex_resume_job(state, &request.thread_id, message)
}

fn archive_thread_with_state(
    state: &DesktopState,
    request: DesktopThreadIdRequest,
) -> Result<DesktopActionResponse> {
    let plan = job_service::plan_thread_archive_with_capability(state.platform(), &request.thread_id)?;
    set_thread_archived(&state.codex_paths(), &plan.thread_id, true)?;
    Ok(job_service::thread_state_action_response(&plan)?.into())
}

fn restore_thread_with_state(
    state: &DesktopState,
    request: DesktopThreadIdRequest,
) -> Result<DesktopActionResponse> {
    let plan = job_service::plan_thread_restore_with_capability(state.platform(), &request.thread_id)?;
    set_thread_archived(&state.codex_paths(), &plan.thread_id, false)?;
    Ok(job_service::thread_state_action_response(&plan)?.into())
}

fn rename_thread_with_state(
    state: &DesktopState,
    request: DesktopRenameThreadRequest,
) -> Result<DesktopActionResponse> {
    let plan = job_service::plan_thread_rename_with_capability(
        state.platform(),
        job_service::ThreadRenameRequest {
            thread_id: request.thread_id,
            name: request.name,
        },
    )?;
    let name = plan.name.as_deref().unwrap_or_default();
    set_thread_title(&state.codex_paths(), &plan.thread_id, name)?;
    job_service::thread_state_action_response(&plan).map(Into::into)
}

fn fork_thread_unavailable(request: DesktopThreadIdRequest) -> DesktopActionResponse {
    let mut response = unavailable_action(
        "forkThread",
        "fork is unavailable in the local Codex read model",
    );
    response.thread_id = Some(request.thread_id);
    response
}

fn list_followups_with_state(
    state: &DesktopState,
    request: DesktopFollowupRequest,
) -> Result<Vec<ThreadFollowUp>> {
    job_service::list_followups_with_capability(
        &state.db,
        state.platform(),
        job_service::FollowUpListRequest {
            thread_id: request.thread_id,
            limit: request.limit,
        },
    )
}

fn enqueue_followup_with_state(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
) -> Result<ThreadFollowUp> {
    let attachments = prepare_request_attachments(state, &request.attachments)?;
    let Some(thread_id) = request
        .thread_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        anyhow::bail!("thread_id is required");
    };
    job_service::enqueue_followup_with_capability(
        &state.db,
        state.platform(),
        job_service::ThreadSteerRequest {
            thread_id: Some(thread_id.to_string()),
            message: request.into_thread_message(attachments),
        },
    )
}

fn cancel_followup_with_state(
    state: &DesktopState,
    request: DesktopCancelFollowupRequest,
) -> Result<DesktopActionResponse> {
    Ok(job_service::cancel_followup_with_capability(
        &state.db,
        state.platform(),
        job_service::FollowUpCancelRequest {
            thread_id: request.thread_id,
            followup_id: request.followup_id,
        },
    )?
    .into())
}

fn start_codex_new_thread_job(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    start_codex_job_from_request(state, request, job_service::CodexActionKind::Exec)
}

fn start_codex_resume_job(
    state: &DesktopState,
    thread_id: &str,
    message: String,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    start_codex_job_from_request(
        state,
        DesktopSendMessageRequest {
            thread_id: Some(thread_id.to_string()),
            message,
            ..DesktopSendMessageRequest::default()
        },
        job_service::CodexActionKind::Resume,
    )
}

fn start_codex_job_from_request(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
    kind: job_service::CodexActionKind,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    let spec = codex_job_spec_for_request(state, request, kind)?;
    let resolved = state.resolved_codex_paths();
    let job_id = state.jobs.start_codex_job(
        &spec.title,
        &resolved.home,
        &spec.cwd,
        spec.args,
        spec.prompt,
    )?;
    state
        .db
        .link_job_thread(&job_id, spec.thread_id.as_deref(), None)?;
    Ok(job_service::codex_action_submitted(
        spec.thread_id,
        Some(job_id),
    ))
}

pub(crate) fn codex_job_spec_for_request(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
    kind: job_service::CodexActionKind,
) -> Result<job_service::CodexJobSpec> {
    let attachments = prepare_request_attachments(state, &request.attachments)?;
    let action = request
        .into_thread_message(attachments)
        .into_job_action(kind);
    let config = state.config();
    job_service::build_codex_job_spec(&action, config.codex.workspace.clone())
}

fn prepare_request_attachments(
    state: &DesktopState,
    attachment_ids: &[String],
) -> Result<Vec<uploads::PreparedAttachment>> {
    upload_service::validate_attachment_id_count(attachment_ids)?;
    let root = uploads::upload_root(&state.resolved_codex_paths().home);
    uploads::prepare_uploads(&root, attachment_ids)
}

fn derive_active_job_id(state: &DesktopState, thread_id: &str) -> Option<String> {
    state
        .db
        .running_job_for_thread(thread_id)
        .ok()
        .flatten()
        .map(|job| job.id)
}

fn thread_list_with_jobs(state: &DesktopState, query: ThreadsQuery) -> Result<Vec<ThreadSummary>> {
    let paths = state.codex_paths();
    let fetch_limit = thread_service::thread_list_fetch_limit(query.status.as_deref(), query.limit);
    let hidden_thread_ids = hidden_thread_ids(&paths).unwrap_or_default();
    let archived_thread_ids = archived_thread_ids(&paths).unwrap_or_default();
    Ok(thread_service::build_threads_overview(
        list_threads(&paths, None, query.q.as_deref(), fetch_limit)?,
        state.db.running_thread_jobs()?,
        query,
        &hidden_thread_ids,
        &archived_thread_ids,
    )
    .threads)
}

fn apply_running_job_to_detail(state: &DesktopState, detail: &mut ThreadDetail) -> Result<()> {
    if let Some(job) = state.db.running_job_for_thread(&detail.summary.id)? {
        thread_service::apply_running_job_to_summary(&mut detail.summary, &job);
    }
    Ok(())
}

fn detail_block_limit(limit: Option<usize>, full: Option<bool>) -> Option<usize> {
    thread_service::normalize_thread_detail_block_limit(limit, full.unwrap_or(false))
}

fn block_page_limit(limit: Option<usize>) -> usize {
    thread_service::normalize_thread_block_limit(limit)
}

fn unavailable_action(command: &str, message: &str) -> DesktopActionResponse {
    job_service::action_unavailable(command, message).into()
}

impl DesktopSendMessageRequest {
    fn into_thread_message(
        self,
        prepared_attachments: Vec<uploads::PreparedAttachment>,
    ) -> job_service::ThreadMessageRequest {
        job_service::ThreadMessageRequest {
            thread_id: self.thread_id,
            message: self.message,
            attachments: self.attachments,
            prepared_attachments,
            model: self.model,
            service_tier: self.service_tier,
            reasoning_effort: self.reasoning_effort,
            cwd: self.cwd,
            permission_profile: self.permission_profile,
            approval_policy: self.approval_policy,
            sandbox_mode: self.sandbox_mode,
            network_access: self.network_access,
            collaboration_mode: self.collaboration_mode,
        }
    }
}
