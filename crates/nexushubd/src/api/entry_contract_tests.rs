use std::{fs, path::PathBuf};

fn src(path: &str) -> String {
    fs::read_to_string(src_path(path)).unwrap_or_default()
}

fn src_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(path)
}

fn production_section(source: &str) -> &str {
    source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("source should have a production section")
}

fn assert_absent(source: &str, needles: &[&str], context: &str) {
    for needle in needles {
        assert!(
            !source.contains(needle),
            "{context}: forbidden production source fragment remained: {needle}"
        );
    }
}

fn assert_present(source: &str, needles: &[&str], context: &str) {
    for needle in needles {
        assert!(
            source.contains(needle),
            "{context}: expected production source fragment missing: {needle}"
        );
    }
}

#[test]
fn api_entry_delegates_transport_dispatch_and_payload_to_submodules() {
    let api = src("api.rs");
    assert_present(
        &api,
        &[
            "mod routes;",
            "mod rpc_dispatch;",
            "mod payload;",
            "mod cleanup;",
            "mod goals;",
            "mod jobs;",
            "mod probe;",
            "mod security;",
            "mod system;",
            "mod uploads;",
            "mod web_auth;",
        ],
        "api.rs should declare thin API submodules",
    );

    let production = production_section(&api);
    assert_absent(
        production,
        &[
            "Router::new()",
            "async fn rpc_dispatch",
            "fn rpc_payload<",
            "fn rpc_wrapped_payload<",
            "fn rpc_nested_payload<",
            "fn rpc_required_string(",
            "async fn get_probe_status",
            "async fn get_probe_settings",
            "async fn patch_probe_settings",
            "async fn get_probe_events",
            "async fn get_probe_logs_db_status",
            "async fn start_probe_action",
            "async fn probe_status_cached_value",
            "fn probe_settings_value(",
            "fn redact_probe_event(",
            "async fn public_settings",
            "async fn get_security",
            "async fn patch_security",
            "async fn change_password",
            "fn security_response(",
            "async fn upload_files",
            "async fn delete_upload_file",
            "fn upload_service_error(",
            "async fn login",
            "async fn logout",
            "async fn me",
            "fn client_ip(",
            "enum TurnstileLoginAction",
            "fn turnstile_login_action(",
            "async fn system_status",
            "async fn system_version",
            "async fn system_update_status",
            "async fn http_version_info",
            "async fn github_latest_release",
            "async fn npm_latest_version",
            "struct CwdQuery",
            "async fn codex_models",
            "async fn codex_permission_profiles",
            "async fn codex_config",
            "async fn start_update_action",
            "async fn list_jobs",
            "async fn job_detail",
            "async fn archive_delete_dry_run",
            "struct ArchiveExecuteRequest",
            "async fn archive_delete_execute",
            "async fn hidden_threads_delete_dry_run",
            "async fn hidden_threads_delete_execute",
            "struct GoalQuery",
            "async fn codex_goal_get",
            "async fn codex_goal_set",
            "async fn codex_goal_clear",
            "async fn codex_goal_pause",
            "async fn codex_goal_resume",
        ],
        "api.rs should delegate transport/dispatch/payload concerns",
    );
}

