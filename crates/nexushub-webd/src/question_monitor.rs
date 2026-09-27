//! Ordinary feedback requests share the existing Codex sender and persistent records.
use super::{probe_runtime, record_probe_event_with_bark, task_notification_suppression_reason};
use anyhow::Result;
use nexushub_core::{
    codex::{self, AssistantFeedback},
    config::Config,
    db::PanelDb,
    probe::{ProbeBuiltEvent, ProbeEventInput},
};
use serde_json::json;
use std::{collections::HashMap, path::Path, time::UNIX_EPOCH};

fn is_fresh(completed_ms: i64) -> bool {
    let age = chrono::Utc::now().timestamp_millis() - completed_ms;
    (0..=600_000).contains(&age)
}

fn enabled(config: &Config) -> bool {
    config.probe.enabled
        && config.probe.notifications.enabled
        && config.probe.notifications.notify_codex
        && config.probe.notifications.notify_reply_needed
}

fn event_for(
    config: &Config,
    thread_id: &str,
    title: &str,
    path: &Path,
    feedback: &AssistantFeedback,
) -> ProbeBuiltEvent {
    let mut event = probe_runtime(config).build_event(
        ProbeEventInput::hook_stop_with_context(
            Some(thread_id),
            Some(&feedback.turn_id),
            Some(thread_id),
            path.to_str(),
            Some(&feedback.body),
            "reply-needed",
        )
        .with_thread_title(Some(title))
        .with_body_source(Some("assistant_question")),
    );
    event.payload["feedback_completed_at_ms"] = json!(feedback.completed_at_ms);
    event
}

pub fn delivery_key(event: &ProbeBuiltEvent) -> Option<String> {
    if event.payload["body_source"] != "assistant_question" {
        return None;
    }
    use sha2::{Digest, Sha256};
    Some(format!(
        "probe_feedback_delivery:{}",
        hex::encode(Sha256::digest(
            format!(
                "{}:{}",
                event.thread_id.as_deref()?,
                event.turn_id.as_deref()?
            )
            .as_bytes()
        ))
    ))
}

/// All notification entrances re-read the authoritative native thread before sending.
pub fn prepare_event(
    config: &Config,
    db: &PanelDb,
    event: ProbeBuiltEvent,
) -> Result<ProbeBuiltEvent> {
    if !matches!(
        event.event_type.as_str(),
        "completion" | "hook_stop" | "reply_needed"
    ) || matches!(
        event.payload["body_source"].as_str(),
        Some("proposed_plan" | "request_user_input")
    ) {
        return Ok(event);
    }
    let candidate = codex::assistant_feedback_request(&event.bark_body).is_some();
    if !candidate && event.payload["body_source"] != "assistant_question" {
        return Ok(event);
    }
    let mut suppressed = event.clone();
    suppressed.suppression_reason = Some("unconfirmed_feedback_turn".into());
    let Some(id) = event.thread_id.as_deref() else {
        return Ok(suppressed);
    };
    let paths = codex::resolve_codex_paths(&config.codex.home).codex_paths();
    let Ok(threads) = codex::notification_thread_headers(&paths) else {
        return Ok(suppressed);
    };
    let Some(thread) = threads.into_iter().find(|thread| thread.id == id) else {
        return Ok(suppressed);
    };
    let Some(path) = thread.rollout_path else {
        return Ok(suppressed);
    };
    let Ok(Some(feedback)) = codex::rollout_pending_feedback(&path) else {
        return Ok(suppressed);
    };
    if event
        .turn_id
        .as_deref()
        .is_some_and(|turn| turn != feedback.turn_id)
    {
        return Ok(suppressed);
    }
    let Some(since) = db.feedback_notification_baseline(enabled(config))? else {
        suppressed.suppression_reason = Some("feedback_notifications_disabled".into());
        return Ok(suppressed);
    };
    if feedback.completed_at_ms <= since || !is_fresh(feedback.completed_at_ms) {
        suppressed.suppression_reason = Some("feedback_before_baseline_or_stale".into());
        return Ok(suppressed);
    }
    Ok(event_for(config, id, &thread.title, &path, &feedback))
}

