use crate::overview::DesktopState;
use anyhow::Result;
use nexushub_core::services::{
    goals::{self as goal_service, GoalGetRequest, GoalUpdateRequest},
    use_cases::NexusHubUseCases,
};

pub(crate) type DesktopGoalView = goal_service::GoalView;

pub(crate) async fn get_goal_with_state(
    state: &DesktopState,
    request: GoalGetRequest,
) -> Result<DesktopGoalView> {
    let use_cases = NexusHubUseCases::new(state.platform()).goals();
    let plan = use_cases.get(request)?;
    goal_service::execute_goal_get(&state.goal_client, &state.resolved_codex_paths().home, plan)
        .await
}

pub(crate) async fn save_goal_with_state(
    state: &DesktopState,
    request: GoalUpdateRequest,
) -> Result<DesktopGoalView> {
    let use_cases = NexusHubUseCases::new(state.platform()).goals();
    let plan = use_cases.save(request)?;
    goal_service::execute_goal_command(
        &state.goal_client,
        &state.resolved_codex_paths().home,
        plan.command,
    )
    .await
}

pub(crate) async fn clear_goal_with_state(
    state: &DesktopState,
    request: GoalGetRequest,
) -> Result<DesktopGoalView> {
    let use_cases = NexusHubUseCases::new(state.platform()).goals();
    let plan = use_cases.clear(request.thread_id.as_deref())?;
    goal_service::execute_goal_command(
        &state.goal_client,
        &state.resolved_codex_paths().home,
        plan.command,
    )
    .await
}

pub(crate) async fn pause_goal_with_state(
    state: &DesktopState,
    request: GoalGetRequest,
) -> Result<DesktopGoalView> {
    let use_cases = NexusHubUseCases::new(state.platform()).goals();
    let thread_id = goal_service::required_thread_id(request.thread_id.as_deref())?;
    let plan = use_cases.pause(&thread_id)?;
    goal_service::execute_goal_command(
        &state.goal_client,
        &state.resolved_codex_paths().home,
        plan.command,
    )
    .await
}

pub(crate) async fn resume_goal_with_state(
    state: &DesktopState,
    request: GoalGetRequest,
) -> Result<DesktopGoalView> {
    let use_cases = NexusHubUseCases::new(state.platform()).goals();
    let thread_id = goal_service::required_thread_id(request.thread_id.as_deref())?;
    let plan = use_cases.resume(&thread_id)?;
    goal_service::execute_goal_command(
        &state.goal_client,
        &state.resolved_codex_paths().home,
        plan.command,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexushub_core::{
        codex::CodexGoalClient,
        config::Config,
        crypto::SecretBox,
        db::{PanelDb, ThreadGoalUpdate},
        platform::{PlatformKind, PlatformPaths},
    };
    use std::{fs, os::unix::fs::PermissionsExt, time::Duration};

    #[tokio::test]
    async fn desktop_goal_get_uses_app_server_instead_of_shadow_store() {
        let temp = tempfile::tempdir().unwrap();
        let codex_home = temp.path().join("codex-home");
        fs::create_dir_all(codex_home.join("sessions")).unwrap();
        fs::write(codex_home.join("state_5.sqlite"), b"").unwrap();
        fs::write(codex_home.join("session_index.jsonl"), b"").unwrap();
        let executable = temp.path().join("codex");
        fs::write(
            &executable,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'codex-cli 0.144.2'
  exit 0
fi
while IFS= read -r line; do
  case "$line" in
    *'"method":"initialize"'*)
      echo '{"id":1,"result":{"userAgent":"fake","codexHome":"/tmp/codex-home","platformFamily":"unix","platformOs":"macos"}}'
      ;;
    *'"method":"thread/goal/get"'*)
      echo '{"id":2,"result":{"goal":{"threadId":"thread-a","objective":"official desktop goal","status":"usageLimited","tokenBudget":4000,"tokensUsed":4500,"timeUsedSeconds":60,"createdAt":100,"updatedAt":200}}}'
      ;;
  esac
done
"#,
        )
        .unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();

        let mut config = Config::for_platform_kind_with_home(PlatformKind::Macos, temp.path());
        config.paths.db_path = temp.path().join("nexushub.sqlite");
        config.codex.home = codex_home;
        let db =
            PanelDb::open_with_secret_box(&config.paths.db_path, SecretBox::deterministic_dev())
                .unwrap();
        db.upsert_thread_goal(ThreadGoalUpdate {
            thread_id: "thread-a",
            objective: Some("stale shadow goal"),
            token_budget: Some(1),
            status: "paused",
            completed_at: None,
            blocked_reason: None,
        })
        .unwrap();
        let client = CodexGoalClient::with_candidates(vec![executable], Duration::from_secs(10));
        let state = DesktopState::new_with_goal_client(
            config,
            db,
            PlatformPaths::for_kind_with_home(PlatformKind::Macos, temp.path()),
            client,
        );

        let goal = get_goal_with_state(
            &state,
            GoalGetRequest {
                thread_id: Some("thread-a".to_string()),
            },
        )
        .await
        .unwrap();

        assert_eq!(goal.objective.as_deref(), Some("official desktop goal"));
        assert_eq!(goal.status, "usageLimited");
        assert_eq!(
            goal.raw.as_ref().and_then(|raw| raw.source.as_deref()),
            Some("codex_app_server")
        );
    }
}
