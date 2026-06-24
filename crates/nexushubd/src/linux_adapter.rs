use crate::{
    auth::AuthContext,
    state::{AppState, CachedThreadDetail, FileSignature, ThreadDetailCacheSignature},
};
use anyhow::{anyhow, Result};
use nexushub_core::{
    codex::{self, CodexPaths, ThreadDetail, ThreadStatus, ThreadSummary},
    config::{patch_probe_config_toml, Config},
    db::JobRecord,
    platform::PlatformPaths,
    services::{
        cleanup as cleanup_service, jobs as job_service, probe as probe_service,
        settings as settings_service,
        threads::{self as thread_service, ThreadBlocksPage, ThreadsQuery},
        updates::{self as update_service, UpdateAction},
        use_cases::{JobDetailPlan, JobListPlan, NexusHubUseCases},
    },
};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs,
    path::{Path, Path as FsPath},
    time::UNIX_EPOCH,
};

pub struct CodexJobAuditPlan<'a> {
    pub action: &'a str,
    pub resource_kind: &'a str,
    pub resource_id: Option<&'a str>,
    pub metadata: Value,
}

pub fn apply_probe_settings_save_plan(
    state: &AppState,
    auth: &AuthContext,
    config_path: &Path,
    plan: settings_service::ProbeSettingsSavePlan,
) -> Result<Config> {
    let text = fs::read_to_string(config_path)?;
    let updated = patch_probe_config_toml(&text, &plan.config_patch)?;
    fs::write(config_path, updated)?;
    let response_config = Config::load(config_path)?;

    for write in &plan.secret_writes {
        state
            .db
            .set_secret_setting_bytes(&write.setting_key, write.secret_value.as_bytes())?;
    }

    state.replace_config(response_config.clone());
    state.db.record_audit(
        Some(&auth.admin_id),
        "probe_settings.updated",
        Some("probe"),
        Some("settings"),
        None,
        json!({"config_path": config_path}),
    )?;

    Ok(response_config)
}

pub fn start_probe_action_plan(
    state: &AppState,
    auth: &AuthContext,
    plan: probe_service::ProbeActionPlan,
    compact: bool,
) -> Result<String> {
    let action = plan.action;
    let spec = plan
        .job
        .ok_or_else(|| anyhow!("Probe job is unavailable"))?;
    let mut command = spec.command.clone();
    if compact
        && matches!(
            action,
            probe_service::ProbeAction::LogsDbDryRun | probe_service::ProbeAction::LogsDbExecute
        )
    {
        command = format!("{command} {}", shell_quote("--compact"));
    }

    state.db.record_audit(
        Some(&auth.admin_id),
        &format!("{}.started", spec.kind),
        Some("probe"),
        Some(&spec.title),
        None,
        json!({"args": spec.args, "action": action.as_rpc_action()}),
    )?;
    let group = spec.exclusive_group.as_deref().unwrap_or(&spec.kind);
    state
        .jobs
        .start_exclusive_shell_job(&spec.kind, &spec.title, command, group)
}

pub fn start_codex_job_spec(
    state: &AppState,
    auth: &AuthContext,
    spec: job_service::CodexJobSpec,
    link_thread_id: Option<&str>,
    mut audit: CodexJobAuditPlan<'_>,
) -> Result<String> {
    let resolved = state.resolved_codex_paths();
    let cwd = spec.cwd.clone();
    let job_id = state.jobs.start_codex_job(
        &spec.title,
        &resolved.home,
        &spec.cwd,
        spec.args,
        spec.prompt,
    )?;
    if let Value::Object(ref mut object) = audit.metadata {
        object.insert("job_id".to_string(), json!(job_id.clone()));
    }
    state.db.link_job_thread(&job_id, link_thread_id, None)?;
    state.db.record_audit(
        Some(&auth.admin_id),
        audit.action,
        Some(audit.resource_kind),
        audit.resource_id.or(Some(&job_id)),
        None,
        audit.metadata,
    )?;
    tracing::debug!(
        job_id,
        cwd = %cwd.display(),
        thread_id = link_thread_id.unwrap_or(""),
        "started Linux Codex job from shared core spec"
    );
    Ok(job_id)
}

