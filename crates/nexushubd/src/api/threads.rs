use super::{api_error, ok, system::http_update_platform, ApiError, ApiResponse};
use crate::{
    auth::{require_auth, require_csrf},
    linux_adapter,
    state::AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    Json,
};
use nexushub_core::{
    codex::{self, MessageBlock},
    services::{
        jobs as job_service,
        threads::{self as thread_service, ThreadsQuery},
        uploads as upload_service,
        use_cases::NexusHubUseCases,
    },
    uploads::PreparedAttachment,
};
use serde::Deserialize;
use serde_json::json;
use std::{collections::HashMap, time::Duration};

const THREAD_EVENT_BLOCK_WINDOW: usize = 160;

#[derive(Debug, Deserialize)]
pub(crate) struct ThreadDetailQuery {
    limit: Option<usize>,
    before: Option<String>,
    full: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ThreadBlocksQuery {
    limit: Option<usize>,
    before: Option<String>,
}

pub(crate) async fn list_threads(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ThreadsQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(linux_adapter::list_threads_read_model(&state, query)?)
}

pub(crate) async fn thread_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<ThreadDetailQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .threads()
        .detail_read(thread_service::ThreadDetailRequest {
            id: id.clone(),
            limit: query.limit,
            full: query.full,
            before: query.before.clone(),
        })
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    match linux_adapter::window_thread_detail_read_model(&state, &plan.detail)
        .map_err(api_error_for_thread_detail_load)?
    {
        Some(detail) => ok(detail),
        None => Err(api_error(StatusCode::NOT_FOUND, "thread not found")),
    }
}

fn api_error_for_thread_detail_load(err: anyhow::Error) -> ApiError {
    let message = err.to_string();
    api_error(StatusCode::INTERNAL_SERVER_ERROR, &message)
}

pub(crate) async fn thread_blocks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<ThreadBlocksQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .threads()
        .blocks(&id, query.limit, query.before.clone())
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    match linux_adapter::thread_blocks_read_model(&state, &plan)
        .map_err(api_error_for_thread_detail_load)?
    {
        Some(page) => ok(page),
        None => Err(api_error(StatusCode::NOT_FOUND, "thread not found")),
    }
}

pub(crate) async fn create_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut payload): Json<job_service::ThreadMessageRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    payload.prepared_attachments = prepare_request_attachments(&state, &payload.attachments)?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .threads()
        .create_job(payload, state.config().codex.workspace.clone())
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::start_thread_command_execution_plan(
        &state, &auth, plan,
    )?)
}

fn prepare_request_attachments(
    state: &AppState,
    attachment_ids: &[String],
) -> Result<Vec<PreparedAttachment>, ApiError> {
    linux_adapter::prepare_request_attachments(state, attachment_ids).map_err(|err| {
        let status = if err
            .to_string()
            .contains(upload_service::ATTACHMENT_ID_LIMIT_MESSAGE)
        {
            StatusCode::PAYLOAD_TOO_LARGE
        } else {
            StatusCode::BAD_REQUEST
        };
        api_error(status, &err.to_string())
    })
}

pub(crate) async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(mut payload): Json<job_service::ThreadMessageRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    payload.prepared_attachments = prepare_request_attachments(&state, &payload.attachments)?;
    payload.thread_id = Some(id.clone());
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .threads()
        .send_job(
            job_service::ThreadSendRequest {
                thread_id: Some(id.clone()),
                message: payload,
            },
            state.config().codex.workspace.clone(),
        )
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::start_thread_command_execution_plan(
        &state, &auth, plan,
    )?)
}

pub(crate) async fn steer_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(mut payload): Json<job_service::ThreadMessageRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    payload.prepared_attachments = prepare_request_attachments(&state, &payload.attachments)?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .threads()
        .enqueue_followup(job_service::ThreadSteerRequest {
            thread_id: Some(id.clone()),
            message: payload,
        })
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    let followup = linux_adapter::enqueue_followup_plan(
        &state,
        &auth,
        plan,
        "thread.followup.enqueued_after_steer_fallback",
    )
    .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::codex_followup_queued_response(
        followup.thread_id,
    ))
}

