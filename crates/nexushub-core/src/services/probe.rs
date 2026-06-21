use crate::{
    codex::{self, CodexPaths, ThreadStatus, ThreadSummary},
    config::Config,
    db::JobRecord,
    platform::PlatformPaths,
    probe as probe_core,
    services::system::{require_capability, Capability},
    services::threads::{self, ThreadListRuntimeState},
};
use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::{path::Path, str::FromStr};

pub const PROBE_REPLY_NEEDED_FRESH_WINDOW_SECONDS: i64 = 10 * 60;

#[derive(Debug, Clone, Default)]
pub struct ProbeStatusAggregation {
    pub recent_event_count: usize,
    pub running_threads: Vec<ThreadSummary>,
    pub reply_needed_threads: Vec<ThreadSummary>,
    pub recoverable_threads: Vec<ThreadSummary>,
}

#[derive(Debug, Clone)]
pub struct ProbeStatusFacadePlan {
    pub required_capability: Capability,
    pub status: ProbeStatusAggregation,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProbeAction {
    #[serde(rename = "barkTest", alias = "bark-test")]
    BarkTest,
    #[serde(rename = "installHooks", alias = "hooks-install")]
    InstallHooks,
    #[serde(rename = "logsDbDryRun", alias = "logs-db-dry-run")]
    LogsDbDryRun,
    #[serde(rename = "logsDbExecute", alias = "logs-db-execute")]
    LogsDbExecute,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeExecutionKind {
    FixedShellJob,
    LogsDbMaintenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeFixedJobSpec {
    pub kind: String,
    pub title: String,
    pub args: Vec<String>,
    pub command: String,
    pub exclusive_group: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeLogsDbMaintenanceSpec {
    pub dry_run: bool,
    pub compact: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeActionPlan {
    pub required_capability: Capability,
    pub action: ProbeAction,
    pub execution: ProbeExecutionKind,
    pub job: Option<ProbeFixedJobSpec>,
    pub maintenance: Option<ProbeLogsDbMaintenanceSpec>,
    pub diagnostic_plan: Option<probe_core::ProbeActionPlan>,
}

pub fn probe_status_with_capability(
    config: &Config,
    platform: &PlatformPaths,
) -> Result<ProbeStatusFacadePlan> {
    require_capability(platform, Capability::Probe)?;
    Ok(ProbeStatusFacadePlan {
        required_capability: Capability::Probe,
        status: aggregate_probe_status(config),
    })
}

pub fn plan_probe_action(
    config: &Config,
    platform: &PlatformPaths,
    action: ProbeAction,
) -> Result<ProbeActionPlan> {
    plan_probe_action_with_device_key(config, platform, action, false)
}

pub fn plan_probe_action_with_device_key(
    config: &Config,
    platform: &PlatformPaths,
    action: ProbeAction,
    device_key_configured: bool,
) -> Result<ProbeActionPlan> {
    let required_capability = probe_action_capability(action);
    require_capability(platform, required_capability)?;
    let runtime = probe_core::ProbeRuntime::new(config.clone(), platform.clone());
    let diagnostic_plan = match action {
        ProbeAction::BarkTest => Some(runtime.bark_test_plan(device_key_configured)),
        ProbeAction::InstallHooks => {
            Some(runtime.plan_action(probe_core::ProbeActionPlanKind::InstallHooks)?)
        }
        ProbeAction::LogsDbDryRun | ProbeAction::LogsDbExecute => {
            Some(runtime.plan_action(probe_core::ProbeActionPlanKind::LogsDbMaintain)?)
        }
    };
    let job = Some(probe_fixed_job_spec(platform, action)?);
    let maintenance = match action {
        ProbeAction::LogsDbDryRun => Some(ProbeLogsDbMaintenanceSpec {
            dry_run: true,
            compact: false,
        }),
        ProbeAction::LogsDbExecute => Some(ProbeLogsDbMaintenanceSpec {
            dry_run: false,
            compact: false,
        }),
        ProbeAction::BarkTest | ProbeAction::InstallHooks => None,
    };
    let execution = if maintenance.is_some() {
        ProbeExecutionKind::LogsDbMaintenance
    } else {
        ProbeExecutionKind::FixedShellJob
    };
    Ok(ProbeActionPlan {
        required_capability,
        action,
        execution,
        job,
        maintenance,
        diagnostic_plan,
    })
}

pub fn aggregate_probe_status(config: &Config) -> ProbeStatusAggregation {
    let limit = config.probe.recent_limit.clamp(1, 200);
    ProbeStatusAggregation {
        recent_event_count: recent_probe_event_count(&config.paths.db_path, limit as u32)
            .unwrap_or(0),
        running_threads: probe_threads_for_status(config, "running", limit).unwrap_or_default(),
        reply_needed_threads: probe_threads_for_status(config, "reply-needed", limit)
            .unwrap_or_default(),
        recoverable_threads: probe_threads_for_status(config, "recoverable", limit)
            .unwrap_or_default(),
    }
}

pub fn probe_threads_for_status(
    config: &Config,
    status: &str,
    limit: usize,
) -> Result<Vec<ThreadSummary>> {
    let resolved = codex::resolve_codex_paths(&config.codex.home);
    probe_threads_for_status_with_paths(
        &resolved.codex_paths(),
        &config.paths.db_path,
        status,
        limit,
    )
}

pub fn probe_threads_for_status_with_paths(
    paths: &CodexPaths,
    panel_db_path: &Path,
    status: &str,
    limit: usize,
) -> Result<Vec<ThreadSummary>> {
    let limit = limit.clamp(1, 200);
    let local_fetch_limit = threads::thread_list_fetch_limit(Some(status), Some(limit));
    let hidden_thread_ids = codex::hidden_thread_ids(paths).unwrap_or_default();
    let archived_thread_ids = codex::archived_thread_ids(paths).unwrap_or_default();
    let threads = codex::list_threads(paths, None, None, local_fetch_limit)?;
    let running_jobs = running_thread_jobs(panel_db_path).unwrap_or_default();
    let mut threads = threads::apply_thread_list_runtime_state(
        threads,
        ThreadListRuntimeState {
            running_jobs: &running_jobs,
            hidden_thread_ids: &hidden_thread_ids,
            archived_thread_ids: &archived_thread_ids,
        },
    );
    if status == "reply-needed" {
        threads.retain(probe_reply_needed_thread_is_fresh);
    }
    Ok(threads::thread_summaries_for_status(threads, status, limit))
}

impl ProbeAction {
    pub fn as_rpc_action(self) -> &'static str {
        match self {
            Self::BarkTest => "bark-test",
            Self::InstallHooks => "hooks-install",
            Self::LogsDbDryRun => "logs-db-dry-run",
            Self::LogsDbExecute => "logs-db-execute",
        }
    }

    pub fn as_desktop_command(self) -> &'static str {
        match self {
            Self::BarkTest => "startProbeBarkTest",
            Self::InstallHooks => "startProbeHooksInstall",
            Self::LogsDbDryRun => "startProbeLogsDbDryRun",
            Self::LogsDbExecute => "startProbeLogsDbExecute",
        }
    }
}

impl FromStr for ProbeAction {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim() {
            "barkTest" | "bark-test" => Ok(Self::BarkTest),
            "installHooks" | "hooks-install" => Ok(Self::InstallHooks),
            "logsDbDryRun" | "logs-db-dry-run" => Ok(Self::LogsDbDryRun),
            "logsDbExecute" | "logs-db-execute" => Ok(Self::LogsDbExecute),
            action => Err(anyhow!("unknown probe action: {action}")),
        }
    }
}

fn probe_action_capability(action: ProbeAction) -> Capability {
    match action {
        ProbeAction::BarkTest | ProbeAction::InstallHooks => Capability::Probe,
        ProbeAction::LogsDbDryRun | ProbeAction::LogsDbExecute => Capability::ProbeLogMaintenance,
    }
}

fn probe_fixed_job_spec(
    platform: &PlatformPaths,
    action: ProbeAction,
) -> Result<ProbeFixedJobSpec> {
    let (kind, title, args, exclusive_group) = match action {
        ProbeAction::BarkTest => (
            "probe_bark_test",
            "探针 Bark 测试",
            vec!["probe", "bark-test"],
            "probe_bark",
        ),
        ProbeAction::InstallHooks => (
            "probe_hooks_install",
            "探针 Hook 安装",
            vec!["probe", "hooks-install"],
            "probe_hooks",
        ),
        ProbeAction::LogsDbDryRun => (
            "probe_logs_db_maintain_dry_run",
            "Codex logs DB 维护 dry-run",
            vec!["probe", "logs-db-maintain", "--dry-run"],
            "probe_logs_db",
        ),
        ProbeAction::LogsDbExecute => (
            "probe_logs_db_maintain",
            "Codex logs DB 维护",
            vec!["probe", "logs-db-maintain"],
            "probe_logs_db",
        ),
    };
    let args = args.into_iter().map(str::to_string).collect::<Vec<_>>();
    Ok(ProbeFixedJobSpec {
        kind: kind.to_string(),
        title: title.to_string(),
        command: fixed_probe_shell_command(platform, &args),
        args,
        exclusive_group: Some(exclusive_group.to_string()),
    })
}

fn fixed_probe_shell_command(platform: &PlatformPaths, args: &[String]) -> String {
    let mut parts = vec![
        platform.daemon_binary().display().to_string(),
        "--config".to_string(),
        platform.config_file.display().to_string(),
    ];
    parts.extend(args.iter().cloned());
    parts
        .iter()
        .map(|part| shell_quote(part))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '/' | '.' | ':' | '='))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

fn recent_probe_event_count(path: &Path, limit: u32) -> rusqlite::Result<usize> {
    let conn = open_readonly_panel_db(path)?;
    if !table_exists(&conn, "probe_events")? {
        return Ok(0);
    }
    conn.query_row(
        "SELECT COUNT(*) FROM (
            SELECT 1 FROM probe_events ORDER BY created_at DESC, rowid DESC LIMIT ?1
        )",
        params![limit.clamp(1, 500)],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count.max(0) as usize)
}