#[test]
fn api_entry_does_not_reimplement_domain_or_linux_execution_boundaries() {
    let api = src("api.rs");
    let production = production_section(&api);
    let probe_api = src("api/probe.rs");
    let probe_production = production_section(&probe_api);
    let security_api = src("api/security.rs");
    let security_production = production_section(&security_api);
    let system_api = src("api/system.rs");
    let system_production = production_section(&system_api);
    let jobs_api = src("api/jobs.rs");
    let jobs_production = production_section(&jobs_api);
    let cleanup_api = src("api/cleanup.rs");
    let cleanup_production = production_section(&cleanup_api);
    let goals_api = src("api/goals.rs");
    let goals_production = production_section(&goals_api);
    let combined_production = format!(
        "{production}\n{probe_production}\n{security_production}\n{system_production}\n{jobs_production}\n{cleanup_production}\n{goals_production}"
    );
    let adapter = src("linux_adapter.rs");

    assert_present(
        production,
        &[
            "NexusHubUseCases::new",
            "linux_adapter::list_threads_read_model",
            "linux_adapter::window_thread_detail_read_model",
            "linux_adapter::thread_blocks_read_model",
        ],
        "api.rs should call the core/linux adapter boundary",
    );
    assert_present(
        cleanup_production,
        &["linux_adapter::execute_cleanup_plan"],
        "api/cleanup.rs should call the fixed Linux cleanup execution landing",
    );
    assert_present(
        jobs_production,
        &[
            "linux_adapter::list_jobs_plan",
            "linux_adapter::job_detail_plan",
        ],
        "api/jobs.rs should call the fixed Linux job read-model landing",
    );
    assert_present(
        &combined_production,
        &["NexusHubUseCases::with_config", ".security()?"],
        "api security/probe submodules should call config-backed core facades",
    );

    assert_absent(
        &combined_production,
        &[
            "state.db.claim_next_pending_followup(",
            "state.db.mark_followup_submitted(",
            "state.db.mark_followup_error(",
            "state.db.running_thread_jobs(",
            "state.db.running_job_for_thread(",
            "state.db.list_jobs(",
            "state.db.job(",
            "job_responses(",
            "fn job_response(",
            "update::analyze_job_failure(",
            "cleanup_service::execute_archived_with_capability(",
            "cleanup_service::execute_hidden_with_capability(",
            "security_service::plan_security_patch_with_capability(",
            "security_service::security_view_with_capability(",
            "if plan.requires_confirmation && !payload.confirmed",
            "build_threads_overview(",
            "merge_running_jobs(",
            "apply_running_job_to_summary(",
            "linux_adapter::autosubmit_ready_followups(",
            "linux_adapter::autosubmit_pending_followup(",
            "normalize_thread_detail_block_limit(",
            "normalize_thread_block_limit(",
        ],
        "api.rs should delegate domain orchestration to core/linux_adapter",
    );

    assert_present(
        &adapter,
        &[
            "claim_next_followup(",
            "apply_followup_submitted(",
            "apply_followup_error(",
            "state.jobs.start_codex_job(",
            "codex::set_thread_archived(",
            "NexusHubUseCases::new(&platform).cleanup()",
            ".execute_archived(",
            ".execute_hidden(",
        ],
        "linux_adapter should keep the minimal fixed side-effect landing",
    );
}

#[test]
fn api_entry_does_not_orchestrate_thread_job_goal_followup_or_cleanup_business_state() {
    let api = src("api.rs");
    let production = production_section(&api);
    let goals_api = src("api/goals.rs");
    let goals_production = production_section(&goals_api);

    assert_present(
        production,
        &[
            "NexusHubUseCases::new(&platform)",
            "linux_adapter::start_thread_command_execution_plan",
            "linux_adapter::enqueue_followup_plan",
            "linux_adapter::resolve_thread_stop_plan",
            "linux_adapter::start_codex_resume_action",
        ],
        "api.rs should stay at auth/payload/core-plan/adapter-call level",
    );
    assert_present(
        goals_production,
        &[
            "NexusHubUseCases::new(&platform)",
            "linux_adapter::goal_get_plan",
            "linux_adapter::apply_goal_command_plan",
        ],
        "api/goals.rs should stay at auth/payload/core-plan/adapter-call level",
    );

    assert_absent(
        production,
        &[
            "job_service::build_codex_job_spec(",
            "job_service::enqueue_planned_followup(",
            "job_service::codex_action_submitted(",
            "job_service::resolve_thread_stop_job(",
            "job_service::followup_view(",
            "job_service::followup_views(",
            "job_service::thread_stop_response(",
            "job_service::cancel_followup_response(",
            "goal_service::goal_get_response_with_capability(",
            "goal_service::save_goal_with_capability(",
            "goal_service::clear_goal_with_capability(",
            "goal_service::pause_goal_with_capability(",
            "goal_service::resume_goal_with_capability(",
            "state.db.record_audit(\n        Some(&auth.admin_id),\n        \"thread.",
            "state.db.record_audit(\n        Some(&auth.admin_id),\n        \"archives.",
            "state.db.record_audit(\n        Some(&auth.admin_id),\n        \"hidden_threads.",
        ],
        "api.rs must not own thread/job/read-model/follow-up/cleanup business orchestration",
    );
}