pub(crate) async fn list_followups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let platform = http_update_platform();
    let plan =
        NexusHubUseCases::new(&platform)
            .threads()
            .followups(job_service::FollowUpListRequest {
                thread_id: id,
                limit: Some(20),
            })?;
    let items = linux_adapter::list_followups_plan(&state, plan)?;
    ok(json!({ "items": items }))
}

pub(crate) async fn enqueue_followup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(mut payload): Json<job_service::ThreadMessageRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    payload.prepared_attachments = prepare_request_attachments(&state, &payload.attachments)?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .threads()
        .enqueue_followup(job_service::ThreadSteerRequest {
            thread_id: Some(id.clone()),
            message: payload,
        })
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(
        linux_adapter::enqueue_followup_plan(&state, &auth, plan, "thread.followup.enqueued")
            .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?,
    )
}

pub(crate) async fn cancel_followup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, followup_id)): Path<(String, String)>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .threads()
        .cancel_followup(job_service::FollowUpCancelRequest {
            thread_id: id.clone(),
            followup_id: followup_id.clone(),
        })
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    let response = linux_adapter::cancel_followup_plan(&state, plan)?;
    ok(response)
}

#[derive(Debug, Deserialize)]
pub(crate) struct StopThreadRequest {
    turn_id: Option<String>,
    job_id: Option<String>,
}

pub(crate) async fn stop_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Option<Json<StopThreadRequest>>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let payload = payload.map(|Json(value)| value);
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .threads()
        .stop(job_service::ThreadStopRequest {
            thread_id: id,
            turn_id: payload.as_ref().and_then(|value| value.turn_id.clone()),
            job_id: payload.as_ref().and_then(|value| value.job_id.clone()),
        })
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    let stop = match linux_adapter::resolve_thread_stop_plan(&state, &plan) {
        Ok(stop) => stop,
        Err(err) => {
            linux_adapter::record_thread_audit(
                &state,
                &auth,
                "thread.stop.requested",
                &plan.thread_id,
                json!({"turn_id": plan.turn_id}),
            )?;
            return Err(api_error(StatusCode::BAD_REQUEST, &err.to_string()));
        }
    };
    ok(linux_adapter::cancel_thread_stop_plan(
        &state, &auth, &stop,
    )?)
}

pub(crate) async fn archive_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .threads()
        .archive(&id)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::apply_thread_state_action_plan(
        &state, &auth, &plan,
    )?)
}

pub(crate) async fn restore_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .threads()
        .restore(&id)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::apply_thread_state_action_plan(
        &state, &auth, &plan,
    )?)
}

#[derive(Debug, Deserialize)]
pub(crate) struct RenameThreadRequest {
    name: String,
}

pub(crate) async fn rename_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<RenameThreadRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .threads()
        .rename(job_service::ThreadRenameRequest {
            thread_id: id,
            name: payload.name,
        })
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::apply_thread_state_action_plan(
        &state, &auth, &plan,
    )?)
}

pub(crate) async fn fork_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    linux_adapter::record_thread_audit(
        &state,
        &auth,
        "thread.fork.unsupported",
        &id,
        json!({"available": false}),
    )?;
    Err(api_error(
        StatusCode::NOT_IMPLEMENTED,
        "fork is unavailable in the local Codex read model",
    ))
}

#[derive(Debug, Deserialize)]
pub(crate) struct PlanAcceptRequest {
    turn_id: Option<String>,
    item_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PlanReviseRequest {
    turn_id: Option<String>,
    item_id: Option<String>,
    instructions: String,
}

pub(crate) async fn plan_accept(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<PlanAcceptRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let result = start_codex_resume_job(
        &state,
        &auth,
        &id,
        job_service::plan_accept_resume_message(),
    )?;
    linux_adapter::record_thread_audit(
        &state,
        &auth,
        "thread.plan.accept",
        &id,
        json!({"turn_id": payload.turn_id, "item_id": payload.item_id, "job_fallback": true}),
    )?;
    ok(result)
}

pub(crate) async fn plan_revise(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<PlanReviseRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let instructions = payload.instructions.trim();
    if instructions.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "revision instructions cannot be empty",
        ));
    }
    let result = start_codex_resume_job(
        &state,
        &auth,
        &id,
        job_service::plan_revise_resume_message(instructions),
    )?;
    linux_adapter::record_thread_audit(
        &state,
        &auth,
        "thread.plan.revise",
        &id,
        json!({"turn_id": payload.turn_id, "item_id": payload.item_id, "job_fallback": true}),
    )?;
    ok(result)
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApprovalAnswerRequest {
    turn_id: Option<String>,
    item_id: Option<String>,
    request_id: Option<String>,
    decision: String,
}

