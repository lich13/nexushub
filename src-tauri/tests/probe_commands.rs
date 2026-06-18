use nexushub_core::{
    config::Config,
    crypto::SecretBox,
    db::PanelDb,
    platform::{PlatformKind, PlatformPaths},
};
use nexushub_desktop_lib::{
    desktop_probe_status_with_state, desktop_update_status_with_state, DesktopState,
};
use serde_json::json;
use std::{path::Path, process::Command};

fn desktop_state(temp: &tempfile::TempDir) -> DesktopState {
    let mut config = Config::for_platform_kind_with_home(PlatformKind::Macos, temp.path());
    config.paths.data_dir = temp.path().join("data");
    config.paths.db_path = temp.path().join("panel.sqlite");
    config.paths.log_dir = temp.path().join("logs");
    config.codex.home = temp.path().join("codex-home");
    config.codex.workspace = temp.path().join("workspace");
    config.probe.recent_limit = 10;
    std::fs::create_dir_all(&config.paths.data_dir).unwrap();
    std::fs::create_dir_all(&config.paths.log_dir).unwrap();
    std::fs::create_dir_all(&config.codex.home).unwrap();
    std::fs::create_dir_all(config.codex.home.join("sessions")).unwrap();
    std::fs::create_dir_all(&config.codex.workspace).unwrap();
    let db = PanelDb::open_with_secret_box(&config.paths.db_path, SecretBox::deterministic_dev())
        .unwrap();
    DesktopState::new(
        config,
        db,
        PlatformPaths::for_kind_with_home(PlatformKind::Macos, temp.path()),
    )
}

fn write_codex_thread(
    codex_home: &Path,
    id: &str,
    title: &str,
    rollout_events: &[serde_json::Value],
) {
    let now = chrono::Utc::now().timestamp();
    let rollout = codex_home
        .join("sessions")
        .join(format!("rollout-{id}.jsonl"));
    let text = rollout_events
        .iter()
        .map(serde_json::Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&rollout, text).unwrap();
    let db = codex_home.join("state_5.sqlite");
    let status = Command::new("sqlite3")
        .arg(&db)
        .arg(format!(
            "CREATE TABLE IF NOT EXISTS threads(
                id TEXT PRIMARY KEY,
                rollout_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                source TEXT NOT NULL,
                cwd TEXT NOT NULL,
                title TEXT NOT NULL,
                preview TEXT NOT NULL DEFAULT ''
            );
            INSERT OR REPLACE INTO threads(id, rollout_path, created_at, updated_at, source, cwd, title, preview)
            VALUES('{id}', '{}', 1, {now}, 'codex', '/tmp', '{title}', '');",
            rollout.display()
        ))
        .status()
        .unwrap();
    assert!(status.success());
}

#[tokio::test]
async fn typed_probe_status_includes_real_thread_buckets() {
    let temp = tempfile::tempdir().unwrap();
    let state = desktop_state(&temp);
    let codex_home = state.config().codex.home;
    write_codex_thread(
        &codex_home,
        "running-thread",
        "运行中的线程",
        &[json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-live"}})],
    );
    write_codex_thread(
        &codex_home,
        "reply-thread",
        "等待回复的线程",
        &[
            json!({"type":"response_item","turn_id":"turn-choice","payload":{"type":"function_call","name":"request_user_input","call_id":"choice-1","arguments":{"questions":[{"id":"choice","question":"选择方案","options":[{"label":"A"}]}]}}}),
        ],
    );

    let status = desktop_probe_status_with_state(&state).await.unwrap();

    assert_eq!(status.running_count, 1);
    assert_eq!(status.running_threads[0].id, "running-thread");
    assert_eq!(status.reply_needed_count, 1);
    assert_eq!(status.reply_needed_threads[0].id, "reply-thread");
}

#[tokio::test]
async fn desktop_update_status_uses_macos_tauri_updater_shape() {
    let temp = tempfile::tempdir().unwrap();
    let state = desktop_state(&temp);

    let status = desktop_update_status_with_state(&state, Some("v0.1.101"), None).unwrap();
    let serialized = serde_json::to_string(&status).unwrap();

    assert_eq!(status.method, nexushub_core::services::updates::UpdateExecutionMethod::MacosTauriUpdater);
    assert_eq!(status.current_version, env!("CARGO_PKG_VERSION"));
    assert!(status.capabilities.iter().any(|capability| capability == "signature_verification"));
    assert!(!serialized.contains("systemctl"));
    assert!(!serialized.contains("nginx"));
    assert!(!serialized.contains("/opt/nexushub"));
}
