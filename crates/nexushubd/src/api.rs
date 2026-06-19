use crate::{
    auth::{
        expired_session_cookie, expires_at, random_token, require_auth, require_csrf,
        session_cookie, verify_password, SESSION_COOKIE,
    },
    state::{
        AppState, CachedProbeStatus, CachedThreadDetail, FileSignature, ThreadDetailCacheSignature,
    },
    turnstile::verify_turnstile,
};
use anyhow::Result as AnyhowResult;
use axum::{
    extract::connect_info::ConnectInfo,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{any, delete, get, post},
    Json, Router,
};
use nexushub_core::{
    archive,
    claude_code::{self, ClaudePaths},
    codex::{self, CodexPaths, MessageBlock, ThreadDetail, ThreadStatus, ThreadSummary},
    config::{patch_probe_config_toml, Config},
    db::{JobRecord, NewSession, ThreadFollowUp, ThreadGoal, ThreadGoalUpdate},
    jobs::CodexActionResult,
    local,
    platform::{PlatformKind, PlatformPaths},
    probe::{redact_probe_event_for_output, ProbeRuntime},
    providers::ProviderRegistry,
    services::{
        jobs as job_service,
        probe::probe_threads_for_status_with_paths,
        settings as settings_service,
        threads::{
            apply_running_job_to_summary, build_threads_overview, filter_thread_summaries,
            merge_running_jobs, prune_hidden_thread_summaries, thread_list_fetch_limit,
            ThreadsQuery,
        },
        updates::{self as update_service, UpdateAction},
    },
    update,
    uploads::{
        self, cleanup_upload_ids, prepare_uploads, prompt_with_attachment_context,
        PreparedAttachment, MAX_TOTAL_UPLOAD_BYTES, MAX_UPLOAD_FILES, MAX_UPLOAD_FILE_BYTES,
    },
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    net::SocketAddr,
    path::{Path as FsPath, PathBuf},
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
        .route(
            "/api/probe/settings",
            get(get_probe_settings).patch(patch_probe_settings),
        )
        .route("/api/probe/diagnostics", get(get_probe_diagnostics))
        .route("/api/probe/running", get(get_probe_running))
        .route("/api/probe/reply-needed", get(get_probe_reply_needed))
        .route("/api/probe/recoverable", get(get_probe_recoverable))
        .route("/api/probe/lifecycle", get(get_probe_lifecycle))
        .route("/api/probe/hook-status", get(get_probe_hook_status))
        .route("/api/probe/hooks/install", post(probe_hooks_install))
        .route("/api/probe/events", get(get_probe_events))
        .route("/api/probe/logs-db/status", get(get_probe_logs_db_status))
        .route("/api/probe/logs-db/maintain", post(probe_logs_db_maintain))
        .route("/api/probe/bark/test", post(probe_bark_test))
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
        .route("/api/system/update/status", get(system_update_status))
        .route("/api/system/update/precheck", post(system_update_precheck))
        .route("/api/system/update/install", post(system_update_install))
        .route("/api/system/update/prune", post(system_update_prune))
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
        .route("/api/codex/goal/pause", post(codex_goal_pause))
        .route("/api/codex/goal/resume", post(codex_goal_resume))
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
        .route("/api/rpc/threadEvents/:id", get(thread_events))
        .route(
            "/api/rpc/uploadFiles",
            post(upload_files).layer(DefaultBodyLimit::max(MAX_TOTAL_UPLOAD_BYTES + 1024 * 1024)),
        )
        .route("/api/rpc/:command", post(rpc_dispatch))
        .route("/api/jobs", get(list_jobs))
        .route("/api/jobs/:id", get(job_detail))
        .route("/api/jobs/:id/events", get(job_events))
        .route("/api/*path", any(api_not_found))
        .with_state(state)
}

async fn healthz() -> ApiResponse {
    ok(json!({"ok": true}))
}

async fn api_not_found() -> ApiResponse {
    Err(api_error(StatusCode::NOT_FOUND, "not found"))
}

async fn rpc_dispatch(
    State(state): State<AppState>,
    Path(command): Path<String>,
    connect: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(args): Json<Value>,
) -> ApiResponse {
    match command.as_str() {
        "getPublicSettings" => public_settings(State(state)).await,
        "login" => login(State(state), connect, headers, Json(rpc_payload(&args)?)).await,
        "logout" => logout(State(state), headers).await,
        "me" => me(State(state), headers).await,
        "getSecurity" => get_security(State(state), headers).await,
        "saveSecurity" => {
            patch_security(
                State(state),
                headers,
                Json(rpc_nested_payload(&args, "settings")?),
            )
            .await
        }
        "changePassword" => change_password(State(state), headers, Json(rpc_payload(&args)?)).await,
        "listProviders" => list_providers(State(state), headers).await,
        "getClaudeCodeOverview" => claude_code_overview(State(state), headers).await,
        "getPlatformOverview" => platform_overview(State(state), headers).await,
        "listPlugins" => list_plugins(State(state), headers).await,
        "getProbeStatus" => {
            get_probe_status(
                State(state),
                Query(ProbeStatusQuery {
                    refresh: args.get("refresh").and_then(Value::as_bool),
                }),
                headers,
            )
            .await
        }
        "getProbeSettings" => get_probe_settings(State(state), headers).await,
        "saveProbeSettings" => {
            patch_probe_settings(
                State(state),
                headers,
                Json(rpc_nested_payload(&args, "settings")?),
            )
            .await
        }
        "getProbeLogsDbStatus" => get_probe_logs_db_status(State(state), headers).await,
        "getProbeEvents" => {
            get_probe_events(
                State(state),
                Query(ProbeEventsQuery {
                    limit: args
                        .get("limit")
                        .and_then(Value::as_u64)
                        .map(|value| value as u32),
                }),
                headers,
            )
            .await
        }
        "startProbeJob" => match rpc_string(&args, "action").as_deref() {
            Some("bark-test") => probe_bark_test(State(state), headers).await,
            Some("hooks-install") => probe_hooks_install(State(state), headers).await,
            Some("logs-db-dry-run") => {
                probe_logs_db_maintain(
                    State(state),
                    headers,
                    Some(Json(ProbeLogsDbMaintainRequest {
                        dry_run: Some(true),
                        compact: Some(false),
                    })),
                )
                .await
            }
            Some("logs-db-execute") => {
                probe_logs_db_maintain(
                    State(state),
                    headers,
                    Some(Json(ProbeLogsDbMaintainRequest {
                        dry_run: Some(false),
                        compact: Some(false),
                    })),
                )
                .await
            }
            Some(action) => Err(api_error(
                StatusCode::BAD_REQUEST,
                &format!("unknown probe action: {action}"),
            )),
            None => Err(api_error(StatusCode::BAD_REQUEST, "action is required")),
        },
        "dryRunArchiveDelete" => archive_delete_dry_run(State(state), headers).await,
        "startArchiveDelete" => {
            archive_delete_execute(
                State(state),
                headers,
                Json(ArchiveExecuteRequest { confirmed: true }),
            )
            .await
        }
        "dryRunHiddenThreadDelete" => hidden_threads_delete_dry_run(State(state), headers).await,
        "startHiddenThreadDelete" => {
            hidden_threads_delete_execute(
                State(state),
                headers,
                Json(ArchiveExecuteRequest { confirmed: true }),
            )
            .await
        }
        "getUpdateStatus" => system_update_status(State(state), headers).await,
        "runUpdateAction" => match rpc_string(&args, "action").as_deref() {
            Some("check") => system_update_precheck(State(state), headers).await,
            Some("install") => system_update_install(State(state), headers).await,
            Some("prune") => system_update_prune(State(state), headers).await,
            Some(action) => Err(api_error(
                StatusCode::BAD_REQUEST,
                &format!("unknown update action: {action}"),
            )),
            None => Err(api_error(StatusCode::BAD_REQUEST, "action is required")),
        },
        "listThreads" => list_threads(State(state), headers, Query(rpc_payload(&args)?)).await,
        "getThread" => {
            thread_detail(
                State(state),
                headers,
                Path(rpc_required_string(&args, "id")?),
                Query(rpc_nested_payload_or_empty(&args, "options")?),
            )
            .await
        }
        "getThreadBlocks" => {
            thread_blocks(
                State(state),
                headers,
                Path(rpc_required_string(&args, "id")?),
                Query(rpc_nested_payload_or_empty(&args, "options")?),
            )
            .await
        }
        "createThread" => create_thread(State(state), headers, Json(rpc_payload(&args)?)).await,
        "sendMessage" => {
            send_message(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_nested_payload(&args, "payload")?),
            )
            .await
        }
        "steerThread" => {
            steer_thread(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_nested_payload(&args, "payload")?),
            )
            .await
        }
        "listFollowUps" => {
            list_followups(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
            )
            .await
        }
        "enqueueFollowUp" => {
            enqueue_followup(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_nested_payload(&args, "payload")?),
            )
            .await
        }
        "cancelFollowUp" => {
            cancel_followup(
                State(state),
                headers,
                Path((
                    rpc_required_string(&args, "threadId")?,
                    rpc_required_string(&args, "followUpId")?,
                )),
            )
            .await
        }
        "stopThread" => {
            stop_thread(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Some(Json(rpc_nested_payload_or_empty(&args, "payload")?)),
            )
            .await
        }
        "archiveThread" => {
            archive_thread(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
            )
            .await
        }
        "restoreThread" => {
            restore_thread(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
            )
            .await
        }
        "renameThread" => {
            rename_thread(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_payload(&args)?),
            )
            .await
        }
        "forkThread" => {
            fork_thread(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
            )
            .await
        }
        "acceptPlan" => {
            plan_accept(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_nested_payload(&args, "payload")?),
            )
            .await
        }
        "revisePlan" => {
            plan_revise(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_nested_payload(&args, "payload")?),
            )
            .await
        }
        "answerElicitation" => {
            answer_elicitation(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_payload(&args)?),
            )
            .await
        }
        "answerApproval" => {
            answer_approval(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_nested_payload(&args, "payload")?),
            )
            .await
        }
        "deleteUpload" => {
            delete_upload_file(
                State(state),
                headers,
                Path(rpc_required_string(&args, "id")?),
            )
            .await
        }
        "getSystemStatus" => system_status(State(state), headers).await,
        "getSystemVersion" => system_version(State(state), headers).await,
        "listModels" => codex_models(State(state), headers).await,
        "listPermissionProfiles" => {
            codex_permission_profiles(State(state), headers, Query(rpc_payload_or_empty(&args)?))
                .await
        }
        "getCodexConfig" => {
            codex_config(State(state), headers, Query(rpc_payload_or_empty(&args)?)).await
        }
        "getCodexGoal" => {
            codex_goal_get(
                State(state),
                headers,
                Query(GoalQuery {
                    thread_id: rpc_string(&args, "threadId"),
                }),
            )
            .await
        }
        "saveCodexGoal" => codex_goal_set(State(state), headers, Json(rpc_payload(&args)?)).await,
        "clearCodexGoal" => {
            codex_goal_clear(State(state), headers, Json(rpc_payload(&args)?)).await
        }
        "pauseCodexGoal" => {
            codex_goal_pause(State(state), headers, Json(rpc_payload(&args)?)).await
        }
        "resumeCodexGoal" => {
            codex_goal_resume(State(state), headers, Json(rpc_payload(&args)?)).await
        }
        "listJobs" => {
            list_jobs(
                State(state),
                headers,
                Query(rpc_query_strings(&args, &["limit"])),
            )
            .await
        }
        "getJob" => {
            job_detail(
                State(state),
                headers,
                Path(rpc_required_string(&args, "id")?),
            )
            .await
        }
        _ => Err(api_error(
            StatusCode::NOT_FOUND,
            &format!("unknown rpc command: {command}"),
        )),
    }
}

