use crate::{
    auth::AuthContext,
    state::{AppState, CachedThreadDetail, FileSignature, ThreadDetailCacheSignature},
};
use anyhow::{anyhow, Result};
use nexushub_core::services::jobs::{
    cancel_followup_response as core_cancel_followup_response,
    codex_action_submitted as core_codex_action_submitted, followup_view as core_followup_view,
    followup_views as core_followup_views,
    thread_state_action_response as core_thread_state_action_response,
    thread_stop_response as core_thread_stop_response,
};
use nexushub_core::{
    codex::{self, CodexPaths, ThreadDetail, ThreadStatus, ThreadSummary},
    config::{patch_probe_config_toml, Config},
    db::JobRecord,
    jobs::CodexActionResult,
    platform::PlatformPaths,
    services::{
        cleanup as cleanup_service, goals as goal_service, jobs as job_service,
        probe as probe_service, settings as settings_service,
        threads::{self as thread_service, ThreadBlocksPage, ThreadsQuery},
        updates::{self as update_service, UpdateAction},
        uploads as upload_service,
        use_cases::{JobDetailPlan, JobListPlan, NexusHubUseCases},
    },
    uploads::{self as upload_core, PreparedAttachment, UploadOutcome},
};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs,
    path::{Path, Path as FsPath},
    time::UNIX_EPOCH,
};

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

pub fn start_thread_command_execution_plan(
    state: &AppState,
    auth: &AuthContext,
    plan: job_service::ThreadCommandExecutionPlan,
) -> Result<CodexActionResult> {
    start_thread_command_execution_plan_inner(state, Some(auth), plan)
}

fn start_thread_command_execution_plan_inner(
    state: &AppState,
    auth: Option<&AuthContext>,
    plan: job_service::ThreadCommandExecutionPlan,
) -> Result<CodexActionResult> {
    let resolved = state.resolved_codex_paths();
    let job_id = state.jobs.start_codex_job(
        &plan.spec.title,
        &resolved.home,
        &plan.spec.cwd,
        plan.spec.args.clone(),
        plan.spec.prompt.clone(),
    )?;
    state.db.link_job_thread(
        &job_id,
        plan.link.thread_id.as_deref(),
        plan.link.turn_id.as_deref(),
    )?;
    if let Some(auth) = auth {
        state.db.record_audit(
            Some(&auth.admin_id),
            &plan.audit.action,
            Some(&plan.audit.target_type),
            plan.audit
                .target_id
                .as_deref()
                .or(plan.link.thread_id.as_deref())
                .or(Some(&job_id)),
            None,
            plan.audit_detail(&job_id)?,
        )?;
    }
    plan.submitted_response(&job_id)
}

pub fn start_codex_resume_action(
    state: &AppState,
    auth: &AuthContext,
    thread_id: &str,
    message: String,
) -> Result<CodexActionResult> {
    let platform = PlatformPaths::for_kind(nexushub_core::platform::PlatformKind::Linux);
    let plan = NexusHubUseCases::new(&platform).threads().send_job(
        job_service::ThreadSendRequest {
            thread_id: Some(thread_id.to_string()),
            message: job_service::ThreadMessageRequest {
                thread_id: Some(thread_id.to_string()),
                message,
                ..job_service::ThreadMessageRequest::default()
            },
        },
        state.config().codex.workspace.clone(),
    )?;
    start_thread_command_execution_plan(state, auth, plan)
}

pub fn enqueue_followup_plan(
    state: &AppState,
    auth: &AuthContext,
    plan: job_service::FollowUpEnqueueFacadePlan,
    audit_action: &'static str,
) -> Result<job_service::FollowUpView> {
    let followup = job_service::enqueue_planned_followup(&state.db, plan.followup)?;
    state.db.record_audit(
        Some(&auth.admin_id),
        audit_action,
        Some("thread"),
        Some(&followup.thread_id),
        None,
        json!({"followup_id": followup.id}),
    )?;
    Ok(core_followup_view(followup))
}

pub fn goal_get_plan(
    state: &AppState,
    plan: goal_service::GoalGetPlan,
) -> Result<goal_service::GoalView> {
    let Some(thread_id) = plan.thread_id.as_deref() else {
        return Ok(goal_service::goal_empty("missing_thread"));
    };
    Ok(goal_service::goal_response(
        state.db.get_thread_goal(thread_id)?.as_ref(),
    ))
}

pub fn apply_goal_command_plan(
    state: &AppState,
    plan: goal_service::GoalCommandFacadePlan,
) -> Result<goal_service::GoalView> {
    let command: goal_service::GoalCommandPlan = plan.command;
    NexusHubUseCases::new(&PlatformPaths::for_kind(
        nexushub_core::platform::PlatformKind::Linux,
    ))
    .goals()
    .apply(&state.db, command)
}

