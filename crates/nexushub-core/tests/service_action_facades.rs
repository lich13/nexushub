use nexushub_core::{
    codex::{MessageBlock, ThreadDetail, ThreadStatus, ThreadSummary},
    db::ThreadGoal,
};
use nexushub_core::{
    config::Config,
    platform::{PlatformKind, PlatformPaths},
    services::{
        cleanup::{plan_cleanup_action, CleanupAction, CleanupTarget},
        commands,
        goals::{plan_goal_command_with_capability, GoalCommandKind, GoalUpdateRequest},
        jobs::{
            archive_thread_response, cancel_followup_response,
            plan_followup_cancel_with_capability, plan_followup_enqueue_with_capability,
            plan_followup_list_with_capability, plan_thread_archive_with_capability,
            plan_thread_command_with_capability, plan_thread_rename_with_capability,
            plan_thread_restore_with_capability, plan_thread_send_with_capability,
            plan_thread_steer_with_capability, plan_thread_stop_with_capability,
            rename_thread_response, resolve_thread_stop_job, thread_state_action_response,
            thread_stop_response, FollowUpCancelRequest, FollowUpListRequest, ThreadCommandKind,
            ThreadCommandRequest, ThreadMessageRequest, ThreadRenameRequest, ThreadSendRequest,
            ThreadSteerRequest, ThreadStopRequest,
        },
        probe::{
            plan_probe_action, plan_probe_action_with_config_path, ProbeAction, ProbeExecutionKind,
            ProbeUseCases,
        },
        settings::{
            ProbeNotificationsSavePatch, ProbeSecretState, ProbeSettingsSavePatch,
            ProbeSettingsSaveRequest, SettingsUseCases,
        },
        system::Capability,
        threads::{
            normalize_thread_block_limit, normalize_thread_detail_block_limit,
            plan_thread_blocks_request, plan_thread_cleanup_action, plan_thread_detail_request,
            plan_threads_list_request, thread_blocks_page_for_plan, ThreadCleanupAction,
            ThreadDetailRequest, ThreadsQuery,
        },
        updates::{plan_update_action, UpdateAction, UpdateExecutionMethod, UpdateUseCases},
        uploads::{
            plan_delete_upload_with_capability, plan_store_uploads_with_capability,
            plan_upload_validation_with_capability, validate_attachment_id_count, UploadBatchItem,
        },
    },
};

#[test]
fn probe_and_update_actions_expose_shared_rpc_and_desktop_command_names() {
    assert_eq!(
        ProbeAction::BarkTest.as_rpc_action(),
        commands::PROBE_BARK_TEST
    );
    assert_eq!(
        ProbeAction::BarkTest.as_desktop_command(),
        commands::PROBE_BARK_TEST
    );
    assert_eq!(
        ProbeAction::InstallHooks.as_rpc_action(),
        commands::PROBE_INSTALL_HOOKS
    );
    assert_eq!(
        ProbeAction::InstallHooks.as_desktop_command(),
        commands::PROBE_INSTALL_HOOKS
    );
    assert_eq!(
        ProbeAction::LogsDbDryRun.as_rpc_action(),
        commands::PROBE_LOGS_DB_DRY_RUN
    );
    assert_eq!(
        ProbeAction::LogsDbDryRun.as_desktop_command(),
        commands::PROBE_LOGS_DB_DRY_RUN
    );
    assert_eq!(
        ProbeAction::LogsDbExecute.as_rpc_action(),
        commands::PROBE_LOGS_DB_EXECUTE
    );
    assert_eq!(
        ProbeAction::LogsDbExecute.as_desktop_command(),
        commands::PROBE_LOGS_DB_EXECUTE
    );

    assert_eq!(UpdateAction::Check.as_rpc_action(), commands::UPDATES_CHECK);
    assert_eq!(
        UpdateAction::Check.as_desktop_command(),
        commands::UPDATES_CHECK
    );
    assert_eq!(
        UpdateAction::Install.as_rpc_action(),
        commands::UPDATES_INSTALL
    );
    assert_eq!(
        UpdateAction::Install.as_desktop_command(),
        commands::UPDATES_INSTALL
    );
    assert_eq!(UpdateAction::Prune.as_rpc_action(), commands::UPDATES_PRUNE);
    assert_eq!(
        UpdateAction::Prune.as_desktop_command(),
        commands::UPDATES_PRUNE
    );
}

