use std::{collections::HashMap, path::PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

pub const CODEX_SUBMITTED_MESSAGE: &str = "已提交给 Codex";

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

fn cli_config_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::PathBuf};

    use crate::services::jobs::{
        build_codex_job_spec, elicitation_answer_resume_message, plan_accept_resume_message,
        plan_revise_resume_message, CodexActionKind, JobActionRequest,
    };

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
}
