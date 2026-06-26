use super::{
    block_changed, load_probe_threads, probe_config_path, router, seed_thread_event_blocks,
    test_support::source_line_count, thread_event_block_key, turnstile_login_action,
    update_service, TurnstileLoginAction, UpdateAction,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use nexushub_core::codex::{MessageBlock, ThreadDetail, ThreadStatus, ThreadSummary};
use nexushub_core::{
    config::Config,
    db::{JobRecord, NewSession, PanelDb, ThreadFollowUp},
    platform::{PlatformKind, PlatformPaths},
    services::{
        app_server_threads::{
            app_server_detail_from_read, app_server_thread_list_fetch_limit,
            app_server_thread_summaries, apply_app_server_thread_detail, archived_filter,
            merge_thread_summaries, thread_title,
        },
        goals::normalize_goal_response_value,
        jobs as job_service,
        jobs::ThreadMessageRequest,
        probe::PROBE_REPLY_NEEDED_FRESH_WINDOW_SECONDS,
        threads as thread_service,
    },
    uploads::{PreparedAttachment, UploadKind},
};
use rusqlite::{params, Connection};
use serde_json::json;
use std::collections::HashSet;
use std::{
    collections::HashMap,
    env, fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex, OnceLock,
    },
};
use tower::ServiceExt;

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);
static CONFIG_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[test]
fn api_facade_stays_under_line_budget() {
    assert!(
        source_line_count("api.rs") < 260,
        "api.rs should stay a thin facade; move tests and domain logic into api/* modules"
    );
}

struct ConfigEnvGuard {
    _guard: std::sync::MutexGuard<'static, ()>,
    previous: Option<std::ffi::OsString>,
}

impl ConfigEnvGuard {
    fn set(path: &std::path::Path) -> Self {
        let guard = CONFIG_ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = env::var_os("NEXUSHUB_CONFIG");
        env::set_var("NEXUSHUB_CONFIG", path);
        Self {
            _guard: guard,
            previous,
        }
    }
}

impl Drop for ConfigEnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_ref() {
            env::set_var("NEXUSHUB_CONFIG", previous);
        } else {
            env::remove_var("NEXUSHUB_CONFIG");
        }
    }
}

fn temp_test_dir(prefix: &str) -> PathBuf {
    let unique = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    env::temp_dir().join(format!("{}-{}-{unique}", prefix, std::process::id()))
}

fn seed_codex_logs_db(path: &std::path::Path, timestamps: &[i64]) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL,
            ts_nanos INTEGER NOT NULL,
            level TEXT NOT NULL,
            target TEXT NOT NULL,
            feedback_log_body TEXT,
            module_path TEXT,
            file TEXT,
            line INTEGER,
            thread_id TEXT,
            process_uuid TEXT,
            estimated_bytes INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX idx_logs_ts ON logs(ts DESC, ts_nanos DESC, id DESC);
        "#,
    )
    .unwrap();
    for ts in timestamps {
        conn.execute(
            "INSERT INTO logs(ts, ts_nanos, level, target, estimated_bytes) VALUES(?1, 0, 'INFO', 'test', 1)",
            params![ts],
        )
        .unwrap();
    }
}

fn mark_codex_home(home: &std::path::Path) {
    fs::create_dir_all(home.join("sessions")).unwrap();
    fs::write(home.join("state_5.sqlite"), b"").unwrap();
    fs::write(home.join("session_index.jsonl"), b"").unwrap();
    fs::create_dir_all(home.join("app-server-control")).unwrap();
}

fn authenticated_test_state() -> (crate::state::AppState, String, String) {
    let mut config = Config::default();
    config.security.cookie_secure = false;
    config.codex.bridge_enabled = false;
    config.update.panel_precheck_command = "true".to_string();
    config.update.panel_update_command = "true".to_string();
    config.update.prune_command = "true".to_string();

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

fn authenticated_test_state_with_config_file(
) -> (crate::state::AppState, String, String, PathBuf, PathBuf) {
    let (state, session_token, csrf_token) = authenticated_test_state();
    let dir = temp_test_dir("nexushub-config");
    fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("config.toml");
    fs::write(
        &config_path,
        toml::to_string_pretty(&state.config()).unwrap(),
    )
    .unwrap();
    (state, session_token, csrf_token, dir, config_path)
}

fn fallback_summary(id: &str, title: &str) -> ThreadSummary {
    ThreadSummary {
        id: id.to_string(),
        title: title.to_string(),
        status: ThreadStatus::Recent,
        updated_at: None,
        archived_at: None,
        message_count: 0,
        latest_message: None,
        cwd: None,
        model: None,
        rollout_path: None,
        active_turn_id: None,
        active_job_id: None,
        pending_elicitation: None,
        last_event_kind: None,
    }
}

fn seed_local_codex_thread(home: &std::path::Path, thread_id: &str, title: &str) -> PathBuf {
    fs::create_dir_all(home).unwrap();
    mark_codex_home(home);
    let rollout = home.join(format!("{thread_id}.jsonl"));
    fs::write(
        &rollout,
        [
            json!({"session_meta":{"payload":{"id":thread_id}}}).to_string(),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"local detail body"}]}}).to_string(),
        ]
        .join("\n"),
    )
    .unwrap();
    let conn = Connection::open(home.join("state_5.sqlite")).unwrap();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS threads(
            id TEXT PRIMARY KEY,
            rollout_path TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            source TEXT NOT NULL,
            model_provider TEXT NOT NULL,
            cwd TEXT NOT NULL,
            title TEXT NOT NULL,
            sandbox_policy TEXT NOT NULL,
            approval_mode TEXT NOT NULL,
            archived INTEGER NOT NULL DEFAULT 0,
            archived_at INTEGER,
            preview TEXT NOT NULL DEFAULT ''
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO threads(id, rollout_path, created_at, updated_at, source, model_provider, cwd, title, sandbox_policy, approval_mode, archived, archived_at, preview)
         VALUES(?1, ?2, 1, 2, 'codex', '', '/tmp', ?3, '', '', 0, NULL, 'preview should not replace title')",
        (thread_id, rollout.display().to_string(), title),
    )
    .unwrap();
    rollout
}

fn app_server_missing_socket_state() -> (crate::state::AppState, String, String, PathBuf) {
    let (state, session_token, csrf_token) = authenticated_test_state();
    let home = temp_test_dir("nexushub-local-codex");
    let mut config = state.config();
    config.codex.home = home.clone();
    config.codex.workspace = home.clone();
    config.codex.bridge_enabled = true;
    config.codex.app_server_socket = Some(home.join("missing-app-server.sock"));
    state.replace_config(config);
    (state, session_token, csrf_token, home)
}

async fn request_rpc_json(
    app: axum::Router,
    command: &str,
    body: &str,
    session_token: &str,
    csrf_token: Option<&str>,
) -> serde_json::Value {
    let mut builder = Request::builder()
        .method("POST")
        .uri(format!("/api/rpc/{command}"))
        .header("cookie", format!("nexushub_session={session_token}"))
        .header("content-type", "application/json");
    if let Some(csrf_token) = csrf_token {
        builder = builder.header("x-csrf-token", csrf_token);
    }
    let response = app
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK, "RPC {command}");
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn request_rpc_status(
    app: axum::Router,
    command: &str,
    body: &str,
    session_token: Option<&str>,
    csrf_token: Option<&str>,
) -> StatusCode {
    let mut builder = Request::builder()
        .method("POST")
        .uri(format!("/api/rpc/{command}"))
        .header("content-type", "application/json");
    if let Some(session_token) = session_token {
        builder = builder.header("cookie", format!("nexushub_session={session_token}"));
    }
    if let Some(csrf_token) = csrf_token {
        builder = builder.header("x-csrf-token", csrf_token);
    }
    app.oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
        .status()
}

#[tokio::test]
async fn thread_routes_use_local_state_when_app_server_socket_is_missing() {
    let (state, session_token, csrf_token, home) = app_server_missing_socket_state();
    seed_local_codex_thread(&home, "thread-a", "local title");
    let app = router(state);

    let list = request_rpc_json(
        app.clone(),
        "threads.list",
        r#"{"limit":10}"#,
        &session_token,
        None,
    )
    .await;
    assert_eq!(list[0]["id"], "thread-a");
    assert_eq!(list[0]["title"], "local title");

    let detail = request_rpc_json(
        app.clone(),
        "threads.detail",
        r#"{"id":"thread-a"}"#,
        &session_token,
        None,
    )
    .await;
    assert_eq!(detail["summary"]["title"], "local title");

    let _ = request_rpc_json(
        app,
        "threads.rename",
        r#"{"threadId":"thread-a","name":"local renamed"}"#,
        &session_token,
        Some(&csrf_token),
    )
    .await;

    let rows = nexushub_core::codex::list_threads(
        &nexushub_core::codex::CodexPaths::new(&home),
        None,
        Some("thread-a"),
        10,
    )
    .unwrap();
    assert_eq!(rows[0].title, "local renamed");
    let _ = fs::remove_dir_all(home);
}

#[tokio::test]
async fn probe_threads_use_local_state_when_app_server_socket_is_missing() {
    let (state, _session_token, _csrf_token, home) = app_server_missing_socket_state();
    seed_local_codex_thread(&home, "thread-a", "local title");

    let rows = load_probe_threads(&state, "recent", 10).await.unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "thread-a");
    assert_eq!(rows[0].title, "local title");
    let _ = fs::remove_dir_all(home);
}

#[tokio::test]
async fn goal_routes_use_local_store_without_app_server_socket() {
    let (state, session_token, csrf_token, home) = app_server_missing_socket_state();
    seed_local_codex_thread(&home, "thread-a", "local title");
    let app = router(state);

    let initial = request_rpc_json(
        app.clone(),
        "threads.goal.get",
        r#"{"threadId":"thread-a"}"#,
        &session_token,
        None,
    )
    .await;
    assert_eq!(initial["available"], true);
    assert_eq!(initial["enabled"], false);
    assert_eq!(initial["status"], "idle");

    let set = request_rpc_json(
        app.clone(),
        "threads.goal.save",
        r#"{"thread_id":"thread-a","objective":"ship local goal","token_budget":12345,"status":"paused","enabled":false}"#,
        &session_token,
        Some(&csrf_token),
    )
    .await;
    assert_eq!(set["available"], true);
    assert_eq!(set["enabled"], true);
    assert_eq!(set["objective"], "ship local goal");
    assert_eq!(set["token_budget"], 12345);
    assert_eq!(set["status"], "paused");

    let get = request_rpc_json(
        app.clone(),
        "threads.goal.get",
        r#"{"threadId":"thread-a"}"#,
        &session_token,
        None,
    )
    .await;
    assert_eq!(get["available"], true);
    assert_eq!(get["enabled"], true);
    assert_eq!(get["objective"], "ship local goal");
    assert_eq!(get["token_budget"], 12345);
    assert_eq!(get["status"], "paused");

    let saved_active = request_rpc_json(
        app.clone(),
        "threads.goal.save",
        r#"{"thread_id":"thread-a","objective":"ship local goal","token_budget":12345}"#,
        &session_token,
        Some(&csrf_token),
    )
    .await;
    assert_eq!(saved_active["available"], true);
    assert_eq!(saved_active["enabled"], true);
    assert_eq!(saved_active["objective"], "ship local goal");
    assert_eq!(saved_active["token_budget"], 12345);
    assert_eq!(saved_active["status"], "active");

    let paused = request_rpc_json(
        app.clone(),
        "threads.goal.pause",
        r#"{"thread_id":"thread-a"}"#,
        &session_token,
        Some(&csrf_token),
    )
    .await;
    assert_eq!(paused["available"], true);
    assert_eq!(paused["enabled"], true);
    assert_eq!(paused["objective"], "ship local goal");
    assert_eq!(paused["token_budget"], 12345);
    assert_eq!(paused["status"], "paused");

    let resumed = request_rpc_json(
        app.clone(),
        "threads.goal.resume",
        r#"{"thread_id":"thread-a"}"#,
        &session_token,
        Some(&csrf_token),
    )
    .await;
    assert_eq!(resumed["available"], true);
    assert_eq!(resumed["enabled"], true);
    assert_eq!(resumed["objective"], "ship local goal");
    assert_eq!(resumed["token_budget"], 12345);
    assert_eq!(resumed["status"], "active");

    let cleared = request_rpc_json(
        app.clone(),
        "threads.goal.clear",
        r#"{"thread_id":"thread-a"}"#,
        &session_token,
        Some(&csrf_token),
    )
    .await;
    assert_eq!(cleared["available"], true);
    assert_eq!(cleared["enabled"], false);
    assert_eq!(cleared["objective"], serde_json::Value::Null);
    assert_eq!(cleared["token_budget"], serde_json::Value::Null);
    assert_eq!(cleared["status"], "cleared");

    let resumed_after_clear = request_rpc_json(
        app,
        "threads.goal.resume",
        r#"{"thread_id":"thread-a"}"#,
        &session_token,
        Some(&csrf_token),
    )
    .await;
    assert_eq!(resumed_after_clear["available"], true);
    assert_eq!(resumed_after_clear["enabled"], true);
    assert_eq!(resumed_after_clear["status"], "active");
    let _ = fs::remove_dir_all(home);
}