fn running_thread_jobs(path: &Path) -> rusqlite::Result<Vec<JobRecord>> {
    let conn = open_readonly_panel_db(path)?;
    if !table_exists(&conn, "jobs")? {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        r#"
        SELECT id, kind, status, title, thread_id, turn_id, started_at, finished_at, exit_code,
               substr(output, 1, 24000), error
        FROM jobs
        WHERE status='running' AND thread_id IS NOT NULL
        ORDER BY started_at DESC
        "#,
    )?;
    let rows = stmt.query_map([], job_from_row)?;
    rows.collect()
}

fn open_readonly_panel_db(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
}

fn table_exists(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        params![name],
        |row| row.get::<_, i64>(0),
    )
    .map(|count| count > 0)
}

fn probe_reply_needed_thread_is_fresh(thread: &ThreadSummary) -> bool {
    if !matches!(thread.status, ThreadStatus::ReplyNeeded) {
        return false;
    }
    if !thread_updated_within(thread, PROBE_REPLY_NEEDED_FRESH_WINDOW_SECONDS) {
        return false;
    }
    thread.pending_elicitation.is_some()
        || thread.latest_message.as_deref().is_some_and(|value| {
            value.contains("<proposed_plan>")
                || value.contains("</proposed_plan>")
                || !value.trim().is_empty()
        })
}

