use super::{api_error, http_update_platform, ok, ApiResponse};
use crate::{
    auth::{require_auth, require_csrf},
    state::AppState,
};
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use nexushub_core::{pi::PiDeleteRequest, services::use_cases::NexusHubUseCases};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct PiListQuery {
    pub q: Option<String>,
    pub limit: Option<usize>,
}

pub(crate) async fn pi_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PiListQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(NexusHubUseCases::new(&http_update_platform())
        .pi()
        .list(query.limit.unwrap_or(100), query.q.as_deref())?)
}

pub(crate) async fn pi_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_key): Path<String>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(NexusHubUseCases::new(&http_update_platform())
        .pi()
        .detail(&session_key)?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PiRenameRequest {
    pub session_key: String,
    pub title: String,
}

pub(crate) async fn pi_rename(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PiRenameRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    ok(NexusHubUseCases::new(&http_update_platform())
        .pi()
        .rename(&request.session_key, &request.title)
        .await?)
}

pub(crate) async fn pi_delete_preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_key): Path<String>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    ok(NexusHubUseCases::new(&http_update_platform())
        .pi()
        .delete_preview(&session_key)?)
}

pub(crate) async fn pi_delete_execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PiDeleteRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    ok(NexusHubUseCases::new(&http_update_platform())
        .pi()
        .delete_execute(request)?)
}
