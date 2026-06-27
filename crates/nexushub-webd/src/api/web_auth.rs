use super::{api_error, ok, ApiResponse};
use crate::{
    auth::{
        expired_session_cookie, expires_at, login_username_for_surface, random_token, require_auth,
        session_cookie, verify_password, SESSION_COOKIE,
    },
    state::AppState,
    turnstile::verify_turnstile,
};
use axum::{
    extract::{connect_info::ConnectInfo, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use nexushub_core::db::NewSession;
use nexushub_core::services::system::HostSurface;
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub(crate) struct LoginRequest {
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) turnstile_token: Option<String>,
}

pub(crate) async fn login(
    State(state): State<AppState>,
    connect: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> ApiResponse {
    let ip = client_ip(&headers, &state, connect.as_ref().map(|c| c.0));
    let limiter_key = format!(
        "{}:{}",
        payload.username,
        ip.as_deref().unwrap_or("unknown")
    );
    if !state
        .login_limiter
        .lock()
        .expect("login limiter")
        .check(&limiter_key)
    {
        return Err(api_error(
            StatusCode::TOO_MANY_REQUESTS,
            "too many login attempts",
        ));
    }
    let mut security = state
        .db
        .security_settings(state.config().security.session_ttl_seconds)?;
    if state.host_surface() == HostSurface::DesktopLanWebui {
        let config = state.config();
        security.turnstile_enabled = false;
        security.turnstile_required = false;
        security.session_ttl_seconds = config.desktop_webui.session_ttl_seconds;
    }
    match turnstile_login_action(security.turnstile_enabled, security.turnstile_required) {
        TurnstileLoginAction::Skip => {}
        TurnstileLoginAction::FailClosed => {
            return Err(api_error(StatusCode::FORBIDDEN, "turnstile is required"));
        }
        TurnstileLoginAction::Verify => {
            let token = payload.turnstile_token.as_deref().unwrap_or("");
            if let Err(err) = verify_turnstile(&state, token, ip.as_deref()).await {
                state.db.record_audit(
                    None,
                    "login.turnstile_failed",
                    Some("auth"),
                    Some(&payload.username),
                    ip.as_deref(),
                    json!({"error": err.to_string()}),
                )?;
                return Err(api_error(StatusCode::UNAUTHORIZED, &err.to_string()));
            }
        }
    }
    let login_username = login_username_for_surface(state.host_surface(), &payload.username);
    let Some(admin) = state.db.admin_by_username(&login_username)? else {
        return Err(api_error(StatusCode::UNAUTHORIZED, "invalid credentials"));
    };
    if !verify_password(&payload.password, &admin.password_hash) {
        state.db.record_audit(
            Some(&admin.id),
            "login.failed",
            Some("admin"),
            Some(&payload.username),
            ip.as_deref(),
            Value::Object(Default::default()),
        )?;
        return Err(api_error(StatusCode::UNAUTHORIZED, "invalid credentials"));
    }
    let token = random_token();
    let csrf = random_token();
    let ttl = security.session_ttl_seconds;
    let session_id = Uuid::new_v4().to_string();
    state.db.create_session(NewSession {
        id: &session_id,
        admin_id: &admin.id,
        token: &token,
        csrf_token: &csrf,
        user_agent: headers
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok()),
        ip: ip.as_deref(),
        expires_at: expires_at(ttl),
    })?;
    state.db.record_audit(
        Some(&admin.id),
        "login.success",
        Some("admin"),
        Some(&payload.username),
        ip.as_deref(),
        Value::Object(Default::default()),
    )?;
    let mut response = Json(json!({
        "id": admin.id,
        "username": payload.username,
        "csrf_token": csrf,
    }))
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie(
            &token,
            state.config().security.cookie_secure,
            ttl,
        ))
        .expect("valid cookie"),
    );
    Ok(response)
}

pub(crate) async fn logout(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    if let Some(token) = crate::auth::extract_cookie(&headers, SESSION_COOKIE) {
        state.db.revoke_session(&token)?;
    }
    let mut response = Json(json!({"ok": true})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&expired_session_cookie(
            state.config().security.cookie_secure,
        ))
        .expect("valid cookie"),
    );
    Ok(response)
}

pub(crate) async fn me(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(json!({
        "id": auth.admin_id,
        "username": auth.username,
        "csrf_token": null,
        "session_id": auth.session_id
    }))
}

fn client_ip(headers: &HeaderMap, state: &AppState, source: Option<SocketAddr>) -> Option<String> {
    if state.config().server.trust_forwarded_headers {
        if let Some(ip) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            return ip.split(',').next().map(|v| v.trim().to_string());
        }
        if let Some(ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
            return Some(ip.to_string());
        }
    }
    source.map(|addr| addr.ip().to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnstileLoginAction {
    Skip,
    Verify,
    FailClosed,
}

pub(crate) fn turnstile_login_action(enabled: bool, required: bool) -> TurnstileLoginAction {
    if enabled {
        TurnstileLoginAction::Verify
    } else if required {
        TurnstileLoginAction::FailClosed
    } else {
        TurnstileLoginAction::Skip
    }
}
