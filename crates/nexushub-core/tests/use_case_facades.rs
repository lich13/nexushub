use nexushub_core::{
    codex::ThreadStatus,
    config::Config,
    db::{JobRecord, SecuritySettings, ThreadFollowUp},
    platform::{PlatformKind, PlatformPaths},
    services::{
        cleanup::{CleanupExecuteRequest, CleanupOperationKind, CleanupTarget},
        commands,
        jobs::{
            FollowUpClaimRequest, FollowUpErrorRequest, FollowUpSubmitResultRequest,
            ThreadMessageRequest, ThreadRenameRequest, ThreadSendRequest, ThreadSteerRequest,
            ThreadStopRequest,
        },
        probe::{ProbeAction, ProbeExecutionKind, ProbeLogsDbMaintenanceRequest},
        security::{PasswordChangeRequest, SecurityPatch},
        settings::{
            ProbeNotificationsSavePatch, ProbeSecretState, ProbeSettingsSavePatch,
            ProbeSettingsSaveRequest,
        },
        system::Capability,
        updates::{UpdateAction, UpdateExecutionMethod},
        uploads::{UploadBatchItem, UploadRetentionRequest},
        use_cases::NexusHubUseCases,
    },
};
use serde_json::json;
use std::path::PathBuf;

#[test]
fn thread_use_cases_emit_same_action_plans_for_linux_and_macos_adapters() {
    let linux_platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let linux = NexusHubUseCases::new(&linux_platform);
    let mac_home = temp_dir("nexushub-use-case-macos");
    std::fs::create_dir_all(&mac_home).unwrap();
    let mac_platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &mac_home);
    let macos = NexusHubUseCases::new(&mac_platform);

    let create = ThreadMessageRequest {
        message: "  start work  ".to_string(),
        model: Some("gpt-5.5".to_string()),
        ..ThreadMessageRequest::default()
    };
    assert_eq!(
        serde_json::to_value(linux.threads().create(create.clone()).unwrap()).unwrap(),
        serde_json::to_value(macos.threads().create(create).unwrap()).unwrap()
    );

    let send = ThreadSendRequest {
        thread_id: Some(" thread-a ".to_string()),
        message: ThreadMessageRequest {
            message: " continue ".to_string(),
            service_tier: Some("priority".to_string()),
            ..ThreadMessageRequest::default()
        },
    };
    assert_eq!(
        serde_json::to_value(linux.threads().send(send.clone()).unwrap()).unwrap(),
        serde_json::to_value(macos.threads().send(send).unwrap()).unwrap()
    );

    let steer = ThreadSteerRequest {
        thread_id: Some(" thread-a ".to_string()),
        message: ThreadMessageRequest {
            message: " queue this ".to_string(),
            ..ThreadMessageRequest::default()
        },
    };
    assert_eq!(
        serde_json::to_value(linux.threads().steer(steer.clone()).unwrap()).unwrap(),
        serde_json::to_value(macos.threads().steer(steer).unwrap()).unwrap()
    );

    let stop = ThreadStopRequest {
        thread_id: " thread-a ".to_string(),
        turn_id: Some(" turn-a ".to_string()),
        job_id: Some(" job-a ".to_string()),
    };
    assert_eq!(
        serde_json::to_value(linux.threads().stop(stop.clone()).unwrap()).unwrap(),
        serde_json::to_value(macos.threads().stop(stop).unwrap()).unwrap()
    );

    assert_eq!(
        serde_json::to_value(linux.threads().archive(" thread-a ").unwrap()).unwrap(),
        serde_json::to_value(macos.threads().archive(" thread-a ").unwrap()).unwrap()
    );
    assert_eq!(
        serde_json::to_value(linux.threads().restore(" thread-a ").unwrap()).unwrap(),
        serde_json::to_value(macos.threads().restore(" thread-a ").unwrap()).unwrap()
    );

    let rename = ThreadRenameRequest {
        thread_id: " thread-a ".to_string(),
        name: " New name ".to_string(),
    };
    assert_eq!(
        serde_json::to_value(linux.threads().rename(rename.clone()).unwrap()).unwrap(),
        serde_json::to_value(macos.threads().rename(rename).unwrap()).unwrap()
    );

    std::fs::remove_dir_all(mac_home).unwrap();
}

