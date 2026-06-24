use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    codex::ThreadStatus,
    db::{PanelDb, ThreadFollowUp},
    jobs::CodexActionResult,
    platform::PlatformPaths,
    services::commands,
    services::system::{require_capability, Capability},
    uploads::{prompt_with_attachment_context, PreparedAttachment},
};

pub const CODEX_SUBMITTED_MESSAGE: &str = "已提交给 Codex";
pub const DEFAULT_ATTACHMENT_MESSAGE: &str = "请根据以下附件内容继续处理。";
pub const FOLLOW_UP_REQUIRED_MESSAGE: &str = "follow-up message is required";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CodexActionKind {
    #[default]
    Exec,
    Resume,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadActionRequest {
    pub message: String,
    #[serde(default)]
    pub attachments: Vec<String>,
    pub model: Option<String>,
    #[serde(default, alias = "serviceTier")]
    pub service_tier: Option<String>,
    #[serde(default, alias = "reasoningEffort")]
    pub reasoning_effort: Option<String>,
    pub cwd: Option<PathBuf>,
    #[serde(default, alias = "permissionProfile")]
    pub permission_profile: Option<String>,
    #[serde(default, alias = "approvalPolicy")]
    pub approval_policy: Option<String>,
    #[serde(default, alias = "sandboxMode")]
    pub sandbox_mode: Option<String>,
    #[serde(default, alias = "networkAccess")]
    pub network_access: Option<bool>,
    #[serde(default, alias = "collaborationMode")]
    pub collaboration_mode: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobActionRequest {
    pub kind: CodexActionKind,
    pub thread_id: Option<String>,
    pub message: String,
    pub cwd: Option<PathBuf>,
    pub model: Option<String>,
    #[serde(default, alias = "serviceTier")]
    pub service_tier: Option<String>,
    #[serde(default, alias = "reasoningEffort")]
    pub reasoning_effort: Option<String>,
    #[serde(default, alias = "permissionProfile")]
    pub permission_profile: Option<String>,
    #[serde(default, alias = "approvalPolicy")]
    pub approval_policy: Option<String>,
    #[serde(default, alias = "sandboxMode")]
    pub sandbox_mode: Option<String>,
    #[serde(default, alias = "networkAccess")]
    pub network_access: Option<bool>,
    #[serde(default, alias = "collaborationMode")]
    pub collaboration_mode: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadMessageRequest {
    #[serde(default, alias = "thread_id")]
    pub thread_id: Option<String>,
    pub message: String,
    #[serde(default)]
    pub attachments: Vec<String>,
    #[serde(default, alias = "preparedAttachments", alias = "prepared_attachments")]
    pub prepared_attachments: Vec<PreparedAttachment>,
    pub model: Option<String>,
    #[serde(default, alias = "serviceTier", alias = "service_tier")]
    pub service_tier: Option<String>,
    #[serde(default, alias = "reasoningEffort", alias = "reasoning_effort")]
    pub reasoning_effort: Option<String>,
    pub cwd: Option<String>,
    #[serde(default, alias = "permissionProfile", alias = "permission_profile")]
    pub permission_profile: Option<String>,
    #[serde(default, alias = "approvalPolicy", alias = "approval_policy")]
    pub approval_policy: Option<String>,
    #[serde(default, alias = "sandboxMode", alias = "sandbox_mode")]
    pub sandbox_mode: Option<String>,
    #[serde(default, alias = "networkAccess", alias = "network_access")]
    pub network_access: Option<bool>,
    #[serde(default, alias = "collaborationMode", alias = "collaboration_mode")]
    pub collaboration_mode: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThreadCommandKind {
    #[default]
    Create,
    Resume,
    #[serde(alias = "followup", alias = "steer")]
    FollowUp,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCommandRequest {
    pub command: ThreadCommandKind,
    #[serde(default, alias = "thread_id")]
    pub thread_id: Option<String>,
    pub message: ThreadMessageRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FollowUpView {
    pub id: String,
    pub thread_id: String,
    pub status: String,
    pub message: String,
    pub options: Value,
    pub created_at: i64,
    pub updated_at: i64,
    pub submitted_at: Option<i64>,
    pub cancelled_at: Option<i64>,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionResponse {
    pub ok: bool,
    pub available: bool,
    pub command: String,
    pub message: String,
    pub thread_id: Option<String>,
    pub job_id: Option<String>,
    pub data: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadFollowUpPlan {
    pub thread_id: String,
    pub message: String,
    pub options: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCommandPlan {
    pub command: ThreadCommandKind,
    pub thread_id: Option<String>,
    pub action: Option<JobActionRequest>,
    pub followup: Option<ThreadFollowUpPlan>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCommandFacadePlan {
    pub required_capability: Capability,
    pub command: ThreadCommandPlan,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSendRequest {
    #[serde(default, alias = "thread_id")]
    pub thread_id: Option<String>,
    pub message: ThreadMessageRequest,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSteerRequest {
    #[serde(default, alias = "thread_id")]
    pub thread_id: Option<String>,
    pub message: ThreadMessageRequest,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpListRequest {
    #[serde(alias = "thread_id")]
    pub thread_id: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpListPlan {
    pub required_capability: Capability,
    pub thread_id: String,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpEnqueueFacadePlan {
    pub required_capability: Capability,
    pub followup: ThreadFollowUpPlan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpClaimRequest {
    #[serde(alias = "thread_id")]
    pub thread_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpClaimPlan {
    pub required_capability: Capability,
    pub command: String,
    pub thread_id: String,
    pub from_status: String,
    pub to_status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpSubmitPlan {
    pub required_capability: Capability,
    pub command: String,
    pub followup_id: String,
    pub thread_id: String,
    pub action: JobActionRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpSubmitResultRequest {
    #[serde(alias = "followup_id", alias = "followUpId")]
    pub followup_id: String,
    pub result: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpSubmitResultPlan {
    pub required_capability: Capability,
    pub command: String,
    pub followup_id: String,
    pub result: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpErrorRequest {
    #[serde(alias = "followup_id", alias = "followUpId")]
    pub followup_id: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpErrorPlan {
    pub required_capability: Capability,
    pub command: String,
    pub followup_id: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpCancelRequest {
    #[serde(alias = "thread_id")]
    pub thread_id: String,
    #[serde(alias = "followup_id", alias = "followUpId")]
    pub followup_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpCancelPlan {
    pub required_capability: Capability,
    pub thread_id: String,
    pub followup_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FollowUpTransitionKind {
    Claim,
    Submitted { result: Value },
    Error { error: String },
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpTransitionPlan {
    pub followup_id: String,
    pub from_status: Option<String>,
    pub to_status: String,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpAutoSubmitPlan {
    pub should_claim_pending: bool,
    pub should_start_resume_job: bool,
    pub skip_reason: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStopRequest {
    #[serde(alias = "thread_id")]
    pub thread_id: String,
    #[serde(default, alias = "turn_id")]
    pub turn_id: Option<String>,
    #[serde(default, alias = "job_id")]
    pub job_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStopPlan {
    pub required_capability: Capability,
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub job_id: Option<String>,
    pub requires_active_job_lookup: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStopJobPlan {
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub job_id: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRenameRequest {
    #[serde(alias = "thread_id")]
    pub thread_id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStateActionPlan {
    pub required_capability: Capability,
    pub command: String,
    pub thread_id: String,
    pub archived: Option<bool>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexJobSpec {
    pub title: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub prompt: String,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobThreadLinkPlan {
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResponsePlan {
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditMetadataPlan {
    pub action: String,
    pub target_type: String,
    pub target_id: Option<String>,
    pub detail: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCommandExecutionPlan {
    pub required_capability: Capability,
    pub command: String,
    pub spec: CodexJobSpec,
    pub link: JobThreadLinkPlan,
    pub response: ActionResponsePlan,
    pub audit: AuditMetadataPlan,
}

impl ThreadCommandExecutionPlan {
    pub fn submitted_response(&self, job_id: &str) -> Result<CodexActionResult> {
        Ok(codex_action_submitted(
            self.response.thread_id.clone(),
            Some(required_job_id(job_id)?),
        ))
    }

    pub fn audit_detail(&self, job_id: &str) -> Result<Value> {
        let mut detail = self.audit.detail.clone();
        let job_id = required_job_id(job_id)?;
        if let Value::Object(ref mut object) = detail {
            object.insert("job_id".to_string(), json!(job_id));
            Ok(detail)
        } else {
            Ok(json!({"job_id": job_id}))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpAutoSubmitExecutionPlan {
    pub required_capability: Capability,
    pub autosubmit: FollowUpAutoSubmitPlan,
    pub claim: Option<FollowUpClaimPlan>,
    pub job: Option<ThreadCommandExecutionPlan>,
    pub followup_id: Option<String>,
}

impl FollowUpAutoSubmitExecutionPlan {
    pub fn submitted_result(&self, job_id: &str) -> Result<FollowUpSubmitResultPlan> {
        let followup_id = self
            .followup_id
            .as_deref()
            .ok_or_else(|| anyhow!("followup_id is required"))?;
        plan_followup_submitted_result(followup_id, json!({"job_id": required_job_id(job_id)?}))
    }

    pub fn submitted_response(&self, job_id: &str) -> Result<ActionResponse> {
        Ok(followup_submitted_response(&self.submitted_result(job_id)?))
    }

    pub fn error_result(&self, error: &str) -> Result<FollowUpErrorPlan> {
        let followup_id = self
            .followup_id
            .as_deref()
            .ok_or_else(|| anyhow!("followup_id is required"))?;
        plan_followup_error_result(followup_id, error)
    }

    pub fn error_response(&self, error: &str) -> Result<ActionResponse> {
        Ok(followup_error_response(&self.error_result(error)?))
    }
}

impl JobActionRequest {
    pub fn exec(message: impl Into<String>) -> Self {
        Self {
            kind: CodexActionKind::Exec,
            message: message.into(),
            ..Self::default()
        }
    }

    pub fn resume(thread_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: CodexActionKind::Resume,
            thread_id: Some(thread_id.into()),
            message: message.into(),
            ..Self::default()
        }
    }
}

impl From<ThreadActionRequest> for JobActionRequest {
    fn from(request: ThreadActionRequest) -> Self {
        Self {
            kind: CodexActionKind::Exec,
            thread_id: None,
            message: request.message,
            cwd: request.cwd,
            model: request.model,
            service_tier: request.service_tier,
            reasoning_effort: request.reasoning_effort,
            permission_profile: request.permission_profile,
            approval_policy: request.approval_policy,
            sandbox_mode: request.sandbox_mode,
            network_access: request.network_access,
            collaboration_mode: request.collaboration_mode,
        }
    }
}

impl ThreadMessageRequest {
    pub fn effective_message(&self) -> String {
        effective_message(&self.message, &self.prepared_attachments)
    }

    pub fn prompt_with_attachment_context(&self) -> String {
        prompt_with_attachment_context(&self.effective_message(), &self.prepared_attachments)
    }

    pub fn options_json(&self) -> Value {
        thread_message_options_json(self)
    }

    pub fn into_job_action(self, kind: CodexActionKind) -> JobActionRequest {
        let message = self.prompt_with_attachment_context();
        JobActionRequest {
            kind,
            thread_id: self.thread_id,
            message,
            cwd: self
                .cwd
                .and_then(|value| non_empty_owned(&value))
                .map(PathBuf::from),
            model: self.model,
            service_tier: self.service_tier,
            reasoning_effort: self.reasoning_effort,
            permission_profile: self.permission_profile,
            approval_policy: self.approval_policy,
            sandbox_mode: self.sandbox_mode,
            network_access: self.network_access,
            collaboration_mode: self.collaboration_mode,
        }
    }

    pub fn into_followup_message_and_options(self) -> Result<(String, Value)> {
        let message = self.effective_message();
        if message.is_empty() {
            return Err(anyhow!(FOLLOW_UP_REQUIRED_MESSAGE));
        }
        Ok((message, self.options_json()))
    }
}

pub fn normalize_thread_command_request(
    request: ThreadCommandRequest,
) -> Result<ThreadCommandPlan> {
    match request.command {
        ThreadCommandKind::Create => {
            let mut message = request.message;
            message.thread_id = None;
            let action = message.into_job_action(CodexActionKind::Exec);
            Ok(ThreadCommandPlan {
                command: ThreadCommandKind::Create,
                thread_id: None,
                action: Some(action),
                followup: None,
            })
        }
        ThreadCommandKind::Resume => {
            let thread_id = required_command_thread_id(
                request.thread_id.as_deref(),
                request.message.thread_id.as_deref(),
            )?;
            let mut message = request.message;
            message.thread_id = Some(thread_id.clone());
            let action = message.into_job_action(CodexActionKind::Resume);
            Ok(ThreadCommandPlan {
                command: ThreadCommandKind::Resume,
                thread_id: Some(thread_id),
                action: Some(action),
                followup: None,
            })
        }
        ThreadCommandKind::FollowUp => plan_steer_thread_as_followup(request),
    }
}

pub fn plan_thread_command_with_capability(
    platform: &PlatformPaths,
    request: ThreadCommandRequest,
) -> Result<ThreadCommandFacadePlan> {
    require_capability(platform, Capability::Jobs)?;
    Ok(ThreadCommandFacadePlan {
        required_capability: Capability::Jobs,
        command: normalize_thread_command_request(request)?,
    })
}

pub fn plan_thread_send_with_capability(
    platform: &PlatformPaths,
    request: ThreadSendRequest,
) -> Result<ThreadCommandFacadePlan> {
    require_capability(platform, Capability::Jobs)?;
    let command = if non_empty(request.thread_id.as_deref())
        .or_else(|| non_empty(request.message.thread_id.as_deref()))
        .is_some()
    {
        ThreadCommandKind::Resume
    } else {
        ThreadCommandKind::Create
    };
    Ok(ThreadCommandFacadePlan {
        required_capability: Capability::Jobs,
        command: normalize_thread_command_request(ThreadCommandRequest {
            command,
            thread_id: request.thread_id,
            message: request.message,
        })?,
    })
}

pub fn plan_thread_steer_with_capability(
    platform: &PlatformPaths,
    request: ThreadSteerRequest,
) -> Result<ThreadCommandFacadePlan> {
    require_capability(platform, Capability::Jobs)?;
    Ok(ThreadCommandFacadePlan {
        required_capability: Capability::Jobs,
        command: plan_steer_thread_as_followup(ThreadCommandRequest {
            command: ThreadCommandKind::FollowUp,
            thread_id: request.thread_id,
            message: request.message,
        })?,
    })
}

pub fn plan_followup_list_with_capability(
    platform: &PlatformPaths,
    request: FollowUpListRequest,
) -> Result<FollowUpListPlan> {
    require_capability(platform, Capability::Jobs)?;
    Ok(FollowUpListPlan {
        required_capability: Capability::Jobs,
        thread_id: required_command_thread_id(Some(&request.thread_id), None)?,
        limit: normalize_followup_limit(request.limit),
    })
}

pub fn plan_followup_enqueue_with_capability(
    platform: &PlatformPaths,
    request: ThreadSteerRequest,
) -> Result<FollowUpEnqueueFacadePlan> {
    let plan = plan_thread_steer_with_capability(platform, request)?;
    let followup = plan
        .command
        .followup
        .ok_or_else(|| anyhow!("missing follow-up plan"))?;
    Ok(FollowUpEnqueueFacadePlan {
        required_capability: Capability::Jobs,
        followup,
    })
}

pub fn plan_followup_claim_with_capability(
    platform: &PlatformPaths,
    request: FollowUpClaimRequest,
) -> Result<FollowUpClaimPlan> {
    require_capability(platform, Capability::Jobs)?;
    Ok(FollowUpClaimPlan {
        required_capability: Capability::Jobs,
        command: commands::THREADS_FOLLOWUPS_CLAIM.to_string(),
        thread_id: required_command_thread_id(Some(&request.thread_id), None)?,
        from_status: "pending".to_string(),
        to_status: "submitting".to_string(),
    })
}

pub fn plan_followup_submit_with_capability(
    platform: &PlatformPaths,
    followup: &ThreadFollowUp,
) -> Result<FollowUpSubmitPlan> {
    require_capability(platform, Capability::Jobs)?;
    if followup.status != "submitting" {
        return Err(anyhow!("follow-up must be claimed before submit"));
    }
    let followup_id =
        non_empty_owned(&followup.id).ok_or_else(|| anyhow!("followup_id is required"))?;
    let thread_id = required_command_thread_id(Some(&followup.thread_id), None)?;
    let mut request = followup_request(followup);
    request.thread_id = Some(thread_id.clone());
    Ok(FollowUpSubmitPlan {
        required_capability: Capability::Jobs,
        command: commands::THREADS_FOLLOWUPS_SUBMIT.to_string(),
        followup_id,
        thread_id,
        action: request.into_job_action(CodexActionKind::Resume),
    })
}

pub fn plan_followup_submitted_with_capability(
    platform: &PlatformPaths,
    request: FollowUpSubmitResultRequest,
) -> Result<FollowUpSubmitResultPlan> {
    require_capability(platform, Capability::Jobs)?;
    let followup_id =
        non_empty_owned(&request.followup_id).ok_or_else(|| anyhow!("followup_id is required"))?;
    Ok(FollowUpSubmitResultPlan {
        required_capability: Capability::Jobs,
        command: commands::THREADS_FOLLOWUPS_SUBMIT.to_string(),
        followup_id,
        result: request.result,
    })
}

pub fn plan_followup_error_with_capability(
    platform: &PlatformPaths,
    request: FollowUpErrorRequest,
) -> Result<FollowUpErrorPlan> {
    require_capability(platform, Capability::Jobs)?;
    let followup_id =
        non_empty_owned(&request.followup_id).ok_or_else(|| anyhow!("followup_id is required"))?;
    let error = non_empty_owned(&request.error).ok_or_else(|| anyhow!("error is required"))?;
    Ok(FollowUpErrorPlan {
        required_capability: Capability::Jobs,
        command: commands::THREADS_FOLLOWUPS_ERROR.to_string(),
        followup_id,
        error,
    })
}

pub fn plan_followup_cancel_with_capability(
    platform: &PlatformPaths,
    request: FollowUpCancelRequest,
) -> Result<FollowUpCancelPlan> {
    require_capability(platform, Capability::Jobs)?;
    let followup_id =
        non_empty_owned(&request.followup_id).ok_or_else(|| anyhow!("followup_id is required"))?;
    Ok(FollowUpCancelPlan {
        required_capability: Capability::Jobs,
        thread_id: required_command_thread_id(Some(&request.thread_id), None)?,
        followup_id,
    })
}

pub fn plan_followup_status_transition(
    followup_id: &str,
    kind: FollowUpTransitionKind,
) -> Result<FollowUpTransitionPlan> {
    let followup_id =
        non_empty_owned(followup_id).ok_or_else(|| anyhow!("followup_id is required"))?;
    let plan = match kind {
        FollowUpTransitionKind::Claim => FollowUpTransitionPlan {
            followup_id,
            from_status: Some("pending".to_string()),
            to_status: "submitting".to_string(),
            result: None,
            error: None,
        },
        FollowUpTransitionKind::Submitted { result } => FollowUpTransitionPlan {
            followup_id,
            from_status: Some("submitting".to_string()),
            to_status: "submitted".to_string(),
            result: Some(result),
            error: None,
        },
        FollowUpTransitionKind::Error { error } => FollowUpTransitionPlan {
            followup_id,
            from_status: Some("submitting".to_string()),
            to_status: "error".to_string(),
            result: None,
            error: Some(non_empty_owned(&error).ok_or_else(|| anyhow!("error is required"))?),
        },
        FollowUpTransitionKind::Cancel => FollowUpTransitionPlan {
            followup_id,
            from_status: Some("pending".to_string()),
            to_status: "cancelled".to_string(),
            result: None,
            error: None,
        },
    };
    Ok(plan)
}

pub fn plan_followup_autosubmit(
    thread_status: ThreadStatus,
    has_pending_followup: bool,
) -> FollowUpAutoSubmitPlan {
    if !matches!(thread_status, ThreadStatus::Recent) {
        return FollowUpAutoSubmitPlan {
            should_claim_pending: false,
            should_start_resume_job: false,
            skip_reason: Some("thread is not idle".to_string()),
        };
    }
    if !has_pending_followup {
        return FollowUpAutoSubmitPlan {
            should_claim_pending: false,
            should_start_resume_job: false,
            skip_reason: Some("no pending follow-up".to_string()),
        };
    }
    FollowUpAutoSubmitPlan {
        should_claim_pending: true,
        should_start_resume_job: true,
        skip_reason: None,
    }
}

pub fn plan_queued_followup_job_spec(
    followup: &ThreadFollowUp,
    default_workspace: PathBuf,
) -> Result<CodexJobSpec> {
    let mut request = followup_request(followup);
    request.thread_id = Some(required_command_thread_id(Some(&followup.thread_id), None)?);
    let mut spec = build_codex_job_spec(
        &request.into_job_action(CodexActionKind::Resume),
        default_workspace,
    )?;
    spec.title = "Codex queued follow-up".to_string();
    Ok(spec)
}

pub fn plan_thread_command_job_execution(
    platform: &PlatformPaths,
    request: ThreadCommandRequest,
    default_workspace: PathBuf,
) -> Result<ThreadCommandExecutionPlan> {
    let facade = plan_thread_command_with_capability(platform, request)?;
    thread_command_execution_plan_from_facade(facade, default_workspace)
}

pub fn plan_thread_send_job_execution(
    platform: &PlatformPaths,
    request: ThreadSendRequest,
    default_workspace: PathBuf,
) -> Result<ThreadCommandExecutionPlan> {
    let facade = plan_thread_send_with_capability(platform, request)?;
    thread_command_execution_plan_from_facade(facade, default_workspace)
}

pub fn plan_followup_autosubmit_execution(
    platform: &PlatformPaths,
    thread_status: ThreadStatus,
    followup: &ThreadFollowUp,
    default_workspace: PathBuf,
) -> Result<FollowUpAutoSubmitExecutionPlan> {
    require_capability(platform, Capability::Jobs)?;
    let thread_id = required_command_thread_id(Some(&followup.thread_id), None)?;
    let autosubmit = plan_followup_autosubmit(thread_status, true);
    if !autosubmit.should_claim_pending || !autosubmit.should_start_resume_job {
        return Ok(FollowUpAutoSubmitExecutionPlan {
            required_capability: Capability::Jobs,
            autosubmit,
            claim: None,
            job: None,
            followup_id: None,
        });
    }
    let claim = plan_followup_claim_with_capability(
        platform,
        FollowUpClaimRequest {
            thread_id: thread_id.clone(),
        },
    )?;
    let spec = plan_queued_followup_job_spec(followup, default_workspace)?;
    let followup_id =
        non_empty_owned(&followup.id).ok_or_else(|| anyhow!("followup_id is required"))?;
    Ok(FollowUpAutoSubmitExecutionPlan {
        required_capability: Capability::Jobs,
        autosubmit,
        claim: Some(claim),
        job: Some(ThreadCommandExecutionPlan {
            required_capability: Capability::Jobs,
            command: commands::THREADS_FOLLOWUPS_SUBMIT.to_string(),
            spec,
            link: JobThreadLinkPlan {
                thread_id: Some(thread_id.clone()),
                turn_id: None,
            },
            response: ActionResponsePlan {
                thread_id: Some(thread_id.clone()),
            },
            audit: AuditMetadataPlan {
                action: "thread.followup.autosubmit_job_started".to_string(),
                target_type: "thread".to_string(),
                target_id: Some(thread_id),
                detail: json!({
                    "followup_id": followup_id.clone(),
                    "job_fallback": true,
                }),
            },
        }),
        followup_id: Some(followup_id),
    })
}

pub fn plan_thread_stop_with_capability(
    platform: &PlatformPaths,
    request: ThreadStopRequest,
) -> Result<ThreadStopPlan> {
    require_capability(platform, Capability::Jobs)?;
    let job_id = non_empty(request.job_id.as_deref()).map(str::to_string);
    Ok(ThreadStopPlan {
        required_capability: Capability::Jobs,
        thread_id: required_command_thread_id(Some(&request.thread_id), None)?,
        turn_id: non_empty(request.turn_id.as_deref()).map(str::to_string),
        requires_active_job_lookup: job_id.is_none(),
        job_id,
    })
}

pub fn resolve_thread_stop_job(
    plan: &ThreadStopPlan,
    active_job_id: Option<String>,
) -> Result<ThreadStopJobPlan> {
    let job_id = plan
        .job_id
        .as_deref()
        .and_then(|value| non_empty(Some(value)))
        .map(str::to_string)
        .or_else(|| non_empty(active_job_id.as_deref()).map(str::to_string))
        .ok_or_else(|| anyhow!("stop requires job_id or an active fallback job"))?;
    Ok(ThreadStopJobPlan {
        thread_id: plan.thread_id.clone(),
        turn_id: plan.turn_id.clone(),
        job_id,
    })
}

pub fn thread_stop_response(plan: &ThreadStopJobPlan, cancelled: bool) -> ActionResponse {
    action_ok(
        commands::THREADS_STOP,
        if cancelled {
            "sent TERM to local Codex job"
        } else {
            "local job is no longer running"
        },
        Some(plan.thread_id.clone()),
        Some(plan.job_id.clone()),
        Some(json!({"turn_id": plan.turn_id, "cancelled": cancelled})),
    )
}

pub fn plan_thread_archive_with_capability(
    platform: &PlatformPaths,
    thread_id: &str,
) -> Result<ThreadStateActionPlan> {
    plan_thread_archive_state_action(platform, thread_id, true)
}

pub fn plan_thread_restore_with_capability(
    platform: &PlatformPaths,
    thread_id: &str,
) -> Result<ThreadStateActionPlan> {
    plan_thread_archive_state_action(platform, thread_id, false)
}

pub fn plan_thread_rename_with_capability(
    platform: &PlatformPaths,
    request: ThreadRenameRequest,
) -> Result<ThreadStateActionPlan> {
    require_capability(platform, Capability::ThreadArchiveActions)?;
    let name = non_empty_owned(&request.name).ok_or_else(|| anyhow!("name cannot be empty"))?;
    Ok(ThreadStateActionPlan {
        required_capability: Capability::ThreadArchiveActions,
        command: commands::THREADS_RENAME.to_string(),
        thread_id: required_command_thread_id(Some(&request.thread_id), None)?,
        archived: None,
        name: Some(name),
    })
}

pub fn plan_steer_thread_as_followup(request: ThreadCommandRequest) -> Result<ThreadCommandPlan> {
    let thread_id = required_command_thread_id(
        request.thread_id.as_deref(),
        request.message.thread_id.as_deref(),
    )?;
    let message = request.message;
    let (followup_message, options) = message.into_followup_message_and_options()?;
    Ok(ThreadCommandPlan {
        command: ThreadCommandKind::FollowUp,
        thread_id: Some(thread_id.clone()),
        action: None,
        followup: Some(ThreadFollowUpPlan {
            thread_id,
            message: followup_message,
            options,
        }),
    })
}

pub fn build_codex_job_spec(
    request: &JobActionRequest,
    default_workspace: PathBuf,
) -> Result<CodexJobSpec> {
    let prompt = request.message.trim().to_string();
    if prompt.is_empty() {
        return Err(anyhow!("message is required"));
    }

    let cwd = request
        .cwd
        .as_ref()
        .filter(|path| !path.as_os_str().is_empty())
        .cloned()
        .unwrap_or(default_workspace);
    let (title, thread_id, mut args) = match request.kind {
        CodexActionKind::Exec => (
            "Codex new thread".to_string(),
            None,
            vec![
                "exec".to_string(),
                "--json".to_string(),
                "--skip-git-repo-check".to_string(),
                "-".to_string(),
            ],
        ),
        CodexActionKind::Resume => {
            let thread_id = non_empty(request.thread_id.as_deref())
                .ok_or_else(|| anyhow!("thread_id is required"))?
                .to_string();
            (
                "Codex resume thread".to_string(),
                Some(thread_id.clone()),
                codex_resume_args(&thread_id),
            )
        }
    };
    add_codex_common_args(&mut args, request);

    Ok(CodexJobSpec {
        title,
        args,
        cwd,
        prompt,
        thread_id,
    })
}

fn thread_command_execution_plan_from_facade(
    facade: ThreadCommandFacadePlan,
    default_workspace: PathBuf,
) -> Result<ThreadCommandExecutionPlan> {
    let command = facade.command;
    let action = command
        .action
        .ok_or_else(|| anyhow!("thread command plan is missing Codex job action"))?;
    let spec = build_codex_job_spec(&action, default_workspace)?;
    let command_name = thread_command_rpc_name(command.command);
    let thread_id = spec.thread_id.clone();
    let audit = thread_command_audit_plan(command.command, thread_id.clone(), &spec);
    Ok(ThreadCommandExecutionPlan {
        required_capability: facade.required_capability,
        command: command_name.to_string(),
        link: JobThreadLinkPlan {
            thread_id: thread_id.clone(),
            turn_id: None,
        },
        response: ActionResponsePlan { thread_id },
        spec,
        audit,
    })
}

fn thread_command_rpc_name(command: ThreadCommandKind) -> &'static str {
    match command {
        ThreadCommandKind::Create => commands::THREADS_CREATE,
        ThreadCommandKind::Resume => commands::THREADS_SEND,
        ThreadCommandKind::FollowUp => commands::THREADS_STEER,
    }
}

fn thread_command_audit_plan(
    command: ThreadCommandKind,
    thread_id: Option<String>,
    spec: &CodexJobSpec,
) -> AuditMetadataPlan {
    match command {
        ThreadCommandKind::Create => AuditMetadataPlan {
            action: "thread.create.job_started".to_string(),
            target_type: "job".to_string(),
            target_id: None,
            detail: json!({"cwd": spec.cwd.display().to_string()}),
        },
        ThreadCommandKind::Resume => AuditMetadataPlan {
            action: "thread.message.job_started".to_string(),
            target_type: "thread".to_string(),
            target_id: thread_id,
            detail: json!({}),
        },
        ThreadCommandKind::FollowUp => AuditMetadataPlan {
            action: "thread.followup.job_started".to_string(),
            target_type: "thread".to_string(),
            target_id: thread_id,
            detail: json!({}),
        },
    }
}

pub fn effective_message(message: &str, attachments: &[PreparedAttachment]) -> String {
    let message = message.trim();
    if message.is_empty() && !attachments.is_empty() {
        DEFAULT_ATTACHMENT_MESSAGE.to_string()
    } else {
        message.to_string()
    }
}

pub fn codex_action_submitted(
    thread_id: Option<String>,
    job_id: Option<String>,
) -> CodexActionResult {
    CodexActionResult {
        bridge: false,
        thread_id,
        turn_id: None,
        job_id,
        fallback: true,
        message: Some(CODEX_SUBMITTED_MESSAGE.to_string()),
    }
}

pub fn action_ok(
    command: &str,
    message: &str,
    thread_id: Option<String>,
    job_id: Option<String>,
    data: Option<Value>,
) -> ActionResponse {
    ActionResponse {
        ok: true,
        available: true,
        command: command.to_string(),
        message: message.to_string(),
        thread_id,
        job_id,
        data,
    }
}

pub fn action_unavailable(command: &str, message: &str) -> ActionResponse {
    ActionResponse {
        ok: false,
        available: false,
        command: command.to_string(),
        message: message.to_string(),
        thread_id: None,
        job_id: None,
        data: None,
    }
}

pub fn fork_thread_unavailable_response(thread_id: Option<String>) -> ActionResponse {
    let mut response = action_unavailable(
        commands::THREADS_FORK,
        "fork is unavailable in the local Codex read model",
    );
    response.thread_id = thread_id.and_then(|value| non_empty_owned(&value));
    response
}

pub fn archive_thread_response(thread_id: String, archived: bool) -> ActionResponse {
    let (command, message) = if archived {
        (
            commands::THREADS_ARCHIVE,
            "thread archived in local Codex state",
        )
    } else {
        (
            commands::THREADS_RESTORE,
            "thread restored in local Codex state",
        )
    };
    action_ok(command, message, Some(thread_id), None, None)
}

pub fn rename_thread_response(thread_id: String, name: &str) -> Result<ActionResponse> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("name cannot be empty"));
    }
    Ok(action_ok(
        commands::THREADS_RENAME,
        "thread renamed in local Codex state",
        Some(thread_id),
        None,
        Some(json!({"name": name})),
    ))
}

pub fn thread_state_action_response(plan: &ThreadStateActionPlan) -> Result<ActionResponse> {
    match plan.command.as_str() {
        commands::THREADS_ARCHIVE => Ok(archive_thread_response(plan.thread_id.clone(), true)),
        commands::THREADS_RESTORE => Ok(archive_thread_response(plan.thread_id.clone(), false)),
        commands::THREADS_RENAME => rename_thread_response(
            plan.thread_id.clone(),
            plan.name.as_deref().unwrap_or_default(),
        ),
        _ => Err(anyhow!("unsupported thread state action: {}", plan.command)),
    }
}

pub fn cancel_followup_response(
    command: &str,
    thread_id: String,
    followup_id: String,
    cancelled: bool,
) -> ActionResponse {
    action_ok(
        command,
        if cancelled {
            "follow-up cancelled"
        } else {
            "follow-up was not pending"
        },
        Some(thread_id),
        None,
        Some(json!({"followup_id": followup_id, "cancelled": cancelled})),
    )
}

pub fn list_followups_with_capability(
    db: &PanelDb,
    platform: &PlatformPaths,
    request: FollowUpListRequest,
) -> Result<Vec<ThreadFollowUp>> {
    let plan = plan_followup_list_with_capability(platform, request)?;
    db.list_followups(&plan.thread_id, plan.limit as u32)
}

pub fn enqueue_followup_with_capability(
    db: &PanelDb,
    platform: &PlatformPaths,
    request: ThreadSteerRequest,
) -> Result<ThreadFollowUp> {
    let plan = plan_followup_enqueue_with_capability(platform, request)?;
    enqueue_planned_followup(db, plan.followup)
}

pub fn enqueue_planned_followup(
    db: &PanelDb,
    followup: ThreadFollowUpPlan,
) -> Result<ThreadFollowUp> {
    db.enqueue_followup(&followup.thread_id, &followup.message, followup.options)
}

pub fn claim_next_followup_with_capability(
    db: &PanelDb,
    platform: &PlatformPaths,
    request: FollowUpClaimRequest,
) -> Result<Option<ThreadFollowUp>> {
    let plan = plan_followup_claim_with_capability(platform, request)?;
    db.claim_next_pending_followup(&plan.thread_id)
}

pub fn mark_followup_submitted_with_capability(
    db: &PanelDb,
    platform: &PlatformPaths,
    request: FollowUpSubmitResultRequest,
) -> Result<ActionResponse> {
    let plan = plan_followup_submitted_with_capability(platform, request)?;
    db.mark_followup_submitted(&plan.followup_id, plan.result.clone())?;
    Ok(followup_submitted_response(&plan))
}

pub fn mark_followup_error_with_capability(
    db: &PanelDb,
    platform: &PlatformPaths,
    request: FollowUpErrorRequest,
) -> Result<ActionResponse> {
    let plan = plan_followup_error_with_capability(platform, request)?;
    db.mark_followup_error(&plan.followup_id, &plan.error)?;
    Ok(followup_error_response(&plan))
}

pub fn cancel_followup_with_capability(
    db: &PanelDb,
    platform: &PlatformPaths,
    request: FollowUpCancelRequest,
) -> Result<ActionResponse> {
    let plan = plan_followup_cancel_with_capability(platform, request)?;
    let cancelled = db.cancel_followup(&plan.thread_id, &plan.followup_id)?;
    Ok(cancel_followup_response(
        commands::THREADS_FOLLOWUPS_CANCEL,
        plan.thread_id,
        plan.followup_id,
        cancelled,
    ))
}

pub fn followup_submitted_response(plan: &FollowUpSubmitResultPlan) -> ActionResponse {
    action_ok(
        &plan.command,
        "follow-up submitted",
        None,
        None,
        Some(json!({"followup_id": plan.followup_id, "result": plan.result})),
    )
}

pub fn followup_error_response(plan: &FollowUpErrorPlan) -> ActionResponse {
    action_ok(
        &plan.command,
        "follow-up marked as failed",
        None,
        None,
        Some(json!({"followup_id": plan.followup_id, "error": plan.error})),
    )
}

pub fn thread_message_options_json(request: &ThreadMessageRequest) -> Value {
    json!({
        "model": &request.model,
        "service_tier": &request.service_tier,
        "reasoning_effort": &request.reasoning_effort,
        "cwd": &request.cwd,
        "permission_profile": &request.permission_profile,
        "approval_policy": &request.approval_policy,
        "sandbox_mode": &request.sandbox_mode,
        "network_access": request.network_access,
        "collaboration_mode": &request.collaboration_mode,
        "attachments": &request.attachments,
        "prepared_attachments": &request.prepared_attachments,
    })
}

pub fn followup_request(followup: &ThreadFollowUp) -> ThreadMessageRequest {
    let options = serde_json::from_str::<Value>(&followup.options_json).unwrap_or(Value::Null);
    ThreadMessageRequest {
        message: followup.message.clone(),
        attachments: string_array_option(&options, "attachments"),
        prepared_attachments: options
            .get("prepared_attachments")
            .cloned()
            .and_then(|value| serde_json::from_value::<Vec<PreparedAttachment>>(value).ok())
            .unwrap_or_default(),
        model: string_option(&options, "model"),
        service_tier: string_option(&options, "service_tier")
            .or_else(|| string_option(&options, "serviceTier")),
        reasoning_effort: string_option(&options, "reasoning_effort")
            .or_else(|| string_option(&options, "reasoningEffort")),
        cwd: string_option(&options, "cwd"),
        permission_profile: string_option(&options, "permission_profile")
            .or_else(|| string_option(&options, "permissionProfile")),
        approval_policy: string_option(&options, "approval_policy")
            .or_else(|| string_option(&options, "approvalPolicy")),
        sandbox_mode: string_option(&options, "sandbox_mode")
            .or_else(|| string_option(&options, "sandboxMode")),
        network_access: options
            .get("network_access")
            .or_else(|| options.get("networkAccess"))
            .and_then(Value::as_bool),
        collaboration_mode: string_option(&options, "collaboration_mode")
            .or_else(|| string_option(&options, "collaborationMode")),
        ..ThreadMessageRequest::default()
    }
}

pub fn followup_views(items: Vec<ThreadFollowUp>) -> Vec<FollowUpView> {
    items.into_iter().map(followup_view).collect()
}

pub fn followup_view(item: ThreadFollowUp) -> FollowUpView {
    FollowUpView {
        id: item.id,
        thread_id: item.thread_id,
        status: item.status,
        message: item.message,
        options: serde_json::from_str::<Value>(&item.options_json).unwrap_or(Value::Null),
        created_at: item.created_at,
        updated_at: item.updated_at,
        submitted_at: item.submitted_at,
        cancelled_at: item.cancelled_at,
        result: item
            .result_json
            .and_then(|value| serde_json::from_str::<Value>(&value).ok()),
        error: item.error,
    }
}

pub fn codex_resume_args(thread_id: &str) -> Vec<String> {
    vec![
        "exec".to_string(),
        "resume".to_string(),
        "--all".to_string(),
        "--json".to_string(),
        thread_id.to_string(),
        "-".to_string(),
    ]
}

pub fn plan_accept_resume_message() -> String {
    "是，实施此计划".to_string()
}

pub fn plan_revise_resume_message(instructions: &str) -> String {
    format!(
        "否，请告知 Codex 如何调整\n\n请保持 Plan Mode，只根据下面的修改要求重新给出计划，不要开始实施。\n\n修改要求：\n{}",
        instructions.trim()
    )
}

pub fn elicitation_answer_resume_message(answers: &HashMap<String, Vec<String>>) -> String {
    let mut rows = answers.iter().collect::<Vec<_>>();
    rows.sort_by_key(|(question, _)| *question);
    rows.into_iter()
        .map(|(question, answers)| format!("{question}: {}", answers.join(", ")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn add_codex_common_args(args: &mut Vec<String>, request: &JobActionRequest) {
    if let Some(model) = non_empty(request.model.as_deref()) {
        args.splice(1..1, ["-m".to_string(), model.to_string()]);
    }
    if let Some(reasoning) = non_empty(request.reasoning_effort.as_deref()) {
        args.splice(
            1..1,
            [
                "-c".to_string(),
                format!(
                    "model_reasoning_effort=\"{}\"",
                    cli_config_string(reasoning)
                ),
            ],
        );
    }
    if let Some(service_tier) = non_empty(request.service_tier.as_deref()) {
        args.splice(
            1..1,
            [
                "-c".to_string(),
                format!("model_service_tier=\"{}\"", cli_config_string(service_tier)),
            ],
        );
    }
    if let Some(approval_policy) = non_empty(request.approval_policy.as_deref()) {
        args.splice(
            1..1,
            [
                "-c".to_string(),
                format!("approval_policy=\"{}\"", cli_config_string(approval_policy)),
            ],
        );
    }
    if let Some(sandbox_mode) = non_empty(request.sandbox_mode.as_deref()) {
        args.splice(
            1..1,
            [
                "-c".to_string(),
                format!("sandbox_mode=\"{}\"", cli_config_string(sandbox_mode)),
            ],
        );
    }
    if let Some(network_access) = request.network_access {
        args.splice(
            1..1,
            [
                "-c".to_string(),
                format!(
                    "network_access=\"{}\"",
                    if network_access {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ),
            ],
        );
    }
    if let Some(collaboration_mode) = non_empty(request.collaboration_mode.as_deref()) {
        let enabled = matches!(
            collaboration_mode,
            "enabled" | "on" | "true" | "async" | "parallel"
        );
        args.splice(
            1..1,
            [
                "-c".to_string(),
                format!(
                    "features.collaboration_modes={}",
                    if enabled { "true" } else { "false" }
                ),
            ],
        );
    }
    apply_permission_profile_defaults(args, request);
}

fn apply_permission_profile_defaults(args: &mut Vec<String>, request: &JobActionRequest) {
    let Some(profile) = non_empty(request.permission_profile.as_deref()) else {
        return;
    };
    if request
        .sandbox_mode
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        let sandbox = match profile {
            "danger-full-access" => Some("danger-full-access"),
            "workspace-write" => Some("workspace-write"),
            "read-only" => Some("read-only"),
            _ => None,
        };
        if let Some(sandbox) = sandbox {
            args.splice(
                1..1,
                ["-c".to_string(), format!("sandbox_mode=\"{sandbox}\"")],
            );
        }
    }
    if request
        .approval_policy
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        let approval = match profile {
            "danger-full-access" => Some("never"),
            "workspace-write" | "read-only" => Some("on-request"),
            _ => None,
        };
        if let Some(approval) = approval {
            args.splice(
                1..1,
                ["-c".to_string(), format!("approval_policy=\"{approval}\"")],
            );
        }
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn non_empty_owned(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn required_job_id(value: &str) -> Result<String> {
    non_empty_owned(value).ok_or_else(|| anyhow!("job_id is required"))
}

fn plan_followup_submitted_result(
    followup_id: &str,
    result: Value,
) -> Result<FollowUpSubmitResultPlan> {
    Ok(FollowUpSubmitResultPlan {
        required_capability: Capability::Jobs,
        command: commands::THREADS_FOLLOWUPS_SUBMIT.to_string(),
        followup_id: non_empty_owned(followup_id)
            .ok_or_else(|| anyhow!("followup_id is required"))?,
        result,
    })
}

fn plan_followup_error_result(followup_id: &str, error: &str) -> Result<FollowUpErrorPlan> {
    Ok(FollowUpErrorPlan {
        required_capability: Capability::Jobs,
        command: commands::THREADS_FOLLOWUPS_ERROR.to_string(),
        followup_id: non_empty_owned(followup_id)
            .ok_or_else(|| anyhow!("followup_id is required"))?,
        error: non_empty_owned(error).ok_or_else(|| anyhow!("error is required"))?,
    })
}

fn required_command_thread_id(
    request_thread_id: Option<&str>,
    message_thread_id: Option<&str>,
) -> Result<String> {
    non_empty(request_thread_id)
        .or_else(|| non_empty(message_thread_id))
        .map(str::to_string)
        .ok_or_else(|| anyhow!("thread_id is required"))
}

fn normalize_followup_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(20).clamp(1, 200)
}

fn plan_thread_archive_state_action(
    platform: &PlatformPaths,
    thread_id: &str,
    archived: bool,
) -> Result<ThreadStateActionPlan> {
    require_capability(platform, Capability::ThreadArchiveActions)?;
    Ok(ThreadStateActionPlan {
        required_capability: Capability::ThreadArchiveActions,
        command: if archived {
            commands::THREADS_ARCHIVE
        } else {
            commands::THREADS_RESTORE
        }
        .to_string(),
        thread_id: required_command_thread_id(Some(thread_id), None)?,
        archived: Some(archived),
        name: None,
    })
}

fn string_option(options: &Value, key: &str) -> Option<String> {
    options.get(key).and_then(Value::as_str).map(str::to_string)
}

fn string_array_option(options: &Value, key: &str) -> Vec<String> {
    options
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn cli_config_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::PathBuf};

    use crate::codex::ThreadStatus;
    use crate::services::commands;
    use crate::services::jobs::{
        archive_thread_response, build_codex_job_spec, cancel_followup_response,
        codex_action_submitted, effective_message, elicitation_answer_resume_message,
        followup_request, followup_view, fork_thread_unavailable_response,
        normalize_thread_command_request, plan_accept_resume_message, plan_followup_autosubmit,
        plan_followup_status_transition, plan_queued_followup_job_spec, plan_revise_resume_message,
        plan_steer_thread_as_followup, rename_thread_response, thread_message_options_json,
        CodexActionKind, FollowUpTransitionKind, JobActionRequest, ThreadCommandKind,
        ThreadCommandRequest, ThreadMessageRequest,
    };
    use crate::{
        db::ThreadFollowUp,
        uploads::{PreparedAttachment, UploadKind},
    };
    use serde_json::json;

    #[test]
    fn exec_action_request_builds_codex_job_spec_and_argv() {
        let request = JobActionRequest {
            kind: CodexActionKind::Exec,
            thread_id: None,
            message: "  start new work  ".to_string(),
            cwd: Some(PathBuf::from("/tmp/project")),
            model: Some("gpt-5.5".to_string()),
            service_tier: Some("priority".to_string()),
            reasoning_effort: Some("xhigh".to_string()),
            permission_profile: Some("danger-full-access".to_string()),
            approval_policy: None,
            sandbox_mode: None,
            network_access: Some(true),
            collaboration_mode: Some("async".to_string()),
        };

        let spec = build_codex_job_spec(&request, PathBuf::from("/default/workspace")).unwrap();

        assert_eq!(spec.title, "Codex new thread");
        assert_eq!(spec.thread_id, None);
        assert_eq!(spec.cwd, PathBuf::from("/tmp/project"));
        assert_eq!(spec.prompt, "start new work");
        assert_eq!(spec.args[0], "exec");
        assert!(spec.args.windows(2).any(|pair| pair == ["-m", "gpt-5.5"]));
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["-c", "model_reasoning_effort=\"xhigh\""]));
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["-c", "model_service_tier=\"priority\""]));
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["-c", "sandbox_mode=\"danger-full-access\""]));
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["-c", "approval_policy=\"never\""]));
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["-c", "network_access=\"enabled\""]));
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["-c", "features.collaboration_modes=true"]));
        assert!(spec
            .args
            .ends_with(&["--skip-git-repo-check".to_string(), "-".to_string()]));
    }

    #[test]
    fn resume_action_request_builds_codex_job_spec_and_argv() {
        let request = JobActionRequest {
            kind: CodexActionKind::Resume,
            thread_id: Some("thread-a".to_string()),
            message: "continue".to_string(),
            cwd: None,
            model: Some("gpt-5.4".to_string()),
            service_tier: None,
            reasoning_effort: Some("high".to_string()),
            permission_profile: Some("read-only".to_string()),
            approval_policy: None,
            sandbox_mode: None,
            network_access: Some(false),
            collaboration_mode: Some("off".to_string()),
        };

        let spec = build_codex_job_spec(&request, PathBuf::from("/default/workspace")).unwrap();

        assert_eq!(spec.title, "Codex resume thread");
        assert_eq!(spec.thread_id.as_deref(), Some("thread-a"));
        assert_eq!(spec.cwd, PathBuf::from("/default/workspace"));
        assert_eq!(
            spec.args
                .iter()
                .map(String::as_str)
                .filter(|arg| matches!(
                    *arg,
                    "exec" | "resume" | "--all" | "--json" | "thread-a" | "-"
                ))
                .collect::<Vec<_>>(),
            vec!["exec", "resume", "--all", "--json", "thread-a", "-"]
        );
        assert!(spec.args.windows(2).any(|pair| pair == ["-m", "gpt-5.4"]));
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["-c", "model_reasoning_effort=\"high\""]));
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["-c", "sandbox_mode=\"read-only\""]));
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["-c", "approval_policy=\"on-request\""]));
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["-c", "network_access=\"disabled\""]));
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["-c", "features.collaboration_modes=false"]));
    }

    #[test]
    fn plan_and_elicitation_resume_helpers_generate_stable_messages() {
        assert_eq!(plan_accept_resume_message(), "是，实施此计划");
        assert_eq!(
            plan_revise_resume_message("  先补测试，再实现  "),
            "否，请告知 Codex 如何调整\n\n请保持 Plan Mode，只根据下面的修改要求重新给出计划，不要开始实施。\n\n修改要求：\n先补测试，再实现"
        );

        let answers = HashMap::from([
            ("q2".to_string(), vec!["B".to_string(), "C".to_string()]),
            ("q1".to_string(), vec!["A".to_string()]),
        ]);

        assert_eq!(
            elicitation_answer_resume_message(&answers),
            "q1: A\nq2: B, C"
        );
    }

    #[test]
    fn resume_action_requires_thread_id_and_non_empty_message() {
        let missing_thread = JobActionRequest {
            kind: CodexActionKind::Resume,
            thread_id: None,
            message: "continue".to_string(),
            ..JobActionRequest::default()
        };
        assert!(build_codex_job_spec(&missing_thread, PathBuf::from("/workspace")).is_err());

        let empty_message = JobActionRequest {
            kind: CodexActionKind::Exec,
            message: "   ".to_string(),
            ..JobActionRequest::default()
        };
        assert!(build_codex_job_spec(&empty_message, PathBuf::from("/workspace")).is_err());
    }

    #[test]
    fn thread_message_helpers_normalize_attachment_only_prompt_and_options() {
        let attachment = PreparedAttachment {
            id: "upload-1".to_string(),
            name: "notes.md".to_string(),
            mime: "text/markdown".to_string(),
            kind: UploadKind::Markdown,
            size: 42,
            sha256: "abc".to_string(),
            text: Some("hello".to_string()),
            truncated: false,
            local_image_path: None,
            local_file_path: None,
        };
        let request = ThreadMessageRequest {
            message: "   ".to_string(),
            attachments: vec!["upload-1".to_string()],
            prepared_attachments: vec![attachment.clone()],
            model: Some("gpt-5.5".to_string()),
            service_tier: Some("priority".to_string()),
            reasoning_effort: Some("high".to_string()),
            cwd: Some(" /tmp/work ".to_string()),
            permission_profile: Some("workspace-write".to_string()),
            approval_policy: None,
            sandbox_mode: None,
            network_access: Some(true),
            collaboration_mode: Some("async".to_string()),
            ..ThreadMessageRequest::default()
        };

        assert_eq!(
            effective_message(&request.message, &request.prepared_attachments),
            "请根据以下附件内容继续处理。"
        );
        assert!(request
            .prompt_with_attachment_context()
            .contains("## 附加文件上下文"));
        let options = thread_message_options_json(&request);
        assert_eq!(options["service_tier"], "priority");
        assert_eq!(options["attachments"][0], "upload-1");
        assert_eq!(options["prepared_attachments"][0]["name"], "notes.md");

        let action = request.clone().into_job_action(CodexActionKind::Resume);
        assert_eq!(action.cwd, Some(PathBuf::from("/tmp/work")));
        assert_eq!(action.model.as_deref(), Some("gpt-5.5"));
        assert!(action.message.contains("请根据以下附件内容继续处理。"));
    }

    #[test]
    fn thread_command_request_normalizes_exec_resume_and_followup_paths() {
        let create = normalize_thread_command_request(ThreadCommandRequest {
            command: ThreadCommandKind::Create,
            thread_id: Some("ignored".to_string()),
            message: ThreadMessageRequest {
                message: "  start  ".to_string(),
                cwd: Some(" /tmp/work ".to_string()),
                ..ThreadMessageRequest::default()
            },
        })
        .unwrap();
        let create_action = create.action.expect("create action");
        assert_eq!(create_action.kind, CodexActionKind::Exec);
        assert_eq!(create_action.thread_id, None);
        assert_eq!(create_action.message, "start");
        assert_eq!(create_action.cwd, Some(PathBuf::from("/tmp/work")));
        assert!(create.followup.is_none());

        let resume = normalize_thread_command_request(ThreadCommandRequest {
            command: ThreadCommandKind::Resume,
            thread_id: Some(" thread-a ".to_string()),
            message: ThreadMessageRequest {
                message: " continue ".to_string(),
                model: Some("gpt-5.5".to_string()),
                ..ThreadMessageRequest::default()
            },
        })
        .unwrap();
        let resume_action = resume.action.expect("resume action");
        assert_eq!(resume_action.kind, CodexActionKind::Resume);
        assert_eq!(resume_action.thread_id.as_deref(), Some("thread-a"));
        assert_eq!(resume_action.message, "continue");
        assert_eq!(resume_action.model.as_deref(), Some("gpt-5.5"));

        let steer = normalize_thread_command_request(ThreadCommandRequest {
            command: ThreadCommandKind::FollowUp,
            thread_id: Some(" thread-a ".to_string()),
            message: ThreadMessageRequest {
                message: "  queue this  ".to_string(),
                attachments: vec!["upload-a".to_string()],
                ..ThreadMessageRequest::default()
            },
        })
        .unwrap();
        assert!(steer.action.is_none());
        let followup = steer.followup.expect("followup plan");
        assert_eq!(followup.thread_id, "thread-a");
        assert_eq!(followup.message, "queue this");
        assert_eq!(followup.options["attachments"][0], "upload-a");
    }

    #[test]
    fn steer_thread_helper_plans_followup_without_resume_action() {
        let plan = plan_steer_thread_as_followup(ThreadCommandRequest {
            command: ThreadCommandKind::Resume,
            thread_id: Some(" thread-a ".to_string()),
            message: ThreadMessageRequest {
                thread_id: Some("ignored-when-top-level-present".to_string()),
                message: "  follow up when ready  ".to_string(),
                model: Some("gpt-5.5".to_string()),
                ..ThreadMessageRequest::default()
            },
        })
        .unwrap();

        assert_eq!(plan.command, ThreadCommandKind::FollowUp);
        assert_eq!(plan.thread_id.as_deref(), Some("thread-a"));
        assert!(plan.action.is_none());
        let followup = plan.followup.expect("followup plan");
        assert_eq!(followup.thread_id, "thread-a");
        assert_eq!(followup.message, "follow up when ready");
        assert_eq!(followup.options["model"], "gpt-5.5");
    }

    #[test]
    fn thread_command_request_rejects_missing_thread_for_resume_and_followup() {
        let missing_resume = normalize_thread_command_request(ThreadCommandRequest {
            command: ThreadCommandKind::Resume,
            thread_id: Some(" ".to_string()),
            message: ThreadMessageRequest {
                message: "continue".to_string(),
                ..ThreadMessageRequest::default()
            },
        });
        assert!(missing_resume
            .unwrap_err()
            .to_string()
            .contains("thread_id is required"));

        let missing_followup = normalize_thread_command_request(ThreadCommandRequest {
            command: ThreadCommandKind::FollowUp,
            thread_id: None,
            message: ThreadMessageRequest {
                message: "queue".to_string(),
                ..ThreadMessageRequest::default()
            },
        });
        assert!(missing_followup
            .unwrap_err()
            .to_string()
            .contains("thread_id is required"));
    }

    #[test]
    fn followup_round_trips_options_and_serializes_view() {
        let options = json!({
            "model": "gpt-5.4",
            "serviceTier": "priority",
            "reasoningEffort": "high",
            "permissionProfile": "danger-full-access",
            "approvalPolicy": "never",
            "sandboxMode": "danger-full-access",
            "networkAccess": true,
            "collaborationMode": "parallel",
            "attachments": ["upload-a"],
            "prepared_attachments": [{
                "id": "upload-a",
                "name": "a.txt",
                "mime": "text/plain",
                "kind": "markdown",
                "size": 12,
                "sha256": "hash",
                "text": "body",
                "truncated": false
            }]
        });
        let followup = ThreadFollowUp {
            id: "f1".to_string(),
            thread_id: "thread-a".to_string(),
            status: "pending".to_string(),
            message: "continue".to_string(),
            options_json: options.to_string(),
            created_at: 1,
            updated_at: 2,
            submitted_at: None,
            cancelled_at: None,
            result_json: Some(json!({"job_id":"job-a"}).to_string()),
            error: None,
        };

        let request = followup_request(&followup);
        assert_eq!(request.message, "continue");
        assert_eq!(request.attachments, vec!["upload-a"]);
        assert_eq!(request.prepared_attachments[0].name, "a.txt");
        assert_eq!(request.service_tier.as_deref(), Some("priority"));
        assert_eq!(request.reasoning_effort.as_deref(), Some("high"));
        assert_eq!(
            request.permission_profile.as_deref(),
            Some("danger-full-access")
        );
        assert_eq!(request.network_access, Some(true));

        let view = followup_view(followup);
        let serialized = serde_json::to_value(&view).unwrap();
        assert_eq!(serialized["thread_id"], "thread-a");
        assert!(serialized.get("threadId").is_none());
        assert_eq!(view.options["model"], "gpt-5.4");
        assert_eq!(view.result.unwrap()["job_id"], "job-a");
    }

    #[test]
    fn followup_lifecycle_plans_define_shared_status_transitions() {
        let claim = plan_followup_status_transition(" f1 ", FollowUpTransitionKind::Claim).unwrap();
        assert_eq!(claim.followup_id, "f1");
        assert_eq!(claim.from_status.as_deref(), Some("pending"));
        assert_eq!(claim.to_status, "submitting");
        assert!(claim.result.is_none());
        assert!(claim.error.is_none());

        let submitted = plan_followup_status_transition(
            " f1 ",
            FollowUpTransitionKind::Submitted {
                result: json!({"job_id":"job-a"}),
            },
        )
        .unwrap();
        assert_eq!(submitted.from_status.as_deref(), Some("submitting"));
        assert_eq!(submitted.to_status, "submitted");
        assert_eq!(submitted.result.unwrap()["job_id"], "job-a");

        let failed = plan_followup_status_transition(
            " f1 ",
            FollowUpTransitionKind::Error {
                error: "  failed locally  ".to_string(),
            },
        )
        .unwrap();
        assert_eq!(failed.to_status, "error");
        assert_eq!(failed.error.as_deref(), Some("failed locally"));

        let cancel =
            plan_followup_status_transition(" f1 ", FollowUpTransitionKind::Cancel).unwrap();
        assert_eq!(cancel.from_status.as_deref(), Some("pending"));
        assert_eq!(cancel.to_status, "cancelled");

        assert!(plan_followup_status_transition(" ", FollowUpTransitionKind::Claim).is_err());
        assert!(plan_followup_status_transition(
            "f1",
            FollowUpTransitionKind::Error {
                error: " ".to_string()
            }
        )
        .is_err());
    }

    #[test]
    fn queued_followup_autosubmit_and_resume_spec_are_core_defined() {
        let followup = ThreadFollowUp {
            id: "f1".to_string(),
            thread_id: "thread-a".to_string(),
            status: "submitting".to_string(),
            message: "continue later".to_string(),
            options_json: json!({"model":"gpt-5.5","network_access":true}).to_string(),
            created_at: 1,
            updated_at: 2,
            submitted_at: None,
            cancelled_at: None,
            result_json: None,
            error: None,
        };

        let autosubmit = plan_followup_autosubmit(ThreadStatus::Recent, true);
        assert!(autosubmit.should_claim_pending);
        assert!(autosubmit.should_start_resume_job);
        assert!(!plan_followup_autosubmit(ThreadStatus::Running, true).should_claim_pending);
        assert!(!plan_followup_autosubmit(ThreadStatus::Recent, false).should_start_resume_job);

        let spec = plan_queued_followup_job_spec(&followup, PathBuf::from("/workspace")).unwrap();

        assert_eq!(spec.title, "Codex queued follow-up");
        assert_eq!(spec.thread_id.as_deref(), Some("thread-a"));
        assert_eq!(spec.prompt, "continue later");
        assert!(spec.args.windows(2).any(|pair| pair == ["-m", "gpt-5.5"]));
        assert!(spec
            .args
            .windows(2)
            .any(|pair| pair == ["-c", "network_access=\"enabled\""]));
    }

    #[test]
    fn shared_action_responses_match_desktop_contract() {
        assert_eq!(
            codex_action_submitted(Some("thread-a".to_string()), Some("job-a".to_string()))
                .message
                .as_deref(),
            Some("已提交给 Codex")
        );

        let archived = archive_thread_response("thread-a".to_string(), true);
        assert!(archived.ok);
        assert_eq!(archived.command, commands::THREADS_ARCHIVE);
        assert_eq!(archived.thread_id.as_deref(), Some("thread-a"));

        let renamed = rename_thread_response("thread-a".to_string(), "  新名字  ").unwrap();
        assert_eq!(renamed.command, commands::THREADS_RENAME);
        assert_eq!(renamed.data.unwrap()["name"], "新名字");
        assert!(rename_thread_response("thread-a".to_string(), "   ").is_err());

        let cancelled = cancel_followup_response(
            commands::THREADS_FOLLOWUPS_CANCEL,
            "thread-a".to_string(),
            "f1".to_string(),
            false,
        );
        assert_eq!(cancelled.message, "follow-up was not pending");
        assert_eq!(cancelled.data.unwrap()["cancelled"], false);

        let fork = fork_thread_unavailable_response(Some(" thread-a ".to_string()));
        assert!(!fork.ok);
        assert!(!fork.available);
        assert_eq!(fork.command, commands::THREADS_FORK);
        assert_eq!(fork.thread_id.as_deref(), Some("thread-a"));
    }
}
