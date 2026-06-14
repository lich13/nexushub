use crate::{
    auth::{
        expired_session_cookie, expires_at, random_token, require_auth, require_csrf,
        session_cookie, verify_password, SESSION_COOKIE,
    },
    state::{AppState, CachedThreadDetail, FileSignature, ThreadDetailCacheSignature},
    turnstile::verify_turnstile,
};
use axum::{
    extract::connect_info::ConnectInfo,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{delete, get, post},
    Json, Router,
};
use nexushub_core::{
    app_server::{BridgeActionResult, BridgeTurnOptions},
    archive,
    claude_code::{self, ClaudePaths},
    codex::{self, CodexPaths, MessageBlock, ThreadDetail, ThreadStatus, ThreadSummary},
    db::{JobRecord, NewSession, ThreadFollowUp},
    platform::PlatformPaths,
    providers::ProviderRegistry,
    sentinel::{self, SentinelConfig},
    update,
    uploads::{
        self, cleanup_upload_ids, prepare_uploads, prompt_with_attachment_context,
        PreparedAttachment, MAX_TOTAL_UPLOAD_BYTES, MAX_UPLOAD_FILES, MAX_UPLOAD_FILE_BYTES,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    net::SocketAddr,
    path::Path as FsPath,
    time::{Duration, UNIX_EPOCH},
};
use uuid::Uuid;

type ApiResponse = Result<Response, ApiError>;
const THREAD_DETAIL_DEFAULT_BLOCK_LIMIT: usize = 120;
const THREAD_DETAIL_MAX_BLOCK_LIMIT: usize = 500;
const THREAD_EVENT_BLOCK_WINDOW: usize = 160;

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

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/api/public/settings", get(public_settings))
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/me", get(me))
        .route("/api/security", get(get_security).patch(patch_security))
        .route("/api/security/password", post(change_password))
        .route("/api/providers", get(list_providers))
        .route(
            "/api/providers/claude-code/overview",
            get(claude_code_overview),
        )
        .route("/api/platform", get(platform_overview))
        .route("/api/plugins", get(list_plugins))
        .route("/api/probe/status", get(get_probe_status))
        .route("/api/probe/dashboard", get(get_probe_dashboard))
        .route("/api/probe/running", get(get_probe_running))
        .route("/api/probe/reply-needed", get(get_probe_reply_needed))
        .route("/api/probe/recoverable", get(get_probe_recoverable))
        .route("/api/probe/thread-probe/:id", get(get_probe_thread_probe))
        .route("/api/probe/hook-status", get(get_probe_hook_status))
        .route("/api/probe/logs-db/status", get(get_probe_logs_db_status))
        .route("/api/probe/hooks/install", post(probe_hooks_install))
        .route("/api/probe/bark/test", post(probe_bark_test))
        .route("/api/probe/logs-db/maintain", post(probe_logs_db_maintain))
        .route("/api/sentinel/status", get(get_probe_status))
        .route(
            "/api/providers/claude-code/jobs/version-check",
            post(claude_code_version_check),
        )
        .route(
            "/api/providers/claude-code/jobs/update/precheck",
            post(claude_code_update_precheck),
        )
        .route(
            "/api/providers/claude-code/jobs/update/start",
            post(claude_code_update_start),
        )
        .route(
            "/api/providers/claude-code/jobs/smoke",
            post(claude_code_smoke),
        )
        .route(
            "/api/providers/claude-code/jobs/cache-status",
            post(claude_code_cache_status),
        )
        .route("/api/threads", get(list_threads).post(create_thread))
        .route("/api/threads/:id", get(thread_detail))
        .route("/api/threads/:id/blocks", get(thread_blocks))
        .route("/api/threads/:id/messages", post(send_message))
        .route("/api/threads/:id/steer", post(steer_thread))
        .route(
            "/api/threads/:id/follow-ups",
            get(list_followups).post(enqueue_followup),
        )
        .route(
            "/api/threads/:id/follow-ups/:followup_id/cancel",
            post(cancel_followup),
        )
        .route("/api/threads/:id/stop", post(stop_thread))
        .route("/api/threads/:id/archive", post(archive_thread))
        .route("/api/threads/:id/restore", post(restore_thread))
        .route("/api/threads/:id/rename", post(rename_thread))
        .route("/api/threads/:id/fork", post(fork_thread))
        .route("/api/threads/:id/plan/accept", post(plan_accept))
        .route("/api/threads/:id/plan/revise", post(plan_revise))
        .route("/api/threads/:id/elicitation", post(answer_elicitation))
        .route("/api/threads/:id/approval", post(answer_approval))
        .route("/api/threads/:id/events", get(thread_events))
        .route(
            "/api/uploads",
            post(upload_files).layer(DefaultBodyLimit::max(MAX_TOTAL_UPLOAD_BYTES + 1024 * 1024)),
        )
        .route("/api/uploads/:id", delete(delete_upload_file))
        .route("/api/system/status", get(system_status))
        .route("/api/system/version", get(system_version))
        .route(
            "/api/system/panel/update/precheck",
            post(panel_update_precheck),
        )
        .route("/api/system/panel/update/start", post(panel_update_start))
        .route("/api/system/panel/update/prune", post(panel_update_prune))
        .route(
            "/api/system/codex/update/precheck",
            post(codex_update_precheck),
        )
        .route("/api/system/codex/update/start", post(codex_update_start))
        .route("/api/system/codex/update/prune", post(codex_update_prune))
        .route("/api/system/update/precheck", post(codex_update_precheck))
        .route("/api/system/update/start", post(codex_update_start))
        .route("/api/system/update/prune", post(codex_update_prune))
        .route("/api/codex/models", get(codex_models))
        .route(
            "/api/codex/permission-profiles",
            get(codex_permission_profiles),
        )
        .route(
            "/api/codex/permissionProfiles",
            get(codex_permission_profiles),
        )
        .route("/api/codex/config", get(codex_config))
        .route("/api/codex/goal", get(codex_goal_get).post(codex_goal_set))
        .route("/api/codex/goal/clear", post(codex_goal_clear))
        .route("/api/archives/delete/dry-run", post(archive_delete_dry_run))
        .route("/api/archives/delete/execute", post(archive_delete_execute))
        .route(
            "/api/hidden-threads/delete/dry-run",
            post(hidden_threads_delete_dry_run),
        )
        .route(
            "/api/hidden-threads/delete/execute",
            post(hidden_threads_delete_execute),
        )
        .route("/api/jobs", get(list_jobs))
        .route("/api/jobs/:id", get(job_detail))
        .route("/api/jobs/:id/events", get(job_events))
        .with_state(state)
}

async fn healthz() -> ApiResponse {
    ok(json!({"ok": true}))
}

async fn public_settings(State(state): State<AppState>) -> ApiResponse {
    let security = state
        .db
        .security_settings(state.config.security.session_ttl_seconds)?;
    let turnstile_action = state
        .db
        .get_setting("turnstile_expected_action")?
        .or_else(|| state.config.security.turnstile_expected_action.clone())
        .unwrap_or_else(|| "login".to_string());
    ok(json!({
        "site_name": "NexusHub",
        "turnstile_enabled": security.turnstile_enabled,
        "turnstile_required": security.turnstile_required,
        "turnstile_site_key": security.turnstile_site_key.unwrap_or_else(|| nexushub_core::config::DEFAULT_TURNSTILE_SITE_KEY.to_string()),
        "turnstile_action": turnstile_action,
        "admin_configured": state.db.admin_count()? > 0,
        "base_url": state.config.server.public_base_url,
    }))
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
    ok(PlatformPaths::current())
}

async fn list_plugins(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(json!([
        {"id": "codex", "label": "Codex", "status": "ready", "kind": "builtin"},
        {"id": "probe", "label": "Probe", "status": "preview", "kind": "builtin"},
        {"id": "claude_code", "label": "Claude Code", "status": "preview", "kind": "builtin"},
        {"id": "system_ops", "label": "System / Ops", "status": "ready", "kind": "builtin"}
    ]))
}

async fn get_probe_status(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(probe_status_value().await)
}

#[derive(Debug, Deserialize)]
struct ProbeListQuery {
    limit: Option<usize>,
}

async fn get_probe_dashboard(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let status = probe_status_value().await;
    let running = load_probe_threads(&state, "running", 50).await?;
    let reply_needed = load_probe_threads(&state, "reply-needed", 50).await?;
    let recoverable = load_probe_threads(&state, "recoverable", 50).await?;
    let doctor_args = [probe_doctor_command()];
    let (hook_status, logs_db_status, doctor) = tokio::join!(
        probe_json_command(&["hook-status"]),
        probe_json_command(&["logs-db-status"]),
        probe_json_command(&doctor_args)
    );
    ok(json!({
        "status": status,
        "running": running,
        "reply_needed": reply_needed,
        "recoverable": recoverable,
        "recent_events": [],
        "diagnostics": {
            "doctor": doctor,
            "hook_status": hook_status,
            "logs_db_status": logs_db_status,
        }
    }))
}

async fn get_probe_running(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ProbeListQuery>,
) -> ApiResponse {
    probe_thread_list_response(state, headers, "running", query.limit).await
}

async fn get_probe_reply_needed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ProbeListQuery>,
) -> ApiResponse {
    probe_thread_list_response(state, headers, "reply-needed", query.limit).await
}

async fn get_probe_recoverable(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ProbeListQuery>,
) -> ApiResponse {
    probe_thread_list_response(state, headers, "recoverable", query.limit).await
}

async fn probe_thread_list_response(
    state: AppState,
    headers: HeaderMap,
    status: &'static str,
    limit: Option<usize>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let limit = limit.unwrap_or(50).clamp(1, 200);
    let cli = probe_json_command(&[status, &limit.to_string()]).await;
    let threads = load_probe_threads(&state, status, limit).await?;
    ok(json!({
        "source": "nexushub_read_model",
        "items": threads,
        "probe_cli": cli,
    }))
}

async fn get_probe_thread_probe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let detail = load_merged_thread_detail(&state, &id, "probe thread")
        .await
        .map_err(api_error_for_thread_detail_load)?;
    let cli = probe_json_command(&["debug-app-server-thread", &id]).await;
    match detail {
        Some(detail) => ok(json!({
            "thread_id": id,
            "summary": detail.summary,
            "total_blocks": detail.total_blocks,
            "raw_event_count": detail.raw_event_count,
            "probe_cli": cli,
        })),
        None => Err(api_error(StatusCode::NOT_FOUND, "thread not found")),
    }
}

async fn get_probe_hook_status(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(probe_json_command(&["hook-status"]).await)
}

async fn get_probe_logs_db_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(probe_json_command(&["logs-db-status"]).await)
}

async fn probe_hooks_install(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    start_probe_fixed_job(
        state,
        headers,
        "probe_hooks_install",
        "Probe install hooks",
        probe_hooks_install_args(),
        "probe_hooks",
    )
    .await
}

async fn probe_bark_test(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    start_probe_fixed_job(
        state,
        headers,
        "probe_bark_test",
        "Probe Bark test",
        vec!["test-bark".to_string()],
        "probe_bark",
    )
    .await
}

async fn probe_logs_db_maintain(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    start_probe_fixed_job(
        state,
        headers,
        "probe_logs_db_maintain",
        "Probe logs DB maintain",
        vec!["logs-db-maintain".to_string(), "--dry-run".to_string()],
        "probe_logs_db",
    )
    .await
}

async fn start_probe_fixed_job(
    state: AppState,
    headers: HeaderMap,
    kind: &str,
    title: &str,
    args: Vec<String>,
    group: &str,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    state.db.record_audit(
        Some(&auth.admin_id),
        &format!("{kind}.started"),
        Some("probe"),
        Some(title),
        None,
        json!({"args": args}),
    )?;
    let command = fixed_probe_shell_command(&args);
    let id = state
        .jobs
        .start_exclusive_shell_job(kind, title, command, group)
        .map_err(|err| api_error(StatusCode::CONFLICT, &err.to_string()))?;
    ok(json!({"job_id": id}))
}

