use crate::overview::DesktopState;
use anyhow::Result;
use nexushub_core::services::goals::{self as goal_service, GoalUpdateRequest};

pub(crate) type DesktopGoalView = goal_service::GoalView;

pub(crate) fn get_goal_with_state(
    state: &DesktopState,
    thread_id: Option<String>,
) -> Result<DesktopGoalView> {
    goal_service::goal_get_response_with_capability(
        &state.db,
        state.platform(),
        goal_service::GoalGetRequest { thread_id },
    )
}

pub(crate) fn save_goal_from_parts_with_state(
    state: &DesktopState,
    thread_id: Option<String>,
    objective: Option<String>,
    token_budget: Option<u64>,
) -> Result<DesktopGoalView> {
    goal_service::save_goal_with_capability(
        &state.db,
        state.platform(),
        GoalUpdateRequest {
            thread_id,
            objective,
            token_budget,
            status: None,
            enabled: None,
        },
    )
}

pub(crate) fn clear_goal_from_parts_with_state(
    state: &DesktopState,
    thread_id: Option<String>,
) -> Result<DesktopGoalView> {
    goal_service::clear_goal_with_capability(&state.db, state.platform(), thread_id.as_deref())
}

pub(crate) fn pause_goal_from_parts_with_state(
    state: &DesktopState,
    thread_id: Option<String>,
) -> Result<DesktopGoalView> {
    goal_service::pause_goal_with_capability(
        &state.db,
        state.platform(),
        &goal_service::required_thread_id(thread_id.as_deref())?,
    )
}

pub(crate) fn resume_goal_from_parts_with_state(
    state: &DesktopState,
    thread_id: Option<String>,
) -> Result<DesktopGoalView> {
    goal_service::resume_goal_with_capability(
        &state.db,
        state.platform(),
        &goal_service::required_thread_id(thread_id.as_deref())?,
    )
}
