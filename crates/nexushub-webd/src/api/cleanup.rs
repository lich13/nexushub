use super::{api_error, http_update_platform, ok, ApiResponse};
use crate::{
    auth::{require_auth, require_csrf},
    linux_adapter,
    state::AppState,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use nexushub_core::services::{
    cleanup::{CleanupExecuteRequest, CleanupOperationKind, CleanupTarget},
    use_cases::NexusHubUseCases,
};
use serde::Deserialize;

pub(crate) async fn archive_delete_dry_run(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .cleanup()
        .dry_run(CleanupTarget::Archived)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::execute_cleanup_plan(&state, &auth, plan)?)
}

#[derive(Debug, Deserialize)]
pub(crate) struct ArchiveExecuteRequest {
    confirmed: bool,
    #[serde(default, alias = "expectedCount", alias = "expected_count")]
    expected_count: Option<u64>,
}

pub(crate) async fn archive_delete_execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ArchiveExecuteRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .cleanup()
        .execute_confirmed(
            CleanupTarget::Archived,
            CleanupExecuteRequest {
                confirmed: payload.confirmed,
                expected_count: payload.expected_count,
            },
        )
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::execute_cleanup_plan(&state, &auth, plan)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?)
}

pub(crate) async fn hidden_threads_delete_dry_run(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .cleanup()
        .operation(CleanupTarget::Hidden, CleanupOperationKind::DryRun)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::execute_cleanup_plan(&state, &auth, plan)?)
}

pub(crate) async fn hidden_threads_delete_execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ArchiveExecuteRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .cleanup()
        .execute_confirmed(
            CleanupTarget::Hidden,
            CleanupExecuteRequest {
                confirmed: payload.confirmed,
                expected_count: payload.expected_count,
            },
        )
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(linux_adapter::execute_cleanup_plan(&state, &auth, plan)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?)
}
