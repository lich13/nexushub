use super::*;

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