#[test]
fn followup_use_cases_plan_claim_submit_completion_and_error_without_adapter_logic() {
    let linux_platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let linux = NexusHubUseCases::new(&linux_platform);
    let mac_home = temp_dir("nexushub-followup-use-case-macos");
    std::fs::create_dir_all(&mac_home).unwrap();
    let mac_platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &mac_home);
    let macos = NexusHubUseCases::new(&mac_platform);

    let claim = FollowUpClaimRequest {
        thread_id: " thread-a ".to_string(),
    };
    let linux_claim = linux.threads().claim_followup(claim.clone()).unwrap();
    let mac_claim = macos.threads().claim_followup(claim).unwrap();
    assert_eq!(linux_claim, mac_claim);
    assert_eq!(linux_claim.required_capability, Capability::Jobs);
    assert_eq!(linux_claim.command, commands::THREADS_FOLLOWUPS_CLAIM);
    assert_eq!(linux_claim.thread_id, "thread-a");
    assert_eq!(linux_claim.from_status, "pending");
    assert_eq!(linux_claim.to_status, "submitting");

    let followup = ThreadFollowUp {
        id: "followup-a".to_string(),
        thread_id: "thread-a".to_string(),
        status: "submitting".to_string(),
        message: "  continue with queued work  ".to_string(),
        options_json: json!({
            "model": "gpt-5.5",
            "attachments": ["018f0a59-f18a-7fa9-98fb-3bd51964d001"],
            "cwd": " /tmp/work "
        })
        .to_string(),
        created_at: 1,
        updated_at: 2,
        submitted_at: None,
        cancelled_at: None,
        result_json: None,
        error: None,
    };
    let submit = linux.threads().submit_followup(&followup).unwrap();
    assert_eq!(submit.required_capability, Capability::Jobs);
    assert_eq!(submit.command, commands::THREADS_FOLLOWUPS_SUBMIT);
    assert_eq!(submit.followup_id, "followup-a");
    assert_eq!(submit.thread_id, "thread-a");
    assert_eq!(submit.action.thread_id.as_deref(), Some("thread-a"));
    assert_eq!(submit.action.message, "continue with queued work");
    assert_eq!(submit.action.model.as_deref(), Some("gpt-5.5"));

    let completed = linux
        .threads()
        .mark_followup_submitted(FollowUpSubmitResultRequest {
            followup_id: " followup-a ".to_string(),
            result: json!({"job_id": "job-a", "turn_id": "turn-a"}),
        })
        .unwrap();
    assert_eq!(completed.command, commands::THREADS_FOLLOWUPS_SUBMIT);
    assert_eq!(completed.followup_id, "followup-a");
    assert_eq!(completed.result["job_id"], "job-a");

    let errored = linux
        .threads()
        .mark_followup_error(FollowUpErrorRequest {
            followup_id: " followup-a ".to_string(),
            error: " local state unavailable ".to_string(),
        })
        .unwrap();
    assert_eq!(errored.command, commands::THREADS_FOLLOWUPS_ERROR);
    assert_eq!(errored.followup_id, "followup-a");
    assert_eq!(errored.error, "local state unavailable");

    assert!(linux
        .threads()
        .submit_followup(&ThreadFollowUp {
            status: "pending".to_string(),
            ..followup
        })
        .unwrap_err()
        .to_string()
        .contains("claimed"));

    std::fs::remove_dir_all(mac_home).unwrap();
}