pub fn start_codex_resume_spec(
    state: &AppState,
    spec: job_service::CodexJobSpec,
    thread_id: &str,
) -> Result<String> {
    let resolved = state.resolved_codex_paths();
    let job_id = state.jobs.start_codex_job(
        &spec.title,
        &resolved.home,
        &spec.cwd,
        spec.args,
        spec.prompt,
    )?;
    state.db.link_job_thread(&job_id, Some(thread_id), None)?;
    Ok(job_id)
}

pub fn list_threads_read_model(
    state: &AppState,
    query: ThreadsQuery,
) -> Result<Vec<ThreadSummary>> {
    let platform = PlatformPaths::for_kind(nexushub_core::platform::PlatformKind::Linux);
    let plan = NexusHubUseCases::new(&platform)
        .threads()
        .list_read(query)?;
    let paths = state.codex_paths();
    let hidden_thread_ids = if plan.include_hidden_thread_ids {
        codex::hidden_thread_ids(&paths).unwrap_or_else(|err| {
            tracing::warn!("failed to read hidden thread metadata: {err}");
            HashSet::new()
        })
    } else {
        HashSet::new()
    };
    let archived_thread_ids = if plan.include_archived_thread_ids {
        codex::archived_thread_ids(&paths).unwrap_or_else(|err| {
            tracing::warn!("failed to read archived thread metadata: {err}");
            HashSet::new()
        })
    } else {
        HashSet::new()
    };
    let running_jobs = if plan.include_running_jobs {
        state.db.running_thread_jobs()?
    } else {
        Vec::new()
    };
    Ok(thread_service::build_threads_overview(
        codex::list_threads(
            &paths,
            None,
            plan.list.query.q.as_deref(),
            plan.list.fetch_limit,
        )?,
        running_jobs,
        plan.list.query,
        &hidden_thread_ids,
        &archived_thread_ids,
    )
    .threads)
}

pub fn probe_threads_read_model(
    state: &AppState,
    status: &'static str,
    limit: usize,
) -> Result<Vec<ThreadSummary>> {
    let paths = state.codex_paths();
    if thread_service::thread_list_fetch_limit(Some(status), Some(limit)) == usize::MAX {
        return probe_service::probe_threads_for_status_with_paths(
            &paths,
            state.db.path(),
            status,
            limit,
        );
    }
    list_threads_read_model(
        state,
        ThreadsQuery {
            status: Some(status.to_string()),
            q: None,
            limit: Some(limit.clamp(1, 200)),
        },
    )
}

pub fn load_thread_detail_read_model(
    state: &AppState,
    thread_id: &str,
) -> Result<Option<ThreadDetail>> {
    let paths = state.codex_paths();
    let mut detail = load_base_thread_detail_cached(state, &paths, thread_id)?;
    if let Some(detail) = detail.take() {
        let active_job = state.db.running_job_for_thread(&detail.summary.id)?;
        return Ok(Some(thread_service::apply_thread_detail_runtime_state(
            detail,
            active_job.as_ref(),
        )));
    }
    Ok(None)
}

pub fn autosubmit_ready_followups(state: &AppState, threads: &mut [ThreadSummary]) {
    for summary in threads {
        if !matches!(summary.status, ThreadStatus::Recent) {
            continue;
        }
        let mut detail = ThreadDetail {
            summary: summary.clone(),
            messages: Vec::new(),
            blocks: Vec::new(),
            raw_event_count: 0,
            total_blocks: 0,
            has_more_blocks: false,
            before_cursor: None,
        };
        autosubmit_pending_followup(state, &mut detail);
        if matches!(detail.summary.status, ThreadStatus::Running) {
            *summary = detail.summary;
        }
    }
}