#[test]
fn probe_actions_parse_string_aliases_and_plan_fixed_jobs_in_core() {
    let config = Config::for_platform_kind(PlatformKind::Linux);
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);

    assert_eq!(
        "barkTest".parse::<ProbeAction>().unwrap(),
        ProbeAction::BarkTest
    );
    assert_eq!(
        "installHooks".parse::<ProbeAction>().unwrap(),
        ProbeAction::InstallHooks
    );
    assert_eq!(
        "logsDbDryRun".parse::<ProbeAction>().unwrap(),
        ProbeAction::LogsDbDryRun
    );
    assert_eq!(
        "logs-db-execute".parse::<ProbeAction>().unwrap(),
        ProbeAction::LogsDbExecute
    );
    assert!("unknown".parse::<ProbeAction>().is_err());

    let bark = plan_probe_action(&config, &platform, ProbeAction::BarkTest).unwrap();
    assert_eq!(bark.required_capability, Capability::Probe);
    assert_eq!(bark.action, ProbeAction::BarkTest);
    assert_eq!(bark.execution, ProbeExecutionKind::FixedShellJob);
    assert_eq!(bark.job.as_ref().unwrap().kind, "probe_bark_test");
    assert_eq!(
        bark.job.as_ref().unwrap().args,
        vec!["probe".to_string(), "bark-test".to_string()]
    );
    assert_eq!(
        bark.job.as_ref().unwrap().exclusive_group.as_deref(),
        Some("probe_bark")
    );
    assert!(bark.maintenance.is_none());
    assert_eq!(
        bark.diagnostic_plan.as_ref().map(|plan| plan.kind.as_str()),
        Some("bark-test")
    );

    let dry_run = plan_probe_action(&config, &platform, ProbeAction::LogsDbDryRun).unwrap();
    assert_eq!(dry_run.required_capability, Capability::ProbeLogMaintenance);
    assert_eq!(dry_run.action, ProbeAction::LogsDbDryRun);
    assert_eq!(
        dry_run.job.as_ref().unwrap().kind,
        "probe_logs_db_maintain_dry_run"
    );
    assert!(dry_run
        .job
        .as_ref()
        .unwrap()
        .args
        .contains(&"--dry-run".to_string()));
    assert!(dry_run.maintenance.as_ref().unwrap().dry_run);
    assert!(!dry_run.maintenance.as_ref().unwrap().compact);

    let execute = plan_probe_action(&config, &platform, ProbeAction::LogsDbExecute).unwrap();
    assert_eq!(execute.required_capability, Capability::ProbeLogMaintenance);
    assert_eq!(execute.job.as_ref().unwrap().kind, "probe_logs_db_maintain");
    assert!(!execute
        .job
        .as_ref()
        .unwrap()
        .args
        .contains(&"--dry-run".to_string()));
    assert!(!execute.maintenance.as_ref().unwrap().dry_run);
}

#[test]
fn retired_string_action_multiplexers_do_not_reenter_core_facades() {
    let service_sources = [
        ("cleanup", include_str!("../src/services/cleanup.rs")),
        ("probe", include_str!("../src/services/probe.rs")),
        ("updates", include_str!("../src/services/updates.rs")),
        ("use_cases", include_str!("../src/services/use_cases.rs")),
    ];

    for (name, source) in service_sources {
        for forbidden in [
            "startProbeJob",
            "runUpdateAction",
            "updatesPrune",
            "backupPrune",
            "dryRunArchiveDelete",
            "startArchiveDelete",
            "dryRunHiddenThreadDelete",
            "startHiddenThreadDelete",
        ] {
            assert!(
                !source.contains(forbidden),
                "{name} service must not accept retired string action: {forbidden}"
            );
        }
    }

    let cleanup_source = include_str!("../src/services/cleanup.rs");
    assert!(
        !cleanup_source.contains("impl FromStr for CleanupAction"),
        "cleanup execute/dry-run selection must stay typed and must not parse string actions"
    );

    let update_source = include_str!("../src/services/updates.rs");
    assert!(
        !update_source.contains("impl FromStr for UpdateAction"),
        "update actions must stay typed and must not reintroduce a string action multiplexer"
    );
}

#[test]
fn probe_fixed_job_command_is_core_generated_and_can_use_config_path_override() {
    let config = Config::for_platform_kind(PlatformKind::Linux);
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);

    let default_plan = plan_probe_action(&config, &platform, ProbeAction::BarkTest).unwrap();
    let default_command = &default_plan.job.as_ref().unwrap().command;
    assert_eq!(
        default_command,
        "/usr/local/bin/nexushub-webd --config /etc/nexushub-webd/config.toml probe bark-test"
    );

    let custom_config = std::path::Path::new("/tmp/nexushub custom/config.toml");
    let override_plan = plan_probe_action_with_config_path(
        &config,
        &platform,
        ProbeAction::BarkTest,
        custom_config,
    )
    .unwrap();
    let override_job = override_plan.job.as_ref().unwrap();

    assert_eq!(
        override_job.command,
        "/usr/local/bin/nexushub-webd --config '/tmp/nexushub custom/config.toml' probe bark-test"
    );
    assert_eq!(
        override_job.args,
        vec!["probe".to_string(), "bark-test".to_string()]
    );
    assert!(!override_job
        .command
        .contains("/etc/nexushub-webd/config.toml"));
}

#[test]
fn probe_action_capability_gate_rejects_unsupported_platforms() {
    let config = Config::for_platform_kind(PlatformKind::Windows);
    let platform = PlatformPaths::for_kind(PlatformKind::Windows);

    let err = plan_probe_action(&config, &platform, ProbeAction::InstallHooks)
        .expect_err("Windows must not expose Probe actions");

    assert!(err.to_string().contains("probe is unavailable on windows"));
}