#[tokio::test]
async fn rpc_goal_wrapper_preserves_goal_dto_shape() {
    let (state, session_token, csrf_token, home) = app_server_missing_socket_state();
    seed_local_codex_thread(&home, "thread-a", "local title");
    let app = router(state);

    let rpc_initial = request_rpc_json(
        app.clone(),
        "threads.goal.get",
        r#"{"threadId":"thread-a"}"#,
        &session_token,
        None,
    )
    .await;
    assert_eq!(rpc_initial["available"], true);
    assert_eq!(rpc_initial["enabled"], false);
    assert_eq!(rpc_initial["status"], "idle");

    let rpc_saved = request_rpc_json(
        app.clone(),
        "threads.goal.save",
        r#"{"request":{"threadId":"thread-a","objective":"ship rpc","tokenBudget":2048}}"#,
        &session_token,
        Some(&csrf_token),
    )
    .await;
    assert_eq!(rpc_saved["available"], true);
    assert_eq!(rpc_saved["enabled"], true);
    assert_eq!(rpc_saved["objective"], "ship rpc");
    assert_eq!(rpc_saved["token_budget"], 2048);

    let rpc_after_save = request_rpc_json(
        app,
        "threads.goal.get",
        r#"{"threadId":"thread-a"}"#,
        &session_token,
        None,
    )
    .await;
    assert_eq!(rpc_after_save, rpc_saved);
    let _ = fs::remove_dir_all(home);
}

#[tokio::test]
async fn rpc_probe_typed_commands_start_matching_jobs() {
    for (command, kind) in [
        ("probe.barkTest", "probe_bark_test"),
        ("probe.installHooks", "probe_hooks_install"),
        ("probe.logsDbDryRun", "probe_logs_db_maintain_dry_run"),
        ("probe.logsDbExecute", "probe_logs_db_maintain"),
    ] {
        let (state, session_token, csrf_token) = authenticated_test_state();
        let app = router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/rpc/{command}"))
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .header("x-csrf-token", csrf_token.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK, "{command}");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let job_id = payload["job_id"].as_str().unwrap();
        let job = state.db.job(job_id).unwrap().unwrap();
        assert_eq!(job.kind, kind, "{command}");
    }
}

#[tokio::test]
async fn rpc_update_typed_commands_start_update_jobs() {
    for (command, kind) in [
        ("updates.check", "nexushub_update_check"),
        ("updates.install", "nexushub_update_install"),
        ("updates.prune", "nexushub_update_prune"),
    ] {
        let (rpc_state, rpc_session_token, rpc_csrf_token) = authenticated_test_state();
        let rpc = request_rpc_json(
            router(rpc_state.clone()),
            command,
            "{}",
            &rpc_session_token,
            Some(&rpc_csrf_token),
        )
        .await;
        let job_id = rpc["job_id"].as_str().unwrap();
        let job = rpc_state.db.job(job_id).unwrap().unwrap();
        assert_eq!(job.kind, kind, "{command}");
    }
}

#[tokio::test]
async fn rpc_cleanup_execute_requires_expected_count_confirmation() {
    let (state, session_token, csrf_token) = authenticated_test_state();
    let app = router(state);

    for command in ["cleanup.archiveExecute", "cleanup.hiddenExecute"] {
        let status = request_rpc_status(
            app.clone(),
            command,
            r#"{"confirmed":true}"#,
            Some(&session_token),
            Some(&csrf_token),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{command}");
    }
}

#[test]
fn linux_probe_actions_execute_shared_core_job_command() {
    let api_source = include_str!("probe.rs")
        .split("\nmod tests {")
        .next()
        .expect("api/probe.rs source must include production section");
    let adapter_source = include_str!("../linux_adapter.rs");

    assert!(
        api_source.contains("linux_adapter::linux_probe_action_plan"),
        "Linux HTTP Probe handlers should request the shared core Probe plan through the adapter"
    );
    assert!(
        api_source.contains("linux_adapter::start_probe_action_plan"),
        "Linux HTTP Probe handlers should delegate Linux execution to the adapter"
    );
    assert!(
        !api_source.contains("patch_probe_config_toml"),
        "api.rs must not patch Probe config TOML directly"
    );
    assert!(
        !api_source.contains("start_exclusive_shell_job(&spec.kind"),
        "api.rs must not start Probe shell jobs directly"
    );
    for forbidden in [
        "state.jobs.start_codex_job(",
        "state.jobs.cancel_job(",
        "codex::set_thread_archived(",
        "codex::set_thread_title(",
        "start_shell_job(&spec.kind",
    ] {
        assert!(
            !api_source.contains(forbidden),
            "api.rs must execute host-specific thread/job plans through linux_adapter: {forbidden}"
        );
    }
    assert!(
        adapter_source.contains("spec.command"),
        "Linux adapter should execute ProbeFixedJobSpec.command planned by nexushub-core"
    );
    assert!(
        adapter_source.contains("patch_probe_config_toml"),
        "Linux adapter is the only nexushubd module that applies Probe settings TOML patches"
    );
    assert!(
        adapter_source.contains("ProbeUseCases::new"),
        "Linux adapter should request Probe action plans through ProbeUseCases"
    );
    assert!(
        adapter_source.contains("UpdateUseCases::new"),
        "Linux adapter should request update action plans through UpdateUseCases"
    );
}

#[test]
fn linux_entry_does_not_reimplement_migrated_goal_or_followup_transactions() {
    let source = include_str!("../api.rs")
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("api source must include production section");
    let cleanup_source = include_str!("cleanup.rs");
    let goals_source = include_str!("goals.rs");
    let jobs_source = include_str!("jobs.rs");
    let threads_source = include_str!("threads.rs");
    let handler_source =
        format!("{source}\n{cleanup_source}\n{goals_source}\n{jobs_source}\n{threads_source}");
    let adapter_source = include_str!("../linux_adapter.rs");

    for required in [
        "NexusHubUseCases::new",
        "linux_adapter::start_thread_command_execution_plan",
        "linux_adapter::start_codex_resume_action",
        "linux_adapter::resolve_thread_stop_plan",
        "linux_adapter::enqueue_followup_plan",
        "linux_adapter::cancel_thread_stop_plan",
        "linux_adapter::apply_thread_state_action_plan",
        "linux_adapter::cancel_followup_plan",
        "linux_adapter::execute_cleanup_plan",
        "linux_adapter::list_jobs_plan",
        "linux_adapter::job_detail_plan",
        "linux_adapter::goal_get_plan",
        "linux_adapter::apply_goal_command_plan",
        "linux_adapter::goal_pause_plan",
        "linux_adapter::goal_resume_plan",
    ] {
        assert!(
            handler_source.contains(required),
            "Linux RPC handlers must call the shared core facade/plan: {required}"
        );
    }
    for required_adapter_landing in [
        "NexusHubUseCases::new(&platform).cleanup()",
        ".dry_run_archived(",
        ".execute_archived(",
        ".dry_run_hidden(",
        ".execute_hidden(",
        ".validate_expected_count(",
    ] {
        assert!(
            adapter_source.contains(required_adapter_landing),
            "Linux adapter must retain cleanup side-effect landing: {required_adapter_landing}"
        );
    }

    for forbidden in [
        "state.db.get_thread_goal(",
        "state.db.upsert_thread_goal(",
        "state.db.delete_thread_goal(",
        "state.db.update_thread_goal_status(",
        "state.db.list_followups(",
        "state.db.enqueue_followup(",
        "state.db.cancel_followup(",
        "payload.name.trim()",
        "job_service::archive_thread_response(",
        "job_service::rename_thread_response(",
        "upload_service::plan_store_uploads(items)",
        "uploads::delete_upload(&root, &id)",
        "archive::plan_delete_archived(",
        "archive::execute_delete_archived(",
        "archive::plan_delete_hidden(",
        "archive::execute_delete_hidden(",
        "cleanup_service::dry_run_archived_with_capability",
        "cleanup_service::execute_archived_with_capability",
        "cleanup_service::dry_run_hidden_with_capability",
        "cleanup_service::execute_hidden_with_capability",
        "cleanup_service::validate_cleanup_expected_count",
        "\"stopThread\"",
        "\"bridge\": false",
        "\"cancelFollowUp\"",
    ] {
        assert!(
            !source.contains(forbidden),
            "Linux RPC handlers must not reimplement migrated goal/follow-up transactions: {forbidden}"
        );
    }

    let rpc_dispatch_source = include_str!("rpc_dispatch.rs");
    assert!(
        !rpc_dispatch_source.contains("ArchiveExecuteRequest { confirmed: true }"),
        "Linux RPC cleanup execute must preserve confirmed/expectedCount from the request payload"
    );

    for forbidden_keyword in [
        &format!("{}_", "desktop"),
        &format!("get{}", "Desktop"),
        &format!("start{}Job", "Probe"),
        &format!("run{}Action", "Update"),
    ] {
        assert!(
            !source.contains(forbidden_keyword),
            "api.rs must not reintroduce retired desktop/probe/update command surface: {forbidden_keyword}"
        );
    }
}

#[tokio::test]
async fn rpc_system_status_exposes_capabilities_dto_shape() {
    let (state, session_token, _) = authenticated_test_state();
    let app = router(state);

    let rpc = request_rpc_json(app, "system.status", "{}", &session_token, None).await;

    assert_eq!(rpc["capabilities"]["threads"], true);
    assert_eq!(rpc["capabilities"]["web_auth"], true);
    assert_eq!(rpc["capabilities"]["turnstile"], true);
    assert_eq!(rpc["capabilities"]["systemd"], true);
    assert_eq!(rpc["capabilities"]["nginx"], true);
    assert_eq!(rpc["capabilities"]["linux_update_job"], true);
}

#[tokio::test]
async fn rpc_update_typed_actions_start_jobs() {
    for (command, kind) in [
        ("updates.check", "nexushub_update_check"),
        ("updates.install", "nexushub_update_install"),
        ("updates.prune", "nexushub_update_prune"),
    ] {
        let (state, session_token, csrf_token) = authenticated_test_state();
        let rpc = request_rpc_json(
            router(state.clone()),
            command,
            "{}",
            &session_token,
            Some(&csrf_token),
        )
        .await;
        let job_id = rpc["job_id"].as_str().unwrap();
        let job = state.db.job(job_id).unwrap().unwrap();
        assert_eq!(job.kind, kind, "{command}");
    }
}

#[tokio::test]
async fn rpc_update_status_uses_shared_update_status_shape() {
    let (state, session_token, _) = authenticated_test_state();
    let app = router(state);

    let rpc = request_rpc_json(app, "updates.status", "{}", &session_token, None).await;

    assert_eq!(rpc["method"], "linux_systemd_job");
    assert_eq!(rpc["state"], "idle");
}

#[tokio::test]
async fn rpc_enqueue_followup_accepts_thread_id_and_payload_wrappers() {
    let (state, session_token, csrf_token) = authenticated_test_state();
    let app = router(state.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rpc/threads.followups.enqueue")
                .header("cookie", format!("nexushub_session={session_token}"))
                .header("x-csrf-token", csrf_token.as_str())
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"payload":{"threadId":"thread-a","message":"continue","serviceTier":"priority"}}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["thread_id"], "thread-a");
    assert_eq!(payload["message"], "continue");
    assert_eq!(payload["options"]["service_tier"], "priority");
}

