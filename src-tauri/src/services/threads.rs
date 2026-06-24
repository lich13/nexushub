use anyhow::Result;
use nexushub_core::{
    codex::{
        archived_thread_ids, hidden_thread_ids, list_threads, set_thread_archived,
        set_thread_title, thread_detail, ThreadDetail, ThreadSummary,
    },
    db::{JobRecord, ThreadFollowUp},
    services::{
        jobs as job_service,
        threads::{self as thread_service, ThreadBlocksPage, ThreadsQuery},
        uploads as upload_service,
        use_cases::NexusHubUseCases,
    },
    uploads,
};
use serde::Deserialize;
use std::collections::HashMap;

use crate::{overview::DesktopState, services::actions::DesktopActionResponse};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopSendMessageRequest {
    #[serde(default, alias = "threadId", alias = "thread_id")]
    pub thread_id: Option<String>,
    pub message: String,
    #[serde(default)]
    pub attachments: Vec<String>,
    pub model: Option<String>,
    #[serde(alias = "service_tier")]
    pub service_tier: Option<String>,
    #[serde(alias = "reasoning_effort")]
    pub reasoning_effort: Option<String>,
    pub cwd: Option<String>,
    #[serde(alias = "permission_profile")]
    pub permission_profile: Option<String>,
    #[serde(alias = "approval_policy")]
    pub approval_policy: Option<String>,
    #[serde(alias = "sandbox_mode")]
    pub sandbox_mode: Option<String>,
    #[serde(alias = "network_access")]
    pub network_access: Option<bool>,
    #[serde(alias = "collaboration_mode")]
    pub collaboration_mode: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadListRequest {
    pub status: Option<String>,
    pub query: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadDetailRequest {
    pub id: String,
    pub limit: Option<usize>,
    pub full: Option<bool>,
    pub before: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadBlocksRequest {
    pub id: String,
    pub limit: Option<usize>,
    pub before: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopStopRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
    #[serde(alias = "turn_id")]
    pub turn_id: Option<String>,
    #[serde(alias = "job_id")]
    pub job_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopThreadIdRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopRenameThreadRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopPlanAcceptRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
    #[serde(alias = "turn_id")]
    pub turn_id: Option<String>,
    #[serde(alias = "item_id")]
    pub item_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopPlanReviseRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
    #[serde(alias = "turn_id")]
    pub turn_id: Option<String>,
    #[serde(alias = "item_id")]
    pub item_id: Option<String>,
    pub instructions: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopElicitationAnswerRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
    pub answers: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopApprovalAnswerRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopFollowupRequest {
    pub thread_id: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopCancelFollowupRequest {
    #[serde(alias = "threadId", alias = "thread_id")]
    pub thread_id: String,
    #[serde(alias = "followUpId", alias = "followupId", alias = "followup_id")]
    pub followup_id: String,
}

pub(crate) fn thread_summaries_with_query(
    state: &DesktopState,
    query: ThreadsQuery,
) -> Result<Vec<ThreadSummary>> {
    let paths = state.codex_paths();
    let plan = thread_service::plan_thread_list_read(state.platform(), query)?;
    let hidden_thread_ids = if plan.include_hidden_thread_ids {
        hidden_thread_ids(&paths).unwrap_or_default()
    } else {
        Default::default()
    };
    let archived_thread_ids = if plan.include_archived_thread_ids {
        archived_thread_ids(&paths).unwrap_or_default()
    } else {
        Default::default()
    };
    let running_jobs = if plan.include_running_jobs {
        state.db.running_thread_jobs()?
    } else {
        Vec::new()
    };
    let raw_threads = list_threads(
        &paths,
        None,
        plan.list.query.q.as_deref(),
        plan.list.fetch_limit,
    )?;
    let pending_followups = raw_threads
        .iter()
        .flat_map(|thread| pending_followup_for_thread(state, &thread.id).transpose())
        .collect::<Result<Vec<_>>>()?;
    let view = thread_service::thread_list_read_model(
        state.platform(),
        thread_service::ThreadReadModelInputs {
            threads: raw_threads,
            running_jobs,
            hidden_thread_ids,
            archived_thread_ids,
            pending_followups,
            default_workspace: state.config().codex.workspace.clone(),
        },
        plan.list.query,
    )?;
    execute_autosubmit_effects(state, view.autosubmit_effects)?;
    Ok(view.threads)
}

#[allow(dead_code)]
pub(crate) fn codex_job_spec_for_request(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
    kind: job_service::CodexActionKind,
) -> Result<job_service::CodexJobSpec> {
    let attachments = prepare_request_attachments(state, &request.attachments)?;
    let message = request.into_thread_message(attachments);
    let config = state.config();
    let plan = job_service::plan_thread_command_job_execution(
        state.platform(),
        job_service::ThreadCommandRequest {
            command: match kind {
                job_service::CodexActionKind::Exec => job_service::ThreadCommandKind::Create,
                job_service::CodexActionKind::Resume => job_service::ThreadCommandKind::Resume,
            },
            thread_id: message.thread_id.clone(),
            message,
        },
        config.codex.workspace.clone(),
    )?;
    Ok(plan.spec)
}

pub(crate) fn prepare_request_attachments(
    state: &DesktopState,
    attachment_ids: &[String],
) -> Result<Vec<uploads::PreparedAttachment>> {
    upload_service::validate_attachment_id_count(attachment_ids)?;
    let root = uploads::upload_root(&state.resolved_codex_paths().home);
    uploads::prepare_uploads(&root, attachment_ids)
}

impl DesktopSendMessageRequest {
    pub(crate) fn with_thread_id_fallback(mut self, thread_id: Option<String>) -> Self {
        if self.thread_id.is_none() {
            self.thread_id = thread_id;
        }
        self
    }

    pub(crate) fn without_thread_id(mut self) -> Self {
        self.thread_id = None;
        self
    }

    pub(crate) fn into_thread_message(
        self,
        prepared_attachments: Vec<uploads::PreparedAttachment>,
    ) -> job_service::ThreadMessageRequest {
        job_service::ThreadMessageRequest {
            thread_id: self.thread_id,
            message: self.message,
            attachments: self.attachments,
            prepared_attachments,
            model: self.model,
            service_tier: self.service_tier,
            reasoning_effort: self.reasoning_effort,
            cwd: self.cwd,
            permission_profile: self.permission_profile,
            approval_policy: self.approval_policy,
            sandbox_mode: self.sandbox_mode,
            network_access: self.network_access,
            collaboration_mode: self.collaboration_mode,
        }
    }
}

pub(crate) fn threads_with_state(
    state: &DesktopState,
    request: ThreadListRequest,
) -> Result<Vec<ThreadSummary>> {
    thread_summaries_with_query(
        state,
        ThreadsQuery {
            status: request.status,
            q: request.query,
            limit: request.limit,
        },
    )
}

pub(crate) fn thread_detail_with_state(
    state: &DesktopState,
    request: ThreadDetailRequest,
) -> Result<Option<ThreadDetail>> {
    let plan = thread_service::plan_thread_detail_read(
        state.platform(),
        thread_service::ThreadDetailRequest {
            id: request.id,
            limit: request.limit,
            full: request.full,
            before: request.before,
        },
    )?;
    let Some(detail) = load_thread_detail_read_model(state, &plan.detail.thread_id)? else {
        return Ok(None);
    };
    Ok(Some(thread_service::window_thread_detail_for_plan(
        detail,
        &plan.detail,
    )))
}

pub(crate) fn thread_blocks_with_state(
    state: &DesktopState,
    request: ThreadBlocksRequest,
) -> Result<Option<ThreadBlocksPage>> {
    let plan = thread_service::plan_thread_blocks_read(
        state.platform(),
        &request.id,
        request.limit,
        request.before,
    )?;
    let Some(detail) = load_thread_detail_read_model(state, &plan.detail.thread_id)? else {
        return Ok(None);
    };
    Ok(Some(thread_service::thread_blocks_page_for_plan(
        detail,
        &plan.detail,
    )))
}

pub(crate) fn send_message_with_state(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    let attachments = prepare_request_attachments(state, &request.attachments)?;
    let config = state.config();
    let plan = job_service::plan_thread_send_job_execution(
        state.platform(),
        job_service::ThreadSendRequest {
            thread_id: request.thread_id.clone(),
            message: request.into_thread_message(attachments),
        },
        config.codex.workspace.clone(),
    )?;
    start_codex_job_from_plan(state, plan)
}

pub(crate) fn steer_thread_with_state(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    let attachments = prepare_request_attachments(state, &request.attachments)?;
    let facade = job_service::plan_thread_steer_with_capability(
        state.platform(),
        job_service::ThreadSteerRequest {
            thread_id: request.thread_id.clone(),
            message: request.into_thread_message(attachments),
        },
    )?;
    let followup = facade
        .command
        .followup
        .ok_or_else(|| anyhow::anyhow!("thread steer plan is missing follow-up action"))?;
    let followup = job_service::enqueue_planned_followup(&state.db, followup)?;
    Ok(job_service::codex_action_submitted(
        Some(followup.thread_id),
        None,
    ))
}

pub(crate) fn stop_thread_with_state(
    state: &DesktopState,
    request: DesktopStopRequest,
) -> Result<DesktopActionResponse> {
    let plan = job_service::plan_thread_stop_with_capability(
        state.platform(),
        job_service::ThreadStopRequest {
            thread_id: request.thread_id,
            turn_id: request.turn_id,
            job_id: request.job_id,
        },
    )?;
    let active_job_id = plan
        .requires_active_job_lookup
        .then(|| derive_active_job_id(state, &plan.thread_id))
        .flatten();
    let Ok(stop) = job_service::resolve_thread_stop_job(&plan, active_job_id) else {
        return Ok(unavailable_action(
            nexushub_core::services::commands::THREADS_STOP,
            "stop requires a running local fallback job; Codex app-server stop is not available in the native read model",
        ));
    };
    let cancelled = state.jobs.cancel_job(&stop.job_id)?;
    Ok(job_service::thread_stop_response(&stop, cancelled).into())
}

pub(crate) fn accept_plan_with_state(
    state: &DesktopState,
    request: DesktopPlanAcceptRequest,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    let _ = (request.turn_id, request.item_id);
    start_codex_resume_job(
        state,
        &request.thread_id,
        job_service::plan_accept_resume_message(),
    )
}

pub(crate) fn revise_plan_with_state(
    state: &DesktopState,
    request: DesktopPlanReviseRequest,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    let _ = (request.turn_id, request.item_id);
    let instructions = request.instructions.trim();
    if instructions.is_empty() {
        anyhow::bail!("revision instructions cannot be empty");
    }
    start_codex_resume_job(
        state,
        &request.thread_id,
        job_service::plan_revise_resume_message(instructions),
    )
}

pub(crate) fn answer_approval_with_state(
    request: DesktopApprovalAnswerRequest,
) -> Result<DesktopActionResponse> {
    let _ = request.payload;
    let mut response = unavailable_action(
        nexushub_core::services::commands::THREADS_APPROVAL_ANSWER,
        "approval actions are unavailable in the local Codex read model",
    );
    response.thread_id = Some(request.thread_id);
    Ok(response)
}

pub(crate) fn answer_elicitation_with_state(
    state: &DesktopState,
    request: DesktopElicitationAnswerRequest,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    let message = job_service::elicitation_answer_resume_message(&request.answers);
    if message.trim().is_empty() {
        anyhow::bail!("answers cannot be empty");
    }
    start_codex_resume_job(state, &request.thread_id, message)
}

pub(crate) fn archive_thread_with_state(
    state: &DesktopState,
    request: DesktopThreadIdRequest,
) -> Result<DesktopActionResponse> {
    let plan =
        job_service::plan_thread_archive_with_capability(state.platform(), &request.thread_id)?;
    set_thread_archived(&state.codex_paths(), &plan.thread_id, true)?;
    Ok(job_service::thread_state_action_response(&plan)?.into())
}

pub(crate) fn restore_thread_with_state(
    state: &DesktopState,
    request: DesktopThreadIdRequest,
) -> Result<DesktopActionResponse> {
    let plan =
        job_service::plan_thread_restore_with_capability(state.platform(), &request.thread_id)?;
    set_thread_archived(&state.codex_paths(), &plan.thread_id, false)?;
    Ok(job_service::thread_state_action_response(&plan)?.into())
}

pub(crate) fn rename_thread_with_state(
    state: &DesktopState,
    request: DesktopRenameThreadRequest,
) -> Result<DesktopActionResponse> {
    let plan = job_service::plan_thread_rename_with_capability(
        state.platform(),
        job_service::ThreadRenameRequest {
            thread_id: request.thread_id,
            name: request.name,
        },
    )?;
    let name = plan.name.as_deref().unwrap_or_default();
    set_thread_title(&state.codex_paths(), &plan.thread_id, name)?;
    job_service::thread_state_action_response(&plan).map(Into::into)
}

pub(crate) fn fork_thread_unavailable(request: DesktopThreadIdRequest) -> DesktopActionResponse {
    job_service::fork_thread_unavailable_response(Some(request.thread_id)).into()
}

pub(crate) fn list_followups_with_state(
    state: &DesktopState,
    request: DesktopFollowupRequest,
) -> Result<Vec<ThreadFollowUp>> {
    NexusHubUseCases::new(state.platform())
        .threads()
        .list_followups(
        &state.db,
        job_service::FollowUpListRequest {
            thread_id: request.thread_id,
            limit: request.limit,
        },
    )
}

pub(crate) fn enqueue_followup_with_state(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
) -> Result<ThreadFollowUp> {
    let attachments = prepare_request_attachments(state, &request.attachments)?;
    NexusHubUseCases::new(state.platform())
        .threads()
        .apply_enqueue_followup(
        &state.db,
        job_service::ThreadSteerRequest {
            thread_id: request.thread_id.clone(),
            message: request.into_thread_message(attachments),
        },
    )
}

pub(crate) fn cancel_followup_with_state(
    state: &DesktopState,
    request: DesktopCancelFollowupRequest,
) -> Result<DesktopActionResponse> {
    Ok(NexusHubUseCases::new(state.platform())
        .threads()
        .apply_cancel_followup(
        &state.db,
        job_service::FollowUpCancelRequest {
            thread_id: request.thread_id,
            followup_id: request.followup_id,
        },
    )?
    .into())
}

pub(crate) fn start_codex_resume_job(
    state: &DesktopState,
    thread_id: &str,
    message: String,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    start_codex_job_from_request(
        state,
        DesktopSendMessageRequest {
            thread_id: Some(thread_id.to_string()),
            message,
            ..DesktopSendMessageRequest::default()
        },
        job_service::CodexActionKind::Resume,
    )
}

pub(crate) fn start_codex_job_from_request(
    state: &DesktopState,
    request: DesktopSendMessageRequest,
    kind: job_service::CodexActionKind,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    let attachments = prepare_request_attachments(state, &request.attachments)?;
    let message = request.into_thread_message(attachments);
    let config = state.config();
    let plan = job_service::plan_thread_command_job_execution(
        state.platform(),
        job_service::ThreadCommandRequest {
            command: match kind {
                job_service::CodexActionKind::Exec => job_service::ThreadCommandKind::Create,
                job_service::CodexActionKind::Resume => job_service::ThreadCommandKind::Resume,
            },
            thread_id: message.thread_id.clone(),
            message,
        },
        config.codex.workspace.clone(),
    )?;
    start_codex_job_from_plan(state, plan)
}

pub(crate) fn start_codex_job_from_plan(
    state: &DesktopState,
    plan: job_service::ThreadCommandExecutionPlan,
) -> Result<nexushub_core::jobs::CodexActionResult> {
    let spec = &plan.spec;
    let resolved = state.resolved_codex_paths();
    let job_id = state.jobs.start_codex_job(
        &spec.title,
        &resolved.home,
        &spec.cwd,
        spec.args.clone(),
        spec.prompt.clone(),
    )?;
    state.db.link_job_thread(
        &job_id,
        plan.link.thread_id.as_deref(),
        plan.link.turn_id.as_deref(),
    )?;
    plan.submitted_response(&job_id)
}

pub(crate) fn derive_active_job_id(state: &DesktopState, thread_id: &str) -> Option<String> {
    state
        .db
        .running_job_for_thread(thread_id)
        .ok()
        .flatten()
        .map(|job| job.id)
}

fn load_thread_detail_read_model(
    state: &DesktopState,
    thread_id: &str,
) -> Result<Option<ThreadDetail>> {
    let paths = state.codex_paths();
    let Some(detail) = thread_detail(&paths, thread_id)? else {
        return Ok(None);
    };
    let active_job = active_job_for_thread(state, &detail.summary.id)?;
    let pending_followup = pending_followup_for_thread(state, &detail.summary.id)?;
    let view = thread_service::thread_detail_read_model(
        state.platform(),
        detail,
        active_job,
        pending_followup,
        state.config().codex.workspace.clone(),
    )?;
    execute_autosubmit_effects(state, view.autosubmit_effects)?;
    Ok(Some(view.detail))
}

fn active_job_for_thread(state: &DesktopState, thread_id: &str) -> Result<Option<JobRecord>> {
    state.db.running_job_for_thread(thread_id)
}

fn pending_followup_for_thread(
    state: &DesktopState,
    thread_id: &str,
) -> Result<Option<ThreadFollowUp>> {
    NexusHubUseCases::new(state.platform())
        .threads()
        .pending_followup(&state.db, thread_id)
}

fn execute_autosubmit_effects(
    state: &DesktopState,
    effects: Vec<job_service::FollowUpAutoSubmitExecutionPlan>,
) -> Result<()> {
    for plan in effects {
        let Some(claim) = plan.claim.as_ref() else {
            continue;
        };
        let use_cases = NexusHubUseCases::new(state.platform()).threads();
        let Some(followup) = use_cases.claim_next_followup(
            &state.db,
            job_service::FollowUpClaimRequest {
                thread_id: claim.thread_id.clone(),
            },
        )? else {
            continue;
        };
        let Some(job_plan) = plan.job.clone() else {
            continue;
        };
        match start_codex_job_from_plan(state, job_plan) {
            Ok(result) => {
                let Some(job_id) = result.job_id else {
                    continue;
                };
                let submit = plan.submitted_result(&job_id)?;
                let _ = use_cases.apply_followup_submitted(
                    &state.db,
                    job_service::FollowUpSubmitResultRequest {
                        followup_id: submit.followup_id,
                        result: submit.result,
                    },
                );
            }
            Err(err) => {
                let error = plan.error_result(&err.to_string()).unwrap_or_else(|_| {
                    job_service::FollowUpErrorPlan {
                        required_capability: plan.required_capability,
                        command: nexushub_core::services::commands::THREADS_FOLLOWUPS_ERROR
                            .to_string(),
                        followup_id: followup.id,
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
    Ok(())
}

pub(crate) fn unavailable_action(command: &str, message: &str) -> DesktopActionResponse {
    job_service::action_unavailable(command, message).into()
}
