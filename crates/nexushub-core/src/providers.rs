use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentProviderId {
    Codex,
    ClaudeCode,
    Cursor,
    Gemini,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentProviderInfo {
    pub id: AgentProviderId,
    pub label: String,
    pub status: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub safety: String,
}

#[derive(Debug, Clone)]
pub struct ProviderRegistry {
    providers: Vec<AgentProviderInfo>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self {
            providers: vec![
                AgentProviderInfo {
                    id: AgentProviderId::Codex,
                    label: "Codex".to_string(),
                    status: "ready".to_string(),
                    description: "Full NexusHub control surface backed by official Codex state DB, session index, rollout files, logs_2.sqlite, and controlled jobs.".to_string(),
                    capabilities: vec![
                        "ready".to_string(),
                        "threads".to_string(),
                        "chat".to_string(),
                        "plan_questions".to_string(),
                        "uploads".to_string(),
                        "updates".to_string(),
                        "doctor".to_string(),
                    ],
                    safety: "uses existing Codex local state and controlled jobs without mutating official schema".to_string(),
                },
                AgentProviderInfo {
                    id: AgentProviderId::ClaudeCode,
                    label: "Claude Code".to_string(),
                    status: "preview".to_string(),
                    description:
                        "Read-only Claude Code overview from ~/.claude, including projects, recent sessions, settings, MCP, install, cache, and log summaries.".to_string(),
                    capabilities: vec![
                        "readonly".to_string(),
                        "projects".to_string(),
                        "sessions".to_string(),
                        "recent_sessions".to_string(),
                        "settings_read".to_string(),
                        "mcp_summary".to_string(),
                        "install_health".to_string(),
                        "cache_log_status".to_string(),
                    ],
                    safety: "read-only filesystem inspection only; no ~/.claude writes and no launch, resume, send, or stop controls".to_string(),
                },
                AgentProviderInfo {
                    id: AgentProviderId::Cursor,
                    label: "Cursor CLI".to_string(),
                    status: "planned".to_string(),
                    description: "Provider slot reserved for Cursor CLI once command and session contracts are defined.".to_string(),
                    capabilities: Vec::new(),
                    safety: "no command execution is exposed".to_string(),
                },
                AgentProviderInfo {
                    id: AgentProviderId::Gemini,
                    label: "Gemini CLI".to_string(),
                    status: "planned".to_string(),
                    description: "Provider slot reserved for Gemini CLI once command and session contracts are defined.".to_string(),
                    capabilities: Vec::new(),
                    safety: "no command execution is exposed".to_string(),
                },
            ],
        }
    }
}

impl ProviderRegistry {
    pub fn list(&self) -> Vec<AgentProviderInfo> {
        self.providers.clone()
    }
}