#[tokio::test]
async fn local_codex_routes_do_not_require_app_server_socket() {
    let (state, session_token, csrf_token, home) = app_server_missing_socket_state();
    seed_local_codex_thread(&home, "thread-a", "local title");
    let app = router(state.clone());

    for (command, body) in [
        ("system.models", "{}"),
        ("system.permissionProfiles", "{}"),
        ("system.codexConfig", "{}"),
        ("system.status", "{}"),
    ] {
        let value = request_rpc_json(app.clone(), command, body, &session_token, None).await;
        let text = serde_json::to_string(&value).unwrap();
        assert!(!text.contains("app-server"), "{command}");
    }

    let config_value = request_rpc_json(
        app.clone(),
        "system.codexConfig",
        r#"{"cwd":"/tmp/workspace"}"#,
        &session_token,
        None,
    )
    .await;
    assert_eq!(config_value["cwd"], "/tmp/workspace");
    assert_eq!(config_value["raw"]["source"], "local");

    for (command, body) in [
        (
            "threads.plan.accept",
            r#"{"threadId":"thread-a","payload":{"turn_id":"turn-a","item_id":"plan-a"}}"#,
        ),
        (
            "threads.plan.revise",
            r#"{"threadId":"thread-a","payload":{"turn_id":"turn-a","item_id":"plan-a","instructions":"补充检查"}}"#,
        ),
        (
            "threads.elicitation.answer",
            r#"{"threadId":"thread-a","answers":{"q1":["继续"]}}"#,
        ),
    ] {
        let value = request_rpc_json(
            app.clone(),
            command,
            body,
            &session_token,
            Some(&csrf_token),
        )
        .await;
        assert_eq!(value["bridge"], false, "{command}");
        assert_eq!(value["fallback"], true, "{command}");
        assert_eq!(value["message"], "已提交给 Codex", "{command}");
        let message = value["message"].as_str().unwrap_or_default();
        assert!(!message.contains("fallback"), "{command}");
        assert!(!message.contains("bridge"), "{command}");
        assert!(!message.contains("codex exec"), "{command}");
        assert!(!message.contains("job"), "{command}");
        assert!(
            value["job_id"].as_str().is_some_and(|id| !id.is_empty()),
            "{command}"
        );
    }

    for (command, body) in [
        ("threads.fork", r#"{"threadId":"thread-a"}"#),
        (
            "threads.approval.answer",
            r#"{"threadId":"thread-a","payload":{"turn_id":"turn-a","item_id":"approval-a","decision":"approve"}}"#,
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/rpc/{command}"))
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .header("x-csrf-token", csrf_token.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED, "{command}");
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let error = value["error"].as_str().unwrap();
        assert!(!error.contains("app-server"), "{command}");
    }
    let _ = fs::remove_dir_all(home);
}

#[test]
fn plan_resume_messages_match_tui_intent() {
    assert_eq!(job_service::plan_accept_resume_message(), "是，实施此计划");

    let revise = job_service::plan_revise_resume_message("  补充灰度验证  ");
    assert_eq!(
        revise,
        "否，请告知 Codex 如何调整\n\n请保持 Plan Mode，只根据下面的修改要求重新给出计划，不要开始实施。\n\n修改要求：\n补充灰度验证"
    );
}

#[test]
fn plan_and_elicitation_resume_args_use_controlled_json_stdin_semantics() {
    assert_eq!(
        job_service::codex_resume_args("thread-a"),
        vec!["exec", "resume", "--all", "--json", "thread-a", "-"]
    );
}

#[test]
fn elicitation_answer_resume_message_is_stable_user_answer_text() {
    let answers = HashMap::from([
        ("q2".to_string(), vec!["B".to_string(), "C".to_string()]),
        ("q1".to_string(), vec!["A".to_string()]),
    ]);

    assert_eq!(
        job_service::elicitation_answer_resume_message(&answers),
        "q1: A\nq2: B, C"
    );
}

#[test]
fn followup_request_round_trips_attachment_options() {
    let prepared_attachments = vec![
        PreparedAttachment {
            id: "upload-md".to_string(),
            name: "notes.md".to_string(),
            mime: "text/markdown".to_string(),
            size: 42,
            sha256: "sha-md".to_string(),
            kind: UploadKind::Markdown,
            text: Some("# Notes\n\n- keep context".to_string()),
            local_image_path: None,
            local_file_path: None,
            truncated: false,
        },
        PreparedAttachment {
            id: "upload-image".to_string(),
            name: "screen.png".to_string(),
            mime: "image/png".to_string(),
            size: 12,
            sha256: "sha-image".to_string(),
            kind: UploadKind::Image,
            text: None,
            local_image_path: Some(PathBuf::from("/tmp/nexushub/uploads/screen.png")),
            local_file_path: None,
            truncated: false,
        },
    ];
    let request = ThreadMessageRequest {
        thread_id: None,
        message: String::new(),
        attachments: vec!["upload-md".to_string(), "upload-image".to_string()],
        prepared_attachments: prepared_attachments.clone(),
        model: Some("gpt-5.5".to_string()),
        service_tier: Some("priority".to_string()),
        reasoning_effort: Some("xhigh".to_string()),
        cwd: Some("/tmp/workspace".to_string()),
        permission_profile: Some("danger-full-access".to_string()),
        approval_policy: Some("never".to_string()),
        sandbox_mode: Some("danger-full-access".to_string()),
        network_access: Some(true),
        collaboration_mode: Some("async".to_string()),
    };
    let followup = ThreadFollowUp {
        id: "follow-up-1".to_string(),
        thread_id: "thread-a".to_string(),
        status: "pending".to_string(),
        message: job_service::effective_message(&request.message, &request.prepared_attachments),
        options_json: request.options_json().to_string(),
        created_at: 1,
        updated_at: 1,
        submitted_at: None,
        cancelled_at: None,
        result_json: None,
        error: None,
    };

    let restored = job_service::followup_request(&followup);

    assert_eq!(restored.message, "请根据以下附件内容继续处理。".to_string());
    assert_eq!(restored.attachments, request.attachments);
    assert_eq!(restored.prepared_attachments.len(), 2);
    assert_eq!(
        restored.prepared_attachments[0].text,
        prepared_attachments[0].text
    );
    assert_eq!(
        restored.prepared_attachments[1].local_image_path,
        prepared_attachments[1].local_image_path
    );
    assert_eq!(restored.model.as_deref(), Some("gpt-5.5"));
    assert_eq!(restored.service_tier.as_deref(), Some("priority"));
    assert_eq!(restored.reasoning_effort.as_deref(), Some("xhigh"));
    assert_eq!(restored.network_access, Some(true));
}

#[test]
fn normalize_goal_response_maps_goal_statuses() {
    for (status, enabled) in [
        ("active", true),
        ("running", true),
        ("complete", true),
        ("completed", true),
        ("blocked", true),
        ("paused", true),
        ("idle", false),
        ("missing_thread", false),
        ("cleared", false),
    ] {
        let normalized = if enabled {
            normalize_goal_response_value(&json!({
                "goal": {
                    "objective": "ship",
                    "tokenBudget": 100,
                    "status": status
                }
            }))
        } else {
            normalize_goal_response_value(&json!({
                "goal": {
                    "objective": null,
                    "tokenBudget": null,
                    "status": status
                }
            }))
        };
        assert_eq!(normalized["status"], status, "{status}");
        assert_eq!(normalized["enabled"], enabled, "{status}");
    }
    let object_status = normalize_goal_response_value(&json!({
        "goal": {
            "objective": "ship",
            "tokenBudget": 100,
            "status": { "type": "Completed" }
        }
    }));
    assert_eq!(object_status["status"], "completed");
    assert_eq!(object_status["enabled"], true);
}

#[tokio::test]
async fn goal_resume_route_requires_csrf_and_uses_local_goal_store() {
    let (state, session_token, csrf_token) = authenticated_test_state();
    let app = router(state.clone());

    let missing_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rpc/threads.goal.resume")
                .header("cookie", format!("nexushub_session={session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"thread_id":"thread-a"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

    let app = router(state);
    let resumed = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rpc/threads.goal.resume")
                .header("cookie", format!("nexushub_session={session_token}"))
                .header("x-csrf-token", csrf_token.as_str())
                .header("content-type", "application/json")
                .body(Body::from(r#"{"thread_id":"thread-a"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resumed.status(), StatusCode::OK);
    let body = to_bytes(resumed.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["available"], true);
    assert_eq!(payload["enabled"], true);
    assert_eq!(payload["status"], "active");
    assert_eq!(payload["raw"]["source"], "local");
}

#[tokio::test]
async fn goal_pause_route_requires_csrf_and_preserves_local_goal() {
    let (state, session_token, csrf_token) = authenticated_test_state();
    state
        .db
        .upsert_thread_goal(nexushub_core::db::ThreadGoalUpdate {
            thread_id: "thread-a",
            objective: Some("ship paused goal"),
            token_budget: Some(9876),
            status: "active",
            completed_at: None,
            blocked_reason: None,
        })
        .unwrap();
    let app = router(state.clone());

    let missing_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rpc/threads.goal.pause")
                .header("cookie", format!("nexushub_session={session_token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"thread_id":"thread-a"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

    let app = router(state);
    let paused = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/rpc/threads.goal.pause")
                .header("cookie", format!("nexushub_session={session_token}"))
                .header("x-csrf-token", csrf_token.as_str())
                .header("content-type", "application/json")
                .body(Body::from(r#"{"thread_id":"thread-a"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(paused.status(), StatusCode::OK);
    let body = to_bytes(paused.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["available"], true);
    assert_eq!(payload["enabled"], true);
    assert_eq!(payload["objective"], "ship paused goal");
    assert_eq!(payload["token_budget"], 9876);
    assert_eq!(payload["status"], "paused");
    assert_eq!(payload["raw"]["source"], "local");
}

#[tokio::test]
async fn panel_update_routes_require_auth_and_csrf_and_start_fixed_panel_jobs() {
    for (command, kind, title) in [
        (
            "updates.check",
            "nexushub_update_check",
            "NexusHub update precheck",
        ),
        (
            "updates.install",
            "nexushub_update_install",
            "NexusHub update install",
        ),
        (
            "updates.prune",
            "nexushub_update_prune",
            "NexusHub update backup prune",
        ),
    ] {
        let (state, session_token, csrf_token) = authenticated_test_state();
        let app = router(state.clone());

        let unauthorized = request_rpc_status(app.clone(), command, "{}", None, None).await;
        assert_eq!(unauthorized, StatusCode::UNAUTHORIZED, "{command}");

        let missing_csrf =
            request_rpc_status(app.clone(), command, "{}", Some(&session_token), None).await;
        assert_eq!(missing_csrf, StatusCode::FORBIDDEN, "{command}");

        let payload = request_rpc_json(
            app.clone(),
            command,
            "{}",
            &session_token,
            Some(&csrf_token),
        )
        .await;
        let job_id = payload["job_id"].as_str().unwrap();
        let job = state.db.job(job_id).unwrap().unwrap();

        assert_eq!(job.kind, kind, "{command}");
        assert_eq!(job.title, title, "{command}");
    }
}

#[test]
fn linux_update_adapter_builds_shell_job_specs_outside_core_service() {
    let mut config = Config::for_platform_kind(PlatformKind::Linux);
    config.update.panel_precheck_command = "nexushub-update --precheck".to_string();
    config.update.panel_update_command = "nexushub-update --install".to_string();
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);

    let precheck = update_service::linux_update_job_spec(
        &config,
        update_service::update_action_plan(&platform, UpdateAction::Check),
    )
    .unwrap();
    assert_eq!(precheck.kind, "nexushub_update_check");
    assert_eq!(precheck.command, "nexushub-update --precheck");

    let install = update_service::linux_update_job_spec(
        &config,
        update_service::update_action_plan(&platform, UpdateAction::Install),
    )
    .unwrap();
    assert_eq!(install.kind, "nexushub_update_install");
    assert_eq!(install.command, "nexushub-update --install");

    let prune = update_service::linux_update_job_spec(
        &config,
        update_service::update_action_plan(&platform, UpdateAction::Prune),
    )
    .unwrap();
    assert_eq!(prune.kind, "nexushub_update_prune");
    assert!(prune.command.contains("release update backups"));
}

#[tokio::test]
async fn unified_update_status_requires_auth_and_uses_shared_shape() {
    let (state, session_token, _) = authenticated_test_state();
    let app = router(state.clone());

    let unauthorized = request_rpc_status(app.clone(), "updates.status", "{}", None, None).await;
    assert_eq!(unauthorized, StatusCode::UNAUTHORIZED);

    let payload = request_rpc_json(app, "updates.status", "{}", &session_token, None).await;
    assert_eq!(payload["method"], "linux_systemd_job");
    assert_eq!(payload["state"], "idle");
    assert_eq!(payload["channel"], "stable");
    assert!(payload["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "job_history"));
}

#[tokio::test]
async fn system_status_exposes_linux_capabilities_without_macos_web_entries() {
    let (state, session_token, _) = authenticated_test_state();
    let app = router(state);

    let payload = request_rpc_json(app, "system.status", "{}", &session_token, None).await;
    assert_eq!(payload["platform"], "linux");
    assert_eq!(payload["capabilities"]["threads"], true);
    assert_eq!(payload["capabilities"]["jobs"], true);
    assert_eq!(payload["capabilities"]["probe"], true);
    assert_eq!(payload["capabilities"]["status"], true);
    assert_eq!(payload["capabilities"]["settings"], true);
    assert_eq!(payload["capabilities"]["job_history"], true);
    assert_eq!(payload["capabilities"]["web_auth"], true);
    assert_eq!(payload["capabilities"]["security_settings"], true);
    assert_eq!(payload["capabilities"]["turnstile"], true);
    assert_eq!(payload["capabilities"]["systemd"], true);
    assert_eq!(payload["capabilities"]["nginx"], true);
    assert_eq!(payload["capabilities"]["public_endpoint"], true);
    assert_eq!(payload["capabilities"]["admin_password"], true);
    assert_eq!(payload["capabilities"]["linux_update_job"], true);
    let text = serde_json::to_string(&payload).unwrap();
    for forbidden in [
        "LaunchAgent",
        "launchagent",
        "Cloudflare Tunnel",
        "cloudflare_tunnel",
        "macos_web_login",
        "browser_webui",
    ] {
        assert!(!text.contains(forbidden), "{forbidden}");
    }
}

#[tokio::test]
async fn probe_status_routes_use_canonical_probe_name_without_sentinel_alias() {
    let (state, session_token, _) = authenticated_test_state();
    let dir = temp_test_dir("nexushub-probe-status-resolved");
    let codex_home = dir.join(".codex");
    mark_codex_home(&codex_home);
    let mut config = state.config();
    config.codex.home = codex_home.clone();
    state.replace_config(config);
    let app = router(state.clone());

    let status = request_rpc_json(app.clone(), "probe.status", "{}", &session_token, None).await;
    assert_eq!(status["label"], "Probe");
    assert_ne!(status["label"], "Sentinel");
    assert!(status["flavor"].as_str().is_some());
    assert!(status["hook_status"].as_str().is_some());
    assert_eq!(status["logs_db_status"], "missing_db");
    assert_eq!(status["codex_home"], codex_home.to_string_lossy().as_ref());
    assert_eq!(
        status["configured_codex_home"],
        codex_home.to_string_lossy().as_ref()
    );
    assert_eq!(
        status["resolved_codex_home"],
        codex_home.to_string_lossy().as_ref()
    );
    assert_eq!(status["codex_home_source"], "configured");
    assert_eq!(status["logs_db_source"], "configured");
    assert!(status["discovery_warnings"].as_array().unwrap().is_empty());
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn probe_status_route_returns_lightweight_snapshot_immediately_and_refreshes_background() {
    let (state, session_token, _) = authenticated_test_state();
    let dir = temp_test_dir("nexushub-probe-status-snapshot");
    let codex_home = dir.join(".codex");
    mark_codex_home(&codex_home);
    let mut config = state.config();
    config.codex.home = codex_home.clone();
    state.replace_config(config);
    let app = router(state.clone());

    let first_status =
        request_rpc_json(app.clone(), "probe.status", "{}", &session_token, None).await;
    assert_eq!(first_status["label"], "Probe");
    assert_eq!(first_status["snapshot_status"], "initial");
    assert_eq!(first_status["is_refreshing"], true);
    assert_eq!(first_status["snapshot_age_seconds"], 0);

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let second_status = request_rpc_json(app, "probe.status", "{}", &session_token, None).await;
    assert_eq!(second_status["snapshot_status"], "cached");
    assert!(second_status["snapshot_age_seconds"].as_i64().is_some());
    assert!(second_status["running_threads"].as_array().is_some());
    assert!(second_status["reply_needed_threads"].as_array().is_some());
    assert!(second_status["recoverable_threads"].as_array().is_some());
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn probe_reply_needed_bucket_only_includes_fresh_pending_actions() {
    let (state, _, _) = authenticated_test_state();
    let dir = temp_test_dir("nexushub-probe-reply-needed-fresh");
    let codex_home = dir.join(".codex");
    mark_codex_home(&codex_home);
    let now = chrono::Utc::now().timestamp();
    let old_updated = now - PROBE_REPLY_NEEDED_FRESH_WINDOW_SECONDS - 60;
    let fresh_updated = now - 30;
    let old_rollout = codex_home.join("old-plan.jsonl");
    let fresh_rollout = codex_home.join("fresh-plan.jsonl");
    let plan_events = |thread_id: &str, turn_id: &str| {
        [
            json!({"session_meta":{"payload":{"id":thread_id}}}).to_string(),
            json!({"type":"event_msg","payload":{"type":"item_completed","thread_id":thread_id,"turn_id":turn_id,"item":{"type":"Plan","id":format!("{turn_id}-plan"),"text":"# 计划\n- 等待确认"}}}).to_string(),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"<proposed_plan>\n# 计划\n- 等待确认\n</proposed_plan>"}],"phase":"final_answer"}}).to_string(),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":turn_id,"last_agent_message":null}}).to_string(),
        ]
        .join("\n")
    };
    fs::write(&old_rollout, plan_events("old-thread", "turn-old")).unwrap();
    fs::write(&fresh_rollout, plan_events("fresh-thread", "turn-fresh")).unwrap();
    let conn = Connection::open(codex_home.join("state_5.sqlite")).unwrap();
    conn.execute_batch(
        "CREATE TABLE threads(
            id TEXT PRIMARY KEY,
            rollout_path TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            source TEXT NOT NULL,
            cwd TEXT NOT NULL,
            title TEXT NOT NULL,
            preview TEXT NOT NULL DEFAULT ''
        );",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO threads(id, rollout_path, created_at, updated_at, source, cwd, title, preview)
         VALUES('old-thread', ?1, ?2, ?2, 'codex', '/tmp', '旧计划', '')",
        params![old_rollout.display().to_string(), old_updated],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO threads(id, rollout_path, created_at, updated_at, source, cwd, title, preview)
         VALUES('fresh-thread', ?1, ?2, ?2, 'codex', '/tmp', '新计划', '')",
        params![fresh_rollout.display().to_string(), fresh_updated],
    )
    .unwrap();
    let mut config = state.config();
    config.codex.home = codex_home;
    state.replace_config(config);

    let rows = load_probe_threads(&state, "reply-needed", 20)
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "fresh-thread");
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn probe_events_route_lists_recent_events_with_auth_and_redacts_sensitive_payloads() {
    let (state, session_token, _) = authenticated_test_state();
    state
        .db
        .record_probe_event(nexushub_core::db::NewProbeEvent {
            kind: "hook-stop",
            thread_id: Some("thread-a"),
            title: Some("Hook event"),
            message: Some("stop hook received"),
            dedupe_key: Some("hook-stop:thread-a"),
            source: "test",
            payload: json!({
                "session_id": "session-a",
                "transcript_path": "/tmp/turn.json",
                "last_assistant_message": {
                    "summary": "hello",
                    "sha256": "abc123"
                },
                "body_summary": "<proposed_plan>\n# Safe summary\n</proposed_plan>",
                "body_sha256": "abc123",
                "body_length": 9876,
                "body_source": "last_assistant_message",
                "body_truncated": true,
                "device_key": "secret-device",
                "bark": {
                    "title": "等待回复：Hook event",
                    "chunk_count": 3,
                    "request_count": 3,
                    "sent": true,
                    "http_status": 200,
                    "request_url": "https://api.day.app/secret-device/title",
                    "device_key_configured": true,
                    "dedupe_key": "hook-stop:thread-a",
                    "body": "完整 Bark 正文不应返回"
                },
                "nested": {
                    "token": "abc",
                    "ok": true,
                    "headers": [{ "Authorization": "Bearer abc" }]
                }
            }),
        })
        .unwrap();
    let app = router(state);

    let unauthorized =
        request_rpc_status(app.clone(), "probe.events", r#"{"limit":1}"#, None, None).await;
    assert_eq!(unauthorized, StatusCode::UNAUTHORIZED);

    let payload =
        request_rpc_json(app, "probe.events", r#"{"limit":1}"#, &session_token, None).await;
    assert_eq!(payload["limit"], 1);
    assert_eq!(payload["events"].as_array().unwrap().len(), 1);
    let event = &payload["events"][0];
    assert_eq!(event["kind"], "hook-stop");
    assert_eq!(event["payload"]["device_key"], "[redacted]");
    assert_eq!(event["payload"]["bark"]["request_url"], "[redacted]");
    assert_eq!(event["payload"]["bark"]["device_key_configured"], true);
    assert_eq!(event["payload"]["bark"]["dedupe_key"], "hook-stop:thread-a");
    assert_eq!(event["payload"]["bark"]["title"], "等待回复：Hook event");
    assert_eq!(event["payload"]["bark"]["chunk_count"], 3);
    assert_eq!(event["payload"]["bark"]["request_count"], 3);
    assert!(event["payload"]["bark"].get("body").is_none());
    assert_eq!(event["payload"]["body_summary"], "# Safe summary");
    assert_eq!(event["payload"]["body_sha256"], "abc123");
    assert_eq!(event["payload"]["body_length"], 9876);
    assert_eq!(event["payload"]["body_source"], "last_assistant_message");
    assert_eq!(event["payload"]["body_truncated"], true);
    assert_eq!(event["payload"]["nested"]["token"], "[redacted]");
    assert_eq!(
        event["payload"]["nested"]["headers"][0]["Authorization"],
        "[redacted]"
    );
    assert_eq!(event["payload"]["session_id"], "session-a");
    assert_eq!(event["payload"]["transcript_path"], "/tmp/turn.json");
    assert_eq!(
        event["payload"]["last_assistant_message"]["summary"],
        "hello"
    );
    assert!(!serde_json::to_string(event)
        .unwrap()
        .contains("完整 Bark 正文不应返回"));
    assert!(!serde_json::to_string(event)
        .unwrap()
        .contains("<proposed_plan>"));
}

#[tokio::test]
async fn probe_settings_rejects_invalid_server_url_before_storing_device_key() {
    let (state, session_token, csrf_token) = authenticated_test_state();
    let dir = temp_test_dir("nexushub-probe-settings-invalid-url");
    fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join("config.toml");
    let mut config = Config::default();
    config.security.secret_key = state.config().security.secret_key.clone();
    config.paths.db_path = state.config().paths.db_path.clone();
    fs::write(&config_path, toml::to_string_pretty(&config).unwrap()).unwrap();
    let _config_env = ConfigEnvGuard::set(&config_path);
    let app = router(state.clone());

    let status = request_rpc_status(
        app,
        "probe.settings.save",
        r#"{"settings":{"notifications":{"server_url":"http://example.com","device_key":"secret-device"}}}"#,
        Some(&session_token),
        Some(&csrf_token),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(state
        .db
        .get_secret_setting_bytes("probe_bark_device_key")
        .unwrap()
        .is_none());
    assert!(!fs::read_to_string(probe_config_path())
        .unwrap()
        .contains("http://example.com"));
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn probe_alignment_routes_require_auth_and_expose_safe_probe_surfaces() {
    let (state, session_token, csrf_token) = authenticated_test_state();
    let app = router(state.clone());

    for (command, kind, title, forbidden_arg) in [
        (
            "probe.installHooks",
            "probe_hooks_install",
            "探针 Hook 安装",
            "codex-sentinel",
        ),
        (
            "probe.barkTest",
            "probe_bark_test",
            "探针 Bark 测试",
            "device_key",
        ),
        (
            "probe.logsDbDryRun",
            "probe_logs_db_maintain_dry_run",
            "Codex logs DB 维护 dry-run",
            "rm -rf",
        ),
    ] as [(&str, &str, &str, &str); 3]
    {
        let missing_csrf =
            request_rpc_status(app.clone(), command, "{}", Some(&session_token), None).await;
        assert_eq!(missing_csrf, StatusCode::FORBIDDEN, "{command}");

        let payload = request_rpc_json(
            app.clone(),
            command,
            "{}",
            &session_token,
            Some(&csrf_token),
        )
        .await;
        let job_id = payload["job_id"].as_str().unwrap();
        let job = state.db.job(job_id).unwrap().unwrap();
        assert_eq!(job.kind, kind);
        assert_eq!(job.title, title);
        assert!(!job.output.contains(forbidden_arg));
    }

    let (execute_state, execute_session_token, execute_csrf_token) = authenticated_test_state();
    let execute_app = router(execute_state.clone());
    let execute_payload = request_rpc_json(
        execute_app,
        "probe.logsDbExecute",
        "{}",
        &execute_session_token,
        Some(&execute_csrf_token),
    )
    .await;
    let execute_job_id = execute_payload["job_id"].as_str().unwrap();
    let execute_job = execute_state.db.job(execute_job_id).unwrap().unwrap();
    assert_eq!(execute_job.kind, "probe_logs_db_maintain");
    assert_eq!(execute_job.title, "Codex logs DB 维护");
    assert!(!execute_job.output.contains("--dry-run"));
}

#[tokio::test]
async fn probe_logs_db_status_reports_codex_logs_2_not_panel_probe_tables() {
    let (state, session_token, _) = authenticated_test_state();
    state
        .db
        .record_probe_event(nexushub_core::db::NewProbeEvent {
            kind: "hook-stop",
            thread_id: Some("thread-a"),
            title: Some("Hook event"),
            message: Some("stop hook received"),
            dedupe_key: Some("hook-stop:thread-a"),
            source: "test",
            payload: json!({"ok": true}),
        })
        .unwrap();
    state
        .db
        .claim_probe_dedupe("probe_event", "test-key", 300)
        .unwrap();
    state
        .db
        .set_setting(
            "probe_logs_db_last_maintain",
            &json!({"dry_run": false, "deleted_rows": 2, "target": "codex_logs_2"}).to_string(),
        )
        .unwrap();

    let dir = temp_test_dir("nexushub-api-codex-logs");
    let codex_home = dir.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    let logs_path = codex_home.join("logs_2.sqlite");
    let now = chrono::Utc::now().timestamp();
    seed_codex_logs_db(&logs_path, &[now - 300_000, now - 100]);
    let mut config = state.config();
    config.codex.home = codex_home.clone();
    config.probe.logs_db.retention_days = 2;
    state.replace_config(config);

    let logs_db = request_rpc_json(
        router(state.clone()),
        "probe.logsDb.status",
        "{}",
        &session_token,
        None,
    )
    .await;

    assert_eq!(logs_db["target"], "codex_logs_2");
    assert_eq!(logs_db["path"], logs_path.to_string_lossy().as_ref());
    assert_eq!(
        logs_db["configured_codex_home"],
        codex_home.to_string_lossy().as_ref()
    );
    assert_eq!(
        logs_db["resolved_codex_home"],
        codex_home.to_string_lossy().as_ref()
    );
    assert_eq!(logs_db["codex_home_source"], "configured");
    assert_eq!(logs_db["logs_db_source"], "configured");
    assert!(logs_db["discovery_warnings"].as_array().unwrap().is_empty());
    assert_eq!(logs_db["total_rows"], 2);
    assert_eq!(logs_db["old_rows"], 1);
    assert_eq!(logs_db["retained_rows"], 1);
    assert_eq!(logs_db["status"], "ok");
    assert_eq!(logs_db["logs_db_status"], "ok");
    assert!(logs_db.get("event_count").is_none());
    assert!(logs_db.get("dedupe_count").is_none());
    assert!(logs_db["last_run"].as_str().is_some());
    assert_eq!(logs_db["last_result"], "execute: deleted_rows=2");
    assert_eq!(logs_db["recent_result"], logs_db["last_result"]);
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn probe_settings_hook_status_and_logs_db_status_are_available() {
    let (state, session_token, csrf_token, _dir, config_path) =
        authenticated_test_state_with_config_file();
    let _config_env = ConfigEnvGuard::set(&config_path);
    let logs_dir = temp_test_dir("nexushub-settings-codex-logs");
    let codex_home = logs_dir.join(".codex");
    fs::create_dir_all(&codex_home).unwrap();
    let logs_path = codex_home.join("logs_2.sqlite");
    let now = chrono::Utc::now().timestamp();
    seed_codex_logs_db(&logs_path, &[now - 300_000, now - 100]);
    let mut config = state.config();
    config.codex.home = codex_home.clone();
    config.probe.logs_db.retention_days = 2;
    state.replace_config(config.clone());
    fs::write(&config_path, toml::to_string_pretty(&config).unwrap()).unwrap();
    state
        .db
        .record_probe_event(nexushub_core::db::NewProbeEvent {
            kind: "hook-stop",
            thread_id: Some("thread-a"),
            title: Some("Hook event"),
            message: Some("stop hook received"),
            dedupe_key: Some("hook-stop:thread-a"),
            source: "test",
            payload: json!({"ok": true}),
        })
        .unwrap();
    state
        .db
        .claim_probe_dedupe("probe_event", "test-key", 300)
        .unwrap();
    state
        .db
        .set_setting(
            "probe_logs_db_last_maintain",
            &json!({"dry_run": true, "target": "codex_logs_2", "would_delete_rows": 1}).to_string(),
        )
        .unwrap();
    let app = router(state.clone());

    let settings = request_rpc_json(
        app.clone(),
        "probe.settings.get",
        "{}",
        &session_token,
        None,
    )
    .await;
    assert_eq!(settings["probe"]["logs_db"]["retention_days"], 2);
    let probe_status =
        request_rpc_json(app.clone(), "probe.status", "{}", &session_token, None).await;
    assert!(probe_status["hook_status"].as_str().is_some());

    let logs_db = request_rpc_json(
        app.clone(),
        "probe.logsDb.status",
        "{}",
        &session_token,
        None,
    )
    .await;
    assert!(logs_db.get("event_count").is_none());
    assert!(logs_db.get("dedupe_count").is_none());
    assert_eq!(logs_db["target"], "codex_logs_2");
    assert_eq!(
        logs_db["configured_codex_home"],
        logs_path.parent().unwrap().to_string_lossy().as_ref()
    );
    assert_eq!(
        logs_db["resolved_codex_home"],
        logs_path.parent().unwrap().to_string_lossy().as_ref()
    );
    assert_eq!(logs_db["codex_home_source"], "configured");
    assert_eq!(logs_db["logs_db_source"], "configured");
    assert_eq!(logs_db["total_rows"], 2);
    assert_eq!(logs_db["old_rows"], 1);
    assert_eq!(logs_db["retained_rows"], 1);
    assert!(logs_db["last_maintain_at"]
        .as_str()
        .is_some_and(|value| !value.is_empty()));
    assert_eq!(logs_db["last_result"], "dry-run: would_delete_rows=1");

    let payload = request_rpc_json(
        app,
        "probe.settings.save",
        r#"{"probe":{"poll_seconds":20,"notifications":{"enabled":true,"device_key":"secret-bark-key"}}}"#,
        &session_token,
        Some(&csrf_token),
    )
    .await;
    let body = serde_json::to_vec(&payload).unwrap();
    assert_eq!(
        payload["codex"]["home"],
        logs_path.parent().unwrap().to_string_lossy().as_ref()
    );
    assert_eq!(
        payload["codex"]["resolved_codex_home"],
        logs_path.parent().unwrap().to_string_lossy().as_ref()
    );
    assert_eq!(payload["codex"]["codex_home_source"], "configured");
    assert_eq!(payload["codex"]["logs_db_source"], "configured");
    assert_eq!(payload["notifications"]["device_key_configured"], true);
    assert!(payload["notifications"].get("device_key").is_none());
    assert!(!body
        .windows(b"secret-bark-key".len())
        .any(|window| { window == b"secret-bark-key" }));
    assert_eq!(
        state
            .db
            .get_secret_setting_bytes("probe_bark_device_key")
            .unwrap()
            .unwrap(),
        b"secret-bark-key"
    );
}

#[tokio::test]
async fn probe_settings_patch_requires_csrf() {
    let (state, session_token, _csrf_token, _dir, config_path) =
        authenticated_test_state_with_config_file();
    let _config_env = ConfigEnvGuard::set(&config_path);
    let app = router(state);

    let status = request_rpc_status(
        app,
        "probe.settings.save",
        r#"{"probe":{"poll_seconds":25}}"#,
        Some(&session_token),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn probe_settings_patch_rejects_missing_config_file() {
    let (state, session_token, csrf_token) = authenticated_test_state();
    let dir = temp_test_dir("nexushub-missing-config");
    fs::create_dir_all(&dir).unwrap();
    let missing_path = dir.join("missing-config.toml");
    let _config_env = ConfigEnvGuard::set(&missing_path);
    let app = router(state);

    let status = request_rpc_status(
        app,
        "probe.settings.save",
        r#"{"probe":{"poll_seconds":25}}"#,
        Some(&session_token),
        Some(&csrf_token),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn probe_settings_patch_refreshes_runtime_config_snapshots() {
    let (state, session_token, csrf_token, _dir, config_path) =
        authenticated_test_state_with_config_file();
    let _config_env = ConfigEnvGuard::set(&config_path);
    let app = router(state.clone());

    let _ = request_rpc_json(
        app.clone(),
        "probe.settings.save",
        r#"{"codex":{"host_label":"fresh-host"},"probe":{"poll_seconds":33,"logs_db":{"retention_days":2}}}"#,
        &session_token,
        Some(&csrf_token),
    )
    .await;

    let settings = request_rpc_json(
        app.clone(),
        "probe.settings.get",
        "{}",
        &session_token,
        None,
    )
    .await;
    assert_eq!(settings["probe"]["poll_seconds"], 33);
    assert_eq!(settings["codex"]["host_label"], "fresh-host");

    let status = request_rpc_json(app.clone(), "probe.status", "{}", &session_token, None).await;
    assert_eq!(status["poll_seconds"], 33);
    assert_eq!(status["host_label"], "fresh-host");

    let logs_status = request_rpc_json(
        app.clone(),
        "probe.logsDb.status",
        "{}",
        &session_token,
        None,
    )
    .await;
    assert_eq!(logs_status["retention_days"], 2);

    let payload = request_rpc_json(
        app,
        "probe.barkTest",
        "{}",
        &session_token,
        Some(&csrf_token),
    )
    .await;
    let job_id = payload["job_id"].as_str().unwrap();
    let job = state.db.job(job_id).unwrap().unwrap();
    assert_eq!(job.kind, "probe_bark_test");
    let config_text = fs::read_to_string(&config_path).unwrap();
    assert!(config_text.contains("fresh-host"));
}

#[tokio::test]
async fn plugin_list_exposes_descriptions_and_unavailable_reasons_for_composer_mentions() {
    let (state, session_token, _csrf_token) = authenticated_test_state();
    let app = router(state);

    let plugins = request_rpc_json(app, "system.plugins", "{}", &session_token, None).await;
    let rows = plugins.as_array().unwrap();

    assert!(rows.iter().any(|plugin| {
        plugin["id"] == "codex"
            && plugin["description"].as_str().is_some_and(|value| {
                value.contains("Codex 本地状态") && !value.contains("app-server")
            })
    }));
    assert!(rows.iter().any(|plugin| {
        plugin["id"] == "probe"
            && plugin["description"]
                .as_str()
                .is_some_and(|value| value.contains("探针"))
    }));
    assert!(rows.iter().any(|plugin| {
        plugin["id"] == "claude_code"
            && plugin["unavailable_reason"]
                .as_str()
                .is_some_and(|value| value.contains("只读"))
    }));
}

#[test]
fn thread_block_page_returns_latest_window_without_detail_fields() {
    let detail = ThreadDetail {
        summary: fallback_summary("thread-a", "wanka"),
        messages: vec![],
        blocks: (0..6)
            .map(|index| MessageBlock {
                id: format!("block-{index}"),
                role: "assistant".to_string(),
                kind: "message".to_string(),
                display_kind: None,
                status: None,
                text: Some(format!("message-{index}")),
                summary: None,
                input: None,
                truncated: None,
                resolved: None,
                answers: Vec::new(),
                plan_status: None,
                group_id: None,
                tool_name: None,
                call_id: None,
                turn_id: None,
                item_id: None,
                created_at: None,
                questions: Vec::new(),
                payload: None,
            })
            .collect(),
        raw_event_count: 6,
        total_blocks: 6,
        has_more_blocks: false,
        before_cursor: None,
    };

    let plan = thread_service::plan_thread_blocks_request(
        &PlatformPaths::for_kind(PlatformKind::Linux),
        "thread-a",
        Some(2),
        None,
    )
    .unwrap();
    let page = thread_service::thread_blocks_page_for_plan(detail, &plan);
    let value = serde_json::to_value(&page).unwrap();

    assert_eq!(page.thread_id, "thread-a");
    assert_eq!(page.total_blocks, 6);
    assert!(page.has_more_blocks);
    assert_eq!(page.before_cursor.as_deref(), Some("b:4"));
    assert_eq!(page.blocks.len(), 2);
    assert_eq!(page.blocks[0].text.as_deref(), Some("message-4"));
    assert_eq!(page.blocks[1].text.as_deref(), Some("message-5"));
    assert!(value.get("summary").is_none());
    assert!(value.get("messages").is_none());
    assert!(value.get("raw_event_count").is_none());
}

#[test]
fn thread_block_page_uses_before_cursor() {
    let detail = ThreadDetail {
        summary: fallback_summary("thread-a", "wanka"),
        messages: vec![],
        blocks: (0..6)
            .map(|index| MessageBlock {
                id: format!("block-{index}"),
                role: "assistant".to_string(),
                kind: "message".to_string(),
                display_kind: None,
                status: None,
                text: Some(format!("message-{index}")),
                summary: None,
                input: None,
                truncated: None,
                resolved: None,
                answers: Vec::new(),
                plan_status: None,
                group_id: None,
                tool_name: None,
                call_id: None,
                turn_id: None,
                item_id: None,
                created_at: None,
                questions: Vec::new(),
                payload: None,
            })
            .collect(),
        raw_event_count: 6,
        total_blocks: 6,
        has_more_blocks: false,
        before_cursor: None,
    };

    let plan = thread_service::plan_thread_blocks_request(
        &PlatformPaths::for_kind(PlatformKind::Linux),
        "thread-a",
        Some(2),
        Some("b:4".to_string()),
    )
    .unwrap();
    let page = thread_service::thread_blocks_page_for_plan(detail, &plan);

    assert_eq!(page.total_blocks, 6);
    assert!(page.has_more_blocks);
    assert_eq!(page.before_cursor.as_deref(), Some("b:2"));
    assert_eq!(page.blocks.len(), 2);
    assert_eq!(page.blocks[0].text.as_deref(), Some("message-2"));
    assert_eq!(page.blocks[1].text.as_deref(), Some("message-3"));
}

#[test]
fn disabled_turnstile_without_required_skips_verification() {
    assert_eq!(
        turnstile_login_action(false, false),
        TurnstileLoginAction::Skip
    );
}

#[test]
fn enabled_turnstile_verifies_even_when_required_is_false() {
    assert_eq!(
        turnstile_login_action(true, false),
        TurnstileLoginAction::Verify
    );
    assert_eq!(
        turnstile_login_action(true, true),
        TurnstileLoginAction::Verify
    );
}

#[test]
fn required_turnstile_fails_closed_when_not_enabled() {
    assert_eq!(
        turnstile_login_action(false, true),
        TurnstileLoginAction::FailClosed
    );
}

#[test]
fn thread_title_reads_explicit_name_without_using_preview() {
    assert_eq!(
        thread_title(&json!({
            "name": "wanka",
            "preview": "接手这个线程的工作 019e86d2..."
        })),
        Some("wanka".to_string())
    );
    assert_eq!(
        thread_title(&json!({
            "preview": "接手这个线程的工作 019e86d2..."
        })),
        None
    );
}

#[test]
fn app_server_thread_summary_keeps_fallback_title_when_only_preview_exists() {
    let fallback = vec![fallback_summary("thread-a", "wanka")];
    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "preview": "接手这个线程的工作 019e86d2...",
                "updatedAt": 1780824385
            }]
        }),
        &fallback,
    );

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "wanka");
    assert_eq!(
        rows[0].latest_message.as_deref(),
        Some("接手这个线程的工作 019e86d2...")
    );
}

#[test]
fn app_server_thread_detail_does_not_overwrite_title_with_preview() {
    let mut detail = ThreadDetail {
        summary: fallback_summary("thread-a", "wanka"),
        messages: Vec::new(),
        blocks: Vec::new(),
        raw_event_count: 0,
        total_blocks: 0,
        has_more_blocks: false,
        before_cursor: None,
    };

    apply_app_server_thread_detail(
        &mut detail,
        &json!({
            "thread": {
                "id": "thread-a",
                "preview": "接手这个线程的工作 019e86d2...",
                "updatedAt": 1780824385,
                "cwd": "/home/ubuntu/codex-workspace"
            }
        }),
    );

    assert_eq!(detail.summary.title, "wanka");
    assert_eq!(
        detail.summary.cwd.as_deref(),
        Some("/home/ubuntu/codex-workspace")
    );
    assert_eq!(detail.summary.last_event_kind, None);
}

#[test]
fn app_server_thread_detail_does_not_overwrite_title_with_placeholder() {
    let mut detail = ThreadDetail {
        summary: fallback_summary("thread-a", "wanka"),
        messages: Vec::new(),
        blocks: Vec::new(),
        raw_event_count: 0,
        total_blocks: 0,
        has_more_blocks: false,
        before_cursor: None,
    };

    apply_app_server_thread_detail(
        &mut detail,
        &json!({
            "thread": {
                "id": "thread-a",
                "name": "未命名线程",
                "status": { "type": "active" }
            }
        }),
    );

    assert_eq!(detail.summary.title, "wanka");
    assert_eq!(detail.summary.status, ThreadStatus::Running);
}

#[test]
fn app_server_thread_detail_updates_title_with_real_name() {
    let mut detail = ThreadDetail {
        summary: fallback_summary("thread-a", "wanka"),
        messages: Vec::new(),
        blocks: Vec::new(),
        raw_event_count: 0,
        total_blocks: 0,
        has_more_blocks: false,
        before_cursor: None,
    };

    apply_app_server_thread_detail(
        &mut detail,
        &json!({
            "thread": {
                "id": "thread-a",
                "name": "wanka renamed"
            }
        }),
    );

    assert_eq!(detail.summary.title, "wanka renamed");
}

#[test]
fn app_server_active_status_overrides_stale_fallback_recent() {
    let fallback = vec![fallback_summary("thread-a", "wanka")];
    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "status": { "type": "active" }
            }]
        }),
        &fallback,
    );

    assert_eq!(rows[0].status, ThreadStatus::Running);
}

#[test]
fn app_server_recoverable_fallback_takes_priority_over_active_signal() {
    let mut fallback = fallback_summary("thread-a", "wanka");
    fallback.status = ThreadStatus::Recoverable;
    fallback.rollout_path = Some(PathBuf::from("/tmp/rollout-thread-a.jsonl"));
    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "status": { "type": "active" },
                "activeTurnId": "turn-live"
            }]
        }),
        &[fallback],
    );

    assert_eq!(rows[0].status, ThreadStatus::Recoverable);
    assert_eq!(rows[0].active_turn_id.as_deref(), Some("turn-live"));
}

