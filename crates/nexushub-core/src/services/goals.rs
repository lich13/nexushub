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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GoalCommandKind {
    Save,
    Clear,
    Pause,
    Resume,
}

#[cfg(test)]
mod tests {
    use crate::{
        db::ThreadGoal,
        services::goals::{
            plan_clear_goal_update, plan_goal_update, plan_pause_goal_update,
            plan_resume_goal_update, GoalCommandKind, GoalUpdateRequest,
        },
    };

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

        let existing = thread_goal("thread-a", Some("Keep context"), Some(512), "active");
        let paused = plan_pause_goal_update(" thread-a ", Some(&existing)).unwrap();
        assert_eq!(paused.command, GoalCommandKind::Pause);
        assert_eq!(paused.update.objective.as_deref(), Some("Keep context"));
        assert_eq!(paused.update.token_budget, Some(512));
        assert_eq!(paused.update.status, "paused");

        let resumed = plan_resume_goal_update(" thread-a ", Some(&existing)).unwrap();
        assert_eq!(resumed.command, GoalCommandKind::Resume);
        assert_eq!(resumed.update.status, "active");
    }

    #[test]
    fn goal_pause_and_resume_can_plan_without_existing_goal() {
        let paused = plan_pause_goal_update("thread-a", None).unwrap();
        assert_eq!(paused.update.thread_id, "thread-a");
        assert_eq!(paused.update.objective, None);
        assert_eq!(paused.update.token_budget, None);
        assert_eq!(paused.update.status, "paused");

        let resumed = plan_resume_goal_update("thread-a", None).unwrap();
        assert_eq!(resumed.update.thread_id, "thread-a");
        assert_eq!(resumed.update.status, "active");
    }

    fn thread_goal(
        thread_id: &str,
        objective: Option<&str>,
        token_budget: Option<u64>,
        status: &str,
    ) -> ThreadGoal {
        ThreadGoal {
            thread_id: thread_id.to_string(),
            objective: objective.map(str::to_string),
            token_budget,
            status: status.to_string(),
            created_at: 1,
            updated_at: 2,
            completed_at: None,
            blocked_reason: None,
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalCommandPlan {
    pub command: GoalCommandKind,
    pub update: GoalUpdatePlan,
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

pub fn plan_goal_update(request: GoalUpdateRequest) -> Result<GoalCommandPlan> {
    Ok(GoalCommandPlan {
        command: GoalCommandKind::Save,
        update: plan_save_goal(request)?,
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

pub fn plan_clear_goal_update(thread_id: Option<&str>) -> Result<GoalCommandPlan> {
    Ok(GoalCommandPlan {
        command: GoalCommandKind::Clear,
        update: plan_clear_goal(thread_id.unwrap_or_default())?,
    })
}

pub fn plan_pause_goal(goal: &ThreadGoal) -> GoalUpdatePlan {
    plan_goal_status(goal, "paused")
}

pub fn plan_resume_goal(goal: &ThreadGoal) -> GoalUpdatePlan {
    plan_goal_status(goal, "active")
}

pub fn plan_pause_goal_update(
    thread_id: &str,
    existing: Option<&ThreadGoal>,
) -> Result<GoalCommandPlan> {
    Ok(GoalCommandPlan {
        command: GoalCommandKind::Pause,
        update: plan_goal_status_for_thread(thread_id, existing, "paused")?,
    })
}

pub fn plan_resume_goal_update(
    thread_id: &str,
    existing: Option<&ThreadGoal>,
) -> Result<GoalCommandPlan> {
    Ok(GoalCommandPlan {
        command: GoalCommandKind::Resume,
        update: plan_goal_status_for_thread(thread_id, existing, "active")?,
    })
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