pub fn goal_pause_plan(
    state: &AppState,
    thread_id: &str,
) -> Result<goal_service::GoalCommandFacadePlan> {
    let existing = state.db.get_thread_goal(thread_id)?;
    NexusHubUseCases::new(&PlatformPaths::for_kind(
        nexushub_core::platform::PlatformKind::Linux,
    ))
    .goals()
    .pause(thread_id, existing.as_ref())
}

pub fn goal_resume_plan(
    state: &AppState,
    thread_id: &str,
) -> Result<goal_service::GoalCommandFacadePlan> {
    let existing = state.db.get_thread_goal(thread_id)?;
    NexusHubUseCases::new(&PlatformPaths::for_kind(
        nexushub_core::platform::PlatformKind::Linux,
    ))
    .goals()
    .resume(thread_id, existing.as_ref())
}

pub fn resolve_thread_stop_plan(
    state: &AppState,
    plan: &job_service::ThreadStopPlan,
) -> Result<job_service::ThreadStopJobPlan> {
    let active_job_id = if plan.requires_active_job_lookup {
        derive_active_job_id(state, &plan.thread_id)
    } else {
        None
    };
    NexusHubUseCases::new(&PlatformPaths::for_kind(
        nexushub_core::platform::PlatformKind::Linux,
    ))
    .threads()
    .resolve_stop(plan, active_job_id)
}

pub fn codex_followup_queued_response(thread_id: String) -> CodexActionResult {
    core_codex_action_submitted(Some(thread_id), None)
}

pub fn record_thread_audit(
    state: &AppState,
    auth: &AuthContext,
    action: &'static str,
    thread_id: &str,
    detail: Value,
) -> Result<()> {
    state.db.record_audit(
        Some(&auth.admin_id),
        action,
        Some("thread"),
        Some(thread_id),
        None,
        detail,
    )
}

pub fn prepare_request_attachments(
    state: &AppState,
    attachment_ids: &[String],
) -> Result<Vec<PreparedAttachment>> {
    upload_service::validate_attachment_id_count(attachment_ids)?;
    let resolved = state.resolved_codex_paths();
    let root = upload_core::upload_root(&resolved.home);
    upload_core::prepare_uploads(&root, attachment_ids)
}

pub fn cleanup_stale_uploads_plan(state: &AppState) -> Result<()> {
    let protected_ids = state
        .db
        .active_followup_upload_ids()
        .unwrap_or_else(|err| {
            tracing::warn!("active follow-up upload lookup failed: {err}");
            HashSet::new()
        })
        .into_iter()
        .collect::<Vec<_>>();
    let platform = PlatformPaths::for_kind(nexushub_core::platform::PlatformKind::Linux);
    let plan: upload_service::UploadRetentionPlan = NexusHubUseCases::new(&platform)
        .uploads()
        .retention(upload_service::UploadRetentionRequest {
            protected_ids,
            ttl_seconds: Some(upload_core::UPLOAD_TTL_SECONDS),
        })?;
    let resolved = state.resolved_codex_paths();
    let root = upload_core::upload_root(&resolved.home);
    if let Err(err) = upload_service::execute_upload_retention_plan(&root, &plan) {
        tracing::warn!("stale upload cleanup failed: {err}");
    }
    Ok(())
}

pub fn store_upload_plan(
    state: &AppState,
    auth: &AuthContext,
    plan: upload_service::UploadStorePlan,
) -> Result<UploadOutcome> {
    let total_files = plan.total_files;
    let total_bytes = plan.total_bytes;
    let resolved = state.resolved_codex_paths();
    let root = upload_core::upload_root(&resolved.home);
    let outcome = upload_service::store_upload_plan(&root, plan)?;
    state.db.record_audit(
        Some(&auth.admin_id),
        "uploads.create",
        Some("upload"),
        None,
        None,
        json!({"files": total_files, "bytes": total_bytes}),
    )?;
    Ok(outcome)
}