#[test]
fn app_server_not_loaded_preserves_recoverable_fallback() {
    let mut fallback = fallback_summary("thread-a", "wanka");
    fallback.status = ThreadStatus::Recoverable;
    fallback.rollout_path = Some(PathBuf::from("/tmp/rollout-thread-a.jsonl"));
    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "status": { "type": "notLoaded" },
                "path": "/tmp/rollout-thread-a.jsonl"
            }]
        }),
        &[fallback],
    );

    assert_eq!(rows[0].status, ThreadStatus::Recoverable);
}

#[test]
fn app_server_thread_summaries_filter_subagent_rows_without_main_fallback() {
    let rows = app_server_thread_summaries(
        &json!({
            "threads": [
                {
                    "id": "main-thread",
                    "name": "主线程",
                    "sourceKind": "root"
                },
                {
                    "id": "child-thread",
                    "name": "worker",
                    "parentThreadId": "main-thread",
                    "sourceKind": "subAgentRun",
                    "agentNickname": "worker"
                },
                {
                    "id": "child-source",
                    "name": "explorer",
                    "source": { "kind": "subagent" },
                    "agentRole": "explorer"
                },
                {
                    "id": "child-nested-source",
                    "name": "nested",
                    "source": { "subagent": { "thread_spawn": { "parentThreadId": "main-thread" } } }
                }
            ]
        }),
        &[],
    );

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "main-thread");
}