async fn claude_code_version_check(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResponse {
    start_claude_code_fixed_job(
        state,
        headers,
        "claude_code_version_check",
        "Claude Code version check",
        "command -v claude && claude --version",
        "claude_code_version_check",
    )
    .await
}

async fn claude_code_update_precheck(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResponse {
    start_claude_code_fixed_job(
        state,
        headers,
        "claude_code_update_precheck",
        "Claude Code update precheck",
        "command -v claude && claude --version && npm view @anthropic-ai/claude-code version",
        "claude_code_update_precheck",
    )
    .await
}

async fn claude_code_update_start(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResponse {
    start_claude_code_fixed_job(
        state,
        headers,
        "claude_code_update_start",
        "Claude Code update",
        "npm install -g @anthropic-ai/claude-code@latest && claude --version",
        "claude_code_update_start",
    )
    .await
}

async fn claude_code_smoke(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    start_claude_code_fixed_job(
        state,
        headers,
        "claude_code_smoke",
        "Claude Code smoke",
        "claude -p 'ping' --output-format json",
        "claude_code_smoke",
    )
    .await
}

async fn claude_code_cache_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResponse {
    start_claude_code_fixed_job(
        state,
        headers,
        "claude_code_cache_status",
        "Claude Code cache/log status",
        "printf 'Claude home: '; printf '%s\\n' \"${CLAUDE_CONFIG_DIR:-$HOME/.claude}\"; du -sh \"${CLAUDE_CONFIG_DIR:-$HOME/.claude}\" 2>/dev/null || true; find \"${CLAUDE_CONFIG_DIR:-$HOME/.claude}\" -maxdepth 2 -type f 2>/dev/null | wc -l",
        "claude_code_status",
    )
    .await
}

async fn start_claude_code_fixed_job(
    state: AppState,
    headers: HeaderMap,
    kind: &str,
    title: &str,
    command: &str,
    group: &str,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    state.db.record_audit(
        Some(&auth.admin_id),
        &format!("{kind}.started"),
        Some("claude_code"),
        Some(title),
        None,
        json!({}),
    )?;
    let id = state
        .jobs
        .start_exclusive_shell_job(kind, title, command.to_string(), group)
        .map_err(|err| api_error(StatusCode::CONFLICT, &err.to_string()))?;
    ok(json!({"job_id": id}))
}

#[derive(Debug, Clone)]
struct ProbeCommandProfile {
    flavor: &'static str,
    binary: Option<std::path::PathBuf>,
    service_kind: &'static str,
    service_name: &'static str,
    config_path: std::path::PathBuf,
}

impl ProbeCommandProfile {
    fn available(&self) -> bool {
        self.binary.as_ref().is_some_and(|path| path.exists())
    }
}

fn probe_command_profile() -> ProbeCommandProfile {
    let server_bin =
        std::path::PathBuf::from("/opt/codex-sentinel-server/bin/codex-sentinel-server");
    if server_bin.exists() {
        return ProbeCommandProfile {
            flavor: "server",
            binary: Some(server_bin),
            service_kind: "systemd",
            service_name: "codex-sentinel-server",
            config_path: std::path::PathBuf::from("/etc/codex-sentinel-server/config.toml"),
        };
    }
    let lite_bin = std::path::PathBuf::from(
        "/Applications/Codex Sentinel Lite.app/Contents/MacOS/codex-sentinel-lite",
    );
    if lite_bin.exists() {
        return ProbeCommandProfile {
            flavor: "lite",
            binary: Some(lite_bin),
            service_kind: "launchd",
            service_name: "local.codex-sentinel-lite",
            config_path: std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".codex-sentinel-lite/config.toml"),
        };
    }
    ProbeCommandProfile {
        flavor: "unavailable",
        binary: None,
        service_kind: "unknown",
        service_name: "probe",
        config_path: PlatformPaths::current().config_file,
    }
}

async fn probe_status_value() -> Value {
    let profile = probe_command_profile();
    let paths = PlatformPaths::current();
    let fallback = sentinel::sentinel_status(&paths, &SentinelConfig::default());
    json!({
        "label": "Probe",
        "enabled": fallback.enabled,
        "available": profile.available(),
        "platform": paths.kind,
        "service_kind": profile.service_kind,
        "service_name": profile.service_name,
        "flavor": profile.flavor,
        "binary_path": profile.binary.as_ref().map(|path| path.display().to_string()),
        "hook_status": if profile.available() { "managed" } else { fallback.hook_status.as_str() },
        "bark_status": fallback.bark_status,
        "logs_db_status": if profile.available() { "maintenance_ready" } else { fallback.logs_db_status.as_str() },
        "recent_event_count": fallback.recent_event_count,
        "reply_needed_count": fallback.reply_needed_count,
        "recoverable_count": fallback.recoverable_count,
        "config_path": profile.config_path,
    })
}

fn probe_doctor_command() -> &'static str {
    if probe_command_profile().flavor == "lite" {
        "lifecycle-status"
    } else {
        "doctor"
    }
}

fn probe_hooks_install_args() -> Vec<String> {
    if probe_command_profile().flavor == "server" {
        vec![
            "install-hooks-root".to_string(),
            "--restart-app-server".to_string(),
        ]
    } else {
        vec!["install-hooks".to_string()]
    }
}

async fn probe_json_command(args: &[&str]) -> Value {
    let profile = probe_command_profile();
    let Some(binary) = profile.binary.as_ref().filter(|path| path.exists()) else {
        return json!({
            "available": false,
            "flavor": profile.flavor,
            "error": "Probe binary not found"
        });
    };
    let Some(command_args) = probe_args_for_profile(&profile, args) else {
        return json!({
            "available": false,
            "flavor": profile.flavor,
            "error": format!("Probe command is not available for {} profile", profile.flavor),
            "requested": args,
        });
    };
    let output = tokio::time::timeout(
        Duration::from_secs(12),
        tokio::process::Command::new(binary)
            .args(&command_args)
            .output(),
    )
    .await;
    let output = match output {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => {
            return json!({
                "available": false,
                "flavor": profile.flavor,
                "error": err.to_string(),
            });
        }
        Err(_) => {
            return json!({
                "available": false,
                "flavor": profile.flavor,
                "error": "Probe command timed out",
            });
        }
    };
    let stdout = nexushub_core::security::redact_output(&String::from_utf8_lossy(&output.stdout));
    let stderr = nexushub_core::security::redact_output(&String::from_utf8_lossy(&output.stderr));
    let parsed = serde_json::from_str::<Value>(&stdout).ok();
    json!({
        "available": output.status.success(),
        "flavor": profile.flavor,
        "exit_code": output.status.code(),
        "args": command_args,
        "data": parsed.unwrap_or(Value::Null),
        "stdout": if stdout.trim().is_empty() { Value::Null } else { Value::String(stdout) },
        "stderr": if stderr.trim().is_empty() { Value::Null } else { Value::String(stderr) },
    })
}

fn probe_args_for_profile(profile: &ProbeCommandProfile, args: &[&str]) -> Option<Vec<String>> {
    let Some(command) = args.first().copied() else {
        return Some(Vec::new());
    };
    if profile.flavor == "server" {
        match command {
            "hook-status" => return Some(vec!["doctor".to_string()]),
            "debug-app-server-thread" => return None,
            _ => {}
        }
    }
    if profile.flavor == "lite" && command == "test-bark" {
        return None;
    }
    Some(args.iter().map(|arg| (*arg).to_string()).collect())
}

fn fixed_probe_shell_command(args: &[String]) -> String {
    let profile = probe_command_profile();
    let Some(binary) = profile.binary.as_ref().filter(|path| path.exists()) else {
        return "printf 'Probe binary not found\\n'; exit 127".to_string();
    };
    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let Some(command_args) = probe_args_for_profile(&profile, &arg_refs) else {
        return format!(
            "printf 'Probe command is not available for {} profile\\n'; exit 2",
            profile.flavor
        );
    };
    std::iter::once(shell_quote(&binary.display().to_string()))
        .chain(command_args.iter().map(|arg| shell_quote(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

async fn load_probe_threads(
    state: &AppState,
    status: &'static str,
    limit: usize,
) -> anyhow::Result<Vec<ThreadSummary>> {
    let paths = CodexPaths::new(&state.config.codex.home);
    let local_fetch_limit = thread_list_fetch_limit(Some(status), Some(limit));
    let app_fetch_limit = app_server_thread_list_fetch_limit(Some(status), Some(limit));
    let hidden_thread_ids = codex::hidden_thread_ids(&paths).unwrap_or_else(|err| {
        tracing::warn!("failed to read hidden thread metadata for probe: {err}");
        HashSet::new()
    });
    let mut threads = codex::list_threads(&paths, None, None, local_fetch_limit)?;
    if state.bridge.enabled() {
        match state
            .bridge
            .thread_list(app_fetch_limit, archived_filter(Some(status)), None)
            .await
        {
            Ok(value) => {
                let app_threads = app_server_thread_summaries(&value, &threads);
                if !app_threads.is_empty() {
                    threads = merge_thread_summaries(threads, app_threads);
                }
            }
            Err(err) => {
                tracing::warn!(
                    "app-server thread/list failed in probe; using state DB fallback: {err}"
                );
            }
        }
    }
    threads = prune_hidden_thread_summaries(threads, &hidden_thread_ids);
    apply_running_jobs_to_threads(state, &mut threads)?;
    threads = prune_hidden_thread_summaries(threads, &hidden_thread_ids);
    Ok(filter_thread_summaries(
        threads,
        Some(status),
        None,
        limit.clamp(1, 200),
    ))
}

async fn upload_files(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let root = uploads::upload_root(&state.config.codex.home);
    let protected_upload_ids = state.db.active_followup_upload_ids().unwrap_or_else(|err| {
        tracing::warn!("active follow-up upload lookup failed: {err}");
        HashSet::new()
    });
    if let Err(err) = uploads::cleanup_stale_uploads_except(
        &root,
        Duration::from_secs(uploads::UPLOAD_TTL_SECONDS),
        &protected_upload_ids,
    ) {
        tracing::warn!("stale upload cleanup failed: {err}");
    }
    let mut files = Vec::new();
    let mut total_bytes = 0usize;
    let mut rollback = true;
    let result: Result<uploads::UploadOutcome, ApiError> = async {
        while let Some(field) = multipart.next_field().await.map_err(|err| {
            api_error(
                StatusCode::BAD_REQUEST,
                &format!("invalid upload body: {err}"),
            )
        })? {
            let Some(file_name) = field.file_name().map(str::to_string) else {
                continue;
            };
            if files.len() >= MAX_UPLOAD_FILES {
                return Err(api_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "一次最多上传 5 个文件",
                ));
            }
            let mime = field.content_type().map(str::to_string);
            let bytes = field.bytes().await.map_err(|err| {
                api_error(
                    StatusCode::BAD_REQUEST,
                    &format!("read upload failed: {err}"),
                )
            })?;
            if bytes.len() > MAX_UPLOAD_FILE_BYTES {
                return Err(api_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "单个文件不能超过 10 MiB",
                ));
            }
            total_bytes += bytes.len();
            if total_bytes > MAX_TOTAL_UPLOAD_BYTES {
                return Err(api_error(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "一次上传总大小不能超过 30 MiB",
                ));
            }
            let record = uploads::store_upload(&root, &file_name, mime.as_deref(), &bytes)
                .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
            files.push(record);
        }
        if files.is_empty() {
            return Err(api_error(StatusCode::BAD_REQUEST, "没有可上传的文件"));
        }
        state.db.record_audit(
            Some(&auth.admin_id),
            "uploads.create",
            Some("upload"),
            None,
            None,
            json!({"files": files.len(), "bytes": total_bytes}),
        )?;
        rollback = false;
        Ok(uploads::UploadOutcome {
            files: files.clone(),
        })
    }
    .await;
    if rollback {
        let ids = files.iter().map(|file| file.id.clone()).collect::<Vec<_>>();
        cleanup_upload_ids(&root, &ids);
    }
    ok(result?)
}

async fn delete_upload_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let root = uploads::upload_root(&state.config.codex.home);
    let deleted = uploads::delete_upload(&root, &id)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    if deleted {
        state.db.record_audit(
            Some(&auth.admin_id),
            "uploads.delete",
            Some("upload"),
            Some(&id),
            None,
            json!({}),
        )?;
    }
    ok(json!({"ok": true, "deleted": deleted}))
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
    turnstile_token: Option<String>,
}

async fn login(
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
    let security = state
        .db
        .security_settings(state.config.security.session_ttl_seconds)?;
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
    let Some(admin) = state.db.admin_by_username(&payload.username)? else {
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
        Some(&admin.username),
        ip.as_deref(),
        Value::Object(Default::default()),
    )?;
    let mut response = Json(json!({
        "id": admin.id,
        "username": admin.username,
        "csrf_token": csrf,
    }))
    .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie(
            &token,
            state.config.security.cookie_secure,
            ttl,
        ))
        .expect("valid cookie"),
    );
    Ok(response)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    if let Some(token) = crate::auth::extract_cookie(&headers, SESSION_COOKIE) {
        state.db.revoke_session(&token)?;
    }
    let mut response = Json(json!({"ok": true})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&expired_session_cookie(state.config.security.cookie_secure))
            .expect("valid cookie"),
    );
    Ok(response)
}

async fn me(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(json!({
        "id": auth.admin_id,
        "username": auth.username,
        "csrf_token": null,
        "session_id": auth.session_id
    }))
}

async fn get_security(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(security_response(&state)?)
}

#[derive(Debug, Deserialize)]
struct SecurityPatch {
    turnstile_enabled: Option<bool>,
    turnstile_required: Option<bool>,
    turnstile_site_key: Option<String>,
    turnstile_secret_key: Option<String>,
    session_ttl_seconds: Option<u64>,
    turnstile_expected_hostname: Option<String>,
    turnstile_expected_action: Option<String>,
}

async fn patch_security(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SecurityPatch>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    if let Some(value) = payload.turnstile_enabled {
        state
            .db
            .set_setting("turnstile_enabled", if value { "true" } else { "false" })?;
    }
    if let Some(value) = payload.turnstile_required {
        state
            .db
            .set_setting("turnstile_required", if value { "true" } else { "false" })?;
    }
    if let Some(value) = payload.turnstile_site_key {
        state.db.set_setting("turnstile_site_key", &value)?;
    }
    let secret_key_changed = payload
        .turnstile_secret_key
        .as_ref()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false);
    if let Some(value) = payload.turnstile_secret_key.as_ref() {
        if !value.trim().is_empty() {
            state.db.set_turnstile_secret(value)?;
        }
    }
    if let Some(ttl) = payload.session_ttl_seconds {
        if ttl < 300 {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "session ttl must be at least 300 seconds",
            ));
        }
        state
            .db
            .set_setting("session_ttl_seconds", &ttl.to_string())?;
    }
    if let Some(value) = payload.turnstile_expected_hostname {
        state
            .db
            .set_setting("turnstile_expected_hostname", value.trim())?;
    }
    if let Some(value) = payload.turnstile_expected_action {
        state
            .db
            .set_setting("turnstile_expected_action", value.trim())?;
    }
    state.db.record_audit(
        Some(&auth.admin_id),
        "security.updated",
        Some("security"),
        Some("settings"),
        None,
        json!({"turnstile_secret_key": if secret_key_changed { Some("[configured]") } else { None }}),
    )?;
    ok(security_response(&state)?)
}

#[derive(Debug, Deserialize)]
struct PasswordChange {
    current_password: String,
    new_password: String,
}

async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PasswordChange>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let admin = state
        .db
        .admin_by_id(&auth.admin_id)?
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "unauthorized"))?;
    if !verify_password(&payload.current_password, &admin.password_hash) {
        return Err(api_error(
            StatusCode::UNAUTHORIZED,
            "invalid current password",
        ));
    }
    if payload.new_password.len() < 12 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "new password must be at least 12 characters",
        ));
    }
    let hash = crate::auth::hash_password(&payload.new_password)?;
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