#[test]
fn linux_adapter_executes_core_plans_without_defining_business_semantics() {
    let adapter = src("linux_adapter.rs");
    let production = production_section(&adapter);

    assert_present(
        production,
        &[
            "ThreadCommandExecutionPlan",
            "FollowUpAutoSubmitExecutionPlan",
            "GoalCommandPlan",
            "GoalGetPlan",
            "CleanupOperationPlan",
            "UploadRetentionPlan",
            "UploadStorePlan",
            "UploadDeletePlan",
            ".submitted_response(&job_id)",
            ".audit_detail(&job_id)",
        ],
        "linux_adapter should execute core-authored plans",
    );

    assert_absent(
        production,
        &[
            "\"Codex new thread\"",
            "\"Codex resume thread\"",
            "\"Codex queued follow-up\"",
            "\"pending\".to_string()",
            "\"submitting\".to_string()",
            "\"submitted\".to_string()",
            "\"thread.followup.enqueued\"",
            "\"thread.followup.enqueued_after_steer_fallback\"",
            "job_service::plan_followup_autosubmit(",
            "job_service::plan_queued_followup_job_spec(",
            "job_service::codex_action_submitted(",
            "job_service::thread_state_action_response(",
            "job_service::followup_view(",
            "job_service::followup_views(",
            "job_service::cancel_followup_response(",
            "job_service::thread_stop_response(",
            "cleanup_confirmation_message(",
            "archive deletion must be confirmed",
            "hidden thread deletion must be confirmed",
            "cleanup expectedCount is required before deletion",
            "cleanup_service::dry_run_archived_with_capability(",
            "cleanup_service::execute_archived_with_capability(",
            "cleanup_service::dry_run_hidden_with_capability(",
            "cleanup_service::execute_hidden_with_capability(",
            "cleanup_service::validate_cleanup_expected_count(",
            "ThreadStatus::Running",
            "active_job_id = Some(job_id)",
        ],
        "linux_adapter must not define business state semantics while landing core plans",
    );

    assert_present(
        production,
        &[
            "state.db.link_job_thread(",
            "state.db.record_audit(",
            "state.jobs.start_codex_job(",
            "state.jobs.cancel_job(",
            ".list_jobs(",
            ".job(",
            "NexusHubUseCases::new(&platform).threads()",
            ".apply_cancel_followup(",
            ".get_thread_goal(",
            ".apply(&state.db, command)",
            "codex::set_thread_archived(",
            "codex::set_thread_title(",
            "NexusHubUseCases::new(&platform).cleanup()",
            ".dry_run_archived(",
            ".execute_archived(",
            ".dry_run_hidden(",
            ".execute_hidden(",
            ".validate_expected_count(",
            "upload_service::store_upload_plan(",
            "upload_service::execute_delete_upload_plan(",
            "upload_service::execute_upload_retention_plan(",
        ],
        "linux_adapter should expose only the fixed Linux DB/job/Codex/upload side-effect landings",
    );
    assert_absent(
        production,
        &["state.db.list_followups(", "state.db.cancel_followup("],
        "linux_adapter should execute follow-up effects through core use-case facade",
    );
}

