use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ThreadStatus {
    Recent,
    Running,
    ReplyNeeded,
    Recoverable,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub id: String,
    pub title: String,
    pub status: ThreadStatus,
    pub updated_at: Option<String>,
    pub archived_at: Option<String>,
    pub message_count: usize,
    pub latest_message: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub rollout_path: Option<PathBuf>,
    pub active_turn_id: Option<String>,
    pub active_job_id: Option<String>,
    pub pending_elicitation: Option<PendingElicitation>,
    pub last_event_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadDetail {
    pub summary: ThreadSummary,
    pub messages: Vec<CodexMessage>,
    pub blocks: Vec<MessageBlock>,
    pub raw_event_count: usize,
    pub total_blocks: usize,
    pub has_more_blocks: bool,
    pub before_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexMessage {
    pub role: String,
    pub kind: String,
    pub text: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PendingElicitation {
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub questions: Vec<UserInputQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserInputQuestion {
    pub id: String,
    pub header: Option<String>,
    pub question: String,
    pub options: Vec<UserInputOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserInputOption {
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserInputAnswer {
    pub question_id: String,
    pub answers: Vec<String>,
    pub note: Option<String>,
}

pub fn extract_proposed_plan_text(text: &str) -> Option<String> {
    let (_, after_start) = text.split_once("<proposed_plan>")?;
    let plan = after_start
        .split_once("</proposed_plan>")
        .map(|(body, _)| body)
        .unwrap_or(after_start)
        .trim();
    (!plan.is_empty()).then(|| plan.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageBlock {
    pub id: String,
    pub role: String,
    pub kind: String,
    pub display_kind: Option<String>,
    pub status: Option<String>,
    pub text: Option<String>,
    pub summary: Option<String>,
    pub input: Option<String>,
    pub truncated: Option<bool>,
    pub resolved: Option<bool>,
    pub answers: Vec<UserInputAnswer>,
    pub plan_status: Option<String>,
    pub group_id: Option<String>,
    pub tool_name: Option<String>,
    pub call_id: Option<String>,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub created_at: Option<String>,
    pub questions: Vec<UserInputQuestion>,
    pub payload: Option<Value>,
}
