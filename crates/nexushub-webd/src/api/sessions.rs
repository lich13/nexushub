use super::{api_error, ok, ApiResponse};
use crate::{
    auth::{require_auth, require_csrf},
    state::AppState,
};
use axum::{extract::State, http::HeaderMap, Json};
use nexushub_core::services::{
    sessions::{SessionBatchExecuteRequest, SessionBatchRequest},
    use_cases::NexusHubUseCases,
};

pub(crate) async fn bulk_preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SessionBatchRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let result = tokio::task::spawn_blocking(move || {
        NexusHubUseCases::new(state.platform())
            .sessions(state.codex_paths())
            .bulk_preview(request)
    })
    .await
    .map_err(anyhow::Error::from)??;
    ok(result)
}

pub(crate) async fn bulk_execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SessionBatchExecuteRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let detail = serde_json::json!({"provider": request.provider, "operation": request.operation, "count": request.items.len()});
    state.db.record_audit(
        Some(&auth.admin_id),
        "sessions.bulkExecute.requested",
        Some("sessions"),
        None,
        None,
        detail,
    )?;
    let runtime = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        NexusHubUseCases::new(runtime.platform())
            .sessions(runtime.codex_paths())
            .bulk_execute(request)
    })
    .await
    .map_err(anyhow::Error::from)??;
    state.db.record_audit(Some(&auth.admin_id), "sessions.bulkExecute.completed", Some("sessions"), None, None,
        serde_json::json!({"succeeded": result.items.iter().filter(|item| item.status == "succeeded").count(), "blocked": result.items.iter().filter(|item| item.status == "blocked").count(), "failed": result.items.iter().filter(|item| item.status == "failed").count()}))?;
    ok(result)
}
