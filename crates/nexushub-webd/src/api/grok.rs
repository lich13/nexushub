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
use nexushub_core::{grok::GrokDeleteRequest, services::use_cases::NexusHubUseCases};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct GrokListQuery {
    pub q: Option<String>,
    pub limit: Option<usize>,
}

pub(crate) async fn grok_list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<GrokListQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(NexusHubUseCases::new(&http_update_platform())
        .grok()
        .list(query.limit.unwrap_or(50), query.q.as_deref())?)
}

pub(crate) async fn grok_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(NexusHubUseCases::new(&http_update_platform())
        .grok()
        .detail(&id)?)
}

#[derive(Debug, Deserialize)]
pub(crate) struct GrokRenameRequest {
    pub id: String,
    pub title: String,
}

pub(crate) async fn grok_rename(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<GrokRenameRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    ok(NexusHubUseCases::new(&http_update_platform())
        .grok()
        .rename(&request.id, &request.title)
        .await?)
}

pub(crate) async fn grok_delete_preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    ok(NexusHubUseCases::new(&http_update_platform())
        .grok()
        .delete_preview(&id)?)
}

pub(crate) async fn grok_delete_execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<GrokDeleteRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    ok(NexusHubUseCases::new(&http_update_platform())
        .grok()
        .delete_execute(request)?)
}
