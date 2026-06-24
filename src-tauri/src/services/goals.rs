use crate::overview::DesktopState;
use anyhow::Result;
use nexushub_core::services::goals::{self as goal_service, GoalGetRequest, GoalUpdateRequest};

pub(crate) type DesktopGoalView = goal_service::GoalView;

pub(crate) fn get_goal_with_state(
    state: &DesktopState,
    request: GoalGetRequest,
) -> Result<DesktopGoalView> {
    goal_service::goal_get_response_with_capability(&state.db, state.platform(), request)
}

pub(crate) fn save_goal_with_state(
    state: &DesktopState,
    request: GoalUpdateRequest,
) -> Result<DesktopGoalView> {
    goal_service::save_goal_with_capability(&state.db, state.platform(), request)
}

pub(crate) fn clear_goal_with_state(
    state: &DesktopState,
    request: GoalGetRequest,
) -> Result<DesktopGoalView> {
    goal_service::clear_goal_with_capability(
        &state.db,
        state.platform(),
        request.thread_id.as_deref(),
    )
}

pub(crate) fn pause_goal_with_state(
    state: &DesktopState,
    request: GoalGetRequest,
) -> Result<DesktopGoalView> {
    goal_service::pause_goal_with_capability(
        &state.db,
        state.platform(),
        &goal_service::required_thread_id(request.thread_id.as_deref())?,
    )
}

pub(crate) fn resume_goal_with_state(
    state: &DesktopState,
    request: GoalGetRequest,
) -> Result<DesktopGoalView> {
    goal_service::resume_goal_with_capability(
        &state.db,
        state.platform(),
        &goal_service::required_thread_id(request.thread_id.as_deref())?,
    )
}
