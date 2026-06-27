use super::{api_error, http_update_platform, ok, ApiResponse};
use crate::{
    auth::{require_auth, require_csrf},
    linux_adapter,
    state::AppState,
};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use nexushub_core::services::{goals as goal_service, use_cases::NexusHubUseCases};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct GoalQuery {
    pub(crate) thread_id: Option<String>,
}

pub(crate) async fn codex_goal_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<GoalQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .goals()
        .get(goal_service::GoalGetRequest {
            thread_id: query.thread_id,
        })
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::goal_get_plan(&state, plan)?)
}

pub(crate) async fn codex_goal_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<goal_service::GoalUpdateRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .goals()
        .save(payload)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::apply_goal_command_plan(&state, plan)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?)
}

pub(crate) async fn codex_goal_clear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<goal_service::GoalUpdateRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .goals()
        .clear(payload.thread_id.as_deref())
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::apply_goal_command_plan(&state, plan)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?)
}

pub(crate) async fn codex_goal_pause(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<goal_service::GoalUpdateRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let thread_id = payload.thread_id.as_deref().unwrap_or_default();
    let plan = linux_adapter::goal_pause_plan(&state, thread_id)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::apply_goal_command_plan(&state, plan)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?)
}

pub(crate) async fn codex_goal_resume(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<goal_service::GoalUpdateRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let thread_id = payload.thread_id.as_deref().unwrap_or_default();
    let plan = linux_adapter::goal_resume_plan(&state, thread_id)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::apply_goal_command_plan(&state, plan)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?)
}