#[test]
fn followup_autosubmit_use_case_plans_claim_resume_completion_and_audit_semantics() {
    let linux_platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let linux = NexusHubUseCases::new(&linux_platform);
    let mac_home = temp_dir("nexushub-followup-autosubmit-macos");
    std::fs::create_dir_all(&mac_home).unwrap();
    let mac_platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &mac_home);
    let macos = NexusHubUseCases::new(&mac_platform);

    let followup = ThreadFollowUp {
        id: " followup-a ".to_string(),
        thread_id: " thread-a ".to_string(),
        status: "submitting".to_string(),
        message: "  continue queued work  ".to_string(),
        options_json: json!({
            "model": "gpt-5.5",
            "network_access": true,
            "cwd": " /tmp/project "
        })
        .to_string(),
        created_at: 1,
        updated_at: 2,
        submitted_at: None,
        cancelled_at: None,
        result_json: None,
        error: None,
    };

    let linux_plan = linux
        .threads()
        .autosubmit_followup_job(
            ThreadStatus::Recent,
            &followup,
            PathBuf::from("/default/workspace"),
        )
        .unwrap();
    let mac_plan = macos
        .threads()
        .autosubmit_followup_job(
            ThreadStatus::Recent,
            &followup,
            PathBuf::from("/default/workspace"),
        )
        .unwrap();
    assert_eq!(
        serde_json::to_value(&linux_plan).unwrap(),
        serde_json::to_value(&mac_plan).unwrap()
    );
    assert!(linux_plan.autosubmit.should_claim_pending);
    assert!(linux_plan.autosubmit.should_start_resume_job);
    assert_eq!(
        linux_plan.claim.as_ref().unwrap().command,
        commands::THREADS_FOLLOWUPS_CLAIM
    );

    let job = linux_plan.job.as_ref().unwrap();
    assert_eq!(job.command, commands::THREADS_FOLLOWUPS_SUBMIT);
    assert_eq!(job.spec.title, "Codex queued follow-up");
    assert_eq!(job.spec.thread_id.as_deref(), Some("thread-a"));
    assert_eq!(job.spec.cwd, PathBuf::from("/tmp/project"));
    assert_eq!(job.link.thread_id.as_deref(), Some("thread-a"));
    assert_eq!(job.response.thread_id.as_deref(), Some("thread-a"));
    assert_eq!(job.audit.action, "thread.followup.autosubmit_job_started");
    assert_eq!(job.audit.target_type, "thread");
    assert_eq!(job.audit.target_id.as_deref(), Some("thread-a"));
    assert_eq!(job.audit.detail["followup_id"], "followup-a");
    assert_eq!(job.audit.detail["job_fallback"], true);

    let submitted = linux_plan.submitted_result(" job-a ").unwrap();
    assert_eq!(submitted.command, commands::THREADS_FOLLOWUPS_SUBMIT);
    assert_eq!(submitted.followup_id, "followup-a");
    assert_eq!(submitted.result["job_id"], "job-a");
    let submitted_response = linux_plan.submitted_response(" job-a ").unwrap();
    assert_eq!(
        submitted_response.data.unwrap()["followup_id"],
        "followup-a"
    );

    let errored = linux_plan.error_result(" local spawn failed ").unwrap();
    assert_eq!(errored.command, commands::THREADS_FOLLOWUPS_ERROR);
    assert_eq!(errored.followup_id, "followup-a");
    assert_eq!(errored.error, "local spawn failed");
    let error_response = linux_plan.error_response(" local spawn failed ").unwrap();
    assert_eq!(error_response.data.unwrap()["error"], "local spawn failed");

    assert!(linux
        .threads()
        .autosubmit_followup_job(
            ThreadStatus::Running,
            &followup,
            PathBuf::from("/default/workspace"),
        )
        .unwrap()
        .job
        .is_none());

    std::fs::remove_dir_all(mac_home).unwrap();
}

#[test]
fn thread_command_lifecycle_use_cases_plan_codex_job_link_response_and_audit() {
    let linux_platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let use_cases = NexusHubUseCases::new(&linux_platform);

    let create = use_cases
        .threads()
        .create_job(
            ThreadMessageRequest {
                message: "  start work  ".to_string(),
                model: Some("gpt-5.5".to_string()),
                ..ThreadMessageRequest::default()
            },
            PathBuf::from("/workspace"),
        )
        .unwrap();
    assert_eq!(create.command, commands::THREADS_CREATE);
    assert_eq!(create.spec.title, "Codex new thread");
    assert_eq!(create.spec.cwd, PathBuf::from("/workspace"));
    assert!(create.link.thread_id.is_none());
    assert!(create.response.thread_id.is_none());
    assert_eq!(create.audit.action, "thread.create.job_started");
    assert_eq!(create.audit.target_type, "job");
    assert_eq!(create.audit.detail["cwd"], "/workspace");
    let create_response = create.submitted_response(" job-new ").unwrap();
    assert_eq!(create_response.job_id.as_deref(), Some("job-new"));
    assert!(create_response.thread_id.is_none());
    assert_eq!(
        create.audit_detail(" job-new ").unwrap()["job_id"],
        "job-new"
    );

    let send = use_cases
        .threads()
        .send_job(
            ThreadSendRequest {
                thread_id: Some(" thread-a ".to_string()),
                message: ThreadMessageRequest {
                    message: "  continue  ".to_string(),
                    reasoning_effort: Some("high".to_string()),
                    ..ThreadMessageRequest::default()
                },
            },
            PathBuf::from("/workspace"),
        )
        .unwrap();
    assert_eq!(send.command, commands::THREADS_SEND);
    assert_eq!(send.spec.title, "Codex resume thread");
    assert_eq!(send.spec.thread_id.as_deref(), Some("thread-a"));
    assert_eq!(send.link.thread_id.as_deref(), Some("thread-a"));
    assert_eq!(send.link.turn_id, None);
    assert_eq!(send.response.thread_id.as_deref(), Some("thread-a"));
    assert_eq!(send.audit.action, "thread.message.job_started");
    assert_eq!(send.audit.target_type, "thread");
    assert_eq!(send.audit.target_id.as_deref(), Some("thread-a"));
    let send_response = send.submitted_response(" job-a ").unwrap();
    assert_eq!(send_response.thread_id.as_deref(), Some("thread-a"));
    assert_eq!(send_response.job_id.as_deref(), Some("job-a"));
}