#[test]
fn app_server_thread_summaries_keep_fallback_main_thread_even_when_app_row_looks_subagent() {
    let fallback = vec![fallback_summary("thread-a", "wanka")];
    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "parentThreadId": "parent",
                "sourceKind": "subAgentRun",
                "agentNickname": "worker"
            }]
        }),
        &fallback,
    );

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "thread-a");
    assert_eq!(rows[0].title, "wanka");
}

#[test]
fn app_server_threads_merge_preserves_local_only_fallback_rows() {
    let fallback = vec![
        fallback_summary("thread-a", "local fallback"),
        fallback_summary("thread-b", "shared fallback"),
    ];
    let app_threads = vec![fallback_summary("thread-b", "app-server")];

    let rows = merge_thread_summaries(fallback, app_threads);

    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|row| row.id == "thread-a"));
    assert_eq!(
        rows.iter()
            .find(|row| row.id == "thread-b")
            .map(|row| row.title.as_str()),
        Some("app-server")
    );
}

#[test]
fn app_server_threads_merge_keeps_fallback_title_when_app_row_has_placeholder_title() {
    let fallback = vec![fallback_summary("thread-a", "wanka")];
    let mut app_thread = fallback_summary("thread-a", "未命名线程");
    app_thread.status = ThreadStatus::Running;
    app_thread.latest_message = Some("正在执行".to_string());

    let rows = merge_thread_summaries(fallback, vec![app_thread]);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title, "wanka");
    assert_eq!(rows[0].status, ThreadStatus::Running);
    assert_eq!(rows[0].latest_message.as_deref(), Some("正在执行"));
}

