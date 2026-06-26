use crate::{auth::require_auth, state::AppState};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
#[cfg(test)]
use nexushub_core::services::updates::{self as update_service, UpdateAction};
use nexushub_core::{
    claude_code::{self, ClaudePaths},
    local,
    providers::ProviderRegistry,
};
use serde::Serialize;
use serde_json::json;

mod cleanup;
mod goals;
mod jobs;
mod payload;
mod probe;
mod routes;
mod rpc_dispatch;
mod security;
mod system;
mod threads;
mod uploads;
mod web_auth;

#[cfg(test)]
mod entry_contract_tests;
#[cfg(test)]
mod integration_tests;
#[cfg(test)]
mod legacy_routes_tests;
#[cfg(test)]
mod test_support;

pub(crate) use cleanup::{
    archive_delete_dry_run, archive_delete_execute, hidden_threads_delete_dry_run,
    hidden_threads_delete_execute,
};
pub(crate) use goals::{
    codex_goal_clear, codex_goal_get, codex_goal_pause, codex_goal_resume, codex_goal_set,
    GoalQuery,
};
pub(crate) use jobs::{job_detail, list_jobs};
#[cfg(test)]
pub(crate) use probe::probe_config_path;
pub(crate) use probe::{
    get_probe_events, get_probe_logs_db_status, get_probe_settings, get_probe_status,
    load_probe_threads, patch_probe_settings, spawn_probe_status_refresh, start_probe_action,
    ProbeEventsQuery, ProbeStatusQuery,
};
pub(crate) use routes::router;
pub(crate) use security::{change_password, get_security, patch_security, public_settings};
pub(crate) use system::{
    codex_config, codex_models, codex_permission_profiles, http_update_platform,
    start_update_action, system_status, system_update_status, system_version,
};
pub(crate) use threads::{
    answer_approval, answer_elicitation, archive_thread, cancel_followup, create_thread,
    enqueue_followup, fork_thread, list_followups, list_threads, plan_accept, plan_revise,
    rename_thread, restore_thread, send_message, steer_thread, stop_thread, thread_blocks,
    thread_detail, thread_events,
};
#[cfg(test)]
pub(crate) use threads::{block_changed, seed_thread_event_blocks, thread_event_block_key};
pub(crate) use uploads::{delete_upload_file, upload_files};
pub(crate) use web_auth::{login, logout, me};
#[cfg(test)]
pub(crate) use web_auth::{turnstile_login_action, LoginRequest, TurnstileLoginAction};

type ApiResponse = Result<Response, ApiError>;

pub struct ApiError(Box<Response>);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        *self.0
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        api_error(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string())
    }
}

async fn list_providers(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(ProviderRegistry::default().list())
}

async fn claude_code_overview(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let paths = std::env::var_os("NEXUSHUB_CLAUDE_HOME")
        .map(ClaudePaths::new)
        .unwrap_or_else(ClaudePaths::default_for_user);
    ok(claude_code::claude_overview(&paths)?)
}

async fn platform_overview(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(state.platform().clone())
}

async fn list_plugins(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(local::local_plugin_catalog())
}

fn ok<T: Serialize>(value: T) -> ApiResponse {
    Ok(Json(value).into_response())
}

fn api_error(status: StatusCode, message: &str) -> ApiError {
    ApiError(Box::new(
        (status, Json(json!({ "error": message }))).into_response(),
    ))
}