#[test]
fn upload_use_cases_plan_validation_store_delete_and_retention_policy() {
    let linux_platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let linux = NexusHubUseCases::new(&linux_platform);
    let mac_home = temp_dir("nexushub-upload-use-case-macos");
    std::fs::create_dir_all(&mac_home).unwrap();
    let mac_platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &mac_home);
    let macos = NexusHubUseCases::new(&mac_platform);

    let batch = vec![UploadBatchItem {
        name: " ../notes.md ".to_string(),
        mime: None,
        bytes: b"# Notes".to_vec(),
    }];
    let validation = linux.uploads().validate(&batch).unwrap();
    assert_eq!(validation.required_capability, Capability::Jobs);
    assert_eq!(validation.total_files, 1);

    assert_eq!(
        serde_json::to_value(linux.uploads().store(batch.clone()).unwrap()).unwrap(),
        serde_json::to_value(macos.uploads().store(batch).unwrap()).unwrap()
    );

    let delete = linux
        .uploads()
        .delete(" 018f0a59-f18a-7fa9-98fb-3bd51964d001 ")
        .unwrap();
    assert_eq!(delete.id, "018f0a59-f18a-7fa9-98fb-3bd51964d001");

    let retention = linux
        .uploads()
        .retention(UploadRetentionRequest {
            protected_ids: vec![
                "018f0a59-f18a-7fa9-98fb-3bd51964d001".to_string(),
                "018f0a59-f18a-7fa9-98fb-3bd51964d001".to_string(),
                " 018f0a59-f18a-7fa9-98fb-3bd51964d002 ".to_string(),
            ],
            ttl_seconds: Some(60),
        })
        .unwrap();
    assert_eq!(retention.required_capability, Capability::Jobs);
    assert_eq!(retention.ttl_seconds, 60);
    assert_eq!(
        retention.protected_ids,
        vec![
            "018f0a59-f18a-7fa9-98fb-3bd51964d001".to_string(),
            "018f0a59-f18a-7fa9-98fb-3bd51964d002".to_string(),
        ]
    );

    assert!(linux
        .uploads()
        .retention(UploadRetentionRequest {
            protected_ids: vec!["not-a-uuid".to_string()],
            ttl_seconds: None,
        })
        .unwrap_err()
        .to_string()
        .contains("invalid upload id"));

    std::fs::remove_dir_all(mac_home).unwrap();
}

