use std::{fs, path::PathBuf};

fn src(path: &str) -> String {
    fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join(path),
    )
    .unwrap_or_default()
}

fn production_section(source: &str) -> &str {
    source
        .split("\n#[cfg(test)]\nmod tests {")
        .next()
        .expect("source should have a production section")
}

#[test]
fn api_entry_exposes_only_current_submodules() {
    let api = src("api.rs");
    for module in [
        "mod cleanup;",
        "mod grok;",
        "mod jobs;",
        "mod payload;",
        "mod probe;",
        "mod routes;",
        "mod rpc_dispatch;",
        "mod security;",
        "mod system;",
        "mod threads;",
        "mod web_auth;",
    ] {
        assert!(api.contains(module), "missing current API module: {module}");
    }
    for retired in ["mod goals;", "mod uploads;", "mod desktop_webui;"] {
        assert!(
            !api.contains(retired),
            "retired API module remained: {retired}"
        );
    }
    let production = production_section(&api);
    for handler in [
        "async fn rpc_dispatch",
        "async fn create_thread",
        "async fn send_message",
    ] {
        assert!(
            !production.contains(handler),
            "API entry owns retired handler: {handler}"
        );
    }
}

#[test]
fn routes_keep_health_events_and_single_rpc_transport_surface() {
    let routes = src("api/routes.rs");
    assert!(routes.contains(".route(\"/healthz\", get(healthz))"));
    assert!(routes.contains(".route(RPC_THREAD_EVENTS_ROUTE, get(thread_events))"));
    assert!(routes.contains(".route(RPC_COMMAND_ROUTE, post(rpc_dispatch))"));
    assert!(routes.contains("LEGACY_API_FALLBACK_ROUTE"));
    assert!(!routes.contains("RPC_UPLOAD_FILES_ROUTE"));
}

#[test]
fn rpc_surface_separates_retired_commands_from_current_allowlist() {
    let surface = src("rpc_surface.rs");
    let commands = src("../../nexushub-core/src/services/commands.rs");
    assert!(surface.contains("is_transport_rpc_command"));
    assert!(surface.contains("is_business_rpc_command"));
    assert!(commands.contains("RETIRED_COMMANDS"));
    assert!(commands.contains("grok.list"));
    assert!(commands.contains("grok.deleteExecute"));
    assert!(commands.contains("THREADS_SEND"));
    assert!(commands.contains("RETIRED_COMMANDS"));
}

#[test]
fn current_adapters_delegate_thread_reads_and_grok_operations() {
    let threads = production_section(&src("api/threads.rs")).to_string();
    let grok = production_section(&src("api/grok.rs")).to_string();
    assert!(threads.contains("list_threads"));
    assert!(threads.contains("thread_detail"));
    for operation in [
        "grok_list",
        "grok_detail",
        "grok_rename",
        "grok_delete_preview",
        "grok_delete_execute",
    ] {
        assert!(
            grok.contains(operation),
            "missing Grok adapter operation: {operation}"
        );
    }
    for retired in [
        "create_thread",
        "send_message",
        "enqueue_followup",
        "stop_thread",
        "fork_thread",
    ] {
        assert!(
            !threads.contains(retired),
            "retired thread operation remained: {retired}"
        );
    }
}

#[test]
fn desktop_lan_webui_and_upload_entrypoints_are_retired() {
    let lib = src("../lib.rs");
    let commands = src("../commands/mod.rs");
    let resources = src("../resources.rs");
    for source in [&lib, &commands, &resources] {
        assert!(!source.contains("desktop_webui"));
        assert!(!source.contains("startDesktopWebUi"));
    }
    assert!(!src("../commands/system.rs").contains("getClaudeCodeOverview"));
}
