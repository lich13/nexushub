use crate::{
    codex::{self, CodexPaths, ThreadStatus, ThreadSummary},
    config::Config,
    db::JobRecord,
    platform::PlatformPaths,
    probe as probe_core,
    services::commands,
    services::system::{require_capability, Capability},
    services::threads::{self, ThreadListRuntimeState},
};
use anyhow::{anyhow, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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

#[derive(Debug, Clone, Copy)]
pub struct ProbeUseCases<'a> {
    config: &'a Config,
    platform: &'a PlatformPaths,
}

impl<'a> ProbeUseCases<'a> {
    pub fn new(config: &'a Config, platform: &'a PlatformPaths) -> Self {
        Self { config, platform }
    }

    pub fn status(self) -> Result<ProbeStatusFacadePlan> {
        probe_status_with_capability(self.config, self.platform)
    }

    pub fn action(self, action: ProbeAction) -> Result<ProbeActionPlan> {
        plan_probe_action(self.config, self.platform, action)
    }

    pub fn action_with_device_key(
        self,
        action: ProbeAction,
        device_key_configured: bool,
    ) -> Result<ProbeActionPlan> {
        plan_probe_action_with_device_key(self.config, self.platform, action, device_key_configured)
    }

    pub fn action_with_device_key_and_config_path(
        self,
        action: ProbeAction,
        device_key_configured: bool,
        config_path: impl AsRef<Path>,
    ) -> Result<ProbeActionPlan> {
        plan_probe_action_with_device_key_and_config_path(
            self.config,
            self.platform,
            action,
            device_key_configured,
            config_path,
        )
    }

    pub fn logs_db_maintenance_plan(self, dry_run: bool) -> Result<ProbeActionPlan> {
        self.action(if dry_run {
            ProbeAction::LogsDbDryRun
        } else {
            ProbeAction::LogsDbExecute
        })
    }

    pub fn logs_db_maintenance(
        self,
        request: ProbeLogsDbMaintenanceRequest,
    ) -> Result<ProbeActionPlan> {
        plan_probe_logs_db_maintenance(self.config, self.platform, request)
    }