#[derive(Debug, Deserialize)]
struct ThreadsQuery {
    status: Option<String>,
    q: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ThreadDetailQuery {
    limit: Option<usize>,
    before: Option<String>,
    full: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ThreadBlocksQuery {
    limit: Option<usize>,
    before: Option<String>,
}

#[derive(Debug, Serialize)]
struct ThreadBlockPage {
    thread_id: String,
    blocks: Vec<MessageBlock>,
    total_blocks: usize,
    has_more_blocks: bool,
    before_cursor: Option<String>,
}

async fn list_threads(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ThreadsQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let paths = CodexPaths::new(&state.config.codex.home);
    let response_limit = requested_thread_limit(query.limit);
    let local_fetch_limit = thread_list_fetch_limit(query.status.as_deref(), query.limit);
    let app_fetch_limit = app_server_thread_list_fetch_limit(query.status.as_deref(), query.limit);
    let hidden_thread_ids = codex::hidden_thread_ids(&paths).unwrap_or_else(|err| {
        tracing::warn!("failed to read hidden thread metadata: {err}");
        HashSet::new()
    });
    let mut threads = codex::list_threads(&paths, None, query.q.as_deref(), local_fetch_limit)?;
    if state.bridge.enabled() {
        match state
            .bridge
            .thread_list(
                app_fetch_limit,
                archived_filter(query.status.as_deref()),
                query.q.as_deref(),
            )
            .await
        {
            Ok(value) => {
                let app_threads = app_server_thread_summaries(&value, &threads);
                if !app_threads.is_empty() {
                    threads = merge_thread_summaries(threads, app_threads);
                }
            }
            Err(err) => {
                tracing::warn!("app-server thread/list failed; using state DB fallback: {err}");
            }
        }
    }
    threads = prune_hidden_thread_summaries(threads, &hidden_thread_ids);
    apply_running_jobs_to_threads(&state, &mut threads)?;
    threads = prune_hidden_thread_summaries(threads, &hidden_thread_ids);
    submit_ready_followups_from_list(&state, &mut threads).await;
    threads = filter_thread_summaries(
        threads,
        query.status.as_deref(),
        query.q.as_deref(),
        response_limit,
    );
    ok(threads)
}

async fn thread_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<ThreadDetailQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let block_limit = detail_block_limit(query.limit, query.full);
    match load_merged_thread_detail(&state, &id, "thread detail")
        .await
        .map_err(api_error_for_thread_detail_load)?
    {
        Some(detail) => ok(codex::window_thread_detail(
            detail,
            block_limit,
            query.before.as_deref(),
        )),
        None => Err(api_error(StatusCode::NOT_FOUND, "thread not found")),
    }
}

fn api_error_for_thread_detail_load(err: anyhow::Error) -> ApiError {
    let message = err.to_string();
    let status = if message.starts_with("app-server thread/read failed") {
        StatusCode::BAD_GATEWAY
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    api_error(status, &message)
}

async fn thread_blocks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<ThreadBlocksQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    match load_merged_thread_detail(&state, &id, "thread blocks")
        .await
        .map_err(api_error_for_thread_detail_load)?
    {
        Some(detail) => ok(thread_block_page(
            &id,
            detail,
            query.limit,
            query.before.as_deref(),
        )),
        None => Err(api_error(StatusCode::NOT_FOUND, "thread not found")),
    }
}

fn thread_block_page(
    thread_id: &str,
    detail: ThreadDetail,
    limit: Option<usize>,
    before: Option<&str>,
) -> ThreadBlockPage {
    let window = codex::window_thread_detail(detail, Some(block_page_limit(limit)), before);
    ThreadBlockPage {
        thread_id: thread_id.to_string(),
        blocks: window.blocks,
        total_blocks: window.total_blocks,
        has_more_blocks: window.has_more_blocks,
        before_cursor: window.before_cursor,
    }
}

async fn load_merged_thread_detail(
    state: &AppState,
    id: &str,
    label: &str,
) -> anyhow::Result<Option<ThreadDetail>> {
    let paths = CodexPaths::new(&state.config.codex.home);
    let mut detail = load_base_thread_detail_cached(state, &paths, id)?;
    if state.bridge.enabled() {
        match state.bridge.thread_read(id.to_string(), true).await {
            Ok(value) => match detail.as_mut() {
                Some(detail) => apply_app_server_thread_detail(detail, &value),
                None => detail = app_server_detail_from_read(&value),
            },
            Err(err) if detail.is_some() => {
                tracing::warn!(
                    "app-server thread/read failed in {label}; using rollout detail: {err}"
                );
            }
            Err(err) => anyhow::bail!("app-server thread/read failed: {err}"),
        }
    }
    if let Some(detail) = detail.as_mut() {
        apply_running_job_to_detail(state, detail)?;
        submit_pending_followup_if_ready(state, detail).await;
    }
    Ok(detail)
}

fn load_base_thread_detail_cached(
    state: &AppState,
    paths: &CodexPaths,
    id: &str,
) -> anyhow::Result<Option<ThreadDetail>> {
    if let Some(cached) = state
        .rollout_detail_cache
        .lock()
        .expect("rollout detail cache mutex")
        .get(id)
        .cloned()
    {
        let signature = thread_detail_cache_signature(paths, cached.signature.rollout_path.clone());
        if cached.signature == signature {
            return Ok(Some(cached.detail));
        }
    }

    let detail = codex::thread_detail(paths, id)?;
    let signature = thread_detail_cache_signature(
        paths,
        detail
            .as_ref()
            .and_then(|detail| detail.summary.rollout_path.clone()),
    );
    if let Some(detail) = detail.as_ref() {
        state
            .rollout_detail_cache
            .lock()
            .expect("rollout detail cache mutex")
            .insert(
                id.to_string(),
                CachedThreadDetail {
                    signature,
                    detail: detail.clone(),
                },
            );
    } else {
        state
            .rollout_detail_cache
            .lock()
            .expect("rollout detail cache mutex")
            .remove(id);
    }
    Ok(detail)
}

fn thread_detail_cache_signature(
    paths: &CodexPaths,
    rollout_path: Option<std::path::PathBuf>,
) -> ThreadDetailCacheSignature {
    ThreadDetailCacheSignature {
        rollout: rollout_path.as_deref().and_then(file_signature),
        rollout_path,
        state_db: file_signature(&paths.state_db()),
        session_index: file_signature(&paths.session_index()),
    }
}

fn file_signature(path: &FsPath) -> Option<FileSignature> {
    let metadata = fs::metadata(path).ok()?;
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis());
    Some(FileSignature {
        len: metadata.len(),
        modified_ms,
    })
}

#[derive(Debug, Deserialize)]
struct CreateThreadRequest {
    message: String,
    #[serde(default)]
    attachments: Vec<String>,
    model: Option<String>,
    service_tier: Option<String>,
    reasoning_effort: Option<String>,
    cwd: Option<String>,
    permission_profile: Option<String>,
    approval_policy: Option<String>,
    sandbox_mode: Option<String>,
    network_access: Option<bool>,
    collaboration_mode: Option<String>,
}

async fn create_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateThreadRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let prepared_attachments = prepare_request_attachments(&state, &payload.attachments)?;
    let effective_message = effective_message(&payload.message, &prepared_attachments);
    let cwd = payload
        .cwd
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| state.config.codex.workspace.clone());
    let bridge_options = BridgeTurnOptions {
        message: effective_message.clone(),
        attachments: prepared_attachments.clone(),
        model: payload.model.clone(),
        service_tier: payload.service_tier.clone(),
        reasoning_effort: payload.reasoning_effort.clone(),
        cwd: Some(cwd.display().to_string()),
        permission_profile: payload.permission_profile.clone(),
        approval_policy: payload.approval_policy.clone(),
        sandbox_mode: payload.sandbox_mode.clone(),
        network_access: payload.network_access,
        collaboration_mode: payload.collaboration_mode.clone(),
    };
    if state.bridge.enabled() {
        match state.bridge.start_thread(bridge_options).await {
            Ok(result) => {
                state.db.record_audit(
                    Some(&auth.admin_id),
                    "thread.create.bridge_started",
                    Some("thread"),
                    result.thread_id.as_deref(),
                    None,
                    json!({"turn_id": result.turn_id}),
                )?;
                return ok(result);
            }
            Err(err) => {
                tracing::warn!(
                    "app-server bridge create failed; falling back to codex exec: {err}"
                );
            }
        }
    }
    let mut args = vec![
        "exec".to_string(),
        "--json".to_string(),
        "--skip-git-repo-check".to_string(),
        "-".to_string(),
    ];
    if let Some(model) = payload.model.filter(|value| !value.trim().is_empty()) {
        args.splice(1..1, ["-m".to_string(), model]);
    }
    if let Some(reasoning) = payload
        .reasoning_effort
        .filter(|value| !value.trim().is_empty())
    {
        args.splice(
            1..1,
            [
                "-c".to_string(),
                format!("model_reasoning_effort=\"{reasoning}\""),
            ],
        );
    }
    if let Some(service_tier) = payload
        .service_tier
        .filter(|value| !value.trim().is_empty())
    {
        args.splice(
            1..1,
            [
                "-c".to_string(),
                format!(
                    "model_service_tier=\"{}\"",
                    cli_config_string(&service_tier)
                ),
            ],
        );
    }
    let job_id = state.jobs.start_codex_job(
        "Codex new thread",
        &state.config.codex.home,
        &cwd,
        args,
        prompt_with_attachment_context(&effective_message, &prepared_attachments),
    )?;
    state.db.link_job_thread(&job_id, None, None)?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.create.job_started",
        Some("job"),
        Some(&job_id),
        None,
        json!({"cwd": cwd.display().to_string()}),
    )?;
    ok(BridgeActionResult {
        bridge: false,
        thread_id: None,
        turn_id: None,
        job_id: Some(job_id),
        fallback: true,
        message: Some("app-server bridge unavailable; started codex exec fallback job".to_string()),
    })
}

#[derive(Debug, Deserialize)]
struct SendMessageRequest {
    message: String,
    #[serde(default)]
    attachments: Vec<String>,
    #[serde(default)]
    prepared_attachments: Vec<PreparedAttachment>,
    model: Option<String>,
    service_tier: Option<String>,
    reasoning_effort: Option<String>,
    cwd: Option<String>,
    permission_profile: Option<String>,
    approval_policy: Option<String>,
    sandbox_mode: Option<String>,
    network_access: Option<bool>,
    collaboration_mode: Option<String>,
}

impl SendMessageRequest {
    fn bridge_options(&self) -> BridgeTurnOptions {
        BridgeTurnOptions {
            message: effective_message(&self.message, &self.prepared_attachments),
            attachments: self.prepared_attachments.clone(),
            model: self.model.clone(),
            service_tier: self.service_tier.clone(),
            reasoning_effort: self.reasoning_effort.clone(),
            cwd: self.cwd.clone(),
            permission_profile: self.permission_profile.clone(),
            approval_policy: self.approval_policy.clone(),
            sandbox_mode: self.sandbox_mode.clone(),
            network_access: self.network_access,
            collaboration_mode: self.collaboration_mode.clone(),
        }
    }

    fn options_json(&self) -> Value {
        json!({
            "model": &self.model,
            "service_tier": &self.service_tier,
            "reasoning_effort": &self.reasoning_effort,
            "cwd": &self.cwd,
            "permission_profile": &self.permission_profile,
            "approval_policy": &self.approval_policy,
            "sandbox_mode": &self.sandbox_mode,
            "network_access": self.network_access,
            "collaboration_mode": &self.collaboration_mode,
            "attachments": &self.attachments,
            "prepared_attachments": &self.prepared_attachments,
        })
    }
}

fn effective_message(message: &str, attachments: &[PreparedAttachment]) -> String {
    let message = message.trim();
    if message.is_empty() && !attachments.is_empty() {
        "请根据以下附件内容继续处理。".to_string()
    } else {
        message.to_string()
    }
}

fn prepare_request_attachments(
    state: &AppState,
    attachment_ids: &[String],
) -> Result<Vec<PreparedAttachment>, ApiError> {
    if attachment_ids.len() > MAX_UPLOAD_FILES {
        return Err(api_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "一次最多发送 5 个附件",
        ));
    }
    let root = uploads::upload_root(&state.config.codex.home);
    prepare_uploads(&root, attachment_ids)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))
}

async fn send_message(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(mut payload): Json<SendMessageRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    payload.prepared_attachments = prepare_request_attachments(&state, &payload.attachments)?;
    if state.bridge.enabled() {
        match state
            .bridge
            .send_turn(id.clone(), payload.bridge_options())
            .await
        {
            Ok(result) => {
                state.db.record_audit(
                    Some(&auth.admin_id),
                    "thread.message.bridge_started",
                    Some("thread"),
                    Some(&id),
                    None,
                    json!({"turn_id": result.turn_id}),
                )?;
                return ok(result);
            }
            Err(err) => {
                tracing::warn!("app-server bridge send failed; falling back to codex exec: {err}");
            }
        }
    }
    let args = vec![
        "exec".to_string(),
        "resume".to_string(),
        "--all".to_string(),
        "--json".to_string(),
        id.clone(),
        "-".to_string(),
    ];
    let job_id = state.jobs.start_codex_job(
        "Codex resume thread",
        &state.config.codex.home,
        &state.config.codex.workspace,
        args,
        prompt_with_attachment_context(
            &effective_message(&payload.message, &payload.prepared_attachments),
            &payload.prepared_attachments,
        ),
    )?;
    state.db.link_job_thread(&job_id, Some(&id), None)?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.message.job_started",
        Some("thread"),
        Some(&id),
        None,
        json!({"job_id": job_id}),
    )?;
    ok(BridgeActionResult {
        bridge: false,
        thread_id: Some(id),
        turn_id: None,
        job_id: Some(job_id),
        fallback: true,
        message: Some("app-server bridge unavailable; started codex exec fallback job".to_string()),
    })
}