pub fn autosubmit_pending_followup(state: &AppState, detail: &mut ThreadDetail) {
    let autosubmit = job_service::plan_followup_autosubmit(detail.summary.status.clone(), true);
    if !autosubmit.should_claim_pending {
        return;
    }
    let thread_id = detail.summary.id.clone();
    let platform = PlatformPaths::for_kind(nexushub_core::platform::PlatformKind::Linux);
    let use_cases = NexusHubUseCases::new(&platform).threads();
    let Ok(Some(followup)) = use_cases.claim_next_followup(
        &state.db,
        job_service::FollowUpClaimRequest {
            thread_id: thread_id.clone(),
        },
    ) else {
        return;
    };
    let spec = match job_service::plan_queued_followup_job_spec(
        &followup,
        state.config().codex.workspace.clone(),
    ) {
        Ok(spec) => spec,
        Err(err) => {
            let message = err.to_string();
            let _ = use_cases.apply_followup_error(
                &state.db,
                job_service::FollowUpErrorRequest {
                    followup_id: followup.id.clone(),
                    error: message.clone(),
                },
            );
            tracing::warn!("failed to build follow-up codex job spec: {message}");
            return;
        }
    };
    match start_codex_resume_spec(state, spec, &thread_id) {
        Ok(job_id) => {
            let _ = use_cases.apply_followup_submitted(
                &state.db,
                job_service::FollowUpSubmitResultRequest {
                    followup_id: followup.id.clone(),
                    result: json!({"job_id": job_id}),
                },
            );
            detail.summary.status = ThreadStatus::Running;
            detail.summary.active_job_id = Some(job_id);
        }
        Err(err) => {
            let _ = use_cases.apply_followup_error(
                &state.db,
                job_service::FollowUpErrorRequest {
                    followup_id: followup.id,
                    error: err.to_string(),
                },
            );
        }
    }
}

pub fn derive_active_job_id(state: &AppState, thread_id: &str) -> Option<String> {
    state
        .db
        .running_job_for_thread(thread_id)
        .ok()
        .flatten()
        .map(|job| job.id)
}

pub fn thread_blocks_page(
    detail: ThreadDetail,
    plan: &thread_service::ThreadDetailPlan,
) -> ThreadBlocksPage {
    thread_service::thread_blocks_page_for_plan(detail, plan)
}

pub fn window_thread_detail(
    detail: ThreadDetail,
    plan: &thread_service::ThreadDetailPlan,
) -> ThreadDetail {
    thread_service::window_thread_detail_for_plan(detail, plan)
}

pub fn list_jobs_plan(state: &AppState, plan: JobListPlan) -> Result<Vec<Value>> {
    state
        .db
        .list_jobs(plan.limit)?
        .into_iter()
        .map(job_response_value)
        .collect()
}

pub fn job_detail_plan(state: &AppState, plan: JobDetailPlan) -> Result<Option<Value>> {
    state
        .db
        .job(&plan.job_id)?
        .map(job_response_value)
        .transpose()
}

pub fn execute_cleanup_plan(
    state: &AppState,
    auth: &AuthContext,
    plan: cleanup_service::CleanupOperationPlan,
) -> Result<Value> {
    if plan.requires_confirmation && !plan.confirmation.confirmed {
        anyhow::bail!(cleanup_confirmation_message(plan.target));
    }
    let paths = state.codex_paths();
    let platform = PlatformPaths::for_kind(nexushub_core::platform::PlatformKind::Linux);
    match plan.target {
        cleanup_service::CleanupTarget::Archived => {
            let result = if plan.execute {
                let before = cleanup_service::dry_run_archived_with_capability(&platform, &paths)?;
                ensure_cleanup_expected_count(
                    plan.confirmation.expected_count,
                    before.archived_threads,
                )?;
                let result = cleanup_service::execute_archived_with_capability(&platform, &paths)?;
                state.db.record_audit(
                    Some(&auth.admin_id),
                    "archives.delete.execute",
                    Some("archives"),
                    Some("root-codex"),
                    None,
                    json!({"before_archived": result.before.archived_threads, "deleted_rollout_files": result.deleted_rollout_files}),
                )?;
                serde_json::to_value(result)?
            } else {
                serde_json::to_value(cleanup_service::dry_run_archived_with_capability(
                    &platform, &paths,
                )?)?
            };
            Ok(result)
        }
        cleanup_service::CleanupTarget::Hidden => {
            let result = if plan.execute {
                let before = cleanup_service::dry_run_hidden_with_capability(&platform, &paths)?;
                ensure_cleanup_expected_count(
                    plan.confirmation.expected_count,
                    before.hidden_threads,
                )?;
                let result = cleanup_service::execute_hidden_with_capability(&platform, &paths)?;
                state.db.record_audit(
                    Some(&auth.admin_id),
                    "hidden_threads.delete.execute",
                    Some("hidden_threads"),
                    Some("root-codex"),
                    None,
                    json!({
                        "before_hidden": result.before.hidden_threads,
                        "deleted_threads": result.deleted_threads,
                        "deleted_rollout_files": result.deleted_rollout_files,
                    }),
                )?;
                serde_json::to_value(result)?
            } else {
                serde_json::to_value(cleanup_service::dry_run_hidden_with_capability(
                    &platform, &paths,
                )?)?
            };
            Ok(result)
        }
    }
}