pub fn delete_upload_plan(
    state: &AppState,
    auth: &AuthContext,
    plan: upload_service::UploadDeletePlan,
) -> Result<bool> {
    let resolved = state.resolved_codex_paths();
    let root = upload_core::upload_root(&resolved.home);
    let deleted = upload_service::execute_delete_upload_plan(&root, &plan)?;
    if deleted {
        state.db.record_audit(
            Some(&auth.admin_id),
            "uploads.delete",
            Some("upload"),
            Some(&plan.id),
            None,
            json!({}),
        )?;
    }
    Ok(deleted)
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
    let mut threads = thread_service::build_threads_overview(
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
    .threads;
    autosubmit_ready_followups(state, &mut threads);
    Ok(threads)
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
    if let Some(mut detail) = detail.take() {
        let active_job = state.db.running_job_for_thread(&detail.summary.id)?;
        detail = thread_service::apply_thread_detail_runtime_state(detail, active_job.as_ref());
        autosubmit_pending_followup(state, &mut detail);
        return Ok(Some(detail));
    }
    Ok(None)
}

pub fn window_thread_detail_read_model(
    state: &AppState,
    plan: &thread_service::ThreadDetailPlan,
) -> Result<Option<ThreadDetail>> {
    let Some(detail) = load_thread_detail_read_model(state, &plan.thread_id)? else {
        return Ok(None);
    };
    Ok(Some(thread_service::window_thread_detail_for_plan(
        detail, plan,
    )))
}

pub fn thread_blocks_read_model(
    state: &AppState,
    plan: &thread_service::ThreadDetailPlan,
) -> Result<Option<ThreadBlocksPage>> {
    let Some(detail) = load_thread_detail_read_model(state, &plan.thread_id)? else {
        return Ok(None);
    };
    Ok(Some(thread_service::thread_blocks_page_for_plan(
        detail, plan,
    )))
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
        if detail.summary.active_job_id.is_some() {
            *summary = detail.summary;
        }
    }
}

pub fn autosubmit_pending_followup(state: &AppState, detail: &mut ThreadDetail) {
    if !matches!(detail.summary.status, ThreadStatus::Recent) {
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
    let plan: job_service::FollowUpAutoSubmitExecutionPlan = match use_cases
        .autosubmit_followup_job(
            detail.summary.status.clone(),
            &followup,
            state.config().codex.workspace.clone(),
        ) {
        Ok(plan) => plan,
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
    let Some(job_plan) = plan.job.clone() else {
        return;
    };
    match start_thread_command_execution_plan_inner(state, None, job_plan) {
        Ok(result) => {
            let Some(job_id) = result.job_id else {
                return;
            };
            let submit = match plan.submitted_result(&job_id) {
                Ok(submit) => submit,
                Err(err) => {
                    let _ = use_cases.apply_followup_error(
                        &state.db,
                        job_service::FollowUpErrorRequest {
                            followup_id: followup.id.clone(),
                            error: err.to_string(),
                        },
                    );
                    return;
                }
            };
            let _ = use_cases.apply_followup_submitted(
                &state.db,
                job_service::FollowUpSubmitResultRequest {
                    followup_id: submit.followup_id,
                    result: submit.result,
                },
            );
            if let Ok(active_job) = state.db.running_job_for_thread(&thread_id) {
                let current = detail.clone();
                *detail =
                    thread_service::apply_thread_detail_runtime_state(current, active_job.as_ref());
            }
        }
        Err(err) => {
            let error = plan.error_result(&err.to_string()).unwrap_or_else(|_| {
                job_service::FollowUpErrorPlan {
                    required_capability: plan.required_capability,
                    command: nexushub_core::services::commands::THREADS_FOLLOWUPS_ERROR.to_string(),
                    followup_id: followup.id.clone(),
                    error: err.to_string(),
                }
            });
            let _ = use_cases.apply_followup_error(
                &state.db,
                job_service::FollowUpErrorRequest {
                    followup_id: error.followup_id,
                    error: error.error,
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
        anyhow::bail!("cleanup execute must be confirmed");
    }
    let paths = state.codex_paths();
    let platform = PlatformPaths::for_kind(nexushub_core::platform::PlatformKind::Linux);
    match plan.target {
        cleanup_service::CleanupTarget::Archived => {
            let result = if plan.execute {
                let before = cleanup_service::dry_run_archived_with_capability(&platform, &paths)?;
                cleanup_service::validate_cleanup_expected_count(&plan, before.archived_threads)?;
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
                cleanup_service::validate_cleanup_expected_count(&plan, before.hidden_threads)?;
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
    Ok(core_thread_stop_response(stop, cancelled))
}

pub fn cancel_followup_plan(
    state: &AppState,
    plan: job_service::FollowUpCancelPlan,
) -> Result<job_service::ActionResponse> {
    let cancelled = state
        .db
        .cancel_followup(&plan.thread_id, &plan.followup_id)?;
    Ok(core_cancel_followup_response(
        nexushub_core::services::commands::THREADS_FOLLOWUPS_CANCEL,
        plan.thread_id,
        plan.followup_id,
        cancelled,
    ))
}

pub fn list_followups_plan(
    state: &AppState,
    plan: job_service::FollowUpListPlan,
) -> Result<Vec<job_service::FollowUpView>> {
    Ok(core_followup_views(
        state.db.list_followups(&plan.thread_id, plan.limit)?,
    ))
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
    core_thread_state_action_response(plan)
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
