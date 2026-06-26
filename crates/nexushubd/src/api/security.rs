use super::{api_error, ok, ApiResponse};
use crate::{
    auth::{require_auth, require_csrf, verify_password},
    state::AppState,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use nexushub_core::{
    platform::{PlatformKind, PlatformPaths},
    services::{security as security_service, use_cases::NexusHubUseCases},
};
use serde_json::{json, Value};

pub(crate) async fn public_settings(State(state): State<AppState>) -> ApiResponse {
    let config = state.config();
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let security = state
        .db
        .security_settings(config.security.session_ttl_seconds)?;
    ok(NexusHubUseCases::with_config(&config, &platform)
        .security()?
        .public_view(
            security,
            state.db.get_setting("turnstile_expected_action")?,
            state.db.admin_count()? > 0,
            config.server.public_base_url.clone(),
        )?)
}

pub(crate) async fn get_security(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(security_response(&state)?)
}

pub(crate) async fn patch_security(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<security_service::SecurityPatch>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let config = state.config();
    let plan = NexusHubUseCases::with_config(&config, &platform)
        .security()
        .and_then(|security| security.patch(payload))
        .map(|plan| plan.patch)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    for write in &plan.settings {
        state.db.set_setting(write.key, &write.value)?;
    }
    if let Some(secret_key) = plan.turnstile_secret_key.as_deref() {
        state.db.set_turnstile_secret(secret_key)?;
    }
    state.db.record_audit(
        Some(&auth.admin_id),
        "security.updated",
        Some("security"),
        Some("settings"),
        None,
        plan.audit_detail,
    )?;
    ok(security_response(&state)?)
}

pub(crate) async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<security_service::PasswordChangeRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let admin = state
        .db
        .admin_by_id(&auth.admin_id)?
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "unauthorized"))?;
    let current_password_matches = verify_password(&payload.current_password, &admin.password_hash);
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let config = state.config();
    let plan = NexusHubUseCases::with_config(&config, &platform)
        .security()
        .and_then(|security| security.change_password(payload, current_password_matches))
        .map(|plan| plan.change)
        .map_err(|err| {
            let message = err.to_string();
            let status = if message.contains("invalid current password") {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::BAD_REQUEST
            };
            api_error(status, &message)
        })?;
    let hash = crate::auth::hash_password(&plan.new_password)?;
    state.db.upsert_admin(&admin.id, &admin.username, &hash)?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "security.password_changed",
        Some("admin"),
        Some(&admin.username),
        None,
        Value::Object(Default::default()),
    )?;
    ok(json!({"ok": true}))
}

fn security_response(state: &AppState) -> anyhow::Result<Value> {
    let config = state.config();
    let security = state
        .db
        .security_settings(config.security.session_ttl_seconds)?;
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let view = NexusHubUseCases::with_config(&config, &platform)
        .security()?
        .view(
            security,
            state.db.get_setting("turnstile_expected_hostname")?,
            state.db.get_setting("turnstile_expected_action")?,
        )?;
    serde_json::to_value(view).map_err(Into::into)
}
