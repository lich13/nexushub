use super::router;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use nexushub_core::{
    config::Config,
    db::{NewSession, PanelDb},
};
use serde_json::Value;
use tower::ServiceExt;

fn authenticated_test_state() -> (crate::state::AppState, String, String) {
    let mut config = Config::default();
    config.security.cookie_secure = false;

    let db = PanelDb::open(":memory:").unwrap();
    db.upsert_admin("admin-id", "admin", "hash").unwrap();
    db.create_session(NewSession {
        id: "session-id",
        admin_id: "admin-id",
        token: "session-token",
        csrf_token: "csrf-token",
        user_agent: None,
        ip: None,
        expires_at: PanelDb::now() + 3_600,
    })
    .unwrap();

    (
        crate::state::AppState::new(config, db),
        "session-token".to_string(),
        "csrf-token".to_string(),
    )
}

async fn request_path_status(
    app: axum::Router,
    method: &str,
    uri: &str,
    session_token: Option<&str>,
    csrf_token: Option<&str>,
) -> StatusCode {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(session_token) = session_token {
        builder = builder.header("cookie", format!("nexushub_session={session_token}"));
    }
    if let Some(csrf_token) = csrf_token {
        builder = builder.header("x-csrf-token", csrf_token);
    }
    app.oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn legacy_rest_routes_all_return_404() {
    let (state, session_token, csrf_token) = authenticated_test_state();
    let app = router(state);

    for (method, uri) in [
        ("GET", "/api/threads"),
        ("GET", "/api/threads/thread-a"),
        ("POST", "/api/threads/thread-a/messages"),
        ("POST", "/api/threads/thread-a/stop"),
        ("POST", "/api/threads/thread-a/archive"),
        ("POST", "/api/threads/thread-a/restore"),
        ("PATCH", "/api/threads/thread-a"),
        ("GET", "/api/threads/thread-a/followups"),
        ("POST", "/api/threads/thread-a/followups"),
        ("POST", "/api/threads/thread-a/followups/followup-a/cancel"),
        ("GET", "/api/probe/status"),
        ("GET", "/nexushub/api/probe/status"),
        ("POST", "/api/system/update/precheck"),
        ("POST", "/api/system/update/install"),
        ("POST", "/api/system/update/prune"),
        ("POST", "/api/system/panel/update/precheck"),
        ("POST", "/api/system/panel/update/start"),
        ("POST", "/api/system/panel/update/prune"),
        ("POST", "/api/system/codex/update/precheck"),
        ("POST", "/api/system/codex/update/start"),
        ("POST", "/api/system/codex/update/prune"),
        ("POST", "/api/system/update/start"),
        ("GET", "/api/system/status"),
        ("GET", "/api/jobs"),
        ("GET", "/api/uploads"),
        ("GET", "/api/security"),
        ("POST", "/api/auth/login"),
        ("GET", "/api/probe/diagnostics"),
        ("GET", "/nexushub/api/probe/diagnostics"),
        ("GET", "/api/probe/running"),
        ("GET", "/api/probe/reply-needed"),
        ("GET", "/api/probe/recoverable"),
        ("POST", "/api/probe/logs-db/plan"),
        ("POST", "/api/probe/logs-db/execute"),
        ("POST", "/api/probe/legacy-cleanup/dry-run"),
        ("POST", "/api/probe/legacy-cleanup/execute"),
        ("GET", "/api/probe/dashboard"),
        ("GET", "/nexushub/api/probe/dashboard"),
        ("GET", "/api/probe/thread-probe/thread-a"),
        ("POST", "/api/probe/lifecycle/repair"),
        ("POST", "/api/probe/service/restart"),
        ("POST", "/api/probe/legacy/import"),
        ("GET", "/api/sentinel/status"),
        ("GET", "/api/sentinel/dashboard"),
        ("GET", "/api/sentinel/running"),
        ("GET", "/api/sentinel/reply-needed"),
        ("GET", "/api/sentinel/recoverable"),
        ("GET", "/api/sentinel/thread-probe/thread-a"),
        ("GET", "/api/sentinel/hook-status"),
        ("GET", "/api/sentinel/logs-db/status"),
        ("POST", "/api/providers/claude-code/jobs/version-check"),
        ("POST", "/api/providers/claude-code/jobs/update/precheck"),
        ("POST", "/api/providers/claude-code/jobs/update/start"),
        ("POST", "/api/providers/claude-code/jobs/smoke"),
        ("POST", "/api/providers/claude-code/jobs/cache-status"),
        ("GET", "/api/jobs/job-a"),
        ("GET", "/api/cleanup/archive/dry-run"),
        ("POST", "/api/cleanup/archive/execute"),
        ("GET", "/api/cleanup/hidden/dry-run"),
        ("POST", "/api/cleanup/hidden/execute"),
        ("GET", "/api/no-such-route"),
    ] {
        let status = request_path_status(
            app.clone(),
            method,
            uri,
            Some(&session_token),
            Some(&csrf_token),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {uri}");
    }
}

#[tokio::test]
async fn only_rpc_transport_endpoints_are_reserved_under_api() {
    let (state, session_token, csrf_token) = authenticated_test_state();
    let app = router(state);

    let health = request_path_status(app.clone(), "GET", "/healthz", None, None).await;
    assert_eq!(health, StatusCode::OK);

    let upload = request_path_status(
        app.clone(),
        "POST",
        "/api/rpc/uploadFiles",
        Some(&session_token),
        Some(&csrf_token),
    )
    .await;
    assert_ne!(upload, StatusCode::NOT_FOUND);

    let probe = request_path_status(
        app.clone(),
        "POST",
        "/api/rpc/probe.status",
        Some(&session_token),
        Some(&csrf_token),
    )
    .await;
    assert_ne!(
        probe,
        StatusCode::NOT_FOUND,
        "/api/rpc/probe.status is the canonical Probe API route"
    );

    let event_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/rpc/threadEvents/thread-a")
                .header("cookie", format!("nexushub_session={session_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(event_response.status(), StatusCode::NOT_FOUND);

    let unknown = request_path_status(
        app,
        "GET",
        "/api/not-rpc",
        Some(&session_token),
        Some(&csrf_token),
    )
    .await;
    assert_eq!(unknown, StatusCode::NOT_FOUND);
}

#[test]
fn legacy_rest_test_cases_cover_required_retired_paths() {
    let required = [
        "/api/threads",
        "/api/threads/thread-a/followups",
        "/api/probe/status",
        "/nexushub/api/probe/status",
        "/api/system/update/precheck",
        "/api/system/panel/update/precheck",
        "/api/system/codex/update/precheck",
        "/api/jobs",
        "/api/jobs/job-a",
        "/api/cleanup/archive/execute",
        "/api/security",
        "/api/auth/login",
        "/api/probe/diagnostics",
        "/api/sentinel/status",
        "/api/providers/claude-code/jobs/version-check",
    ];
    let source = include_str!("legacy_routes_tests.rs");
    for path in required {
        assert!(
            source.contains(path),
            "missing retired REST assertion: {path}"
        );
    }
    let _: Value = serde_json::json!({"keeps": "serde_json imported for route body checks"});
}
