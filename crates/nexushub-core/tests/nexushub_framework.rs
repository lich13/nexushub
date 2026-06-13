use nexushub_core::{
    claude_code::{discover_claude_projects, ClaudePaths},
    config::Config,
    platform::{PlatformKind, PlatformPaths},
    providers::{AgentProviderId, ProviderRegistry},
};
use serde_json::json;
use std::{fs, path::PathBuf, time::SystemTime};

#[test]
fn default_config_uses_nexushub_runtime_names() {
    let config = Config::default();

    assert_eq!(config.paths.data_dir.to_string_lossy(), "/opt/nexushub");
    assert_eq!(
        config.paths.db_path.to_string_lossy(),
        "/opt/nexushub/nexushub.sqlite"
    );
    assert_eq!(
        config.paths.webui_dir.to_string_lossy(),
        "/opt/nexushub/webui"
    );
    assert_eq!(config.paths.log_dir.to_string_lossy(), "/opt/nexushub/logs");
    assert_eq!(
        config.update.panel_update_command,
        "/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest"
    );
}

#[test]
fn platform_paths_cover_linux_macos_and_windows() {
    assert_eq!(
        PlatformPaths::for_kind(PlatformKind::Linux).data_dir,
        PathBuf::from("/opt/nexushub")
    );
    assert_eq!(
        PlatformPaths::for_kind(PlatformKind::Macos).data_dir,
        PathBuf::from("~/Library/Application Support/NexusHub")
    );
    assert_eq!(
        PlatformPaths::for_kind(PlatformKind::Windows).data_dir,
        PathBuf::from(r"%ProgramData%\NexusHub")
    );
}

#[test]
fn provider_registry_exposes_codex_and_claude_preview() {
    let registry = ProviderRegistry::default();
    let providers = registry.list();

    assert!(providers
        .iter()
        .any(|provider| { provider.id == AgentProviderId::Codex && provider.status == "ready" }));
    assert!(providers.iter().any(|provider| {
        provider.id == AgentProviderId::ClaudeCode && provider.status == "preview"
    }));
    assert!(providers
        .iter()
        .any(|provider| provider.id == AgentProviderId::Cursor));
    assert!(providers
        .iter()
        .any(|provider| provider.id == AgentProviderId::Gemini));
}

#[test]
fn claude_code_discovery_reads_project_sessions_from_claude_home() {
    let root = temp_dir("nexushub-claude-discovery");
    let projects_dir = root.join("projects").join("-Users-gosu-demo");
    fs::create_dir_all(&projects_dir).unwrap();
    fs::write(
        projects_dir.join("session-a.jsonl"),
        format!(
            "{}\n{}\n",
            json!({
                "type": "summary",
                "summary": "Implement provider abstraction",
                "timestamp": "2026-06-13T04:00:00Z"
            }),
            json!({
                "type": "user",
                "message": {"content": "hello"},
                "timestamp": "2026-06-13T04:01:00Z"
            })
        ),
    )
    .unwrap();
    fs::write(
        root.join("settings.json"),
        r#"{"permissions":{"allow":["Read"],"deny":["Write"]}}"#,
    )
    .unwrap();

    let projects = discover_claude_projects(&ClaudePaths::new(&root)).unwrap();

    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].id, "-Users-gosu-demo");
    assert_eq!(projects[0].display_name, "/Users/gosu/demo");
    assert_eq!(projects[0].sessions.len(), 1);
    assert_eq!(projects[0].sessions[0].id, "session-a");
    assert_eq!(
        projects[0].sessions[0].title.as_deref(),
        Some("Implement provider abstraction")
    );

    fs::remove_dir_all(root).unwrap();
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{label}-{unique}"))
}