#[test]
fn app_server_threads_merge_keeps_archived_fallback_out_of_default_list() {
    let mut archived = fallback_summary("thread-a", "archived");
    archived.status = ThreadStatus::Archived;
    archived.archived_at = Some("2026-06-16T00:00:00Z".to_string());
    let mut app_thread = fallback_summary("thread-a", "app-server");
    app_thread.status = ThreadStatus::Running;

    let rows = merge_thread_summaries(vec![archived], vec![app_thread]);
    let default_rows = thread_service::filter_thread_summaries(rows, None, None, 10);

    assert!(default_rows.is_empty());
}

#[test]
fn hidden_state_db_thread_ids_prune_app_server_rows_after_merge() {
    let fallback = vec![fallback_summary("main-thread", "wanka")];
    let app_threads = vec![
        fallback_summary("main-thread", "app main"),
        fallback_summary("child-thread", "subagent child"),
    ];
    let rows = merge_thread_summaries(fallback, app_threads);
    let hidden = ["child-thread".to_string()].into_iter().collect();

    let rows = thread_service::prune_hidden_thread_summaries(rows, &hidden);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "main-thread");
}

#[test]
fn filter_thread_summaries_excludes_archived_for_default_and_all_status() {
    let recent = fallback_summary("recent-thread", "recent");
    let mut archived = fallback_summary("archived-thread", "archived");
    archived.status = ThreadStatus::Archived;

    let default_rows = thread_service::filter_thread_summaries(
        vec![recent.clone(), archived.clone()],
        None,
        None,
        10,
    );
    let all_rows =
        thread_service::filter_thread_summaries(vec![recent, archived], Some("all"), None, 10);

    assert_eq!(default_rows.len(), 1);
    assert_eq!(default_rows[0].id, "recent-thread");
    assert_eq!(all_rows.len(), 1);
    assert_eq!(all_rows[0].id, "recent-thread");
    assert_eq!(archived_filter(None), Some(false));
    assert_eq!(archived_filter(Some("all")), Some(false));
}

#[test]
fn status_filtered_thread_lists_overfetch_before_final_limit() {
    assert_eq!(
        thread_service::thread_list_fetch_limit(None, Some(120)),
        120
    );
    assert_eq!(
        thread_service::thread_list_fetch_limit(Some("all"), Some(120)),
        120
    );
    assert_eq!(
        thread_service::thread_list_fetch_limit(Some("running"), Some(120)),
        usize::MAX
    );
    assert_eq!(
        thread_service::thread_list_fetch_limit(Some("reply-needed"), Some(120)),
        usize::MAX
    );
    assert_eq!(
        thread_service::thread_list_fetch_limit(Some("recoverable"), Some(120)),
        usize::MAX
    );
    assert_eq!(
        app_server_thread_list_fetch_limit(Some("running"), Some(120)),
        500
    );
    assert_eq!(
        app_server_thread_list_fetch_limit(Some("all"), Some(120)),
        120
    );
}

#[test]
fn running_job_marks_summary_running_with_active_job_id() {
    let mut summary = fallback_summary("thread-a", "wanka");
    let job = JobRecord {
        id: "job-live".to_string(),
        kind: "codex_chat".to_string(),
        status: "running".to_string(),
        title: "Codex resume thread".to_string(),
        thread_id: Some("thread-a".to_string()),
        turn_id: None,
        started_at: 1,
        finished_at: None,
        exit_code: None,
        output: String::new(),
        error: None,
    };

    thread_service::apply_running_job_to_summary(&mut summary, &job);

    assert_eq!(summary.status, ThreadStatus::Running);
    assert_eq!(summary.active_job_id.as_deref(), Some("job-live"));
    assert_eq!(summary.last_event_kind, None);
}

#[test]
fn running_job_does_not_inject_archived_thread_missing_from_limited_local_list() {
    let mut archived = fallback_summary("thread-archived", "old");
    archived.status = ThreadStatus::Archived;
    archived.archived_at = Some("2026-06-16T00:00:00Z".to_string());
    let job = JobRecord {
        id: "job-live".to_string(),
        kind: "codex_chat".to_string(),
        status: "running".to_string(),
        title: "Codex resume archived".to_string(),
        thread_id: Some("thread-archived".to_string()),
        turn_id: Some("turn-live".to_string()),
        started_at: 1,
        finished_at: None,
        exit_code: None,
        output: String::new(),
        error: None,
    };
    let mut rows = vec![fallback_summary("thread-visible", "visible")];

    let archived_ids = HashSet::from([archived.id.clone()]);
    thread_service::merge_running_jobs(&mut rows, &[job], &archived_ids);

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "thread-visible");
}

