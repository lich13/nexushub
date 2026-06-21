use nexushub_core::{
    config::Config,
    platform::{PlatformKind, PlatformPaths},
    services::{
        goals::{plan_goal_command_with_capability, GoalUpdateRequest},
        jobs::{
            plan_thread_command_with_capability, ThreadCommandKind, ThreadCommandRequest,
            ThreadMessageRequest,
        },
        probe::{plan_probe_action, ProbeAction, ProbeExecutionKind},
        system::Capability,
        threads::{
            plan_thread_blocks_request, plan_thread_cleanup_action, plan_thread_detail_request,
            ThreadCleanupAction, ThreadDetailRequest,
        },
        updates::{plan_update_action, UpdateAction, UpdateExecutionMethod},
        uploads::{plan_store_uploads_with_capability, UploadBatchItem},
    },
};

#[test]
fn probe_and_update_actions_expose_shared_rpc_and_desktop_command_names() {
    assert_eq!(ProbeAction::BarkTest.as_rpc_action(), "bark-test");
    assert_eq!(ProbeAction::BarkTest.as_desktop_command(), "probe.barkTest");
    assert_eq!(ProbeAction::InstallHooks.as_rpc_action(), "hooks-install");
    assert_eq!(
        ProbeAction::InstallHooks.as_desktop_command(),
        "probe.installHooks"
    );
    assert_eq!(ProbeAction::LogsDbDryRun.as_rpc_action(), "logs-db-dry-run");
    assert_eq!(
        ProbeAction::LogsDbDryRun.as_desktop_command(),
        "probe.logsDbDryRun"
    );
    assert_eq!(
        ProbeAction::LogsDbExecute.as_rpc_action(),
        "logs-db-execute"
    );
    assert_eq!(
        ProbeAction::LogsDbExecute.as_desktop_command(),
        "probe.logsDbExecute"
    );

    assert_eq!(UpdateAction::Check.as_rpc_action(), "check");
    assert_eq!(UpdateAction::Check.as_desktop_command(), "updates.check");
    assert_eq!(UpdateAction::Install.as_rpc_action(), "install");
    assert_eq!(
        UpdateAction::Install.as_desktop_command(),
        "updates.install"
    );
    assert_eq!(UpdateAction::Prune.as_rpc_action(), "prune");
    assert_eq!(UpdateAction::Prune.as_desktop_command(), "updates.prune");
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
    assert_eq!(mac_check.native.as_ref().unwrap().command, "check");

    let mac_install = plan_update_action(&mac_config, &mac, UpdateAction::Install).unwrap();
    assert_eq!(mac_install.native.as_ref().unwrap().command, "install");

    let err = plan_update_action(&mac_config, &mac, UpdateAction::Prune)
        .expect_err("backup prune is a Linux-only update action");
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