fn rpc_payload<T: DeserializeOwned>(value: &Value) -> Result<T, ApiError> {
    serde_json::from_value(value.clone())
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))
}

fn rpc_payload_or_empty<T: DeserializeOwned>(value: &Value) -> Result<T, ApiError> {
    if value.is_null() {
        serde_json::from_value(json!({}))
    } else {
        serde_json::from_value(value.clone())
    }
    .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))
}

fn rpc_nested_payload<T: DeserializeOwned>(value: &Value, key: &str) -> Result<T, ApiError> {
    let Some(payload) = value.get(key) else {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            &format!("{key} is required"),
        ));
    };
    serde_json::from_value(payload.clone())
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))
}

fn rpc_nested_payload_or_empty<T: DeserializeOwned>(
    value: &Value,
    key: &str,
) -> Result<T, ApiError> {
    let payload = value.get(key).cloned().unwrap_or_else(|| json!({}));
    serde_json::from_value(payload)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))
}

fn rpc_required_string(value: &Value, key: &str) -> Result<String, ApiError> {
    rpc_string(value, key)
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, &format!("{key} is required")))
}

fn rpc_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .or_else(|| {
            key.strip_suffix("Id")
                .and_then(|prefix| value.get(format!("{prefix}_id")))
        })
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn rpc_query_strings(value: &Value, keys: &[&str]) -> HashMap<String, String> {
    keys.iter()
        .filter_map(|key| {
            value.get(*key).and_then(|value| match value {
                Value::String(text) if !text.trim().is_empty() => {
                    Some(((*key).to_string(), text.trim().to_string()))
                }
                Value::Number(number) => Some(((*key).to_string(), number.to_string())),
                Value::Bool(boolean) => Some(((*key).to_string(), boolean.to_string())),
                _ => None,
            })
        })
        .collect()
}

async fn public_settings(State(state): State<AppState>) -> ApiResponse {
    let security = state
        .db
        .security_settings(state.config().security.session_ttl_seconds)?;
    let turnstile_action = state
        .db
        .get_setting("turnstile_expected_action")?
        .or_else(|| state.config().security.turnstile_expected_action.clone())
        .unwrap_or_else(|| "login".to_string());
    ok(json!({
        "site_name": "NexusHub",
        "turnstile_enabled": security.turnstile_enabled,
        "turnstile_required": security.turnstile_required,
        "turnstile_site_key": security.turnstile_site_key.unwrap_or_else(|| nexushub_core::config::DEFAULT_TURNSTILE_SITE_KEY.to_string()),
        "turnstile_action": turnstile_action,
        "admin_configured": state.db.admin_count()? > 0,
        "base_url": state.config().server.public_base_url,
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
    ok(PlatformPaths::for_kind(PlatformKind::Linux))
}

async fn list_plugins(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(local::local_plugin_catalog())
}

#[derive(Debug, Deserialize)]
struct ProbeStatusQuery {
    refresh: Option<bool>,
}

async fn get_probe_status(
    State(state): State<AppState>,
    Query(query): Query<ProbeStatusQuery>,
    headers: HeaderMap,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(probe_status_cached_value(state, query.refresh.unwrap_or(false)).await)
}

async fn get_probe_settings(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(probe_settings_value(&state)?)
}

async fn get_probe_diagnostics(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(json!(probe_runtime(&state).diagnostics()))
}

async fn patch_probe_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<settings_service::ProbeSettingsSaveRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let config_path = probe_config_path();
    if !config_path.exists() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            &format!("config file not found: {}", config_path.display()),
        ));
    }
    let normalized = request
        .normalize()
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    let text = fs::read_to_string(&config_path).map_err(anyhow::Error::from)?;
    let updated = patch_probe_config_toml(&text, &normalized.config_patch)?;
    fs::write(&config_path, updated).map_err(anyhow::Error::from)?;
    let response_config = Config::load(&config_path)?;
    if let Some(device_key) = normalized.bark_device_key {
        state.db.set_secret_setting_bytes(
            settings_service::PROBE_BARK_DEVICE_KEY_SETTING,
            device_key.as_bytes(),
        )?;
    }
    state.replace_config(response_config.clone());
    state.db.record_audit(
        Some(&auth.admin_id),
        "probe_settings.updated",
        Some("probe"),
        Some("settings"),
        None,
        json!({"config_path": config_path}),
    )?;
    ok(probe_settings_value_for_config(&state, &response_config)?)
}

async fn get_probe_lifecycle(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(
        serde_json::to_value(probe_runtime(&state).lifecycle_status())
            .map_err(anyhow::Error::from)?,
    )
}

async fn get_probe_hook_status(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(json!(probe_runtime(&state).hook_status()))
}

#[derive(Debug, Deserialize)]
struct ProbeThreadsQuery {
    limit: Option<usize>,
}

async fn get_probe_running(
    State(state): State<AppState>,
    Query(query): Query<ProbeThreadsQuery>,
    headers: HeaderMap,
) -> ApiResponse {
    get_probe_threads_for_status(state, query, headers, "running").await
}

async fn get_probe_reply_needed(
    State(state): State<AppState>,
    Query(query): Query<ProbeThreadsQuery>,
    headers: HeaderMap,
) -> ApiResponse {
    get_probe_threads_for_status(state, query, headers, "reply-needed").await
}

async fn get_probe_recoverable(
    State(state): State<AppState>,
    Query(query): Query<ProbeThreadsQuery>,
    headers: HeaderMap,
) -> ApiResponse {
    get_probe_threads_for_status(state, query, headers, "recoverable").await
}

async fn get_probe_threads_for_status(
    state: AppState,
    query: ProbeThreadsQuery,
    headers: HeaderMap,
    status: &'static str,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let limit = query
        .limit
        .unwrap_or(state.config().probe.recent_limit)
        .clamp(1, 200);
    let threads = load_probe_threads(&state, status, limit).await?;
    ok(json!({
        "status": status,
        "limit": limit,
        "count": threads.len(),
        "threads": threads,
    }))
}

#[derive(Debug, Deserialize)]
struct ProbeEventsQuery {
    limit: Option<u32>,
}

async fn get_probe_events(
    State(state): State<AppState>,
    Query(query): Query<ProbeEventsQuery>,
    headers: HeaderMap,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let limit = query
        .limit
        .unwrap_or(state.config().probe.recent_limit as u32)
        .clamp(1, 500);
    let events = state
        .db
        .list_probe_events(limit)?
        .into_iter()
        .map(redact_probe_event)
        .collect::<Vec<_>>();
    ok(json!({
        "events": events,
        "limit": limit,
    }))
}

async fn get_probe_logs_db_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let config = state.config();
    let mut value = serde_json::to_value(
        ProbeRuntime::new(config.clone(), PlatformPaths::current()).logs_db_status(),
    )
    .map_err(anyhow::Error::from)?;
    if let Some(object) = value.as_object_mut() {
        if let Some((raw, updated_at)) = state
            .db
            .get_setting_with_updated_at("probe_logs_db_last_maintain")?
        {
            let last_run =
                timestamp_to_rfc3339(updated_at).unwrap_or_else(|| updated_at.to_string());
            let next_run_ts = updated_at
                + i64::from(config.probe.logs_db.maintenance_interval_hours.max(1)) * 3_600;
            let next_run =
                timestamp_to_rfc3339(next_run_ts).unwrap_or_else(|| next_run_ts.to_string());
            let last_result = probe_logs_db_last_result(&raw);
            let last_run_value = serde_json::from_str::<Value>(&raw).unwrap_or(Value::String(raw));
            object.insert("last_run".to_string(), json!(last_run));
            object.insert("last_run_at".to_string(), json!(last_run));
            object.insert("last_maintain_at".to_string(), json!(last_run));
            object.insert("next_run".to_string(), json!(next_run));
            object.insert("next_run_at".to_string(), json!(next_run));
            object.insert("next_maintain_at".to_string(), json!(next_run));
            object.insert("last_result".to_string(), json!(last_result));
            object.insert("recent_result".to_string(), json!(last_result));
            object.insert("last_maintain".to_string(), last_run_value);
        }
    }
    ok(value)
}