#[test]
fn linux_adapter_api_and_probe_daemon_do_not_reintroduce_read_model_or_probe_business_decisions() {
    let adapter = src("linux_adapter.rs");
    let adapter_production = production_section(&adapter);
    let probe_api = src("api/probe.rs");
    let probe_api_production = production_section(&probe_api);
    let daemon = src("main.rs");
    let daemon_production = production_section(&daemon);
    let core_probe = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../nexushub-core/src/services/probe.rs"),
    )
    .unwrap_or_default();

    assert_present(
        adapter_production,
        &[
            "thread_service::thread_list_read_model",
            "thread_service::thread_detail_read_model",
            "execute_autosubmit_effects",
        ],
        "linux_adapter should pass local reads into core read-model/effect plans",
    );
    assert_absent(
        adapter_production,
        &[
            "fn autosubmit_ready_followups",
            "fn autosubmit_pending_followup",
            "if !matches!(detail.summary.status, ThreadStatus::Recent)",
            "if !matches!(summary.status, ThreadStatus::Recent)",
            "state.db.claim_next_pending_followup(",
            "state.db.mark_followup_submitted(",
            "state.db.mark_followup_error(",
            "running_job_for_thread(&detail.summary.id)",
            "build_threads_overview(",
            "apply_thread_detail_runtime_state(",
        ],
        "linux_adapter must not define follow-up state transitions or thread read-model merge semantics",
    );

    assert_present(
        probe_api_production,
        &[
            "probe_service::probe_logs_db_status_view",
            "probe_service::probe_status_snapshot_view",
        ],
        "api/probe.rs should use core Probe views for logs-db and snapshot metadata",
    );
    assert_absent(
        probe_api_production,
        &[
            "fn probe_logs_db_last_result",
            "\"last_run\".to_string()",
            "\"next_run\".to_string()",
            "\"snapshot_age_seconds\".to_string()",
            "\"snapshot_status\".to_string()",
            "\"is_refreshing\".to_string()",
        ],
        "api/probe.rs must not own Probe logs-db view or snapshot metadata semantics",
    );

    assert_present(
        daemon_production,
        &[
            "probe_service::probe_event_record_plan",
            "probe_service::probe_bark_delivery_decision",
            "probe_service::probe_bark_status_label",
            "probe_service::probe_logs_db_scheduler_plan",
            "probe_service::probe_logs_db_stored_result",
            "probe_service::probe_passive_thread_notification_plan",
        ],
        "daemon main should ask core for Probe event/notification decisions",
    );
    assert_present(
        &core_probe,
        &[
            "pub fn probe_event_record_plan",
            "pub fn probe_bark_delivery_decision",
            "pub fn probe_bark_status_label",
            "pub fn probe_logs_db_scheduler_plan",
            "pub fn probe_logs_db_stored_result",
            "normalize_probe_event_dedupe_key(&mut event)",
            "probe_passive_unresolved_action_marker_key(&event)",
        ],
        "core Probe service should own event dedupe and passive marker planning",
    );
    assert_absent(
        daemon_production,
        &[
            "fn normalize_probe_event_dedupe_key(",
            "fn probe_thread_notification_body(",
            "fn probe_thread_passive_bark_fresh(",
            "fn format_proposed_plan_reply_needed(",
            "fn passive_unresolved_action_marker_key(",
            "fn normalize_proposed_plan_dedupe_key(",
            "fn normalize_request_user_input_dedupe_key(",
            "fn probe_event_bark_switch_enabled(",
            "fn probe_bark_status_label(",
            "fn probe_logs_db_compaction_due(",
            "fn add_probe_events_maintenance_fields(",
            "\"logs_db_disabled\".to_string()",
            "\"not_due\".to_string()",
            "event.payload[\"reason_label\"] = json!(\"等待用户确认\")",
            "event.payload[\"reason_label\"] = json!(\"异常/可恢复\")",
        ],
        "daemon main.rs must not define Probe notification decision semantics",
    );
}

#[test]
fn transport_endpoints_stay_out_of_business_rpc_allowlist() {
    let routes = src("api/routes.rs");
    let dispatch = src("api/rpc_dispatch.rs");
    let surface = src("rpc_surface.rs");

    assert_present(
        &routes,
        &[
            ".route(RPC_THREAD_EVENTS_ROUTE, get(thread_events))",
            ".route(\n            RPC_UPLOAD_FILES_ROUTE,",
            ".route(RPC_COMMAND_ROUTE, post(rpc_dispatch))",
            ".route(LEGACY_API_FALLBACK_ROUTE, any(api_not_found))",
        ],
        "routes should reserve transport endpoints explicitly before command dispatch",
    );
    assert_present(
        &dispatch,
        &[
            "is_transport_rpc_command(&command)",
            "transport endpoint is not a business rpc command",
            "is_business_rpc_command(&command)",
        ],
        "rpc dispatch should reject transport commands as business RPC",
    );
    assert_present(
        &surface,
        &[
            "ALLOWED_TRANSPORT_COMMANDS.contains(&command)",
            "rpc_commands::is_allowed_rpc_command(command)",
        ],
        "rpc surface should keep business and transport allowlists separate",
    );
    assert_absent(
        &surface,
        &[
            "command == \"uploadFiles\" || rpc_commands::is_allowed_rpc_command(command)",
            "command == \"threadEvents\" || rpc_commands::is_allowed_rpc_command(command)",
        ],
        "transport endpoints must not sneak into the business allowlist",
    );
}
