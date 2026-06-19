use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::db::{ThreadGoal, ThreadGoalUpdate};

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalUpdatePlan {
    pub thread_id: String,
    pub objective: Option<String>,
    pub token_budget: Option<u64>,
    pub status: String,
    pub completed_at: Option<i64>,
    pub blocked_reason: Option<String>,
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

impl GoalUpdatePlan {
    pub fn as_thread_goal_update(&self) -> ThreadGoalUpdate<'_> {
        ThreadGoalUpdate {
            thread_id: &self.thread_id,
            objective: self.objective.as_deref(),
            token_budget: self.token_budget,
            status: &self.status,
            completed_at: self.completed_at,
            blocked_reason: self.blocked_reason.as_deref(),
        }
    }
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

pub fn plan_pause_goal(goal: &ThreadGoal) -> GoalUpdatePlan {
    plan_goal_status(goal, "paused")
}

pub fn plan_resume_goal(goal: &ThreadGoal) -> GoalUpdatePlan {
    plan_goal_status(goal, "active")
}

pub fn plan_goal_status_for_thread(
    thread_id: &str,
    existing: Option<&ThreadGoal>,
    status: &str,
) -> Result<GoalUpdatePlan> {
    let thread_id = required_thread_id(Some(thread_id))?;
    Ok(GoalUpdatePlan {
        thread_id,
        objective: existing.and_then(|goal| goal.objective.clone()),
        token_budget: existing.and_then(|goal| goal.token_budget),
        status: normalize_goal_status(
            Some(status),
            None,
            existing.and_then(|goal| goal.objective.as_deref()),
        ),
        completed_at: None,
        blocked_reason: None,
    })
}

pub fn goal_response(goal: Option<&ThreadGoal>) -> GoalView {
    let Some(goal) = goal else {
        return goal_empty("idle");
    };
    GoalView {
        available: true,
        enabled: goal_enabled(goal),
        thread_id: Some(goal.thread_id.clone()),
        objective: goal.objective.clone(),
        token_budget: goal.token_budget,
        status: goal.status.clone(),
        completed_at: goal.completed_at,
        blocked_reason: goal.blocked_reason.clone(),
        raw: Some(GoalRawView {
            source: Some("local".to_string()),
            thread_id: Some(goal.thread_id.clone()),
            created_at: Some(goal.created_at),
            updated_at: Some(goal.updated_at),
        }),
    }
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

pub fn goal_enabled(goal: &ThreadGoal) -> bool {
    if matches!(goal.status.as_str(), "idle" | "missing_thread" | "cleared") {
        return false;
    }
    goal.objective
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
        || matches!(
            goal.status.as_str(),
            "active" | "running" | "complete" | "completed" | "blocked" | "paused"
        )
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

fn plan_goal_status(goal: &ThreadGoal, status: &str) -> GoalUpdatePlan {
    GoalUpdatePlan {
        thread_id: goal.thread_id.clone(),
        objective: goal.objective.clone(),
        token_budget: goal.token_budget,
        status: normalize_goal_status(Some(status), None, goal.objective.as_deref()),
        completed_at: None,
        blocked_reason: None,
    }
}
