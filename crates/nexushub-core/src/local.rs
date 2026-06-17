use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalPluginInfo {
    pub id: String,
    pub label: String,
    pub status: String,
    pub kind: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
    pub invocation_template: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexModelInfo {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexPermissionProfile {
    pub id: String,
    pub label: String,
    pub sandbox_mode: String,
    pub approval_policy: String,
    pub network_access: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalCodexConfig {
    pub model: Option<String>,
    pub reasoning_effort: Option<String>,
    pub cwd: String,
    pub permission_profile: String,
    pub approval_policy: String,
    pub sandbox_mode: String,
    pub network_access: bool,
    pub raw: LocalCodexConfigSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalCodexConfigSource {
    pub source: String,
    pub available: bool,
}

pub fn local_plugin_catalog() -> Vec<LocalPluginInfo> {
    vec![
        LocalPluginInfo {
            id: "codex".to_string(),
            label: "Codex".to_string(),
            status: "ready".to_string(),
            kind: "builtin".to_string(),
            description: "Codex 本地状态、线程和受控 job 操作".to_string(),
            unavailable_reason: None,
            invocation_template: "@Codex ".to_string(),
        },
        LocalPluginInfo {
            id: "probe".to_string(),
            label: "Probe".to_string(),
            status: "ready".to_string(),
            kind: "builtin".to_string(),
            description: "云机探针状态、Hook、Bark 和日志库维护".to_string(),
            unavailable_reason: None,
            invocation_template: "@Probe ".to_string(),
        },
        LocalPluginInfo {
            id: "claude_code".to_string(),
            label: "Claude Code".to_string(),
            status: "preview".to_string(),
            kind: "builtin".to_string(),
            description: "Claude Code 项目、会话和 MCP 只读预览".to_string(),
            unavailable_reason: Some(
                "当前仅支持只读预览，暂不支持从 Web 端调用 Claude Code".to_string(),
            ),
            invocation_template: "@Claude Code ".to_string(),
        },
        LocalPluginInfo {
            id: "system_ops".to_string(),
            label: "System/Ops".to_string(),
            status: "ready".to_string(),
            kind: "builtin".to_string(),
            description: "固定系统运维动作和发布更新任务".to_string(),
            unavailable_reason: None,
            invocation_template: "@System/Ops ".to_string(),
        },
    ]
}

pub fn default_codex_models() -> Vec<CodexModelInfo> {
    vec![
        CodexModelInfo {
            id: "gpt-5.5".to_string(),
            label: "GPT-5.5".to_string(),
            default: true,
        },
        CodexModelInfo {
            id: "gpt-5.5-codex".to_string(),
            label: "GPT-5.5 Codex".to_string(),
            default: false,
        },
        CodexModelInfo {
            id: "gpt-5.4".to_string(),
            label: "GPT-5.4".to_string(),
            default: false,
        },
        CodexModelInfo {
            id: "gpt-5.4-mini".to_string(),
            label: "GPT-5.4 mini".to_string(),
            default: false,
        },
        CodexModelInfo {
            id: "gpt-5.3-codex".to_string(),
            label: "GPT-5.3 Codex".to_string(),
            default: false,
        },
    ]
}

pub fn default_permission_profiles() -> Vec<CodexPermissionProfile> {
    vec![
        CodexPermissionProfile {
            id: "danger-full-access".to_string(),
            label: "Danger full access".to_string(),
            sandbox_mode: "danger-full-access".to_string(),
            approval_policy: "never".to_string(),
            network_access: true,
            default: true,
        },
        CodexPermissionProfile {
            id: "workspace-write".to_string(),
            label: "Workspace write".to_string(),
            sandbox_mode: "workspace-write".to_string(),
            approval_policy: "on-request".to_string(),
            network_access: true,
            default: false,
        },
        CodexPermissionProfile {
            id: "read-only".to_string(),
            label: "Read only".to_string(),
            sandbox_mode: "read-only".to_string(),
            approval_policy: "on-request".to_string(),
            network_access: false,
            default: false,
        },
    ]
}

pub fn local_codex_config(config: &Config, cwd: Option<&str>) -> LocalCodexConfig {
    LocalCodexConfig {
        model: None,
        reasoning_effort: None,
        cwd: normalized_cwd(cwd, &config.codex.workspace),
        permission_profile: "danger-full-access".to_string(),
        approval_policy: "never".to_string(),
        sandbox_mode: "danger-full-access".to_string(),
        network_access: true,
        raw: LocalCodexConfigSource {
            source: "local".to_string(),
            available: true,
        },
    }
}

fn normalized_cwd(cwd: Option<&str>, workspace: &Path) -> String {
    cwd.map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| workspace.to_str().unwrap_or(""))
        .to_string()
}

fn is_false(value: &bool) -> bool {
    !*value
}