#[test]
fn app_server_idle_status_clears_stale_running_without_pending_signal() {
    let mut fallback = fallback_summary("thread-a", "wanka");
    fallback.status = ThreadStatus::Running;
    fallback.active_turn_id = Some("turn-old".to_string());
    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "status": { "type": "idle" }
            }]
        }),
        &[fallback],
    );

    assert_eq!(rows[0].status, ThreadStatus::Recent);
    assert_eq!(rows[0].active_turn_id, None);
}

#[test]
fn app_server_idle_status_preserves_reply_needed_with_current_pending_signal() {
    let mut fallback = fallback_summary("thread-a", "wanka");
    fallback.status = ThreadStatus::ReplyNeeded;
    fallback.active_turn_id = Some("turn-choice".to_string());
    fallback.pending_elicitation = Some(nexushub_core::codex::PendingElicitation {
        turn_id: Some("turn-choice".to_string()),
        item_id: Some("item-choice".to_string()),
        questions: Vec::new(),
    });
    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "status": { "type": "idle" }
            }]
        }),
        &[fallback],
    );

    assert_eq!(rows[0].status, ThreadStatus::ReplyNeeded);
}

#[test]
fn running_signal_takes_priority_over_stale_pending_signal_for_not_loaded() {
    let mut fallback = fallback_summary("thread-a", "wanka");
    fallback.status = ThreadStatus::ReplyNeeded;
    fallback.pending_elicitation = Some(nexushub_core::codex::PendingElicitation {
        turn_id: Some("turn-old".to_string()),
        item_id: Some("item-old".to_string()),
        questions: Vec::new(),
    });

    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "status": { "type": "notLoaded" },
                "turns": [{
                    "id": "turn-live",
                    "items": [{ "status": { "type": "running" } }]
                }]
            }]
        }),
        &[fallback],
    );

    assert_eq!(rows[0].status, ThreadStatus::Running);
    assert_eq!(rows[0].active_turn_id.as_deref(), Some("turn-live"));
    assert!(rows[0].pending_elicitation.is_none());
}

