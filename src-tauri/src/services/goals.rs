use crate::overview::DesktopState;
use anyhow::Result;
use nexushub_core::services::{
    goals::{self as goal_service, GoalGetRequest, GoalUpdateRequest},
    use_cases::NexusHubUseCases,
};

pub(crate) type DesktopGoalView = goal_service::GoalView;

pub(crate) fn get_goal_with_state(
    state: &DesktopState,
    request: GoalGetRequest,
) -> Result<DesktopGoalView> {
    let use_cases = NexusHubUseCases::new(state.platform()).goals();
    let plan = use_cases.get(request)?;
    let Some(thread_id) = plan.thread_id.as_deref() else {
        return Ok(goal_service::goal_empty("missing_thread"));
    };
    Ok(goal_service::goal_response(
        state.db.get_thread_goal(thread_id)?.as_ref(),
    ))
}

pub(crate) fn save_goal_with_state(
    state: &DesktopState,
    request: GoalUpdateRequest,
) -> Result<DesktopGoalView> {
    let use_cases = NexusHubUseCases::new(state.platform()).goals();
    let plan = use_cases.save(request)?;
    use_cases.apply(&state.db, plan.command)
}

pub(crate) fn clear_goal_with_state(
    state: &DesktopState,
    request: GoalGetRequest,
) -> Result<DesktopGoalView> {
    let use_cases = NexusHubUseCases::new(state.platform()).goals();
    let plan = use_cases.clear(request.thread_id.as_deref())?;
    use_cases.apply(&state.db, plan.command)
}

pub(crate) fn pause_goal_with_state(
    state: &DesktopState,
    request: GoalGetRequest,
) -> Result<DesktopGoalView> {
    let use_cases = NexusHubUseCases::new(state.platform()).goals();
    let thread_id = goal_service::required_thread_id(request.thread_id.as_deref())?;
    let existing = state.db.get_thread_goal(&thread_id)?;
    let plan = use_cases.pause(&thread_id, existing.as_ref())?;
    use_cases.apply(&state.db, plan.command)
}

pub(crate) fn resume_goal_with_state(
    state: &DesktopState,
    request: GoalGetRequest,
) -> Result<DesktopGoalView> {
    let use_cases = NexusHubUseCases::new(state.platform()).goals();
    let thread_id = goal_service::required_thread_id(request.thread_id.as_deref())?;
    let existing = state.db.get_thread_goal(&thread_id)?;
    let plan = use_cases.resume(&thread_id, existing.as_ref())?;
    use_cases.apply(&state.db, plan.command)
}