async fn resolve_active_turn_id(state: &AppState, id: &str) -> Option<String> {
    let paths = CodexPaths::new(&state.config.codex.home);
    let mut detail = match load_base_thread_detail_cached(state, &paths, id) {
        Ok(Some(detail)) => Some(detail),
        Ok(None) | Err(_) => None,
    };
    if state.bridge.enabled() {
        if let Ok(value) = state.bridge.thread_read(id.to_string(), true).await {
            match detail.as_mut() {
                Some(detail) => apply_app_server_thread_detail(detail, &value),
                None => detail = app_server_detail_from_read(&value),
            }
        }
    }
    if let Some(detail) = detail.as_mut() {
        let _ = apply_running_job_to_detail(state, detail);
        return detail.summary.active_turn_id.clone();
    }
    None
}

async fn steer_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(mut payload): Json<SendMessageRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    payload.prepared_attachments = prepare_request_attachments(&state, &payload.attachments)?;
    let message = effective_message(&payload.message, &payload.prepared_attachments);
    if message.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "follow-up message is required",
        ));
    }

    if state.bridge.enabled() {
        match resolve_active_turn_id(&state, &id).await {
            Some(expected_turn_id) => {
                match state
                    .bridge
                    .steer_turn(
                        id.clone(),
                        expected_turn_id.clone(),
                        payload.bridge_options(),
                    )
                    .await
                {
                    Ok(mut result) => {
                        if result.turn_id.is_none() {
                            result.turn_id = Some(expected_turn_id);
                        }
                        state.db.record_audit(
                            Some(&auth.admin_id),
                            "thread.followup.steered",
                            Some("thread"),
                            Some(&id),
                            None,
                            json!({"turn_id": result.turn_id}),
                        )?;
                        return ok(result);
                    }
                    Err(err) => {
                        tracing::warn!("turn/steer failed; queueing follow-up fallback: {err}");
                    }
                }
            }
            None => {
                tracing::warn!("turn/steer skipped because active turn could not be resolved");
            }
        }
    }

    let followup = state
        .db
        .enqueue_followup(&id, &message, payload.options_json())?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.followup.enqueued_after_steer_fallback",
        Some("thread"),
        Some(&id),
        None,
        json!({"followup_id": followup.id}),
    )?;
    ok(BridgeActionResult {
        bridge: true,
        thread_id: Some(id),
        turn_id: None,
        job_id: None,
        fallback: true,
        message: Some(
            "turn/steer unavailable; queued follow-up for the next idle turn".to_string(),
        ),
    })
}

async fn list_followups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(json!({ "items": followup_responses(state.db.list_followups(&id, 20)?) }))
}

async fn enqueue_followup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(mut payload): Json<SendMessageRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    payload.prepared_attachments = prepare_request_attachments(&state, &payload.attachments)?;
    let message = effective_message(&payload.message, &payload.prepared_attachments);
    if message.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "follow-up message is required",
        ));
    }
    let followup = state
        .db
        .enqueue_followup(&id, &message, payload.options_json())?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.followup.enqueued",
        Some("thread"),
        Some(&id),
        None,
        json!({"followup_id": followup.id}),
    )?;
    ok(followup_response(followup))
}

async fn cancel_followup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, followup_id)): Path<(String, String)>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let cancelled = state.db.cancel_followup(&id, &followup_id)?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.followup.cancelled",
        Some("thread"),
        Some(&id),
        None,
        json!({"followup_id": followup_id, "cancelled": cancelled}),
    )?;
    ok(json!({"ok": cancelled}))
}

fn apply_running_jobs_to_threads(
    state: &AppState,
    threads: &mut Vec<ThreadSummary>,
) -> anyhow::Result<()> {
    let jobs = state.db.running_thread_jobs()?;
    let mut by_thread: HashMap<&str, &JobRecord> = HashMap::new();
    for job in &jobs {
        if let Some(thread_id) = job.thread_id.as_deref() {
            by_thread.entry(thread_id).or_insert(job);
        }
    }
    for thread in threads.iter_mut() {
        if let Some(job) = by_thread.get(thread.id.as_str()) {
            apply_running_job_to_summary(thread, job);
        }
    }
    for job in by_thread.values() {
        let Some(thread_id) = job.thread_id.as_deref() else {
            continue;
        };
        if !threads.iter().any(|thread| thread.id == thread_id) {
            threads.push(thread_summary_from_running_job(job));
        }
    }
    Ok(())
}

fn apply_running_job_to_detail(state: &AppState, detail: &mut ThreadDetail) -> anyhow::Result<()> {
    if let Some(job) = state.db.running_job_for_thread(&detail.summary.id)? {
        apply_running_job_to_summary(&mut detail.summary, &job);
    }
    Ok(())
}

fn apply_running_job_to_summary(summary: &mut ThreadSummary, job: &JobRecord) {
    if matches!(summary.status, ThreadStatus::Archived) {
        return;
    }
    summary.status = ThreadStatus::Running;
    summary.active_job_id = Some(job.id.clone());
    if summary.active_turn_id.is_none() {
        summary.active_turn_id = job.turn_id.clone();
    }
    if summary.latest_message.is_none() {
        summary.latest_message = Some(job.title.clone());
    }
}

fn thread_summary_from_running_job(job: &JobRecord) -> ThreadSummary {
    ThreadSummary {
        id: job.thread_id.clone().unwrap_or_else(|| job.id.clone()),
        title: "未命名线程".to_string(),
        status: ThreadStatus::Running,
        updated_at: timestamp_to_rfc3339(job.started_at),
        archived_at: None,
        message_count: 0,
        latest_message: Some(job.title.clone()),
        cwd: None,
        model: None,
        rollout_path: None,
        active_turn_id: job.turn_id.clone(),
        active_job_id: Some(job.id.clone()),
        pending_elicitation: None,
        last_event_kind: None,
    }
}

async fn submit_ready_followups_from_list(state: &AppState, threads: &mut [ThreadSummary]) {
    for summary in threads {
        if !matches!(summary.status, ThreadStatus::Recent) {
            continue;
        }
        let mut detail = ThreadDetail {
            summary: summary.clone(),
            messages: Vec::new(),
            blocks: Vec::new(),
            raw_event_count: 0,
            total_blocks: 0,
            has_more_blocks: false,
            before_cursor: None,
        };
        submit_pending_followup_if_ready(state, &mut detail).await;
        if matches!(detail.summary.status, ThreadStatus::Running) {
            *summary = detail.summary;
        }
    }
}

async fn submit_pending_followup_if_ready(state: &AppState, detail: &mut ThreadDetail) {
    if !matches!(detail.summary.status, ThreadStatus::Recent) {
        return;
    }
    let thread_id = detail.summary.id.clone();
    let Ok(Some(followup)) = state.db.claim_next_pending_followup(&thread_id) else {
        return;
    };
    let request = followup_request(&followup);
    if state.bridge.enabled() {
        match state
            .bridge
            .send_turn(thread_id.clone(), request.bridge_options())
            .await
        {
            Ok(result) => {
                let _ = state
                    .db
                    .mark_followup_submitted(&followup.id, json!(result));
                detail.summary.status = ThreadStatus::Running;
                detail.summary.active_turn_id = result.turn_id;
                return;
            }
            Err(err) => {
                tracing::warn!(
                    "queued follow-up bridge send failed; falling back to codex exec: {err}"
                );
            }
        }
    }
    let args = vec![
        "exec".to_string(),
        "resume".to_string(),
        "--all".to_string(),
        "--json".to_string(),
        thread_id.clone(),
        "-".to_string(),
    ];
    match state.jobs.start_codex_job(
        "Codex queued follow-up",
        &state.config.codex.home,
        &state.config.codex.workspace,
        args,
        prompt_with_attachment_context(
            &effective_message(&request.message, &request.prepared_attachments),
            &request.prepared_attachments,
        ),
    ) {
        Ok(job_id) => {
            let _ = state.db.link_job_thread(&job_id, Some(&thread_id), None);
            let _ = state
                .db
                .mark_followup_submitted(&followup.id, json!({"job_id": job_id}));
            detail.summary.status = ThreadStatus::Running;
            detail.summary.active_job_id = Some(job_id);
        }
        Err(err) => {
            let message = err.to_string();
            let _ = state.db.mark_followup_error(&followup.id, &message);
        }
    }
}

fn followup_request(followup: &ThreadFollowUp) -> SendMessageRequest {
    let options = serde_json::from_str::<Value>(&followup.options_json).unwrap_or(Value::Null);
    let attachments = options
        .get("attachments")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let prepared_attachments = options
        .get("prepared_attachments")
        .cloned()
        .and_then(|value| serde_json::from_value::<Vec<PreparedAttachment>>(value).ok())
        .unwrap_or_default();
    SendMessageRequest {
        message: followup.message.clone(),
        attachments,
        prepared_attachments,
        model: options
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string),
        service_tier: options
            .get("service_tier")
            .or_else(|| options.get("serviceTier"))
            .and_then(Value::as_str)
            .map(str::to_string),
        reasoning_effort: options
            .get("reasoning_effort")
            .and_then(Value::as_str)
            .map(str::to_string),
        cwd: options
            .get("cwd")
            .and_then(Value::as_str)
            .map(str::to_string),
        permission_profile: options
            .get("permission_profile")
            .and_then(Value::as_str)
            .map(str::to_string),
        approval_policy: options
            .get("approval_policy")
            .and_then(Value::as_str)
            .map(str::to_string),
        sandbox_mode: options
            .get("sandbox_mode")
            .and_then(Value::as_str)
            .map(str::to_string),
        network_access: options.get("network_access").and_then(Value::as_bool),
        collaboration_mode: options
            .get("collaboration_mode")
            .and_then(Value::as_str)
            .map(str::to_string),
    }
}

fn followup_responses(items: Vec<ThreadFollowUp>) -> Vec<Value> {
    items.into_iter().map(followup_response).collect()
}

fn followup_response(item: ThreadFollowUp) -> Value {
    json!({
        "id": item.id,
        "thread_id": item.thread_id,
        "status": item.status,
        "message": item.message,
        "options": serde_json::from_str::<Value>(&item.options_json).unwrap_or(Value::Null),
        "created_at": item.created_at,
        "updated_at": item.updated_at,
        "submitted_at": item.submitted_at,
        "cancelled_at": item.cancelled_at,
        "result": item.result_json.and_then(|value| serde_json::from_str::<Value>(&value).ok()),
        "error": item.error,
    })
}

async fn derive_active_turn_id(state: &AppState, thread_id: &str) -> Option<String> {
    if state.bridge.enabled() {
        if let Ok(value) = state.bridge.thread_read(thread_id.to_string(), true).await {
            if let Some(thread) = value.get("thread") {
                if let Some(turn_id) = app_thread_active_turn_id(thread) {
                    return Some(turn_id);
                }
            }
        }
    }
    let paths = CodexPaths::new(&state.config.codex.home);
    codex::thread_detail(&paths, thread_id)
        .ok()
        .flatten()
        .and_then(|detail| detail.summary.active_turn_id)
}

fn derive_active_job_id(state: &AppState, thread_id: &str) -> Option<String> {
    state
        .db
        .running_job_for_thread(thread_id)
        .ok()
        .flatten()
        .map(|job| job.id)
}

#[derive(Debug, Deserialize)]
struct StopThreadRequest {
    turn_id: Option<String>,
    job_id: Option<String>,
}

async fn stop_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    payload: Option<Json<StopThreadRequest>>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let payload = payload.map(|Json(value)| value);
    let turn_id = payload.as_ref().and_then(|value| value.turn_id.clone());
    let turn_id = if turn_id.is_none() && state.bridge.enabled() {
        derive_active_turn_id(&state, &id).await
    } else {
        turn_id
    };
    if let Some(turn_id) = turn_id {
        if state.bridge.enabled() {
            match state.bridge.stop_turn(id.clone(), turn_id.clone()).await {
                Ok(()) => {
                    state.db.record_audit(
                        Some(&auth.admin_id),
                        "thread.stop.bridge",
                        Some("thread"),
                        Some(&id),
                        None,
                        json!({"turn_id": turn_id}),
                    )?;
                    return ok(json!({"ok": true, "bridge": true}));
                }
                Err(err) => {
                    tracing::warn!("app-server bridge stop failed: {err}");
                }
            }
        }
    }
    let job_id = payload
        .as_ref()
        .and_then(|value| value.job_id.clone())
        .or_else(|| derive_active_job_id(&state, &id));
    if let Some(job_id) = job_id.as_deref() {
        let cancelled = state.jobs.cancel_job(job_id)?;
        state.db.record_audit(
            Some(&auth.admin_id),
            "thread.stop.job_cancel",
            Some("job"),
            Some(job_id),
            None,
            json!({"thread_id": id, "cancelled": cancelled}),
        )?;
        return ok(json!({"ok": cancelled, "bridge": false, "job_id": job_id}));
    }
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.stop.requested",
        Some("thread"),
        Some(&id),
        None,
        Value::Object(Default::default()),
    )?;
    Err(api_error(
        StatusCode::BAD_REQUEST,
        "stop requires turn_id for app-server bridge or job_id for fallback job",
    ))
}

async fn archive_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    if state.bridge.enabled() {
        if let Err(err) = state.bridge.archive_thread(id.clone()).await {
            tracing::warn!("app-server bridge archive failed; falling back to state DB: {err}");
        }
    }
    let paths = CodexPaths::new(&state.config.codex.home);
    codex::set_thread_archived(&paths, &id, true)?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.archived",
        Some("thread"),
        Some(&id),
        None,
        json!({}),
    )?;
    ok(json!({"ok": true}))
}

