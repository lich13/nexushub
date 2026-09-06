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
        let before = directory_fingerprint(&session.path).unwrap();
        assert!(rename_native(
            &executable,
            &paths,
            &session,
            "New title",
            Duration::from_secs(2)
        )
        .await
        .is_err());
        assert_eq!(directory_fingerprint(&session.path).unwrap(), before);
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
    assert!(error.to_string().contains("result unknown"));
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

fn fixture() -> (GrokPaths, String, PathBuf) {
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
fn grok_delete_revalidates_content_and_does_not_touch_workspace() {
    let (paths, id, path) = fixture();
    let preview = preview_grok_delete(&paths, &id).unwrap();
    fs::write(path.join("updates.jsonl"), "{\"changed\":true}\n").unwrap();
    assert!(execute_grok_delete(
        &paths,
        GrokDeleteRequest {
            id: id.clone(),
            confirmed: true,
            fingerprint: preview.fingerprint
        }
    )
    .is_err());
    assert!(path.exists());
    let preview = preview_grok_delete(&paths, &id).unwrap();
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
fn grok_delete_rejects_active_and_unconfirmed_sessions() {
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
        serde_json::to_vec(&serde_json::json!({id.clone(): {"pid": 123}})).unwrap(),
    )
    .unwrap();
    assert!(preview_grok_delete(&paths, &id).is_err());
    assert!(path.exists());
    fs::remove_dir_all(paths.home).unwrap();
}

#[test]
fn grok_active_session_registry_accepts_native_array_shape() {
    let (paths, id, _path) = fixture();
    fs::write(
        paths.home.join("active_sessions.json"),
        serde_json::json!([{"session_id": id, "pid": 123}]).to_string(),
    )
    .unwrap();
    let sessions = list_grok_sessions(&paths, 10, None).unwrap();
    assert_eq!(sessions[0].status, "running");
    fs::remove_dir_all(paths.home).unwrap();
}

#[cfg(unix)]
#[test]
fn grok_delete_rejects_symlink_in_session() {
    let (paths, id, path) = fixture();
    std::os::unix::fs::symlink("/tmp", path.join("escape")).unwrap();
    assert!(preview_grok_delete(&paths, &id).is_err());
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
