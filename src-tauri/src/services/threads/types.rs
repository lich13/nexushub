use nexushub_core::{services::jobs as job_service, uploads};
use serde::Deserialize;
use std::collections::HashMap;

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