async fn restore_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    if state.bridge.enabled() {
        if let Err(err) = state.bridge.unarchive_thread(id.clone()).await {
            tracing::warn!("app-server bridge restore failed; falling back to state DB: {err}");
        }
    }
    let paths = CodexPaths::new(&state.config.codex.home);
    codex::set_thread_archived(&paths, &id, false)?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.restored",
        Some("thread"),
        Some(&id),
        None,
        json!({}),
    )?;
    ok(json!({"ok": true}))
}

#[derive(Debug, Deserialize)]
struct RenameThreadRequest {
    name: String,
}

async fn rename_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<RenameThreadRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(api_error(StatusCode::BAD_REQUEST, "name cannot be empty"));
    }
    if state.bridge.enabled() {
        match state
            .bridge
            .rename_thread(id.clone(), name.to_string())
            .await
        {
            Ok(()) => {
                let paths = CodexPaths::new(&state.config.codex.home);
                if let Err(err) = codex::set_thread_title(&paths, &id, name) {
                    tracing::warn!("state DB thread title fallback update failed: {err}");
                }
                state.db.record_audit(
                    Some(&auth.admin_id),
                    "thread.renamed",
                    Some("thread"),
                    Some(&id),
                    None,
                    json!({"name": name}),
                )?;
                return ok(json!({"ok": true, "bridge": true}));
            }
            Err(err) => {
                tracing::warn!("app-server bridge rename failed: {err}");
            }
        }
    }
    Err(api_error(
        StatusCode::BAD_GATEWAY,
        "rename requires app-server bridge",
    ))
}

async fn fork_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    match state.bridge.fork_thread(id.clone()).await {
        Ok(result) => {
            state.db.record_audit(
                Some(&auth.admin_id),
                "thread.forked",
                Some("thread"),
                Some(&id),
                None,
                json!({"new_thread_id": result.thread_id}),
            )?;
            ok(result)
        }
        Err(err) => Err(api_error(
            StatusCode::BAD_GATEWAY,
            &format!("fork requires app-server bridge: {err}"),
        )),
    }
}

#[derive(Debug, Deserialize)]
struct PlanAcceptRequest {
    turn_id: Option<String>,
    item_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlanReviseRequest {
    turn_id: Option<String>,
    item_id: Option<String>,
    instructions: String,
}

async fn plan_accept(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<PlanAcceptRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let mut result = send_bridge_reply(&state, &id, "1".to_string()).await?;
    result.fallback = true;
    result.message = Some("Plan accept sent as a new turn because app-server does not expose a dedicated plan accept method".to_string());
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.plan.accept",
        Some("thread"),
        Some(&id),
        None,
        json!({"turn_id": payload.turn_id, "item_id": payload.item_id, "bridge_fallback": true}),
    )?;
    ok(result)
}

async fn plan_revise(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<PlanReviseRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let instructions = payload.instructions.trim();
    if instructions.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "revision instructions cannot be empty",
        ));
    }
    let mut result =
        send_bridge_reply(&state, &id, format!("请调整计划：\n{instructions}")).await?;
    result.fallback = true;
    result.message = Some("Plan revision sent as a new turn because app-server does not expose a dedicated plan revise method".to_string());
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.plan.revise",
        Some("thread"),
        Some(&id),
        None,
        json!({"turn_id": payload.turn_id, "item_id": payload.item_id, "bridge_fallback": true}),
    )?;
    ok(result)
}

#[derive(Debug, Deserialize)]
struct ApprovalAnswerRequest {
    turn_id: Option<String>,
    item_id: Option<String>,
    request_id: Option<String>,
    decision: String,
}

async fn answer_approval(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<ApprovalAnswerRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.approval.unsupported",
        Some("thread"),
        Some(&id),
        None,
        json!({
            "turn_id": payload.turn_id,
            "item_id": payload.item_id,
            "request_id": payload.request_id,
            "decision": payload.decision
        }),
    )?;
    Err(api_error(
        StatusCode::NOT_IMPLEMENTED,
        "approval response requires the live app-server JSON-RPC request connection and is not supported by this panel version",
    ))
}

#[derive(Debug, Deserialize)]
struct ElicitationAnswerRequest {
    answers: HashMap<String, Vec<String>>,
}

async fn answer_elicitation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(payload): Json<ElicitationAnswerRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let message = payload
        .answers
        .iter()
        .map(|(question, answers)| format!("{question}: {}", answers.join(", ")))
        .collect::<Vec<_>>()
        .join("\n");
    if message.trim().is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "answers cannot be empty",
        ));
    }
    let bridge_options = BridgeTurnOptions {
        message,
        attachments: Vec::new(),
        model: None,
        service_tier: None,
        reasoning_effort: None,
        cwd: None,
        permission_profile: None,
        approval_policy: None,
        sandbox_mode: None,
        network_access: None,
        collaboration_mode: None,
    };
    match state.bridge.send_turn(id.clone(), bridge_options).await {
        Ok(mut result) => {
            result.fallback = true;
            result.message = Some("Elicitation answer sent as a new turn because live server-request response is not available".to_string());
            ok(result)
        }
        Err(err) => Err(api_error(
            StatusCode::BAD_GATEWAY,
            &format!("elicitation reply failed: {err}"),
        )),
    }
}

async fn send_bridge_reply(
    state: &AppState,
    thread_id: &str,
    message: String,
) -> Result<BridgeActionResult, ApiError> {
    let bridge_options = BridgeTurnOptions {
        message,
        attachments: Vec::new(),
        model: None,
        service_tier: None,
        reasoning_effort: None,
        cwd: None,
        permission_profile: None,
        approval_policy: None,
        sandbox_mode: None,
        network_access: None,
        collaboration_mode: None,
    };
    state
        .bridge
        .send_turn(thread_id.to_string(), bridge_options)
        .await
        .map_err(|err| {
            api_error(
                StatusCode::BAD_GATEWAY,
                &format!("plan reply failed: {err}"),
            )
        })
}

async fn thread_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let event_state = state.clone();
    let stream = async_stream::stream! {
        let mut sent_blocks: HashMap<String, String> = HashMap::new();
        let mut seeded_blocks = false;
        loop {
            match load_merged_thread_detail(&event_state, &id, "event stream").await {
                Ok(Some(mut detail)) => {
                    detail = codex::window_thread_detail(detail, Some(THREAD_EVENT_BLOCK_WINDOW), None);
                    if !seeded_blocks {
                        seed_thread_event_blocks(&mut sent_blocks, &detail.blocks);
                        seeded_blocks = true;
                    }
                    for block in &detail.blocks {
                        if block_changed(sent_blocks.get(&block.id), block) {
                            let key = thread_event_block_key(block);
                            yield Ok::<Event, std::convert::Infallible>(
                                Event::default()
                                    .event("block")
                                    .data(serde_json::to_string(block).unwrap_or_else(|_| "{}".to_string()))
                            );
                            sent_blocks.insert(block.id.clone(), key);
                        }
                    }
                    yield Ok::<Event, std::convert::Infallible>(
                        Event::default().event("summary").data(serde_json::to_string(&detail.summary).unwrap_or_else(|_| "{}".to_string()))
                    );
                }
                Ok(None) => {
                    yield Ok::<Event, std::convert::Infallible>(
                        Event::default().event("error").data(json!({"message":"thread not found"}).to_string())
                    );
                    break;
                }
                Err(err) => {
                    yield Ok::<Event, std::convert::Infallible>(
                        Event::default().event("error").data(json!({"message": err.to_string()}).to_string())
                    );
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    };
    Ok(Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(25))
                .text("ping"),
        )
        .into_response())
}

async fn system_status(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let mut status = nexushub_core::system::system_status(&state.config).await?;
    if state.bridge.enabled() {
        if let Ok(value) = state.bridge.thread_list(500, Some(false), None).await {
            let (source_counts, hidden_count) = app_server_thread_visibility_diagnostics(&value);
            status.app_server_source_counts = source_counts;
            status.app_server_hidden_thread_count = hidden_count;
        }
    }
    ok(status)
}

async fn system_version(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(nexushub_core::system::version_info().await?)
}

#[derive(Debug, Deserialize)]
struct CwdQuery {
    cwd: Option<String>,
}

async fn codex_models(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    match state.bridge.model_list().await {
        Ok(value) => ok(value),
        Err(err) => Err(api_error(
            StatusCode::BAD_GATEWAY,
            &format!("model/list failed: {err}"),
        )),
    }
}

async fn codex_permission_profiles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CwdQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    match state.bridge.permission_profile_list(query.cwd).await {
        Ok(value) => ok(value),
        Err(err) => Err(api_error(
            StatusCode::BAD_GATEWAY,
            &format!("permissionProfile/list failed: {err}"),
        )),
    }
}

async fn codex_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CwdQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    match state.bridge.config_read(query.cwd).await {
        Ok(value) => ok(normalize_config_response(&value, &state)),
        Err(err) => Err(api_error(
            StatusCode::BAD_GATEWAY,
            &format!("config/read failed: {err}"),
        )),
    }
}

#[derive(Debug, Deserialize)]
struct GoalQuery {
    thread_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoalUpdateRequest {
    thread_id: Option<String>,
    objective: Option<String>,
    token_budget: Option<u64>,
    status: Option<String>,
    enabled: Option<bool>,
}

async fn codex_goal_get(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<GoalQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let Some(thread_id) = non_empty(query.thread_id.as_deref()) else {
        return ok(goal_empty("missing_thread"));
    };
    match state.bridge.goal_get(thread_id.to_string()).await {
        Ok(value) => ok(normalize_goal_response(&value)),
        Err(err) => Err(api_error(
            StatusCode::BAD_GATEWAY,
            &format!("thread/goal/get failed: {err}"),
        )),
    }
}

async fn codex_goal_set(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<GoalUpdateRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let Some(thread_id) = non_empty(payload.thread_id.as_deref()) else {
        return Err(api_error(StatusCode::BAD_REQUEST, "thread_id is required"));
    };
    let objective = payload
        .objective
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    if objective.is_empty() || payload.enabled == Some(false) {
        return codex_goal_clear_inner(&state, thread_id).await;
    }
    match state
        .bridge
        .goal_set(
            thread_id.to_string(),
            objective.to_string(),
            payload.status.clone(),
            payload.token_budget,
        )
        .await
    {
        Ok(value) => ok(normalize_goal_response(&value)),
        Err(err) => Err(api_error(
            StatusCode::BAD_GATEWAY,
            &format!("thread/goal/set failed: {err}"),
        )),
    }
}

async fn codex_goal_clear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<GoalUpdateRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let Some(thread_id) = non_empty(payload.thread_id.as_deref()) else {
        return Err(api_error(StatusCode::BAD_REQUEST, "thread_id is required"));
    };
    codex_goal_clear_inner(&state, thread_id).await
}

async fn codex_goal_clear_inner(state: &AppState, thread_id: &str) -> ApiResponse {
    match state.bridge.goal_clear(thread_id.to_string()).await {
        Ok(_) => ok(goal_empty("cleared")),
        Err(err) => Err(api_error(
            StatusCode::BAD_GATEWAY,
            &format!("thread/goal/clear failed: {err}"),
        )),
    }
}

async fn panel_update_precheck(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let id = state.jobs.start_shell_job(
        "panel_update_precheck",
        "Panel update precheck",
        state.config.update.panel_precheck_command.clone(),
    )?;
    ok(json!({"job_id": id}))
}

async fn panel_update_start(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "panel.update.started",
        Some("system"),
        Some("panel"),
        None,
        json!({}),
    )?;
    let id = state.jobs.start_shell_job(
        "panel_update_start",
        "Panel update latest",
        update::panel_update_command(&state.config.update.panel_update_command),
    )?;
    ok(json!({"job_id": id}))
}

async fn panel_update_prune(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "panel.update.prune_started",
        Some("system"),
        Some("panel"),
        None,
        json!({}),
    )?;
    let id = state.jobs.start_shell_job(
        "panel_update_prune",
        "Panel backup prune",
        update::panel_prune_command(),
    )?;
    ok(json!({"job_id": id}))
}

async fn codex_update_precheck(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let id = state
        .jobs
        .start_exclusive_shell_job(
            "codex_update_precheck",
            "Codex update precheck",
            state.config.update.precheck_command.clone(),
            "codex_update",
        )
        .map_err(|err| api_error(StatusCode::CONFLICT, &err.to_string()))?;
    ok(json!({"job_id": id}))
}

async fn codex_update_start(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "system.update.started",
        Some("system"),
        Some("codex"),
        None,
        json!({}),
    )?;
    let id = state
        .jobs
        .start_exclusive_shell_job(
            "codex_update_start",
            "Codex update + prune + doctor",
            state.config.update.update_command.clone(),
            "codex_update",
        )
        .map_err(|err| api_error(StatusCode::CONFLICT, &err.to_string()))?;
    ok(json!({"job_id": id}))
}

async fn codex_update_prune(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "system.update.prune_started",
        Some("system"),
        Some("codex"),
        None,
        json!({}),
    )?;
    let id = state
        .jobs
        .start_exclusive_shell_job(
            "codex_update_prune",
            "Codex release prune",
            state.config.update.prune_command.clone(),
            "codex_update",
        )
        .map_err(|err| api_error(StatusCode::CONFLICT, &err.to_string()))?;
    ok(json!({"job_id": id}))
}

async fn archive_delete_dry_run(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let paths = CodexPaths::new(&state.config.codex.home);
    ok(archive::plan_delete_archived(&paths)?)
}

#[derive(Debug, Deserialize)]
struct ArchiveExecuteRequest {
    confirmed: bool,
}

async fn archive_delete_execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ArchiveExecuteRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    if !payload.confirmed {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "archive deletion must be confirmed",
        ));
    }
    let paths = CodexPaths::new(&state.config.codex.home);
    let result = archive::execute_delete_archived(&paths)?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "archives.delete.execute",
        Some("archives"),
        Some("root-codex"),
        None,
        json!({"before_archived": result.before.archived_threads, "deleted_rollout_files": result.deleted_rollout_files}),
    )?;
    ok(result)
}

