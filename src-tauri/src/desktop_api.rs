use anyhow::{anyhow, Context, Result};
use nexushub_core::{
    archive::{execute_delete_archived, execute_delete_hidden},
    claude_code::{self, ClaudePaths},
    codex,
    config::{
        patch_probe_config_toml, valid_probe_notification_server_url, Config, ProbeConfigFilePatch,
    },
    db::{JobRecord, PanelDb, ThreadFollowUp, ThreadGoal, ThreadGoalUpdate},
    jobs::{CodexActionResult, JobRunner},
    local,
    platform::PlatformPaths,
    probe::ProbeRuntime,
    providers::ProviderRegistry,
    system::{system_status_with_paths, version_info},
    uploads::{self, UploadOutcome},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

const CODEX_SUBMITTED_MESSAGE: &str = "已提交给 Codex";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopApiRequest {
    pub path: String,
    pub method: Option<String>,
    pub body: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopApiUpload {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone)]
pub struct DesktopApiState {
    config: Arc<RwLock<Config>>,
    pub db: PanelDb,
    pub jobs: JobRunner,
    config_path: PathBuf,
    platform: PlatformPaths,
}

impl DesktopApiState {
    pub fn new() -> Result<Self> {
        let config = load_desktop_config();
        std::fs::create_dir_all(&config.paths.data_dir)?;
        std::fs::create_dir_all(&config.paths.log_dir)?;
        std::fs::create_dir_all(&config.codex.workspace)?;
        let db = open_panel_db(&config)?;
        Ok(Self::from_parts(
            config,
            db,
            Config::current_default_config_path(),
            PlatformPaths::current(),
        ))
    }

    fn from_parts(
        config: Config,
        db: PanelDb,
        config_path: PathBuf,
        platform: PlatformPaths,
    ) -> Self {
        let jobs = JobRunner::new(db.clone());
        Self {
            config: Arc::new(RwLock::new(config)),
            db,
            jobs,
            config_path,
            platform,
        }
    }

    fn config(&self) -> Config {
        self.config
            .read()
            .expect("desktop api config rwlock")
            .clone()
    }

    fn replace_config(&self, config: Config) {
        *self.config.write().expect("desktop api config rwlock") = config;
    }

    fn codex_paths(&self) -> codex::CodexPaths {
        codex::resolve_codex_paths(&self.config().codex.home).codex_paths()
    }

    fn resolved_codex_home(&self) -> PathBuf {
        codex::resolve_codex_paths(&self.config().codex.home).home
    }
}

pub async fn handle_desktop_api(
    state: &DesktopApiState,
    request: DesktopApiRequest,
) -> Result<Value> {
    let method = request
        .method
        .as_deref()
        .unwrap_or("GET")
        .to_ascii_uppercase();
    let (path, query) = split_path_query(&request.path);
    let segments = path
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    match (method.as_str(), segments.as_slice()) {
        ("GET", ["api", "public", "settings"]) => Ok(json!({
            "site_name": "NexusHub",
            "turnstile_enabled": false,
            "turnstile_required": false,
            "turnstile_site_key": "",
            "turnstile_action": "login",
            "admin_configured": true,
            "desktop": true
        })),
        ("GET", ["api", "auth", "me"]) => Ok(json!({
            "id": "desktop",
            "username": "本机用户",
            "csrf_token": null,
            "session_id": "desktop"
        })),
        ("POST", ["api", "auth", "login"]) => Ok(json!({
            "id": "desktop",
            "username": "本机用户",
            "csrf_token": null,
            "session_id": "desktop"
        })),
        ("POST", ["api", "auth", "logout"]) => Ok(json!({ "ok": true })),
        ("GET", ["api", "threads"]) => list_threads(state, query),
        ("POST", ["api", "threads"]) => create_thread(state, request.body).await,
        ("GET", ["api", "threads", thread_id]) => thread_detail(state, thread_id),
        ("GET", ["api", "threads", thread_id, "blocks"]) => thread_blocks(state, thread_id),
        ("POST", ["api", "threads", thread_id, "messages"]) => {
            send_message(state, thread_id, request.body).await
        }
        ("POST", ["api", "threads", thread_id, "steer"]) => {
            steer_thread(state, thread_id, request.body)
        }
        ("GET", ["api", "threads", thread_id, "follow-ups"]) => list_followups(state, thread_id),
        ("POST", ["api", "threads", thread_id, "follow-ups"]) => {
            enqueue_followup(state, thread_id, request.body)
        }
        ("POST", ["api", "threads", thread_id, "follow-ups", followup_id, "cancel"]) => {
            Ok(json!({ "ok": state.db.cancel_followup(thread_id, followup_id)? }))
        }
        ("POST", ["api", "threads", thread_id, "stop"]) => {
            stop_thread(state, thread_id, request.body)
        }
        ("POST", ["api", "threads", thread_id, "archive"]) => {
            codex::set_thread_archived(&state.codex_paths(), thread_id, true)?;
            Ok(json!({ "ok": true }))
        }
        ("POST", ["api", "threads", thread_id, "restore"]) => {
            codex::set_thread_archived(&state.codex_paths(), thread_id, false)?;
            Ok(json!({ "ok": true }))
        }
        ("POST", ["api", "threads", thread_id, "rename"]) => {
            rename_thread(state, thread_id, request.body)
        }
        ("POST", ["api", "threads", thread_id, "fork"]) => unsupported_action(thread_id, "fork"),
        ("POST", ["api", "threads", thread_id, "plan", "accept"]) => Ok(serde_json::to_value(
            start_resume_job(state, thread_id, "是，实施此计划".to_string())?,
        )?),
        ("POST", ["api", "threads", thread_id, "plan", "revise"]) => {
            revise_plan(state, thread_id, request.body)
        }
        ("POST", ["api", "threads", thread_id, "elicitation"]) => {
            answer_elicitation(state, thread_id, request.body)
        }
        ("POST", ["api", "threads", thread_id, "approval"]) => {
            unsupported_action(thread_id, "approval")
        }
        ("GET", ["api", "system", "status"]) => Ok(serde_json::to_value(
            system_status_with_paths(&state.config(), &state.platform).await?,
        )?),
        ("GET", ["api", "system", "version"]) => Ok(serde_json::to_value(version_info().await?)?),
        ("GET", ["api", "security"]) => Ok(json!({
            "turnstile_enabled": false,
            "turnstile_required": false,
            "turnstile_site_key": "",
            "turnstile_secret_configured": false,
            "session_ttl_seconds": 0,
            "turnstile_expected_hostname": null,
            "turnstile_expected_action": null
        })),
        ("PATCH", ["api", "security"]) | ("POST", ["api", "security", "password"]) => {
            Err(anyhow!("macOS App 不提供网页登录安全设置"))
        }
        ("GET", ["api", "providers"]) => {
            Ok(serde_json::to_value(ProviderRegistry::default().list())?)
        }
        ("GET", ["api", "providers", "claude-code", "overview"]) => {
            let paths = std::env::var_os("NEXUSHUB_CLAUDE_HOME")
                .map(ClaudePaths::new)
                .unwrap_or_else(ClaudePaths::default_for_user);
            Ok(serde_json::to_value(claude_code::claude_overview(&paths)?)?)
        }
        ("GET", ["api", "platform"]) => Ok(serde_json::to_value(PlatformPaths::current())?),
        ("GET", ["api", "plugins"]) => Ok(serde_json::to_value(local::local_plugin_catalog())?),
        ("GET", ["api", "codex", "models"]) => {
            Ok(serde_json::to_value(local::default_codex_models())?)
        }
        ("GET", ["api", "codex", "permission-profiles"]) => {
            Ok(serde_json::to_value(local::default_permission_profiles())?)
        }
        ("GET", ["api", "codex", "config"]) => Ok(serde_json::to_value(
            local::local_codex_config(&state.config(), None),
        )?),
        ("GET", ["api", "codex", "goal"]) => get_goal(state, query),
        ("POST", ["api", "codex", "goal"]) => save_goal(state, request.body),
        ("POST", ["api", "codex", "goal", "clear"]) => {
            update_goal_status(state, request.body, "cleared")
        }
        ("POST", ["api", "codex", "goal", "pause"]) => {
            update_goal_status(state, request.body, "paused")
        }
        ("POST", ["api", "codex", "goal", "resume"]) => {
            update_goal_status(state, request.body, "active")
        }
        ("GET", ["api", "probe", "status"]) => {
            Ok(serde_json::to_value(probe_runtime(state).status().await?)?)
        }
        ("GET", ["api", "probe", "settings"]) => probe_settings(state),
        ("PATCH", ["api", "probe", "settings"]) => save_probe_settings(state, request.body),
        ("GET", ["api", "probe", "logs-db", "status"]) => {
            Ok(serde_json::to_value(probe_runtime(state).logs_db_status())?)
        }
        ("GET", ["api", "probe", "events"]) => {
            Ok(json!({ "events": state.db.list_probe_events(30)?, "limit": 30 }))
        }
        ("POST", ["api", "probe", "bark", "test"]) => start_probe_job(
            state,
            "probe_bark_test",
            "探针 Bark 测试",
            &["probe", "bark-test"],
            "probe_bark",
        ),
        ("POST", ["api", "probe", "hooks", "install"]) => start_probe_job(
            state,
            "probe_hooks_install",
            "探针 Hook 安装",
            &["probe", "hooks-install"],
            "probe_hooks",
        ),
        ("POST", ["api", "probe", "logs-db", "maintain"]) => {
            start_probe_logs_db_maintain(state, request.body)
        }
        ("POST", ["api", "archives", "delete", "dry-run"]) => Ok(serde_json::to_value(
            nexushub_core::archive::plan_delete_archived(&state.codex_paths())?,
        )?),
        ("POST", ["api", "archives", "delete", "execute"]) => {
            require_confirmed(request.body.as_ref())?;
            Ok(serde_json::to_value(execute_delete_archived(
                &state.codex_paths(),
            )?)?)
        }
        ("POST", ["api", "hidden-threads", "delete", "dry-run"]) => Ok(serde_json::to_value(
            nexushub_core::archive::plan_delete_hidden(&state.codex_paths())?,
        )?),
        ("POST", ["api", "hidden-threads", "delete", "execute"]) => {
            require_confirmed(request.body.as_ref())?;
            Ok(serde_json::to_value(execute_delete_hidden(
                &state.codex_paths(),
            )?)?)
        }
        ("POST", ["api", "system", "panel", "update", _]) => Err(anyhow!(
            "macOS App 不提供 Linux 面板更新任务，请使用 DMG 或 Release 资产更新。"
        )),
        ("GET", ["api", "jobs"]) => Ok(serde_json::to_value(job_values(state.db.list_jobs(30)?))?),
        ("GET", ["api", "jobs", job_id]) => {
            let Some(job) = state.db.job(job_id)? else {
                return Err(anyhow!("job not found"));
            };
            Ok(job_value(job))
        }
        ("DELETE", ["api", "uploads", id]) => delete_upload(state, id),
        _ => Err(anyhow!(
            "desktop API route is not available: {} {}",
            method,
            request.path
        )),
    }
}

pub fn store_desktop_uploads(
    state: &DesktopApiState,
    files: Vec<DesktopApiUpload>,
) -> Result<UploadOutcome> {
    let root = uploads::upload_root(&state.resolved_codex_home());
    let mut stored = Vec::new();
    for file in files {
        stored.push(uploads::store_upload(
            &root,
            &file.name,
            Some(&file.mime),
            &file.bytes,
        )?);
    }
    Ok(UploadOutcome { files: stored })
}

fn load_desktop_config() -> Config {
    Config::load(Config::current_default_config_path())
        .unwrap_or_else(|_| Config::for_platform_kind(nexushub_core::platform::PlatformKind::Macos))
}

fn open_panel_db(config: &Config) -> Result<PanelDb> {
    let secret_box = config
        .secret_box()
        .unwrap_or_else(|_| nexushub_core::crypto::SecretBox::deterministic_dev());
    PanelDb::open_with_secret_box(&config.paths.db_path, secret_box)
}

fn split_path_query(path: &str) -> (&str, Option<&str>) {
    match path.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (path, None),
    }
}

