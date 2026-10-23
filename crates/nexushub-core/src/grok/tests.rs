use super::*;

#[cfg(unix)]
fn native_fixture(script: &str) -> (GrokPaths, GrokSessionSummary, PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let (paths, id, _) = fixture();
    let mut session = resolve_session(&paths, &id).unwrap();
    session.cwd = paths.home.to_string_lossy().into_owned();
    let executable = paths.home.join("fake-grok");
    fs::write(&executable, script).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    (paths, session, executable)
}

#[cfg(unix)]
#[tokio::test]
async fn grok_native_rename_uses_only_fixed_protocol_and_correlates_responses() {
    let (paths, session, executable) = native_fixture(
        r#"#!/bin/sh
[ "$#" -eq 2 ] && [ "$1" = agent ] && [ "$2" = stdio ] || exit 2
IFS= read -r request
printf '%s\n' "$request" >> "$GROK_HOME/requests.jsonl"
printf '%s\n' '{"jsonrpc":"2.0","method":"session/update","params":{}}' '{"jsonrpc":"2.0","id":99,"result":{}}' '{"jsonrpc":"2.0","id":1,"result":{}}'
IFS= read -r request
printf '%s\n' "$request" >> "$GROK_HOME/requests.jsonl"
printf '%s\n' '{"jsonrpc":"2.0","id":2,"result":{}}'
"#,
    );
    rename_native(
        &executable,
        &paths,
        &session,
        "A quoted \"title\"",
        Duration::from_secs(2),
    )
    .await
    .unwrap();
    let requests: Vec<Value> = fs::read_to_string(paths.home.join("requests.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["method"], "initialize");
    assert_eq!(requests[1]["method"], "_x.ai/session/rename");
    assert_eq!(
        requests[1]["params"],
        serde_json::json!({"sessionId":session.id,"cwd":session.cwd,"title":"A quoted \"title\""})
    );
    assert_eq!(
        resolve_session(&paths, &session.id).unwrap().title,
        "Original"
    );
    fs::remove_dir_all(paths.home).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn grok_native_rename_failure_does_not_modify_session_files() {
    for response in ["not-json", r#"{"id":1,"error":{"code":-1}}"#, r#"{"id":1}"#] {
        let script = format!(
            "#!/bin/sh\nread -r request\nprintf '%s\\n' '{}'\n",
            response
        );
        let (paths, session, executable) = native_fixture(&script);
        let before = directory_snapshot(&session.path).unwrap();
        assert!(rename_native(
            &executable,
            &paths,
            &session,
            "New title",
            Duration::from_secs(2)
        )
        .await
        .is_err());
        assert_eq!(directory_snapshot(&session.path).unwrap(), before);
        fs::remove_dir_all(paths.home).unwrap();
    }
}

#[cfg(unix)]
#[tokio::test]
async fn grok_native_rename_timeout_reaps_the_child() {
    let (paths, session, executable) = native_fixture("#!/bin/sh\nprintf '%s' $$ > \"$GROK_HOME/pid\"\nwhile read -r request; do read -r next; done\n");
    let error = rename_native(
        &executable,
        &paths,
        &session,
        "New title",
        Duration::from_secs(2),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("result unknown"), "{error:#}");
    let pid = fs::read_to_string(paths.home.join("pid")).unwrap();
    let status = std::process::Command::new("kill")
        .args(["-0", pid.trim()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(!status.success());
    fs::remove_dir_all(paths.home).unwrap();
}

pub(super) fn fixture() -> (GrokPaths, String, PathBuf) {
    let home = std::env::temp_dir().join(format!("nexushub-grok-{}", uuid::Uuid::new_v4()));
    let id = uuid::Uuid::new_v4().to_string();
    let path = home.join("sessions/%2Fwork").join(&id);
    fs::create_dir_all(&path).unwrap();
    fs::write(
        path.join("summary.json"),
        serde_json::to_vec(&serde_json::json!({
            "info": {"id": id, "cwd": "/work"}, "generated_title": "Original", "num_messages": 3
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(path.join("updates.jsonl"), "").unwrap();
    fs::write(home.join("active_sessions.json"), "{}").unwrap();
    (GrokPaths { home }, id, path)
}

#[test]
fn lists_sort_by_conversation_time_for_updates_and_native_history() {
    use std::io::Write;
    let (paths, first_id, first) = fixture();
    let second_id = uuid::Uuid::new_v4().to_string();
    let second = first.parent().unwrap().join(&second_id);
    fs::create_dir_all(&second).unwrap();
    // A session without summary metadata can still be ordered from its native messages.
    fs::write(second.join("chat_history.jsonl"), format!("{}\n", serde_json::json!({"type":"assistant","timestamp":"2026-10-01T04:00:00Z","content":"Fixture answer"}))).unwrap();
    let updates = first.join("updates.jsonl");
    let message = |kind: &str, time: &str| serde_json::json!({"timestamp":time,"params":{"update":{"sessionUpdate":kind,"content":{"text":"Fixture request"}}}});
    fs::write(
        &updates,
        format!(
            "{}\n{}\n",
            message("user_message_chunk", "2026-10-01T11:00:00+08:00"),
            message("tool_call", "2026-12-01T00:00:00Z")
        ),
    )
    .unwrap();
    let mut metadata: Value =
        serde_json::from_slice(&fs::read(first.join("summary.json")).unwrap()).unwrap();
    metadata["updated_at"] = serde_json::json!("2026-12-02T00:00:00Z");
    fs::write(first.join("summary.json"), metadata.to_string()).unwrap();
    assert_eq!(
        list_grok_sessions(&paths, 1, None).unwrap()[0].id,
        second_id
    );
    let reads = HISTORY_READS.with(|count| count.get());
    list_grok_sessions(&paths, 1, None).unwrap();
    assert_eq!(HISTORY_READS.with(|count| count.get()), reads);
    let mut file = fs::OpenOptions::new().append(true).open(&updates).unwrap();
    writeln!(
        file,
        "{}",
        message("agent_message_chunk", "2026-10-01T05:00:00Z")
    )
    .unwrap();
    let latest = list_grok_sessions(&paths, 1, None).unwrap();
    assert_eq!(latest[0].id, first_id);
    assert_eq!(
        latest[0].updated_at.as_deref(),
        Some("2026-10-01T05:00:00Z")
    );
    assert_eq!(
        grok_session_detail(&paths, &first_id, None)
            .unwrap()
            .summary
            .updated_at,
        latest[0].updated_at
    );
    fs::remove_dir_all(paths.home).unwrap();
}

#[test]
fn grok_history_cache_reuses_unchanged_files_and_invalidates_updates() {
    let (paths, id, path) = fixture();
    HISTORY_READS.with(|reads| reads.set(0));
    assert!(grok_session_detail(&paths, &id, None)
        .unwrap()
        .events
        .is_empty());
    assert!(grok_session_detail(&paths, &id, None)
        .unwrap()
        .events
        .is_empty());
    HISTORY_READS.with(|reads| assert_eq!(reads.get(), 1));
    fs::write(path.join("updates.jsonl"), "{\"params\":{\"update\":{\"sessionUpdate\":\"agent_message_chunk\",\"content\":{\"text\":\"Final answer\"}}}}\n").unwrap();
    assert_eq!(
        grok_session_detail(&paths, &id, None).unwrap().events[0]
            .text
            .as_deref(),
        Some("Final answer")
    );
    HISTORY_READS.with(|reads| assert_eq!(reads.get(), 2));
    fs::remove_dir_all(&paths.home).unwrap();
    assert!(grok_session_detail(&paths, &id, None).is_err());
}

#[test]
fn grok_cached_tool_and_user_chunk_snapshots_remain_stable_after_append_updates() {
    use base64::Engine as _;

    let (paths, id, path) = fixture();
    let history = path.join("updates.jsonl");
    let attachment_path = paths.home.join("attachment.png");
    let attachment = base64::engine::general_purpose::STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aN1cAAAAASUVORK5CYII=")
        .unwrap();
    fs::write(&attachment_path, attachment).unwrap();
    let initial = [
        serde_json::json!({"params":{"update":{"sessionUpdate":"tool_call","title":"Open file","toolCallId":"call-1"}}}),
        serde_json::json!({"params":{"update":{"sessionUpdate":"user_message_chunk","promptId":"prompt-1","content":[{"type":"text","text":"Inspect "},{"type":"local_image","path":attachment_path.to_string_lossy().into_owned()}]}}}),
    ];
    fs::write(
        &history,
        initial
            .iter()
            .map(|record| record.to_string())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();

    let previous = grok_session_detail(&paths, &id, None).unwrap();
    assert_eq!(previous.events.len(), 2);
    assert_eq!(previous.events[0].text.as_deref(), Some("Open file"));
    assert_eq!(previous.events[1].text.as_deref(), Some("Inspect "));
    let previous_user = previous.events[1].user_message.as_ref().unwrap();
    assert_eq!(previous_user.attachments.len(), 1);
    let previous_attachment = &previous_user.attachments[0];
    assert_eq!(previous_attachment.reason, None);
    assert_eq!(
        previous_user
            .read_attachment(&previous_attachment.id)
            .unwrap()
            .mime_type,
        "image/png"
    );

    let appended = [
        serde_json::json!({"params":{"update":{"sessionUpdate":"user_message_chunk","promptId":"prompt-1","content":{"type":"text","text":"the file"}}}}),
        serde_json::json!({"params":{"update":{"sessionUpdate":"tool_call_update","title":"Open file.txt","toolCallId":"call-1","status":"completed"}}}),
    ];
    fs::remove_file(&attachment_path).unwrap();
    let appended_text = appended
        .iter()
        .map(|record| record.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut file = fs::OpenOptions::new().append(true).open(&history).unwrap();
    std::io::Write::write_all(&mut file, appended_text.as_bytes()).unwrap();
    drop(file);

    let latest = grok_session_detail(&paths, &id, None).unwrap();
    assert_eq!(previous.events[0].text.as_deref(), Some("Open file"));
    assert_eq!(previous.events[0].status, None);
    assert_eq!(previous.events[1].text.as_deref(), Some("Inspect "));
    assert_eq!(
        previous.events[1].user_message.as_ref().unwrap().text,
        "Inspect "
    );
    assert_eq!(latest.events[0].text.as_deref(), Some("Open file.txt"));
    assert_eq!(latest.events[0].status.as_deref(), Some("completed"));
    assert_eq!(latest.events[1].text.as_deref(), Some("Inspect the file"));
    assert_eq!(
        latest.events[1].user_message.as_ref().unwrap().text,
        "Inspect the file"
    );
    let latest_user = latest.events[1].user_message.as_ref().unwrap();
    assert_eq!(latest_user.attachments.len(), 1);
    let latest_attachment = &latest_user.attachments[0];
    assert!(latest_attachment.reason.is_some());
    assert_ne!(latest_attachment.id, previous_attachment.id);
    assert!(latest_user.read_attachment(&latest_attachment.id).is_err());
    assert_eq!(previous_attachment.reason, None);

    let response = serde_json::to_value(&latest).unwrap();
    assert!(response["events"].is_array());
    assert!(response["events"][0].is_object());
    assert_eq!(response["events"][0]["kind"], "tool_call");
    assert_eq!(response["events"][0]["callId"], "call-1");
    assert_eq!(response["events"][0]["text"], "Open file.txt");
    assert_eq!(response["events"][0]["status"], "completed");
    assert!(response["events"][0].get("call_id").is_none());
    assert!(response["events"][1].is_object());
    assert_eq!(response["events"][1]["kind"], "user_message_chunk");
    assert_eq!(
        response["events"][1]["userMessage"]["text"],
        "Inspect the file"
    );
    assert!(response["events"][1]["userMessage"]["attachments"][0]["reason"].is_string());

    fs::remove_dir_all(&paths.home).unwrap();
}

#[test]
fn grok_delete_ignores_changed_content_and_does_not_touch_workspace() {
    let (paths, id, path) = fixture();
    let preview = preview_grok_delete(&paths, &id).unwrap();
    fs::write(path.join("updates.jsonl"), "{\"changed\":true}\n").unwrap();
    let result = execute_grok_delete(
        &paths,
        GrokDeleteRequest {
            id,
            confirmed: true,
            fingerprint: preview.fingerprint,
        },
    )
    .unwrap();
    assert!(result.deleted);
    assert!(!path.exists());
    assert!(paths.sessions().exists());
    fs::remove_dir_all(paths.home).unwrap();
}

#[test]
fn grok_delete_preview_accepts_unterminated_activity_tail_without_modifying_native_files() {
    let (paths, id, path) = fixture();
    let events = path.join("events.jsonl");
    let complete_turn = [
        serde_json::json!({
            "type": "turn_started",
            "session_id": id.clone(),
            "session_relationship": "primary",
            "schema_version": "1.0"
        }),
        serde_json::json!({"type": "turn_ended", "outcome": "completed"}),
    ]
    .iter()
    .map(Value::to_string)
    .collect::<Vec<_>>()
    .join("\n");
    let original = format!("{complete_turn}\n{{\"type\":\"turn_started\"");
    fs::write(&events, &original).unwrap();

    // Activity display and deletion are independent.
    let listed = list_grok_sessions(&paths, 10, None).unwrap();
    assert_eq!(listed[0].status, "recent");
    let eligibility = listed[0].deletion_eligibility.as_ref().unwrap();
    assert!(eligibility.allowed);
    assert!(eligibility.reason.is_none());
    let before = directory_snapshot(&path).unwrap();
    assert!(preview_grok_delete(&paths, &id).is_ok());
    assert_eq!(fs::read(&events).unwrap(), original.as_bytes());
    assert_eq!(directory_snapshot(&path).unwrap(), before);

    fs::remove_dir_all(paths.home).unwrap();
}

#[test]
fn grok_delete_preview_allows_unowned_running_activity_and_preserves_native_files() {
    let (paths, id, path) = fixture();
    let events = path.join("events.jsonl");
    let running = serde_json::json!({
        "type": "turn_started",
        "session_id": id.clone(),
        "session_relationship": "primary",
        "schema_version": "1.0"
    })
    .to_string()
        + "\n";
    fs::write(&events, &running).unwrap();

    let listed = list_grok_sessions(&paths, 10, None).unwrap();
    assert_eq!(listed[0].status, "unknown");
    assert_eq!(
        listed[0].deletion_eligibility,
        Some(GrokDeletionEligibility {
            allowed: true,
            reason: None,
        })
    );
    let before = directory_snapshot(&path).unwrap();
    let preview = preview_grok_delete(&paths, &id).unwrap();
    let sessions = crate::services::sessions::SessionUseCases {
        codex: crate::codex::CodexPaths {
            home: paths.home.join("unused-codex"),
        },
        claude: crate::claude::ClaudePaths {
            config: paths.home.join("unused-claude"),
            projects: paths.home.join("unused-claude-projects"),
        },
        grok: paths.clone(),
        platform: crate::platform::PlatformPaths::for_kind(crate::platform::PlatformKind::Macos),
    };
    let batch = sessions
        .bulk_preview(crate::services::sessions::SessionBatchRequest {
            include_descendants: false,
            provider: crate::services::sessions::SessionProvider::Grok,
            operation: crate::services::sessions::SessionOperation::Delete,
            session_keys: vec![id.clone()],
        })
        .unwrap();
    assert!(batch.items[0].allowed);
    assert_eq!(
        batch.items[0].fingerprint.as_deref(),
        Some(preview.fingerprint.as_str())
    );
    assert_eq!(preview.id, id);
    assert_eq!(fs::read(&events).unwrap(), running.as_bytes());
    assert_eq!(directory_snapshot(&path).unwrap(), before);

    fs::remove_dir_all(paths.home).unwrap();
}

#[test]
fn grok_delete_preview_allows_a_valid_abnormal_terminal_turn() {
    let (paths, id, path) = fixture();
    let events = path.join("events.jsonl");
    let records = [
        serde_json::json!({
            "type": "turn_started",
            "session_id": id.clone(),
            "session_relationship": "primary",
            "schema_version": "1.0"
        }),
        serde_json::json!({"type": "turn_ended", "outcome": "failed"}),
    ];
    let native = records
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(&events, &native).unwrap();

    let listed = list_grok_sessions(&paths, 10, None).unwrap();
    assert_eq!(listed[0].status, "recent");
    assert_eq!(
        listed[0].deletion_eligibility,
        Some(GrokDeletionEligibility {
            allowed: true,
            reason: None,
        })
    );
    let before = directory_snapshot(&path).unwrap();
    assert!(preview_grok_delete(&paths, &id).is_ok());
    assert_eq!(fs::read(&events).unwrap(), native.as_bytes());
    assert_eq!(directory_snapshot(&path).unwrap(), before);

    fs::remove_dir_all(paths.home).unwrap();
}

#[test]
fn grok_delete_preview_accepts_corrupt_activity_records() {
    let (paths, id, path) = fixture();
    let events = path.join("events.jsonl");
    let corrupt = b"not-json\n";
    fs::write(&events, corrupt).unwrap();

    let listed = list_grok_sessions(&paths, 10, None).unwrap();
    assert_eq!(listed[0].status, "unknown");
    let eligibility = listed[0].deletion_eligibility.as_ref().unwrap();
    assert!(eligibility.allowed);
    assert!(eligibility.reason.is_none());
    assert!(preview_grok_delete(&paths, &id).is_ok());
    assert_eq!(fs::read(&events).unwrap(), corrupt);

    fs::remove_dir_all(paths.home).unwrap();
}

#[test]
fn grok_delete_requires_confirmation_and_cleans_active_registrations() {
    let (paths, id, path) = fixture();
    let preview = preview_grok_delete(&paths, &id).unwrap();
    assert!(execute_grok_delete(
        &paths,
        GrokDeleteRequest {
            id: id.clone(),
            confirmed: false,
            fingerprint: preview.fingerprint
        }
    )
    .is_err());
    fs::write(
        paths.home.join("active_sessions.json"),
        serde_json::to_vec(&serde_json::json!({id.clone(): {}})).unwrap(),
    )
    .unwrap();
    assert!(preview_grok_delete(&paths, &id).is_ok());
    let result = execute_grok_delete(
        &paths,
        GrokDeleteRequest {
            id: id.clone(),
            confirmed: true,
            fingerprint: String::new(),
        },
    )
    .unwrap();
    assert!(result.deleted);
    assert!(!path.exists());
    assert_eq!(
        fs::read_to_string(paths.home.join("active_sessions.json"))
            .unwrap()
            .trim(),
        "{}"
    );
    fs::remove_dir_all(paths.home).unwrap();
}

#[test]
fn grok_storage_write_failures_explain_systemd_remediation() {
    let missing = std::env::temp_dir()
        .join(format!("nexushub-grok-missing-{}", uuid::Uuid::new_v4()))
        .join("sessions");
    let error = ensure_grok_storage_writable(&missing, "preview deletion").unwrap_err();
    let message = format!("{error:#}");
    assert!(message.contains("Grok session storage is not writable"));
    assert!(message.contains("ReadWritePaths"));
    assert!(message.contains("systemctl daemon-reload"));
    assert!(message.contains(&missing.display().to_string()));
}

#[test]
fn grok_registration_without_process_identity_does_not_prove_running() {
    let (paths, id, _path) = fixture();
    fs::write(
        paths.home.join("active_sessions.json"),
        serde_json::json!([{"session_id": id}]).to_string(),
    )
    .unwrap();
    let sessions = list_grok_sessions(&paths, 10, None).unwrap();
    assert_eq!(sessions[0].status, "unknown");
    fs::remove_dir_all(paths.home).unwrap();
}

#[cfg(unix)]
#[test]
fn grok_delete_unlinks_symlink_in_session() {
    let (paths, id, path) = fixture();
    std::os::unix::fs::symlink("/tmp", path.join("escape")).unwrap();
    assert!(
        execute_grok_delete(
            &paths,
            GrokDeleteRequest {
                id,
                confirmed: true,
                fingerprint: String::new()
            }
        )
        .unwrap()
        .deleted
    );
    assert!(!path.exists());
    assert!(Path::new("/tmp").exists());
    fs::remove_dir_all(paths.home).unwrap();
}

#[test]
fn grok_history_merges_chunks_and_hides_thoughts() {
    let (paths, id, path) = fixture();
    let updates = [
        serde_json::json!({"timestamp":1,"params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"text":"Hello "}}}}),
        serde_json::json!({"timestamp":2,"params":{"update":{"sessionUpdate":"agent_message_chunk","content":{"text":"world"}}}}),
        serde_json::json!({"timestamp":3,"params":{"update":{"sessionUpdate":"agent_thought_chunk","content":{"text":"private"}}}}),
        serde_json::json!({"timestamp":4,"params":{"update":{"sessionUpdate":"tool_call","title":"Read file","toolCallId":"c1"}}}),
    ];
    fs::write(
        path.join("updates.jsonl"),
        updates
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    let detail = grok_session_detail(&paths, &id, None).unwrap();
    assert_eq!(detail.events.len(), 2);
    assert_eq!(detail.events[0].text.as_deref(), Some("Hello world"));
    assert_eq!(detail.events[1].text.as_deref(), Some("Read file"));
    fs::remove_dir_all(paths.home).unwrap();
}

#[test]
fn grok_history_merges_tool_updates_by_call_identity() {
    let (paths, id, path) = fixture();
    let updates = [
        serde_json::json!({"params":{"update":{"sessionUpdate":"tool_call","title":"read_file","toolCallId":"c1"}}}),
        serde_json::json!({"params":{"update":{"sessionUpdate":"tool_call_update","title":"Read README.md","toolCallId":"c1","status":"in_progress"}}}),
        serde_json::json!({"params":{"update":{"sessionUpdate":"tool_call","title":"List files","toolCallId":"c2"}}}),
        serde_json::json!({"params":{"update":{"sessionUpdate":"tool_call_update","toolCallId":"c1","status":"completed","content":[{"type":"content","content":{"type":"text","text":"File contents"}}]}}}),
        serde_json::json!({"params":{"update":{"sessionUpdate":"tool_call_update","toolCallId":"c2","status":"completed"}}}),
    ];
    fs::write(
        path.join("updates.jsonl"),
        updates
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    let detail = grok_session_detail(&paths, &id, None).unwrap();
    assert_eq!(detail.events.len(), 2);
    assert_eq!(detail.events[0].text.as_deref(), Some("Read README.md"));
    let event = serde_json::to_value(&detail.events[0]).unwrap();
    assert_eq!(event["status"], "completed");
    assert_eq!(event["detail"], "File contents");
    assert_eq!(detail.events[1].text.as_deref(), Some("List files"));
    fs::remove_dir_all(paths.home).unwrap();
}

#[test]
fn user_chunks_keep_images_and_boundaries_with_stable_ids() {
    use serde_json::json;
    let (paths, id, _) = fixture();
    let summary = resolve_session(&paths, &id).unwrap();
    let history = summary.path.join("updates.jsonl");
    let records = [
        json!({"params":{"update":{"sessionUpdate":"user_message_chunk","promptId":"p1","updateId":"u1","content":{"type":"text","text":"First\n"}}}}),
        json!({"params":{"update":{"sessionUpdate":"user_message_chunk","promptId":"p1","updateId":"u1","content":{"type":"text","text":"First\n"}}}}),
        json!({"params":{"update":{"sessionUpdate":"user_message_chunk","promptId":"p1","content":{"type":"image","url":"https://example.com/image.png"}}}}),
        json!({"params":{"update":{"sessionUpdate":"user_message_chunk","promptId":"p2","content":{"type":"text","text":"Second"}}}}),
    ];
    fs::write(
        &history,
        records
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\r\n")
            + "\r\n",
    )
    .unwrap();
    let events = read_history(&history).unwrap();
    assert_eq!(events.len(), 2);
    let first = events[0].user_message.as_ref().unwrap();
    assert_eq!(first.text, "First\n");
    assert_eq!(first.attachments.len(), 1);
    assert_ne!(first.id, events[1].user_message.as_ref().unwrap().id);
    assert_eq!(
        first.id,
        read_history(&history).unwrap()[0]
            .user_message
            .as_ref()
            .unwrap()
            .id
    );
    fs::remove_dir_all(paths.home).unwrap();
}

pub(super) fn directory_snapshot(path: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let mut result = walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            (
                e.path().strip_prefix(path).unwrap().into(),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    result.sort();
    Ok(result)
}

#[test]
fn native_system_only_history_is_empty_and_updates_remain_preferred() {
    let (paths, id, path) = fixture();
    fs::remove_file(path.join("updates.jsonl")).unwrap();
    fs::write(path.join("chat_history.jsonl"), "{\"type\":\"system\",\"content\":\"fixture\"}\n{\"type\":\"user\",\"synthetic_reason\":\"system_reminder\",\"content\":\"fixture\"}\n").unwrap();
    assert!(grok_session_detail(&paths, &id, None)
        .unwrap()
        .events
        .is_empty());
    fs::write(
        path.join("chat_history.jsonl"),
        "{\"type\":\"user\",\"content\":\"native fixture\"}\n",
    )
    .unwrap();
    fs::write(path.join("updates.jsonl"),"{\"sessionUpdate\":\"user_message_chunk\",\"content\":{\"text\":\"updates fixture\",\"type\":\"text\"}}\n").unwrap();
    let detail = grok_session_detail(&paths, &id, None).unwrap();
    assert_eq!(detail.events.len(), 1);
    assert_eq!(detail.events[0].text.as_deref(), Some("updates fixture"));
    fs::remove_dir_all(paths.home).unwrap();
}

#[test]
fn deletion_locates_all_workspaces_with_missing_or_corrupt_summaries() {
    let (paths, id, path) = fixture();
    fs::write(path.join("summary.json"), "{broken").unwrap();
    let other = paths.sessions().join("another-workspace").join(&id);
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("unknown"), "fixture").unwrap();
    fs::write(
        paths.home.join("active_sessions.json"),
        serde_json::json!([{ "session_id":id,"pid":123},{"session_id":"retained","pid":456}])
            .to_string(),
    )
    .unwrap();
    let preview = preview_grok_delete(&paths, &id).unwrap();
    assert_eq!(preview.file_count, 3);
    assert!(
        execute_grok_delete(
            &paths,
            GrokDeleteRequest {
                id: id.clone(),
                confirmed: true,
                fingerprint: "old fingerprint".into()
            }
        )
        .unwrap()
        .deleted
    );
    assert!(!path.exists());
    assert!(!other.exists());
    assert_eq!(
        serde_json::from_str::<Value>(
            &fs::read_to_string(paths.home.join("active_sessions.json")).unwrap()
        )
        .unwrap(),
        serde_json::json!([{ "session_id":"retained","pid":456 }])
    );
    assert!(
        execute_grok_delete(
            &paths,
            GrokDeleteRequest {
                id,
                confirmed: true,
                fingerprint: String::new()
            }
        )
        .unwrap()
        .deleted
    );
    fs::remove_dir_all(paths.home).unwrap();
}

#[test]
fn native_history_fallback_preserves_messages_tools_and_paging_without_system_context() {
    let (paths, id, path) = fixture();
    fs::remove_file(path.join("updates.jsonl")).unwrap();
    let rows = [
        serde_json::json!({"type":"system","content":"hidden fixture"}),
        serde_json::json!({"type":"user","synthetic_reason":"system_reminder","content":"hidden reminder"}),
        serde_json::json!({"type":"user","content":"question"}),
        serde_json::json!({"type":"assistant","content":"checking","tool_calls":[{"id":"call","name":"Read","arguments":"{}"}]}),
        serde_json::json!({"type":"tool_result","tool_call_id":"call","content":"tool output"}),
        serde_json::json!({"type":"reasoning","encrypted_content":"hidden"}),
        serde_json::json!({"type":"assistant","content":"answer"}),
    ];
    fs::write(
        path.join("chat_history.jsonl"),
        rows.iter().map(|r| format!("{r}\n")).collect::<String>(),
    )
    .unwrap();
    let latest = grok_session_page(&paths, &id, None, Some(2), None, None).unwrap();
    assert!(latest.has_more);
    assert_eq!(latest.total_events, 4);
    let all = grok_session_detail(&paths, &id, None).unwrap();
    assert_eq!(all.events.len(), 4);
    assert_eq!(all.events[0].text.as_deref(), Some("question"));
    assert_eq!(all.events[2].detail.as_deref(), Some("tool output"));
    assert_eq!(all.events[2].status.as_deref(), Some("completed"));
    let before = latest.before_cursor.clone().unwrap();
    let older = grok_session_page(&paths, &id, None, Some(2), Some(&before), None).unwrap();
    assert_eq!(older.events.len(), 2);
    use std::io::Write;
    writeln!(
        fs::OpenOptions::new()
            .append(true)
            .open(path.join("chat_history.jsonl"))
            .unwrap(),
        "{}",
        serde_json::json!({"type":"user","content":"next question"})
    )
    .unwrap();
    assert_eq!(
        grok_session_detail(&paths, &id, None).unwrap().events.len(),
        5
    );
    fs::remove_dir_all(paths.home).unwrap();
}