#[test]
fn app_server_not_loaded_uses_rollout_running_signal() {
    let root = unique_temp_dir("paneld-notloaded-running");
    fs::create_dir_all(&root).unwrap();
    let rollout = root.join("rollout-thread-a.jsonl");
    fs::write(
        &rollout,
        [
            json!({"type":"turn_started","turn_id":"turn-active"}).to_string(),
            json!({"type":"response_item","turn_id":"turn-active","payload":{"type":"function_call","name":"exec_command","call_id":"call-1","arguments":{"cmd":"sleep 10"}}}).to_string(),
        ].join("\n"),
    )
    .unwrap();

    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "name": "wanka",
                "status": { "type": "notLoaded" },
                "path": rollout.display().to_string()
            }]
        }),
        &[],
    );

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, ThreadStatus::Running);
    assert_eq!(rows[0].active_turn_id.as_deref(), Some("turn-active"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn app_server_not_loaded_uses_event_msg_task_started_rollout_signal() {
    let root = unique_temp_dir("paneld-notloaded-event-task-running");
    fs::create_dir_all(&root).unwrap();
    let rollout = root.join("rollout-thread-a.jsonl");
    fs::write(
        &rollout,
        [
            json!({"type":"event_msg","payload":{"type":"task_complete","last_agent_message":"done"}}).to_string(),
            json!({"type":"event_msg","payload":{"type":"task_started"}}).to_string(),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"continue"}]}}).to_string(),
        ]
        .join("\n"),
    )
    .unwrap();

    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "name": "wanka",
                "status": { "type": "notLoaded" },
                "path": rollout.display().to_string()
            }]
        }),
        &[],
    );

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, ThreadStatus::Running);
    assert!(rows[0].active_turn_id.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn app_server_not_loaded_preserves_rollout_running_after_prior_task_complete() {
    let root = unique_temp_dir("paneld-notloaded-prior-complete");
    fs::create_dir_all(&root).unwrap();
    let rollout = root.join("rollout-thread-a.jsonl");
    fs::write(
        &rollout,
        [
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-old"}}).to_string(),
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-live"}}).to_string(),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-old","last_agent_message":"done"}}).to_string(),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"still working"}]}}).to_string(),
        ]
        .join("\n"),
    )
    .unwrap();

    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "name": "xianbao",
                "status": { "type": "notLoaded" },
                "path": rollout.display().to_string()
            }]
        }),
        &[],
    );

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, ThreadStatus::Running);
    assert_eq!(rows[0].active_turn_id.as_deref(), Some("turn-live"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn app_server_not_loaded_completed_rollout_stays_recent() {
    let root = unique_temp_dir("paneld-notloaded-completed");
    fs::create_dir_all(&root).unwrap();
    let rollout = root.join("rollout-thread-a.jsonl");
    fs::write(
        &rollout,
        [
            json!({"type":"turn_started","turn_id":"turn-active"}).to_string(),
            json!({"type":"turn_completed","turn_id":"turn-active"}).to_string(),
        ]
        .join("\n"),
    )
    .unwrap();

    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "status": { "type": "notLoaded" },
                "path": rollout.display().to_string()
            }]
        }),
        &[],
    );

    assert_eq!(rows[0].status, ThreadStatus::Recent);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn app_server_not_loaded_completed_rollout_clears_stale_fallback_running_when_path_is_current() {
    let root = unique_temp_dir("paneld-notloaded-stale-running");
    fs::create_dir_all(&root).unwrap();
    let rollout = root.join("rollout-thread-a.jsonl");
    fs::write(
        &rollout,
        [
            json!({"type":"turn_started","turn_id":"turn-old"}).to_string(),
            json!({"type":"turn_completed","turn_id":"turn-old"}).to_string(),
        ]
        .join("\n"),
    )
    .unwrap();
    let mut fallback = fallback_summary("thread-a", "wanka");
    fallback.status = ThreadStatus::Running;
    fallback.active_turn_id = Some("turn-old".to_string());
    fallback.rollout_path = Some(rollout.clone());
    fallback.last_event_kind = Some("task_started".to_string());

    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "status": { "type": "notLoaded" },
                "path": rollout.display().to_string()
            }]
        }),
        &[fallback],
    );

    assert_eq!(rows[0].status, ThreadStatus::Recent);
    assert_eq!(rows[0].active_turn_id, None);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn app_server_not_loaded_ld_style_completed_rollout_clears_stale_running() {
    let root = unique_temp_dir("paneld-notloaded-ld-completed");
    fs::create_dir_all(&root).unwrap();
    let rollout = root.join("rollout-thread-a.jsonl");
    fs::write(
        &rollout,
        [
            json!({"type":"event_msg","payload":{"type":"task_started"}}).to_string(),
            json!({"type":"event_msg","payload":{"type":"task_complete","last_agent_message":null}}).to_string(),
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-stale"}}).to_string(),
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-latest"}}).to_string(),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-latest","last_agent_message":"done"}}).to_string(),
        ]
        .join("\n"),
    )
    .unwrap();
    let mut fallback = fallback_summary("thread-a", "LD");
    fallback.status = ThreadStatus::Running;
    fallback.active_turn_id = Some("turn-stale".to_string());
    fallback.rollout_path = Some(rollout.clone());
    fallback.last_event_kind = Some("task_started".to_string());

    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "name": "LD",
                "status": { "type": "notLoaded" },
                "path": rollout.display().to_string()
            }]
        }),
        &[fallback.clone()],
    );

    assert_eq!(rows[0].status, ThreadStatus::Recent);
    assert_eq!(rows[0].active_turn_id, None);

    let mut detail = ThreadDetail {
        summary: fallback,
        messages: Vec::new(),
        blocks: Vec::new(),
        raw_event_count: 0,
        total_blocks: 0,
        has_more_blocks: false,
        before_cursor: None,
    };
    apply_app_server_thread_detail(
        &mut detail,
        &json!({
            "thread": {
                "id": "thread-a",
                "name": "LD",
                "status": { "type": "notLoaded" },
                "path": rollout.display().to_string()
            }
        }),
    );

    assert_eq!(detail.summary.status, ThreadStatus::Recent);
    assert_eq!(detail.summary.active_turn_id, None);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn app_server_active_does_not_override_completed_rollout_wait_agent() {
    let root = unique_temp_dir("paneld-active-completed-wait-agent");
    fs::create_dir_all(&root).unwrap();
    let rollout = root.join("rollout-thread-a.jsonl");
    fs::write(
        &rollout,
        [
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-main"}}).to_string(),
            json!({"type":"response_item","turn_id":"turn-main","payload":{"type":"function_call","name":"wait_agent","call_id":"wait-agent-1","arguments":{"targets":["agent-1"]}}}).to_string(),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-main","last_agent_message":"主线程完成。"}}).to_string(),
        ]
        .join("\n"),
    )
    .unwrap();
    let mut fallback = fallback_summary("thread-a", "wanka");
    fallback.rollout_path = Some(rollout.clone());

    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "name": "wanka",
                "status": { "type": "active" },
                "activeTurnId": "turn-main",
                "path": rollout.display().to_string()
            }]
        }),
        &[fallback.clone()],
    );
    let running = thread_service::filter_thread_summaries(rows.clone(), Some("running"), None, 50);

    assert_eq!(rows[0].status, ThreadStatus::Recent);
    assert_eq!(rows[0].active_turn_id, None);
    assert!(running.is_empty());

    let mut detail = ThreadDetail {
        summary: fallback,
        messages: Vec::new(),
        blocks: Vec::new(),
        raw_event_count: 0,
        total_blocks: 0,
        has_more_blocks: false,
        before_cursor: None,
    };
    apply_app_server_thread_detail(
        &mut detail,
        &json!({
            "thread": {
                "id": "thread-a",
                "name": "wanka",
                "status": { "type": "active" },
                "activeTurnId": "turn-main",
                "path": rollout.display().to_string()
            }
        }),
    );

    assert_eq!(detail.summary.status, ThreadStatus::Recent);
    assert_eq!(detail.summary.active_turn_id, None);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn app_server_not_loaded_running_item_does_not_override_completed_rollout_wait_agent() {
    let root = unique_temp_dir("paneld-notloaded-completed-wait-agent");
    fs::create_dir_all(&root).unwrap();
    let rollout = root.join("rollout-thread-a.jsonl");
    fs::write(
        &rollout,
        [
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-main"}}).to_string(),
            json!({"type":"response_item","turn_id":"turn-main","payload":{"type":"function_call","name":"wait_agent","call_id":"wait-agent-1","arguments":{"targets":["agent-1"]}}}).to_string(),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-main","last_agent_message":"主线程完成。"}}).to_string(),
        ]
        .join("\n"),
    )
    .unwrap();
    let mut fallback = fallback_summary("thread-a", "wanka");
    fallback.rollout_path = Some(rollout.clone());

    let app_thread = json!({
        "id": "thread-a",
        "name": "wanka",
        "status": { "type": "notLoaded" },
        "path": rollout.display().to_string(),
        "turns": [{
            "id": "turn-main",
            "items": [{ "type": "function_call", "name": "wait_agent", "status": { "type": "running" } }]
        }]
    });
    let rows = app_server_thread_summaries(
        &json!({ "threads": [app_thread.clone()] }),
        &[fallback.clone()],
    );
    let running = thread_service::filter_thread_summaries(rows.clone(), Some("running"), None, 50);

    assert_eq!(rows[0].status, ThreadStatus::Recent);
    assert_eq!(rows[0].active_turn_id, None);
    assert!(running.is_empty());

    let mut detail = ThreadDetail {
        summary: fallback,
        messages: Vec::new(),
        blocks: Vec::new(),
        raw_event_count: 0,
        total_blocks: 0,
        has_more_blocks: false,
        before_cursor: None,
    };
    apply_app_server_thread_detail(&mut detail, &json!({ "thread": app_thread }));

    assert_eq!(detail.summary.status, ThreadStatus::Recent);
    assert_eq!(detail.summary.active_turn_id, None);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn app_server_newer_active_turn_overrides_completed_rollout_wait_agent() {
    let root = unique_temp_dir("paneld-newer-active-after-completed-wait-agent");
    fs::create_dir_all(&root).unwrap();
    let rollout = root.join("rollout-thread-a.jsonl");
    fs::write(
        &rollout,
        [
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-main"}}).to_string(),
            json!({"type":"response_item","turn_id":"turn-main","payload":{"type":"function_call","name":"wait_agent","call_id":"wait-agent-1","arguments":{"targets":["agent-1"]}}}).to_string(),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-main","last_agent_message":"主线程完成。"}}).to_string(),
        ]
        .join("\n"),
    )
    .unwrap();
    let mut fallback = fallback_summary("thread-a", "wanka");
    fallback.rollout_path = Some(rollout.clone());

    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "name": "wanka",
                "status": { "type": "active" },
                "activeTurnId": "turn-new",
                "path": rollout.display().to_string()
            }]
        }),
        &[fallback],
    );

    assert_eq!(rows[0].status, ThreadStatus::Running);
    assert_eq!(rows[0].active_turn_id.as_deref(), Some("turn-new"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn app_server_not_loaded_without_rollout_path_clears_stale_local_active_turn() {
    let mut fallback = fallback_summary("thread-a", "wanka");
    fallback.status = ThreadStatus::Running;
    fallback.active_turn_id = Some("turn-live".to_string());
    fallback.last_event_kind = Some("task_started".to_string());

    let rows = app_server_thread_summaries(
        &json!({
            "threads": [{
                "id": "thread-a",
                "status": { "type": "notLoaded" }
            }]
        }),
        &[fallback],
    );

    assert_eq!(rows[0].status, ThreadStatus::Recent);
    assert_eq!(rows[0].active_turn_id, None);
    assert_eq!(rows[0].last_event_kind.as_deref(), Some("task_started"));
}

#[test]
fn app_server_thread_detail_not_loaded_clears_stale_local_active_turn() {
    let mut summary = fallback_summary("thread-a", "wanka");
    summary.status = ThreadStatus::Running;
    summary.active_turn_id = Some("turn-live".to_string());
    summary.last_event_kind = Some("task_started".to_string());
    let mut detail = ThreadDetail {
        summary,
        messages: Vec::new(),
        blocks: Vec::new(),
        raw_event_count: 0,
        total_blocks: 0,
        has_more_blocks: false,
        before_cursor: None,
    };

    apply_app_server_thread_detail(
        &mut detail,
        &json!({
            "thread": {
                "id": "thread-a",
                "status": { "type": "notLoaded" }
            }
        }),
    );

    assert_eq!(detail.summary.status, ThreadStatus::Recent);
    assert_eq!(detail.summary.active_turn_id, None);
    assert_eq!(
        detail.summary.last_event_kind.as_deref(),
        Some("task_started")
    );
}

#[test]
fn app_server_thread_read_turns_provide_running_status_and_active_turn() {
    let mut detail = ThreadDetail {
        summary: fallback_summary("thread-a", "wanka"),
        messages: Vec::new(),
        blocks: Vec::new(),
        raw_event_count: 0,
        total_blocks: 0,
        has_more_blocks: false,
        before_cursor: None,
    };

    apply_app_server_thread_detail(
        &mut detail,
        &json!({
            "thread": {
                "id": "thread-a",
                "status": { "type": "notLoaded" },
                "turns": [{
                    "id": "turn-live",
                    "items": [{ "status": { "type": "running" } }]
                }]
            }
        }),
    );

    assert_eq!(detail.summary.status, ThreadStatus::Running);
    assert_eq!(detail.summary.active_turn_id.as_deref(), Some("turn-live"));
}

#[test]
fn app_server_detail_from_read_builds_detail_from_rollout_path_without_fallback_db_row() {
    let root = unique_temp_dir("paneld-detail-read");
    fs::create_dir_all(&root).unwrap();
    let rollout = root.join("rollout-thread-a.jsonl");
    fs::write(
        &rollout,
        json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"hello from rollout"}]}}).to_string(),
    )
    .unwrap();

    let detail = app_server_detail_from_read(&json!({
        "thread": {
            "id": "thread-a",
            "name": "wanka",
            "status": { "type": "notLoaded" },
            "path": rollout.display().to_string()
        }
    }))
    .unwrap();

    assert_eq!(detail.summary.id, "thread-a");
    assert_eq!(detail.summary.title, "wanka");
    assert_eq!(detail.blocks.len(), 1);
    assert_eq!(detail.blocks[0].text.as_deref(), Some("hello from rollout"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn app_server_detail_from_read_builds_blocks_from_turn_items_without_rollout_path() {
    let detail = app_server_detail_from_read(&json!({
        "thread": {
            "id": "thread-a",
            "name": "wanka",
            "status": { "type": "notLoaded" },
            "turns": [{
                "id": "turn-plan",
                "items": [
                    {
                        "type": "userMessage",
                        "id": "user-1",
                        "content": [{ "type": "input_text", "text": "帮我修复" }]
                    },
                    {
                        "type": "userMessage",
                        "id": "subagent-context",
                        "content": [{ "type": "input_text", "text": "<subagent_notification>{\"agent_path\":\"/tmp/child\"}</subagent_notification>" }]
                    },
                    {
                        "type": "agentMessage",
                        "id": "agent-1",
                        "text": "我先检查。"
                    },
                    {
                        "type": "plan",
                        "id": "plan-1",
                        "text": "<proposed_plan>\n# Summary\n- Fix it\n</proposed_plan>"
                    },
                    {
                        "type": "requestUserInput",
                        "id": "choice-1",
                        "turnId": "turn-plan",
                        "questions": [{
                            "id": "q1",
                            "header": "选择",
                            "question": "选择方案",
                            "options": [{ "label": "A", "description": "执行 A" }]
                        }]
                    }
                ]
            }]
        }
    }))
    .unwrap();

    assert_eq!(detail.summary.status, ThreadStatus::ReplyNeeded);
    assert_eq!(detail.summary.active_turn_id.as_deref(), Some("turn-plan"));
    assert_eq!(detail.blocks.len(), 4);
    assert_eq!(detail.blocks[0].role, "user");
    assert_eq!(detail.blocks[0].text.as_deref(), Some("帮我修复"));
    assert_eq!(detail.blocks[1].role, "assistant");
    assert_eq!(detail.blocks[1].text.as_deref(), Some("我先检查。"));
    assert_eq!(detail.blocks[2].kind, "plan");
    assert_eq!(detail.blocks[2].item_id.as_deref(), Some("plan-1"));
    assert_eq!(detail.blocks[3].kind, "request_user_input");
    assert_eq!(detail.blocks[3].call_id.as_deref(), Some("choice-1"));
    assert_eq!(detail.blocks[3].questions[0].question, "选择方案");
    assert_eq!(detail.total_blocks, detail.blocks.len());
}

#[test]
fn app_server_thread_detail_clears_stale_reply_needed_when_idle() {
    let mut summary = fallback_summary("thread-a", "wanka");
    summary.status = ThreadStatus::ReplyNeeded;
    let mut detail = ThreadDetail {
        summary,
        messages: Vec::new(),
        blocks: Vec::new(),
        raw_event_count: 0,
        total_blocks: 0,
        has_more_blocks: false,
        before_cursor: None,
    };

    apply_app_server_thread_detail(
        &mut detail,
        &json!({
            "thread": {
                "id": "thread-a",
                "status": { "type": "idle" }
            }
        }),
    );

    assert_eq!(detail.summary.status, ThreadStatus::Recent);
}

#[test]
fn app_server_thread_detail_ignores_historical_pending_blocks_without_active_turn() {
    let mut detail = ThreadDetail {
        summary: fallback_summary("thread-a", "wanka"),
        messages: Vec::new(),
        blocks: vec![
            MessageBlock {
                id: "choice-old".to_string(),
                role: "assistant".to_string(),
                kind: "request_user_input".to_string(),
                display_kind: Some("question".to_string()),
                status: Some("pending".to_string()),
                text: None,
                summary: None,
                input: None,
                truncated: None,
                resolved: Some(false),
                answers: Vec::new(),
                plan_status: None,
                group_id: Some("call-old".to_string()),
                tool_name: None,
                call_id: Some("call-old".to_string()),
                turn_id: Some("turn-old".to_string()),
                item_id: Some("item-old".to_string()),
                created_at: None,
                questions: vec![nexushub_core::codex::UserInputQuestion {
                    id: "q1".to_string(),
                    header: None,
                    question: "旧选择".to_string(),
                    options: Vec::new(),
                }],
                payload: None,
            },
            MessageBlock {
                id: "assistant-later".to_string(),
                role: "assistant".to_string(),
                kind: "message".to_string(),
                display_kind: None,
                status: None,
                text: Some("已经继续执行。".to_string()),
                summary: None,
                input: None,
                truncated: Some(false),
                resolved: None,
                answers: Vec::new(),
                plan_status: None,
                group_id: Some("turn-new".to_string()),
                tool_name: None,
                call_id: None,
                turn_id: Some("turn-new".to_string()),
                item_id: None,
                created_at: None,
                questions: Vec::new(),
                payload: None,
            },
        ],
        raw_event_count: 2,
        total_blocks: 2,
        has_more_blocks: false,
        before_cursor: None,
    };

    apply_app_server_thread_detail(
        &mut detail,
        &json!({
            "thread": {
                "id": "thread-a",
                "status": { "type": "notLoaded" }
            }
        }),
    );

    assert_eq!(detail.summary.status, ThreadStatus::Recent);
    assert!(detail.summary.pending_elicitation.is_none());
    assert!(detail.summary.active_turn_id.is_none());
}

#[test]
fn app_server_status_derivation_is_shared_for_list_detail_and_probe_buckets() {
    let mut fallback = fallback_summary("thread-a", "wanka");
    fallback.status = ThreadStatus::ReplyNeeded;
    fallback.active_turn_id = Some("turn-choice".to_string());
    fallback.pending_elicitation = Some(nexushub_core::codex::PendingElicitation {
        turn_id: Some("turn-choice".to_string()),
        item_id: Some("item-choice".to_string()),
        questions: Vec::new(),
    });
    let app_value = json!({
        "threads": [{
            "id": "thread-a",
            "name": "wanka",
            "status": { "type": "idle" }
        }]
    });
    let rows = app_server_thread_summaries(&app_value, &[fallback.clone()]);
    let mut detail = ThreadDetail {
        summary: fallback,
        messages: Vec::new(),
        blocks: Vec::new(),
        raw_event_count: 0,
        total_blocks: 0,
        has_more_blocks: false,
        before_cursor: None,
    };
    apply_app_server_thread_detail(
        &mut detail,
        &json!({
            "thread": {
                "id": "thread-a",
                "name": "wanka",
                "status": { "type": "idle" }
            }
        }),
    );
    let reply_needed =
        thread_service::filter_thread_summaries(rows.clone(), Some("reply-needed"), None, 50);
    let running = thread_service::filter_thread_summaries(rows.clone(), Some("running"), None, 50);

    assert_eq!(rows[0].status, ThreadStatus::ReplyNeeded);
    assert_eq!(detail.summary.status, ThreadStatus::ReplyNeeded);
    assert_eq!(reply_needed.len(), 1);
    assert!(running.is_empty());
}

#[test]
fn thread_event_block_key_changes_when_same_block_content_changes() {
    let mut block = MessageBlock {
        id: "tool-1".to_string(),
        role: "tool".to_string(),
        kind: "function_call".to_string(),
        display_kind: Some("tool".to_string()),
        status: Some("running".to_string()),
        text: None,
        summary: Some("pwd".to_string()),
        input: Some("{\"cmd\":\"pwd\"}".to_string()),
        truncated: Some(false),
        resolved: Some(false),
        answers: Vec::new(),
        plan_status: None,
        group_id: Some("call-1".to_string()),
        tool_name: Some("exec_command".to_string()),
        call_id: Some("call-1".to_string()),
        turn_id: Some("turn-1".to_string()),
        item_id: None,
        created_at: None,
        questions: Vec::new(),
        payload: None,
    };
    let before = thread_event_block_key(&block);
    block.status = Some("completed".to_string());
    block.text = Some("/tmp".to_string());
    let after = thread_event_block_key(&block);

    assert_ne!(before, after);
    assert!(block_changed(None, &block));
    assert!(!block_changed(Some(&after), &block));
}

#[test]
fn seeded_thread_event_blocks_do_not_emit_initial_history_but_emit_changes() {
    let mut block = MessageBlock {
        id: "tool-1".to_string(),
        role: "tool".to_string(),
        kind: "function_call".to_string(),
        display_kind: Some("tool".to_string()),
        status: Some("running".to_string()),
        text: None,
        summary: Some("pwd".to_string()),
        input: Some("{\"cmd\":\"pwd\"}".to_string()),
        truncated: Some(false),
        resolved: Some(false),
        answers: Vec::new(),
        plan_status: None,
        group_id: Some("call-1".to_string()),
        tool_name: Some("exec_command".to_string()),
        call_id: Some("call-1".to_string()),
        turn_id: Some("turn-1".to_string()),
        item_id: None,
        created_at: None,
        questions: Vec::new(),
        payload: None,
    };
    let mut sent = HashMap::new();
    seed_thread_event_blocks(&mut sent, &[block.clone()]);

    assert!(!block_changed(sent.get("tool-1"), &block));
    block.status = Some("completed".to_string());
    block.text = Some("/tmp".to_string());
    assert!(block_changed(sent.get("tool-1"), &block));
}

fn unique_temp_dir(label: &str) -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!(
        "nexushub-{label}-{}-{}-{}",
        std::process::id(),
        counter,
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ))
}