#[test]
fn read_and_history_use_cases_expose_complete_adapter_transaction_plans() {
    let linux_platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let use_cases = NexusHubUseCases::new(&linux_platform);

    let list = use_cases
        .threads()
        .list_read(nexushub_core::services::threads::ThreadsQuery {
            status: Some(" running ".to_string()),
            q: Some(" build ".to_string()),
            limit: Some(25),
        })
        .unwrap();
    assert_eq!(list.list.required_capability, Capability::Threads);
    assert_eq!(list.list.query.status.as_deref(), Some("running"));
    assert_eq!(list.list.query.q.as_deref(), Some("build"));
    assert!(list.include_hidden_thread_ids);
    assert!(list.include_archived_thread_ids);
    assert!(list.include_running_jobs);

    let detail = use_cases
        .threads()
        .detail_read(nexushub_core::services::threads::ThreadDetailRequest {
            id: " thread-a ".to_string(),
            limit: Some(10),
            full: None,
            before: None,
        })
        .unwrap();
    assert_eq!(detail.detail.thread_id, "thread-a");
    assert!(detail.include_active_job);

    let job_list = use_cases.jobs().list(Some(500)).unwrap();
    assert_eq!(job_list.required_capability, Capability::JobHistory);
    assert_eq!(job_list.limit, 200);

    let job_detail = use_cases.jobs().detail(" job-a ").unwrap();
    assert_eq!(job_detail.required_capability, Capability::JobHistory);
    assert_eq!(job_detail.job_id, "job-a");

    let failed = use_cases.jobs().response(JobRecord {
        id: "job-a".to_string(),
        kind: "codex".to_string(),
        status: "failed".to_string(),
        title: "Codex".to_string(),
        thread_id: Some("thread-a".to_string()),
        turn_id: None,
        started_at: 1,
        finished_at: Some(2),
        exit_code: Some(1),
        output: "auth failed".to_string(),
        error: Some("401 Unauthorized".to_string()),
    });
    assert_eq!(failed.job.id, "job-a");
    assert!(failed.failure_analysis.is_some());

    let listed = use_cases.jobs().list_response(vec![
        JobRecord {
            id: "job-a".to_string(),
            kind: "codex".to_string(),
            status: "failed".to_string(),
            title: "Codex".to_string(),
            thread_id: Some("thread-a".to_string()),
            turn_id: None,
            started_at: 1,
            finished_at: Some(2),
            exit_code: Some(1),
            output: "auth failed".to_string(),
            error: Some("401 Unauthorized".to_string()),
        },
        JobRecord {
            id: "job-b".to_string(),
            kind: "nexushub_update_check".to_string(),
            status: "succeeded".to_string(),
            title: "Update".to_string(),
            thread_id: None,
            turn_id: None,
            started_at: 3,
            finished_at: Some(4),
            exit_code: Some(0),
            output: "ok".to_string(),
            error: None,
        },
    ]);
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].job.id, "job-a");
    assert!(listed[0].failure_analysis.is_some());
    assert_eq!(listed[1].job.id, "job-b");
    assert!(listed[1].failure_analysis.is_none());

    let detail_response = use_cases
        .jobs()
        .detail_response(Some(JobRecord {
            id: "job-c".to_string(),
            kind: "codex".to_string(),
            status: "succeeded".to_string(),
            title: "Codex".to_string(),
            thread_id: Some("thread-c".to_string()),
            turn_id: None,
            started_at: 5,
            finished_at: Some(6),
            exit_code: Some(0),
            output: "done".to_string(),
            error: None,
        }))
        .unwrap();
    assert_eq!(detail_response.job.id, "job-c");
    assert!(use_cases.jobs().detail_response(None).is_none());
}

#[test]
fn upload_and_cleanup_use_cases_expose_execute_ready_plans_without_host_types() {
    let linux_platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let use_cases = NexusHubUseCases::new(&linux_platform);

    let delete = use_cases
        .uploads()
        .delete_execute(" 018f0a59-f18a-7fa9-98fb-3bd51964d001 ")
        .unwrap();
    assert_eq!(delete.required_capability, Capability::Jobs);
    assert_eq!(delete.id, "018f0a59-f18a-7fa9-98fb-3bd51964d001");

    let dry_run = use_cases
        .cleanup()
        .operation(CleanupTarget::Archived, CleanupOperationKind::DryRun)
        .unwrap();
    assert_eq!(dry_run.required_capability, Capability::ThreadCleanup);
    assert_eq!(dry_run.target, CleanupTarget::Archived);
    assert_eq!(dry_run.operation, CleanupOperationKind::DryRun);
    assert!(!dry_run.execute);
    assert!(!dry_run.requires_prior_dry_run);

    let execute = use_cases.cleanup().execute(CleanupTarget::Hidden).unwrap();
    assert_eq!(execute.target, CleanupTarget::Hidden);
    assert_eq!(execute.operation, CleanupOperationKind::Execute);
    assert!(execute.execute);
    assert!(execute.requires_confirmation);
    assert!(execute.requires_prior_dry_run);
    assert_eq!(execute.confirmation.expected_count, None);
    assert!(!execute.confirmation.confirmed);
    assert_eq!(
        execute.confirmation.payload,
        json!({"confirmed": false, "expectedCount": null})
    );

    let confirmed = use_cases
        .cleanup()
        .execute_confirmed(
            CleanupTarget::Hidden,
            CleanupExecuteRequest {
                confirmed: true,
                expected_count: Some(3),
            },
        )
        .unwrap();
    assert_eq!(confirmed.target, CleanupTarget::Hidden);
    assert!(confirmed.confirmation.confirmed);
    assert_eq!(confirmed.confirmation.expected_count, Some(3));
    assert_eq!(
        confirmed.confirmation.payload,
        json!({"confirmed": true, "expectedCount": 3})
    );

    assert!(use_cases
        .cleanup()
        .execute_confirmed(
            CleanupTarget::Archived,
            CleanupExecuteRequest {
                confirmed: false,
                expected_count: Some(1),
            },
        )
        .unwrap_err()
        .to_string()
        .contains("confirmed"));
}