fn query_value(query: Option<&str>, key: &str) -> Option<String> {
    query?
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find_map(|(name, value)| (name == key).then(|| percent_decode(value)))
}

fn percent_decode(value: &str) -> String {
    urlencoding::decode(value).map_or_else(|_| value.to_string(), |value| value.into_owned())
}

fn list_threads(state: &DesktopApiState, query: Option<&str>) -> Result<Value> {
    let status = query_value(query, "status");
    let q = query_value(query, "q");
    let limit = query_value(query, "limit")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(120);
    Ok(serde_json::to_value(codex::list_threads(
        &state.codex_paths(),
        status.as_deref(),
        q.as_deref(),
        limit,
    )?)?)
}

fn thread_detail(state: &DesktopApiState, thread_id: &str) -> Result<Value> {
    let detail = codex::thread_detail(&state.codex_paths(), thread_id)?
        .ok_or_else(|| anyhow!("thread not found"))?;
    Ok(serde_json::to_value(detail)?)
}

fn thread_blocks(state: &DesktopApiState, thread_id: &str) -> Result<Value> {
    let detail = codex::thread_detail(&state.codex_paths(), thread_id)?
        .ok_or_else(|| anyhow!("thread not found"))?;
    Ok(json!({
        "thread_id": thread_id,
        "blocks": detail.blocks,
        "total_blocks": detail.total_blocks,
        "has_more_blocks": detail.has_more_blocks,
        "before_cursor": detail.before_cursor
    }))
}

