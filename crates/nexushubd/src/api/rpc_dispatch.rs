use super::{
    answer_approval, answer_elicitation, api_error, archive_delete_dry_run, archive_delete_execute,
    archive_thread, cancel_followup, change_password, claude_code_overview, codex_config,
    codex_goal_clear, codex_goal_get, codex_goal_pause, codex_goal_resume, codex_goal_set,
    codex_models, codex_permission_profiles, create_thread, delete_upload_file, enqueue_followup,
    fork_thread, get_probe_events, get_probe_logs_db_status, get_probe_settings, get_probe_status,
    get_security, hidden_threads_delete_dry_run, hidden_threads_delete_execute, job_detail,
    list_followups, list_jobs, list_plugins, list_providers, login, logout, me,
    patch_probe_settings, patch_security, plan_accept, plan_revise, platform_overview,
    public_settings, rename_thread, restore_thread, send_message, start_probe_action,
    start_update_action, steer_thread, stop_thread, system_status, system_update_status,
    system_version, thread_blocks, thread_detail, ApiResponse, ArchiveExecuteRequest, GoalQuery,
    ProbeEventsQuery, ProbeStatusQuery,
};
use crate::{
    api::payload::{
        rpc_nested_payload, rpc_nested_payload_or_empty, rpc_payload, rpc_payload_or_empty,
        rpc_query_strings, rpc_required_string, rpc_string, rpc_wrapped_payload,
    },
    rpc_surface::{is_business_rpc_command, is_retired_rpc_command, is_transport_rpc_command},
    state::AppState,
};
use axum::{
    extract::connect_info::ConnectInfo,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use nexushub_core::services::{
    commands as rpc_commands,
    probe::{self as probe_service},
    updates::UpdateAction,
};
use serde_json::Value;
use std::net::SocketAddr;

pub(super) async fn rpc_dispatch(
    State(state): State<AppState>,
    Path(command): Path<String>,
    connect: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(args): Json<Value>,
) -> ApiResponse {
    if is_transport_rpc_command(&command) {
        return Err(api_error(
            StatusCode::NOT_FOUND,
            &format!("transport endpoint is not a business rpc command: {command}"),
        ));
    }
    if is_retired_rpc_command(&command) {
        return Err(api_error(
            StatusCode::NOT_FOUND,
            &format!("retired rpc command: {command}"),
        ));
    }
    if !is_business_rpc_command(&command) {
        return Err(api_error(
            StatusCode::NOT_FOUND,
            &format!("unknown rpc command: {command}"),
        ));
    }

    match command.as_str() {
        rpc_commands::AUTH_PUBLIC_SETTINGS => public_settings(State(state)).await,
        rpc_commands::AUTH_LOGIN => {
            login(
                State(state),
                connect,
                headers,
                Json(rpc_wrapped_payload(&args, &["payload", "request"])?),
            )
            .await
        }
        rpc_commands::AUTH_LOGOUT => logout(State(state), headers).await,
        rpc_commands::AUTH_ME => me(State(state), headers).await,
        rpc_commands::SECURITY_GET => get_security(State(state), headers).await,
        rpc_commands::SECURITY_SAVE => {
            patch_security(
                State(state),
                headers,
                Json(rpc_nested_payload(&args, "settings")?),
            )
            .await
        }
        rpc_commands::SECURITY_CHANGE_PASSWORD => {
            change_password(State(state), headers, Json(rpc_payload(&args)?)).await
        }
        rpc_commands::SYSTEM_PROVIDERS => list_providers(State(state), headers).await,
        rpc_commands::SYSTEM_CLAUDE_CODE_OVERVIEW => {
            claude_code_overview(State(state), headers).await
        }
        rpc_commands::SYSTEM_PLATFORM => platform_overview(State(state), headers).await,
        rpc_commands::SYSTEM_PLUGINS => list_plugins(State(state), headers).await,
        rpc_commands::PROBE_STATUS => {
            get_probe_status(
                State(state),
                Query(ProbeStatusQuery {
                    refresh: args.get("refresh").and_then(Value::as_bool),
                }),
                headers,
            )
            .await
        }
        rpc_commands::PROBE_SETTINGS_GET => get_probe_settings(State(state), headers).await,
        rpc_commands::PROBE_SETTINGS_SAVE => {
            patch_probe_settings(
                State(state),
                headers,
                Json(rpc_wrapped_payload(
                    &args,
                    &["settings", "payload", "request"],
                )?),
            )
            .await
        }
        rpc_commands::PROBE_LOGS_DB_STATUS => get_probe_logs_db_status(State(state), headers).await,
        rpc_commands::PROBE_EVENTS => {
            get_probe_events(
                State(state),
                Query(ProbeEventsQuery {
                    limit: args
                        .get("limit")
                        .and_then(Value::as_u64)
                        .map(|value| value as u32),
                }),
                headers,
            )
            .await
        }
        rpc_commands::PROBE_BARK_TEST => {
            start_probe_action(state, headers, probe_service::ProbeAction::BarkTest).await
        }
        rpc_commands::PROBE_INSTALL_HOOKS => {
            start_probe_action(state, headers, probe_service::ProbeAction::InstallHooks).await
        }
        rpc_commands::PROBE_LOGS_DB_DRY_RUN => {
            start_probe_action(state, headers, probe_service::ProbeAction::LogsDbDryRun).await
        }
        rpc_commands::PROBE_LOGS_DB_EXECUTE => {
            start_probe_action(state, headers, probe_service::ProbeAction::LogsDbExecute).await
        }
        rpc_commands::CLEANUP_ARCHIVE_DRY_RUN => {
            archive_delete_dry_run(State(state), headers).await
        }
        rpc_commands::CLEANUP_ARCHIVE_EXECUTE => {
            archive_delete_execute(
                State(state),
                headers,
                Json(ArchiveExecuteRequest { confirmed: true }),
            )
            .await
        }
        rpc_commands::CLEANUP_HIDDEN_DRY_RUN => {
            hidden_threads_delete_dry_run(State(state), headers).await
        }
        rpc_commands::CLEANUP_HIDDEN_EXECUTE => {
            hidden_threads_delete_execute(
                State(state),
                headers,
                Json(ArchiveExecuteRequest { confirmed: true }),
            )
            .await
        }
        rpc_commands::UPDATES_STATUS => system_update_status(State(state), headers).await,
        rpc_commands::UPDATES_CHECK => {
            start_update_action(state, headers, UpdateAction::Check, None).await
        }
        rpc_commands::UPDATES_INSTALL => {
            start_update_action(
                state,
                headers,
                UpdateAction::Install,
                Some("nexushub.update.install_started"),
            )
            .await
        }
        rpc_commands::UPDATES_PRUNE => {
            start_update_action(
                state,
                headers,
                UpdateAction::Prune,
                Some("nexushub.update.prune_started"),
            )
            .await
        }
        rpc_commands::THREADS_LIST => {
            super::list_threads(State(state), headers, Query(rpc_payload(&args)?)).await
        }
        rpc_commands::THREADS_DETAIL => {
            thread_detail(
                State(state),
                headers,
                Path(rpc_required_string(&args, "id")?),
                Query(rpc_nested_payload_or_empty(&args, "options")?),
            )
            .await
        }
        rpc_commands::THREADS_BLOCKS => {
            thread_blocks(
                State(state),
                headers,
                Path(rpc_required_string(&args, "id")?),
                Query(rpc_nested_payload_or_empty(&args, "options")?),
            )
            .await
        }
        rpc_commands::THREADS_CREATE => {
            create_thread(
                State(state),
                headers,
                Json(rpc_wrapped_payload(&args, &["payload", "request"])?),
            )
            .await
        }
        rpc_commands::THREADS_SEND => {
            send_message(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_wrapped_payload(&args, &["payload", "request"])?),
            )
            .await
        }
        rpc_commands::THREADS_STEER => {
            steer_thread(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_wrapped_payload(&args, &["payload", "request"])?),
            )
            .await
        }
        rpc_commands::THREADS_FOLLOWUPS_LIST => {
            list_followups(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
            )
            .await
        }
        rpc_commands::THREADS_FOLLOWUPS_ENQUEUE => {
            enqueue_followup(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_wrapped_payload(&args, &["payload", "request"])?),
            )
            .await
        }
        rpc_commands::THREADS_FOLLOWUPS_CANCEL => {
            cancel_followup(
                State(state),
                headers,
                Path((
                    rpc_required_string(&args, "threadId")?,
                    rpc_required_string(&args, "followUpId")?,
                )),
            )
            .await
        }
        rpc_commands::THREADS_STOP => {
            stop_thread(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Some(Json(rpc_nested_payload_or_empty(&args, "payload")?)),
            )
            .await
        }
        rpc_commands::THREADS_ARCHIVE => {
            archive_thread(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
            )
            .await
        }
        rpc_commands::THREADS_RESTORE => {
            restore_thread(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
            )
            .await
        }
        rpc_commands::THREADS_RENAME => {
            rename_thread(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_payload(&args)?),
            )
            .await
        }
        rpc_commands::THREADS_FORK => {
            fork_thread(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
            )
            .await
        }
        rpc_commands::THREADS_PLAN_ACCEPT => {
            plan_accept(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_nested_payload(&args, "payload")?),
            )
            .await
        }
        rpc_commands::THREADS_PLAN_REVISE => {
            plan_revise(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_nested_payload(&args, "payload")?),
            )
            .await
        }
        rpc_commands::THREADS_ELICITATION_ANSWER => {
            answer_elicitation(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_payload(&args)?),
            )
            .await
        }
        rpc_commands::THREADS_APPROVAL_ANSWER => {
            answer_approval(
                State(state),
                headers,
                Path(rpc_required_string(&args, "threadId")?),
                Json(rpc_nested_payload(&args, "payload")?),
            )
            .await
        }
        rpc_commands::UPLOADS_DELETE => {
            delete_upload_file(
                State(state),
                headers,
                Path(rpc_required_string(&args, "id")?),
            )
            .await
        }
        rpc_commands::SYSTEM_STATUS => system_status(State(state), headers).await,
        rpc_commands::SYSTEM_VERSION => system_version(State(state), headers).await,
        rpc_commands::SYSTEM_MODELS => codex_models(State(state), headers).await,
        rpc_commands::SYSTEM_PERMISSION_PROFILES => {
            codex_permission_profiles(State(state), headers, Query(rpc_payload_or_empty(&args)?))
                .await
        }
        rpc_commands::SYSTEM_CODEX_CONFIG => {
            codex_config(State(state), headers, Query(rpc_payload_or_empty(&args)?)).await
        }
        rpc_commands::THREADS_GOAL_GET => {
            codex_goal_get(
                State(state),
                headers,
                Query(GoalQuery {
                    thread_id: rpc_string(&args, "threadId"),
                }),
            )
            .await
        }
        rpc_commands::THREADS_GOAL_SAVE => {
            codex_goal_set(
                State(state),
                headers,
                Json(rpc_wrapped_payload(&args, &["request", "payload"])?),
            )
            .await
        }
        rpc_commands::THREADS_GOAL_CLEAR => {
            codex_goal_clear(
                State(state),
                headers,
                Json(rpc_wrapped_payload(&args, &["request", "payload"])?),
            )
            .await
        }
        rpc_commands::THREADS_GOAL_PAUSE => {
            codex_goal_pause(
                State(state),
                headers,
                Json(rpc_wrapped_payload(&args, &["request", "payload"])?),
            )
            .await
        }
        rpc_commands::THREADS_GOAL_RESUME => {
            codex_goal_resume(
                State(state),
                headers,
                Json(rpc_wrapped_payload(&args, &["request", "payload"])?),
            )
            .await
        }
        rpc_commands::JOBS_LIST => {
            list_jobs(
                State(state),
                headers,
                Query(rpc_query_strings(&args, &["limit"])),
            )
            .await
        }
        rpc_commands::JOBS_DETAIL => {
            job_detail(
                State(state),
                headers,
                Path(rpc_required_string(&args, "id")?),
            )
            .await
        }
        _ => Err(api_error(
            StatusCode::NOT_FOUND,
            &format!("rpc command allowlist drifted without handler mapping: {command}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use crate::{api::routes::router, state::AppState};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use nexushub_core::{
        config::Config,
        db::{NewSession, PanelDb},
    };
    use tower::ServiceExt;

    fn authenticated_test_state() -> (AppState, String, String) {
        let mut config = Config::default();
        config.security.cookie_secure = false;

        let db = PanelDb::open(":memory:").unwrap();
        db.upsert_admin("admin-id", "admin", "hash").unwrap();
        db.create_session(NewSession {
            id: "session-id",
            admin_id: "admin-id",
            token: "session-token",
            csrf_token: "csrf-token",
            user_agent: None,
            ip: None,
            expires_at: PanelDb::now() + 3_600,
        })
        .unwrap();

        (
            AppState::new(config, db),
            "session-token".to_string(),
            "csrf-token".to_string(),
        )
    }

    async fn request_rpc_status(
        app: axum::Router,
        command: &str,
        body: &str,
        session_token: Option<&str>,
        csrf_token: Option<&str>,
    ) -> StatusCode {
        let mut builder = Request::builder()
            .method("POST")
            .uri(format!("/api/rpc/{command}"))
            .header("content-type", "application/json");
        if let Some(session_token) = session_token {
            builder = builder.header("cookie", format!("nexushub_session={session_token}"));
        }
        if let Some(csrf_token) = csrf_token {
            builder = builder.header("x-csrf-token", csrf_token);
        }
        app.oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn retired_rpc_commands_are_rejected_even_when_authenticated() {
        for command in [
            "startProbeJob",
            "runUpdateAction",
            "getDesktopOverview",
            "getDesktopPlatformStatus",
            "stopThread",
            "archiveThread",
            "restoreThread",
            "renameThread",
            "listFollowUps",
            "enqueueFollowUp",
            "cancelFollowUp",
            "listJobs",
            "getJob",
            "deleteArchivedThreadsDryRun",
            "deleteArchivedThreadsExecute",
            "deleteHiddenThreadsDryRun",
            "deleteHiddenThreadsExecute",
        ] {
            let (state, session_token, csrf_token) = authenticated_test_state();
            let status = request_rpc_status(
                router(state),
                command,
                "{}",
                Some(&session_token),
                Some(&csrf_token),
            )
            .await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{command}");
        }
    }

    #[test]
    fn rpc_dispatch_hard_deletes_string_action_compat_commands() {
        let source = include_str!("rpc_dispatch.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("rpc dispatcher source must include production section");
        for typed in [
            "rpc_commands::PROBE_BARK_TEST",
            "rpc_commands::PROBE_INSTALL_HOOKS",
            "rpc_commands::PROBE_LOGS_DB_DRY_RUN",
            "rpc_commands::PROBE_LOGS_DB_EXECUTE",
            "rpc_commands::UPDATES_CHECK",
            "rpc_commands::UPDATES_INSTALL",
            "rpc_commands::UPDATES_PRUNE",
        ] {
            assert!(
                source.contains(typed),
                "RPC dispatcher must expose typed command {typed}"
            );
        }
        for compat in [
            "\"startProbeJob\"",
            "\"runUpdateAction\"",
            "\"startProbeBarkTest\"",
            "\"startProbeHooksInstall\"",
            "\"startProbeLogsDbDryRun\"",
            "\"startProbeLogsDbExecute\"",
            "\"checkUpdate\"",
            "\"installUpdateAndRestart\"",
            "\"stopThread\"",
            "\"archiveThread\"",
            "\"restoreThread\"",
            "\"renameThread\"",
            "\"listFollowUps\"",
            "\"enqueueFollowUp\"",
            "\"cancelFollowUp\"",
            "\"listJobs\"",
            "\"getJob\"",
            "\"deleteArchivedThreadsDryRun\"",
            "\"deleteArchivedThreadsExecute\"",
            "\"deleteHiddenThreadsDryRun\"",
            "\"deleteHiddenThreadsExecute\"",
        ] {
            assert!(
                !source.contains(compat),
                "{compat} must not remain in the production RPC dispatcher"
            );
        }
        assert!(
            source.contains("is_retired_rpc_command"),
            "RPC dispatcher should consult retired command guard rails"
        );
        assert!(
            source.contains("is_business_rpc_command"),
            "RPC dispatcher should consult shared business RPC allowlist"
        );
        assert!(
            !source.contains("/api/rpc/uploadFiles"),
            "RPC dispatcher should keep upload transport out of business command dispatch"
        );
        assert!(
            !source.contains("/api/rpc/threadEvents/:id"),
            "RPC dispatcher should keep thread event transport out of business command dispatch"
        );
    }
}
