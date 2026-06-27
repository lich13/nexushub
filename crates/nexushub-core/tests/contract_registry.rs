use nexushub_core::platform::{PlatformKind, PlatformPaths};
use nexushub_core::services::{
    commands,
    system::{Capability, HostSurface},
};
use serde_json::Value;
use std::collections::BTreeSet;

fn contract() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/nexushub-contract.json"
    );
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("contract registry must exist at {path}: {err}"));
    serde_json::from_str(&text).expect("contract registry must be valid JSON")
}

fn string_set(value: &Value, key: &str) -> BTreeSet<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("contract key {key} must be an array"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("contract key {key} entries must be strings"))
                .to_string()
        })
        .collect()
}

fn nested_string_set(value: &Value, object_key: &str, key: &str) -> BTreeSet<String> {
    value
        .get(object_key)
        .unwrap_or_else(|| panic!("contract key {object_key} must exist"))
        .get(key)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("contract key {object_key}.{key} must be an array"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| {
                    panic!("contract key {object_key}.{key} entries must be strings")
                })
                .to_string()
        })
        .collect()
}

fn action_set(contract: &Value, predicate: impl Fn(&Value) -> bool) -> BTreeSet<String> {
    contract
        .get("actions")
        .and_then(Value::as_array)
        .expect("contract actions must be an array")
        .iter()
        .filter(|action| predicate(action))
        .map(|action| {
            action
                .get("id")
                .and_then(Value::as_str)
                .expect("contract action id must be a string")
                .to_string()
        })
        .collect()
}

#[test]
fn contract_registry_matches_core_host_surfaces_and_capabilities() {
    let contract = contract();
    let expected_surfaces = HostSurface::ALL
        .iter()
        .map(|surface| surface.as_str().to_string())
        .collect::<BTreeSet<_>>();
    let expected_capabilities = Capability::all()
        .iter()
        .map(|capability| capability.as_str().to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(string_set(&contract, "hostSurfaces"), expected_surfaces);
    assert_eq!(string_set(&contract, "capabilities"), expected_capabilities);
}

#[test]
fn contract_registry_capability_matrix_matches_core_policy() {
    let contract = contract();
    for (surface, platform) in [
        (
            HostSurface::LinuxServerWebui,
            PlatformPaths::for_kind(PlatformKind::Linux),
        ),
        (
            HostSurface::DesktopEmbeddedTauri,
            PlatformPaths::for_kind(PlatformKind::Macos),
        ),
        (
            HostSurface::DesktopLanWebui,
            PlatformPaths::for_kind(PlatformKind::Macos),
        ),
    ] {
        let expected = Capability::all()
            .iter()
            .filter(|capability| capability.is_supported_on_surface(&platform, surface))
            .map(|capability| capability.as_str().to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            nested_string_set(&contract, "capabilitiesByHostSurface", surface.as_str()),
            expected,
            "contract capability matrix drifted for {}",
            surface.as_str()
        );
    }
}

#[test]
fn contract_registry_covers_all_linux_rpc_and_transport_commands() {
    let contract = contract();
    let rpc_actions = action_set(&contract, |action| {
        action
            .get("linuxRpc")
            .and_then(Value::as_str)
            .map(|value| !value.is_empty())
            .unwrap_or(false)
    });
    let transport_actions = action_set(&contract, |action| {
        action.get("kind").and_then(Value::as_str) == Some("transport")
    });

    let expected_rpc = commands::ALLOWED_RPC_COMMANDS
        .iter()
        .map(|command| command.to_string())
        .collect::<BTreeSet<_>>();
    let expected_transport = commands::ALLOWED_TRANSPORT_COMMANDS
        .iter()
        .map(|command| command.to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(rpc_actions, expected_rpc);
    assert_eq!(transport_actions, expected_transport);
}

#[test]
fn shared_actions_declare_core_webui_linux_rpc_and_tauri_mappings() {
    let contract = contract();
    let actions = contract
        .get("actions")
        .and_then(Value::as_array)
        .expect("contract actions must be an array");

    for action in actions {
        let id = action
            .get("id")
            .and_then(Value::as_str)
            .expect("contract action id must be a string");
        let kind = action
            .get("kind")
            .and_then(Value::as_str)
            .expect("contract action kind must be a string");
        let scope = action
            .get("scope")
            .and_then(Value::as_str)
            .expect("contract action scope must be a string");
        if let Some(linux_rpc) = action.get("linuxRpc").and_then(Value::as_str) {
            assert_eq!(
                linux_rpc, id,
                "contract action {id} must not introduce a second Linux RPC name"
            );
        }
        match scope {
            "shared" => {
                for key in ["coreUseCase", "linuxRpc", "tauriCommand", "webuiWrapper"] {
                    assert!(
                        action
                            .get(key)
                            .and_then(Value::as_str)
                            .map(|value| !value.trim().is_empty())
                            .unwrap_or(false),
                        "shared action {id} must declare {key}"
                    );
                }
            }
            "host_only" => {
                assert!(
                    action
                        .get("hostOnlyReason")
                        .and_then(Value::as_str)
                        .map(|value| !value.trim().is_empty())
                        .unwrap_or(false),
                    "host-only action {id} must declare hostOnlyReason"
                );
            }
            "transport" => {
                assert_eq!(kind, "transport", "transport scope must use transport kind");
            }
            other => panic!("unsupported action scope {other} for {id}"),
        }
    }
}
