use nexushub_core::{
    db::ThreadFollowUp,
    platform::{PlatformKind, PlatformPaths},
    services::{
        commands,
        jobs::{
            FollowUpClaimRequest, FollowUpErrorRequest, FollowUpSubmitResultRequest,
            ThreadMessageRequest, ThreadRenameRequest, ThreadSendRequest, ThreadSteerRequest,
            ThreadStopRequest,
        },
        system::Capability,
        uploads::{UploadBatchItem, UploadRetentionRequest},
        use_cases::NexusHubUseCases,
    },
};
use serde_json::json;

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
fn new_core_facade_sources_do_not_import_host_runtime_surfaces() {
    for (name, source) in [
        ("use_cases", include_str!("../src/services/use_cases.rs")),
        ("jobs", include_str!("../src/services/jobs.rs")),
        ("uploads", include_str!("../src/services/uploads.rs")),
        ("cleanup", include_str!("../src/services/cleanup.rs")),
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
