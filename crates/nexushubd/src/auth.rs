use crate::state::AppState;
use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::http::HeaderMap;
use chrono::Utc;
use rand::{distributions::Alphanumeric, Rng};
use serde::Serialize;

pub const SESSION_COOKIE: &str = "nexushub_session";
pub const CSRF_HEADER: &str = "x-csrf-token";

#[derive(Clone, Debug, Serialize)]
pub struct AuthContext {
    pub admin_id: String,
    pub username: String,
    pub session_id: String,
    pub csrf_token_hash: String,
}

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("hash password: {e}"))?
        .to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed_hash) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

pub fn random_token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

pub fn session_cookie(token: &str, secure: bool, max_age_seconds: u64) -> String {
    let mut cookie = format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age_seconds}"
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

pub fn expired_session_cookie(secure: bool) -> String {
    let mut cookie = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

pub fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in value.split(';') {
        let part = part.trim();
        let (cookie_name, cookie_value) = part.split_once('=')?;
        if cookie_name == name {
            return Some(cookie_value.to_string());
        }
    }
    None
}

pub fn require_auth(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<AuthContext, axum::http::StatusCode> {
    let token =
        extract_cookie(headers, SESSION_COOKIE).ok_or(axum::http::StatusCode::UNAUTHORIZED)?;
    let session = state
        .db
        .session_by_token(&token)
        .map_err(|_| axum::http::StatusCode::UNAUTHORIZED)?
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;
    let admin = state
        .db
        .admin_by_id(&session.admin_id)
        .map_err(|_| axum::http::StatusCode::UNAUTHORIZED)?
        .ok_or(axum::http::StatusCode::UNAUTHORIZED)?;
    Ok(AuthContext {
        admin_id: admin.id,
        username: admin.username,
        session_id: session.id,
        csrf_token_hash: session.csrf_token_hash,
    })
}

pub fn require_csrf(headers: &HeaderMap, auth: &AuthContext) -> Result<(), axum::http::StatusCode> {
    let token = headers
        .get(CSRF_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or(axum::http::StatusCode::FORBIDDEN)?;
    if nexushub_core::security::hash_token(token) != auth.csrf_token_hash {
        return Err(axum::http::StatusCode::FORBIDDEN);
    }
    Ok(())
}

pub fn expires_at(ttl_seconds: u64) -> i64 {
    Utc::now().timestamp() + ttl_seconds as i64
}

#[cfg(test)]
mod tests {
    use super::{session_cookie, SESSION_COOKIE};

    #[test]
    fn session_cookie_uses_nexushub_name() {
        assert_eq!(SESSION_COOKIE, "nexushub_session");
        let cookie = session_cookie("token", true, 60);
        assert!(cookie.starts_with("nexushub_session=token;"));
    }
}