async fn create_thread(state: &DesktopApiState, body: Option<Value>) -> Result<Value> {
    let payload = body.ok_or_else(|| anyhow!("body is required"))?;
    let message = payload
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if message.is_empty() {
        return Err(anyhow!("message is required"));
    }
    let mut args = vec![
        "exec".to_string(),
        "--json".to_string(),
        "--skip-git-repo-check".to_string(),
        "-".to_string(),
    ];
    if let Some(model) = payload
        .get("model")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        args.splice(1..1, ["-m".to_string(), model.to_string()]);
    }
    let config = state.config();
    let job_id = state.jobs.start_codex_job(
        "Codex new thread",
        &state.resolved_codex_home(),
        &config.codex.workspace,
        args,
        message.to_string(),
    )?;
    state.db.link_job_thread(&job_id, None, None)?;
    Ok(serde_json::to_value(CodexActionResult {
        bridge: false,
        thread_id: None,
        turn_id: None,
        job_id: Some(job_id),
        fallback: true,
        message: Some(CODEX_SUBMITTED_MESSAGE.to_string()),
    })?)
}

async fn send_message(
    state: &DesktopApiState,
    thread_id: &str,
    body: Option<Value>,
) -> Result<Value> {
    let payload = body.ok_or_else(|| anyhow!("body is required"))?;
    let message = payload
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if message.is_empty() {
        return Err(anyhow!("message is required"));
    }
    Ok(serde_json::to_value(start_resume_job(
        state,
        thread_id,
        message.to_string(),
    )?)?)
}

