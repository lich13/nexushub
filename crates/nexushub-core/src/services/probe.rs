use crate::{
    codex::{self, CodexPaths, ThreadStatus, ThreadSummary},
    config::Config,
    db::JobRecord,
};
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OpenFlags};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

pub const PROBE_REPLY_NEEDED_FRESH_WINDOW_SECONDS: i64 = 10 * 60;

#[derive(Debug, Clone, Default)]
pub struct ProbeStatusAggregation {
    pub recent_event_count: usize,
    pub running_threads: Vec<ThreadSummary>,
    pub reply_needed_threads: Vec<ThreadSummary>,
    pub recoverable_threads: Vec<ThreadSummary>,
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
    let local_fetch_limit = if thread_status_filter_needs_full_scan(status) {
        usize::MAX
    } else {
        limit
    };
    let hidden_thread_ids = codex::hidden_thread_ids(paths).unwrap_or_default();
    let archived_thread_ids = codex::archived_thread_ids(paths).unwrap_or_default();
    let mut threads = codex::list_threads(paths, None, None, local_fetch_limit)?;
    threads = prune_hidden_thread_summaries(threads, &hidden_thread_ids);
    let running_jobs = running_thread_jobs(panel_db_path).unwrap_or_default();
    apply_running_jobs_to_threads(&mut threads, &running_jobs, &archived_thread_ids);
    threads = prune_hidden_thread_summaries(threads, &hidden_thread_ids);
    if status == "reply-needed" {
        threads.retain(probe_reply_needed_thread_is_fresh);
    }
    Ok(filter_thread_summaries(threads, status, limit))
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

fn apply_running_jobs_to_threads(
    threads: &mut Vec<ThreadSummary>,
    jobs: &[JobRecord],
    archived_thread_ids: &HashSet<String>,
) {
    let mut by_thread: HashMap<&str, &JobRecord> = HashMap::new();
    for job in jobs {
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
        apply_running_job_to_thread_list(threads, job, archived_thread_ids);
    }
}

fn apply_running_job_to_thread_list(
    threads: &mut Vec<ThreadSummary>,
    job: &JobRecord,
    archived_thread_ids: &HashSet<String>,
) {
    let Some(thread_id) = job.thread_id.as_deref() else {
        return;
    };
    if archived_thread_ids.contains(thread_id) {
        return;
    }
    if let Some(thread) = threads.iter_mut().find(|thread| thread.id == thread_id) {
        apply_running_job_to_summary(thread, job);
        return;
    }
    threads.push(thread_summary_from_running_job(job));
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

fn filter_thread_summaries(
    mut rows: Vec<ThreadSummary>,
    status: &str,
    limit: usize,
) -> Vec<ThreadSummary> {
    rows.retain(|row| thread_matches_status(row, status));
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

fn thread_status_filter_needs_full_scan(status: &str) -> bool {
    matches!(status, "running" | "reply-needed" | "recoverable")
}

fn thread_matches_status(row: &ThreadSummary, status: &str) -> bool {
    matches!(
        (status, &row.status),
        ("running", ThreadStatus::Running)
            | ("reply-needed", ThreadStatus::ReplyNeeded)
            | ("recoverable", ThreadStatus::Recoverable)
    )
}

fn timestamp_to_rfc3339(ts: i64) -> Option<String> {
    chrono::DateTime::from_timestamp(ts, 0).map(|dt| dt.to_rfc3339())
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
