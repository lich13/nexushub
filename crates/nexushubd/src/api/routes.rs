use super::{api_error, rpc_dispatch::rpc_dispatch, thread_events, upload_files, ApiResponse};
use crate::{
    rpc_surface::{
        LEGACY_API_FALLBACK_ROUTE, RPC_COMMAND_ROUTE, RPC_THREAD_EVENTS_ROUTE,
        RPC_UPLOAD_FILES_ROUTE,
    },
    state::AppState,
};
use axum::{
    extract::DefaultBodyLimit,
    http::StatusCode,
    routing::{any, get, post},
    Router,
};
use nexushub_core::uploads::MAX_TOTAL_UPLOAD_BYTES;
use serde_json::json;

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route(RPC_THREAD_EVENTS_ROUTE, get(thread_events))
        .route(
            RPC_UPLOAD_FILES_ROUTE,
            post(upload_files).layer(DefaultBodyLimit::max(MAX_TOTAL_UPLOAD_BYTES + 1024 * 1024)),
        )
        .route(RPC_COMMAND_ROUTE, post(rpc_dispatch))
        .route(LEGACY_API_FALLBACK_ROUTE, any(api_not_found))
        .with_state(state)
}

async fn healthz() -> ApiResponse {
    super::ok(json!({"ok": true}))
}

async fn api_not_found() -> ApiResponse {
    Err(api_error(StatusCode::NOT_FOUND, "not found"))
}