async fn probe_bark_test(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    start_probe_fixed_job(
        state,
        headers,
        "probe_bark_test",
        "探针 Bark 测试",
        vec!["probe".to_string(), "bark-test".to_string()],
        "probe_bark",
    )
    .await
}

async fn probe_hooks_install(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    start_probe_fixed_job(
        state,
        headers,
        "probe_hooks_install",
        "探针 Hook 安装",
        vec!["probe".to_string(), "hooks-install".to_string()],
        "probe_hooks",
    )
    .await
}

#[derive(Debug, Deserialize)]
struct ProbeLogsDbMaintainRequest {
    dry_run: Option<bool>,
    compact: Option<bool>,
}

async fn probe_logs_db_maintain(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Option<Json<ProbeLogsDbMaintainRequest>>,
) -> ApiResponse {
    let request = body
        .map(|Json(body)| body)
        .unwrap_or(ProbeLogsDbMaintainRequest {
            dry_run: Some(true),
            compact: Some(false),
        });
    let dry_run = request.dry_run.unwrap_or(true);
    let compact = request.compact.unwrap_or(false);
    let mut args = vec!["probe".to_string(), "logs-db-maintain".to_string()];
    if dry_run {
        args.push("--dry-run".to_string());
    }
    if compact {
        args.push("--compact".to_string());
    }
    start_probe_fixed_job(
        state,
        headers,
        if dry_run {
            "probe_logs_db_maintain_dry_run"
        } else {
            "probe_logs_db_maintain"
        },
        if dry_run {
            "Codex logs DB 维护 dry-run"
        } else {
            "Codex logs DB 维护"
        },
        args,
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
    let command = fixed_probe_shell_command(&state, &args);
    let id = state
        .jobs
        .start_exclusive_shell_job(kind, title, command, group)
        .map_err(|err| api_error(StatusCode::CONFLICT, &err.to_string()))?;
    ok(json!({"job_id": id}))
}

fn probe_runtime(state: &AppState) -> ProbeRuntime {
    ProbeRuntime::new(state.config(), PlatformPaths::current())
}

fn probe_config_path() -> PathBuf {
    std::env::var_os("NEXUSHUB_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PlatformPaths::current().config_file)
}

async fn probe_status_cached_value(state: AppState, force_refresh: bool) -> Value {
    if force_refresh {
        let value = probe_status_fresh_value(state.clone()).await;
        store_probe_status_snapshot(&state, value.clone());
        return with_probe_snapshot_metadata(value, 0, false, "fresh");
    }

    if let Some(snapshot) = current_probe_status_snapshot(&state) {
        spawn_probe_status_refresh(state);
        let now = chrono::Utc::now().timestamp();
        let age = now.saturating_sub(snapshot.refreshed_at_unix).max(0);
        return with_probe_snapshot_metadata(snapshot.value, age, true, "cached");
    }

    let value = probe_status_base_value(state.clone()).await;
    spawn_probe_status_refresh(state);
    with_probe_snapshot_metadata(value, 0, true, "initial")
}

async fn probe_status_fresh_value(state: AppState) -> Value {
    match probe_runtime(&state).status().await {
        Ok(status) => json!(status),
        Err(err) => json!({
            "label": "Probe",
            "enabled": state.config().probe.enabled,
            "available": false,
            "flavor": "builtin",
            "error": err.to_string(),
        }),
    }
}

async fn probe_status_base_value(state: AppState) -> Value {
    match probe_runtime(&state).status().await {
        Ok(status) => json!(status),
        Err(err) => json!({
            "label": "Probe",
            "enabled": state.config().probe.enabled,
            "available": false,
            "flavor": "builtin",
            "error": err.to_string(),
        }),
    }
}

fn current_probe_status_snapshot(state: &AppState) -> Option<CachedProbeStatus> {
    state
        .probe_status_cache
        .lock()
        .expect("probe status cache")
        .snapshot
        .clone()
}

fn store_probe_status_snapshot(state: &AppState, value: Value) {
    let mut cache = state.probe_status_cache.lock().expect("probe status cache");
    cache.snapshot = Some(CachedProbeStatus {
        value,
        refreshed_at_unix: chrono::Utc::now().timestamp(),
    });
    cache.refreshing = false;
}

pub(crate) fn spawn_probe_status_refresh(state: AppState) {
    {
        let mut cache = state.probe_status_cache.lock().expect("probe status cache");
        if cache.refreshing {
            return;
        }
        cache.refreshing = true;
    }
    tokio::spawn(async move {
        let value = probe_status_fresh_value(state.clone()).await;
        store_probe_status_snapshot(&state, value);
    });
}

fn with_probe_snapshot_metadata(
    mut value: Value,
    snapshot_age_seconds: i64,
    is_refreshing: bool,
    snapshot_status: &str,
) -> Value {
    if let Value::Object(ref mut object) = value {
        object.insert(
            "snapshot_age_seconds".to_string(),
            json!(snapshot_age_seconds),
        );
        object.insert("is_refreshing".to_string(), json!(is_refreshing));
        object.insert("snapshot_status".to_string(), json!(snapshot_status));
    }
    value
}

fn probe_settings_value(state: &AppState) -> anyhow::Result<Value> {
    probe_settings_value_for_config(state, &state.config())
}

fn probe_settings_value_for_config(state: &AppState, config: &Config) -> anyhow::Result<Value> {
    let secret_state = settings_service::ProbeSecretState::from_secret_bytes(
        state
            .db
            .get_secret_setting_bytes(settings_service::PROBE_BARK_DEVICE_KEY_SETTING)?
            .as_deref(),
    );
    serde_json::to_value(settings_service::build_settings_view(config, secret_state))
        .map_err(anyhow::Error::from)
}

fn fixed_probe_shell_command(state: &AppState, args: &[String]) -> String {
    let config_path = probe_config_path();
    let mut parts = vec![
        "/opt/nexushub/bin/nexushubd".to_string(),
        "--config".to_string(),
        config_path.display().to_string(),
    ];
    parts.extend(args.iter().cloned());
    if args.first().is_some_and(|arg| arg == "nexushubd") {
        return args
            .iter()
            .map(|part| shell_quote(part))
            .collect::<Vec<_>>()
            .join(" ");
    }
    if args.first().is_some_and(|arg| arg == "probe") {
        return parts
            .iter()
            .map(|part| shell_quote(part))
            .collect::<Vec<_>>()
            .join(" ");
    }
    format!(
        "printf '%s\\n' {}; exit 2",
        shell_quote(&format!(
            "Unsupported Probe job for {}",
            state.config().codex.host_label
        ))
    )
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) async fn load_probe_threads(
    state: &AppState,
    status: &'static str,
    limit: usize,
) -> anyhow::Result<Vec<ThreadSummary>> {
    let paths = state.codex_paths();
    if thread_list_fetch_limit(Some(status), Some(limit)) == usize::MAX {
        return probe_threads_for_status_with_paths(&paths, state.db.path(), status, limit);
    }
    let local_fetch_limit = thread_list_fetch_limit(Some(status), Some(limit));
    let hidden_thread_ids = codex::hidden_thread_ids(&paths).unwrap_or_else(|err| {
        tracing::warn!("failed to read hidden thread metadata for probe: {err}");
        HashSet::new()
    });
    let archived_thread_ids = codex::archived_thread_ids(&paths).unwrap_or_else(|err| {
        tracing::warn!("failed to read archived thread metadata for probe: {err}");
        HashSet::new()
    });
    let mut threads = codex::list_threads(&paths, None, None, local_fetch_limit)?;
    threads = prune_hidden_thread_summaries(threads, &hidden_thread_ids);
    apply_running_jobs_to_threads(state, &mut threads, &archived_thread_ids)?;
    threads = prune_hidden_thread_summaries(threads, &hidden_thread_ids);
    Ok(filter_thread_summaries(
        threads,
        Some(status),
        None,
        limit.clamp(1, 200),
    ))
}

fn redact_probe_event(event: nexushub_core::db::ProbeEvent) -> nexushub_core::db::ProbeEvent {
    redact_probe_event_for_output(event)
}

async fn upload_files(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let resolved = state.resolved_codex_paths();
    let root = uploads::upload_root(&resolved.home);
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
    let resolved = state.resolved_codex_paths();
    let root = uploads::upload_root(&resolved.home);
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
        .security_settings(state.config().security.session_ttl_seconds)?;
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
            state.config().security.cookie_secure,
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
        HeaderValue::from_str(&expired_session_cookie(
            state.config().security.cookie_secure,
        ))
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
    let paths = state.codex_paths();
    let local_fetch_limit = thread_list_fetch_limit(query.status.as_deref(), query.limit);
    let hidden_thread_ids = codex::hidden_thread_ids(&paths).unwrap_or_else(|err| {
        tracing::warn!("failed to read hidden thread metadata: {err}");
        HashSet::new()
    });
    let archived_thread_ids = codex::archived_thread_ids(&paths).unwrap_or_else(|err| {
        tracing::warn!("failed to read archived thread metadata: {err}");
        HashSet::new()
    });
    let running_jobs = state.db.running_thread_jobs()?;
    let mut threads = build_threads_overview(
        codex::list_threads(&paths, None, query.q.as_deref(), local_fetch_limit)?,
        running_jobs,
        query,
        &hidden_thread_ids,
        &archived_thread_ids,
    )
    .threads;
    submit_ready_followups_from_list(&state, &mut threads).await;
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
    api_error(StatusCode::INTERNAL_SERVER_ERROR, &message)
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
    let paths = state.codex_paths();
    let mut detail = load_base_thread_detail_cached(state, &paths, id)?;
    let _ = label;
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
    let spec = job_service::build_codex_job_spec(
        &job_service::JobActionRequest {
            kind: job_service::CodexActionKind::Exec,
            thread_id: None,
            message: prompt_with_attachment_context(&effective_message, &prepared_attachments),
            cwd: payload.cwd.and_then(non_empty_string).map(PathBuf::from),
            model: payload.model,
            service_tier: payload.service_tier,
            reasoning_effort: payload.reasoning_effort,
            permission_profile: payload.permission_profile,
            approval_policy: payload.approval_policy,
            sandbox_mode: payload.sandbox_mode,
            network_access: payload.network_access,
            collaboration_mode: payload.collaboration_mode,
        },
        state.config().codex.workspace.clone(),
    )
    .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    let resolved = state.resolved_codex_paths();
    let job_id = state.jobs.start_codex_job(
        &spec.title,
        &resolved.home,
        &spec.cwd,
        spec.args,
        spec.prompt,
    )?;
    state.db.link_job_thread(&job_id, None, None)?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.create.job_started",
        Some("job"),
        Some(&job_id),
        None,
        json!({"cwd": spec.cwd.display().to_string()}),
    )?;
    ok(CodexActionResult {
        bridge: false,
        thread_id: None,
        turn_id: None,
        job_id: Some(job_id),
        fallback: true,
        message: Some(job_service::CODEX_SUBMITTED_MESSAGE.to_string()),
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
    let resolved = state.resolved_codex_paths();
    let root = uploads::upload_root(&resolved.home);
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
    let spec = codex_resume_job_spec(
        &state,
        &id,
        prompt_with_attachment_context(
            &effective_message(&payload.message, &payload.prepared_attachments),
            &payload.prepared_attachments,
        ),
        &payload,
    )
    .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    let resolved = state.resolved_codex_paths();
    let job_id = state.jobs.start_codex_job(
        &spec.title,
        &resolved.home,
        &spec.cwd,
        spec.args,
        spec.prompt,
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
    ok(CodexActionResult {
        bridge: false,
        thread_id: Some(id),
        turn_id: None,
        job_id: Some(job_id),
        fallback: true,
        message: Some(job_service::CODEX_SUBMITTED_MESSAGE.to_string()),
    })
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
    ok(CodexActionResult {
        bridge: false,
        thread_id: Some(id),
        turn_id: None,
        job_id: None,
        fallback: true,
        message: Some("queued follow-up for the next idle turn".to_string()),
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
    archived_thread_ids: &HashSet<String>,
) -> anyhow::Result<()> {
    let jobs = state.db.running_thread_jobs()?;
    merge_running_jobs(threads, &jobs, archived_thread_ids);
    Ok(())
}

fn apply_running_job_to_detail(state: &AppState, detail: &mut ThreadDetail) -> anyhow::Result<()> {
    if let Some(job) = state.db.running_job_for_thread(&detail.summary.id)? {
        apply_running_job_to_summary(&mut detail.summary, &job);
    }
    Ok(())
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
    let spec = match codex_resume_job_spec(
        state,
        &thread_id,
        prompt_with_attachment_context(
            &effective_message(&request.message, &request.prepared_attachments),
            &request.prepared_attachments,
        ),
        &request,
    ) {
        Ok(spec) => spec,
        Err(err) => {
            let message = err.to_string();
            let _ = state.db.mark_followup_error(&followup.id, &message);
            tracing::warn!("failed to build follow-up codex job spec: {message}");
            return;
        }
    };
    let resolved = state.resolved_codex_paths();
    match state.jobs.start_codex_job(
        "Codex queued follow-up",
        &resolved.home,
        &spec.cwd,
        spec.args,
        spec.prompt,
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

fn codex_resume_job_spec(
    state: &AppState,
    thread_id: &str,
    message: String,
    request: &SendMessageRequest,
) -> anyhow::Result<job_service::CodexJobSpec> {
    job_service::build_codex_job_spec(
        &job_service::JobActionRequest {
            kind: job_service::CodexActionKind::Resume,
            thread_id: Some(thread_id.to_string()),
            message,
            cwd: request.cwd.as_ref().and_then(|value| {
                let trimmed = value.trim();
                (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
            }),
            model: request.model.clone(),
            service_tier: request.service_tier.clone(),
            reasoning_effort: request.reasoning_effort.clone(),
            permission_profile: request.permission_profile.clone(),
            approval_policy: request.approval_policy.clone(),
            sandbox_mode: request.sandbox_mode.clone(),
            network_access: request.network_access,
            collaboration_mode: request.collaboration_mode.clone(),
        },
        state.config().codex.workspace.clone(),
    )
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
    let requested_turn_id = payload.as_ref().and_then(|value| value.turn_id.clone());
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
        json!({"turn_id": requested_turn_id}),
    )?;
    Err(api_error(
        StatusCode::BAD_REQUEST,
        "stop requires job_id or an active fallback job",
    ))
}

async fn archive_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let paths = state.codex_paths();
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
    let paths = state.codex_paths();
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
    let paths = state.codex_paths();
    codex::set_thread_title(&paths, &id, name)?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.renamed",
        Some("thread"),
        Some(&id),
        None,
        json!({"name": name}),
    )?;
    ok(json!({"ok": true, "bridge": false}))
}

async fn fork_thread(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.fork.unsupported",
        Some("thread"),
        Some(&id),
        None,
        json!({"available": false}),
    )?;
    Err(api_error(
        StatusCode::NOT_IMPLEMENTED,
        "fork is unavailable in the local Codex read model",
    ))
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
    let result = start_codex_resume_job(&state, &id, job_service::plan_accept_resume_message())?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.plan.accept",
        Some("thread"),
        Some(&id),
        None,
        json!({"turn_id": payload.turn_id, "item_id": payload.item_id, "job_fallback": true}),
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
    let result = start_codex_resume_job(
        &state,
        &id,
        job_service::plan_revise_resume_message(instructions),
    )?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.plan.revise",
        Some("thread"),
        Some(&id),
        None,
        json!({"turn_id": payload.turn_id, "item_id": payload.item_id, "job_fallback": true}),
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
        "approval response is unavailable in the local Codex read model",
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
    let message = job_service::elicitation_answer_resume_message(&payload.answers);
    if message.trim().is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "answers cannot be empty",
        ));
    }
    let result = start_codex_resume_job(&state, &id, message)?;
    ok(result)
}

fn start_codex_resume_job(
    state: &AppState,
    thread_id: &str,
    message: String,
) -> Result<CodexActionResult, ApiError> {
    let spec = job_service::build_codex_job_spec(
        &job_service::JobActionRequest::resume(thread_id, message),
        state.config().codex.workspace.clone(),
    )
    .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    let resolved = state.resolved_codex_paths();
    let job_id = state
        .jobs
        .start_codex_job(
            &spec.title,
            &resolved.home,
            &spec.cwd,
            spec.args,
            spec.prompt,
        )
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()))?;
    state
        .db
        .link_job_thread(&job_id, Some(thread_id), None)
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()))?;
    Ok(CodexActionResult {
        bridge: false,
        thread_id: Some(thread_id.to_string()),
        turn_id: None,
        job_id: Some(job_id),
        fallback: true,
        message: Some(job_service::CODEX_SUBMITTED_MESSAGE.to_string()),
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
    ok(nexushub_core::system::system_status_with_paths(
        &state.config(),
        &PlatformPaths::for_kind(PlatformKind::Linux),
    )
    .await?)
}

async fn system_version(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(http_version_info().await?)
}

async fn system_update_status(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let version = http_version_info().await?;
    ok(update_service::update_status(
        &state.config(),
        &http_update_platform(),
        version.panel_latest.as_deref(),
        None,
    ))
}

async fn http_version_info() -> AnyhowResult<nexushub_core::system::VersionInfo> {
    let inputs = nexushub_core::system::VersionInfoInputs {
        panel_latest: github_latest_release("lich13", "nexushub").await.ok(),
        codex_latest: npm_latest_version("@openai/codex").await.ok(),
    };
    nexushub_core::system::version_info_with_inputs(inputs).await
}

async fn github_latest_release(owner: &str, repo: &str) -> AnyhowResult<String> {
    #[derive(Deserialize)]
    struct Release {
        tag_name: String,
    }
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let release: Release = reqwest::Client::new()
        .get(url)
        .header("user-agent", "nexushub")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(release.tag_name)
}

async fn npm_latest_version(package: &str) -> AnyhowResult<String> {
    #[derive(Deserialize)]
    struct DistTags {
        latest: String,
    }
    #[derive(Deserialize)]
    struct PackageInfo {
        #[serde(rename = "dist-tags")]
        dist_tags: DistTags,
    }
    let encoded = package.replace('/', "%2F");
    let url = format!("https://registry.npmjs.org/{encoded}");
    let package: PackageInfo = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()?
        .get(url)
        .header("user-agent", "nexushub")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(package.dist_tags.latest)
}

#[derive(Debug, Deserialize)]
struct CwdQuery {
    cwd: Option<String>,
}

async fn codex_models(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(local::default_codex_models())
}

async fn codex_permission_profiles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CwdQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let _ = query.cwd;
    ok(local::default_permission_profiles())
}