async fn hidden_threads_delete_dry_run(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let paths = CodexPaths::new(&state.config.codex.home);
    ok(archive::plan_delete_hidden(&paths)?)
}

async fn hidden_threads_delete_execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ArchiveExecuteRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    if !payload.confirmed {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "hidden thread deletion must be confirmed",
        ));
    }
    let paths = CodexPaths::new(&state.config.codex.home);
    let result = archive::execute_delete_hidden(&paths)?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "hidden_threads.delete.execute",
        Some("hidden_threads"),
        Some("root-codex"),
        None,
        json!({
            "before_hidden": result.before.hidden_threads,
            "deleted_threads": result.deleted_threads,
            "deleted_rollout_files": result.deleted_rollout_files,
        }),
    )?;
    ok(result)
}

async fn list_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let limit = query
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50)
        .min(200);
    ok(job_responses(state.db.list_jobs(limit)?))
}

async fn job_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    match state.db.job(&id)? {
        Some(job) => ok(job_response(job)),
        None => Err(api_error(StatusCode::NOT_FOUND, "job not found")),
    }
}

async fn job_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let mut rx = state.jobs.subscribe();
    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(event) if event.job_id == id => {
                    yield Ok::<Event, std::convert::Infallible>(Event::default().event("job").data(serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string())));
                }
                Ok(_) => {}
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    yield Ok::<Event, std::convert::Infallible>(Event::default().event("job").data(json!({"job_id": id, "status": "lagged"}).to_string()));
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    };
    Ok(Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(25))
                .text("ping"),
        )
        .into_response())
}

fn security_response(state: &AppState) -> anyhow::Result<Value> {
    let security = state
        .db
        .security_settings(state.config.security.session_ttl_seconds)?;
    let expected_hostname = state
        .db
        .get_setting("turnstile_expected_hostname")?
        .or_else(|| state.config.security.turnstile_expected_hostname.clone());
    let expected_action = state
        .db
        .get_setting("turnstile_expected_action")?
        .or_else(|| state.config.security.turnstile_expected_action.clone());
    Ok(json!({
        "turnstile_enabled": security.turnstile_enabled,
        "turnstile_required": security.turnstile_required,
        "turnstile_site_key": security.turnstile_site_key.unwrap_or_else(|| nexushub_core::config::DEFAULT_TURNSTILE_SITE_KEY.to_string()),
        "turnstile_secret_configured": security.turnstile_secret_configured,
        "session_ttl_seconds": security.session_ttl_seconds,
        "turnstile_expected_hostname": expected_hostname,
        "turnstile_expected_action": expected_action,
    }))
}

fn archived_filter(status: Option<&str>) -> Option<bool> {
    match status {
        Some("archived") => Some(true),
        Some("all") | None => Some(false),
        _ => Some(false),
    }
}

fn requested_thread_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(80).clamp(1, 500)
}

fn detail_block_limit(limit: Option<usize>, full: Option<bool>) -> Option<usize> {
    if full.unwrap_or(false) {
        None
    } else {
        Some(
            limit
                .unwrap_or(THREAD_DETAIL_DEFAULT_BLOCK_LIMIT)
                .clamp(1, THREAD_DETAIL_MAX_BLOCK_LIMIT),
        )
    }
}

fn block_page_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(THREAD_DETAIL_DEFAULT_BLOCK_LIMIT)
        .clamp(1, THREAD_DETAIL_MAX_BLOCK_LIMIT)
}

fn thread_list_fetch_limit(status: Option<&str>, limit: Option<usize>) -> usize {
    if thread_status_filter_needs_full_scan(status) {
        usize::MAX
    } else {
        requested_thread_limit(limit)
    }
}

fn app_server_thread_list_fetch_limit(status: Option<&str>, limit: Option<usize>) -> usize {
    if thread_status_filter_needs_full_scan(status) {
        500
    } else {
        requested_thread_limit(limit)
    }
}

fn thread_status_filter_needs_full_scan(status: Option<&str>) -> bool {
    matches!(status, Some("running" | "reply-needed" | "recoverable"))
}

fn filter_thread_summaries(
    mut rows: Vec<ThreadSummary>,
    status: Option<&str>,
    q: Option<&str>,
    limit: usize,
) -> Vec<ThreadSummary> {
    let needle = q
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    rows.retain(|row| {
        if let Some(status) = status {
            if status != "all" && !thread_matches_status(row, status) {
                return false;
            }
        }
        if !matches!(status, Some("archived")) && matches!(row.status, ThreadStatus::Archived) {
            return false;
        }
        if let Some(needle) = &needle {
            row.id.to_ascii_lowercase().contains(needle)
                || row.title.to_ascii_lowercase().contains(needle)
                || row
                    .latest_message
                    .as_deref()
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .contains(needle)
        } else {
            true
        }
    });
    rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    rows.truncate(limit.max(1));
    rows
}

fn prune_hidden_thread_summaries(
    rows: Vec<ThreadSummary>,
    hidden_thread_ids: &HashSet<String>,
) -> Vec<ThreadSummary> {
    if hidden_thread_ids.is_empty() {
        return rows;
    }
    rows.into_iter()
        .filter(|row| !hidden_thread_ids.contains(&row.id))
        .collect()
}

fn merge_thread_summaries(
    fallback: Vec<ThreadSummary>,
    app_threads: Vec<ThreadSummary>,
) -> Vec<ThreadSummary> {
    let fallback_by_id: HashMap<String, ThreadSummary> = fallback
        .iter()
        .cloned()
        .map(|thread| (thread.id.clone(), thread))
        .collect();
    let mut rows: Vec<ThreadSummary> = app_threads
        .into_iter()
        .map(|mut row| {
            if let Some(fallback) = fallback_by_id.get(&row.id) {
                preserve_fallback_title(&mut row, fallback);
            }
            row
        })
        .collect();
    for thread in fallback {
        if !rows.iter().any(|row| row.id == thread.id) {
            rows.push(thread);
        }
    }
    rows
}

fn preserve_fallback_title(row: &mut ThreadSummary, fallback: &ThreadSummary) {
    if is_placeholder_thread_title(&row.title) && !is_placeholder_thread_title(&fallback.title) {
        row.title = fallback.title.clone();
    }
}

fn is_placeholder_thread_title(title: &str) -> bool {
    let value = title.trim();
    value.is_empty() || matches!(value, "未命名线程" | "Untitled thread" | "Untitled")
}

fn thread_matches_status(row: &ThreadSummary, status: &str) -> bool {
    matches!(
        (status, &row.status),
        ("running", ThreadStatus::Running)
            | ("reply-needed", ThreadStatus::ReplyNeeded)
            | ("recoverable", ThreadStatus::Recoverable)
            | ("archived", ThreadStatus::Archived)
    ) || (status == "recent" && matches!(row.status, ThreadStatus::Recent))
}

fn app_server_thread_summaries(value: &Value, fallback: &[ThreadSummary]) -> Vec<ThreadSummary> {
    let fallback_by_id: HashMap<&str, &ThreadSummary> = fallback
        .iter()
        .map(|thread| (thread.id.as_str(), thread))
        .collect();
    value
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| value.get("threads").and_then(Value::as_array))
        .into_iter()
        .flatten()
        .filter_map(|thread| app_server_thread_summary(thread, &fallback_by_id))
        .collect()
}

fn app_server_thread_visibility_diagnostics(value: &Value) -> (HashMap<String, usize>, usize) {
    let mut counts = HashMap::new();
    let mut hidden = 0;
    for thread in value
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| value.get("threads").and_then(Value::as_array))
        .into_iter()
        .flatten()
    {
        let label = app_server_source_label(thread);
        *counts.entry(label).or_insert(0) += 1;
        if is_app_server_subagent_thread(thread) || thread_archived(thread) {
            hidden += 1;
        }
    }
    (counts, hidden)
}

fn app_server_source_label(thread: &Value) -> String {
    if thread_archived(thread) {
        return "archived".to_string();
    }
    if is_app_server_subagent_thread(thread) {
        return "subagent".to_string();
    }
    for field in ["sourceKind", "source_kind", "threadSource", "thread_source"] {
        if let Some(value) = thread
            .get(field)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return value.to_ascii_lowercase();
        }
    }
    thread
        .get("source")
        .and_then(|source| {
            source
                .get("kind")
                .or_else(|| source.get("type"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| source.as_str().map(str::to_string))
        })
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "unknown".to_string())
}

fn app_server_thread_summary(
    thread: &Value,
    fallback_by_id: &HashMap<&str, &ThreadSummary>,
) -> Option<ThreadSummary> {
    let id = thread.get("id").and_then(Value::as_str)?.to_string();
    let fallback = fallback_by_id.get(id.as_str()).copied();
    if fallback.is_none() && is_app_server_subagent_thread(thread) {
        return None;
    }
    let status = merge_app_thread_status(
        fallback,
        thread,
        fallback_has_pending_signal(fallback),
        fallback_has_running_signal(fallback) || app_thread_has_running_signal(thread),
    );
    let active_turn_id = merged_active_turn_id(fallback, thread, &status);
    let title = thread_title(thread)
        .filter(|title| {
            !is_placeholder_thread_title(title)
                || fallback.is_none_or(|thread| is_placeholder_thread_title(&thread.title))
        })
        .or_else(|| fallback.map(|thread| thread.title.clone()))
        .unwrap_or_else(|| "未命名线程".to_string());
    let mut summary = ThreadSummary {
        id: id.clone(),
        title,
        status: status.clone(),
        updated_at: thread
            .get("updatedAt")
            .and_then(Value::as_i64)
            .and_then(timestamp_to_rfc3339)
            .or_else(|| fallback.and_then(|thread| thread.updated_at.clone())),
        archived_at: thread
            .get("archivedAt")
            .or_else(|| thread.get("archived_at"))
            .and_then(Value::as_i64)
            .and_then(timestamp_to_rfc3339)
            .or_else(|| fallback.and_then(|thread| thread.archived_at.clone())),
        message_count: fallback.map(|thread| thread.message_count).unwrap_or(0),
        latest_message: thread
            .get("preview")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| fallback.and_then(|thread| thread.latest_message.clone())),
        cwd: thread
            .get("cwd")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| fallback.and_then(|thread| thread.cwd.clone())),
        model: thread
            .get("model")
            .or_else(|| thread.get("modelProvider"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| fallback.and_then(|thread| thread.model.clone())),
        rollout_path: app_thread_rollout_path(thread)
            .or_else(|| fallback.and_then(|thread| thread.rollout_path.clone())),
        active_turn_id,
        active_job_id: fallback.and_then(|thread| thread.active_job_id.clone()),
        pending_elicitation: if matches!(status, ThreadStatus::ReplyNeeded | ThreadStatus::Archived)
        {
            fallback.and_then(|thread| thread.pending_elicitation.clone())
        } else {
            None
        },
        last_event_kind: fallback.and_then(|thread| thread.last_event_kind.clone()),
    };
    if !matches!(app_thread_state(thread), AppThreadState::Recoverable)
        && !thread_archived(thread)
        && summary.rollout_path.is_some()
    {
        let rollout_enriched = codex::enrich_thread_from_rollout(&mut summary).is_ok();
        let pending_signal = fallback_has_pending_signal(Some(&summary));
        let running_signal = fallback_has_running_signal(Some(&summary))
            || (rollout_enriched && matches!(summary.status, ThreadStatus::Running))
            || app_thread_has_running_signal(thread);
        summary.status =
            merge_app_thread_status(Some(&summary), thread, pending_signal, running_signal);
        summary.active_turn_id = merged_active_turn_id(Some(&summary), thread, &summary.status);
        if !matches!(
            summary.status,
            ThreadStatus::ReplyNeeded | ThreadStatus::Archived
        ) {
            summary.pending_elicitation = None;
        }
    }
    Some(summary)
}

