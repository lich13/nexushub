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

#[test]
fn api_entry_delegates_transport_dispatch_and_payload_to_submodules() {
    let api = src("api.rs");
    for module in ["mod routes;", "mod rpc_dispatch;", "mod payload;"] {
        assert!(
            api.contains(module),
            "api.rs should declare thin API submodule: {module}"
        );
    }

    let production = production_section(&api);
    for forbidden in [
        "Router::new()",
        "async fn rpc_dispatch",
        "fn rpc_payload<",
        "fn rpc_wrapped_payload<",
        "fn rpc_nested_payload<",
        "fn rpc_required_string(",
    ] {
        assert!(
            !production.contains(forbidden),
            "api.rs should delegate transport/dispatch/payload concerns: {forbidden}"
        );
    }
}

#[test]
fn api_entry_does_not_reimplement_domain_or_linux_execution_boundaries() {
    let api = src("api.rs");
    let production = production_section(&api);
    let adapter = src("linux_adapter.rs");

    for required in [
        "linux_adapter::list_threads_read_model",
        "linux_adapter::load_thread_detail_read_model",
        "linux_adapter::autosubmit_ready_followups",
        "linux_adapter::autosubmit_pending_followup",
        "linux_adapter::execute_cleanup_plan",
        "linux_adapter::list_jobs_plan",
        "linux_adapter::job_detail_plan",
    ] {
        assert!(
            production.contains(required),
            "api.rs should call the core/linux adapter boundary: {required}"
        );
    }

    for forbidden in [
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
        "if plan.requires_confirmation && !payload.confirmed",
        "build_threads_overview(",
        "merge_running_jobs(",
        "apply_running_job_to_summary(",
        "normalize_thread_detail_block_limit(",
        "normalize_thread_block_limit(",
    ] {
        assert!(
            !production.contains(forbidden),
            "api.rs should delegate domain orchestration to core/linux_adapter: {forbidden}"
        );
    }

    for allowed_adapter_landing in [
        "claim_next_followup(",
        "apply_followup_submitted(",
        "apply_followup_error(",
        "state.jobs.start_codex_job(",
        "codex::set_thread_archived(",
        "cleanup_service::execute_archived_with_capability(",
        "cleanup_service::execute_hidden_with_capability(",
    ] {
        assert!(
            adapter.contains(allowed_adapter_landing),
            "linux_adapter should keep the minimal fixed side-effect landing: {allowed_adapter_landing}"
        );
    }
}