async fn codex_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CwdQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(local::local_codex_config(
        &state.config(),
        query.cwd.as_deref(),
    ))
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
    ok(local_goal_response(
        state.db.get_thread_goal(thread_id)?.as_ref(),
    ))
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
        .filter(|value| !value.is_empty());
    let status = normalize_local_goal_status(payload.status.as_deref(), payload.enabled, objective);
    let goal = state.db.upsert_thread_goal(ThreadGoalUpdate {
        thread_id,
        objective,
        token_budget: payload.token_budget,
        status: &status,
        completed_at: None,
        blocked_reason: None,
    })?;
    ok(local_goal_response(Some(&goal)))
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
    let goal = state.db.upsert_thread_goal(ThreadGoalUpdate {
        thread_id,
        objective: None,
        token_budget: None,
        status: "cleared",
        completed_at: None,
        blocked_reason: None,
    })?;
    ok(local_goal_response(Some(&goal)))
}

async fn codex_goal_pause(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<GoalUpdateRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let Some(thread_id) = non_empty(payload.thread_id.as_deref()) else {
        return Err(api_error(StatusCode::BAD_REQUEST, "thread_id is required"));
    };
    let existing = state.db.get_thread_goal(thread_id)?;
    let objective = existing.as_ref().and_then(|goal| goal.objective.as_deref());
    let token_budget = existing.as_ref().and_then(|goal| goal.token_budget);
    let goal = state.db.upsert_thread_goal(ThreadGoalUpdate {
        thread_id,
        objective,
        token_budget,
        status: "paused",
        completed_at: None,
        blocked_reason: None,
    })?;
    ok(local_goal_response(Some(&goal)))
}