fn ensure_cleanup_expected_count(expected_count: Option<u64>, actual_count: u64) -> Result<()> {
    let Some(expected_count) = expected_count else {
        anyhow::bail!("cleanup expectedCount is required before deletion");
    };
    if expected_count != actual_count {
        anyhow::bail!(
            "cleanup expectedCount mismatch: expected={expected_count} actual={actual_count}"
        );
    }
    Ok(())
}

fn cleanup_confirmation_message(target: cleanup_service::CleanupTarget) -> &'static str {
    match target {
        cleanup_service::CleanupTarget::Archived => "archive deletion must be confirmed",
        cleanup_service::CleanupTarget::Hidden => "hidden thread deletion must be confirmed",
    }
}

fn job_response_value(job: JobRecord) -> Result<Value> {
    let response = NexusHubUseCases::new(&PlatformPaths::for_kind(
        nexushub_core::platform::PlatformKind::Linux,
    ))
    .jobs()
    .response(job);
    let mut value = serde_json::to_value(response)?;
    let Some(analysis) = value
        .get("failure_analysis")
        .cloned()
        .filter(|value| !value.is_null())
    else {
        return Ok(value);
    };
    if let Some(explanation) = analysis.get("explanation").and_then(Value::as_str) {
        value["analysis"] = Value::String(explanation.to_string());
    }
    let suggestions = analysis
        .get("suggestions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    value["explanation"] = Value::String(suggestions);
    Ok(value)
}

fn load_base_thread_detail_cached(
    state: &AppState,
    paths: &CodexPaths,
    id: &str,
) -> Result<Option<ThreadDetail>> {
    if let Some(cached) = state
        .rollout_detail_cache
        .lock()
        .expect("rollout detail cache mutex")
        .get(id)
        .cloned()
    {
        let signature = thread_detail_cache_signature(paths, cached.signature.rollout_path.clone());
        if cached.signature == signature {
            return Ok(Some(cached.detail));
        }
    }

    let detail = codex::thread_detail(paths, id)?;
    let signature = thread_detail_cache_signature(
        paths,
        detail
            .as_ref()
            .and_then(|detail| detail.summary.rollout_path.clone()),
    );
    if let Some(detail) = detail.as_ref() {
        state
            .rollout_detail_cache
            .lock()
            .expect("rollout detail cache mutex")
            .insert(
                id.to_string(),
                CachedThreadDetail {
                    signature,
                    detail: detail.clone(),
                },
            );
    } else {
        state
            .rollout_detail_cache
            .lock()
            .expect("rollout detail cache mutex")
            .remove(id);
    }
    Ok(detail)
}

fn thread_detail_cache_signature(
    paths: &CodexPaths,
    rollout_path: Option<std::path::PathBuf>,
) -> ThreadDetailCacheSignature {
    ThreadDetailCacheSignature {
        rollout: rollout_path.as_deref().and_then(file_signature),
        rollout_path,
        state_db: file_signature(&paths.state_db()),
        session_index: file_signature(&paths.session_index()),
    }
}

fn file_signature(path: &FsPath) -> Option<FileSignature> {
    let metadata = fs::metadata(path).ok()?;
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis());
    Some(FileSignature {
        len: metadata.len(),
        modified_ms,
    })
}

