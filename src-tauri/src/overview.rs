use anyhow::{anyhow, Result};
use nexushub_core::{
    archive::{
        plan_delete_archived, plan_delete_hidden, ArchiveDeletePlan, HiddenThreadDeletePlan,
    },
    claude_code::{claude_overview, ClaudeOverview, ClaudePaths},
    codex::{
        list_threads, resolve_codex_paths, set_thread_archived, set_thread_title, thread_detail,
        window_thread_detail, CodexPaths, MessageBlock, ThreadDetail, ThreadSummary,
    },
    config::{
        patch_probe_config_toml, valid_probe_notification_server_url, CodexProbeConfigPatch,
        Config, ProbeConfigFilePatch, ProbeHooksConfigPatch, ProbeLogsDbConfigPatch,
        ProbeNotificationsConfigPatch, ProbeObservabilityConfigPatch, ProbeSettingsPatch,
    },
    crypto::SecretBox,
    db::{JobRecord, PanelDb, SecuritySettings, ThreadFollowUp, ThreadGoalUpdate},
    jobs::{CodexActionResult, JobRunner},
    local::{
        default_codex_models, default_permission_profiles, local_codex_config,
        local_plugin_catalog, CodexModelInfo, CodexPermissionProfile, LocalCodexConfig,
        LocalPluginInfo,
    },
    platform::{PlatformKind, PlatformPaths},
    probe::{ProbeLogsDbMaintenanceResult, ProbeLogsDbStatus, ProbeRuntime, ProbeStatus},
    system::{system_status_with_paths, SystemStatus},
    update::{analyze_job_failure, JobFailureAnalysis},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

const THREAD_DETAIL_DEFAULT_BLOCK_LIMIT: usize = 120;
const THREAD_DETAIL_MAX_BLOCK_LIMIT: usize = 500;
const CODEX_SUBMITTED_MESSAGE: &str = "已提交给 Codex";
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
    pub thread_id: String,
    pub objective: Option<String>,
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
pub struct DesktopSecurityStatus {
    pub available: bool,
    pub mode: String,
    pub admin_required: bool,
    pub csrf_required: bool,
    pub session_required: bool,
    pub settings: SecuritySettings,
    pub turnstile_expected_hostname: Option<String>,
    pub turnstile_expected_action: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProbeSettings {
    pub codex: Value,
    pub probe: nexushub_core::config::ProbeConfig,
    pub notifications: Value,
    pub logs_db: nexushub_core::config::ProbeLogsDbConfig,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopJobResponse {
    #[serde(flatten)]
    pub job: JobRecord,
    pub failure_analysis: Option<JobFailureAnalysis>,
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
    pub thread_id: Option<String>,
    pub message: String,
    #[serde(default)]
    pub attachments: Vec<String>,
    pub model: Option<String>,
    pub service_tier: Option<String>,
    pub reasoning_effort: Option<String>,
    pub cwd: Option<String>,
    pub permission_profile: Option<String>,
    pub approval_policy: Option<String>,
    pub sandbox_mode: Option<String>,
    pub network_access: Option<bool>,
    pub collaboration_mode: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopStopRequest {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub job_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopThreadIdRequest {
    pub thread_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopRenameThreadRequest {
    pub thread_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopPlanAcceptRequest {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopPlanReviseRequest {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub instructions: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopElicitationAnswerRequest {
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
pub struct DesktopFollowupRequest {
    pub thread_id: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopCancelFollowupRequest {
    pub thread_id: String,
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
    let mut threads = list_threads(&codex_paths, None, None, 40).unwrap_or_else(|err| {
        warnings.push(format!("线程读取失败: {err}"));
        Vec::new()
    });
    let _ = apply_running_jobs_to_threads(state, &mut threads);
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
    let paths = state.codex_paths();
    let fetch_limit = thread_list_fetch_limit(request.status.as_deref(), request.limit);
    let mut threads = list_threads(&paths, None, request.query.as_deref(), fetch_limit)?;
    apply_running_jobs_to_threads(state, &mut threads)?;
    Ok(filter_thread_summaries(
        threads,
        request.status.as_deref(),
        request.query.as_deref(),
        request.limit.unwrap_or(80),
    ))
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
    let objective = request
        .objective
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let token_budget = objective.and(request.token_budget);
    upsert_desktop_goal_with_state(
        state,
        ThreadGoalUpdate {
            thread_id: &request.thread_id,
            objective,
            token_budget,
            status: objective.map_or("cleared", |_| "active"),
            completed_at: None,
            blocked_reason: None,
        },
    )
}

pub fn desktop_clear_goal(thread_id: &str) -> Result<DesktopGoal> {
    let state = DesktopState::current()?;
    desktop_clear_goal_with_state(&state, thread_id)
}

pub fn desktop_clear_goal_with_state(state: &DesktopState, thread_id: &str) -> Result<DesktopGoal> {
    upsert_desktop_goal_with_state(
        state,
        ThreadGoalUpdate {
            thread_id,
            objective: None,
            token_budget: None,
            status: "cleared",
            completed_at: None,
            blocked_reason: None,
        },
    )
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

pub fn desktop_open_config_dir() -> Result<()> {
    open_path(build_desktop_overview()?.paths.app_support_dir)
}

pub fn desktop_open_log_dir() -> Result<()> {
    open_path(build_desktop_overview()?.paths.log_dir)
}

pub fn desktop_send_message_with_state(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
) -> Result<CodexActionResult> {
    let Some(thread_id) = request
        .thread_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return start_codex_new_thread_job(state, request);
    };
    start_codex_resume_job(state, thread_id, effective_message(&request.message))
}

pub fn desktop_continue_thread_with_state(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
) -> Result<CodexActionResult> {
    let Some(thread_id) = request
        .thread_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        anyhow::bail!("thread_id is required");
    };
    start_codex_resume_job(state, thread_id, effective_message(&request.message))
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
    start_codex_resume_job(state, &request.thread_id, plan_accept_resume_message())
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
        plan_revise_resume_message(instructions),
    )
}

pub fn desktop_answer_elicitation_with_state(
    state: &DesktopState,
    request: DesktopElicitationAnswerRequest,
) -> Result<CodexActionResult> {
    let message = elicitation_answer_resume_message(&request.answers);
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
    Ok(ok_action(
        "desktop_archive_thread",
        "thread archived in local Codex state",
        Some(request.thread_id),
        None,
        None,
    ))
}

pub fn desktop_restore_thread_with_state(
    state: &DesktopState,
    request: DesktopThreadIdRequest,
) -> Result<DesktopActionResponse> {
    set_thread_archived(&state.codex_paths(), &request.thread_id, false)?;
    Ok(ok_action(
        "desktop_restore_thread",
        "thread restored in local Codex state",
        Some(request.thread_id),
        None,
        None,
    ))
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
    Ok(ok_action(
        "desktop_rename_thread",
        "thread renamed in local Codex state",
        Some(request.thread_id),
        None,
        Some(json!({"name": name})),
    ))
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
    probe_settings_value(state)
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
        if let Some(top_level_device_key) = normalized_device_key(notifications.device_key) {
            device_key = Some(top_level_device_key);
        }
        let mut nested = probe_patch.notifications.unwrap_or_default();
        merge_probe_notification_patch(&mut nested, notifications.patch);
        probe_patch.notifications = Some(nested);
    }
    if let Some(notifications) = probe_patch.notifications.as_ref() {
        if let Some(server_url) = notifications.server_url.as_deref() {
            if !valid_probe_notification_server_url(server_url) {
                anyhow::bail!(
                    "probe notifications server_url must use HTTPS except localhost HTTP"
                );
            }
        }
    }
    let patch = ProbeConfigFilePatch {
        codex: request.codex,
        probe: Some(probe_patch),
    };
    let text = std::fs::read_to_string(&config_path)?;
    let updated = patch_probe_config_toml(&text, &patch)?;
    std::fs::write(&config_path, updated)?;
    let response_config = Config::load(&config_path)?;
    if let Some(device_key) = device_key {
        state
            .db
            .set_secret_setting_bytes("probe_bark_device_key", device_key.as_bytes())?;
    }
    state.replace_config(response_config);
    desktop_probe_settings_with_state(state)
}

pub fn desktop_probe_logs_db_maintain_with_state(
    state: &DesktopState,
    request: DesktopLogsDbMaintainRequest,
) -> Result<ProbeLogsDbMaintenanceResult> {
    let dry_run = request.dry_run.unwrap_or(true);
    let compact = request.compact.unwrap_or(false);
    let result = ProbeRuntime::new(state.config(), state.platform().clone())
        .maintain_logs_db_with_compaction(dry_run, compact && !dry_run)?;
    state.db.set_setting(
        PROBE_LOGS_DB_LAST_MAINTAIN_SETTING,
        &serde_json::to_string(&result)?,
    )?;
    Ok(result)
}

pub fn desktop_probe_bark_test_with_state(state: &DesktopState) -> Result<DesktopActionResponse> {
    let device_key_configured = state
        .db
        .get_secret_setting_bytes("probe_bark_device_key")?
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

pub fn desktop_archive_delete_dry_run_with_state(
    state: &DesktopState,
) -> Result<ArchiveDeletePlan> {
    plan_delete_archived(&state.codex_paths())
}

pub fn desktop_hidden_delete_dry_run_with_state(
    state: &DesktopState,
) -> Result<HiddenThreadDeletePlan> {
    plan_delete_hidden(&state.codex_paths())
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
    let Some(thread_id) = request
        .thread_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        anyhow::bail!("thread_id is required");
    };
    let message = effective_message(&request.message);
    if message.is_empty() {
        anyhow::bail!("follow-up message is required");
    }
    state
        .db
        .enqueue_followup(thread_id, &message, request.options_json())
}

pub fn desktop_cancel_followup_with_state(
    state: &DesktopState,
    request: DesktopCancelFollowupRequest,
) -> Result<DesktopActionResponse> {
    let cancelled = state
        .db
        .cancel_followup(&request.thread_id, &request.followup_id)?;
    Ok(ok_action(
        "desktop_cancel_followup",
        if cancelled {
            "follow-up cancelled"
        } else {
            "follow-up was not pending"
        },
        Some(request.thread_id),
        None,
        Some(json!({"followup_id": request.followup_id, "cancelled": cancelled})),
    ))
}

pub fn desktop_security_status_with_state(state: &DesktopState) -> Result<DesktopSecurityStatus> {
    let config = state.config();
    let settings = state
        .db
        .security_settings(config.security.session_ttl_seconds)?;
    let expected_hostname = state
        .db
        .get_setting("turnstile_expected_hostname")?
        .or_else(|| config.security.turnstile_expected_hostname.clone());
    let expected_action = state
        .db
        .get_setting("turnstile_expected_action")?
        .or_else(|| config.security.turnstile_expected_action.clone());
    Ok(DesktopSecurityStatus {
        available: true,
        mode: "native".to_string(),
        admin_required: false,
        csrf_required: false,
        session_required: false,
        settings,
        turnstile_expected_hostname: expected_hostname,
        turnstile_expected_action: expected_action,
    })
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
        "desktop_probe_logs_db_maintain",
        "desktop_archive_plan",
        "desktop_hidden_plan",
        "desktop_archive_delete_dry_run",
        "desktop_hidden_delete_dry_run",
        "desktop_jobs",
        "desktop_job_detail",
        "desktop_list_followups",
        "desktop_enqueue_followup",
        "desktop_cancel_followup",
        "desktop_security_status",
        "desktop_platform_status",
        "desktop_claude_code_overview",
        "desktop_open_config_dir",
        "desktop_open_log_dir",
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

impl DesktopSendMessageRequest {
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
        })
    }
}

impl DesktopProbeSettingsPatch {
    fn into_config_patch(self) -> (ProbeSettingsPatch, Option<String>) {
        let (notifications, device_key) = match self.notifications {
            Some(notifications) => (
                Some(notifications.patch),
                normalized_device_key(notifications.device_key),
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
    DesktopActionResponse {
        ok: true,
        available: true,
        command: command.to_string(),
        message: message.to_string(),
        thread_id,
        job_id,
        data,
    }
}

pub fn unavailable_action(command: &str, message: &str) -> DesktopActionResponse {
    DesktopActionResponse {
        ok: false,
        available: false,
        command: command.to_string(),
        message: message.to_string(),
        thread_id: None,
        job_id: None,
        data: None,
    }
}

fn effective_message(message: &str) -> String {
    message.trim().to_string()
}

fn start_codex_new_thread_job(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
) -> Result<CodexActionResult> {
    let message = effective_message(&request.message);
    if message.is_empty() {
        anyhow::bail!("message is required");
    }
    if !request.attachments.is_empty() {
        anyhow::bail!("native desktop uploads are not available yet");
    }
    let config = state.config();
    let cwd = request
        .cwd
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| config.codex.workspace.clone());
    let mut args = vec![
        "exec".to_string(),
        "--json".to_string(),
        "--skip-git-repo-check".to_string(),
        "-".to_string(),
    ];
    add_codex_common_args(&mut args, &request);
    let resolved = state.resolved_codex_paths();
    let job_id =
        state
            .jobs
            .start_codex_job("Codex new thread", &resolved.home, &cwd, args, message)?;
    state.db.link_job_thread(&job_id, None, None)?;
    Ok(CodexActionResult {
        bridge: false,
        thread_id: None,
        turn_id: None,
        job_id: Some(job_id),
        fallback: true,
        message: Some(CODEX_SUBMITTED_MESSAGE.to_string()),
    })
}

fn start_codex_resume_job(
    state: &DesktopState,
    thread_id: &str,
    message: String,
) -> Result<CodexActionResult> {
    let message = message.trim().to_string();
    if message.is_empty() {
        anyhow::bail!("message is required");
    }
    let args = codex_resume_args(thread_id);
    let config = state.config();
    let resolved = state.resolved_codex_paths();
    let job_id = state.jobs.start_codex_job(
        "Codex resume thread",
        &resolved.home,
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

fn add_codex_common_args(args: &mut Vec<String>, request: &DesktopSendMessageRequest) {
    if let Some(model) = request
        .model
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        args.splice(1..1, ["-m".to_string(), model.trim().to_string()]);
    }
    if let Some(reasoning) = request
        .reasoning_effort
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        args.splice(
            1..1,
            [
                "-c".to_string(),
                format!(
                    "model_reasoning_effort=\"{}\"",
                    cli_config_string(reasoning)
                ),
            ],
        );
    }
    if let Some(service_tier) = request
        .service_tier
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        args.splice(
            1..1,
            [
                "-c".to_string(),
                format!("model_service_tier=\"{}\"", cli_config_string(service_tier)),
            ],
        );
    }
}

fn cli_config_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn codex_resume_args(thread_id: &str) -> Vec<String> {
    vec![
        "exec".to_string(),
        "resume".to_string(),
        "--all".to_string(),
        "--json".to_string(),
        thread_id.to_string(),
        "-".to_string(),
    ]
}

fn derive_active_job_id(state: &DesktopState, thread_id: &str) -> Option<String> {
    state
        .db
        .running_job_for_thread(thread_id)
        .ok()
        .flatten()
        .map(|job| job.id)
}

fn plan_accept_resume_message() -> String {
    "是，实施此计划".to_string()
}

fn plan_revise_resume_message(instructions: &str) -> String {
    format!(
        "否，请告知 Codex 如何调整\n\n请保持 Plan Mode，只根据下面的修改要求重新给出计划，不要开始实施。\n\n修改要求：\n{}",
        instructions.trim()
    )
}

fn elicitation_answer_resume_message(
    answers: &std::collections::HashMap<String, Vec<String>>,
) -> String {
    let mut rows = answers.iter().collect::<Vec<_>>();
    rows.sort_by_key(|(question, _)| *question);
    rows.into_iter()
        .map(|(question, answers)| format!("{question}: {}", answers.join(", ")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn apply_running_jobs_to_threads(
    state: &DesktopState,
    threads: &mut Vec<ThreadSummary>,
) -> Result<()> {
    let jobs = state.db.running_thread_jobs()?;
    for job in &jobs {
        if let Some(thread_id) = job.thread_id.as_deref() {
            if let Some(thread) = threads.iter_mut().find(|thread| thread.id == thread_id) {
                apply_running_job_to_summary(thread, job);
            } else {
                threads.push(thread_summary_from_running_job(job));
            }
        }
    }
    Ok(())
}

fn apply_running_job_to_detail(state: &DesktopState, detail: &mut ThreadDetail) -> Result<()> {
    if let Some(job) = state.db.running_job_for_thread(&detail.summary.id)? {
        apply_running_job_to_summary(&mut detail.summary, &job);
    }
    Ok(())
}

fn apply_running_job_to_summary(summary: &mut ThreadSummary, job: &JobRecord) {
    if matches!(summary.status, nexushub_core::codex::ThreadStatus::Archived) {
        return;
    }
    summary.status = nexushub_core::codex::ThreadStatus::Running;
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
        status: nexushub_core::codex::ThreadStatus::Running,
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
        if !matches!(status, Some("archived"))
            && matches!(row.status, nexushub_core::codex::ThreadStatus::Archived)
        {
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

fn thread_matches_status(row: &ThreadSummary, status: &str) -> bool {
    use nexushub_core::codex::ThreadStatus;
    matches!(
        (status, &row.status),
        ("running", ThreadStatus::Running)
            | ("reply-needed", ThreadStatus::ReplyNeeded)
            | ("recoverable", ThreadStatus::Recoverable)
            | ("archived", ThreadStatus::Archived)
            | ("recent", ThreadStatus::Recent)
    )
}

fn thread_list_fetch_limit(status: Option<&str>, limit: Option<usize>) -> usize {
    if matches!(status, Some("running" | "reply-needed" | "recoverable")) {
        usize::MAX
    } else {
        limit.unwrap_or(80).clamp(1, 500)
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

fn timestamp_to_rfc3339(timestamp: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(timestamp, 0).map(|value| value.to_rfc3339())
}

fn probe_settings_value(state: &DesktopState) -> Result<DesktopProbeSettings> {
    let config = state.config();
    let resolved = resolve_codex_paths(&config.codex.home);
    let device_key_configured = state
        .db
        .get_secret_setting_bytes("probe_bark_device_key")?
        .is_some_and(|value| !value.is_empty());
    Ok(DesktopProbeSettings {
        codex: json!({
            "home": resolved.configured_codex_home.clone(),
            "configured_codex_home": resolved.configured_codex_home,
            "resolved_codex_home": resolved.home,
            "codex_home_source": resolved.codex_home_source,
            "logs_db_source": resolved.logs_db_source,
            "discovery_warnings": resolved.discovery_warnings,
            "workspace": config.codex.workspace,
            "host_label": config.codex.host_label,
        }),
        probe: config.probe.clone(),
        notifications: json!({
            "device_key_configured": device_key_configured,
            "server_url": config.probe.notifications.server_url,
            "enabled": config.probe.notifications.enabled,
            "sound": config.probe.notifications.sound,
            "group": config.probe.notifications.group,
            "url": config.probe.notifications.url,
            "notify_completion": config.probe.notifications.notify_completion,
            "notify_reply_needed": config.probe.notifications.notify_reply_needed,
            "notify_recoverable": config.probe.notifications.notify_recoverable,
        }),
        logs_db: config.probe.logs_db,
    })
}

fn normalized_device_key(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn merge_probe_notification_patch(
    target: &mut ProbeNotificationsConfigPatch,
    source: ProbeNotificationsConfigPatch,
) {
    if source.enabled.is_some() {
        target.enabled = source.enabled;
    }
    if source.server_url.is_some() {
        target.server_url = source.server_url;
    }
    if source.sound.is_some() {
        target.sound = source.sound;
    }
    if source.group.is_some() {
        target.group = source.group;
    }
    if source.url.is_some() {
        target.url = source.url;
    }
    if source.notify_completion.is_some() {
        target.notify_completion = source.notify_completion;
    }
    if source.notify_reply_needed.is_some() {
        target.notify_reply_needed = source.notify_reply_needed;
    }
    if source.notify_recoverable.is_some() {
        target.notify_recoverable = source.notify_recoverable;
    }
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
        return empty_goal("missing_thread", None);
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
        return Ok(empty_goal("idle", Some(thread_id.to_string())));
    };
    Ok(goal_response(goal))
}

fn upsert_desktop_goal_with_state(
    state: &DesktopState,
    update: ThreadGoalUpdate<'_>,
) -> Result<DesktopGoal> {
    let goal = state.db.upsert_thread_goal(update)?;
    Ok(goal_response(goal))
}

fn update_existing_desktop_goal_status_with_state(
    state: &DesktopState,
    thread_id: &str,
    status: &'static str,
) -> Result<DesktopGoal> {
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
    Ok(goal_response(goal))
}

fn open_panel_db(config: &Config) -> Result<PanelDb> {
    let secret_box = config
        .secret_box()
        .unwrap_or_else(|_| SecretBox::deterministic_dev());
    PanelDb::open_with_secret_box(&config.paths.db_path, secret_box)
}

fn empty_goal(status: &str, thread_id: Option<String>) -> DesktopGoal {
    DesktopGoal {
        available: true,
        enabled: false,
        thread_id,
        objective: None,
        token_budget: None,
        status: status.to_string(),
        completed_at: None,
        blocked_reason: None,
    }
}

fn goal_response(goal: nexushub_core::db::ThreadGoal) -> DesktopGoal {
    let enabled = goal_enabled(&goal);
    DesktopGoal {
        available: true,
        enabled,
        thread_id: Some(goal.thread_id),
        objective: goal.objective,
        token_budget: goal.token_budget,
        status: goal.status,
        completed_at: goal.completed_at,
        blocked_reason: goal.blocked_reason,
    }
}

fn open_path(path: PathBuf) -> Result<()> {
    std::fs::create_dir_all(&path)?;
    opener::open(path)?;
    Ok(())
}

fn goal_enabled(goal: &nexushub_core::db::ThreadGoal) -> bool {
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
        let active = goal_response(nexushub_core::db::ThreadGoal {
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

        let cleared = goal_response(nexushub_core::db::ThreadGoal {
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
            "desktop_archive_plan",
            "desktop_hidden_plan",
            "desktop_jobs",
            "desktop_job_detail",
            "desktop_security_status",
            "desktop_platform_status",
            "desktop_claude_code_overview",
        ] {
            assert!(
                commands.contains(&expected),
                "missing desktop invoke command: {expected}"
            );
        }

        assert!(
            commands
                .iter()
                .all(|command| !command.contains("login") && !command.contains("csrf")),
            "desktop invoke commands must not expose Web auth/session commands"
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