async fn codex_goal_resume(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<GoalUpdateRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let Some(thread_id) = non_empty(payload.thread_id.as_deref()) else {
        return Err(api_error(StatusCode::BAD_REQUEST, "thread_id is required"));
    };
    let existing = state.db.get_thread_goal(thread_id)?;
    let objective = existing.as_ref().and_then(|goal| goal.objective.as_deref());
    let token_budget = existing.as_ref().and_then(|goal| goal.token_budget);
    let goal = state.db.upsert_thread_goal(ThreadGoalUpdate {
        thread_id,
        objective,
        token_budget,
        status: "active",
        completed_at: None,
        blocked_reason: None,
    })?;
    ok(local_goal_response(Some(&goal)))
}

async fn system_update_precheck(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    start_update_action(state, headers, UpdateAction::Check, None).await
}

async fn system_update_install(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    start_update_action(
        state,
        headers,
        UpdateAction::Install,
        Some("nexushub.update.install_started"),
    )
    .await
}

async fn system_update_prune(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    start_update_action(
        state,
        headers,
        UpdateAction::Prune,
        Some("nexushub.update.prune_started"),
    )
    .await
}

async fn start_update_action(
    state: AppState,
    headers: HeaderMap,
    action: UpdateAction,
    audit_action: Option<&str>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    if let Some(audit_action) = audit_action {
        state.db.record_audit(
            Some(&auth.admin_id),
            audit_action,
            Some("system"),
            Some("updates"),
            None,
            json!({ "action": format!("{action:?}") }),
        )?;
    }
    let plan = update_service::update_action_plan(&http_update_platform(), action);
    let spec = linux_update_job_spec(&state.config(), plan)?;
    let id = if let Some(group) = spec.exclusive_group.as_deref() {
        state
            .jobs
            .start_exclusive_shell_job(&spec.kind, &spec.title, spec.command, group)?
    } else {
        state
            .jobs
            .start_shell_job(&spec.kind, &spec.title, spec.command)?
    };
    ok(json!({"job_id": id}))
}

fn http_update_platform() -> PlatformPaths {
    PlatformPaths::for_kind(PlatformKind::Linux)
}

struct LinuxUpdateJobSpec {
    kind: String,
    title: String,
    command: String,
    exclusive_group: Option<String>,
}

fn linux_update_job_spec(
    config: &Config,
    plan: update_service::UpdateJobPlan,
) -> AnyhowResult<LinuxUpdateJobSpec> {
    if plan.platform != PlatformKind::Linux {
        anyhow::bail!("only Linux WebUI can start server update jobs");
    }
    let exclusive_group = plan.exclusive.then(|| "nexushub-update".to_string());
    match plan.action {
        UpdateAction::Check => Ok(LinuxUpdateJobSpec {
            kind: "nexushub_update_check".to_string(),
            title: "NexusHub update precheck".to_string(),
            command: config.update.panel_precheck_command.clone(),
            exclusive_group,
        }),
        UpdateAction::Install => Ok(LinuxUpdateJobSpec {
            kind: "nexushub_update_install".to_string(),
            title: "NexusHub update install".to_string(),
            command: update::panel_update_command(&config.update.panel_update_command),
            exclusive_group,
        }),
        UpdateAction::Prune => Ok(LinuxUpdateJobSpec {
            kind: "nexushub_update_prune".to_string(),
            title: "NexusHub update backup prune".to_string(),
            command: update::panel_prune_command(),
            exclusive_group,
        }),
    }
}

