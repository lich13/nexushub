use anyhow::{anyhow, Result};
use nexushub_core::{
    archive::{
        plan_delete_archived, plan_delete_hidden, ArchiveDeletePlan, HiddenThreadDeletePlan,
    },
    codex::{
        list_threads, resolve_codex_paths, thread_detail, CodexPaths, ThreadDetail, ThreadSummary,
    },
    config::Config,
    crypto::SecretBox,
    db::{PanelDb, ThreadGoalUpdate},
    local::{
        default_codex_models, default_permission_profiles, local_codex_config,
        local_plugin_catalog, CodexModelInfo, CodexPermissionProfile, LocalCodexConfig,
        LocalPluginInfo,
    },
    platform::{PlatformKind, PlatformPaths},
    probe::{ProbeLogsDbStatus, ProbeRuntime, ProbeStatus},
    system::{system_status_with_paths, SystemStatus},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    let config = load_desktop_config();
    let platform = PlatformPaths::current();
    let overview = build_desktop_overview()?;
    let mut warnings = overview_warning(&overview);
    let resolved = resolve_codex_paths(&config.codex.home);
    warnings.extend(resolved.discovery_warnings.clone());
    let codex_paths = resolved.codex_paths();
    let runtime = ProbeRuntime::new(config.clone(), platform.clone());

    let system = system_status_with_paths(&config, &platform).await.ok();
    let probe = runtime.status().await.ok();
    let logs_db = Some(runtime.logs_db_status());
    let threads = list_threads(&codex_paths, None, None, 40).unwrap_or_else(|err| {
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
    let config = load_desktop_config();
    let paths = resolved_codex_paths(&config);
    list_threads(
        &paths,
        request.status.as_deref(),
        request.query.as_deref(),
        request.limit.unwrap_or(80),
    )
}

pub fn desktop_thread_detail(id: &str) -> Result<Option<ThreadDetail>> {
    let config = load_desktop_config();
    let paths = resolved_codex_paths(&config);
    thread_detail(&paths, id)
}

pub async fn desktop_probe_status() -> Result<ProbeStatus> {
    let config = load_desktop_config();
    ProbeRuntime::new(config, PlatformPaths::current())
        .status()
        .await
}

pub fn desktop_archive_plan() -> Result<ArchiveDeletePlan> {
    let config = load_desktop_config();
    let paths = resolved_codex_paths(&config);
    plan_delete_archived(&paths)
}

pub fn desktop_hidden_plan() -> Result<HiddenThreadDeletePlan> {
    let config = load_desktop_config();
    let paths = resolved_codex_paths(&config);
    plan_delete_hidden(&paths)
}

pub fn desktop_save_goal(request: DesktopGoalRequest) -> Result<DesktopGoal> {
    let objective = request
        .objective
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let token_budget = objective.and(request.token_budget);
    upsert_desktop_goal(ThreadGoalUpdate {
        thread_id: &request.thread_id,
        objective,
        token_budget,
        status: objective.map_or("cleared", |_| "active"),
        completed_at: None,
        blocked_reason: None,
    })
}

pub fn desktop_clear_goal(thread_id: &str) -> Result<DesktopGoal> {
    upsert_desktop_goal(ThreadGoalUpdate {
        thread_id,
        objective: None,
        token_budget: None,
        status: "cleared",
        completed_at: None,
        blocked_reason: None,
    })
}

pub fn desktop_pause_goal(thread_id: &str) -> Result<DesktopGoal> {
    update_existing_desktop_goal_status(thread_id, "paused")
}

pub fn desktop_resume_goal(thread_id: &str) -> Result<DesktopGoal> {
    update_existing_desktop_goal_status(thread_id, "active")
}

pub fn desktop_open_config_dir() -> Result<()> {
    open_path(build_desktop_overview()?.paths.app_support_dir)
}

pub fn desktop_open_log_dir() -> Result<()> {
    open_path(build_desktop_overview()?.paths.log_dir)
}

fn load_desktop_config() -> Config {
    Config::load(Config::current_default_config_path())
        .unwrap_or_else(|_| Config::for_platform_kind(PlatformKind::Macos))
}

fn resolved_codex_paths(config: &Config) -> CodexPaths {
    resolve_codex_paths(&config.codex.home).codex_paths()
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

fn update_existing_desktop_goal_status(
    thread_id: &str,
    status: &'static str,
) -> Result<DesktopGoal> {
    let config = load_desktop_config();
    let db = open_panel_db(&config)?;
    let existing = db.get_thread_goal(thread_id)?;
    let objective = existing.as_ref().and_then(|goal| goal.objective.as_deref());
    let token_budget = existing.as_ref().and_then(|goal| goal.token_budget);
    let goal = db.upsert_thread_goal(ThreadGoalUpdate {
        thread_id,
        objective,
        token_budget,
        status,
        completed_at: None,
        blocked_reason: None,
    })?;
    Ok(goal_response(goal))
}

fn upsert_desktop_goal(update: ThreadGoalUpdate<'_>) -> Result<DesktopGoal> {
    let config = load_desktop_config();
    let db = open_panel_db(&config)?;
    let goal = db.upsert_thread_goal(update)?;
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
}
