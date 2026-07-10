use nexushub_core::{config::Config, db::PanelDb, platform::PlatformKind};
use serde_json::json;
use std::{
    fs,
    io::Write,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[test]
fn hook_request_user_input_accepts_official_pre_tool_use_stdin_with_empty_stdout() {
    let (root, config_path, config) = test_config("valid");
    write_config(&config_path, &config);
    let payload = pre_tool_use_payload(None);

    let output = run_hook(&config_path, payload.to_string().as_bytes());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let db = PanelDb::open(&config.paths.db_path).unwrap();
    let events = db.list_probe_events(10).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, "reply-needed");
    assert_eq!(events[0].thread_id.as_deref(), Some("thread-hook"));
    assert_eq!(events[0].payload["turn_id"], "turn-hook");
    assert_eq!(events[0].payload["call_id"], "call-hook");
    assert_eq!(events[0].payload["body_source"], "request_user_input");
    assert_eq!(events[0].payload["scan_source"], "pre-tool-use-hook");
    assert!(events[0].payload["transcript_path"].is_null());
    assert!(events[0].payload["body_summary"]
        .as_str()
        .is_some_and(|body| body.contains("Choose a mode?") && body.contains("Safe mode")));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn hook_request_user_input_memory_context_is_suppressed_with_empty_stdout() {
    let (root, config_path, mut config) = test_config("memory-suppressed");
    let memory_root = config.codex.home.join("memories");
    fs::create_dir_all(&memory_root).unwrap();
    config.probe.notifications.enabled = true;
    config.probe.notifications.notify_reply_needed = true;
    config.probe.notifications.server_url = "http://127.0.0.1:9".to_string();
    write_config(&config_path, &config);
    let db =
        PanelDb::open_with_secret_box(&config.paths.db_path, config.secret_box().unwrap()).unwrap();
    db.set_secret_setting_bytes("probe_bark_device_key", b"test-device-key")
        .unwrap();
    let mut payload = pre_tool_use_payload(None);
    payload["cwd"] = json!(memory_root);
    payload["tool_input"] = json!("must not be parsed for internal memory work");

    let output = run_hook(&config_path, payload.to_string().as_bytes());

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert!(db.list_probe_events(10).unwrap().is_empty());
    let conn = rusqlite::Connection::open(db.path()).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM probe_dedupe", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn hook_request_user_input_invalid_stdin_fails_open_without_stdout_or_recording() {
    let (root, config_path, config) = test_config("invalid");
    write_config(&config_path, &config);

    let invalid_payloads = vec![
        b"not-json".to_vec(),
        json!({
            "hook_event_name": "PreToolUse",
            "session_id": "thread-hook",
            "turn_id": "turn-hook",
            "tool_name": "exec_command",
            "tool_use_id": "call-hook",
            "tool_input": {"questions": []}
        })
        .to_string()
        .into_bytes(),
    ];
    for payload in invalid_payloads {
        let output = run_hook(&config_path, &payload);
        assert!(output.status.success());
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("probe_hook_request_user_input_failed"));
        assert!(!stderr.contains("not-json"));
    }

    let db = PanelDb::open(&config.paths.db_path).unwrap();
    assert!(db.list_probe_events(10).unwrap().is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn hook_request_user_input_bark_timeout_is_bounded_to_three_seconds() {
    let (root, config_path, mut config) = test_config("timeout");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        if let Ok((_stream, _)) = listener.accept() {
            thread::sleep(Duration::from_secs(8));
        }
    });
    config.probe.notifications.enabled = true;
    config.probe.notifications.notify_reply_needed = true;
    config.probe.notifications.server_url = format!("http://{address}");
    write_config(&config_path, &config);
    let db =
        PanelDb::open_with_secret_box(&config.paths.db_path, config.secret_box().unwrap()).unwrap();
    db.set_secret_setting_bytes("probe_bark_device_key", b"test-device-key")
        .unwrap();

    let started = Instant::now();
    let output = run_hook(
        &config_path,
        pre_tool_use_payload(None).to_string().as_bytes(),
    );
    let elapsed = started.elapsed();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(
        elapsed >= Duration::from_millis(2_500),
        "elapsed: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_millis(4_500),
        "elapsed: {elapsed:?}"
    );
    let events = db.list_probe_events(10).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload["bark"]["reason"], "timeout");
    assert_eq!(events[0].payload["bark"]["request_count"], 1);

    fs::remove_dir_all(root).unwrap();
}

fn pre_tool_use_payload(transcript_path: Option<&Path>) -> serde_json::Value {
    json!({
        "cwd": "/tmp",
        "hook_event_name": "PreToolUse",
        "model": "gpt-test",
        "permission_mode": "default",
        "session_id": "thread-hook",
        "tool_input": {
            "questions": [{
                "id": "mode",
                "header": "Mode",
                "question": "Choose a mode?",
                "options": [
                    {"label": "Safe mode", "description": "Use guarded execution"},
                    {"label": "Fast mode", "description": "Use direct execution"}
                ]
            }]
        },
        "tool_name": "request_user_input",
        "tool_use_id": "call-hook",
        "transcript_path": transcript_path,
        "turn_id": "turn-hook"
    })
}

fn run_hook(config_path: &Path, stdin: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nexushub-webd"))
        .args([
            "--config",
            config_path.to_str().unwrap(),
            "probe",
            "hook-request-user-input",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}

fn test_config(name: &str) -> (PathBuf, PathBuf, Config) {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("nexushub-hook-request-{name}-{unique}"));
    fs::create_dir_all(root.join(".codex/sessions")).unwrap();
    let mut config = Config::for_platform_kind(PlatformKind::Macos);
    config.codex.home = root.join(".codex");
    config.paths.data_dir = root.join("data");
    config.paths.db_path = root.join("data/nexushub.sqlite");
    config.paths.webui_dir = root.join("webui");
    config.paths.log_dir = root.join("logs");
    config.security.secret_key = "7q9DCmCPyxnTrH3FhrV1sUJol1yqPgscQsBnR-mXA2E".to_string();
    config.probe.notifications.enabled = false;
    let config_path = root.join("config.toml");
    (root, config_path, config)
}

fn write_config(path: &Path, config: &Config) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, toml::to_string_pretty(config).unwrap()).unwrap();
}