async fn archive_delete_dry_run(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let paths = state.codex_paths();
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
    let paths = state.codex_paths();
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
    let paths = state.codex_paths();
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
    let paths = state.codex_paths();
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
        .security_settings(state.config().security.session_ttl_seconds)?;
    let expected_hostname = state
        .db
        .get_setting("turnstile_expected_hostname")?
        .or_else(|| state.config().security.turnstile_expected_hostname.clone());
    let expected_action = state
        .db
        .get_setting("turnstile_expected_action")?
        .or_else(|| state.config().security.turnstile_expected_action.clone());
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

#[cfg(test)]
fn archived_filter(status: Option<&str>) -> Option<bool> {
    match status {
        Some("archived") => Some(true),
        Some("all") | None => Some(false),
        _ => Some(false),
    }
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

#[cfg(test)]
fn app_server_thread_list_fetch_limit(status: Option<&str>, limit: Option<usize>) -> usize {
    if matches!(status, Some("running" | "reply-needed" | "recoverable")) {
        500
    } else {
        limit.unwrap_or(80).clamp(1, 500)
    }
}

#[cfg(test)]
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

#[cfg(test)]
fn preserve_fallback_title(row: &mut ThreadSummary, fallback: &ThreadSummary) {
    if is_placeholder_thread_title(&row.title) && !is_placeholder_thread_title(&fallback.title) {
        row.title = fallback.title.clone();
    }
    if matches!(fallback.status, ThreadStatus::Archived) {
        row.status = ThreadStatus::Archived;
        row.archived_at = fallback.archived_at.clone();
    }
}

#[cfg(test)]
fn is_placeholder_thread_title(title: &str) -> bool {
    let value = title.trim();
    value.is_empty() || matches!(value, "未命名线程" | "Untitled thread" | "Untitled")
}

#[cfg(test)]
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

#[cfg(test)]
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
        false,
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
        let suppress_stale_running = rollout_enriched
            && rollout_suppresses_app_running(Some(&summary), thread, running_signal);
        summary.status = merge_app_thread_status(
            Some(&summary),
            thread,
            pending_signal,
            running_signal,
            suppress_stale_running,
        );
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

#[cfg(test)]
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
    let suppress_stale_running = rollout_enriched
        && rollout_suppresses_app_running(Some(&detail.summary), thread, running_signal);
    let status = merge_app_thread_status(
        Some(&detail.summary),
        thread,
        pending_signal,
        running_signal,
        suppress_stale_running,
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
#[cfg(test)]
enum AppThreadState {
    Active,
    Recoverable,
    Idle,
    NotLoaded,
    Unknown,
}

#[cfg(test)]
fn merge_app_thread_status(
    fallback: Option<&ThreadSummary>,
    thread: &Value,
    pending_signal: bool,
    running_signal: bool,
    suppress_stale_running: bool,
) -> ThreadStatus {
    if fallback.is_some_and(|thread| matches!(thread.status, ThreadStatus::Archived))
        || thread_archived(thread)
    {
        return ThreadStatus::Archived;
    }
    if fallback.is_some_and(|thread| matches!(thread.status, ThreadStatus::Recoverable)) {
        return ThreadStatus::Recoverable;
    }
    match app_thread_state(thread) {
        AppThreadState::Active => {
            if suppress_stale_running && !pending_signal {
                fallback_stable_status(fallback)
            } else {
                ThreadStatus::Running
            }
        }
        AppThreadState::Recoverable => ThreadStatus::Recoverable,
        AppThreadState::Idle => {
            if running_signal && !suppress_stale_running {
                ThreadStatus::Running
            } else if pending_signal {
                ThreadStatus::ReplyNeeded
            } else {
                ThreadStatus::Recent
            }
        }
        AppThreadState::NotLoaded => {
            if running_signal && !suppress_stale_running {
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

#[cfg(test)]
fn fallback_stable_status(fallback: Option<&ThreadSummary>) -> ThreadStatus {
    match fallback.map(|thread| &thread.status) {
        Some(ThreadStatus::Archived) => ThreadStatus::Archived,
        Some(ThreadStatus::Recoverable) => ThreadStatus::Recoverable,
        Some(ThreadStatus::Running | ThreadStatus::ReplyNeeded) | None => ThreadStatus::Recent,
        Some(status) => status.clone(),
    }
}

#[cfg(test)]
fn fallback_has_clearable_stale_status(summary: &ThreadSummary) -> bool {
    matches!(
        summary.status,
        ThreadStatus::Running | ThreadStatus::ReplyNeeded
    ) && summary.rollout_path.is_some()
        && summary.active_turn_id.is_none()
        && summary.active_job_id.is_none()
        && summary.pending_elicitation.is_none()
}

#[cfg(test)]
fn rollout_suppresses_app_running(
    fallback: Option<&ThreadSummary>,
    thread: &Value,
    running_signal: bool,
) -> bool {
    if !running_signal {
        return false;
    }
    let Some(summary) = fallback else {
        return false;
    };
    if !matches!(summary.status, ThreadStatus::Recent)
        || summary.active_turn_id.is_some()
        || summary.active_job_id.is_some()
        || summary.pending_elicitation.is_some()
    {
        return false;
    }
    let Some(path) = summary.rollout_path.as_deref() else {
        return false;
    };
    let Some(active_turn_id) = app_thread_active_turn_id(thread) else {
        return false;
    };
    codex::rollout_has_completed_turn(path, Some(active_turn_id.as_str())).unwrap_or(false)
}

#[cfg(test)]
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

#[cfg(test)]
fn fallback_has_pending_signal(fallback: Option<&ThreadSummary>) -> bool {
    let Some(summary) = fallback else {
        return false;
    };
    summary.active_turn_id.is_some() && summary.pending_elicitation.is_some()
}

#[cfg(test)]
fn fallback_has_running_signal(fallback: Option<&ThreadSummary>) -> bool {
    let Some(summary) = fallback else {
        return false;
    };
    summary.active_job_id.is_some()
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
fn detail_has_running_signal(detail: &ThreadDetail) -> bool {
    fallback_has_running_signal(Some(&detail.summary))
        || detail.blocks.iter().any(block_has_running_signal)
}

#[cfg(test)]
fn detail_active_turn_id(detail: &ThreadDetail) -> Option<String> {
    detail
        .blocks
        .iter()
        .rev()
        .find(|block| block_has_running_signal(block))
        .and_then(|block| block.turn_id.clone())
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
fn app_thread_status_text(thread: &Value) -> Option<&str> {
    thread
        .get("status")
        .and_then(|status| status.get("type").or(Some(status)))
        .and_then(Value::as_str)
}

#[cfg(test)]
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

#[cfg(test)]
fn app_thread_has_running_signal(thread: &Value) -> bool {
    app_thread_active_turn_id(thread).is_some()
}

#[cfg(test)]
fn app_thread_has_fallback_rollout_path(thread: &Value) -> bool {
    app_thread_rollout_path(thread).is_some()
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
fn item_id(item: &Value) -> Option<String> {
    item.get("id")
        .or_else(|| item.get("itemId"))
        .or_else(|| item.get("item_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
fn has_non_empty_string(value: &Value, fields: &[&str]) -> bool {
    fields.iter().any(|field| {
        value
            .get(*field)
            .and_then(Value::as_str)
            .is_some_and(|text| !text.trim().is_empty())
    })
}

#[cfg(test)]
fn field_contains_subagent(value: &Value, fields: &[&str]) -> bool {
    fields.iter().any(|field| {
        value
            .get(*field)
            .and_then(Value::as_str)
            .is_some_and(|text| text.to_ascii_lowercase().contains("subagent"))
    })
}

#[cfg(test)]
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

#[cfg(test)]
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

fn probe_logs_db_last_result(raw: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return raw.to_string();
    };
    let dry_run = value
        .get("dry_run")
        .and_then(Value::as_bool)
        .map(|value| if value { "dry-run" } else { "execute" })
        .unwrap_or("maintain");
    let events = value.get("events").and_then(Value::as_u64).unwrap_or(0);
    let dedupe = value.get("dedupe").and_then(Value::as_u64).unwrap_or(0);
    if let Some(skip_reason) = value.get("skip_reason").and_then(Value::as_str) {
        if !skip_reason.is_empty() {
            return format!("{dry_run}: {skip_reason}");
        }
    }
    if let Some(error) = value.get("error").and_then(Value::as_str) {
        if !error.is_empty() {
            return format!("{dry_run}: {error}");
        }
    }
    if value.get("target").and_then(Value::as_str) == Some("codex_logs_2") {
        if dry_run == "dry-run" {
            let would_delete = value
                .get("would_delete_rows")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            return format!("dry-run: would_delete_rows={would_delete}");
        }
        let deleted = value
            .get("deleted_rows")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        return format!("execute: deleted_rows={deleted}");
    }
    format!("{dry_run}: events={events}, dedupe={dedupe}")
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

#[cfg(test)]
fn normalize_goal_response(value: &Value) -> Value {
    let goal = value.get("goal").unwrap_or(value);
    if goal.is_null() {
        return goal_empty("idle");
    }
    let status = goal_status(goal).unwrap_or_else(|| "active".to_string());
    json!({
        "enabled": goal_enabled(goal, &status),
        "objective": goal.get("objective").and_then(Value::as_str),
        "token_budget": goal.get("tokenBudget").or_else(|| goal.get("token_budget")).and_then(Value::as_u64),
        "status": status,
        "raw": value,
    })
}

#[cfg(test)]
fn goal_status(goal: &Value) -> Option<String> {
    goal.get("status")
        .and_then(|value| {
            value.as_str().or_else(|| {
                value
                    .get("type")
                    .or_else(|| value.get("status"))
                    .or_else(|| value.get("state"))
                    .and_then(Value::as_str)
            })
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

#[cfg(test)]
fn goal_enabled(goal: &Value, status: &str) -> bool {
    if matches!(status, "idle" | "missing_thread" | "cleared") {
        return false;
    }
    let objective = goal
        .get("objective")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    objective
        || goal
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(matches!(
                status,
                "active" | "running" | "complete" | "completed" | "blocked" | "paused"
            ))
}

fn goal_empty(status: &str) -> Value {
    json!({
        "available": !matches!(status, "missing_thread" | "unavailable"),
        "enabled": false,
        "objective": null,
        "token_budget": null,
        "status": status,
    })
}

fn local_goal_response(goal: Option<&ThreadGoal>) -> Value {
    let Some(goal) = goal else {
        return goal_empty("idle");
    };
    let enabled = local_goal_enabled(goal);
    json!({
        "available": true,
        "enabled": enabled,
        "objective": goal.objective,
        "token_budget": goal.token_budget,
        "status": goal.status,
        "completed_at": goal.completed_at,
        "blocked_reason": goal.blocked_reason,
        "raw": {
            "source": "local",
            "thread_id": goal.thread_id,
            "created_at": goal.created_at,
            "updated_at": goal.updated_at,
        },
    })
}

fn local_goal_enabled(goal: &ThreadGoal) -> bool {
    if matches!(goal.status.as_str(), "idle" | "missing_thread" | "cleared") {
        return false;
    }
    goal.objective
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
        || matches!(
            goal.status.as_str(),
            "active" | "running" | "complete" | "completed" | "blocked" | "paused"
        )
}

fn normalize_local_goal_status(
    status: Option<&str>,
    enabled: Option<bool>,
    objective: Option<&str>,
) -> String {
    let normalized = status
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    match normalized.as_deref() {
        Some("complete" | "completed") => "complete".to_string(),
        Some("blocked") => "blocked".to_string(),
        Some("paused") => "paused".to_string(),
        Some("cleared" | "clear") => "cleared".to_string(),
        Some("idle") => "idle".to_string(),
        Some("active" | "running") => "active".to_string(),
        Some(_) => "active".to_string(),
        None if enabled == Some(false) && objective.is_none() => "cleared".to_string(),
        None if enabled == Some(false) => "paused".to_string(),
        None if objective.is_some() || enabled == Some(true) => "active".to_string(),
        None => "idle".to_string(),
    }
}

#[allow(dead_code)]
fn goal_unavailable(status: &str) -> Value {
    json!({
        "available": false,
        "enabled": false,
        "objective": null,
        "token_budget": null,
        "status": status,
        "raw": Value::Null,
    })
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn non_empty_string(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
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
        archived_filter, block_changed, effective_message, fixed_probe_shell_command,
        followup_request, linux_update_job_spec, load_probe_threads, merge_thread_summaries,
        normalize_goal_response, probe_config_path, router, seed_thread_event_blocks,
        thread_block_page, thread_event_block_key, thread_title, turnstile_login_action,
        update_service, SendMessageRequest, TurnstileLoginAction, UpdateAction,
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
            jobs as job_service, probe::PROBE_REPLY_NEEDED_FRESH_WINDOW_SECONDS,
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

    struct ConfigEnvGuard {
        _guard: std::sync::MutexGuard<'static, ()>,
        previous: Option<std::ffi::OsString>,
    }

    impl ConfigEnvGuard {
        fn set(path: &std::path::Path) -> Self {
            let guard = CONFIG_ENV_LOCK
                .get_or_init(|| Mutex::new(()))
                .lock()
                .expect("config env lock");
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

    #[tokio::test]
    async fn thread_routes_use_local_state_when_app_server_socket_is_missing() {
        let (state, session_token, csrf_token, home) = app_server_missing_socket_state();
        seed_local_codex_thread(&home, "thread-a", "local title");
        let app = router(state);

        let list_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/threads?limit=10")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list_response.status(), StatusCode::OK);
        let list_body = to_bytes(list_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let list: serde_json::Value = serde_json::from_slice(&list_body).unwrap();
        assert_eq!(list[0]["id"], "thread-a");
        assert_eq!(list[0]["title"], "local title");

        let detail_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/threads/thread-a")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(detail_response.status(), StatusCode::OK);
        let detail_body = to_bytes(detail_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let detail: serde_json::Value = serde_json::from_slice(&detail_body).unwrap();
        assert_eq!(detail["summary"]["title"], "local title");

        let rename_response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/threads/thread-a/rename")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .header("x-csrf-token", csrf_token)
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"local renamed"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rename_response.status(), StatusCode::OK);

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

        async fn request_goal(
            app: axum::Router,
            method: &str,
            uri: &str,
            body: &str,
            session_token: &str,
            csrf_token: &str,
        ) -> serde_json::Value {
            let mut builder = Request::builder()
                .method(method)
                .uri(uri)
                .header("cookie", format!("nexushub_session={session_token}"));
            if method == "POST" {
                builder = builder
                    .header("x-csrf-token", csrf_token)
                    .header("content-type", "application/json");
            }
            let response = app
                .clone()
                .oneshot(builder.body(Body::from(body.to_string())).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{method} {uri}");
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            serde_json::from_slice(&body).unwrap()
        }

        let initial = request_goal(
            app.clone(),
            "GET",
            "/api/codex/goal?thread_id=thread-a",
            "",
            &session_token,
            &csrf_token,
        )
        .await;
        assert_eq!(initial["available"], true);
        assert_eq!(initial["enabled"], false);
        assert_eq!(initial["status"], "idle");

        let set = request_goal(
            app.clone(),
            "POST",
            "/api/codex/goal",
            r#"{"thread_id":"thread-a","objective":"ship local goal","token_budget":12345,"status":"paused","enabled":false}"#,
            &session_token,
            &csrf_token,
        )
        .await;
        assert_eq!(set["available"], true);
        assert_eq!(set["enabled"], true);
        assert_eq!(set["objective"], "ship local goal");
        assert_eq!(set["token_budget"], 12345);
        assert_eq!(set["status"], "paused");

        let get = request_goal(
            app.clone(),
            "GET",
            "/api/codex/goal?thread_id=thread-a",
            "",
            &session_token,
            &csrf_token,
        )
        .await;
        assert_eq!(get["available"], true);
        assert_eq!(get["enabled"], true);
        assert_eq!(get["objective"], "ship local goal");
        assert_eq!(get["token_budget"], 12345);
        assert_eq!(get["status"], "paused");

        let saved_active = request_goal(
            app.clone(),
            "POST",
            "/api/codex/goal",
            r#"{"thread_id":"thread-a","objective":"ship local goal","token_budget":12345}"#,
            &session_token,
            &csrf_token,
        )
        .await;
        assert_eq!(saved_active["available"], true);
        assert_eq!(saved_active["enabled"], true);
        assert_eq!(saved_active["objective"], "ship local goal");
        assert_eq!(saved_active["token_budget"], 12345);
        assert_eq!(saved_active["status"], "active");

        let paused = request_goal(
            app.clone(),
            "POST",
            "/api/codex/goal/pause",
            r#"{"thread_id":"thread-a"}"#,
            &session_token,
            &csrf_token,
        )
        .await;
        assert_eq!(paused["available"], true);
        assert_eq!(paused["enabled"], true);
        assert_eq!(paused["objective"], "ship local goal");
        assert_eq!(paused["token_budget"], 12345);
        assert_eq!(paused["status"], "paused");

        let resumed = request_goal(
            app.clone(),
            "POST",
            "/api/codex/goal/resume",
            r#"{"thread_id":"thread-a"}"#,
            &session_token,
            &csrf_token,
        )
        .await;
        assert_eq!(resumed["available"], true);
        assert_eq!(resumed["enabled"], true);
        assert_eq!(resumed["objective"], "ship local goal");
        assert_eq!(resumed["token_budget"], 12345);
        assert_eq!(resumed["status"], "active");

        let cleared = request_goal(
            app.clone(),
            "POST",
            "/api/codex/goal/clear",
            r#"{"thread_id":"thread-a"}"#,
            &session_token,
            &csrf_token,
        )
        .await;
        assert_eq!(cleared["available"], true);
        assert_eq!(cleared["enabled"], false);
        assert_eq!(cleared["objective"], serde_json::Value::Null);
        assert_eq!(cleared["token_budget"], serde_json::Value::Null);
        assert_eq!(cleared["status"], "cleared");

        let resumed_after_clear = request_goal(
            app,
            "POST",
            "/api/codex/goal/resume",
            r#"{"thread_id":"thread-a"}"#,
            &session_token,
            &csrf_token,
        )
        .await;
        assert_eq!(resumed_after_clear["available"], true);
        assert_eq!(resumed_after_clear["enabled"], true);
        assert_eq!(resumed_after_clear["status"], "active");
        let _ = fs::remove_dir_all(home);
    }

    #[tokio::test]
    async fn rpc_goal_wrapper_preserves_rest_goal_dto_shape() {
        let (state, session_token, csrf_token, home) = app_server_missing_socket_state();
        seed_local_codex_thread(&home, "thread-a", "local title");
        let app = router(state);

        async fn request_json(
            app: axum::Router,
            method: &str,
            uri: &str,
            body: &str,
            session_token: &str,
            csrf_token: Option<&str>,
        ) -> serde_json::Value {
            let mut builder = Request::builder()
                .method(method)
                .uri(uri)
                .header("cookie", format!("nexushub_session={session_token}"));
            if !body.is_empty() {
                builder = builder.header("content-type", "application/json");
            }
            if let Some(csrf_token) = csrf_token {
                builder = builder.header("x-csrf-token", csrf_token);
            }
            let response = app
                .clone()
                .oneshot(builder.body(Body::from(body.to_string())).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{method} {uri}");
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            serde_json::from_slice(&body).unwrap()
        }

        let rest_initial = request_json(
            app.clone(),
            "GET",
            "/api/codex/goal?thread_id=thread-a",
            "",
            &session_token,
            None,
        )
        .await;
        let rpc_initial = request_json(
            app.clone(),
            "POST",
            "/api/rpc/getCodexGoal",
            r#"{"threadId":"thread-a"}"#,
            &session_token,
            None,
        )
        .await;
        assert_eq!(rpc_initial, rest_initial);

        let rpc_saved = request_json(
            app.clone(),
            "POST",
            "/api/rpc/saveCodexGoal",
            r#"{"thread_id":"thread-a","objective":"ship rpc","token_budget":2048}"#,
            &session_token,
            Some(&csrf_token),
        )
        .await;
        assert_eq!(rpc_saved["available"], true);
        assert_eq!(rpc_saved["enabled"], true);
        assert_eq!(rpc_saved["objective"], "ship rpc");
        assert_eq!(rpc_saved["token_budget"], 2048);

        let rest_after_rpc = request_json(
            app,
            "GET",
            "/api/codex/goal?thread_id=thread-a",
            "",
            &session_token,
            None,
        )
        .await;
        assert_eq!(rest_after_rpc, rpc_saved);
        let _ = fs::remove_dir_all(home);
    }

    #[tokio::test]
    async fn local_codex_routes_do_not_require_app_server_socket() {
        let (state, session_token, csrf_token, home) = app_server_missing_socket_state();
        seed_local_codex_thread(&home, "thread-a", "local title");
        let app = router(state.clone());

        for uri in [
            "/api/codex/models",
            "/api/codex/permission-profiles",
            "/api/codex/config",
            "/api/system/status",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(uri)
                        .header("cookie", format!("nexushub_session={session_token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let text = serde_json::to_string(&value).unwrap();
            assert!(!text.contains("app-server"), "{uri}");
        }

        let config_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/codex/config?cwd=%2Ftmp%2Fworkspace")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(config_response.status(), StatusCode::OK);
        let config_body = to_bytes(config_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let config_value: serde_json::Value = serde_json::from_slice(&config_body).unwrap();
        assert_eq!(config_value["cwd"], "/tmp/workspace");
        assert_eq!(config_value["raw"]["source"], "local");

        for (uri, body) in [
            (
                "/api/threads/thread-a/plan/accept",
                r#"{"turn_id":"turn-a","item_id":"plan-a"}"#,
            ),
            (
                "/api/threads/thread-a/plan/revise",
                r#"{"turn_id":"turn-a","item_id":"plan-a","instructions":"补充检查"}"#,
            ),
            (
                "/api/threads/thread-a/elicitation",
                r#"{"answers":{"q1":["继续"]}}"#,
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
                        .header("content-type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(value["bridge"], false, "{uri}");
            assert_eq!(value["fallback"], true, "{uri}");
            assert_eq!(value["message"], "已提交给 Codex", "{uri}");
            let message = value["message"].as_str().unwrap_or_default();
            assert!(!message.contains("fallback"), "{uri}");
            assert!(!message.contains("bridge"), "{uri}");
            assert!(!message.contains("codex exec"), "{uri}");
            assert!(!message.contains("job"), "{uri}");
            assert!(
                value["job_id"].as_str().is_some_and(|id| !id.is_empty()),
                "{uri}"
            );
        }

        for (uri, body) in [
            ("/api/threads/thread-a/fork", ""),
            (
                "/api/threads/thread-a/approval",
                r#"{"turn_id":"turn-a","item_id":"approval-a","decision":"approve"}"#,
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
                        .header("content-type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED, "{uri}");
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let error = value["error"].as_str().unwrap();
            assert!(!error.contains("app-server"), "{uri}");
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
                normalize_goal_response(&json!({
                    "goal": {
                        "objective": "ship",
                        "tokenBudget": 100,
                        "status": status
                    }
                }))
            } else {
                normalize_goal_response(&json!({
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
        let object_status = normalize_goal_response(&json!({
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
                    .uri("/api/codex/goal/resume")
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
                    .uri("/api/codex/goal/resume")
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
                    .uri("/api/codex/goal/pause")
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
                    .uri("/api/codex/goal/pause")
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
        for (uri, kind, title) in [
            (
                "/api/system/update/precheck",
                "nexushub_update_check",
                "NexusHub update precheck",
            ),
            (
                "/api/system/update/install",
                "nexushub_update_install",
                "NexusHub update install",
            ),
            (
                "/api/system/update/prune",
                "nexushub_update_prune",
                "NexusHub update backup prune",
            ),
        ] {
            let (state, session_token, csrf_token) = authenticated_test_state();
            let app = router(state.clone());

            let unauthorized = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(uri)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED, "{uri}");

            let missing_csrf = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(uri)
                        .header("cookie", format!("nexushub_session={session_token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN, "{uri}");

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

            assert_eq!(job.kind, kind, "{uri}");
            assert_eq!(job.title, title, "{uri}");
        }
    }

    #[tokio::test]
    async fn legacy_panel_update_routes_return_404() {
        let (state, session_token, csrf_token) = authenticated_test_state();
        let app = router(state);

        for uri in [
            "/api/system/panel/update/precheck",
            "/api/system/panel/update/start",
            "/api/system/panel/update/prune",
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
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
        }
    }

    #[test]
    fn linux_update_adapter_builds_shell_job_specs_outside_core_service() {
        let mut config = Config::for_platform_kind(PlatformKind::Linux);
        config.update.panel_precheck_command = "nexushub-update --precheck".to_string();
        config.update.panel_update_command = "nexushub-update --install".to_string();
        let platform = PlatformPaths::for_kind(PlatformKind::Linux);

        let precheck = linux_update_job_spec(
            &config,
            update_service::update_action_plan(&platform, UpdateAction::Check),
        )
        .unwrap();
        assert_eq!(precheck.kind, "nexushub_update_check");
        assert_eq!(precheck.command, "nexushub-update --precheck");

        let install = linux_update_job_spec(
            &config,
            update_service::update_action_plan(&platform, UpdateAction::Install),
        )
        .unwrap();
        assert_eq!(install.kind, "nexushub_update_install");
        assert_eq!(install.command, "nexushub-update --install");

        let prune = linux_update_job_spec(
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

        let unauthorized = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/system/update/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/system/update/status")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
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

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/system/status")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
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
        assert_eq!(alias.status(), StatusCode::NOT_FOUND);
        fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn probe_status_route_returns_lightweight_snapshot_immediately_and_refreshes_background()
    {
        let (state, session_token, _) = authenticated_test_state();
        let dir = temp_test_dir("nexushub-probe-status-snapshot");
        let codex_home = dir.join(".codex");
        mark_codex_home(&codex_home);
        let mut config = state.config();
        config.codex.home = codex_home.clone();
        state.replace_config(config);
        let app = router(state.clone());

        let first = app
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
        assert_eq!(first.status(), StatusCode::OK);
        let body = to_bytes(first.into_body(), usize::MAX).await.unwrap();
        let first_status: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(first_status["label"], "Probe");
        assert_eq!(first_status["snapshot_status"], "initial");
        assert_eq!(first_status["is_refreshing"], true);
        assert_eq!(first_status["snapshot_age_seconds"], 0);

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let second = app
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
        assert_eq!(second.status(), StatusCode::OK);
        let body = to_bytes(second.into_body(), usize::MAX).await.unwrap();
        let second_status: serde_json::Value = serde_json::from_slice(&body).unwrap();
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

        let unauthorized = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/probe/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/probe/events?limit=1")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
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

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/probe/settings")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .header("x-csrf-token", csrf_token)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"notifications":{"server_url":"http://example.com","device_key":"secret-device"}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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

        for uri in [
            "/api/probe/diagnostics",
            "/api/probe/running",
            "/api/probe/reply-needed",
            "/api/probe/recoverable",
        ] {
            let unauthorized = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri(uri)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED, "{uri}");

            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri(uri)
                        .header("cookie", format!("nexushub_session={session_token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
            if uri == "/api/probe/diagnostics" {
                assert_eq!(
                    payload["effective_constants"]["legacy_sentinel_cli_runtime"],
                    false
                );
                assert_eq!(payload["effective_constants"]["auto_reply"], false);
                assert_eq!(
                    payload["effective_constants"]["hidden_desktop_control"],
                    false
                );
            } else {
                assert!(payload["threads"].as_array().is_some());
                assert!(payload["count"].as_u64().is_some());
            }
        }

        for (uri, body, kind, title, forbidden_arg) in [
            (
                "/api/probe/hooks/install",
                None,
                "probe_hooks_install",
                "探针 Hook 安装",
                "codex-sentinel",
            ),
            (
                "/api/probe/bark/test",
                None,
                "probe_bark_test",
                "探针 Bark 测试",
                "device_key",
            ),
            (
                "/api/probe/logs-db/maintain",
                None,
                "probe_logs_db_maintain_dry_run",
                "Codex logs DB 维护 dry-run",
                "rm -rf",
            ),
        ]
            as [(&str, Option<&str>, &str, &str, &str); 3]
        {
            let missing_csrf = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(uri)
                        .header("cookie", format!("nexushub_session={session_token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN, "{uri}");

            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(uri)
                        .header("cookie", format!("nexushub_session={session_token}"))
                        .header("x-csrf-token", csrf_token.as_str())
                        .header("content-type", "application/json")
                        .body(body.map_or_else(Body::empty, Body::from))
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
            assert!(!job.output.contains(forbidden_arg));
        }

        let (execute_state, execute_session_token, execute_csrf_token) = authenticated_test_state();
        let execute_app = router(execute_state.clone());
        let execute_response = execute_app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/probe/logs-db/maintain")
                    .header(
                        "cookie",
                        format!("nexushub_session={execute_session_token}"),
                    )
                    .header("x-csrf-token", execute_csrf_token.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"dry_run":false,"compact":false}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(execute_response.status(), StatusCode::OK);
        let execute_body = to_bytes(execute_response.into_body(), usize::MAX)
            .await
            .unwrap();
        let execute_payload: serde_json::Value = serde_json::from_slice(&execute_body).unwrap();
        let execute_job_id = execute_payload["job_id"].as_str().unwrap();
        let execute_job = execute_state.db.job(execute_job_id).unwrap().unwrap();
        assert_eq!(execute_job.kind, "probe_logs_db_maintain");
        assert_eq!(execute_job.title, "Codex logs DB 维护");
        assert!(!execute_job.output.contains("--dry-run"));

        for (method, uri) in [
            ("POST", "/api/probe/logs-db/plan"),
            ("POST", "/api/probe/logs-db/execute"),
            ("POST", "/api/probe/legacy-cleanup/dry-run"),
            ("POST", "/api/probe/legacy-cleanup/execute"),
            ("GET", "/api/probe/dashboard"),
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
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(uri)
                        .header("cookie", format!("nexushub_session={session_token}"))
                        .header("x-csrf-token", csrf_token.as_str())
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {uri}");
        }
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

        let app = router(state.clone());
        let logs_db = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/probe/logs-db/status")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(logs_db.status(), StatusCode::OK);
        let body = to_bytes(logs_db.into_body(), usize::MAX).await.unwrap();
        let logs_db: serde_json::Value = serde_json::from_slice(&body).unwrap();

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
                &json!({"dry_run": true, "target": "codex_logs_2", "would_delete_rows": 1})
                    .to_string(),
            )
            .unwrap();
        let app = router(state.clone());

        for uri in [
            "/api/probe/settings",
            "/api/probe/hook-status",
            "/api/probe/logs-db/status",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri(uri)
                        .header("cookie", format!("nexushub_session={session_token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
        }

        let logs_db = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/probe/logs-db/status")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(logs_db.status(), StatusCode::OK);
        let body = to_bytes(logs_db.into_body(), usize::MAX).await.unwrap();
        let logs_db: serde_json::Value = serde_json::from_slice(&body).unwrap();
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

        let patch = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/probe/settings")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .header("x-csrf-token", csrf_token.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"probe":{"poll_seconds":20,"notifications":{"enabled":true,"device_key":"secret-bark-key"}}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(patch.status(), StatusCode::OK);
        let body = to_bytes(patch.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
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

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/probe/settings")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"probe":{"poll_seconds":25}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn probe_settings_patch_rejects_missing_config_file() {
        let (state, session_token, csrf_token) = authenticated_test_state();
        let dir = temp_test_dir("nexushub-missing-config");
        fs::create_dir_all(&dir).unwrap();
        let missing_path = dir.join("missing-config.toml");
        let _config_env = ConfigEnvGuard::set(&missing_path);
        let app = router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/probe/settings")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .header("x-csrf-token", csrf_token.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"probe":{"poll_seconds":25}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(payload["error"].as_str().unwrap().contains("config"));
    }

    #[tokio::test]
    async fn probe_settings_patch_refreshes_runtime_config_snapshots() {
        let (state, session_token, csrf_token, _dir, config_path) =
            authenticated_test_state_with_config_file();
        let _config_env = ConfigEnvGuard::set(&config_path);
        let app = router(state.clone());

        let patch = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/api/probe/settings")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .header("x-csrf-token", csrf_token.as_str())
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"codex":{"host_label":"fresh-host"},"probe":{"poll_seconds":33,"logs_db":{"retention_days":2}}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(patch.status(), StatusCode::OK);

        let settings = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/probe/settings")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(settings.status(), StatusCode::OK);
        let body = to_bytes(settings.into_body(), usize::MAX).await.unwrap();
        let settings: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(settings["probe"]["poll_seconds"], 33);
        assert_eq!(settings["codex"]["host_label"], "fresh-host");

        let status = app
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
        assert_eq!(status.status(), StatusCode::OK);
        let body = to_bytes(status.into_body(), usize::MAX).await.unwrap();
        let status: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(status["poll_seconds"], 33);
        assert_eq!(status["host_label"], "fresh-host");

        let logs_status = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/probe/logs-db/status")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(logs_status.status(), StatusCode::OK);
        let body = to_bytes(logs_status.into_body(), usize::MAX).await.unwrap();
        let logs_status: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(logs_status["retention_days"], 2);

        let job = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/probe/bark/test")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .header("x-csrf-token", csrf_token.as_str())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(job.status(), StatusCode::OK);
        let body = to_bytes(job.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let job_id = payload["job_id"].as_str().unwrap();
        let job = state.db.job(job_id).unwrap().unwrap();
        assert_eq!(job.kind, "probe_bark_test");
        let command =
            fixed_probe_shell_command(&state, &["probe".to_string(), "bark-test".to_string()]);
        assert!(command.contains(config_path.to_string_lossy().as_ref()));
        let config_text = fs::read_to_string(&config_path).unwrap();
        assert!(config_text.contains("fresh-host"));
    }

    #[tokio::test]
    async fn plugin_list_exposes_descriptions_and_unavailable_reasons_for_composer_mentions() {
        let (state, session_token, _csrf_token) = authenticated_test_state();
        let app = router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/plugins")
                    .header("cookie", format!("nexushub_session={session_token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let plugins: serde_json::Value = serde_json::from_slice(&body).unwrap();
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

    #[tokio::test]
    async fn obsolete_codex_and_claude_code_job_routes_return_404() {
        let (state, session_token, csrf_token) = authenticated_test_state();
        let app = router(state);

        let obsolete_uris = [
            "/api/system/codex/update/precheck",
            "/api/system/codex/update/start",
            "/api/system/codex/update/prune",
            "/api/system/update/start",
            "/api/providers/claude-code/jobs/version-check",
            "/api/providers/claude-code/jobs/update/precheck",
            "/api/providers/claude-code/jobs/update/start",
            "/api/providers/claude-code/jobs/smoke",
            "/api/providers/claude-code/jobs/cache-status",
        ];

        for method in ["POST", "GET"] {
            for uri in obsolete_uris {
                let response = app
                    .clone()
                    .oneshot(
                        Request::builder()
                            .method(method)
                            .uri(uri)
                            .header("cookie", format!("nexushub_session={session_token}"))
                            .header("x-csrf-token", csrf_token.as_str())
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {uri}");
            }
        }

        for method in ["POST", "GET"] {
            let uri = "/api/no-such-route";
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(uri)
                        .header("cookie", format!("nexushub_session={session_token}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {uri}");
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

        apply_running_job_to_summary(&mut summary, &job);

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
        let running =
            thread_service::filter_thread_summaries(rows.clone(), Some("running"), None, 50);

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
        let running =
            thread_service::filter_thread_summaries(rows.clone(), Some("running"), None, 50);

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
        let running =
            thread_service::filter_thread_summaries(rows.clone(), Some("running"), None, 50);

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
}