fn apply_app_server_thread_detail(detail: &mut ThreadDetail, value: &Value) {
    let Some(thread) = value.get("thread") else {
        return;
    };
    if let Some(title) = thread_title(thread) {
        if !is_placeholder_thread_title(&title)
            || is_placeholder_thread_title(&detail.summary.title)
        {
            detail.summary.title = title;
        }
    }
    if let Some(updated_at) = thread
        .get("updatedAt")
        .and_then(Value::as_i64)
        .and_then(timestamp_to_rfc3339)
    {
        detail.summary.updated_at = Some(updated_at);
    }
    if let Some(cwd) = thread.get("cwd").and_then(Value::as_str) {
        detail.summary.cwd = Some(cwd.to_string());
    }
    if let Some(preview) = thread.get("preview").and_then(Value::as_str) {
        detail.summary.latest_message = Some(preview.to_string());
    }
    if let Some(model) = thread
        .get("model")
        .or_else(|| thread.get("modelProvider"))
        .and_then(Value::as_str)
    {
        detail.summary.model = Some(model.to_string());
    }
    if detail.summary.rollout_path.is_none() {
        detail.summary.rollout_path = app_thread_rollout_path(thread);
    }
    let mut rollout_enriched = false;
    if detail.summary.rollout_path.is_some() {
        rollout_enriched = codex::enrich_thread_from_rollout(&mut detail.summary).is_ok();
    }
    let pending_turn_id = detail_pending_turn_id(detail);
    let pending_signal = detail_has_pending_signal(detail) || pending_turn_id.as_deref().is_some();
    let running_signal = detail_has_running_signal(detail)
        || (rollout_enriched && matches!(detail.summary.status, ThreadStatus::Running))
        || app_thread_has_running_signal(thread);
    let status = merge_app_thread_status(
        Some(&detail.summary),
        thread,
        pending_signal,
        running_signal,
    );
    let active_turn_id = merged_active_turn_id(Some(&detail.summary), thread, &status)
        .or_else(|| detail_active_turn_id(detail))
        .or(pending_turn_id);
    detail.summary.active_turn_id = match status {
        ThreadStatus::Recent => None,
        _ => active_turn_id,
    };
    if !matches!(status, ThreadStatus::ReplyNeeded | ThreadStatus::Archived) {
        detail.summary.pending_elicitation = None;
    }
    detail.summary.status = status;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppThreadState {
    Active,
    Recoverable,
    Idle,
    NotLoaded,
    Unknown,
}

fn merge_app_thread_status(
    fallback: Option<&ThreadSummary>,
    thread: &Value,
    pending_signal: bool,
    running_signal: bool,
) -> ThreadStatus {
    if fallback.is_some_and(|thread| matches!(thread.status, ThreadStatus::Archived))
        || thread_archived(thread)
    {
        return ThreadStatus::Archived;
    }
    match app_thread_state(thread) {
        AppThreadState::Active => ThreadStatus::Running,
        AppThreadState::Recoverable => ThreadStatus::Recoverable,
        AppThreadState::Idle => {
            if running_signal {
                ThreadStatus::Running
            } else if pending_signal {
                ThreadStatus::ReplyNeeded
            } else {
                ThreadStatus::Recent
            }
        }
        AppThreadState::NotLoaded => {
            if running_signal {
                ThreadStatus::Running
            } else if pending_signal {
                ThreadStatus::ReplyNeeded
            } else if fallback.is_some_and(|summary| {
                fallback_has_clearable_stale_status(summary)
                    && app_thread_has_fallback_rollout_path(thread)
            }) {
                ThreadStatus::Recent
            } else {
                fallback_stable_status(fallback)
            }
        }
        AppThreadState::Unknown => {
            if running_signal {
                ThreadStatus::Running
            } else if pending_signal {
                ThreadStatus::ReplyNeeded
            } else if fallback.is_some_and(fallback_has_clearable_stale_status) {
                ThreadStatus::Recent
            } else {
                fallback_stable_status(fallback)
            }
        }
    }
}

fn fallback_stable_status(fallback: Option<&ThreadSummary>) -> ThreadStatus {
    match fallback.map(|thread| &thread.status) {
        Some(ThreadStatus::Archived) => ThreadStatus::Archived,
        Some(ThreadStatus::Recoverable) => ThreadStatus::Recoverable,
        Some(ThreadStatus::Running | ThreadStatus::ReplyNeeded) | None => ThreadStatus::Recent,
        Some(status) => status.clone(),
    }
}

fn fallback_has_clearable_stale_status(summary: &ThreadSummary) -> bool {
    matches!(
        summary.status,
        ThreadStatus::Running | ThreadStatus::ReplyNeeded | ThreadStatus::Recoverable
    ) && summary.rollout_path.is_some()
        && summary.active_turn_id.is_none()
        && summary.active_job_id.is_none()
        && summary.pending_elicitation.is_none()
}

fn merged_active_turn_id(
    fallback: Option<&ThreadSummary>,
    thread: &Value,
    status: &ThreadStatus,
) -> Option<String> {
    let active_turn_id = app_thread_active_turn_id(thread)
        .or_else(|| fallback.and_then(|thread| thread.active_turn_id.clone()));
    match status {
        ThreadStatus::Running
        | ThreadStatus::ReplyNeeded
        | ThreadStatus::Recoverable
        | ThreadStatus::Archived => active_turn_id,
        ThreadStatus::Recent => None,
    }
}

fn fallback_has_pending_signal(fallback: Option<&ThreadSummary>) -> bool {
    let Some(summary) = fallback else {
        return false;
    };
    summary.active_turn_id.is_some() && summary.pending_elicitation.is_some()
}

fn fallback_has_running_signal(fallback: Option<&ThreadSummary>) -> bool {
    let Some(summary) = fallback else {
        return false;
    };
    summary.active_job_id.is_some()
}

fn detail_has_pending_signal(detail: &ThreadDetail) -> bool {
    if fallback_has_pending_signal(Some(&detail.summary)) {
        return true;
    }
    let Some(active_turn_id) = detail.summary.active_turn_id.as_deref() else {
        return false;
    };
    detail.blocks.iter().any(|block| {
        block.turn_id.as_deref() == Some(active_turn_id) && block_has_pending_signal(block)
    })
}

fn detail_pending_turn_id(detail: &ThreadDetail) -> Option<String> {
    let mut pending: Option<&MessageBlock> = None;
    for block in &detail.blocks {
        if block_has_pending_signal(block) {
            pending = Some(block);
            continue;
        }
        if pending.is_some_and(|pending| block_clears_pending_signal(block, pending)) {
            pending = None;
        }
    }
    pending.and_then(|block| block.turn_id.clone())
}

fn block_clears_pending_signal(block: &MessageBlock, pending: &MessageBlock) -> bool {
    if block_has_pending_signal(block) {
        return false;
    }
    if let Some(expected) = pending.call_id.as_deref() {
        if block.call_id.as_deref() == Some(expected) {
            return true;
        }
    }
    if let Some(expected) = pending.item_id.as_deref() {
        if block.item_id.as_deref() == Some(expected) {
            return true;
        }
    }
    matches!(block.role.as_str(), "user" | "assistant" | "tool")
}

fn detail_has_running_signal(detail: &ThreadDetail) -> bool {
    fallback_has_running_signal(Some(&detail.summary))
        || detail.blocks.iter().any(block_has_running_signal)
}

fn detail_active_turn_id(detail: &ThreadDetail) -> Option<String> {
    detail
        .blocks
        .iter()
        .rev()
        .find(|block| block_has_running_signal(block))
        .and_then(|block| block.turn_id.clone())
}

fn block_has_pending_signal(block: &MessageBlock) -> bool {
    let kind = block.kind.as_str();
    if kind == "request_user_input" {
        return !block.questions.is_empty();
    }
    if kind == "plan" {
        return block
            .text
            .as_deref()
            .is_some_and(|text| text.contains("<proposed_plan>"));
    }
    kind.contains("approval")
        && block.status.as_deref().is_none_or(|status| {
            matches!(status, "pending" | "running" | "in_progress" | "inProgress")
        })
}

fn block_has_running_signal(block: &MessageBlock) -> bool {
    if block_has_pending_signal(block) {
        return false;
    }
    let status = block.status.as_deref().unwrap_or_default();
    let running_status = matches!(
        status,
        "pending" | "running" | "in_progress" | "inProgress" | "active"
    );
    running_status
        && (block.role == "tool"
            || block.kind.contains("function_call")
            || block.kind.contains("tool")
            || block.kind.contains("command"))
}

fn app_thread_state(thread: &Value) -> AppThreadState {
    match app_thread_status_text(thread) {
        Some("notLoaded" | "not_loaded") => AppThreadState::NotLoaded,
        Some("active" | "running" | "in_progress" | "inProgress" | "generating") => {
            AppThreadState::Active
        }
        Some("systemError" | "system_error" | "recoverable" | "error") => {
            AppThreadState::Recoverable
        }
        Some(
            "idle" | "recent" | "inactive" | "completed" | "complete" | "done" | "finished"
            | "stopped" | "canceled" | "cancelled" | "interrupted" | "success" | "succeeded",
        ) => AppThreadState::Idle,
        Some(_) | None => AppThreadState::Unknown,
    }
}

fn app_thread_status_text(thread: &Value) -> Option<&str> {
    thread
        .get("status")
        .and_then(|status| status.get("type").or(Some(status)))
        .and_then(Value::as_str)
}

fn app_thread_active_turn_id(thread: &Value) -> Option<String> {
    thread
        .pointer("/status/turnId")
        .or_else(|| thread.pointer("/status/turn_id"))
        .or_else(|| thread.get("activeTurnId"))
        .or_else(|| thread.get("active_turn_id"))
        .or_else(|| thread.get("turnId"))
        .or_else(|| thread.get("turn_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| app_thread_turns_active_turn_id(thread))
}

fn app_thread_has_running_signal(thread: &Value) -> bool {
    app_thread_active_turn_id(thread).is_some()
}

fn app_thread_has_fallback_rollout_path(thread: &Value) -> bool {
    app_thread_rollout_path(thread).is_some()
}

fn app_thread_turns_active_turn_id(thread: &Value) -> Option<String> {
    thread
        .get("turns")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .rev()
        .find_map(|turn| {
            let id = turn
                .get("id")
                .or_else(|| turn.get("turnId"))
                .or_else(|| turn.get("turn_id"))
                .and_then(Value::as_str)?;
            if app_thread_state(turn) == AppThreadState::Active || turn_has_running_item(turn) {
                Some(id.to_string())
            } else {
                None
            }
        })
}

fn app_server_detail_from_read(value: &Value) -> Option<ThreadDetail> {
    let thread = value.get("thread")?;
    let fallback_by_id = HashMap::new();
    let mut summary = app_server_thread_summary(thread, &fallback_by_id)?;
    if summary.rollout_path.is_some() {
        let _ = codex::enrich_thread_from_rollout(&mut summary);
    }
    let mut detail = codex::thread_detail_from_summary(summary).ok()?;
    if detail.blocks.is_empty() {
        let events = app_server_thread_item_events(thread);
        if !events.is_empty() {
            detail.raw_event_count = events.len();
            detail.blocks = codex::message_blocks_from_events(events.iter());
            detail.total_blocks = detail.blocks.len();
            detail.has_more_blocks = false;
            detail.before_cursor = None;
        }
    }
    apply_app_server_thread_detail(&mut detail, value);
    Some(detail)
}

fn app_server_thread_item_events(thread: &Value) -> Vec<Value> {
    thread
        .get("turns")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|turn| {
            let turn_id = turn
                .get("id")
                .or_else(|| turn.get("turnId"))
                .or_else(|| turn.get("turn_id"))
                .and_then(Value::as_str)
                .map(str::to_string);
            turn.get("items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .flat_map(move |item| app_server_item_events(turn_id.as_deref(), item))
        })
        .collect()
}

fn app_server_item_events(turn_id: Option<&str>, item: &Value) -> Vec<Value> {
    let Some(payload) = normalize_app_server_item_payload(item) else {
        return Vec::new();
    };
    let item_type = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut event = json!({
        "type": "response_item",
        "payload": payload,
    });
    if let Some(turn_id) = turn_id {
        event["turn_id"] = Value::String(turn_id.to_string());
    }
    if item_type.eq_ignore_ascii_case("plan") {
        let mut marker = json!({
            "type": "item_completed",
            "item": { "type": "Plan" },
        });
        if let Some(turn_id) = turn_id {
            marker["turn_id"] = Value::String(turn_id.to_string());
        }
        if let Some(item_id) = item
            .get("id")
            .or_else(|| item.get("itemId"))
            .or_else(|| item.get("item_id"))
            .and_then(Value::as_str)
        {
            marker["item"]["id"] = Value::String(item_id.to_string());
        }
        vec![marker, event]
    } else {
        vec![event]
    }
}

fn normalize_app_server_item_payload(item: &Value) -> Option<Value> {
    let item_type = item.get("type").and_then(Value::as_str)?;
    match item_type {
        "agentMessage" | "assistantMessage" => item_text(item).map(|text| {
            json!({
                "type": "message",
                "role": "assistant",
                "id": item_id(item),
                "content": [{ "text": text }]
            })
        }),
        "userMessage" => item_text(item).map(|text| {
            json!({
                "type": "message",
                "role": "user",
                "id": item_id(item),
                "content": [{ "text": text }]
            })
        }),
        "plan" | "Plan" => item_text(item).map(|text| {
            json!({
                "type": "Plan",
                "id": item_id(item),
                "text": text,
                "status": item.get("status").cloned().unwrap_or(Value::Null)
            })
        }),
        "requestUserInput" | "request_user_input" | "toolRequestUserInput" => {
            let questions = item
                .get("questions")
                .or_else(|| item.pointer("/params/questions"))
                .cloned()
                .unwrap_or(Value::Null);
            Some(json!({
                "type": "function_call",
                "name": "request_user_input",
                "id": item_id(item),
                "call_id": item_id(item),
                "turn_id": item
                    .get("turnId")
                    .or_else(|| item.get("turn_id"))
                    .cloned()
                    .unwrap_or(Value::Null),
                "arguments": { "questions": questions },
                "status": item.get("status").cloned().unwrap_or(Value::Null)
            }))
        }
        _ => Some(item.clone()),
    }
}

fn item_text(item: &Value) -> Option<String> {
    item.get("text")
        .or_else(|| item.get("message"))
        .or_else(|| item.get("content"))
        .or_else(|| item.get("aggregatedText"))
        .and_then(|value| match value {
            Value::String(text) => Some(text.clone()),
            Value::Array(items) => {
                let text = items
                    .iter()
                    .filter_map(|item| {
                        item.get("text")
                            .or_else(|| item.get("input_text"))
                            .and_then(Value::as_str)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                (!text.trim().is_empty()).then_some(text)
            }
            _ => None,
        })
        .filter(|text| !text.trim().is_empty())
}

fn item_id(item: &Value) -> Option<String> {
    item.get("id")
        .or_else(|| item.get("itemId"))
        .or_else(|| item.get("item_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn turn_has_running_item(turn: &Value) -> bool {
    turn.get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|item| {
            item.get("status")
                .and_then(|status| status.get("type").or(Some(status)))
                .and_then(Value::as_str)
                .is_some_and(|status| {
                    matches!(
                        status,
                        "active" | "running" | "in_progress" | "inProgress" | "pending"
                    )
                })
        })
}

fn app_thread_rollout_path(thread: &Value) -> Option<std::path::PathBuf> {
    thread
        .get("path")
        .or_else(|| thread.get("rollout_path"))
        .or_else(|| thread.get("rolloutPath"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .filter(|path| path.ends_with(".jsonl") || path.contains("rollout-"))
        .map(std::path::PathBuf::from)
}

fn thread_event_block_key(block: &MessageBlock) -> String {
    serde_json::to_string(block).unwrap_or_else(|_| {
        format!(
            "{}:{}:{}:{}:{}:{}",
            block.id,
            block.kind,
            block.status.as_deref().unwrap_or_default(),
            block.summary.as_deref().unwrap_or_default(),
            block.text.as_deref().unwrap_or_default(),
            block.input.as_deref().unwrap_or_default()
        )
    })
}

fn block_changed(previous: Option<&String>, block: &MessageBlock) -> bool {
    previous.is_none_or(|previous| previous != &thread_event_block_key(block))
}

fn seed_thread_event_blocks(sent_blocks: &mut HashMap<String, String>, blocks: &[MessageBlock]) {
    for block in blocks {
        sent_blocks.insert(block.id.clone(), thread_event_block_key(block));
    }
}

fn thread_title(thread: &Value) -> Option<String> {
    ["name", "title"].into_iter().find_map(|field| {
        thread
            .get(field)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn is_app_server_subagent_thread(thread: &Value) -> bool {
    has_non_empty_string(thread, &["parentThreadId", "parent_thread_id"])
        || has_non_empty_string(thread, &["agentPath", "agent_path"])
        || has_non_empty_string(thread, &["agentNickname", "agent_nickname"])
        || has_non_empty_string(thread, &["agentRole", "agent_role"])
        || field_contains_subagent(
            thread,
            &["sourceKind", "source_kind", "threadSource", "thread_source"],
        )
        || source_value_contains_subagent(thread.get("source"))
}

fn has_non_empty_string(value: &Value, fields: &[&str]) -> bool {
    fields.iter().any(|field| {
        value
            .get(*field)
            .and_then(Value::as_str)
            .is_some_and(|text| !text.trim().is_empty())
    })
}

fn field_contains_subagent(value: &Value, fields: &[&str]) -> bool {
    fields.iter().any(|field| {
        value
            .get(*field)
            .and_then(Value::as_str)
            .is_some_and(|text| text.to_ascii_lowercase().contains("subagent"))
    })
}

fn source_value_contains_subagent(source: Option<&Value>) -> bool {
    match source {
        Some(Value::String(text)) => text.to_ascii_lowercase().contains("subagent"),
        Some(Value::Array(items)) => items
            .iter()
            .any(|item| source_value_contains_subagent(Some(item))),
        Some(Value::Object(map)) => map.iter().any(|(key, value)| {
            key.to_ascii_lowercase().contains("subagent")
                || source_value_contains_subagent(Some(value))
        }),
        _ => false,
    }
}

fn thread_archived(thread: &Value) -> bool {
    thread
        .get("archived")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || thread
            .get("archivedAt")
            .or_else(|| thread.get("archived_at"))
            .is_some_and(|value| !value.is_null())
        || thread
            .get("path")
            .and_then(Value::as_str)
            .map(|path| path.contains("/archived/") || path.contains("/archived_sessions/"))
            .unwrap_or(false)
}

fn timestamp_to_rfc3339(value: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(value, 0).map(|dt| dt.to_rfc3339())
}

fn job_responses(jobs: Vec<JobRecord>) -> Vec<Value> {
    jobs.into_iter().map(job_response).collect()
}

fn job_response(job: JobRecord) -> Value {
    let analysis = if job.status == "failed" {
        update::analyze_job_failure(&job.kind, &job.output, job.error.as_deref(), job.exit_code)
    } else {
        None
    };
    let mut value = serde_json::to_value(&job).unwrap_or_else(|_| json!({}));
    if let Some(analysis) = analysis {
        value["failure_analysis"] = serde_json::to_value(&analysis).unwrap_or(Value::Null);
        value["analysis"] = Value::String(analysis.explanation.clone());
        value["explanation"] = Value::String(analysis.suggestions.join(" "));
    }
    value
}

fn normalize_config_response(value: &Value, state: &AppState) -> Value {
    let config = value.get("config").unwrap_or(value);
    json!({
        "model": config.get("model").and_then(Value::as_str),
        "reasoning_effort": config.get("model_reasoning_effort").or_else(|| config.get("reasoning_effort")).and_then(Value::as_str),
        "cwd": config.get("cwd").and_then(Value::as_str).unwrap_or_else(|| state.config.codex.workspace.to_str().unwrap_or("/home/ubuntu/codex-workspace")),
        "permission_profile": config.get("default_permissions").or_else(|| config.get("permissions")).and_then(Value::as_str),
        "approval_policy": config.get("approval_policy").and_then(Value::as_str),
        "sandbox_mode": config.get("sandbox_mode").and_then(Value::as_str),
        "network_access": config.get("sandbox_workspace_write").and_then(|value| value.get("network_access")).and_then(Value::as_bool),
        "raw": value,
    })
}

fn normalize_goal_response(value: &Value) -> Value {
    let goal = value.get("goal").unwrap_or(value);
    if goal.is_null() {
        return goal_empty("idle");
    }
    json!({
        "enabled": true,
        "objective": goal.get("objective").and_then(Value::as_str),
        "token_budget": goal.get("tokenBudget").or_else(|| goal.get("token_budget")).and_then(Value::as_u64),
        "status": goal.get("status").and_then(Value::as_str).unwrap_or("active"),
        "raw": value,
    })
}

fn goal_empty(status: &str) -> Value {
    json!({
        "enabled": false,
        "objective": null,
        "token_budget": null,
        "status": status,
    })
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn cli_config_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn client_ip(headers: &HeaderMap, state: &AppState, source: Option<SocketAddr>) -> Option<String> {
    if state.config.server.trust_forwarded_headers {
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
enum TurnstileLoginAction {
    Skip,
    Verify,
    FailClosed,
}

fn turnstile_login_action(enabled: bool, required: bool) -> TurnstileLoginAction {
    if enabled {
        TurnstileLoginAction::Verify
    } else if required {
        TurnstileLoginAction::FailClosed
    } else {
        TurnstileLoginAction::Skip
    }
}

fn ok<T: Serialize>(value: T) -> ApiResponse {
    Ok(Json(value).into_response())
}

fn api_error(status: StatusCode, message: &str) -> ApiError {
    ApiError(Box::new(
        (status, Json(json!({ "error": message }))).into_response(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        app_server_detail_from_read, app_server_thread_list_fetch_limit,
        app_server_thread_summaries, apply_app_server_thread_detail, apply_running_job_to_summary,
        archived_filter, block_changed, effective_message, filter_thread_summaries,
        followup_request, merge_thread_summaries, prune_hidden_thread_summaries,
        requested_thread_limit, router, seed_thread_event_blocks, thread_block_page,
        thread_event_block_key, thread_list_fetch_limit, thread_title, turnstile_login_action,
        SendMessageRequest, TurnstileLoginAction,
    };
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use nexushub_core::codex::{MessageBlock, ThreadDetail, ThreadStatus, ThreadSummary};
    use nexushub_core::{
        config::Config,
        db::{JobRecord, NewSession, PanelDb, ThreadFollowUp},
        uploads::{PreparedAttachment, UploadKind},
    };
    use serde_json::json;
    use std::{
        collections::HashMap,
        env, fs,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };
    use tower::ServiceExt;

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn authenticated_test_state() -> (crate::state::AppState, String, String) {
        let mut config = Config::default();
        config.security.cookie_secure = false;
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
            expires_at: PanelDb::now() + 60,
        })
        .unwrap();

        (
            crate::state::AppState::new(config, db),
            "session-token".to_string(),
            "csrf-token".to_string(),
        )
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
                truncated: false,
            },
        ];
        let request = SendMessageRequest {
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
            message: effective_message(&request.message, &request.prepared_attachments),
            options_json: request.options_json().to_string(),
            created_at: 1,
            updated_at: 1,
            submitted_at: None,
            cancelled_at: None,
            result_json: None,
            error: None,
        };

        let restored = followup_request(&followup);

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

    #[tokio::test]
    async fn panel_prune_route_requires_csrf_and_starts_fixed_panel_job() {
        let (state, session_token, csrf_token) = authenticated_test_state();
        let app = router(state.clone());

        let missing_csrf = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/system/panel/update/prune")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/system/panel/update/prune")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .header("x-csrf-token", csrf_token)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let job_id = payload["job_id"].as_str().unwrap();
        let job = state.db.job(job_id).unwrap().unwrap();

        assert_eq!(job.kind, "panel_update_prune");
        assert_eq!(job.title, "Panel backup prune");
    }

    #[tokio::test]
    async fn probe_status_routes_use_canonical_probe_name_and_keep_sentinel_alias() {
        let (state, session_token, _) = authenticated_test_state();
        let app = router(state);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/probe/status")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(status["label"], "Probe");
        assert_ne!(status["label"], "Sentinel");
        assert!(status["flavor"].as_str().is_some());
        assert!(status["hook_status"].as_str().is_some());

        let alias = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/sentinel/status")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(alias.status(), StatusCode::OK);
        let body = to_bytes(alias.into_body(), usize::MAX).await.unwrap();
        let alias_status: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(alias_status["label"], "Probe");
        assert_eq!(alias_status["flavor"], status["flavor"]);
    }

    #[tokio::test]
    async fn probe_fixed_job_routes_require_csrf_and_start_known_jobs() {
        let (state, session_token, csrf_token) = authenticated_test_state();
        let app = router(state.clone());

        let missing_csrf = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/probe/hooks/install")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

        for (uri, kind, title) in [
            (
                "/api/probe/hooks/install",
                "probe_hooks_install",
                "Probe install hooks",
            ),
            ("/api/probe/bark/test", "probe_bark_test", "Probe Bark test"),
            (
                "/api/probe/logs-db/maintain",
                "probe_logs_db_maintain",
                "Probe logs DB maintain",
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(uri)
                        .header("cookie", format!("nexushub_session={session_token}"))
                        .header("x-csrf-token", csrf_token.as_str())
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let job_id = payload["job_id"].as_str().unwrap();
            let job = state.db.job(job_id).unwrap().unwrap();
            assert_eq!(job.kind, kind);
            assert_eq!(job.title, title);
        }
    }

    #[tokio::test]
    async fn claude_code_fixed_job_routes_require_csrf_and_start_known_jobs() {
        let (state, session_token, csrf_token) = authenticated_test_state();
        let app = router(state.clone());

        let missing_csrf = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/providers/claude-code/jobs/smoke")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

        for (uri, kind, title) in [
            (
                "/api/providers/claude-code/jobs/version-check",
                "claude_code_version_check",
                "Claude Code version check",
            ),
            (
                "/api/providers/claude-code/jobs/update/precheck",
                "claude_code_update_precheck",
                "Claude Code update precheck",
            ),
            (
                "/api/providers/claude-code/jobs/update/start",
                "claude_code_update_start",
                "Claude Code update",
            ),
            (
                "/api/providers/claude-code/jobs/smoke",
                "claude_code_smoke",
                "Claude Code smoke",
            ),
            (
                "/api/providers/claude-code/jobs/cache-status",
                "claude_code_cache_status",
                "Claude Code cache/log status",
            ),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(uri)
                        .header("cookie", format!("nexushub_session={session_token}"))
                        .header("x-csrf-token", csrf_token.as_str())
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let job_id = payload["job_id"].as_str().unwrap();
            let job = state.db.job(job_id).unwrap().unwrap();
            assert_eq!(job.kind, kind);
            assert_eq!(job.title, title);
        }
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

        let page = thread_block_page("thread-a", detail, Some(2), None);
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

        let page = thread_block_page("thread-a", detail, Some(2), Some("b:4"));

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
    fn hidden_state_db_thread_ids_prune_app_server_rows_after_merge() {
        let fallback = vec![fallback_summary("main-thread", "wanka")];
        let app_threads = vec![
            fallback_summary("main-thread", "app main"),
            fallback_summary("child-thread", "subagent child"),
        ];
        let rows = merge_thread_summaries(fallback, app_threads);
        let hidden = ["child-thread".to_string()].into_iter().collect();

        let rows = prune_hidden_thread_summaries(rows, &hidden);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "main-thread");
    }

    #[test]
    fn filter_thread_summaries_excludes_archived_for_default_and_all_status() {
        let recent = fallback_summary("recent-thread", "recent");
        let mut archived = fallback_summary("archived-thread", "archived");
        archived.status = ThreadStatus::Archived;

        let default_rows =
            filter_thread_summaries(vec![recent.clone(), archived.clone()], None, None, 10);
        let all_rows = filter_thread_summaries(vec![recent, archived], Some("all"), None, 10);

        assert_eq!(default_rows.len(), 1);
        assert_eq!(default_rows[0].id, "recent-thread");
        assert_eq!(all_rows.len(), 1);
        assert_eq!(all_rows[0].id, "recent-thread");
        assert_eq!(archived_filter(None), Some(false));
        assert_eq!(archived_filter(Some("all")), Some(false));
    }

    #[test]
    fn status_filtered_thread_lists_overfetch_before_final_limit() {
        assert_eq!(requested_thread_limit(Some(120)), 120);
        assert_eq!(requested_thread_limit(Some(0)), 1);
        assert_eq!(requested_thread_limit(Some(900)), 500);
        assert_eq!(thread_list_fetch_limit(None, Some(120)), 120);
        assert_eq!(thread_list_fetch_limit(Some("all"), Some(120)), 120);
        assert_eq!(
            thread_list_fetch_limit(Some("running"), Some(120)),
            usize::MAX
        );
        assert_eq!(
            thread_list_fetch_limit(Some("reply-needed"), Some(120)),
            usize::MAX
        );
        assert_eq!(
            thread_list_fetch_limit(Some("recoverable"), Some(120)),
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

        apply_running_job_to_summary(&mut summary, &job);

        assert_eq!(summary.status, ThreadStatus::Running);
        assert_eq!(summary.active_job_id.as_deref(), Some("job-live"));
        assert_eq!(summary.last_event_kind, None);
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
    fn app_server_not_loaded_completed_rollout_clears_stale_fallback_running_when_path_is_current()
    {
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
}
