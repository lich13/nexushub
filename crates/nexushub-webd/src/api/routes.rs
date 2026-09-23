use super::{api_error, rpc_dispatch::rpc_dispatch, thread_events, ApiResponse};
use crate::{
    rpc_surface::{LEGACY_API_FALLBACK_ROUTE, RPC_COMMAND_ROUTE, RPC_THREAD_EVENTS_ROUTE},
    state::AppState,
};
use axum::{
    http::StatusCode,
    routing::{any, get, post},
    Router,
};
use serde_json::json;

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route(RPC_THREAD_EVENTS_ROUTE, get(thread_events))
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
