use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

use crate::{
    codex::{CodexGoalAction, CodexGoalClient, CodexThreadGoal},
    platform::PlatformPaths,
    services::commands,
    services::system::{require_capability, Capability},
};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GoalUpdateRequest {
    #[serde(default, alias = "threadId")]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub objective: Option<String>,
    #[serde(default, alias = "tokenBudget")]
    pub token_budget: Option<u64>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GoalGetRequest {
    #[serde(default, alias = "threadId")]
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GoalCommandKind {
    Save,
    Clear,
    Pause,
    Resume,
}

impl GoalCommandKind {
    pub fn as_rpc_action(self) -> &'static str {
        match self {
            Self::Save => commands::THREADS_GOAL_SAVE,
            Self::Clear => commands::THREADS_GOAL_CLEAR,
            Self::Pause => commands::THREADS_GOAL_PAUSE,
            Self::Resume => commands::THREADS_GOAL_RESUME,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        codex::CodexThreadGoal,
        platform::{PlatformKind, PlatformPaths},
        services::goals::{
            goal_empty, goal_response_from_codex, normalize_goal_status, plan_clear_goal_update,
            plan_goal_get_with_capability, plan_goal_save_with_capability, plan_goal_update,
            plan_pause_goal_update, plan_resume_goal_update, GoalCommandKind, GoalGetRequest,
            GoalUpdateRequest,
        },
    };

    #[test]
    fn codex_goal_response_uses_official_state_without_local_fallback() {
        let empty = goal_response_from_codex(None);
        assert!(empty.available);
        assert!(!empty.enabled);
        assert_eq!(empty.status, "idle");
        assert_eq!(empty.objective, None);

        let active = goal_response_from_codex(Some(&CodexThreadGoal {
            thread_id: "thread-live".to_string(),
            objective: "Ship the real Goal".to_string(),
            status: "budgetLimited".to_string(),
            token_budget: Some(12_000),
            tokens_used: 12_500,
            time_used_seconds: 90,
            created_at: 100,
            updated_at: 200,
        }));
        assert!(active.available);
        assert!(active.enabled);
        assert_eq!(active.thread_id.as_deref(), Some("thread-live"));
        assert_eq!(active.objective.as_deref(), Some("Ship the real Goal"));
        assert_eq!(active.token_budget, Some(12_000));
        assert_eq!(active.status, "budgetLimited");
        assert_eq!(
            active.raw.as_ref().and_then(|raw| raw.source.as_deref()),
            Some("codex_app_server")
        );
        assert_eq!(
            active.raw.as_ref().and_then(|raw| raw.created_at),
            Some(100)
        );
        assert_eq!(
            active.raw.as_ref().and_then(|raw| raw.updated_at),
            Some(200)
        );
    }

    #[test]
    fn goal_update_request_plans_save_clear_pause_and_resume() {
        let save = plan_goal_update(GoalUpdateRequest {
            thread_id: Some(" thread-a ".to_string()),
            objective: Some("  Ship the feature  ".to_string()),
            token_budget: Some(2048),
            status: None,
            enabled: None,
        })
        .unwrap();
        assert_eq!(save.command, GoalCommandKind::Save);
        assert_eq!(save.update.thread_id, "thread-a");
        assert_eq!(save.update.objective.as_deref(), Some("Ship the feature"));
        assert_eq!(save.update.token_budget, Some(2048));
        assert_eq!(save.update.status, "active");

        let clear = plan_clear_goal_update(Some(" thread-a ")).unwrap();
        assert_eq!(clear.command, GoalCommandKind::Clear);
        assert_eq!(clear.update.thread_id, "thread-a");
        assert_eq!(clear.update.objective, None);
        assert_eq!(clear.update.status, "cleared");

        let paused = plan_pause_goal_update(" thread-a ").unwrap();
        assert_eq!(paused.command, GoalCommandKind::Pause);
        assert_eq!(paused.update.objective, None);
        assert_eq!(paused.update.token_budget, None);
        assert_eq!(paused.update.status, "paused");

        let resumed = plan_resume_goal_update(" thread-a ").unwrap();
        assert_eq!(resumed.command, GoalCommandKind::Resume);
        assert_eq!(resumed.update.status, "active");
    }

    #[test]
    fn goal_pause_and_resume_can_plan_without_existing_goal() {
        let paused = plan_pause_goal_update("thread-a").unwrap();
        assert_eq!(paused.update.thread_id, "thread-a");
        assert_eq!(paused.update.objective, None);
        assert_eq!(paused.update.token_budget, None);
        assert_eq!(paused.update.status, "paused");

        let resumed = plan_resume_goal_update("thread-a").unwrap();
        assert_eq!(resumed.update.thread_id, "thread-a");
        assert_eq!(resumed.update.status, "active");
    }

    #[test]
    fn goal_status_semantics_and_capability_gates_are_core_defined() {
        assert_eq!(
            normalize_goal_status(Some("completed"), None, None),
            "complete"
        );
        assert_eq!(normalize_goal_status(Some("running"), None, None), "active");
        assert_eq!(
            normalize_goal_status(None, Some(false), Some("Keep context")),
            "paused"
        );
        assert_eq!(normalize_goal_status(None, Some(false), None), "cleared");
        assert!(!goal_empty("missing_thread").available);

        let linux = PlatformPaths::for_kind(PlatformKind::Linux);
        let macos = PlatformPaths::for_kind(PlatformKind::Macos);
        let windows = PlatformPaths::for_kind(PlatformKind::Windows);

        let get = plan_goal_get_with_capability(
            &macos,
            GoalGetRequest {
                thread_id: Some(" thread-a ".to_string()),
            },
        )
        .unwrap();
        assert_eq!(get.thread_id.as_deref(), Some("thread-a"));
        assert!(!get.missing_thread);

        let save = plan_goal_save_with_capability(
            &linux,
            GoalUpdateRequest {
                thread_id: Some(" thread-a ".to_string()),
                objective: Some("  Ship  ".to_string()),
                token_budget: Some(10),
                status: Some("running".to_string()),
                enabled: None,
            },
        )
        .unwrap();
        assert_eq!(save.command.command, GoalCommandKind::Save);
        assert_eq!(save.command.update.status, "active");

        assert!(plan_goal_get_with_capability(
            &windows,
            GoalGetRequest {
                thread_id: Some("thread-a".to_string())
            },
        )
        .is_err());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalUpdatePlan {
    pub thread_id: String,
    pub objective: Option<String>,
    pub token_budget: Option<u64>,
    pub status: String,
    pub completed_at: Option<i64>,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalCommandPlan {
    pub command: GoalCommandKind,
    pub update: GoalUpdatePlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalCommandFacadePlan {
    pub required_capability: Capability,
    pub command: GoalCommandPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalGetPlan {
    pub required_capability: Capability,
    pub thread_id: Option<String>,
    pub missing_thread: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GoalView {
    pub available: bool,
    pub enabled: bool,
    pub thread_id: Option<String>,
    pub objective: Option<String>,
    pub token_budget: Option<u64>,
    pub status: String,
    pub completed_at: Option<i64>,
    pub blocked_reason: Option<String>,
    pub raw: Option<GoalRawView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GoalRawView {
    pub source: Option<String>,
    pub thread_id: Option<String>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

pub fn required_thread_id(value: Option<&str>) -> Result<String> {
    let Some(thread_id) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err(anyhow!("thread_id is required"));
    };
    Ok(thread_id.to_string())
}

pub fn plan_save_goal(request: GoalUpdateRequest) -> Result<GoalUpdatePlan> {
    let thread_id = required_thread_id(request.thread_id.as_deref())?;
    let objective = request
        .objective
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let status = normalize_goal_status(
        request.status.as_deref(),
        request.enabled,
        objective.as_deref(),
    );
    let token_budget = objective.as_ref().and(request.token_budget);
    Ok(GoalUpdatePlan {
        thread_id,
        objective,
        token_budget,
        status,
        completed_at: None,
        blocked_reason: None,
    })
}

pub fn plan_goal_update(request: GoalUpdateRequest) -> Result<GoalCommandPlan> {
    Ok(GoalCommandPlan {
        command: GoalCommandKind::Save,
        update: plan_save_goal(request)?,
    })
}

pub fn plan_goal_command_with_capability(
    platform: &PlatformPaths,
    request: GoalUpdateRequest,
) -> Result<GoalCommandFacadePlan> {
    require_capability(platform, Capability::Threads)?;
    Ok(GoalCommandFacadePlan {
        required_capability: Capability::Threads,
        command: plan_goal_update(request)?,
    })
}

pub fn plan_goal_get_with_capability(
    platform: &PlatformPaths,
    request: GoalGetRequest,
) -> Result<GoalGetPlan> {
    require_capability(platform, Capability::Threads)?;
    let thread_id = request
        .thread_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Ok(GoalGetPlan {
        required_capability: Capability::Threads,
        missing_thread: thread_id.is_none(),
        thread_id,
    })
}

pub fn plan_goal_save_with_capability(
    platform: &PlatformPaths,
    request: GoalUpdateRequest,
) -> Result<GoalCommandFacadePlan> {
    plan_goal_command_with_capability(platform, request)
}

pub fn plan_clear_goal(thread_id: &str) -> Result<GoalUpdatePlan> {
    Ok(GoalUpdatePlan {
        thread_id: required_thread_id(Some(thread_id))?,
        objective: None,
        token_budget: None,
        status: "cleared".to_string(),
        completed_at: None,
        blocked_reason: None,
    })
}

pub fn plan_clear_goal_update(thread_id: Option<&str>) -> Result<GoalCommandPlan> {
    Ok(GoalCommandPlan {
        command: GoalCommandKind::Clear,
        update: plan_clear_goal(thread_id.unwrap_or_default())?,
    })
}

pub fn plan_goal_clear_with_capability(
    platform: &PlatformPaths,
    thread_id: Option<&str>,
) -> Result<GoalCommandFacadePlan> {
    require_capability(platform, Capability::Threads)?;
    Ok(GoalCommandFacadePlan {
        required_capability: Capability::Threads,
        command: plan_clear_goal_update(thread_id)?,
    })
}

pub fn plan_pause_goal_update(thread_id: &str) -> Result<GoalCommandPlan> {
    Ok(GoalCommandPlan {
        command: GoalCommandKind::Pause,
        update: plan_goal_status_for_thread(thread_id, "paused")?,
    })
}

pub fn plan_goal_pause_with_capability(
    platform: &PlatformPaths,
    thread_id: &str,
) -> Result<GoalCommandFacadePlan> {
    require_capability(platform, Capability::Threads)?;
    Ok(GoalCommandFacadePlan {
        required_capability: Capability::Threads,
        command: plan_pause_goal_update(thread_id)?,
    })
}

pub fn plan_resume_goal_update(thread_id: &str) -> Result<GoalCommandPlan> {
    Ok(GoalCommandPlan {
        command: GoalCommandKind::Resume,
        update: plan_goal_status_for_thread(thread_id, "active")?,
    })
}

pub fn plan_goal_resume_with_capability(
    platform: &PlatformPaths,
    thread_id: &str,
) -> Result<GoalCommandFacadePlan> {
    require_capability(platform, Capability::Threads)?;
    Ok(GoalCommandFacadePlan {
        required_capability: Capability::Threads,
        command: plan_resume_goal_update(thread_id)?,
    })
}

pub fn plan_goal_status_for_thread(thread_id: &str, status: &str) -> Result<GoalUpdatePlan> {
    let thread_id = required_thread_id(Some(thread_id))?;
    Ok(GoalUpdatePlan {
        thread_id,
        objective: None,
        token_budget: None,
        status: normalize_goal_status(Some(status), None, None),
        completed_at: None,
        blocked_reason: None,
    })
}

pub fn goal_response_from_codex(goal: Option<&CodexThreadGoal>) -> GoalView {
    let Some(goal) = goal else {
        return goal_empty("idle");
    };
    GoalView {
        available: true,
        enabled: true,
        thread_id: Some(goal.thread_id.clone()),
        objective: Some(goal.objective.clone()),
        token_budget: goal.token_budget,
        status: goal.status.clone(),
        completed_at: None,
        blocked_reason: None,
        raw: Some(GoalRawView {
            source: Some("codex_app_server".to_string()),
            thread_id: Some(goal.thread_id.clone()),
            created_at: Some(goal.created_at),
            updated_at: Some(goal.updated_at),
        }),
    }
}

pub async fn execute_goal_get(
    client: &CodexGoalClient,
    codex_home: &Path,
    plan: GoalGetPlan,
) -> Result<GoalView> {
    let Some(thread_id) = plan.thread_id.as_deref() else {
        return Ok(goal_empty("missing_thread"));
    };
    let goal = client
        .execute(codex_home, thread_id, CodexGoalAction::Get)
        .await?;
    Ok(goal_response_from_codex(goal.as_ref()))
}

pub async fn execute_goal_command(
    client: &CodexGoalClient,
    codex_home: &Path,
    command: GoalCommandPlan,
) -> Result<GoalView> {
    let action = match command.command {
        GoalCommandKind::Save => CodexGoalAction::Save {
            objective: command
                .update
                .objective
                .clone()
                .ok_or_else(|| anyhow!("objective is required"))?,
            token_budget: command.update.token_budget,
        },
        GoalCommandKind::Clear => CodexGoalAction::Clear,
        GoalCommandKind::Pause => CodexGoalAction::Pause,
        GoalCommandKind::Resume => CodexGoalAction::Resume,
    };
    let goal = client
        .execute(codex_home, &command.update.thread_id, action)
        .await?;
    Ok(goal_response_from_codex(goal.as_ref()))
}

pub fn goal_empty(status: &str) -> GoalView {
    GoalView {
        available: !matches!(status, "missing_thread" | "unavailable"),
        enabled: false,
        thread_id: None,
        objective: None,
        token_budget: None,
        status: status.to_string(),
        completed_at: None,
        blocked_reason: None,
        raw: None,
    }
}

pub fn normalize_goal_response_value(value: &Value) -> Value {
    let goal = value.get("goal").unwrap_or(value);
    if goal.is_null() {
        return serde_json::to_value(goal_empty("idle")).unwrap_or(Value::Null);
    }
    let status = goal_status_from_value(goal).unwrap_or_else(|| "active".to_string());
    json!({
        "enabled": goal_enabled_from_value(goal, &status),
        "objective": goal.get("objective").and_then(Value::as_str),
        "token_budget": goal.get("tokenBudget").or_else(|| goal.get("token_budget")).and_then(Value::as_u64),
        "status": status,
        "raw": value,
    })
}

fn goal_status_from_value(goal: &Value) -> Option<String> {
    goal.get("status")
        .and_then(|value| {
            value.as_str().or_else(|| {
                value
                    .get("type")
                    .or_else(|| value.get("status"))
                    .or_else(|| value.get("state"))
                    .and_then(Value::as_str)
            })
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

fn goal_enabled_from_value(goal: &Value, status: &str) -> bool {
    if matches!(status, "idle" | "missing_thread" | "cleared") {
        return false;
    }
    let objective = goal
        .get("objective")
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    objective
        || goal
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(matches!(
                status,
                "active" | "running" | "complete" | "completed" | "blocked" | "paused"
            ))
}

pub fn normalize_goal_status(
    status: Option<&str>,
    enabled: Option<bool>,
    objective: Option<&str>,
) -> String {
    let normalized = status
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    match normalized.as_deref() {
        Some("complete" | "completed") => "complete".to_string(),
        Some("blocked") => "blocked".to_string(),
        Some("paused") => "paused".to_string(),
        Some("cleared" | "clear") => "cleared".to_string(),
        Some("idle") => "idle".to_string(),
        Some("active" | "running") => "active".to_string(),
        Some(_) => "active".to_string(),
        None if enabled == Some(false) && objective.is_none() => "cleared".to_string(),
        None if enabled == Some(false) => "paused".to_string(),
        None if objective.is_some() || enabled == Some(true) => "active".to_string(),
        None => "idle".to_string(),
    }
}