#[test]
fn cleanup_confirmation_plan_is_shared_for_linux_and_macos_execute_adapters() {
    let linux_platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let linux = NexusHubUseCases::new(&linux_platform);
    let mac_home = temp_dir("nexushub-cleanup-confirmation-macos");
    std::fs::create_dir_all(&mac_home).unwrap();
    let mac_platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &mac_home);
    let macos = NexusHubUseCases::new(&mac_platform);

    let request = CleanupExecuteRequest {
        confirmed: true,
        expected_count: Some(8),
    };
    let linux_plan = linux
        .cleanup()
        .execute_confirmed(CleanupTarget::Archived, request.clone())
        .unwrap();
    let mac_plan = macos
        .cleanup()
        .execute_confirmed(CleanupTarget::Archived, request)
        .unwrap();

    assert_eq!(
        serde_json::to_value(&linux_plan).unwrap(),
        serde_json::to_value(&mac_plan).unwrap()
    );
    assert_eq!(linux_plan.command, commands::CLEANUP_ARCHIVE_EXECUTE);
    assert!(linux_plan.confirmation.confirmed);
    assert_eq!(linux_plan.confirmation.expected_count, Some(8));
    assert_eq!(
        linux_plan.confirmation.payload,
        json!({"confirmed": true, "expectedCount": 8})
    );

    std::fs::remove_dir_all(mac_home).unwrap();
}