fn thread_updated_within(thread: &ThreadSummary, max_age_seconds: i64) -> bool {
    let Some(updated_at) = thread.updated_at.as_deref() else {
        return false;
    };
    let Ok(updated_at) = chrono::DateTime::parse_from_rfc3339(updated_at) else {
        return false;
    };
    let age_seconds = Utc::now()
        .signed_duration_since(updated_at.with_timezone(&Utc))
        .num_seconds();
    (0..=max_age_seconds).contains(&age_seconds)
}

fn job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JobRecord> {
    Ok(JobRecord {
        id: row.get(0)?,
        kind: row.get(1)?,
        status: row.get(2)?,
        title: row.get(3)?,
        thread_id: row.get(4)?,
        turn_id: row.get(5)?,
        started_at: row.get(6)?,
        finished_at: row.get(7)?,
        exit_code: row.get(8)?,
        output: row.get(9)?,
        error: row.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::probe_status_with_capability;
    use crate::{
        config::Config,
        platform::{PlatformKind, PlatformPaths},
        services::system::Capability,
    };

    #[test]
    fn probe_status_facade_requires_probe_capability() {
        let config = Config::for_platform_kind(PlatformKind::Linux);
        let platform = PlatformPaths::for_kind(PlatformKind::Linux);

        let status = probe_status_with_capability(&config, &platform)
            .expect("Linux should allow probe facade");

        assert_eq!(status.required_capability, Capability::Probe);
    }

    #[test]
    fn probe_status_facade_rejects_unsupported_platform() {
        let config = Config::for_platform_kind(PlatformKind::Windows);
        let platform = PlatformPaths::for_kind(PlatformKind::Windows);

        let err = probe_status_with_capability(&config, &platform)
            .expect_err("Windows should not expose probe facade");

        assert!(err.to_string().contains("probe is unavailable on windows"));
    }
}