fn steer_thread(state: &DesktopApiState, thread_id: &str, body: Option<Value>) -> Result<Value> {
    let payload = body.ok_or_else(|| anyhow!("body is required"))?;
    let message = payload
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if message.is_empty() {
        return Err(anyhow!("follow-up message is required"));
    }
    let followup = state.db.enqueue_followup(thread_id, &message, payload)?;
    Ok(followup_value(followup))
}

fn list_followups(state: &DesktopApiState, thread_id: &str) -> Result<Value> {
    Ok(
        json!({ "items": state.db.list_followups(thread_id, 20)?.into_iter().map(followup_value).collect::<Vec<_>>() }),
    )
}

fn enqueue_followup(
    state: &DesktopApiState,
    thread_id: &str,
    body: Option<Value>,
) -> Result<Value> {
    let payload = body.ok_or_else(|| anyhow!("body is required"))?;
    let message = payload
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if message.is_empty() {
        return Err(anyhow!("follow-up message is required"));
    }
    Ok(followup_value(
        state.db.enqueue_followup(thread_id, &message, payload)?,
    ))
}

fn stop_thread(state: &DesktopApiState, thread_id: &str, body: Option<Value>) -> Result<Value> {
    let job_id = body
        .as_ref()
        .and_then(|body| body.get("job_id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            state
                .db
                .running_job_for_thread(thread_id)
                .ok()
                .flatten()
                .map(|job| job.id)
        });
    let Some(job_id) = job_id else {
        return Err(anyhow!("stop requires job_id or an active job"));
    };
    Ok(json!({ "ok": state.jobs.cancel_job(&job_id)?, "bridge": false, "job_id": job_id }))
}

