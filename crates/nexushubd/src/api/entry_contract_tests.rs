use std::{fs, path::PathBuf};

fn src(path: &str) -> String {
    fs::read_to_string(src_path(path)).unwrap_or_default()
}

fn src_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(path)
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

    let production = api
        .split("\n#[cfg(test)]")
        .next()
        .expect("api source should have a production section");
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