#[test]
fn update_action_facade_plans_linux_jobs_and_macos_native_updates_but_not_prune() {
    let linux_config = Config::for_platform_kind(PlatformKind::Linux);
    let linux = PlatformPaths::for_kind(PlatformKind::Linux);
    let check = plan_update_action(&linux_config, &linux, UpdateAction::Check).unwrap();
    assert_eq!(check.required_capability, Capability::LinuxUpdateJob);
    assert_eq!(check.method, UpdateExecutionMethod::LinuxSystemdJob);
    assert_eq!(
        check.linux_job.as_ref().unwrap().kind,
        "nexushub_update_check"
    );
    assert!(check.native.is_none());

    let mac_home = temp_dir("nexushub-update-action-macos");
    std::fs::create_dir_all(&mac_home).unwrap();
    let mac_config = Config::for_platform_kind_with_home(PlatformKind::Macos, &mac_home);
    let mac = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &mac_home);

    let mac_check = plan_update_action(&mac_config, &mac, UpdateAction::Check).unwrap();
    assert_eq!(mac_check.required_capability, Capability::AppUpdater);
    assert_eq!(mac_check.method, UpdateExecutionMethod::MacosTauriUpdater);
    assert!(mac_check.linux_job.is_none());
    assert_eq!(
        mac_check.native.as_ref().unwrap().command,
        commands::UPDATES_CHECK
    );

    let mac_install = plan_update_action(&mac_config, &mac, UpdateAction::Install).unwrap();
    assert_eq!(
        mac_install.native.as_ref().unwrap().command,
        commands::UPDATES_INSTALL
    );

    let err = plan_update_action(&mac_config, &mac, UpdateAction::Prune)
        .expect_err("backup prune is a Linux-only update action");
    assert!(err
        .to_string()
        .contains("prune_backups is unavailable on macos"));

    std::fs::remove_dir_all(mac_home).unwrap();
}