/// Store fingerprints, not reply bodies, in the existing settings store.
pub async fn run(config: &Config, db: &PanelDb) -> Result<()> {
    let Some(since) = db.feedback_notification_baseline(enabled(config))? else {
        return Ok(());
    };
    let paths = codex::resolve_codex_paths(&config.codex.home).codex_paths();
    let previous: HashMap<String, (u64, u128)> = db
        .get_setting("probe_feedback_files")?
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    let mut next = HashMap::new();
    let mut threads = codex::notification_thread_headers(&paths)?;
    threads.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    for thread in threads
        .into_iter()
        .take(config.probe.recent_limit.clamp(1, 200))
    {
        let Some(path) = thread.rollout_path.as_ref() else {
            continue;
        };
        let Ok(meta) = std::fs::metadata(path) else {
            continue;
        };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        let modified = modified.duration_since(UNIX_EPOCH).unwrap_or_default();
        let fingerprint = (meta.len(), modified.as_nanos());
        next.insert(thread.id.clone(), fingerprint);
        if previous.get(&thread.id) == Some(&fingerprint) || modified.as_millis() < since as u128 {
            continue;
        }
        if task_notification_suppression_reason(config, Some(&thread.id)).is_some() {
            continue;
        }
        let Ok(Some(feedback)) = codex::rollout_pending_feedback(path) else {
            // A half-written terminal line must be examined again on the next change.
            continue;
        };
        if feedback.completed_at_ms <= since || !is_fresh(feedback.completed_at_ms) {
            continue;
        }
        let event = event_for(config, &thread.id, &thread.title, path, &feedback);
        record_probe_event_with_bark(config, db, event).await?;
    }
    db.set_setting("probe_feedback_files", &serde_json::to_string(&next)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        fs,
        io::{BufRead, BufReader, Read, Write},
        net::TcpListener,
        path::PathBuf,
        sync::mpsc,
        thread,
        time::Duration,
    };

    struct Fixture {
        root: PathBuf,
        config: Config,
        db: PanelDb,
    }

    impl Fixture {
        fn new(server_url: String) -> Self {
            let root = std::env::temp_dir().join(format!(
                "nexushub-feedback-monitor-{}",
                uuid::Uuid::new_v4()
            ));
            let home = root.join(".codex");
            fs::create_dir_all(home.join("sessions")).unwrap();
            let rollout = home.join("sessions/rollout.jsonl");
            let timestamp = chrono::Utc::now().to_rfc3339();
            let events = [
                json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-a"}}),
                json!({"type":"response_item","payload":{"type":"message","role":"assistant","phase":"final","content":[{"type":"output_text","text":"请确认测试结果。"}]}}),
                json!({"type":"event_msg","timestamp":timestamp,"payload":{"type":"task_complete","turn_id":"turn-a","status":"completed","last_agent_message":"请确认测试结果。"}}),
            ];
            fs::write(
                &rollout,
                events
                    .iter()
                    .map(|value| format!("{value}\n"))
                    .collect::<String>(),
            )
            .unwrap();
            let conn = rusqlite::Connection::open(home.join("state_5.sqlite")).unwrap();
            conn.execute_batch("CREATE TABLE threads (id TEXT PRIMARY KEY, title TEXT, updated_at INTEGER, archived_at INTEGER, archived INTEGER, rollout_path TEXT, cwd TEXT, model TEXT, source TEXT, thread_source TEXT, has_user_event INTEGER, first_user_message TEXT); ").unwrap();
            conn.execute("INSERT INTO threads VALUES ('main','Feedback fixture',?1,NULL,0,?2,'/workspace','fixture','vscode','user',1,'fixture')", rusqlite::params![PanelDb::now(), rollout.to_string_lossy().to_string()]).unwrap();
            let mut config = Config::default();
            config.codex.home = home;
            config.probe.enabled = true;
            config.probe.notifications.enabled = true;
            config.probe.notifications.notify_codex = true;
            config.probe.notifications.notify_reply_needed = true;
            config.probe.notifications.server_url = server_url;
            let db = PanelDb::open(root.join("panel.sqlite")).unwrap();
            db.set_secret_setting_bytes("probe_bark_device_key", b"fixture-device-key")
                .unwrap();
            db.set_setting(
                "probe_feedback_baseline",
                &json!({"enabled":true,"since_ms":chrono::Utc::now().timestamp_millis()-60_000})
                    .to_string(),
            )
            .unwrap();
            Self { root, config, db }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn mock_bark_server(expected: usize) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            for _ in 0..expected {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut headers = String::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    headers.push_str(&line);
                    if line == "\r\n" {
                        break;
                    }
                }
                let length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                let mut body = vec![0_u8; length];
                reader.read_exact(&mut body).unwrap();
                sender
                    .send(String::from_utf8_lossy(&body).into_owned())
                    .unwrap();
                stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 12\r\nConnection: close\r\n\r\n{\"code\":200}").unwrap();
            }
        });
        (url, receiver)
    }

    #[tokio::test]
    async fn monitor_sends_one_feedback_event_and_restart_does_not_repeat_it() {
        let (url, requests) = mock_bark_server(1);
        let fixture = Fixture::new(url);
        run(&fixture.config, &fixture.db).await.unwrap();
        let payload: serde_json::Value =
            serde_json::from_str(&requests.recv_timeout(Duration::from_secs(2)).unwrap()).unwrap();
        assert!(payload["body"].as_str().unwrap().contains("请确认测试结果"));
        assert_eq!(payload["title"], "等待回复：Feedback fixture");
        run(&fixture.config, &fixture.db).await.unwrap();
        assert!(requests.recv_timeout(Duration::from_millis(150)).is_err());
    }

    #[tokio::test]
    async fn monitor_does_not_send_after_a_user_answer_is_appended() {
        let (url, requests) = mock_bark_server(1);
        let fixture = Fixture::new(url);
        let rollout = fixture.config.codex.home.join("sessions/rollout.jsonl");
        fs::OpenOptions::new().append(true).open(&rollout).unwrap().write_all(format!("{}\n", json!({"type":"event_msg","payload":{"type":"user_message","turn_id":"turn-a","message":"已确认"}})).as_bytes()).unwrap();
        run(&fixture.config, &fixture.db).await.unwrap();
        assert!(requests.recv_timeout(Duration::from_millis(150)).is_err());
    }
}
