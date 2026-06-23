use crate::{auth::AuthContext, state::AppState};
use anyhow::{anyhow, Result};
use nexushub_core::{
    codex,
    config::{patch_probe_config_toml, Config},
    platform::PlatformPaths,
    services::{
        jobs as job_service, probe as probe_service, settings as settings_service,
        updates::{self as update_service, UpdateAction},
    },
};
use serde_json::{json, Value};
use std::{fs, path::Path};

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
