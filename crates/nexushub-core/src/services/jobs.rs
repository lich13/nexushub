use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    db::ThreadFollowUp,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexJobSpec {
    pub title: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub prompt: String,
    pub thread_id: Option<String>,
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

fn required_command_thread_id(
    request_thread_id: Option<&str>,
    message_thread_id: Option<&str>,
) -> Result<String> {
    non_empty(request_thread_id)
        .or_else(|| non_empty(message_thread_id))
        .map(str::to_string)
        .ok_or_else(|| anyhow!("thread_id is required"))
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

    use crate::services::commands;
    use crate::services::jobs::{
        archive_thread_response, build_codex_job_spec, cancel_followup_response,
        codex_action_submitted, effective_message, elicitation_answer_resume_message,
        followup_request, followup_view, normalize_thread_command_request,
        plan_accept_resume_message, plan_revise_resume_message, plan_steer_thread_as_followup,
        rename_thread_response, thread_message_options_json, CodexActionKind, JobActionRequest,
        ThreadCommandKind, ThreadCommandRequest, ThreadMessageRequest,
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
    }
}