pub fn cancel_thread_stop_plan(
    state: &AppState,
    auth: &AuthContext,
    stop: &job_service::ThreadStopJobPlan,
) -> Result<job_service::ActionResponse> {
    let cancelled = state.jobs.cancel_job(&stop.job_id)?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "thread.stop.job_cancel",
        Some("job"),
        Some(&stop.job_id),
        None,
        json!({"thread_id": &stop.thread_id, "cancelled": cancelled}),
    )?;
    Ok(job_service::thread_stop_response(stop, cancelled))
}

pub fn cancel_followup_plan(
    state: &AppState,
    plan: job_service::FollowUpCancelPlan,
) -> Result<job_service::ActionResponse> {
    let cancelled = state
        .db
        .cancel_followup(&plan.thread_id, &plan.followup_id)?;
    Ok(job_service::cancel_followup_response(
        nexushub_core::services::commands::THREADS_FOLLOWUPS_CANCEL,
        plan.thread_id,
        plan.followup_id,
        cancelled,
    ))
}

pub fn list_followups_plan(
    state: &AppState,
    plan: job_service::FollowUpListPlan,
) -> Result<Vec<nexushub_core::db::ThreadFollowUp>> {
    state.db.list_followups(&plan.thread_id, plan.limit)
}

pub fn apply_thread_state_action_plan(
    state: &AppState,
    auth: &AuthContext,
    plan: &job_service::ThreadStateActionPlan,
) -> Result<job_service::ActionResponse> {
    let paths = state.codex_paths();
    if let Some(archived) = plan.archived {
        codex::set_thread_archived(&paths, &plan.thread_id, archived)?;
        state.db.record_audit(
            Some(&auth.admin_id),
            if archived {
                "thread.archived"
            } else {
                "thread.restored"
            },
            Some("thread"),
            Some(&plan.thread_id),
            None,
            json!({}),
        )?;
    }
    if let Some(name) = plan.name.as_deref() {
        codex::set_thread_title(&paths, &plan.thread_id, name)?;
        state.db.record_audit(
            Some(&auth.admin_id),
            "thread.renamed",
            Some("thread"),
            Some(&plan.thread_id),
            None,
            json!({"name": name}),
        )?;
    }
    job_service::thread_state_action_response(plan)
}

pub fn start_update_action_plan(
    state: &AppState,
    auth: &AuthContext,
    plan: update_service::UpdateActionPlan,
    audit_action: Option<&str>,
) -> Result<String> {
    let action = plan.action;
    let spec = plan
        .linux_job
        .ok_or_else(|| anyhow!("Linux update job is unavailable"))?;
    if let Some(audit_action) = audit_action {
        state.db.record_audit(
            Some(&auth.admin_id),
            audit_action,
            Some("system"),
            Some("updates"),
            None,
            json!({ "action": format!("{action:?}") }),
        )?;
    }
    if let Some(group) = spec.exclusive_group.as_deref() {
        state
            .jobs
            .start_exclusive_shell_job(&spec.kind, &spec.title, spec.command, group)
    } else {
        state
            .jobs
            .start_shell_job(&spec.kind, &spec.title, spec.command)
    }
}

pub fn linux_probe_action_plan(
    state: &AppState,
    platform: &PlatformPaths,
    action: probe_service::ProbeAction,
    config_path: &Path,
) -> Result<probe_service::ProbeActionPlan> {
    let device_key_configured = state
        .db
        .get_secret_setting_bytes(settings_service::PROBE_BARK_DEVICE_KEY_SETTING)?
        .is_some_and(|value| !value.is_empty());
    let config = state.config();
    probe_service::ProbeUseCases::new(&config, platform).action_with_device_key_and_config_path(
        action,
        device_key_configured,
        config_path,
    )
}

pub fn linux_update_action_plan(
    state: &AppState,
    platform: &PlatformPaths,
    action: UpdateAction,
) -> Result<update_service::UpdateActionPlan> {
    let config = state.config();
    update_service::UpdateUseCases::new(&config, platform).action_plan(action)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