#[test]
fn probe_settings_update_use_cases_group_adapter_ready_plans_in_core() {
    let linux_config = Config::for_platform_kind(PlatformKind::Linux);
    let linux = PlatformPaths::for_kind(PlatformKind::Linux);

    let probe = ProbeUseCases::new(&linux_config, &linux);
    let bark = probe.action(ProbeAction::BarkTest).unwrap();
    assert_eq!(bark.required_capability, Capability::Probe);
    assert_eq!(bark.execution, ProbeExecutionKind::FixedShellJob);
    assert_eq!(bark.job.as_ref().unwrap().kind, "probe_bark_test");

    let logs_dry_run = probe.logs_db_maintenance_plan(true).unwrap();
    assert_eq!(
        logs_dry_run.required_capability,
        Capability::ProbeLogMaintenance
    );
    assert_eq!(logs_dry_run.action, ProbeAction::LogsDbDryRun);
    assert!(logs_dry_run.maintenance.as_ref().unwrap().dry_run);

    let settings = SettingsUseCases::new(&linux_config, &linux);
    let view = settings
        .probe_settings_view(ProbeSecretState::Configured)
        .unwrap();
    assert_eq!(view.required_capability, Capability::Settings);
    assert!(view.settings.notifications.device_key_configured);

    let save = settings
        .save_probe_settings(ProbeSettingsSaveRequest {
            probe: Some(ProbeSettingsSavePatch {
                notifications: Some(ProbeNotificationsSavePatch {
                    device_key: Some(" use-case-bark-key ".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(save.required_capability, Capability::Settings);
    assert_eq!(save.bark_device_key.as_deref(), Some("use-case-bark-key"));
    let serialized = serde_json::to_string(&save).unwrap();
    assert!(serialized.contains("[configured]"));
    assert!(!serialized.contains("use-case-bark-key"));

    let updates = UpdateUseCases::new(&linux_config, &linux);
    let check = updates.check_plan().unwrap();
    assert_eq!(check.required_capability, Capability::LinuxUpdateJob);
    assert_eq!(check.action, UpdateAction::Check);
    assert_eq!(check.method, UpdateExecutionMethod::LinuxSystemdJob);
    let install = updates.install_plan().unwrap();
    assert_eq!(install.action, UpdateAction::Install);
    let prune = updates.prune_plan().unwrap();
    assert_eq!(prune.required_capability, Capability::PruneBackups);
    assert_eq!(prune.action, UpdateAction::Prune);

    let mac_home = temp_dir("nexushub-use-cases-macos");
    std::fs::create_dir_all(&mac_home).unwrap();
    let mac_config = Config::for_platform_kind_with_home(PlatformKind::Macos, &mac_home);
    let mac = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &mac_home);
    let mac_updates = UpdateUseCases::new(&mac_config, &mac);

    assert_eq!(
        mac_updates.check_plan().unwrap().required_capability,
        Capability::AppUpdater
    );
    assert_eq!(
        mac_updates.install_plan().unwrap().method,
        UpdateExecutionMethod::MacosTauriUpdater
    );
    let err = mac_updates
        .prune_plan()
        .expect_err("macOS must not expose Linux backup pruning");
    assert!(err
        .to_string()
        .contains("prune_backups is unavailable on macos"));

    std::fs::remove_dir_all(mac_home).unwrap();
}

#[test]
fn thread_detail_cleanup_followup_upload_facades_are_capability_gated() {
    let linux = PlatformPaths::for_kind(PlatformKind::Linux);
    let windows = PlatformPaths::for_kind(PlatformKind::Windows);

    let detail = plan_thread_detail_request(
        &linux,
        ThreadDetailRequest {
            id: " thread-a ".to_string(),
            limit: Some(999),
            full: Some(false),
            before: Some("b:120".to_string()),
        },
    )
    .unwrap();
    assert_eq!(detail.thread_id, "thread-a");
    assert_eq!(detail.block_limit, Some(500));
    assert_eq!(detail.before.as_deref(), Some("b:120"));

    let full = plan_thread_detail_request(
        &linux,
        ThreadDetailRequest {
            id: "thread-a".to_string(),
            limit: Some(1),
            full: Some(true),
            before: None,
        },
    )
    .unwrap();
    assert_eq!(full.block_limit, None);

    let blocks = plan_thread_blocks_request(&linux, "thread-a", Some(0), None).unwrap();
    assert_eq!(blocks.block_limit, Some(1));
    assert!(!blocks.full);

    let cleanup = plan_thread_cleanup_action(&linux, ThreadCleanupAction::ArchiveDeleteDryRun)
        .expect("Linux can plan cleanup actions");
    assert_eq!(cleanup.required_capability, Capability::ThreadCleanup);
    assert!(!cleanup.execute);

    let followup = plan_thread_command_with_capability(
        &linux,
        ThreadCommandRequest {
            command: ThreadCommandKind::FollowUp,
            thread_id: Some(" thread-a ".to_string()),
            message: ThreadMessageRequest {
                message: "  queue this  ".to_string(),
                ..ThreadMessageRequest::default()
            },
        },
    )
    .unwrap();
    assert_eq!(followup.required_capability, Capability::Jobs);
    assert_eq!(
        followup
            .command
            .followup
            .as_ref()
            .map(|plan| plan.thread_id.as_str()),
        Some("thread-a")
    );

    let upload = plan_store_uploads_with_capability(
        &linux,
        vec![UploadBatchItem {
            name: "notes.md".to_string(),
            mime: None,
            bytes: b"# Notes".to_vec(),
        }],
    )
    .unwrap();
    assert_eq!(upload.required_capability, Capability::Jobs);
    assert_eq!(upload.plan.total_files, 1);
    let delete_upload =
        plan_delete_upload_with_capability(&linux, " 018f0a59-f18a-7fa9-98fb-3bd51964d001 ")
            .unwrap();
    assert_eq!(delete_upload.required_capability, Capability::Jobs);
    assert_eq!(delete_upload.id, "018f0a59-f18a-7fa9-98fb-3bd51964d001");

    let goal = plan_goal_command_with_capability(
        &linux,
        GoalUpdateRequest {
            thread_id: Some(" thread-a ".to_string()),
            objective: Some("  Ship it  ".to_string()),
            token_budget: Some(512),
            status: None,
            enabled: None,
        },
    )
    .unwrap();
    assert_eq!(goal.required_capability, Capability::Threads);
    assert_eq!(goal.command.update.thread_id, "thread-a");
    assert_eq!(goal.command.update.objective.as_deref(), Some("Ship it"));

    assert!(plan_thread_detail_request(
        &windows,
        ThreadDetailRequest {
            id: "thread-a".to_string(),
            limit: None,
            full: None,
            before: None,
        },
    )
    .is_err());
    assert!(
        plan_thread_cleanup_action(&windows, ThreadCleanupAction::HiddenDeleteExecute).is_err()
    );
    assert!(plan_thread_command_with_capability(
        &windows,
        ThreadCommandRequest {
            command: ThreadCommandKind::FollowUp,
            thread_id: Some("thread-a".to_string()),
            message: ThreadMessageRequest {
                message: "queue this".to_string(),
                ..ThreadMessageRequest::default()
            },
        },
    )
    .is_err());
    assert!(plan_goal_command_with_capability(
        &windows,
        GoalUpdateRequest {
            thread_id: Some("thread-a".to_string()),
            objective: Some("ship".to_string()),
            token_budget: None,
            status: None,
            enabled: None,
        },
    )
    .is_err());
}

#[test]
fn thread_list_and_blocks_facades_return_adapter_ready_core_plans() {
    let linux = PlatformPaths::for_kind(PlatformKind::Linux);
    let windows = PlatformPaths::for_kind(PlatformKind::Windows);

    let list = plan_threads_list_request(
        &linux,
        ThreadsQuery {
            status: Some("running".to_string()),
            q: Some("  work  ".to_string()),
            limit: Some(25),
        },
    )
    .unwrap();
    assert_eq!(list.required_capability, Capability::Threads);
    assert_eq!(list.response_limit, 25);
    assert_eq!(list.fetch_limit, usize::MAX);
    assert_eq!(list.query.q.as_deref(), Some("work"));

    let blocks = plan_thread_blocks_request(&linux, " thread-a ", Some(1), None).unwrap();
    let page = thread_blocks_page_for_plan(thread_detail_with_blocks("thread-a", 2), &blocks);
    assert_eq!(page.thread_id, "thread-a");
    assert_eq!(page.blocks.len(), 1);
    assert_eq!(page.total_blocks, 2);
    assert!(page.has_more_blocks);

    assert!(plan_threads_list_request(&windows, ThreadsQuery::default()).is_err());
}

#[test]
fn thread_send_steer_stop_followup_and_state_actions_plan_without_host_dependencies() {
    let linux = PlatformPaths::for_kind(PlatformKind::Linux);
    let windows = PlatformPaths::for_kind(PlatformKind::Windows);

    let create = plan_thread_send_with_capability(
        &linux,
        ThreadSendRequest {
            thread_id: None,
            message: ThreadMessageRequest {
                message: "  start new work  ".to_string(),
                ..ThreadMessageRequest::default()
            },
        },
    )
    .unwrap();
    assert_eq!(create.required_capability, Capability::Jobs);
    assert_eq!(create.command.command, ThreadCommandKind::Create);
    assert_eq!(
        create
            .command
            .action
            .as_ref()
            .map(|action| action.thread_id.as_deref()),
        Some(None)
    );

    let send = plan_thread_send_with_capability(
        &linux,
        ThreadSendRequest {
            thread_id: Some(" thread-a ".to_string()),
            message: ThreadMessageRequest {
                message: " continue ".to_string(),
                ..ThreadMessageRequest::default()
            },
        },
    )
    .unwrap();
    assert_eq!(send.command.command, ThreadCommandKind::Resume);
    assert_eq!(send.command.thread_id.as_deref(), Some("thread-a"));
    assert_eq!(
        send.command
            .action
            .as_ref()
            .and_then(|action| action.thread_id.as_deref()),
        Some("thread-a")
    );

    let steer = plan_thread_steer_with_capability(
        &linux,
        ThreadSteerRequest {
            thread_id: Some(" thread-a ".to_string()),
            message: ThreadMessageRequest {
                message: "  queue this  ".to_string(),
                ..ThreadMessageRequest::default()
            },
        },
    )
    .unwrap();
    assert_eq!(steer.command.command, ThreadCommandKind::FollowUp);
    assert!(steer.command.action.is_none());
    assert_eq!(
        steer
            .command
            .followup
            .as_ref()
            .map(|followup| followup.message.as_str()),
        Some("queue this")
    );

    let list = plan_followup_list_with_capability(
        &linux,
        FollowUpListRequest {
            thread_id: " thread-a ".to_string(),
            limit: Some(999),
        },
    )
    .unwrap();
    assert_eq!(list.required_capability, Capability::Jobs);
    assert_eq!(list.thread_id, "thread-a");
    assert_eq!(list.limit, 200);

    let enqueue = plan_followup_enqueue_with_capability(
        &linux,
        ThreadSteerRequest {
            thread_id: Some(" thread-a ".to_string()),
            message: ThreadMessageRequest {
                message: "  later  ".to_string(),
                ..ThreadMessageRequest::default()
            },
        },
    )
    .unwrap();
    assert_eq!(enqueue.required_capability, Capability::Jobs);
    assert_eq!(enqueue.followup.thread_id, "thread-a");
    assert_eq!(enqueue.followup.message, "later");

    let cancel = plan_followup_cancel_with_capability(
        &linux,
        FollowUpCancelRequest {
            thread_id: " thread-a ".to_string(),
            followup_id: " followup-a ".to_string(),
        },
    )
    .unwrap();
    assert_eq!(cancel.thread_id, "thread-a");
    assert_eq!(cancel.followup_id, "followup-a");

    let stop = plan_thread_stop_with_capability(
        &linux,
        ThreadStopRequest {
            thread_id: " thread-a ".to_string(),
            turn_id: Some(" turn-a ".to_string()),
            job_id: None,
        },
    )
    .unwrap();
    assert!(stop.requires_active_job_lookup);
    let resolved = resolve_thread_stop_job(&stop, Some(" job-a ".to_string())).unwrap();
    assert_eq!(resolved.job_id, "job-a");
    let response = thread_stop_response(&resolved, true);
    assert_eq!(response.command, commands::THREADS_STOP);
    assert_eq!(response.thread_id.as_deref(), Some("thread-a"));
    assert_eq!(response.job_id.as_deref(), Some("job-a"));

    let archive = plan_thread_archive_with_capability(&linux, " thread-a ").unwrap();
    assert_eq!(
        archive.required_capability,
        Capability::ThreadArchiveActions
    );
    assert_eq!(archive.command, commands::THREADS_ARCHIVE);
    assert_eq!(archive.thread_id, "thread-a");
    assert_eq!(archive.archived, Some(true));

    let restore = plan_thread_restore_with_capability(&linux, " thread-a ").unwrap();
    assert_eq!(restore.command, commands::THREADS_RESTORE);
    assert_eq!(restore.archived, Some(false));

    let rename = plan_thread_rename_with_capability(
        &linux,
        ThreadRenameRequest {
            thread_id: " thread-a ".to_string(),
            name: "  New name  ".to_string(),
        },
    )
    .unwrap();
    assert_eq!(rename.command, commands::THREADS_RENAME);
    assert_eq!(rename.name.as_deref(), Some("New name"));

    assert!(plan_thread_send_with_capability(
        &windows,
        ThreadSendRequest {
            thread_id: Some("thread-a".to_string()),
            message: ThreadMessageRequest {
                message: "continue".to_string(),
                ..ThreadMessageRequest::default()
            },
        },
    )
    .is_err());
}

#[test]
fn goal_facades_cover_get_save_clear_pause_and_resume_as_core_plans() {
    let linux = PlatformPaths::for_kind(PlatformKind::Linux);

    let get = nexushub_core::services::goals::plan_goal_get_with_capability(
        &linux,
        nexushub_core::services::goals::GoalGetRequest {
            thread_id: Some(" thread-a ".to_string()),
        },
    )
    .unwrap();
    assert_eq!(get.required_capability, Capability::Threads);
    assert_eq!(get.thread_id.as_deref(), Some("thread-a"));
    assert!(!get.missing_thread);

    let missing = nexushub_core::services::goals::plan_goal_get_with_capability(
        &linux,
        nexushub_core::services::goals::GoalGetRequest { thread_id: None },
    )
    .unwrap();
    assert!(missing.missing_thread);

    let save = nexushub_core::services::goals::plan_goal_save_with_capability(
        &linux,
        GoalUpdateRequest {
            thread_id: Some(" thread-a ".to_string()),
            objective: Some("  Ship it  ".to_string()),
            token_budget: Some(1024),
            status: None,
            enabled: None,
        },
    )
    .unwrap();
    assert_eq!(save.command.command, GoalCommandKind::Save);

    let clear =
        nexushub_core::services::goals::plan_goal_clear_with_capability(&linux, Some(" thread-a "))
            .unwrap();
    assert_eq!(clear.command.command, GoalCommandKind::Clear);
    assert_eq!(clear.command.update.status, "cleared");

    let existing = ThreadGoal {
        thread_id: "thread-a".to_string(),
        objective: Some("Keep context".to_string()),
        token_budget: Some(512),
        status: "active".to_string(),
        created_at: 1,
        updated_at: 2,
        completed_at: None,
        blocked_reason: None,
    };
    let paused = nexushub_core::services::goals::plan_goal_pause_with_capability(
        &linux,
        " thread-a ",
        Some(&existing),
    )
    .unwrap();
    assert_eq!(paused.command.command, GoalCommandKind::Pause);
    assert_eq!(
        paused.command.update.objective.as_deref(),
        Some("Keep context")
    );
    assert_eq!(paused.command.update.status, "paused");

    let resumed = nexushub_core::services::goals::plan_goal_resume_with_capability(
        &linux,
        " thread-a ",
        Some(&existing),
    )
    .unwrap();
    assert_eq!(resumed.command.command, GoalCommandKind::Resume);
    assert_eq!(resumed.command.update.status, "active");
}

#[test]
fn cleanup_and_upload_facades_expose_validation_and_execution_boundaries() {
    let linux = PlatformPaths::for_kind(PlatformKind::Linux);
    let windows = PlatformPaths::for_kind(PlatformKind::Windows);

    let cleanup = plan_cleanup_action(&linux, CleanupAction::HiddenDeleteExecute).unwrap();
    assert_eq!(cleanup.required_capability, Capability::ThreadCleanup);
    assert_eq!(cleanup.command, commands::CLEANUP_HIDDEN_EXECUTE);
    assert_eq!(cleanup.target, CleanupTarget::Hidden);
    assert!(cleanup.execute);
    assert!(cleanup.requires_confirmation);

    for execute in [
        plan_cleanup_action(&linux, CleanupAction::ArchiveDeleteExecute).unwrap(),
        plan_cleanup_action(&linux, CleanupAction::HiddenDeleteExecute).unwrap(),
    ] {
        assert!(execute.execute);
        assert!(
            execute.requires_confirmation,
            "cleanup execute action must require confirmation: {:?}",
            execute.action
        );
    }

    for dry_run in [
        plan_cleanup_action(&linux, CleanupAction::ArchiveDeleteDryRun).unwrap(),
        plan_cleanup_action(&linux, CleanupAction::HiddenDeleteDryRun).unwrap(),
    ] {
        assert!(!dry_run.execute);
        assert!(
            !dry_run.requires_confirmation,
            "cleanup dry-run must not require execute confirmation: {:?}",
            dry_run.action
        );
    }

    let thread_reexport =
        plan_thread_cleanup_action(&linux, ThreadCleanupAction::ArchiveDeleteDryRun).unwrap();
    assert_eq!(thread_reexport.command, commands::CLEANUP_ARCHIVE_DRY_RUN);
    assert_eq!(thread_reexport.target, CleanupTarget::Archived);
    assert!(!thread_reexport.execute);

    let upload_validation = plan_upload_validation_with_capability(
        &linux,
        &[UploadBatchItem {
            name: "notes.md".to_string(),
            mime: None,
            bytes: b"# Notes".to_vec(),
        }],
    )
    .unwrap();
    assert_eq!(upload_validation.required_capability, Capability::Jobs);
    assert_eq!(upload_validation.total_files, 1);
    assert_eq!(upload_validation.total_bytes, 7);
    assert_eq!(upload_validation.max_files, 5);

    assert!(plan_cleanup_action(&windows, CleanupAction::ArchiveDeleteDryRun).is_err());
    assert!(plan_upload_validation_with_capability(&windows, &[]).is_err());
    assert!(
        plan_delete_upload_with_capability(&windows, "018f0a59-f18a-7fa9-98fb-3bd51964d001")
            .is_err()
    );
    assert!(plan_delete_upload_with_capability(&linux, "not-a-uuid").is_err());
}

#[test]
fn core_facade_sources_do_not_import_host_runtime_surfaces() {
    for (name, source) in [
        ("threads", include_str!("../src/services/threads.rs")),
        ("jobs", include_str!("../src/services/jobs.rs")),
        ("goals", include_str!("../src/services/goals.rs")),
        ("uploads", include_str!("../src/services/uploads.rs")),
        ("cleanup", include_str!("../src/services/cleanup.rs")),
    ] {
        for forbidden in [
            "axum",
            "tauri",
            "HeaderMap",
            "Tauri",
            "systemctl",
            "nginx",
            "Nginx",
        ] {
            assert!(
                !source.contains(forbidden),
                "{name} facade source must not import host runtime surface {forbidden}"
            );
        }
    }
}

#[test]
fn thread_limit_and_attachment_id_count_helpers_are_shared_core_contracts() {
    assert_eq!(normalize_thread_detail_block_limit(None, false), Some(120));
    assert_eq!(
        normalize_thread_detail_block_limit(Some(999), false),
        Some(500)
    );
    assert_eq!(normalize_thread_detail_block_limit(Some(0), false), Some(1));
    assert_eq!(normalize_thread_detail_block_limit(Some(25), true), None);

    assert_eq!(normalize_thread_block_limit(None), 120);
    assert_eq!(normalize_thread_block_limit(Some(0)), 1);
    assert_eq!(normalize_thread_block_limit(Some(999)), 500);

    let ids = vec![
        "a".to_string(),
        "b".to_string(),
        "c".to_string(),
        "d".to_string(),
        "e".to_string(),
    ];
    assert!(validate_attachment_id_count(&ids).is_ok());
    let too_many = ids
        .into_iter()
        .chain(std::iter::once("f".to_string()))
        .collect::<Vec<_>>();
    assert!(validate_attachment_id_count(&too_many)
        .unwrap_err()
        .to_string()
        .contains("一次最多发送 5 个附件"));
}

#[test]
fn cleanup_and_thread_action_response_commands_use_unified_dot_contracts() {
    assert_eq!(
        ThreadCleanupAction::ArchiveDeleteDryRun.as_rpc_action(),
        commands::CLEANUP_ARCHIVE_DRY_RUN
    );
    assert_eq!(
        ThreadCleanupAction::ArchiveDeleteExecute.as_rpc_action(),
        commands::CLEANUP_ARCHIVE_EXECUTE
    );
    assert_eq!(
        ThreadCleanupAction::HiddenDeleteDryRun.as_rpc_action(),
        commands::CLEANUP_HIDDEN_DRY_RUN
    );
    assert_eq!(
        ThreadCleanupAction::HiddenDeleteExecute.as_rpc_action(),
        commands::CLEANUP_HIDDEN_EXECUTE
    );

    let archived = archive_thread_response("thread-a".to_string(), true);
    assert_eq!(archived.command, commands::THREADS_ARCHIVE);
    let archive_plan = plan_thread_archive_with_capability(
        &PlatformPaths::for_kind(PlatformKind::Linux),
        "thread-a",
    )
    .unwrap();
    assert_eq!(
        thread_state_action_response(&archive_plan).unwrap().command,
        commands::THREADS_ARCHIVE
    );

    let restored = archive_thread_response("thread-a".to_string(), false);
    assert_eq!(restored.command, commands::THREADS_RESTORE);
    let restore_plan = plan_thread_restore_with_capability(
        &PlatformPaths::for_kind(PlatformKind::Linux),
        "thread-a",
    )
    .unwrap();
    assert_eq!(
        thread_state_action_response(&restore_plan).unwrap().command,
        commands::THREADS_RESTORE
    );

    let renamed = rename_thread_response("thread-a".to_string(), "new name").unwrap();
    assert_eq!(renamed.command, commands::THREADS_RENAME);
    let rename_plan = plan_thread_rename_with_capability(
        &PlatformPaths::for_kind(PlatformKind::Linux),
        ThreadRenameRequest {
            thread_id: "thread-a".to_string(),
            name: " new name ".to_string(),
        },
    )
    .unwrap();
    assert_eq!(
        thread_state_action_response(&rename_plan).unwrap().data,
        Some(serde_json::json!({"name": "new name"}))
    );

    let cancelled = cancel_followup_response(
        commands::THREADS_FOLLOWUPS_CANCEL,
        "thread-a".to_string(),
        "followup-a".to_string(),
        true,
    );
    assert_eq!(cancelled.command, commands::THREADS_FOLLOWUPS_CANCEL);
}

#[test]
fn goal_action_helpers_use_unified_thread_goal_commands() {
    assert_eq!(
        GoalCommandKind::Save.as_rpc_action(),
        commands::THREADS_GOAL_SAVE
    );
    assert_eq!(
        GoalCommandKind::Clear.as_rpc_action(),
        commands::THREADS_GOAL_CLEAR
    );
    assert_eq!(
        GoalCommandKind::Pause.as_rpc_action(),
        commands::THREADS_GOAL_PAUSE
    );
    assert_eq!(
        GoalCommandKind::Resume.as_rpc_action(),
        commands::THREADS_GOAL_RESUME
    );

    for command in [
        commands::THREADS_GOAL_GET,
        commands::THREADS_GOAL_SAVE,
        commands::THREADS_GOAL_CLEAR,
        commands::THREADS_GOAL_PAUSE,
        commands::THREADS_GOAL_RESUME,
    ] {
        assert!(commands::is_allowed_rpc_command(command));
        assert!(!commands::is_retired_command(command));
    }
}

#[test]
fn retired_commands_are_not_emitted_by_core_action_plans() {
    let linux = PlatformPaths::for_kind(PlatformKind::Linux);
    let linux_config = Config::for_platform_kind(PlatformKind::Linux);
    let mut values = vec![
        serde_json::to_value(
            plan_probe_action(&linux_config, &linux, ProbeAction::BarkTest).unwrap(),
        )
        .unwrap(),
        serde_json::to_value(
            plan_probe_action(&linux_config, &linux, ProbeAction::InstallHooks).unwrap(),
        )
        .unwrap(),
        serde_json::to_value(
            plan_probe_action(&linux_config, &linux, ProbeAction::LogsDbDryRun).unwrap(),
        )
        .unwrap(),
        serde_json::to_value(
            plan_probe_action(&linux_config, &linux, ProbeAction::LogsDbExecute).unwrap(),
        )
        .unwrap(),
        serde_json::to_value(
            plan_update_action(&linux_config, &linux, UpdateAction::Check).unwrap(),
        )
        .unwrap(),
        serde_json::to_value(
            plan_update_action(&linux_config, &linux, UpdateAction::Install).unwrap(),
        )
        .unwrap(),
        serde_json::to_value(
            plan_update_action(&linux_config, &linux, UpdateAction::Prune).unwrap(),
        )
        .unwrap(),
        serde_json::to_value(
            plan_thread_cleanup_action(&linux, ThreadCleanupAction::ArchiveDeleteDryRun).unwrap(),
        )
        .unwrap(),
        serde_json::to_value(
            plan_thread_cleanup_action(&linux, ThreadCleanupAction::ArchiveDeleteExecute).unwrap(),
        )
        .unwrap(),
        serde_json::to_value(
            plan_thread_cleanup_action(&linux, ThreadCleanupAction::HiddenDeleteDryRun).unwrap(),
        )
        .unwrap(),
        serde_json::to_value(
            plan_thread_cleanup_action(&linux, ThreadCleanupAction::HiddenDeleteExecute).unwrap(),
        )
        .unwrap(),
    ];

    values.extend([
        serde_json::to_value(archive_thread_response("thread-a".to_string(), true)).unwrap(),
        serde_json::to_value(archive_thread_response("thread-a".to_string(), false)).unwrap(),
        serde_json::to_value(rename_thread_response("thread-a".to_string(), "new name").unwrap())
            .unwrap(),
        serde_json::to_value(cancel_followup_response(
            commands::THREADS_FOLLOWUPS_CANCEL,
            "thread-a".to_string(),
            "followup-a".to_string(),
            true,
        ))
        .unwrap(),
    ]);

    for value in values {
        assert_no_retired_command(&value);
    }
}

fn assert_no_retired_command(value: &serde_json::Value) {
    match value {
        serde_json::Value::String(value) => {
            assert!(
                !commands::is_retired_command(value),
                "action plan emitted retired command: {value}"
            );
        }
        serde_json::Value::Array(items) => {
            for item in items {
                assert_no_retired_command(item);
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values() {
                assert_no_retired_command(item);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    let unique = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    std::env::temp_dir().join(format!("{label}-{unique}"))
}

fn thread_detail_with_blocks(thread_id: &str, block_count: usize) -> ThreadDetail {
    let blocks = (0..block_count)
        .map(|idx| MessageBlock {
            id: format!("b:{idx}"),
            role: "assistant".to_string(),
            kind: "message".to_string(),
            display_kind: None,
            status: None,
            text: Some(format!("block {idx}")),
            summary: None,
            input: None,
            truncated: None,
            resolved: None,
            answers: Vec::new(),
            plan_status: None,
            group_id: None,
            tool_name: None,
            call_id: None,
            turn_id: None,
            item_id: None,
            created_at: None,
            questions: Vec::new(),
            payload: None,
        })
        .collect::<Vec<_>>();

    ThreadDetail {
        summary: ThreadSummary {
            id: thread_id.to_string(),
            title: format!("Thread {thread_id}"),
            status: ThreadStatus::Recent,
            updated_at: None,
            archived_at: None,
            message_count: 1,
            latest_message: None,
            cwd: None,
            model: None,
            rollout_path: None,
            active_turn_id: None,
            active_job_id: None,
            pending_elicitation: None,
            last_event_kind: None,
        },
        messages: Vec::new(),
        blocks,
        raw_event_count: block_count,
        total_blocks: block_count,
        has_more_blocks: false,
        before_cursor: None,
    }
}
