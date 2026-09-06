use crate::{
    auth::AuthContext,
    state::{AppState, CachedThreadDetail, FileSignature, ThreadDetailCacheSignature},
};
use anyhow::{anyhow, Result};
use nexushub_core::services::jobs::thread_state_action_response as core_thread_state_action_response;
use nexushub_core::{
    codex::{self, CodexPaths, ThreadDetail, ThreadSummary},
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
) -> Result<String> {
    let action = plan.action;
    let spec = plan
        .job
        .ok_or_else(|| anyhow!("Probe job is unavailable"))?;

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
        .start_exclusive_shell_job(&spec.kind, &spec.title, spec.command, group)
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
    let raw_threads = codex::list_threads(
        &paths,
        None,
        plan.list.query.q.as_deref(),
        plan.list.fetch_limit,
    )?;
    let view = thread_service::thread_list_read_model(
        &platform,
        thread_service::ThreadReadModelInputs {
            threads: raw_threads,
            running_jobs,
            hidden_thread_ids,
            archived_thread_ids,
            pending_followups: Vec::new(),
            default_workspace: state.config().codex.workspace.clone(),
        },
        plan.list.query,
    )?;
    Ok(view.threads)
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
        let detail_thread_id = detail.summary.id.clone();
        let active_job = active_job_for_thread(state, &detail_thread_id)?;
        let view = thread_service::thread_detail_read_model(
            &PlatformPaths::for_kind(nexushub_core::platform::PlatformKind::Linux),
            detail,
            active_job,
            None,
            state.config().codex.workspace.clone(),
        )?;
        return Ok(Some(view.detail));
    }
    Ok(None)
}

fn active_job_for_thread(state: &AppState, thread_id: &str) -> Result<Option<JobRecord>> {
    state.db.running_job_for_thread(thread_id)
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
    let cleanup = NexusHubUseCases::new(&platform).cleanup();
    match plan.target {
        cleanup_service::CleanupTarget::Archived => {
            let result = if plan.execute {
                let before = cleanup.dry_run_archived(&paths)?;
                cleanup.validate_expected_count(&plan, before.archived_threads)?;
                let result = cleanup.execute_archived(&paths)?;
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
                serde_json::to_value(cleanup.dry_run_archived(&paths)?)?
            };
            Ok(result)
        }
        cleanup_service::CleanupTarget::Hidden => {
            let result = if plan.execute {
                let before = cleanup.dry_run_hidden(&paths)?;
                cleanup.validate_expected_count(&plan, before.hidden_threads)?;
                let result = cleanup.execute_hidden(&paths)?;
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
                serde_json::to_value(cleanup.dry_run_hidden(&paths)?)?
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
    Ok(serde_json::to_value(response)?)
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
    maintenance: Option<probe_service::ProbeLogsDbMaintenanceRequest>,
) -> Result<probe_service::ProbeActionPlan> {
    let device_key_configured = state
        .db
        .get_secret_setting_bytes(settings_service::PROBE_BARK_DEVICE_KEY_SETTING)?
        .is_some_and(|value| !value.is_empty());
    let config = state.config();
    if let Some(request) = maintenance {
        return probe_service::ProbeUseCases::new(&config, platform)
            .logs_db_maintenance_with_config_path(request, config_path);
    }
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
