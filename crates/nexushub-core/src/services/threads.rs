use std::collections::{HashMap, HashSet};

use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};

use crate::{
    codex::{ThreadDetail, ThreadStatus, ThreadSummary},
    db::JobRecord,
    platform::PlatformPaths,
    services::commands,
    services::system::{require_capability, Capability},
};

pub const THREAD_DETAIL_DEFAULT_BLOCK_LIMIT: usize = 120;
pub const THREAD_DETAIL_MAX_BLOCK_LIMIT: usize = 500;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadsQuery {
    pub status: Option<String>,
    pub q: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadsOverview {
    pub threads: Vec<ThreadSummary>,
    pub query: ThreadsQuery,
    pub fetch_limit: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDetailRequest {
    pub id: String,
    pub limit: Option<usize>,
    pub full: Option<bool>,
    pub before: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDetailPlan {
    pub required_capability: Capability,
    pub thread_id: String,
    pub block_limit: Option<usize>,
    pub full: bool,
    pub before: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadCleanupAction {
    #[serde(rename = "archiveDeleteDryRun", alias = "archive-delete-dry-run")]
    ArchiveDeleteDryRun,
    #[serde(rename = "archiveDeleteExecute", alias = "archive-delete-execute")]
    ArchiveDeleteExecute,
    #[serde(rename = "hiddenDeleteDryRun", alias = "hidden-delete-dry-run")]
    HiddenDeleteDryRun,
    #[serde(rename = "hiddenDeleteExecute", alias = "hidden-delete-execute")]
    HiddenDeleteExecute,
}

impl ThreadCleanupAction {
    pub fn as_rpc_action(self) -> &'static str {
        match self {
            Self::ArchiveDeleteDryRun => commands::CLEANUP_ARCHIVE_DRY_RUN,
            Self::ArchiveDeleteExecute => commands::CLEANUP_ARCHIVE_EXECUTE,
            Self::HiddenDeleteDryRun => commands::CLEANUP_HIDDEN_DRY_RUN,
            Self::HiddenDeleteExecute => commands::CLEANUP_HIDDEN_EXECUTE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCleanupPlan {
    pub required_capability: Capability,
    pub action: ThreadCleanupAction,
    pub target: ThreadCleanupTarget,
    pub execute: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadCleanupTarget {
    Archived,
    Hidden,
}

#[derive(Debug, Clone, Copy)]
pub struct ThreadListRuntimeState<'a> {
    pub running_jobs: &'a [JobRecord],
    pub hidden_thread_ids: &'a HashSet<String>,
    pub archived_thread_ids: &'a HashSet<String>,
}

pub fn build_threads_overview(
    threads: Vec<ThreadSummary>,
    running_jobs: Vec<JobRecord>,
    query: ThreadsQuery,
    hidden_thread_ids: &HashSet<String>,
    archived_thread_ids: &HashSet<String>,
) -> ThreadsOverview {
    let response_limit = requested_thread_limit(query.limit);
    let fetch_limit = thread_list_fetch_limit(query.status.as_deref(), query.limit);
    let threads = apply_thread_list_runtime_state(
        threads,
        ThreadListRuntimeState {
            running_jobs: &running_jobs,
            hidden_thread_ids,
            archived_thread_ids,
        },
    );
    let threads = filter_thread_summaries(
        threads,
        query.status.as_deref(),
        query.q.as_deref(),
        response_limit,
    );

    ThreadsOverview {
        threads,
        query,
        fetch_limit,
    }
}

pub fn plan_thread_detail_request(
    platform: &PlatformPaths,
    request: ThreadDetailRequest,
) -> anyhow::Result<ThreadDetailPlan> {
    require_capability(platform, Capability::Threads)?;
    let thread_id = required_thread_id(&request.id)?;
    let full = request.full.unwrap_or(false);
    Ok(ThreadDetailPlan {
        required_capability: Capability::Threads,
        thread_id,
        block_limit: detail_block_limit(request.limit, Some(full)),
        full,
        before: request.before,
    })
}

pub fn plan_thread_blocks_request(
    platform: &PlatformPaths,
    id: &str,
    limit: Option<usize>,
    before: Option<String>,
) -> anyhow::Result<ThreadDetailPlan> {
    plan_thread_detail_request(
        platform,
        ThreadDetailRequest {
            id: id.to_string(),
            limit: Some(block_page_limit(limit)),
            full: Some(false),
            before,
        },
    )
}

pub fn plan_thread_cleanup_action(
    platform: &PlatformPaths,
    action: ThreadCleanupAction,
) -> anyhow::Result<ThreadCleanupPlan> {
    require_capability(platform, Capability::ThreadCleanup)?;
    let (target, execute) = match action {
        ThreadCleanupAction::ArchiveDeleteDryRun => (ThreadCleanupTarget::Archived, false),
        ThreadCleanupAction::ArchiveDeleteExecute => (ThreadCleanupTarget::Archived, true),
        ThreadCleanupAction::HiddenDeleteDryRun => (ThreadCleanupTarget::Hidden, false),
        ThreadCleanupAction::HiddenDeleteExecute => (ThreadCleanupTarget::Hidden, true),
    };
    Ok(ThreadCleanupPlan {
        required_capability: Capability::ThreadCleanup,
        action,
        target,
        execute,
    })
}

pub fn window_thread_detail_for_plan(
    detail: ThreadDetail,
    plan: &ThreadDetailPlan,
) -> ThreadDetail {
    crate::codex::window_thread_detail(detail, plan.block_limit, plan.before.as_deref())
}

pub fn apply_thread_list_runtime_state(
    threads: Vec<ThreadSummary>,
    runtime: ThreadListRuntimeState<'_>,
) -> Vec<ThreadSummary> {
    let mut threads = prune_hidden_thread_summaries(threads, runtime.hidden_thread_ids);
    merge_running_jobs(
        &mut threads,
        runtime.running_jobs,
        runtime.archived_thread_ids,
    );
    prune_hidden_thread_summaries(threads, runtime.hidden_thread_ids)
}

pub fn merge_running_jobs(
    threads: &mut Vec<ThreadSummary>,
    running_jobs: &[JobRecord],
    archived_thread_ids: &HashSet<String>,
) {
    let mut by_thread: HashMap<&str, &JobRecord> = HashMap::new();
    for job in running_jobs {
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

pub fn apply_running_job_to_summary(summary: &mut ThreadSummary, job: &JobRecord) {
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

pub fn thread_list_fetch_limit(status: Option<&str>, limit: Option<usize>) -> usize {
    if thread_status_filter_needs_full_scan(status) {
        usize::MAX
    } else {
        requested_thread_limit(limit)
    }
}

pub fn filter_thread_summaries(
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

pub fn thread_summaries_for_status(
    rows: Vec<ThreadSummary>,
    status: &str,
    limit: usize,
) -> Vec<ThreadSummary> {
    filter_thread_summaries(rows, Some(status), None, limit)
}

pub fn prune_hidden_thread_summaries(
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

pub fn normalize_thread_detail_block_limit(limit: Option<usize>, full: bool) -> Option<usize> {
    if full {
        None
    } else {
        Some(normalize_thread_block_limit(limit))
    }
}

pub fn normalize_thread_block_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(THREAD_DETAIL_DEFAULT_BLOCK_LIMIT)
        .clamp(1, THREAD_DETAIL_MAX_BLOCK_LIMIT)
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

fn requested_thread_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(80).clamp(1, 500)
}

fn required_thread_id(value: &str) -> anyhow::Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        anyhow::bail!("thread_id is required");
    }
    Ok(trimmed.to_string())
}

fn detail_block_limit(limit: Option<usize>, full: Option<bool>) -> Option<usize> {
    normalize_thread_detail_block_limit(limit, full.unwrap_or(false))
}

fn block_page_limit(limit: Option<usize>) -> usize {
    normalize_thread_block_limit(limit)
}

fn thread_status_filter_needs_full_scan(status: Option<&str>) -> bool {
    matches!(status, Some("running" | "reply-needed" | "recoverable"))
}

fn thread_matches_status(row: &ThreadSummary, status: &str) -> bool {
    matches!(
        (status, &row.status),
        ("running", ThreadStatus::Running)
            | ("reply-needed", ThreadStatus::ReplyNeeded)
            | ("recoverable", ThreadStatus::Recoverable)
            | ("archived", ThreadStatus::Archived)
            | ("recent", ThreadStatus::Recent)
    )
}

fn timestamp_to_rfc3339(timestamp: i64) -> Option<String> {
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|time| time.to_rfc3339())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::{
        codex::{ThreadStatus, ThreadSummary},
        db::JobRecord,
        services::threads::{
            build_threads_overview, merge_running_jobs, thread_list_fetch_limit,
            thread_summaries_for_status, ThreadsQuery,
        },
    };

    #[test]
    fn overview_filters_hidden_threads_and_merges_running_jobs() {
        let threads = vec![
            thread(
                "visible",
                ThreadStatus::Recent,
                Some("2026-06-18T10:00:00Z"),
            ),
            thread("hidden", ThreadStatus::Recent, Some("2026-06-18T11:00:00Z")),
            thread(
                "archived",
                ThreadStatus::Archived,
                Some("2026-06-18T12:00:00Z"),
            ),
        ];
        let jobs = vec![
            running_job("job-visible", "visible", Some("turn-visible"), 30),
            running_job("job-hidden", "hidden", Some("turn-hidden"), 40),
            running_job("job-new", "new-running", Some("turn-new"), 50),
            running_job("job-archived", "archived", Some("turn-archived"), 60),
        ];
        let hidden = HashSet::from(["hidden".to_string()]);
        let archived = HashSet::from(["archived".to_string()]);

        let overview = build_threads_overview(
            threads,
            jobs,
            ThreadsQuery {
                status: Some("running".to_string()),
                q: None,
                limit: Some(10),
            },
            &hidden,
            &archived,
        );

        assert_eq!(overview.query.status.as_deref(), Some("running"));
        assert_eq!(overview.fetch_limit, usize::MAX);
        assert_eq!(
            overview
                .threads
                .iter()
                .map(|thread| thread.id.as_str())
                .collect::<Vec<_>>(),
            vec!["visible", "new-running"]
        );
        let visible = overview
            .threads
            .iter()
            .find(|thread| thread.id == "visible")
            .expect("visible running thread");
        assert_eq!(visible.status, ThreadStatus::Running);
        assert_eq!(visible.active_job_id.as_deref(), Some("job-visible"));
        assert_eq!(visible.active_turn_id.as_deref(), Some("turn-visible"));
        assert!(!overview.threads.iter().any(|thread| thread.id == "hidden"));
        assert!(!overview
            .threads
            .iter()
            .any(|thread| thread.id == "archived"));
    }

    #[test]
    fn runtime_state_helper_applies_running_jobs_hidden_and_archived_rules_for_probe() {
        let rows = vec![
            thread(
                "visible",
                ThreadStatus::Recent,
                Some("2026-06-18T10:00:00Z"),
            ),
            thread("hidden", ThreadStatus::Recent, Some("2026-06-18T11:00:00Z")),
            thread(
                "archived",
                ThreadStatus::Archived,
                Some("2026-06-18T12:00:00Z"),
            ),
        ];
        let jobs = vec![
            running_job("job-visible", "visible", Some("turn-visible"), 30),
            running_job("job-hidden", "hidden", Some("turn-hidden"), 40),
            running_job("job-new", "new-running", Some("turn-new"), 50),
            running_job("job-archived", "archived", Some("turn-archived"), 60),
        ];
        let hidden = HashSet::from(["hidden".to_string()]);
        let archived = HashSet::from(["archived".to_string()]);

        let rows = super::apply_thread_list_runtime_state(
            rows,
            super::ThreadListRuntimeState {
                running_jobs: &jobs,
                hidden_thread_ids: &hidden,
                archived_thread_ids: &archived,
            },
        );
        let filtered = thread_summaries_for_status(rows, "running", 10);

        assert_eq!(
            filtered
                .iter()
                .map(|thread| thread.id.as_str())
                .collect::<Vec<_>>(),
            vec!["visible", "new-running"]
        );
        assert_eq!(filtered[0].active_job_id.as_deref(), Some("job-visible"));
    }

    #[test]
    fn merge_running_jobs_preserves_existing_active_turn_and_uses_job_title_fallback() {
        let mut rows = vec![ThreadSummary {
            active_turn_id: Some("turn-from-rollout".to_string()),
            latest_message: None,
            ..thread("thread-a", ThreadStatus::ReplyNeeded, None)
        }];
        let jobs = vec![running_job("job-a", "thread-a", Some("turn-from-job"), 10)];

        merge_running_jobs(&mut rows, &jobs, &HashSet::new());

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, ThreadStatus::Running);
        assert_eq!(rows[0].active_job_id.as_deref(), Some("job-a"));
        assert_eq!(rows[0].active_turn_id.as_deref(), Some("turn-from-rollout"));
        assert_eq!(rows[0].latest_message.as_deref(), Some("Job job-a"));
    }

    #[test]
    fn full_scan_statuses_fetch_all_threads() {
        assert_eq!(
            thread_list_fetch_limit(Some("running"), Some(25)),
            usize::MAX
        );
        assert_eq!(
            thread_list_fetch_limit(Some("reply-needed"), Some(25)),
            usize::MAX
        );
        assert_eq!(
            thread_list_fetch_limit(Some("recoverable"), Some(25)),
            usize::MAX
        );
        assert_eq!(thread_list_fetch_limit(None, Some(25)), 25);
        assert_eq!(thread_list_fetch_limit(Some("recent"), None), 80);
    }

    #[test]
    fn shared_status_filter_keeps_archived_out_of_probe_style_running_lists() {
        let rows = vec![
            thread("recent", ThreadStatus::Recent, Some("2026-06-18T10:00:00Z")),
            thread(
                "archived",
                ThreadStatus::Archived,
                Some("2026-06-18T12:00:00Z"),
            ),
            thread(
                "running",
                ThreadStatus::Running,
                Some("2026-06-18T11:00:00Z"),
            ),
        ];

        let filtered = thread_summaries_for_status(rows, "running", 10);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "running");
    }

    fn thread(id: &str, status: ThreadStatus, updated_at: Option<&str>) -> ThreadSummary {
        ThreadSummary {
            id: id.to_string(),
            title: format!("Thread {id}"),
            status,
            updated_at: updated_at.map(str::to_string),
            archived_at: None,
            message_count: 1,
            latest_message: Some(format!("latest {id}")),
            cwd: None,
            model: None,
            rollout_path: None,
            active_turn_id: None,
            active_job_id: None,
            pending_elicitation: None,
            last_event_kind: None,
        }
    }

    fn running_job(id: &str, thread_id: &str, turn_id: Option<&str>, started_at: i64) -> JobRecord {
        JobRecord {
            id: id.to_string(),
            kind: "codex_chat".to_string(),
            status: "running".to_string(),
            title: format!("Job {id}"),
            thread_id: Some(thread_id.to_string()),
            turn_id: turn_id.map(str::to_string),
            started_at,
            finished_at: None,
            exit_code: None,
            output: String::new(),
            error: None,
        }
    }
}
