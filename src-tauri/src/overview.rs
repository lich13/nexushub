use anyhow::{anyhow, Result};
use nexushub_core::{
    archive::{ArchiveDeletePlan, HiddenThreadDeletePlan},
    codex::{resolve_codex_paths, CodexPaths, MessageBlock, ThreadSummary},
    config::{
        CodexProbeConfigPatch, Config, ProbeHooksConfigPatch, ProbeLogsDbConfigPatch,
        ProbeNotificationsConfigPatch, ProbeObservabilityConfigPatch, ProbeSettingsPatch,
    },
    crypto::SecretBox,
    db::{JobRecord, PanelDb, ProbeEvent},
    jobs::JobRunner,
    local::{
        default_codex_models, default_permission_profiles, local_codex_config,
        local_plugin_catalog, CodexModelInfo, CodexPermissionProfile, LocalCodexConfig,
        LocalPluginInfo,
    },
    platform::{PlatformKind, PlatformPaths},
    probe::{ProbeLogsDbStatus, ProbeRuntime, ProbeStatus},
    services::{jobs as job_service, settings as settings_service},
    system::{system_status_with_paths, SystemStatus},
    update::JobFailureAnalysis,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

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
        let mut config = load_desktop_config();
        std::fs::create_dir_all(&config.paths.data_dir)?;
        std::fs::create_dir_all(&config.paths.log_dir)?;
        if nexushub_core::codex::is_macos_network_volume_path(&config.codex.workspace) {
            config.codex.workspace = default_local_desktop_workspace()
                .ok_or_else(|| anyhow!("cannot resolve local desktop workspace"))?;
        }
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

fn default_local_desktop_workspace() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join("nexushub-workspace"))
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

impl DesktopActionResponse {
    pub(crate) fn with_data(mut self, data: Value) -> Self {
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

impl DesktopProbeSettingsPatch {
    pub(crate) fn into_config_patch(self) -> (ProbeSettingsPatch, Option<String>) {
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
    let runtime = ProbeRuntime::new(config.clone(), state.platform().clone());

    let system = system_status_with_paths(&config, state.platform())
        .await
        .ok();
    let probe = runtime.status().await.ok();
    let logs_db = Some(runtime.logs_db_status());
    let threads = crate::commands::threads::threads_for_home(state).unwrap_or_else(|err| {
        warnings.push(format!("线程读取失败: {err}"));
        Vec::new()
    });
    let archive_plan = None;
    let hidden_plan = None;
    let goal = crate::commands::settings::first_thread_goal(&config, threads.first());

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

fn load_desktop_config() -> Config {
    Config::load(Config::current_default_config_path())
        .unwrap_or_else(|_| Config::for_platform_kind(PlatformKind::Macos))
}

pub(crate) fn open_panel_db(config: &Config) -> Result<PanelDb> {
    let secret_box = config
        .secret_box()
        .unwrap_or_else(|_| SecretBox::deterministic_dev());
    PanelDb::open_with_secret_box(&config.paths.db_path, secret_box)
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
    use nexushub_core::uploads;

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
    fn desktop_probe_save_settings_uses_shared_bark_device_key_constant() {
        let (_temp, state) = test_desktop_state();
        std::fs::create_dir_all(state.platform().config_file.parent().unwrap()).unwrap();
        let config = state.config();
        std::fs::write(
            &state.platform().config_file,
            toml::to_string(&config).unwrap(),
        )
        .unwrap();

        crate::commands::settings::test_probe_save_settings_with_state(
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
        assert!(!include_str!("commands/settings.rs").contains(hardcoded_setter));
    }

    #[test]
    fn desktop_typed_uploads_store_under_local_codex_home() {
        let (_temp, state) = test_desktop_state();

        let outcome = crate::commands::settings::store_uploads_with_state(
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

        let deleted = crate::commands::settings::test_delete_upload_with_state(
            &state,
            DesktopDeleteUploadRequest { id },
        )
        .unwrap();
        assert!(deleted.ok);
        assert!(deleted.deleted);
    }

    #[test]
    fn desktop_typed_uploads_use_shared_batch_validation() {
        let (_temp, state) = test_desktop_state();

        let empty_error = crate::commands::settings::store_uploads_with_state(&state, vec![])
            .unwrap_err()
            .to_string();
        assert!(empty_error.contains("没有可上传的文件"));

        let too_many = (0..6)
            .map(|index| DesktopUploadFile {
                name: format!("note-{index}.md"),
                mime: "text/markdown".to_string(),
                bytes: b"# hello".to_vec(),
            })
            .collect();
        let too_many_error = crate::commands::settings::store_uploads_with_state(&state, too_many)
            .unwrap_err()
            .to_string();
        assert!(too_many_error.contains("一次最多上传 5 个文件"));
    }

    #[test]
    fn desktop_send_message_uses_shared_job_service_and_attachment_context() {
        let (_temp, state) = test_desktop_state();
        let outcome = crate::commands::settings::store_uploads_with_state(
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

        let spec = crate::commands::threads::codex_job_spec_for_request(
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
            job_service::action_unavailable("forkThread", "fork uses Codex app-server state");

        assert!(!response.available);
        assert!(!response.ok);
        assert_eq!(response.command, "forkThread");
        assert!(response
            .message
            .contains("fork uses Codex app-server state"));
    }

    #[tokio::test]
    async fn desktop_home_defers_cleanup_dry_run_plans() {
        let (_temp, state) = test_desktop_state();

        let home = build_desktop_home_with_state(&state).await.unwrap();

        assert!(home.archive_plan.is_none());
        assert!(home.hidden_plan.is_none());
    }

    #[test]
    fn fixed_probe_command_uses_platform_daemon_path() {
        let platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, "/Users/example");
        let config = Config::for_platform_kind_with_home(PlatformKind::Macos, "/Users/example");
        let plan = nexushub_core::services::probe::plan_probe_action(
            &config,
            &platform,
            nexushub_core::services::probe::ProbeAction::BarkTest,
        )
        .unwrap();
        let command = plan.job.unwrap().command;

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