pub(crate) async fn answer_approval(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<ApprovalAnswerRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    linux_adapter::record_thread_audit(
        &state,
        &auth,
        "thread.approval.unsupported",
        &id,
        json!({
            "turn_id": payload.turn_id,
            "item_id": payload.item_id,
            "request_id": payload.request_id,
            "decision": payload.decision
        }),
    )?;
    Err(api_error(
        StatusCode::NOT_IMPLEMENTED,
        "approval response is unavailable in the local Codex read model",
    ))
}

#[derive(Debug, Deserialize)]
pub(crate) struct ElicitationAnswerRequest {
    answers: HashMap<String, Vec<String>>,
}

pub(crate) async fn answer_elicitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<ElicitationAnswerRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let message = job_service::elicitation_answer_resume_message(&payload.answers);
    if message.trim().is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "answers cannot be empty",
        ));
    }
    let result = start_codex_resume_job(&state, &auth, &id, message)?;
    ok(result)
}

fn start_codex_resume_job(
    state: &AppState,
    auth: &crate::auth::AuthContext,
    thread_id: &str,
    message: String,
) -> Result<nexushub_core::jobs::CodexActionResult, ApiError> {
    linux_adapter::start_codex_resume_action(state, auth, thread_id, message)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))
}

pub(crate) async fn thread_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let event_state = state.clone();
    let stream = async_stream::stream! {
        let mut sent_blocks: HashMap<String, String> = HashMap::new();
        let mut seeded_blocks = false;
        loop {
            match linux_adapter::load_thread_detail_read_model(&event_state, &id) {
                Ok(Some(detail)) => {
                    let detail = codex::window_thread_detail(detail, Some(THREAD_EVENT_BLOCK_WINDOW), None);
                    if !seeded_blocks {
                        seed_thread_event_blocks(&mut sent_blocks, &detail.blocks);
                        seeded_blocks = true;
                    }
                    for block in &detail.blocks {
                        if block_changed(sent_blocks.get(&block.id), block) {
                            let key = thread_event_block_key(block);
                            yield Ok::<Event, std::convert::Infallible>(
                                Event::default()
                                    .event("block")
                                    .data(serde_json::to_string(block).unwrap_or_else(|_| "{}".to_string()))
                            );
                            sent_blocks.insert(block.id.clone(), key);
                        }
                    }
                    yield Ok::<Event, std::convert::Infallible>(
                        Event::default().event("summary").data(serde_json::to_string(&detail.summary).unwrap_or_else(|_| "{}".to_string()))
                    );
                }
                Ok(None) => {
                    yield Ok::<Event, std::convert::Infallible>(
                        Event::default().event("error").data(json!({"message":"thread not found"}).to_string())
                    );
                    break;
                }
                Err(err) => {
                    yield Ok::<Event, std::convert::Infallible>(
                        Event::default().event("error").data(json!({"message": err.to_string()}).to_string())
                    );
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    };
    Ok(Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(25))
                .text("ping"),
        )
        .into_response())
}

pub(crate) fn thread_event_block_key(block: &MessageBlock) -> String {
    serde_json::to_string(block).unwrap_or_else(|_| {
        format!(
            "{}:{}:{}:{}:{}:{}",
            block.id,
            block.kind,
            block.status.as_deref().unwrap_or_default(),
            block.summary.as_deref().unwrap_or_default(),
            block.text.as_deref().unwrap_or_default(),
            block.input.as_deref().unwrap_or_default()
        )
    })
}

pub(crate) fn block_changed(previous: Option<&String>, block: &MessageBlock) -> bool {
    previous.is_none_or(|previous| previous != &thread_event_block_key(block))
}

pub(crate) fn seed_thread_event_blocks(
    sent_blocks: &mut HashMap<String, String>,
    blocks: &[MessageBlock],
) {
    for block in blocks {
        sent_blocks.insert(block.id.clone(), thread_event_block_key(block));
    }
}
