use anyhow::{anyhow, Result};
use nexushub_core::{
    archive::{
        execute_delete_archived, execute_delete_hidden, plan_delete_archived, plan_delete_hidden,
        ArchiveDeletePlan, ArchiveDeleteResult, HiddenThreadDeletePlan, HiddenThreadDeleteResult,
    },
    claude_code::{claude_overview, ClaudeOverview, ClaudePaths},
    codex::{
        list_threads, resolve_codex_paths, set_thread_archived, set_thread_title, thread_detail,
        window_thread_detail, CodexPaths, MessageBlock, ThreadDetail, ThreadSummary,
    },
    config::{
        patch_probe_config_toml, CodexProbeConfigPatch, Config, ProbeConfigFilePatch,
        ProbeHooksConfigPatch, ProbeLogsDbConfigPatch, ProbeNotificationsConfigPatch,
        ProbeObservabilityConfigPatch, ProbeSettingsPatch,
    },
    crypto::SecretBox,
    db::{JobRecord, PanelDb, ProbeEvent, ThreadFollowUp},
    jobs::{CodexActionResult, JobRunner},
    local::{
        default_codex_models, default_permission_profiles, local_codex_config,
        local_plugin_catalog, CodexModelInfo, CodexPermissionProfile, LocalCodexConfig,
        LocalPluginInfo,
    },
    platform::{PlatformKind, PlatformPaths},
    probe::{
        redact_probe_event_for_output, ProbeLogsDbMaintenanceResult, ProbeLogsDbStatus,
        ProbeRuntime, ProbeStatus,
    },
    services::{
        goals as goal_service, jobs as job_service, settings as settings_service,
        threads::{self as thread_service, ThreadsQuery},
    },
    system::{system_status_with_paths, SystemStatus},
    update::{analyze_job_failure, JobFailureAnalysis},
    uploads,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

const THREAD_DETAIL_DEFAULT_BLOCK_LIMIT: usize = 120;
const THREAD_DETAIL_MAX_BLOCK_LIMIT: usize = 500;
const PROBE_LOGS_DB_LAST_MAINTAIN_SETTING: &str = "probe_logs_db_last_maintain";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusPaths {
    pub app_support_dir: PathBuf,
    pub config_file: PathBuf,
    pub database_file: PathBuf,
    pub log_dir: PathBuf,
    pub app_log_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopOverview {
    pub product_name: String,
    pub version: String,
    pub identifier: String,
    pub os: String,
    pub arch: String,
    pub paths: NexusPaths,
    pub app_support_dir_ready: bool,
    pub log_dir_ready: bool,
    pub config_file_exists: bool,
    pub database_file_exists: bool,
    pub codex_home: PathBuf,
    pub codex_home_source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopHome {
    pub overview: DesktopOverview,
    pub system: Option<SystemStatus>,
    pub probe: Option<ProbeStatus>,
    pub logs_db: Option<ProbeLogsDbStatus>,
    pub threads: Vec<ThreadSummary>,
    pub plugins: Vec<LocalPluginInfo>,
    pub models: Vec<CodexModelInfo>,
    pub permission_profiles: Vec<CodexPermissionProfile>,
    pub codex_config: LocalCodexConfig,
    pub archive_plan: Option<ArchiveDeletePlan>,
    pub hidden_plan: Option<HiddenThreadDeletePlan>,
    pub goal: DesktopGoal,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopGoal {
    pub available: bool,
    pub enabled: bool,
    pub thread_id: Option<String>,
    pub objective: Option<String>,
    pub token_budget: Option<u64>,
    pub status: String,
    pub completed_at: Option<i64>,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListRequest {
    pub status: Option<String>,
    pub query: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopGoalRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
    pub objective: Option<String>,
    #[serde(alias = "tokenBudget", alias = "token_budget")]
    pub token_budget: Option<u64>,
}

#[derive(Clone)]
pub struct DesktopState {
    config: Arc<RwLock<Config>>,
    pub db: PanelDb,
    pub jobs: JobRunner,
    platform: PlatformPaths,
}

impl DesktopState {
    pub fn current() -> Result<Self> {
        let config = load_desktop_config();
        std::fs::create_dir_all(&config.paths.data_dir)?;
        std::fs::create_dir_all(&config.paths.log_dir)?;
        std::fs::create_dir_all(&config.codex.workspace)?;
        let db = open_panel_db(&config)?;
        Ok(Self::new(config, db, PlatformPaths::current()))
    }

    pub fn new(config: Config, db: PanelDb, platform: PlatformPaths) -> Self {
        let jobs = JobRunner::new(db.clone());
        Self {
            config: Arc::new(RwLock::new(config)),
            db,
            jobs,
            platform,
        }
    }

    pub fn config(&self) -> Config {
        self.config.read().expect("desktop config rwlock").clone()
    }

    pub fn replace_config(&self, config: Config) {
        *self.config.write().expect("desktop config rwlock") = config;
    }

    pub fn resolved_codex_paths(&self) -> nexushub_core::codex::ResolvedCodexPaths {
        let config = self.config();
        resolve_codex_paths(&config.codex.home)
    }

    pub fn codex_paths(&self) -> CodexPaths {
        self.resolved_codex_paths().codex_paths()
    }

    pub fn platform(&self) -> &PlatformPaths {
        &self.platform
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopActionResponse {
    pub ok: bool,
    pub available: bool,
    pub command: String,
    pub message: String,
    pub thread_id: Option<String>,
    pub job_id: Option<String>,
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopThreadBlockPage {
    pub thread_id: String,
    pub blocks: Vec<MessageBlock>,
    pub total_blocks: usize,
    pub has_more_blocks: bool,
    pub before_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProbeSettings {
    pub codex: Value,
    pub probe: nexushub_core::config::ProbeConfig,
    pub notifications: Value,
    pub logs_db: nexushub_core::config::ProbeLogsDbConfig,
}

impl From<settings_service::SettingsView> for DesktopProbeSettings {
    fn from(view: settings_service::SettingsView) -> Self {
        Self {
            codex: serde_json::to_value(view.codex).unwrap_or_else(|_| json!({})),
            probe: view.probe,
            notifications: serde_json::to_value(view.notifications).unwrap_or_else(|_| json!({})),
            logs_db: view.logs_db,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopJobResponse {
    #[serde(flatten)]
    pub job: JobRecord,
    pub failure_analysis: Option<JobFailureAnalysis>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProbeEventsResponse {
    pub events: Vec<ProbeEvent>,
    pub limit: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopDeleteUploadResponse {
    pub ok: bool,
    pub deleted: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopUploadFile {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDetailRequest {
    pub id: String,
    pub limit: Option<usize>,
    pub full: Option<bool>,
    pub before: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadBlocksRequest {
    pub id: String,
    pub limit: Option<usize>,
    pub before: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSendMessageRequest {
    #[serde(default, alias = "threadId", alias = "thread_id")]
    pub thread_id: Option<String>,
    pub message: String,
    #[serde(default)]
    pub attachments: Vec<String>,
    pub model: Option<String>,
    #[serde(alias = "service_tier")]
    pub service_tier: Option<String>,
    #[serde(alias = "reasoning_effort")]
    pub reasoning_effort: Option<String>,
    pub cwd: Option<String>,
    #[serde(alias = "permission_profile")]
    pub permission_profile: Option<String>,
    #[serde(alias = "approval_policy")]
    pub approval_policy: Option<String>,
    #[serde(alias = "sandbox_mode")]
    pub sandbox_mode: Option<String>,
    #[serde(alias = "network_access")]
    pub network_access: Option<bool>,
    #[serde(alias = "collaboration_mode")]
    pub collaboration_mode: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopStopRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
    #[serde(alias = "turn_id")]
    pub turn_id: Option<String>,
    #[serde(alias = "job_id")]
    pub job_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopThreadIdRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopRenameThreadRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopPlanAcceptRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
    #[serde(alias = "turn_id")]
    pub turn_id: Option<String>,
    #[serde(alias = "item_id")]
    pub item_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopPlanReviseRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
    #[serde(alias = "turn_id")]
    pub turn_id: Option<String>,
    #[serde(alias = "item_id")]
    pub item_id: Option<String>,
    pub instructions: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopElicitationAnswerRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
    pub answers: std::collections::HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProbeSettingsRequest {
    pub codex: Option<CodexProbeConfigPatch>,
    pub probe: Option<DesktopProbeSettingsPatch>,
    pub notifications: Option<DesktopProbeNotificationsRequest>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProbeSettingsPatch {
    pub enabled: Option<bool>,
    pub poll_seconds: Option<u64>,
    pub recent_limit: Option<usize>,
    pub hooks: Option<ProbeHooksConfigPatch>,
    pub notifications: Option<DesktopProbeNotificationsRequest>,
    pub observability: Option<ProbeObservabilityConfigPatch>,
    pub logs_db: Option<ProbeLogsDbConfigPatch>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProbeNotificationsRequest {
    pub device_key: Option<String>,
    #[serde(flatten)]
    pub patch: ProbeNotificationsConfigPatch,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopLogsDbMaintainRequest {
    #[serde(alias = "dry_run")]
    pub dry_run: Option<bool>,
    pub compact: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopJobsRequest {
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopJobDetailRequest {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProbeEventsRequest {
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopDeleteUploadRequest {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopFollowupRequest {
    pub thread_id: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopCancelFollowupRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
    #[serde(alias = "followUpId", alias = "followupId", alias = "followup_id")]
    pub followup_id: String,
}

pub fn nexus_paths_for_home(home: impl Into<PathBuf>) -> NexusPaths {
    let home = home.into();
    let app_support_dir = home
        .join("Library")
        .join("Application Support")
        .join("NexusHub");
    let log_dir = home.join("Library").join("Logs").join("NexusHub");

    NexusPaths {
        config_file: app_support_dir.join("config.toml"),
        database_file: app_support_dir.join("nexushub.sqlite"),
        app_log_file: log_dir.join("nexushub.log"),
        app_support_dir,
        log_dir,
    }
}

pub fn build_desktop_overview() -> Result<DesktopOverview> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot resolve home directory"))?;
    let paths = nexus_paths_for_home(home);
    let config = load_desktop_config();
    build_desktop_overview_for_config(paths, &config)
}

fn build_desktop_overview_for_config(
    paths: NexusPaths,
    config: &Config,
) -> Result<DesktopOverview> {
    let resolved = resolve_codex_paths(&config.codex.home);

    Ok(DesktopOverview {
        product_name: "NexusHub".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        identifier: "com.lich13.nexushub".to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        app_support_dir_ready: paths.app_support_dir.is_dir(),
        log_dir_ready: paths.log_dir.is_dir(),
        config_file_exists: paths.config_file.is_file(),
        database_file_exists: paths.database_file.is_file(),
        codex_home: resolved.home,
        codex_home_source: resolved.codex_home_source,
        paths,
    })
}

pub async fn build_desktop_home() -> Result<DesktopHome> {
    let state = DesktopState::current()?;
    build_desktop_home_with_state(&state).await
}

pub async fn build_desktop_home_with_state(state: &DesktopState) -> Result<DesktopHome> {
    let config = state.config();
    let home = dirs::home_dir().ok_or_else(|| anyhow!("cannot resolve home directory"))?;
    let overview = build_desktop_overview_for_config(nexus_paths_for_home(home), &config)?;
    let mut warnings = overview_warning(&overview);
    let resolved = resolve_codex_paths(&config.codex.home);
    warnings.extend(resolved.discovery_warnings.clone());
    let codex_paths = resolved.codex_paths();
    let runtime = ProbeRuntime::new(config.clone(), state.platform().clone());

    let system = system_status_with_paths(&config, state.platform())
        .await
        .ok();
    let probe = runtime.status().await.ok();
    let logs_db = Some(runtime.logs_db_status());
    let threads = thread_list_with_jobs(
        state,
        ThreadsQuery {
            status: None,
            q: None,
            limit: Some(40),
        },
    )
    .unwrap_or_else(|err| {
        warnings.push(format!("线程读取失败: {err}"));
        Vec::new()
    });
    let archive_plan = plan_delete_archived(&codex_paths).ok();
    let hidden_plan = plan_delete_hidden(&codex_paths).ok();
    let goal = first_thread_goal(&config, threads.first());

    Ok(DesktopHome {
        overview,
        system,
        probe,
        logs_db,
        threads,
        plugins: local_plugin_catalog(),
        models: default_codex_models(),
        permission_profiles: default_permission_profiles(),
        codex_config: local_codex_config(&config, None),
        archive_plan,
        hidden_plan,
        goal,
        warnings,
    })
}

pub fn desktop_threads(request: ThreadListRequest) -> Result<Vec<ThreadSummary>> {
    let state = DesktopState::current()?;
    desktop_threads_with_state(&state, request)
}

pub fn desktop_threads_with_state(
    state: &DesktopState,
    request: ThreadListRequest,
) -> Result<Vec<ThreadSummary>> {
    thread_list_with_jobs(
        state,
        ThreadsQuery {
            status: request.status,
            q: request.query,
            limit: request.limit,
        },
    )
}

pub fn desktop_thread_detail(id: &str) -> Result<Option<ThreadDetail>> {
    let state = DesktopState::current()?;
    desktop_thread_detail_with_state(
        &state,
        ThreadDetailRequest {
            id: id.to_string(),
            limit: None,
            full: Some(true),
            before: None,
        },
    )
}

pub fn desktop_thread_detail_with_state(
    state: &DesktopState,
    request: ThreadDetailRequest,
) -> Result<Option<ThreadDetail>> {
    let paths = state.codex_paths();
    let detail = thread_detail(&paths, &request.id)?;
    let Some(mut detail) = detail else {
        return Ok(None);
    };
    apply_running_job_to_detail(state, &mut detail)?;
    Ok(Some(window_thread_detail(
        detail,
        detail_block_limit(request.limit, request.full),
        request.before.as_deref(),
    )))
}

pub fn desktop_thread_blocks_with_state(
    state: &DesktopState,
    request: ThreadBlocksRequest,
) -> Result<Option<DesktopThreadBlockPage>> {
    let Some(detail) = desktop_thread_detail_with_state(
        state,
        ThreadDetailRequest {
            id: request.id.clone(),
            limit: Some(block_page_limit(request.limit)),
            full: Some(false),
            before: request.before,
        },
    )?
    else {
        return Ok(None);
    };
    Ok(Some(DesktopThreadBlockPage {
        thread_id: request.id,
        blocks: detail.blocks,
        total_blocks: detail.total_blocks,
        has_more_blocks: detail.has_more_blocks,
        before_cursor: detail.before_cursor,
    }))
}

pub async fn desktop_probe_status() -> Result<ProbeStatus> {
    let state = DesktopState::current()?;
    desktop_probe_status_with_state(&state).await
}

pub async fn desktop_probe_status_with_state(state: &DesktopState) -> Result<ProbeStatus> {
    ProbeRuntime::new(state.config(), state.platform().clone())
        .status()
        .await
}

pub fn desktop_archive_plan() -> Result<ArchiveDeletePlan> {
    let state = DesktopState::current()?;
    desktop_archive_plan_with_state(&state)
}

pub fn desktop_archive_plan_with_state(state: &DesktopState) -> Result<ArchiveDeletePlan> {
    plan_delete_archived(&state.codex_paths())
}

pub fn desktop_hidden_plan() -> Result<HiddenThreadDeletePlan> {
    let state = DesktopState::current()?;
    desktop_hidden_plan_with_state(&state)
}

pub fn desktop_hidden_plan_with_state(state: &DesktopState) -> Result<HiddenThreadDeletePlan> {
    plan_delete_hidden(&state.codex_paths())
}

pub fn desktop_save_goal(request: DesktopGoalRequest) -> Result<DesktopGoal> {
    let state = DesktopState::current()?;
    desktop_save_goal_with_state(&state, request)
}

pub fn desktop_save_goal_with_state(
    state: &DesktopState,
    request: DesktopGoalRequest,
) -> Result<DesktopGoal> {
    let plan = goal_service::plan_save_goal(goal_service::GoalUpdateRequest {
        thread_id: Some(request.thread_id),
        objective: request.objective,
        token_budget: request.token_budget,
        status: None,
        enabled: None,
    })?;
    upsert_desktop_goal_with_state(state, plan.as_thread_goal_update())
}

pub fn desktop_clear_goal(thread_id: &str) -> Result<DesktopGoal> {
    let state = DesktopState::current()?;
    desktop_clear_goal_with_state(&state, thread_id)
}

pub fn desktop_clear_goal_with_state(state: &DesktopState, thread_id: &str) -> Result<DesktopGoal> {
    let plan = goal_service::plan_clear_goal(thread_id)?;
    upsert_desktop_goal_with_state(state, plan.as_thread_goal_update())
}

pub fn desktop_pause_goal(thread_id: &str) -> Result<DesktopGoal> {
    let state = DesktopState::current()?;
    desktop_pause_goal_with_state(&state, thread_id)
}

pub fn desktop_pause_goal_with_state(state: &DesktopState, thread_id: &str) -> Result<DesktopGoal> {
    update_existing_desktop_goal_status_with_state(state, thread_id, "paused")
}

pub fn desktop_resume_goal(thread_id: &str) -> Result<DesktopGoal> {
    let state = DesktopState::current()?;
    desktop_resume_goal_with_state(&state, thread_id)
}

pub fn desktop_resume_goal_with_state(
    state: &DesktopState,
    thread_id: &str,
) -> Result<DesktopGoal> {
    update_existing_desktop_goal_status_with_state(state, thread_id, "active")
}

pub fn desktop_send_message_with_state(
    state: &DesktopState,
    mut request: DesktopSendMessageRequest,
) -> Result<CodexActionResult> {
    let Some(thread_id) = request
        .thread_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return start_codex_new_thread_job(state, request);
    };
    request.thread_id = Some(thread_id.to_string());
    start_codex_job_from_request(state, request, job_service::CodexActionKind::Resume)
}

pub fn desktop_continue_thread_with_state(
    state: &DesktopState,
    mut request: DesktopSendMessageRequest,
) -> Result<CodexActionResult> {
    let Some(thread_id) = request
        .thread_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        anyhow::bail!("thread_id is required");
    };
    request.thread_id = Some(thread_id.to_string());
    start_codex_job_from_request(state, request, job_service::CodexActionKind::Resume)
}

pub fn desktop_stop_thread_with_state(
    state: &DesktopState,
    request: DesktopStopRequest,
) -> Result<DesktopActionResponse> {
    let job_id = request
        .job_id
        .clone()
        .or_else(|| derive_active_job_id(state, &request.thread_id));
    let Some(job_id) = job_id else {
        return Ok(unavailable_action(
            "desktop_stop_thread",
            "stop requires a running local fallback job; Codex app-server stop is not available in the native read model",
        ));
    };
    let cancelled = state.jobs.cancel_job(&job_id)?;
    Ok(DesktopActionResponse {
        ok: cancelled,
        available: true,
        command: "desktop_stop_thread".to_string(),
        message: if cancelled {
            "sent TERM to local Codex job".to_string()
        } else {
            "local job is no longer running".to_string()
        },
        thread_id: Some(request.thread_id),
        job_id: Some(job_id),
        data: Some(json!({"turn_id": request.turn_id})),
    })
}

pub fn desktop_plan_accept_with_state(
    state: &DesktopState,
    request: DesktopPlanAcceptRequest,
) -> Result<CodexActionResult> {
    let _ = (request.turn_id, request.item_id);
    start_codex_resume_job(
        state,
        &request.thread_id,
        job_service::plan_accept_resume_message(),
    )
}

pub fn desktop_plan_revise_with_state(
    state: &DesktopState,
    request: DesktopPlanReviseRequest,
) -> Result<CodexActionResult> {
    let _ = (request.turn_id, request.item_id);
    let instructions = request.instructions.trim();
    if instructions.is_empty() {
        anyhow::bail!("revision instructions cannot be empty");
    }
    start_codex_resume_job(
        state,
        &request.thread_id,
        job_service::plan_revise_resume_message(instructions),
    )
}

pub fn desktop_answer_elicitation_with_state(
    state: &DesktopState,
    request: DesktopElicitationAnswerRequest,
) -> Result<CodexActionResult> {
    let message = job_service::elicitation_answer_resume_message(&request.answers);
    if message.trim().is_empty() {
        anyhow::bail!("answers cannot be empty");
    }
    start_codex_resume_job(state, &request.thread_id, message)
}

pub fn desktop_archive_thread_with_state(
    state: &DesktopState,
    request: DesktopThreadIdRequest,
) -> Result<DesktopActionResponse> {
    set_thread_archived(&state.codex_paths(), &request.thread_id, true)?;
    Ok(job_service::archive_thread_response(request.thread_id, true).into())
}

pub fn desktop_restore_thread_with_state(
    state: &DesktopState,
    request: DesktopThreadIdRequest,
) -> Result<DesktopActionResponse> {
    set_thread_archived(&state.codex_paths(), &request.thread_id, false)?;
    Ok(job_service::archive_thread_response(request.thread_id, false).into())
}

pub fn desktop_rename_thread_with_state(
    state: &DesktopState,
    request: DesktopRenameThreadRequest,
) -> Result<DesktopActionResponse> {
    let name = request.name.trim();
    if name.is_empty() {
        anyhow::bail!("name cannot be empty");
    }
    set_thread_title(&state.codex_paths(), &request.thread_id, name)?;
    job_service::rename_thread_response(request.thread_id, name).map(Into::into)
}

pub fn desktop_fork_thread_with_state(request: DesktopThreadIdRequest) -> DesktopActionResponse {
    let mut response = unavailable_action(
        "desktop_fork_thread",
        "fork is unavailable in the local Codex read model",
    );
    response.thread_id = Some(request.thread_id);
    response
}

pub fn desktop_probe_settings_with_state(state: &DesktopState) -> Result<DesktopProbeSettings> {
    let config = state.config();
    let secret_state = settings_service::ProbeSecretState::from_secret_bytes(
        state
            .db
            .get_secret_setting_bytes(settings_service::PROBE_BARK_DEVICE_KEY_SETTING)?
            .as_deref(),
    );
    Ok(DesktopProbeSettings::from(
        settings_service::build_settings_view(&config, secret_state),
    ))
}

pub fn desktop_probe_save_settings_with_state(
    state: &DesktopState,
    request: DesktopProbeSettingsRequest,
) -> Result<DesktopProbeSettings> {
    let config_path = state.platform().config_file.clone();
    if !config_path.exists() {
        anyhow::bail!("config file not found: {}", config_path.display());
    }
    let (mut probe_patch, mut device_key) = request
        .probe
        .map(DesktopProbeSettingsPatch::into_config_patch)
        .unwrap_or_default();
    if let Some(notifications) = request.notifications {
        if let Some(top_level_device_key) =
            settings_service::normalize_bark_device_key(notifications.device_key)
        {
            device_key = Some(top_level_device_key);
        }
        let mut nested = probe_patch.notifications.unwrap_or_default();
        settings_service::merge_probe_notification_patch(&mut nested, notifications.patch);
        probe_patch.notifications = Some(nested);
    }
    let patch = settings_service::normalize_probe_config_file_patch(ProbeConfigFilePatch {
        codex: request.codex,
        probe: Some(probe_patch),
    })?;
    let text = std::fs::read_to_string(&config_path)?;
    let updated = patch_probe_config_toml(&text, &patch)?;
    std::fs::write(&config_path, updated)?;
    let response_config = Config::load(&config_path)?;
    if let Some(device_key) = device_key {
        state.db.set_secret_setting_bytes(
            settings_service::PROBE_BARK_DEVICE_KEY_SETTING,
            device_key.as_bytes(),
        )?;
    }
    state.replace_config(response_config);
    desktop_probe_settings_with_state(state)
}

pub fn desktop_probe_logs_db_maintain_with_state(
    state: &DesktopState,
    request: DesktopLogsDbMaintainRequest,
) -> Result<DesktopActionResponse> {
    let dry_run = request.dry_run.unwrap_or(true);
    let compact = request.compact.unwrap_or(false);
    let job_id = format!(
        "desktop-probe-logs-db-{}",
        chrono::Utc::now().timestamp_micros()
    );
    let title = if dry_run {
        "Probe logs-db dry-run"
    } else {
        "Probe logs-db execute"
    };
    state
        .db
        .create_job(&job_id, "probe_logs_db_maintain", title)?;

    let run = (|| -> Result<ProbeLogsDbMaintenanceResult> {
        let result = ProbeRuntime::new(state.config(), state.platform().clone())
            .maintain_logs_db_with_compaction(dry_run, compact && !dry_run)?;
        state.db.set_setting(
            PROBE_LOGS_DB_LAST_MAINTAIN_SETTING,
            &serde_json::to_string(&result)?,
        )?;
        Ok(result)
    })();

    match run {
        Ok(result) => {
            state.db.append_job_output(
                &job_id,
                &format!("{}\n", serde_json::to_string_pretty(&result)?),
            )?;
            state.db.finish_job(&job_id, "succeeded", Some(0), None)?;
            Ok(ok_action(
                "desktop_probe_logs_db_maintain",
                "Probe logs-db maintenance completed",
                None,
                Some(job_id),
                Some(serde_json::to_value(result)?),
            ))
        }
        Err(err) => {
            let message = err.to_string();
            let _ = state
                .db
                .append_job_output(&job_id, &format!("error: {message}\n"));
            let _ = state.db.finish_job(&job_id, "failed", None, Some(&message));
            Err(err)
        }
    }
}

pub fn desktop_probe_bark_test_with_state(state: &DesktopState) -> Result<DesktopActionResponse> {
    let device_key_configured = state
        .db
        .get_secret_setting_bytes(settings_service::PROBE_BARK_DEVICE_KEY_SETTING)?
        .is_some_and(|value| !value.is_empty());
    let runtime = ProbeRuntime::new(state.config(), state.platform().clone());
    let plan = runtime.bark_test_plan(device_key_configured);
    let binary = state.platform().daemon_binary();
    if !binary.is_file() {
        return Ok(unavailable_action(
            "desktop_probe_bark_test",
            &format!(
                "Probe Bark test requires local nexushubd binary; plan is available but job cannot start: {}",
                binary.display()
            ),
        )
        .with_data(serde_json::to_value(plan)?));
    }
    match fixed_probe_command_for_platform(
        state.platform(),
        &state.platform().config_file,
        &["probe".to_string(), "bark-test".to_string()],
    ) {
        Ok(command) => {
            let job_id = state.jobs.start_exclusive_shell_job(
                "probe_bark_test",
                "探针 Bark 测试",
                command,
                "probe_bark",
            )?;
            Ok(ok_action(
                "desktop_probe_bark_test",
                "started local Probe Bark test job",
                None,
                Some(job_id),
                Some(serde_json::to_value(plan)?),
            ))
        }
        Err(err) => Ok(unavailable_action(
            "desktop_probe_bark_test",
            &format!(
                "Probe Bark test requires local nexushubd binary; plan is available but job cannot start: {err}"
            ),
        )
        .with_data(serde_json::to_value(plan)?)),
    }
}

pub fn desktop_probe_hooks_install_with_state(
    state: &DesktopState,
) -> Result<DesktopActionResponse> {
    let binary = state.platform().daemon_binary();
    if !binary.is_file() {
        return Ok(unavailable_action(
            "desktop_probe_hooks_install",
            &format!(
                "Probe Hook install requires local nexushubd binary: {}",
                binary.display()
            ),
        ));
    }
    match fixed_probe_command_for_platform(
        state.platform(),
        &state.platform().config_file,
        &["probe".to_string(), "hooks-install".to_string()],
    ) {
        Ok(command) => {
            let job_id = state.jobs.start_exclusive_shell_job(
                "probe_hooks_install",
                "探针 Hook 安装",
                command,
                "probe_hooks",
            )?;
            Ok(ok_action(
                "desktop_probe_hooks_install",
                "started local Probe Hook install job",
                None,
                Some(job_id),
                None,
            ))
        }
        Err(err) => Ok(unavailable_action(
            "desktop_probe_hooks_install",
            &format!("Probe Hook install job cannot start: {err}"),
        )),
    }
}

pub fn desktop_archive_delete_dry_run_with_state(
    state: &DesktopState,
) -> Result<ArchiveDeletePlan> {
    plan_delete_archived(&state.codex_paths())
}

pub fn desktop_archive_delete_execute_with_state(
    state: &DesktopState,
) -> Result<ArchiveDeleteResult> {
    execute_delete_archived(&state.codex_paths())
}

pub fn desktop_hidden_delete_dry_run_with_state(
    state: &DesktopState,
) -> Result<HiddenThreadDeletePlan> {
    plan_delete_hidden(&state.codex_paths())
}

pub fn desktop_hidden_delete_execute_with_state(
    state: &DesktopState,
) -> Result<HiddenThreadDeleteResult> {
    execute_delete_hidden(&state.codex_paths())
}

pub fn desktop_probe_events_with_state(
    state: &DesktopState,
    request: DesktopProbeEventsRequest,
) -> Result<DesktopProbeEventsResponse> {
    let limit = request
        .limit
        .unwrap_or(state.config().probe.recent_limit as u32)
        .clamp(1, 500);
    let events = state
        .db
        .list_probe_events(limit)?
        .into_iter()
        .map(redact_probe_event_for_output)
        .collect();
    Ok(DesktopProbeEventsResponse { events, limit })
}

pub fn desktop_delete_upload_with_state(
    state: &DesktopState,
    request: DesktopDeleteUploadRequest,
) -> Result<DesktopDeleteUploadResponse> {
    let root = uploads::upload_root(&state.resolved_codex_paths().home);
    let deleted = uploads::delete_upload(&root, &request.id)?;
    Ok(DesktopDeleteUploadResponse { ok: true, deleted })
}

pub fn desktop_store_uploads_with_state(
    state: &DesktopState,
    files: Vec<DesktopUploadFile>,
) -> Result<uploads::UploadOutcome> {
    let root = uploads::upload_root(&state.resolved_codex_paths().home);
    let mut stored = Vec::new();
    for file in files {
        stored.push(uploads::store_upload(
            &root,
            &file.name,
            Some(&file.mime),
            &file.bytes,
        )?);
    }
    Ok(uploads::UploadOutcome { files: stored })
}

pub fn desktop_jobs_with_state(
    state: &DesktopState,
    request: DesktopJobsRequest,
) -> Result<Vec<DesktopJobResponse>> {
    Ok(state
        .db
        .list_jobs(request.limit.unwrap_or(50).min(200))?
        .into_iter()
        .map(job_response)
        .collect())
}

pub fn desktop_job_detail_with_state(
    state: &DesktopState,
    request: DesktopJobDetailRequest,
) -> Result<Option<DesktopJobResponse>> {
    Ok(state.db.job(&request.id)?.map(job_response))
}

pub fn desktop_list_followups_with_state(
    state: &DesktopState,
    request: DesktopFollowupRequest,
) -> Result<Vec<ThreadFollowUp>> {
    state
        .db
        .list_followups(&request.thread_id, request.limit.unwrap_or(20).min(200))
}

pub fn desktop_enqueue_followup_with_state(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
) -> Result<ThreadFollowUp> {
    let attachments = prepare_request_attachments(state, &request.attachments)?;
    let Some(thread_id) = request
        .thread_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        anyhow::bail!("thread_id is required");
    };
    let thread_id = thread_id.to_string();
    let (message, options) = request
        .into_thread_message(attachments)
        .into_followup_message_and_options()?;
    state.db.enqueue_followup(&thread_id, &message, options)
}

pub fn desktop_cancel_followup_with_state(
    state: &DesktopState,
    request: DesktopCancelFollowupRequest,
) -> Result<DesktopActionResponse> {
    let cancelled = state
        .db
        .cancel_followup(&request.thread_id, &request.followup_id)?;
    Ok(job_service::cancel_followup_response(
        "desktop_cancel_followup",
        request.thread_id,
        request.followup_id,
        cancelled,
    )
    .into())
}

pub async fn desktop_platform_status_with_state(
    state: &DesktopState,
) -> Result<(PlatformPaths, Option<SystemStatus>)> {
    let config = state.config();
    let system = system_status_with_paths(&config, state.platform())
        .await
        .ok();
    Ok((state.platform().clone(), system))
}

pub fn desktop_claude_code_overview() -> Result<ClaudeOverview> {
    let paths = std::env::var_os("NEXUSHUB_CLAUDE_HOME")
        .map(ClaudePaths::new)
        .unwrap_or_else(ClaudePaths::default_for_user);
    claude_overview(&paths)
}

#[cfg(test)]
pub fn desktop_native_command_names() -> Vec<&'static str> {
    vec![
        "desktop_overview",
        "desktop_home",
        "desktop_threads",
        "desktop_thread_detail",
        "desktop_thread_blocks",
        "desktop_send_message",
        "desktop_continue_thread",
        "desktop_stop_thread",
        "desktop_plan_accept",
        "desktop_plan_revise",
        "desktop_answer_elicitation",
        "desktop_save_goal",
        "desktop_clear_goal",
        "desktop_pause_goal",
        "desktop_resume_goal",
        "desktop_archive_thread",
        "desktop_restore_thread",
        "desktop_rename_thread",
        "desktop_fork_thread",
        "desktop_probe_status",
        "desktop_probe_settings",
        "desktop_probe_save_settings",
        "desktop_probe_bark_test",
        "desktop_probe_hooks_install",
        "desktop_probe_logs_db_maintain",
        "desktop_probe_events",
        "desktop_archive_plan",
        "desktop_hidden_plan",
        "desktop_archive_delete_dry_run",
        "desktop_archive_delete_execute",
        "desktop_hidden_delete_dry_run",
        "desktop_hidden_delete_execute",
        "desktop_delete_upload",
        "desktop_upload_files_command",
        "desktop_jobs",
        "desktop_job_detail",
        "desktop_list_followups",
        "desktop_enqueue_followup",
        "desktop_cancel_followup",
        "desktop_platform_status",
        "desktop_claude_code_overview",
    ]
}

fn load_desktop_config() -> Config {
    Config::load(Config::current_default_config_path())
        .unwrap_or_else(|_| Config::for_platform_kind(PlatformKind::Macos))
}

impl DesktopActionResponse {
    fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
}

impl From<job_service::ActionResponse> for DesktopActionResponse {
    fn from(value: job_service::ActionResponse) -> Self {
        Self {
            ok: value.ok,
            available: value.available,
            command: value.command,
            message: value.message,
            thread_id: value.thread_id,
            job_id: value.job_id,
            data: value.data,
        }
    }
}

impl DesktopSendMessageRequest {
    fn into_thread_message(
        self,
        prepared_attachments: Vec<uploads::PreparedAttachment>,
    ) -> job_service::ThreadMessageRequest {
        job_service::ThreadMessageRequest {
            thread_id: self.thread_id,
            message: self.message,
            attachments: self.attachments,
            prepared_attachments,
            model: self.model,
            service_tier: self.service_tier,
            reasoning_effort: self.reasoning_effort,
            cwd: self.cwd,
            permission_profile: self.permission_profile,
            approval_policy: self.approval_policy,
            sandbox_mode: self.sandbox_mode,
            network_access: self.network_access,
            collaboration_mode: self.collaboration_mode,
        }
    }

    fn into_job_action(self, kind: job_service::CodexActionKind) -> job_service::JobActionRequest {
        self.into_thread_message(Vec::new()).into_job_action(kind)
    }
}

impl DesktopProbeSettingsPatch {
    fn into_config_patch(self) -> (ProbeSettingsPatch, Option<String>) {
        let (notifications, device_key) = match self.notifications {
            Some(notifications) => (
                Some(notifications.patch),
                settings_service::normalize_bark_device_key(notifications.device_key),
            ),
            None => (None, None),
        };
        (
            ProbeSettingsPatch {
                enabled: self.enabled,
                poll_seconds: self.poll_seconds,
                recent_limit: self.recent_limit,
                hooks: self.hooks,
                notifications,
                observability: self.observability,
                logs_db: self.logs_db,
            },
            device_key,
        )
    }
}

fn ok_action(
    command: &str,
    message: &str,
    thread_id: Option<String>,
    job_id: Option<String>,
    data: Option<Value>,
) -> DesktopActionResponse {
    job_service::action_ok(command, message, thread_id, job_id, data).into()
}

pub fn unavailable_action(command: &str, message: &str) -> DesktopActionResponse {
    job_service::action_unavailable(command, message).into()
}

fn start_codex_new_thread_job(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
) -> Result<CodexActionResult> {
    start_codex_job_from_request(state, request, job_service::CodexActionKind::Exec)
}

fn start_codex_resume_job(
    state: &DesktopState,
    thread_id: &str,
    message: String,
) -> Result<CodexActionResult> {
    start_codex_job_from_request(
        state,
        DesktopSendMessageRequest {
            thread_id: Some(thread_id.to_string()),
            message,
            ..DesktopSendMessageRequest::default()
        },
        job_service::CodexActionKind::Resume,
    )
}

fn start_codex_job_from_request(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
    kind: job_service::CodexActionKind,
) -> Result<CodexActionResult> {
    let spec = desktop_codex_job_spec_for_request(state, request, kind)?;
    let resolved = state.resolved_codex_paths();
    let job_id = state.jobs.start_codex_job(
        &spec.title,
        &resolved.home,
        &spec.cwd,
        spec.args,
        spec.prompt,
    )?;
    state
        .db
        .link_job_thread(&job_id, spec.thread_id.as_deref(), None)?;
    Ok(job_service::codex_action_submitted(
        spec.thread_id,
        Some(job_id),
    ))
}

fn desktop_codex_job_spec_for_request(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
    kind: job_service::CodexActionKind,
) -> Result<job_service::CodexJobSpec> {
    let attachments = prepare_request_attachments(state, &request.attachments)?;
    let action = request
        .into_thread_message(attachments)
        .into_job_action(kind);
    let config = state.config();
    job_service::build_codex_job_spec(&action, config.codex.workspace.clone())
}

fn prepare_request_attachments(
    state: &DesktopState,
    attachment_ids: &[String],
) -> Result<Vec<uploads::PreparedAttachment>> {
    if attachment_ids.len() > uploads::MAX_UPLOAD_FILES {
        anyhow::bail!("一次最多发送 5 个附件");
    }
    let root = uploads::upload_root(&state.resolved_codex_paths().home);
    uploads::prepare_uploads(&root, attachment_ids)
}

fn derive_active_job_id(state: &DesktopState, thread_id: &str) -> Option<String> {
    state
        .db
        .running_job_for_thread(thread_id)
        .ok()
        .flatten()
        .map(|job| job.id)
}

fn thread_list_with_jobs(state: &DesktopState, query: ThreadsQuery) -> Result<Vec<ThreadSummary>> {
    let paths = state.codex_paths();
    let fetch_limit = thread_service::thread_list_fetch_limit(query.status.as_deref(), query.limit);
    let hidden_thread_ids = nexushub_core::codex::hidden_thread_ids(&paths).unwrap_or_default();
    let archived_thread_ids = nexushub_core::codex::archived_thread_ids(&paths).unwrap_or_default();
    Ok(thread_service::build_threads_overview(
        list_threads(&paths, None, query.q.as_deref(), fetch_limit)?,
        state.db.running_thread_jobs()?,
        query,
        &hidden_thread_ids,
        &archived_thread_ids,
    )
    .threads)
}

fn apply_running_job_to_detail(state: &DesktopState, detail: &mut ThreadDetail) -> Result<()> {
    if let Some(job) = state.db.running_job_for_thread(&detail.summary.id)? {
        thread_service::apply_running_job_to_summary(&mut detail.summary, &job);
    }
    Ok(())
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

pub fn fixed_probe_command_for_platform(
    platform: &PlatformPaths,
    config_path: &Path,
    args: &[String],
) -> Result<String> {
    if args.first().is_none_or(|arg| arg != "probe") {
        anyhow::bail!("unsupported Probe job");
    }
    let binary = platform.daemon_binary();
    let mut parts = vec![
        binary.display().to_string(),
        "--config".to_string(),
        config_path.display().to_string(),
    ];
    parts.extend(args.iter().cloned());
    Ok(parts
        .iter()
        .map(|part| shell_quote(part))
        .collect::<Vec<_>>()
        .join(" "))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn job_response(job: JobRecord) -> DesktopJobResponse {
    let failure_analysis = if job.status == "failed" {
        analyze_job_failure(&job.kind, &job.output, job.error.as_deref(), job.exit_code)
    } else {
        None
    };
    DesktopJobResponse {
        job,
        failure_analysis,
    }
}

fn first_thread_goal(config: &Config, first_thread: Option<&ThreadSummary>) -> DesktopGoal {
    let Some(thread) = first_thread else {
        return desktop_goal_from_view(goal_service::goal_empty("missing_thread"));
    };
    get_desktop_goal(config, &thread.id).unwrap_or_else(|err| DesktopGoal {
        available: false,
        enabled: false,
        thread_id: Some(thread.id.clone()),
        objective: None,
        token_budget: None,
        status: "unavailable".to_string(),
        completed_at: None,
        blocked_reason: Some(err.to_string()),
    })
}

fn get_desktop_goal(config: &Config, thread_id: &str) -> Result<DesktopGoal> {
    let db = open_panel_db(config)?;
    let Some(goal) = db.get_thread_goal(thread_id)? else {
        return Ok(desktop_goal_with_thread_id(
            goal_service::goal_empty("idle"),
            Some(thread_id.to_string()),
        ));
    };
    Ok(goal_response(&goal))
}

fn upsert_desktop_goal_with_state(
    state: &DesktopState,
    update: nexushub_core::db::ThreadGoalUpdate<'_>,
) -> Result<DesktopGoal> {
    let goal = state.db.upsert_thread_goal(update)?;
    Ok(goal_response(&goal))
}

fn update_existing_desktop_goal_status_with_state(
    state: &DesktopState,
    thread_id: &str,
    status: &'static str,
) -> Result<DesktopGoal> {
    let existing = state.db.get_thread_goal(thread_id)?;
    let plan = goal_service::plan_goal_status_for_thread(thread_id, existing.as_ref(), status)?;
    let goal = state.db.upsert_thread_goal(plan.as_thread_goal_update())?;
    Ok(goal_response(&goal))
}

fn open_panel_db(config: &Config) -> Result<PanelDb> {
    let secret_box = config
        .secret_box()
        .unwrap_or_else(|_| SecretBox::deterministic_dev());
    PanelDb::open_with_secret_box(&config.paths.db_path, secret_box)
}

fn goal_response(goal: &nexushub_core::db::ThreadGoal) -> DesktopGoal {
    desktop_goal_from_view(goal_service::goal_response(Some(goal)))
}

fn desktop_goal_from_view(view: goal_service::GoalView) -> DesktopGoal {
    desktop_goal_with_thread_id(view, None)
}

fn desktop_goal_with_thread_id(
    view: goal_service::GoalView,
    thread_id: Option<String>,
) -> DesktopGoal {
    DesktopGoal {
        available: view.available,
        enabled: view.enabled,
        thread_id: view.thread_id.or(thread_id),
        objective: view.objective,
        token_budget: view.token_budget,
        status: view.status,
        completed_at: view.completed_at,
        blocked_reason: view.blocked_reason,
    }
}

fn overview_warning(overview: &DesktopOverview) -> Vec<String> {
    let mut warnings = Vec::new();
    if !overview.app_support_dir_ready {
        warnings.push("配置目录尚未创建".to_string());
    }
    if !overview.log_dir_ready {
        warnings.push("日志目录尚未创建".to_string());
    }
    if !overview.config_file_exists {
        warnings.push("未找到 config.toml，将使用内置默认配置".to_string());
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_desktop_state() -> (tempfile::TempDir, DesktopState) {
        let temp = tempfile::tempdir().unwrap();
        let mut config = Config::for_platform_kind_with_home(PlatformKind::Macos, temp.path());
        config.paths.data_dir = temp.path().join("data");
        config.paths.db_path = temp.path().join("panel.sqlite");
        config.paths.log_dir = temp.path().join("logs");
        config.codex.home = temp.path().join("codex-home");
        config.codex.workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&config.paths.data_dir).unwrap();
        std::fs::create_dir_all(&config.paths.log_dir).unwrap();
        std::fs::create_dir_all(&config.codex.home).unwrap();
        std::fs::create_dir_all(config.codex.home.join("sessions")).unwrap();
        std::fs::create_dir_all(&config.codex.workspace).unwrap();
        let db =
            PanelDb::open_with_secret_box(&config.paths.db_path, SecretBox::deterministic_dev())
                .unwrap();
        let state = DesktopState::new(
            config,
            db,
            PlatformPaths::for_kind_with_home(PlatformKind::Macos, temp.path()),
        );
        (temp, state)
    }

    #[test]
    fn mac_style_paths_use_nexushub_application_support_and_logs() {
        let paths = nexus_paths_for_home("/Users/example");

        assert_eq!(
            paths.app_support_dir,
            PathBuf::from("/Users/example/Library/Application Support/NexusHub")
        );
        assert_eq!(
            paths.log_dir,
            PathBuf::from("/Users/example/Library/Logs/NexusHub")
        );
        assert_eq!(
            paths.config_file,
            PathBuf::from("/Users/example/Library/Application Support/NexusHub/config.toml")
        );
        assert_eq!(
            paths.app_log_file,
            PathBuf::from("/Users/example/Library/Logs/NexusHub/nexushub.log")
        );
    }

    #[test]
    fn desktop_overview_exposes_native_paths_without_external_entry_fields() {
        let overview = build_desktop_overview().unwrap();

        assert_eq!(overview.product_name, "NexusHub");
        assert!(overview
            .paths
            .app_support_dir
            .ends_with("Library/Application Support/NexusHub"));
        assert!(overview.paths.log_dir.ends_with("Library/Logs/NexusHub"));
    }

    #[test]
    fn desktop_goal_response_tracks_enabled_state() {
        let active = goal_response(&nexushub_core::db::ThreadGoal {
            thread_id: "thread-a".to_string(),
            objective: Some("ship".to_string()),
            token_budget: Some(1000),
            status: "active".to_string(),
            created_at: 1,
            updated_at: 1,
            completed_at: None,
            blocked_reason: None,
        });

        assert!(active.enabled);
        assert_eq!(active.thread_id.as_deref(), Some("thread-a"));
        assert_eq!(active.objective.as_deref(), Some("ship"));

        let cleared = goal_response(&nexushub_core::db::ThreadGoal {
            thread_id: "thread-a".to_string(),
            objective: None,
            token_budget: None,
            status: "cleared".to_string(),
            created_at: 1,
            updated_at: 2,
            completed_at: None,
            blocked_reason: None,
        });

        assert!(!cleared.enabled);
        assert_eq!(cleared.status, "cleared");
    }

    #[test]
    fn desktop_native_command_manifest_covers_app_workflows_without_http_session_commands() {
        let commands = desktop_native_command_names();

        for expected in [
            "desktop_threads",
            "desktop_thread_detail",
            "desktop_send_message",
            "desktop_continue_thread",
            "desktop_stop_thread",
            "desktop_plan_accept",
            "desktop_plan_revise",
            "desktop_answer_elicitation",
            "desktop_save_goal",
            "desktop_clear_goal",
            "desktop_pause_goal",
            "desktop_resume_goal",
            "desktop_archive_thread",
            "desktop_restore_thread",
            "desktop_rename_thread",
            "desktop_probe_status",
            "desktop_probe_settings",
            "desktop_probe_save_settings",
            "desktop_probe_bark_test",
            "desktop_probe_hooks_install",
            "desktop_probe_logs_db_maintain",
            "desktop_probe_events",
            "desktop_archive_plan",
            "desktop_hidden_plan",
            "desktop_archive_delete_dry_run",
            "desktop_archive_delete_execute",
            "desktop_hidden_delete_dry_run",
            "desktop_hidden_delete_execute",
            "desktop_delete_upload",
            "desktop_upload_files_command",
            "desktop_jobs",
            "desktop_job_detail",
            "desktop_list_followups",
            "desktop_enqueue_followup",
            "desktop_cancel_followup",
            "desktop_platform_status",
            "desktop_claude_code_overview",
        ] {
            assert!(
                commands.contains(&expected),
                "missing desktop invoke command: {expected}"
            );
        }

        assert!(
            !commands.contains(&"desktop_api_command"),
            "desktop invoke commands must not expose retired HTTP bridge"
        );
        // Security and Turnstile controls live only on the Linux Web host.
        assert!(
            !commands.contains(&"desktop_security_status"),
            "macOS desktop commands must not expose Web security/Turnstile entry points"
        );
        for forbidden in ["getSecurity", "saveSecurity", "changePassword"] {
            assert!(
                !commands.contains(&forbidden),
                "macOS desktop commands must not expose Web security command: {forbidden}"
            );
        }
        assert!(
            commands.iter().all(|command| !command.contains("login")
                && !command.contains("csrf")
                && !command.contains("desktop_api")),
            "desktop invoke commands must not expose Web auth/session commands"
        );
    }

    #[test]
    fn desktop_probe_save_settings_uses_shared_bark_device_key_constant() {
        let (_temp, state) = test_desktop_state();
        std::fs::create_dir_all(state.platform().config_file.parent().unwrap()).unwrap();
        let config = state.config();
        std::fs::write(
            &state.platform().config_file,
            toml::to_string(&config).unwrap(),
        )
        .unwrap();

        desktop_probe_save_settings_with_state(
            &state,
            DesktopProbeSettingsRequest {
                codex: None,
                probe: None,
                notifications: Some(DesktopProbeNotificationsRequest {
                    device_key: Some("  shared-device-key  ".to_string()),
                    patch: ProbeNotificationsConfigPatch::default(),
                }),
            },
        )
        .unwrap();

        assert_eq!(
            state
                .db
                .get_secret_setting_bytes(settings_service::PROBE_BARK_DEVICE_KEY_SETTING)
                .unwrap()
                .as_deref(),
            Some(b"shared-device-key".as_slice())
        );
        let hardcoded_setter = concat!("set_secret_setting_bytes(\"", "probe_bark_device_key\"");
        assert!(!include_str!("overview.rs").contains(hardcoded_setter));
    }

    #[test]
    fn desktop_typed_uploads_store_under_local_codex_home() {
        let (_temp, state) = test_desktop_state();

        let outcome = desktop_store_uploads_with_state(
            &state,
            vec![DesktopUploadFile {
                name: "note.md".to_string(),
                mime: "text/markdown".to_string(),
                bytes: b"# hello".to_vec(),
            }],
        )
        .unwrap();

        let id = outcome.files[0].id.clone();
        let root = uploads::upload_root(&state.resolved_codex_paths().home);
        assert!(root.join(&id).join("meta.json").is_file());

        let deleted =
            desktop_delete_upload_with_state(&state, DesktopDeleteUploadRequest { id }).unwrap();
        assert!(deleted.ok);
        assert!(deleted.deleted);
    }

    #[test]
    fn desktop_send_message_uses_shared_job_service_and_attachment_context() {
        let (_temp, state) = test_desktop_state();
        let outcome = desktop_store_uploads_with_state(
            &state,
            vec![DesktopUploadFile {
                name: "plan.md".to_string(),
                mime: "text/markdown".to_string(),
                bytes: b"# Plan\nShip parity".to_vec(),
            }],
        )
        .unwrap();
        let cwd = state.config().paths.data_dir.join("custom-cwd");
        std::fs::create_dir_all(&cwd).unwrap();

        let spec = desktop_codex_job_spec_for_request(
            &state,
            DesktopSendMessageRequest {
                message: "请读取附件".to_string(),
                attachments: vec![outcome.files[0].id.clone()],
                model: Some("gpt-5.5".to_string()),
                service_tier: Some("priority".to_string()),
                reasoning_effort: Some("xhigh".to_string()),
                cwd: Some(cwd.display().to_string()),
                permission_profile: Some("danger-full-access".to_string()),
                network_access: Some(true),
                collaboration_mode: Some("async".to_string()),
                ..DesktopSendMessageRequest::default()
            },
            job_service::CodexActionKind::Exec,
        )
        .unwrap();
        assert_eq!(spec.title, "Codex new thread");
        assert_eq!(spec.thread_id, None);
        assert_eq!(spec.cwd, cwd);
        assert!(spec.prompt.contains("请读取附件"), "{}", spec.prompt);
        assert!(spec.prompt.contains("Ship parity"), "{}", spec.prompt);
        assert!(
            spec.args.windows(2).any(|pair| pair == ["-m", "gpt-5.5"]),
            "{:?}",
            spec.args
        );
        assert!(
            spec.args
                .windows(2)
                .any(|pair| pair == ["-c", "model_reasoning_effort=\"xhigh\""]),
            "{:?}",
            spec.args
        );
        assert!(
            spec.args
                .windows(2)
                .any(|pair| pair == ["-c", "model_service_tier=\"priority\""]),
            "{:?}",
            spec.args
        );
        assert!(
            spec.args
                .windows(2)
                .any(|pair| pair == ["-c", "sandbox_mode=\"danger-full-access\""]),
            "{:?}",
            spec.args
        );
        assert!(
            spec.args
                .windows(2)
                .any(|pair| pair == ["-c", "approval_policy=\"never\""]),
            "{:?}",
            spec.args
        );
        assert!(
            spec.args
                .windows(2)
                .any(|pair| pair == ["-c", "network_access=\"enabled\""]),
            "{:?}",
            spec.args
        );
        assert!(
            spec.args
                .windows(2)
                .any(|pair| pair == ["-c", "features.collaboration_modes=true"]),
            "{:?}",
            spec.args
        );
    }

    #[test]
    fn desktop_unsupported_action_is_explicitly_unavailable() {
        let response =
            unavailable_action("desktop_fork_thread", "fork uses Codex app-server state");

        assert!(!response.available);
        assert!(!response.ok);
        assert_eq!(response.command, "desktop_fork_thread");
        assert!(response
            .message
            .contains("fork uses Codex app-server state"));
    }

    #[test]
    fn fixed_probe_command_uses_platform_daemon_path() {
        let platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, "/Users/example");
        let command = fixed_probe_command_for_platform(
            &platform,
            &PathBuf::from("/Users/example/Library/Application Support/NexusHub/config.toml"),
            &["probe".to_string(), "bark-test".to_string()],
        )
        .unwrap();

        assert!(
            command.contains("'/Users/example/Library/Application Support/NexusHub/bin/nexushubd'"),
            "{command}"
        );
        assert!(!command.contains("/opt/nexushub/bin/nexushubd"));
    }

    #[test]
    fn macos_default_config_does_not_use_linux_panel_update_precheck() {
        let config = Config::for_platform_kind_with_home(PlatformKind::Macos, "/Users/example");

        assert!(config
            .update
            .panel_update_command
            .contains("nexushub-update"));
        assert!(!config.update.panel_precheck_command.contains("systemctl"));
        assert!(!config
            .update
            .panel_precheck_command
            .contains("127.0.0.1:15742/healthz"));
        assert!(!config
            .update
            .panel_precheck_command
            .contains("/opt/nexushub"));
        assert!(config
            .update
            .panel_precheck_command
            .contains("Library/Application Support/NexusHub"));
    }
}