    pub fn logs_db_maintenance_with_config_path(
        self,
        request: ProbeLogsDbMaintenanceRequest,
        config_path: impl AsRef<Path>,
    ) -> Result<ProbeActionPlan> {
        plan_probe_logs_db_maintenance_with_config_path(
            self.config,
            self.platform,
            request,
            config_path,
        )
    }
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeLogsDbMaintenanceRequest {
    pub dry_run: bool,
    pub compact: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeBarkDeliveryDecision {
    pub notifications_enabled: bool,
    pub relevant_switch_enabled: bool,
    pub device_key_configured: bool,
    pub should_send: bool,
    pub skip_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeLogsDbSchedulerPlan {
    pub should_run: bool,
    pub compact: bool,
    pub skip_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProbeLogsDbLastMaintain {
    pub raw: String,
    pub updated_at_unix: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeSnapshotStatus {
    Fresh,
    Cached,
    Initial,
}

impl ProbeSnapshotStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Cached => "cached",
            Self::Initial => "initial",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeThreadNotificationPlan {
    pub body: Option<String>,
    pub body_source: Option<String>,
    pub reason_label: Option<String>,
    pub fresh: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeEventRecordPlan {
    pub event: probe_core::ProbeBuiltEvent,
    pub passive_marker_key: Option<String>,
    pub duplicate_outcome: probe_core::ProbeEventOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeEventRecordWritePlan {
    pub outcome: probe_core::ProbeEventOutcome,
    pub record: Option<ProbeEventRecordWrite>,
    pub passive_marker: Option<ProbePassiveMarkerWrite>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeEventRecordWrite {
    pub kind: String,
    pub thread_id: Option<String>,
    pub title: String,
    pub message: String,
    pub dedupe_key: String,
    pub source: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbePassiveMarkerWrite {
    pub key: String,
    pub value: Value,
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

pub fn probe_logs_db_status_view(
    status: impl Serialize,
    config: &Config,
    last_maintain: Option<ProbeLogsDbLastMaintain>,
) -> Result<Value> {
    let mut value = serde_json::to_value(status)?;
    if let (Some(object), Some(last)) = (value.as_object_mut(), last_maintain) {
        let last_run = timestamp_to_rfc3339_utc(last.updated_at_unix)
            .unwrap_or_else(|| last.updated_at_unix.to_string());
        let next_run_ts = last.updated_at_unix
            + i64::from(config.probe.logs_db.maintenance_interval_hours.max(1)) * 3_600;
        let next_run =
            timestamp_to_rfc3339_utc(next_run_ts).unwrap_or_else(|| next_run_ts.to_string());
        let last_result = probe_logs_db_last_result(&last.raw);
        let last_run_value =
            serde_json::from_str::<Value>(&last.raw).unwrap_or(Value::String(last.raw));
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
    Ok(value)
}

pub fn probe_status_snapshot_view(
    mut value: Value,
    snapshot_age_seconds: i64,
    is_refreshing: bool,
    snapshot_status: ProbeSnapshotStatus,
) -> Value {
    if let Value::Object(ref mut object) = value {
        object.insert(
            "snapshot_age_seconds".to_string(),
            json!(snapshot_age_seconds),
        );
        object.insert("is_refreshing".to_string(), json!(is_refreshing));
        object.insert(
            "snapshot_status".to_string(),
            json!(snapshot_status.as_str()),
        );
    }
    value
}

pub fn probe_status_with_runtime_read_model(
    mut status: probe_core::ProbeStatus,
    read_model: ProbeStatusAggregation,
    recent_event_count_override: Option<usize>,
) -> probe_core::ProbeStatus {
    status.recent_event_count =
        recent_event_count_override.unwrap_or(read_model.recent_event_count);
    status.running_count = read_model.running_threads.len();
    status.reply_needed_count = read_model.reply_needed_threads.len();
    status.recoverable_count = read_model.recoverable_threads.len();
    status.running_threads = read_model.running_threads;
    status.reply_needed_threads = read_model.reply_needed_threads;
    status.recoverable_threads = read_model.recoverable_threads;
    status
}

pub fn plan_probe_action(
    config: &Config,
    platform: &PlatformPaths,
    action: ProbeAction,
) -> Result<ProbeActionPlan> {
    plan_probe_action_with_device_key(config, platform, action, false)
}

pub fn plan_probe_action_with_config_path(
    config: &Config,
    platform: &PlatformPaths,
    action: ProbeAction,
    config_path: impl AsRef<Path>,
) -> Result<ProbeActionPlan> {
    plan_probe_action_with_device_key_and_config_path(config, platform, action, false, config_path)
}

pub fn plan_probe_action_with_device_key(
    config: &Config,
    platform: &PlatformPaths,
    action: ProbeAction,
    device_key_configured: bool,
) -> Result<ProbeActionPlan> {
    plan_probe_action_with_device_key_and_config_path(
        config,
        platform,
        action,
        device_key_configured,
        &platform.config_file,
    )
}

pub fn plan_probe_action_with_device_key_and_config_path(
    config: &Config,
    platform: &PlatformPaths,
    action: ProbeAction,
    device_key_configured: bool,
    config_path: impl AsRef<Path>,
) -> Result<ProbeActionPlan> {
    let required_capability = probe_action_capability(action);
    require_capability(platform, required_capability)?;
    let config_path = config_path.as_ref();
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
    let job = Some(probe_fixed_job_spec(platform, action, config_path)?);
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

pub fn plan_probe_logs_db_maintenance(
    config: &Config,
    platform: &PlatformPaths,
    request: ProbeLogsDbMaintenanceRequest,
) -> Result<ProbeActionPlan> {
    plan_probe_logs_db_maintenance_with_config_path(
        config,
        platform,
        request,
        &platform.config_file,
    )
}

pub fn plan_probe_logs_db_maintenance_with_config_path(
    config: &Config,
    platform: &PlatformPaths,
    request: ProbeLogsDbMaintenanceRequest,
    config_path: impl AsRef<Path>,
) -> Result<ProbeActionPlan> {
    let action = if request.dry_run {
        ProbeAction::LogsDbDryRun
    } else {
        ProbeAction::LogsDbExecute
    };
    let mut plan = plan_probe_action_with_config_path(config, platform, action, config_path)?;
    let compact = request.compact && !request.dry_run;
    if let Some(maintenance) = plan.maintenance.as_mut() {
        maintenance.compact = compact;
    }
    if compact {
        if let Some(job) = plan.job.as_mut() {
            if !job.args.iter().any(|arg| arg == "--compact") {
                job.args.push("--compact".to_string());
            }
            if !job.command.contains("--compact") {
                job.command = format!("{} {}", job.command, shell_quote("--compact"));
            }
        }
    }
    Ok(plan)
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

pub fn probe_passive_thread_notification_plan(
    thread: &ThreadSummary,
    status: &str,
) -> ProbeThreadNotificationPlan {
    let (body, body_source) = probe_thread_notification_body(thread, status);
    let fresh = probe_thread_passive_bark_fresh(thread, status, body_source.as_deref());
    ProbeThreadNotificationPlan {
        body,
        body_source,
        reason_label: probe_passive_reason_label(status).map(str::to_string),
        fresh,
    }
}

fn probe_passive_reason_label(status: &str) -> Option<&'static str> {
    match status {
        "reply-needed" => Some("等待用户确认"),
        "recoverable" => Some("异常/可恢复"),
        _ => None,
    }
}

pub fn probe_event_bark_switch_enabled(config: &Config, kind: &str) -> bool {
    match kind {
        "completion" => config.probe.notifications.notify_completion,
        "reply-needed" => config.probe.notifications.notify_reply_needed,
        "recoverable" => config.probe.notifications.notify_recoverable,
        _ => config.probe.notifications.notify_completion,
    }
}

pub fn probe_bark_delivery_decision(
    config: &Config,
    kind: &str,
    device_key_configured: bool,
    dedupe_claimed: bool,
) -> ProbeBarkDeliveryDecision {
    let relevant_switch_enabled = probe_event_bark_switch_enabled(config, kind);
    let notifications_enabled = config.probe.notifications.enabled;
    let skip_reason = if !notifications_enabled {
        Some("notifications_disabled")
    } else if !relevant_switch_enabled {
        Some("event_switch_disabled")
    } else if !device_key_configured {
        Some("device_key_missing")
    } else if !dedupe_claimed {
        Some("dedupe")
    } else {
        None
    };
    ProbeBarkDeliveryDecision {
        notifications_enabled,
        relevant_switch_enabled,
        device_key_configured,
        should_send: skip_reason.is_none(),
        skip_reason: skip_reason.map(str::to_string),
    }
}

pub fn probe_bark_status_label(sent: bool, skipped: bool, reason: Option<&str>) -> &'static str {
    if sent {
        "sent"
    } else if skipped && reason == Some("dedupe") {
        "dedupe_hit"
    } else if skipped {
        "skipped"
    } else {
        "failed"
    }
}

pub fn probe_logs_db_scheduler_plan(
    config: &Config,
    last_maintain_updated_at: Option<i64>,
    last_compact_updated_at: Option<i64>,
    now_unix: i64,
) -> ProbeLogsDbSchedulerPlan {
    if !config.probe.logs_db.enabled {
        return ProbeLogsDbSchedulerPlan {
            should_run: false,
            compact: false,
            skip_reason: Some("logs_db_disabled".to_string()),
        };
    }
    let interval_seconds =
        i64::from(config.probe.logs_db.maintenance_interval_hours.max(1)) * 3_600;
    if let Some(updated_at) = last_maintain_updated_at {
        if now_unix.saturating_sub(updated_at) < interval_seconds {
            return ProbeLogsDbSchedulerPlan {
                should_run: false,
                compact: false,
                skip_reason: Some("not_due".to_string()),
            };
        }
    }
    ProbeLogsDbSchedulerPlan {
        should_run: true,
        compact: probe_logs_db_compaction_due(config, last_compact_updated_at, now_unix),
        skip_reason: None,
    }
}

pub fn probe_logs_db_compaction_due(
    config: &Config,
    last_compact_updated_at: Option<i64>,
    now_unix: i64,
) -> bool {
    if !config.probe.logs_db.auto_compact_when_codex_closed {
        return false;
    }
    let interval_seconds = i64::from(config.probe.logs_db.compact_interval_hours.max(1)) * 3_600;
    last_compact_updated_at
        .map(|updated_at| now_unix.saturating_sub(updated_at) >= interval_seconds)
        .unwrap_or(true)
}

pub fn probe_logs_db_stored_result(
    result: &impl Serialize,
    probe_events_deleted: usize,
    probe_dedupe_deleted: usize,
    dry_run: bool,
) -> Result<Value> {
    let mut stored = serde_json::to_value(result)?;
    if let Value::Object(object) = &mut stored {
        object.insert(
            "probe_events_target".to_string(),
            Value::String("panel_probe_events".to_string()),
        );
        object.insert("probe_events_dry_run".to_string(), Value::Bool(dry_run));
        object.insert(
            "probe_events_deleted".to_string(),
            json!(probe_events_deleted),
        );
        object.insert(
            "probe_dedupe_deleted".to_string(),
            json!(probe_dedupe_deleted),
        );
    }
    Ok(stored)
}

pub fn probe_event_record_plan(mut event: probe_core::ProbeBuiltEvent) -> ProbeEventRecordPlan {
    normalize_probe_event_dedupe_key(&mut event);
    let passive_marker_key = probe_passive_unresolved_action_marker_key(&event);
    let mut duplicate_outcome = probe_core::ProbeEventOutcome::from_claim(&event, false);
    duplicate_outcome.recorded = false;
    duplicate_outcome.duplicate = true;
    ProbeEventRecordPlan {
        event,
        passive_marker_key,
        duplicate_outcome,
    }
}

pub fn probe_event_record_write_plan(
    event: &probe_core::ProbeBuiltEvent,
    claimed: bool,
    bark: &impl Serialize,
    bark_status: &str,
) -> Result<ProbeEventRecordWritePlan> {
    let mut outcome = probe_core::ProbeEventOutcome::from_claim(event, claimed);
    if !claimed {
        outcome.recorded = false;
        outcome.duplicate = true;
        return Ok(ProbeEventRecordWritePlan {
            outcome,
            record: None,
            passive_marker: None,
        });
    }

    let mut payload = event.payload.clone();
    merge_probe_bark_payload(&mut payload, bark)?;
    payload["dedupe"] = json!({
        "namespace": &event.dedupe_namespace,
        "key": &event.dedupe_key,
        "claimed": claimed,
        "duplicate": !claimed,
        "status": "claimed",
    });
    payload["bark_status"] = json!(bark_status);
    payload["dedupe_status"] = json!("claimed");

    let passive_marker =
        probe_passive_unresolved_action_marker_key(event).map(|key| ProbePassiveMarkerWrite {
            key,
            value: json!({
                "dedupe_key": event.dedupe_key,
                "thread_id": event.thread_id,
                "turn_id": event.turn_id,
                "body_source": event.payload.get("body_source").and_then(Value::as_str),
                "body_sha256": event.payload.get("body_sha256").and_then(Value::as_str),
            }),
        });

    Ok(ProbeEventRecordWritePlan {
        outcome,
        record: Some(ProbeEventRecordWrite {
            kind: event.kind.clone(),
            thread_id: event.thread_id.clone(),
            title: event.title.clone(),
            message: event.message.clone(),
            dedupe_key: event.dedupe_key.clone(),
            source: event.source.clone(),
            payload,
        }),
        passive_marker,
    })
}

pub fn normalize_probe_event_dedupe_key(event: &mut probe_core::ProbeBuiltEvent) {
    if event.kind != "reply-needed" {
        return;
    }
    match event.payload.get("body_source").and_then(Value::as_str) {
        Some("proposed_plan") => normalize_proposed_plan_dedupe_key(event),
        Some("request_user_input") => normalize_request_user_input_dedupe_key(event),
        _ => {}
    }
}

pub fn probe_passive_unresolved_action_marker_key(
    event: &probe_core::ProbeBuiltEvent,
) -> Option<String> {
    if event.kind != "reply-needed" {
        return None;
    }
    if event.payload.get("scan_source").and_then(Value::as_str) != Some("passive-scan") {
        return None;
    }
    let body_source = event.payload.get("body_source").and_then(Value::as_str)?;
    if !matches!(body_source, "proposed_plan" | "request_user_input") {
        return None;
    }
    let thread_id = event
        .thread_id
        .as_deref()
        .or_else(|| event.payload.get("thread_id").and_then(Value::as_str))
        .unwrap_or("unknown");
    let turn_id = event
        .turn_id
        .as_deref()
        .or_else(|| event.payload.get("turn_id").and_then(Value::as_str))
        .unwrap_or("unknown");
    let action_id = event
        .payload
        .get("item_id")
        .and_then(Value::as_str)
        .or_else(|| event.payload.get("call_id").and_then(Value::as_str))
        .unwrap_or(turn_id);
    let content_key = if body_source == "proposed_plan" {
        proposed_plan_hash_from_text(&event.bark_body)
            .or_else(|| {
                event
                    .payload
                    .get("body_sha256")
                    .and_then(Value::as_str)
                    .and_then(|value| value.get(..16))
                    .map(ToString::to_string)
            })
            .unwrap_or_else(|| "unknown".to_string())
    } else {
        request_user_input_hash_from_text(&event.bark_body).unwrap_or_else(|| {
            event
                .payload
                .get("body_sha256")
                .and_then(Value::as_str)
                .and_then(|value| value.get(..16))
                .unwrap_or("unknown")
                .to_string()
        })
    };
    Some(format!(
        "probe_passive_sent_marker:{}:{}:{}:{}:{}:{}",
        dedupe_component(body_source),
        dedupe_component(thread_id),
        dedupe_component(turn_id),
        dedupe_component(action_id),
        dedupe_component(&content_key),
        dedupe_component(&event.kind),
    ))
}

impl ProbeAction {
    pub fn as_rpc_action(self) -> &'static str {
        match self {
            Self::BarkTest => commands::PROBE_BARK_TEST,
            Self::InstallHooks => commands::PROBE_INSTALL_HOOKS,
            Self::LogsDbDryRun => commands::PROBE_LOGS_DB_DRY_RUN,
            Self::LogsDbExecute => commands::PROBE_LOGS_DB_EXECUTE,
        }
    }

    pub fn as_desktop_command(self) -> &'static str {
        self.as_rpc_action()
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

fn merge_probe_bark_payload(payload: &mut Value, bark: &impl Serialize) -> Result<()> {
    let outcome = serde_json::to_value(bark)?;
    if let Some(existing) = payload.get_mut("bark").and_then(Value::as_object_mut) {
        if let Some(outcome_object) = outcome.as_object() {
            for (key, value) in outcome_object {
                existing.insert(key.clone(), value.clone());
            }
        }
    } else {
        payload["bark"] = outcome;
    }
    Ok(())
}

fn probe_thread_notification_body(
    thread: &ThreadSummary,
    status: &str,
) -> (Option<String>, Option<String>) {
    if status == "reply-needed" {
        if let Some(elicitation) = &thread.pending_elicitation {
            if !thread_rollout_still_request_user_input_needed(thread) {
                return (None, None);
            }
            return (
                Some(format_pending_elicitation(elicitation)),
                Some("request_user_input".to_string()),
            );
        }
        if let Some(message) = thread
            .latest_message
            .as_deref()
            .filter(|value| value.contains("<proposed_plan>") && value.contains("</proposed_plan>"))
        {
            if !thread_rollout_still_reply_needed(thread) {
                return (None, None);
            }
            return (
                Some(format_proposed_plan_reply_needed(message)),
                Some("proposed_plan".to_string()),
            );
        }
        if let Some(path) = thread.rollout_path.as_deref() {
            if let Ok(Some(message)) =
                codex::rollout_completion_last_agent_message(path, thread.active_turn_id.as_deref())
            {
                if message.contains("<proposed_plan>") && message.contains("</proposed_plan>") {
                    if !thread_rollout_still_reply_needed(thread) {
                        return (None, None);
                    }
                    return (
                        Some(format_proposed_plan_reply_needed(&message)),
                        Some("proposed_plan".to_string()),
                    );
                }
            }
        }
    }
    if status == "recoverable" {
        if let Some(message) = thread.latest_message.as_deref() {
            return (
                Some(message.to_string()),
                Some("latest_exception".to_string()),
            );
        }
    }
    (
        thread.latest_message.clone(),
        thread
            .latest_message
            .as_ref()
            .map(|_| "latest_message".to_string()),
    )
}

fn probe_thread_passive_bark_fresh(
    thread: &ThreadSummary,
    status: &str,
    body_source: Option<&str>,
) -> bool {
    if status != "reply-needed" {
        return true;
    }
    if !thread_updated_within(thread, PROBE_REPLY_NEEDED_FRESH_WINDOW_SECONDS) {
        return false;
    }
    match body_source {
        Some("request_user_input") => thread_rollout_still_request_user_input_needed(thread),
        Some("proposed_plan") => thread_rollout_still_reply_needed(thread),
        Some(_) | None => false,
    }
}

fn thread_rollout_still_reply_needed(thread: &ThreadSummary) -> bool {
    if thread.rollout_path.is_none() {
        return true;
    }
    let mut refreshed = thread.clone();
    if codex::enrich_thread_from_rollout(&mut refreshed).is_err() {
        return true;
    }
    let reply_needed = matches!(refreshed.status, ThreadStatus::ReplyNeeded);
    let has_plan_message = refreshed.latest_message.as_deref().is_some_and(|value| {
        value.contains("<proposed_plan>") && value.contains("</proposed_plan>")
    });
    let same_turn = refreshed
        .active_turn_id
        .as_deref()
        .is_some_and(|turn_id| Some(turn_id) == thread.active_turn_id.as_deref())
        || (thread.active_turn_id.is_none() && refreshed.active_turn_id.is_none());
    reply_needed && has_plan_message && same_turn
}

fn thread_rollout_still_request_user_input_needed(thread: &ThreadSummary) -> bool {
    if thread.rollout_path.is_none() {
        return thread.pending_elicitation.is_some();
    }
    let mut refreshed = thread.clone();
    if codex::enrich_thread_from_rollout(&mut refreshed).is_err() {
        return false;
    }
    let reply_needed = matches!(refreshed.status, ThreadStatus::ReplyNeeded);
    let has_pending_elicitation = refreshed.pending_elicitation.is_some();
    let same_turn = refreshed
        .active_turn_id
        .as_deref()
        .is_some_and(|turn_id| Some(turn_id) == thread.active_turn_id.as_deref())
        || (thread.active_turn_id.is_none() && refreshed.active_turn_id.is_none());
    reply_needed && has_pending_elicitation && same_turn
}

fn format_pending_elicitation(elicitation: &codex::PendingElicitation) -> String {
    let mut lines = Vec::new();
    for (index, question) in elicitation.questions.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        let number = index + 1;
        lines.push(format!("问题 {number}：{}", question.question.trim()));
        if let Some(header) = question
            .header
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            lines.push(format!("标题：{header}"));
        }
        for (option_index, option) in question.options.iter().enumerate() {
            let marker = option_index + 1;
            lines.push(format!("选项 {marker}：{}", option.label.trim()));
            if let Some(description) = option
                .description
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                lines.push(format!("说明：{description}"));
            }
        }
    }
    if lines.is_empty() {
        "Codex 请求用户输入。".to_string()
    } else {
        lines.join("\n")
    }
}

fn format_proposed_plan_reply_needed(raw: &str) -> String {
    let plan_text =
        codex::extract_proposed_plan_text(raw).unwrap_or_else(|| raw.trim().to_string());
    plan_text.trim().to_string()
}

fn normalize_proposed_plan_dedupe_key(event: &mut probe_core::ProbeBuiltEvent) {
    let thread_id = event
        .thread_id
        .as_deref()
        .or_else(|| event.payload.get("thread_id").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string();
    let turn_id = event
        .turn_id
        .as_deref()
        .or_else(|| event.payload.get("turn_id").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string();
    let item_or_call_id = event
        .payload
        .get("item_id")
        .and_then(Value::as_str)
        .or_else(|| event.payload.get("call_id").and_then(Value::as_str))
        .unwrap_or(turn_id.as_str())
        .to_string();
    let plan_hash = proposed_plan_hash_from_text(&event.bark_body).unwrap_or_else(|| {
        event
            .payload
            .get("body_sha256")
            .and_then(Value::as_str)
            .and_then(|value| value.get(..16))
            .unwrap_or("unknown")
            .to_string()
    });
    event.dedupe_key = format!(
        "{}:{}:{}:{}:{}",
        dedupe_component(&event.kind),
        dedupe_component(&thread_id),
        dedupe_component(&turn_id),
        dedupe_component(&item_or_call_id),
        dedupe_component(&format!("plan_hash:{plan_hash}")),
    );
    event.payload["dedupe_plan_hash"] = json!(plan_hash);
    event.payload["dedupe_item_or_call_id"] = json!(item_or_call_id);
}

fn normalize_request_user_input_dedupe_key(event: &mut probe_core::ProbeBuiltEvent) {
    let thread_id = event
        .thread_id
        .as_deref()
        .or_else(|| event.payload.get("thread_id").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string();
    let turn_id = event
        .turn_id
        .as_deref()
        .or_else(|| event.payload.get("turn_id").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string();
    let call_id = event
        .payload
        .get("call_id")
        .and_then(Value::as_str)
        .or_else(|| event.payload.get("item_id").and_then(Value::as_str))
        .unwrap_or(turn_id.as_str())
        .to_string();
    let input_hash = request_user_input_hash_from_text(&event.bark_body).unwrap_or_else(|| {
        event
            .payload
            .get("body_sha256")
            .and_then(Value::as_str)
            .and_then(|value| value.get(..16))
            .unwrap_or("unknown")
            .to_string()
    });
    event.dedupe_key = format!(
        "{}:{}:{}:{}:{}",
        dedupe_component(&event.kind),
        dedupe_component(&thread_id),
        dedupe_component(&turn_id),
        dedupe_component(&call_id),
        dedupe_component(&format!("input_hash:{input_hash}")),
    );
    event.payload["dedupe_input_hash"] = json!(input_hash);
    event.payload["dedupe_item_or_call_id"] = json!(call_id);
}

fn proposed_plan_hash_from_text(value: &str) -> Option<String> {
    let plan = codex::extract_proposed_plan_text(value)
        .or_else(|| {
            value
                .split_once("Plan 摘要:")
                .map(|(_, plan)| plan.trim().to_string())
                .filter(|plan| !plan.is_empty())
        })
        .or_else(|| {
            value
                .split_once("待回复内容：")
                .map(|(_, plan)| plan.trim().to_string())
                .filter(|plan| !plan.is_empty())
        })?;
    Some(stable_hash64_hex(plan.trim()))
}

fn request_user_input_hash_from_text(value: &str) -> Option<String> {
    let body = value
        .split_once("待回复内容：")
        .map(|(_, body)| body)
        .unwrap_or(value);
    let normalized = body
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.starts_with("时间：")
                && !line.starts_with("时间:")
                && !line.starts_with("事件时间：")
                && !line.starts_with("事件时间:")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let normalized = normalized.trim();
    (!normalized.is_empty()).then(|| stable_hash64_hex(normalized))
}

fn stable_hash64_hex(value: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn dedupe_component(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if normalized.is_empty() {
        "unknown".to_string()
    } else {
        normalized
    }
}

fn probe_fixed_job_spec(
    platform: &PlatformPaths,
    action: ProbeAction,
    config_path: &Path,
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
        command: fixed_probe_shell_command(platform, config_path, &args),
        args,
        exclusive_group: Some(exclusive_group.to_string()),
    })
}

fn fixed_probe_shell_command(
    platform: &PlatformPaths,
    config_path: &Path,
    args: &[String],
) -> String {
    let mut parts = vec![
        platform.daemon_binary().display().to_string(),
        "--config".to_string(),
        config_path.display().to_string(),
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

fn timestamp_to_rfc3339_utc(timestamp: i64) -> Option<String> {
    chrono::DateTime::<Utc>::from_timestamp(timestamp, 0).map(|time| time.to_rfc3339())
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
    if value.get("ok").and_then(Value::as_bool) == Some(true) {
        return "ok".to_string();
    }
    format!("{dry_run}: events={events}, dedupe={dedupe}")
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
    use super::{
        plan_probe_action, plan_probe_logs_db_maintenance, probe_bark_delivery_decision,
        probe_bark_status_label, probe_logs_db_scheduler_plan, probe_logs_db_stored_result,
        probe_passive_thread_notification_plan, probe_status_with_capability, ProbeAction,
        ProbeExecutionKind, ProbeLogsDbMaintenanceRequest,
    };
    use crate::{
        codex::{ThreadStatus, ThreadSummary},
        config::Config,
        platform::{PlatformKind, PlatformPaths},
        services::system::Capability,
    };
    use serde_json::json;

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

    #[test]
    fn probe_action_plans_are_platform_scoped_and_include_execution_contracts() {
        let linux_config = Config::for_platform_kind(PlatformKind::Linux);
        let linux = PlatformPaths::for_kind(PlatformKind::Linux);
        let bark = plan_probe_action(&linux_config, &linux, ProbeAction::BarkTest)
            .expect("Linux should allow fixed Probe jobs");

        assert_eq!(bark.required_capability, Capability::Probe);
        assert_eq!(bark.execution, ProbeExecutionKind::FixedShellJob);
        assert_eq!(bark.job.as_ref().unwrap().kind, "probe_bark_test");
        assert!(bark.maintenance.is_none());

        let mac_config = Config::for_platform_kind(PlatformKind::Macos);
        let macos = PlatformPaths::for_kind(PlatformKind::Macos);
        let dry_run = plan_probe_action(&mac_config, &macos, ProbeAction::LogsDbDryRun)
            .expect("macOS should allow local Probe logs DB dry-runs");

        assert_eq!(dry_run.required_capability, Capability::ProbeLogMaintenance);
        assert_eq!(dry_run.execution, ProbeExecutionKind::LogsDbMaintenance);
        assert!(dry_run.maintenance.unwrap().dry_run);
        assert!(dry_run
            .job
            .unwrap()
            .args
            .iter()
            .any(|arg| arg == "--dry-run"));

        let windows_config = Config::for_platform_kind(PlatformKind::Windows);
        let windows = PlatformPaths::for_kind(PlatformKind::Windows);
        let bark_err = plan_probe_action(&windows_config, &windows, ProbeAction::BarkTest)
            .expect_err("Windows should not allow Probe actions");
        assert!(bark_err
            .to_string()
            .contains("probe is unavailable on windows"));
        let maintenance_err =
            plan_probe_action(&windows_config, &windows, ProbeAction::LogsDbExecute)
                .expect_err("Windows should not allow Probe logs DB maintenance");
        assert!(maintenance_err
            .to_string()
            .contains("probe_log_maintenance is unavailable on windows"));
    }

    #[test]
    fn logs_db_maintenance_plan_can_include_compact_without_adapter_mutation() {
        let config = Config::for_platform_kind(PlatformKind::Linux);
        let platform = PlatformPaths::for_kind(PlatformKind::Linux);

        let execute = plan_probe_logs_db_maintenance(
            &config,
            &platform,
            ProbeLogsDbMaintenanceRequest {
                dry_run: false,
                compact: true,
            },
        )
        .expect("Linux should plan execute maintenance with compact metadata");

        let maintenance = execute.maintenance.expect("maintenance spec");
        assert!(!maintenance.dry_run);
        assert!(maintenance.compact);
        let job = execute.job.expect("fixed job spec");
        assert_eq!(job.kind, "probe_logs_db_maintain");
        assert!(job.args.iter().any(|arg| arg == "--compact"));
        assert!(job.command.contains("--compact"));

        let dry_run = plan_probe_logs_db_maintenance(
            &config,
            &platform,
            ProbeLogsDbMaintenanceRequest {
                dry_run: true,
                compact: true,
            },
        )
        .expect("dry-run maintenance should stay plannable");
        let maintenance = dry_run.maintenance.expect("maintenance spec");
        assert!(maintenance.dry_run);
        assert!(!maintenance.compact);
        let job = dry_run.job.expect("fixed job spec");
        assert!(job.args.iter().any(|arg| arg == "--dry-run"));
        assert!(!job.args.iter().any(|arg| arg == "--compact"));
    }

    #[test]
    fn bark_delivery_and_status_labels_are_core_defined() {
        let mut config = Config::for_platform_kind(PlatformKind::Linux);
        config.probe.notifications.enabled = true;
        config.probe.notifications.notify_reply_needed = false;

        let disabled = probe_bark_delivery_decision(&config, "reply-needed", true, true);
        assert!(!disabled.should_send);
        assert_eq!(
            disabled.skip_reason.as_deref(),
            Some("event_switch_disabled")
        );
        assert_eq!(
            probe_bark_status_label(false, true, Some("dedupe")),
            "dedupe_hit"
        );
        assert_eq!(
            probe_bark_status_label(false, true, Some("sent_marker")),
            "skipped"
        );
        assert_eq!(
            probe_bark_status_label(false, false, Some("http_status")),
            "failed"
        );

        config.probe.notifications.notify_reply_needed = true;
        assert!(probe_bark_delivery_decision(&config, "reply-needed", true, true).should_send);
        assert_eq!(
            probe_bark_delivery_decision(&config, "reply-needed", false, true)
                .skip_reason
                .as_deref(),
            Some("device_key_missing")
        );
    }

    #[test]
    fn passive_notification_and_logs_db_scheduler_plans_are_core_defined() {
        let config = Config::for_platform_kind(PlatformKind::Linux);
        let now = 1_000_000;
        let not_due = probe_logs_db_scheduler_plan(&config, Some(now - 60), None, now);
        assert!(!not_due.should_run);
        assert_eq!(not_due.skip_reason.as_deref(), Some("not_due"));

        let due = probe_logs_db_scheduler_plan(&config, None, None, now);
        assert!(due.should_run);
        assert!(due.compact);

        let stored = probe_logs_db_stored_result(
            &json!({"target":"codex_logs_2","deleted_rows":2}),
            3,
            4,
            false,
        )
        .expect("stored result");
        assert_eq!(stored["probe_events_target"], "panel_probe_events");
        assert_eq!(stored["probe_events_deleted"], 3);
        assert_eq!(stored["probe_dedupe_deleted"], 4);

        let thread = ThreadSummary {
            id: "thread-a".to_string(),
            title: "Thread A".to_string(),
            status: ThreadStatus::Recoverable,
            updated_at: None,
            archived_at: None,
            message_count: 1,
            latest_message: Some("failed".to_string()),
            cwd: None,
            model: None,
            rollout_path: None,
            active_turn_id: None,
            active_job_id: None,
            pending_elicitation: None,
            last_event_kind: None,
        };
        let plan = probe_passive_thread_notification_plan(&thread, "recoverable");
        assert_eq!(plan.reason_label.as_deref(), Some("异常/可恢复"));
    }
}