#[test]
fn operational_use_cases_expose_probe_settings_updates_system_and_security_plans() {
    let linux_config = Config::for_platform_kind(PlatformKind::Linux);
    let linux_platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let linux = NexusHubUseCases::with_config(&linux_config, &linux_platform);

    let settings_view = linux
        .settings()
        .unwrap()
        .probe_settings_view(ProbeSecretState::Configured)
        .unwrap();
    assert_eq!(settings_view.required_capability, Capability::Settings);
    assert!(settings_view.settings.notifications.device_key_configured);

    let settings_save = linux
        .settings()
        .unwrap()
        .save_probe_settings(ProbeSettingsSaveRequest {
            probe: Some(ProbeSettingsSavePatch {
                notifications: Some(ProbeNotificationsSavePatch {
                    device_key: Some(" top-level-bark-key ".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(settings_save.required_capability, Capability::Settings);
    assert_eq!(
        settings_save.bark_device_key.as_deref(),
        Some("top-level-bark-key")
    );
    let serialized_save = serde_json::to_string(&settings_save).unwrap();
    assert!(serialized_save.contains("[configured]"));
    assert!(!serialized_save.contains("top-level-bark-key"));

    let bark = linux
        .probe()
        .unwrap()
        .action_with_device_key(ProbeAction::BarkTest, true)
        .unwrap();
    assert_eq!(bark.required_capability, Capability::Probe);
    assert_eq!(bark.execution, ProbeExecutionKind::FixedShellJob);
    assert_eq!(bark.diagnostic_plan.unwrap().kind, "bark-test".to_string());

    let maintenance = linux
        .probe()
        .unwrap()
        .logs_db_maintenance(ProbeLogsDbMaintenanceRequest {
            dry_run: false,
            compact: true,
        })
        .unwrap();
    assert_eq!(
        maintenance.required_capability,
        Capability::ProbeLogMaintenance
    );
    assert!(maintenance.maintenance.unwrap().compact);

    let update_status = linux.updates().unwrap().status(None, None).unwrap();
    assert_eq!(update_status.required_capability, Capability::AppUpdater);
    let check = linux.updates().unwrap().check_plan().unwrap();
    assert_eq!(check.required_capability, Capability::LinuxUpdateJob);
    assert_eq!(check.action, UpdateAction::Check);
    assert_eq!(check.method, UpdateExecutionMethod::LinuxSystemdJob);
    let prune = linux.updates().unwrap().prune_plan().unwrap();
    assert_eq!(prune.required_capability, Capability::PruneBackups);
    assert_eq!(prune.action, UpdateAction::Prune);

    let capabilities = linux.system().unwrap().capabilities();
    assert!(capabilities.probe);
    assert!(capabilities.prune_backups);
    let gate = linux
        .system()
        .unwrap()
        .capability_gate(Capability::PruneBackups);
    assert!(gate.supported);
    assert_eq!(gate.capability, Capability::PruneBackups);

    let security_settings = SecuritySettings {
        turnstile_enabled: true,
        turnstile_required: true,
        turnstile_site_key: None,
        turnstile_secret_configured: false,
        session_ttl_seconds: 900,
    };
    let public_security = linux
        .security()
        .unwrap()
        .public_view(security_settings.clone(), None, true, None)
        .unwrap();
    assert_eq!(public_security.required_capability, Capability::WebAuth);
    assert_eq!(public_security.public.turnstile_action, "login");
    let security_patch = linux
        .security()
        .unwrap()
        .patch(SecurityPatch {
            turnstile_secret_key: Some(" secret-key ".to_string()),
            session_ttl_seconds: Some(900),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        security_patch.required_capability,
        Capability::SecuritySettings
    );
    assert_eq!(
        security_patch.patch.turnstile_secret_key.as_deref(),
        Some("secret-key")
    );
    let password = linux
        .security()
        .unwrap()
        .change_password(
            PasswordChangeRequest {
                current_password: "old-password".to_string(),
                new_password: "new-password-123".to_string(),
            },
            true,
        )
        .unwrap();
    assert_eq!(password.required_capability, Capability::AdminPassword);
}

#[test]
fn macos_prune_plan_fails_at_capability_gate_without_linux_remediation_advice() {
    let home = temp_dir("nexushub-top-level-macos");
    std::fs::create_dir_all(&home).unwrap();
    let config = Config::for_platform_kind_with_home(PlatformKind::Macos, &home);
    let platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, &home);
    let use_cases = NexusHubUseCases::with_config(&config, &platform);

    let err = use_cases
        .updates()
        .unwrap()
        .prune_plan()
        .expect_err("macOS must not plan Linux backup pruning");
    let message = err.to_string();
    assert!(message.contains("prune_backups is unavailable on macos"));
    for forbidden in [
        "Linux",
        "linux server",
        "systemd",
        "Nginx",
        "nginx",
        "sudo",
        "systemctl",
    ] {
        assert!(
            !message.contains(forbidden),
            "macOS prune error must not include Linux host advice: {message}"
        );
    }

    let gate = use_cases
        .system()
        .unwrap()
        .capability_gate(Capability::PruneBackups);
    assert!(!gate.supported);
    assert_eq!(gate.error.as_deref(), Some(message.as_str()));
    assert!(!use_cases.system().unwrap().capabilities().prune_backups);

    std::fs::remove_dir_all(home).unwrap();
}

#[test]
fn config_backed_use_cases_report_missing_config_in_core() {
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let use_cases = NexusHubUseCases::new(&platform);

    for result in [
        use_cases.settings().map(|_| ()),
        use_cases.probe().map(|_| ()),
        use_cases.updates().map(|_| ()),
        use_cases.system().map(|_| ()),
        use_cases.security().map(|_| ()),
    ] {
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("config is required"));
    }
}

#[test]
fn new_core_facade_sources_do_not_import_host_runtime_surfaces() {
    for (name, source) in [
        ("use_cases", include_str!("../src/services/use_cases.rs")),
        ("jobs", include_str!("../src/services/jobs.rs")),
        ("uploads", include_str!("../src/services/uploads.rs")),
        ("cleanup", include_str!("../src/services/cleanup.rs")),
        ("settings", include_str!("../src/services/settings.rs")),
        ("probe", include_str!("../src/services/probe.rs")),
        ("updates", include_str!("../src/services/updates.rs")),
        ("system", include_str!("../src/services/system.rs")),
        ("security", include_str!("../src/services/security.rs")),
    ] {
        for forbidden in ["axum", "tauri", "nexushubd", "src-tauri", "HeaderMap"] {
            assert!(
                !source.contains(forbidden),
                "{name} service source must not depend on host runtime surface {forbidden}"
            );
        }
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