fn rename_thread(state: &DesktopApiState, thread_id: &str, body: Option<Value>) -> Result<Value> {
    let name = body
        .as_ref()
        .and_then(|body| body.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("name cannot be empty"))?;
    codex::set_thread_title(&state.codex_paths(), thread_id, name)?;
    Ok(json!({ "ok": true, "bridge": false }))
}

fn revise_plan(state: &DesktopApiState, thread_id: &str, body: Option<Value>) -> Result<Value> {
    let instructions = body
        .as_ref()
        .and_then(|body| body.get("instructions"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("revision instructions cannot be empty"))?;
    let message = format!(
        "否，请告知 Codex 如何调整\n\n请保持 Plan Mode，只根据下面的修改要求重新给出计划，不要开始实施。\n\n修改要求：\n{}",
        instructions
    );
    Ok(serde_json::to_value(start_resume_job(
        state, thread_id, message,
    )?)?)
}

fn answer_elicitation(
    state: &DesktopApiState,
    thread_id: &str,
    body: Option<Value>,
) -> Result<Value> {
    let answers = body
        .as_ref()
        .and_then(|body| body.get("answers"))
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("answers cannot be empty"))?;
    let mut rows = answers.iter().collect::<Vec<_>>();
    rows.sort_by_key(|(question, _)| *question);
    let message = rows
        .into_iter()
        .map(|(question, answers)| {
            let values = answers
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            format!("{question}: {values}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    if message.trim().is_empty() {
        return Err(anyhow!("answers cannot be empty"));
    }
    Ok(serde_json::to_value(start_resume_job(
        state, thread_id, message,
    )?)?)
}

fn start_resume_job(
    state: &DesktopApiState,
    thread_id: &str,
    message: String,
) -> Result<CodexActionResult> {
    let args = vec![
        "exec".to_string(),
        "resume".to_string(),
        "--all".to_string(),
        "--json".to_string(),
        thread_id.to_string(),
        "-".to_string(),
    ];
    let config = state.config();
    let job_id = state.jobs.start_codex_job(
        "Codex resume thread",
        &state.resolved_codex_home(),
        &config.codex.workspace,
        args,
        message,
    )?;
    state.db.link_job_thread(&job_id, Some(thread_id), None)?;
    Ok(CodexActionResult {
        bridge: false,
        thread_id: Some(thread_id.to_string()),
        turn_id: None,
        job_id: Some(job_id),
        fallback: true,
        message: Some(CODEX_SUBMITTED_MESSAGE.to_string()),
    })
}

fn unsupported_action(thread_id: &str, action: &str) -> Result<Value> {
    Err(anyhow!("macOS App 当前不支持 {action} 操作：{thread_id}"))
}

fn get_goal(state: &DesktopApiState, query: Option<&str>) -> Result<Value> {
    let thread_id =
        query_value(query, "thread_id").ok_or_else(|| anyhow!("thread_id is required"))?;
    Ok(goal_value(state.db.get_thread_goal(&thread_id)?))
}

fn save_goal(state: &DesktopApiState, body: Option<Value>) -> Result<Value> {
    let body = body.ok_or_else(|| anyhow!("body is required"))?;
    let thread_id = body
        .get("thread_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("thread_id is required"))?;
    let objective = body
        .get("objective")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let token_budget = body.get("token_budget").and_then(Value::as_u64);
    let goal = state.db.upsert_thread_goal(ThreadGoalUpdate {
        thread_id,
        objective,
        token_budget: objective.and(token_budget),
        status: objective.map_or("cleared", |_| "active"),
        completed_at: None,
        blocked_reason: None,
    })?;
    Ok(goal_value(Some(goal)))
}

fn update_goal_status(
    state: &DesktopApiState,
    body: Option<Value>,
    status: &'static str,
) -> Result<Value> {
    let body = body.ok_or_else(|| anyhow!("body is required"))?;
    let thread_id = body
        .get("thread_id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("thread_id is required"))?;
    if status == "cleared" {
        let goal = state.db.upsert_thread_goal(ThreadGoalUpdate {
            thread_id,
            objective: None,
            token_budget: None,
            status,
            completed_at: None,
            blocked_reason: None,
        })?;
        return Ok(goal_value(Some(goal)));
    }
    let existing = state.db.get_thread_goal(thread_id)?;
    let objective = existing.as_ref().and_then(|goal| goal.objective.as_deref());
    let token_budget = existing.as_ref().and_then(|goal| goal.token_budget);
    let goal = state.db.upsert_thread_goal(ThreadGoalUpdate {
        thread_id,
        objective,
        token_budget,
        status,
        completed_at: None,
        blocked_reason: None,
    })?;
    Ok(goal_value(Some(goal)))
}

fn goal_value(goal: Option<ThreadGoal>) -> Value {
    match goal {
        Some(goal) => {
            let enabled = !matches!(goal.status.as_str(), "idle" | "cleared")
                && (goal
                    .objective
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
                    || matches!(
                        goal.status.as_str(),
                        "active" | "paused" | "complete" | "completed" | "blocked"
                    ));
            json!({
                "available": true,
                "enabled": enabled,
                "thread_id": goal.thread_id,
                "objective": goal.objective,
                "token_budget": goal.token_budget,
                "status": goal.status,
                "completed_at": goal.completed_at,
                "blocked_reason": goal.blocked_reason
            })
        }
        None => json!({
            "available": true,
            "enabled": false,
            "thread_id": null,
            "objective": null,
            "token_budget": null,
            "status": "idle",
            "completed_at": null,
            "blocked_reason": null
        }),
    }
}

fn probe_runtime(state: &DesktopApiState) -> ProbeRuntime {
    ProbeRuntime::new(state.config(), state.platform.clone())
}

fn probe_settings(state: &DesktopApiState) -> Result<Value> {
    let config = state.config();
    Ok(json!({
        "codex": {
            "home": config.codex.home,
            "workspace": config.codex.workspace,
            "host_label": config.codex.host_label
        },
        "probe": config.probe,
        "notifications": config.probe.notifications,
        "logs_db": config.probe.logs_db,
        "discovery_warnings": []
    }))
}

fn save_probe_settings(state: &DesktopApiState, body: Option<Value>) -> Result<Value> {
    let body = body.ok_or_else(|| anyhow!("body is required"))?;
    if let Some(server_url) = body
        .pointer("/notifications/server_url")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        if !valid_probe_notification_server_url(server_url) {
            return Err(anyhow!(
                "probe notifications server_url must use HTTPS except localhost HTTP"
            ));
        }
    }
    if let Some(device_key) = body
        .pointer("/notifications/device_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        state
            .db
            .set_secret_setting_bytes("probe_bark_device_key", device_key.as_bytes())?;
    }
    ensure_config_file(&state.config_path, &state.config())?;
    let patch: ProbeConfigFilePatch = serde_json::from_value(body)?;
    let text = std::fs::read_to_string(&state.config_path)
        .with_context(|| format!("read config {}", state.config_path.display()))?;
    let updated = patch_probe_config_toml(&text, &patch)?;
    std::fs::write(&state.config_path, updated)
        .with_context(|| format!("write config {}", state.config_path.display()))?;
    let updated_config = Config::load(&state.config_path)?;
    state.replace_config(updated_config);
    probe_settings(state)
}

fn ensure_config_file(path: &Path, config: &Config) -> Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create config dir {}", parent.display()))?;
    }
    let text = toml::to_string_pretty(config).context("serialize desktop config")?;
    std::fs::write(path, text).with_context(|| format!("write config {}", path.display()))
}

fn start_probe_job(
    state: &DesktopApiState,
    kind: &str,
    title: &str,
    args: &[&str],
    group: &str,
) -> Result<Value> {
    let binary = state.platform.daemon_binary();
    if !binary.is_file() {
        return Err(anyhow!(
            "macOS App 当前不提供此任务：需要本机 nexushubd 二进制，路径 {}",
            binary.display()
        ));
    }
    let mut command = vec![
        shell_quote(&binary.display().to_string()),
        "--config".to_string(),
        shell_quote(&state.config_path.display().to_string()),
    ];
    command.extend(args.iter().map(|arg| shell_quote(arg)));
    let job_id = state
        .jobs
        .start_exclusive_shell_job(kind, title, command.join(" "), group)?;
    Ok(json!({ "job_id": job_id }))
}

fn start_probe_logs_db_maintain(state: &DesktopApiState, body: Option<Value>) -> Result<Value> {
    let dry_run = body
        .as_ref()
        .and_then(|body| body.get("dry_run"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let compact = body
        .as_ref()
        .and_then(|body| body.get("compact"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut args = vec!["probe", "logs-db-maintain"];
    if dry_run {
        args.push("--dry-run");
    }
    if compact {
        args.push("--compact");
    }
    start_probe_job(
        state,
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
        &args,
        "probe_logs_db",
    )
}

fn require_confirmed(body: Option<&Value>) -> Result<()> {
    if body
        .and_then(|body| body.get("confirmed"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(());
    }
    Err(anyhow!("confirmed=true is required"))
}

fn delete_upload(state: &DesktopApiState, id: &str) -> Result<Value> {
    let root = uploads::upload_root(&state.resolved_codex_home());
    let deleted = uploads::delete_upload(&root, id)?;
    Ok(json!({ "ok": true, "deleted": deleted }))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn followup_value(item: ThreadFollowUp) -> Value {
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
        "result": item.result_json.and_then(|raw| serde_json::from_str::<Value>(&raw).ok()),
        "error": item.error
    })
}

fn job_values(jobs: Vec<JobRecord>) -> Vec<Value> {
    jobs.into_iter().map(job_value).collect()
}

fn job_value(job: JobRecord) -> Value {
    serde_json::to_value(job).unwrap_or_else(|_| json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> (tempfile::TempDir, DesktopApiState) {
        let temp = tempfile::tempdir().unwrap();
        let mut config = Config::for_platform_kind_with_home(
            nexushub_core::platform::PlatformKind::Macos,
            temp.path(),
        );
        config.paths.data_dir = temp.path().join("data");
        config.paths.db_path = temp.path().join("panel.sqlite");
        config.paths.log_dir = temp.path().join("logs");
        config.codex.home = temp.path().join("codex-home");
        config.codex.workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&config.paths.data_dir).unwrap();
        std::fs::create_dir_all(&config.paths.log_dir).unwrap();
        std::fs::create_dir_all(&config.codex.home).unwrap();
        std::fs::create_dir_all(&config.codex.workspace).unwrap();
        let db = PanelDb::open_with_secret_box(
            &config.paths.db_path,
            nexushub_core::crypto::SecretBox::deterministic_dev(),
        )
        .unwrap();
        let state = DesktopApiState::from_parts(
            config,
            db,
            temp.path().join("config.toml"),
            PlatformPaths::for_kind_with_home(
                nexushub_core::platform::PlatformKind::Macos,
                temp.path(),
            ),
        );
        (temp, state)
    }

    #[test]
    fn split_path_query_preserves_api_path() {
        assert_eq!(
            split_path_query("/api/threads?limit=120"),
            ("/api/threads", Some("limit=120"))
        );
    }

    #[tokio::test]
    async fn desktop_public_settings_need_no_admin_or_csrf() {
        let (_temp, state) = test_state();
        let value = handle_desktop_api(
            &state,
            DesktopApiRequest {
                path: "/api/public/settings".to_string(),
                method: Some("GET".to_string()),
                body: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(value["admin_configured"], true);
        assert_eq!(value["turnstile_enabled"], false);
    }

    #[tokio::test]
    async fn missing_desktop_capabilities_return_unsupported_errors_not_success() {
        let (_temp, state) = test_state();

        for (path, method) in [
            ("/api/system/panel/update/start", "POST"),
            ("/api/threads/thread-a/fork", "POST"),
            ("/api/threads/thread-a/approval", "POST"),
            ("/api/probe/bark/test", "POST"),
        ] {
            let err = handle_desktop_api(
                &state,
                DesktopApiRequest {
                    path: path.to_string(),
                    method: Some(method.to_string()),
                    body: Some(json!({})),
                },
            )
            .await
            .unwrap_err();
            let message = err.to_string();

            assert!(
                message.contains("不支持") || message.contains("不提供"),
                "{path} returned non-unsupported error: {message}"
            );
            assert!(
                !message.contains("\"ok\":true"),
                "{path} reported fake success: {message}"
            );
        }
    }

    #[tokio::test]
    async fn desktop_goal_save_pause_resume_and_clear_use_panel_db() {
        let (_temp, state) = test_state();

        let saved = handle_desktop_api(
            &state,
            DesktopApiRequest {
                path: "/api/codex/goal".to_string(),
                method: Some("POST".to_string()),
                body: Some(json!({
                    "thread_id": "thread-a",
                    "objective": "ship desktop goal",
                    "token_budget": 5000
                })),
            },
        )
        .await
        .unwrap();
        assert_eq!(saved["status"], "active");
        assert_eq!(saved["enabled"], true);
        assert_eq!(saved["objective"], "ship desktop goal");
        assert_eq!(saved["token_budget"], 5000);

        let paused = handle_desktop_api(
            &state,
            DesktopApiRequest {
                path: "/api/codex/goal/pause".to_string(),
                method: Some("POST".to_string()),
                body: Some(json!({ "thread_id": "thread-a" })),
            },
        )
        .await
        .unwrap();
        assert_eq!(paused["status"], "paused");
        assert_eq!(paused["enabled"], true);
        assert_eq!(paused["objective"], "ship desktop goal");

        let resumed = handle_desktop_api(
            &state,
            DesktopApiRequest {
                path: "/api/codex/goal/resume".to_string(),
                method: Some("POST".to_string()),
                body: Some(json!({ "thread_id": "thread-a" })),
            },
        )
        .await
        .unwrap();
        assert_eq!(resumed["status"], "active");
        assert_eq!(resumed["objective"], "ship desktop goal");

        let cleared = handle_desktop_api(
            &state,
            DesktopApiRequest {
                path: "/api/codex/goal/clear".to_string(),
                method: Some("POST".to_string()),
                body: Some(json!({ "thread_id": "thread-a" })),
            },
        )
        .await
        .unwrap();
        assert_eq!(cleared["status"], "cleared");
        assert_eq!(cleared["enabled"], false);
        assert_eq!(cleared["objective"], serde_json::Value::Null);
        assert_eq!(cleared["token_budget"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn desktop_probe_settings_patch_writes_config_and_secret_without_csrf() {
        let (_temp, state) = test_state();

        let updated = handle_desktop_api(
            &state,
            DesktopApiRequest {
                path: "/api/probe/settings".to_string(),
                method: Some("PATCH".to_string()),
                body: Some(json!({
                    "probe": {
                        "enabled": true,
                        "poll_seconds": 9,
                        "notifications": {
                            "enabled": true,
                            "server_url": "https://api.day.app",
                            "notify_reply_needed": true
                        }
                    },
                    "notifications": {
                        "device_key": "secret-device-key"
                    }
                })),
            },
        )
        .await
        .unwrap();

        assert_eq!(updated["probe"]["poll_seconds"], 9);
        assert_eq!(
            updated["notifications"]["server_url"],
            "https://api.day.app"
        );
        assert!(state.config_path.is_file());
        let config_text = std::fs::read_to_string(&state.config_path).unwrap();
        assert!(config_text.contains("poll_seconds = 9"));
        assert!(!config_text.contains("secret-device-key"));
        assert_eq!(
            state
                .db
                .get_secret_setting_bytes("probe_bark_device_key")
                .unwrap()
                .unwrap(),
            b"secret-device-key"
        );
    }

    #[tokio::test]
    async fn desktop_delete_execute_requires_confirmed_body() {
        let (_temp, state) = test_state();

        let err = handle_desktop_api(
            &state,
            DesktopApiRequest {
                path: "/api/archives/delete/execute".to_string(),
                method: Some("POST".to_string()),
                body: Some(json!({})),
            },
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("confirmed=true"));
    }

    #[test]
    fn desktop_upload_store_and_delete_use_local_codex_home() {
        let (_temp, state) = test_state();

        let outcome = store_desktop_uploads(
            &state,
            vec![DesktopApiUpload {
                name: "note.md".to_string(),
                mime: "text/markdown".to_string(),
                bytes: b"# hello".to_vec(),
            }],
        )
        .unwrap();
        let id = outcome.files[0].id.clone();
        let root = uploads::upload_root(&state.resolved_codex_home());
        assert!(root.join(&id).join("meta.json").is_file());

        let deleted = delete_upload(&state, &id).unwrap();
        assert_eq!(deleted["deleted"], true);
        assert!(!root.join(&id).exists());
    }
}
