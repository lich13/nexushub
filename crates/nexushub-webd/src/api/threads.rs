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
        use_cases::NexusHubUseCases,
    },
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
    ok(linux_adapter::apply_thread_state_action_plan(&state, &auth, &plan).await?)
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
    ok(linux_adapter::apply_thread_state_action_plan(&state, &auth, &plan).await?)
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
    ok(linux_adapter::apply_thread_state_action_plan(&state, &auth, &plan).await?)
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
