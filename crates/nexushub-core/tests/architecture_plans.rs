use std::{collections::HashSet, path::PathBuf};

use nexushub_core::{
    codex::{ThreadDetail, ThreadStatus, ThreadSummary},
    config::Config,
    db::{JobRecord, ThreadFollowUp},
    platform::{PlatformKind, PlatformPaths},
    services::{
        probe::{
            probe_logs_db_status_view, probe_status_snapshot_view, ProbeLogsDbLastMaintain,
            ProbeSnapshotStatus,
        },
        threads::{thread_detail_read_model, thread_list_read_model, ThreadReadModelInputs},
    },
};
use serde_json::json;

#[test]
fn core_thread_read_model_merges_running_jobs_and_plans_autosubmit_effects() {
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let pending = followup("followup-a", "idle-thread", "continue");
    let known_running_job = running_job("job-running", "known-thread", Some("turn-running"), 20);
    let new_running_job = running_job("job-new", "new-running-thread", None, 30);

    let view = thread_list_read_model(
        &platform,
        ThreadReadModelInputs {
            threads: vec![
                thread(
                    "known-thread",
                    ThreadStatus::Recent,
                    Some("2026-06-24T10:00:00Z"),
                ),
                thread(
                    "idle-thread",
                    ThreadStatus::Recent,
                    Some("2026-06-24T11:00:00Z"),
                ),
                thread(
                    "hidden-thread",
                    ThreadStatus::Recent,
                    Some("2026-06-24T12:00:00Z"),
                ),
            ],
            running_jobs: vec![known_running_job, new_running_job],
            hidden_thread_ids: HashSet::from(["hidden-thread".to_string()]),
            archived_thread_ids: HashSet::new(),
            pending_followups: vec![pending.clone()],
            default_workspace: PathBuf::from("/workspace"),
        },
        nexushub_core::services::threads::ThreadsQuery {
            status: Some("running".to_string()),
            q: None,
            limit: Some(20),
        },
    )
    .expect("thread read model should be planned in core");

    let ids = view
        .threads
        .iter()
        .map(|thread| thread.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&"new-running-thread"));
    assert!(ids.contains(&"known-thread"));
    assert!(view.autosubmit_effects.is_empty());

    let idle_view = thread_list_read_model(
        &platform,
        ThreadReadModelInputs {
            threads: vec![thread(
                "idle-thread",
                ThreadStatus::Recent,
                Some("2026-06-24T11:00:00Z"),
            )],
            running_jobs: vec![],
            hidden_thread_ids: HashSet::new(),
            archived_thread_ids: HashSet::new(),
            pending_followups: vec![pending],
            default_workspace: PathBuf::from("/workspace"),
        },
        nexushub_core::services::threads::ThreadsQuery {
            status: Some("recent".to_string()),
            q: None,
            limit: Some(20),
        },
    )
    .expect("idle follow-up should produce core autosubmit effect plan");

    assert_eq!(idle_view.autosubmit_effects.len(), 1);
    assert_eq!(
        idle_view.autosubmit_effects[0]
            .claim
            .as_ref()
            .expect("claim plan")
            .to_status,
        "submitting"
    );
    assert_eq!(
        idle_view.autosubmit_effects[0]
            .job
            .as_ref()
            .expect("resume job plan")
            .spec
            .thread_id
            .as_deref(),
        Some("idle-thread")
    );
}

#[test]
fn core_thread_detail_read_model_returns_updated_detail_and_autosubmit_effect() {
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let detail = ThreadDetail {
        summary: thread(
            "idle-thread",
            ThreadStatus::Recent,
            Some("2026-06-24T11:00:00Z"),
        ),
        messages: vec![],
        blocks: vec![],
        raw_event_count: 0,
        total_blocks: 0,
        has_more_blocks: false,
        before_cursor: None,
    };

    let view = thread_detail_read_model(
        &platform,
        detail,
        None,
        Some(followup("followup-a", "idle-thread", "continue")),
        PathBuf::from("/workspace"),
    )
    .expect("core should own detail read-model/autosubmit planning");

    assert_eq!(view.detail.summary.status, ThreadStatus::Recent);
    assert_eq!(view.autosubmit_effects.len(), 1);
    assert_eq!(
        view.autosubmit_effects[0]
            .followup_id
            .as_deref()
            .expect("followup id"),
        "followup-a"
    );
}

#[test]
fn core_probe_views_own_logs_db_status_and_snapshot_metadata() {
    let mut config = Config::for_platform_kind(PlatformKind::Linux);
    config.probe.logs_db.maintenance_interval_hours = 6;
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let runtime = nexushub_core::probe::ProbeRuntime::new(config.clone(), platform);
    let status = runtime.logs_db_status();

    let view = probe_logs_db_status_view(
        status,
        &config,
        Some(ProbeLogsDbLastMaintain {
            raw: json!({"ok": true, "mode": "dry-run"}).to_string(),
            updated_at_unix: 1_700_000_000,
        }),
    )
    .expect("core should build logs-db status API view");

    assert_eq!(view["last_result"], "ok");
    assert_eq!(view["recent_result"], "ok");
    assert_eq!(
        view["last_maintain"],
        json!({"ok": true, "mode": "dry-run"})
    );
    assert_eq!(view["next_run"], "2023-11-15T04:13:20+00:00");

    let snapshot = probe_status_snapshot_view(
        json!({"label": "Probe"}),
        42,
        true,
        ProbeSnapshotStatus::Cached,
    );
    assert_eq!(snapshot["snapshot_age_seconds"], 42);
    assert_eq!(snapshot["is_refreshing"], true);
    assert_eq!(snapshot["snapshot_status"], "cached");
}

fn thread(id: &str, status: ThreadStatus, updated_at: Option<&str>) -> ThreadSummary {
    ThreadSummary {
        id: id.to_string(),
        title: format!("Thread {id}"),
        status,
        updated_at: updated_at.map(str::to_string),
        archived_at: None,
        message_count: 1,
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

fn running_job(id: &str, thread_id: &str, turn_id: Option<&str>, started_at: i64) -> JobRecord {
    JobRecord {
        id: id.to_string(),
        kind: "codex".to_string(),
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

fn followup(id: &str, thread_id: &str, message: &str) -> ThreadFollowUp {
    ThreadFollowUp {
        id: id.to_string(),
        thread_id: thread_id.to_string(),
        status: "pending".to_string(),
        message: message.to_string(),
        options_json: json!({}).to_string(),
        created_at: 1,
        updated_at: 1,
        submitted_at: None,
        cancelled_at: None,
        result_json: None,
        error: None,
    }
}
