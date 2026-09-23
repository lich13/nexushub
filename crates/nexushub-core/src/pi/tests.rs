use super::*;
use serde_json::json;
use std::{fs, path::Path};

fn fixture() -> (PathBuf, PiPaths, PathBuf) {
    let root = env::temp_dir().join(format!("nexushub-pi-test-{}", uuid::Uuid::new_v4()));
    let sessions = root.join("agent/sessions");
    let cwd = root.join("workspace");
    fs::create_dir_all(&sessions).unwrap();
    fs::create_dir_all(&cwd).unwrap();
    (root, PiPaths::from_sessions(sessions), cwd)
}

fn entry_header(id: &str, cwd: &Path, version: u32) -> Value {
    json!({
        "type": "session",
        "version": version,
        "id": id,
        "timestamp": "2026-09-22T00:00:00Z",
        "cwd": cwd,
    })
}

fn user(id: &str, parent: Option<&str>, text: &str) -> Value {
    json!({
        "type":"message",
        "id":id,
        "parentId":parent,
        "timestamp":"2026-09-22T00:00:01Z",
        "message":{"role":"user","content":text,"timestamp":1}
    })
}

fn assistant(id: &str, parent: Option<&str>, content: Value) -> Value {
    json!({
        "type":"message",
        "id":id,
        "parentId":parent,
        "timestamp":"2026-09-22T00:00:02Z",
        "message":{"role":"assistant","content":content,"timestamp":2,"stopReason":"stop"}
    })
}

fn write_jsonl(path: &Path, entries: &[Value]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut text = entries
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    text.push('\n');
    fs::write(path, text).unwrap();
}

fn inactive(_: &Path) -> Activity {
    Activity::Inactive
}

#[test]
fn duplicate_native_ids_remain_addressable_by_distinct_session_keys() {
    let (root, paths, cwd) = fixture();
    write_jsonl(
        &paths.sessions.join("one/session.jsonl"),
        &[entry_header("custom-id", &cwd, 3), user("a", None, "first")],
    );
    write_jsonl(
        &paths.sessions.join("two/session.jsonl"),
        &[
            entry_header("custom-id", &cwd, 3),
            user("b", None, "second"),
        ],
    );
    let sessions = list_pi_sessions(&paths, 100, Some("custom-id")).unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(sessions[0].id, sessions[1].id);
    assert_ne!(sessions[0].session_key, sessions[1].session_key);
    for session in &sessions {
        let detail = resolve_session_with_root(
            &paths,
            &canonical_root(&paths.sessions).unwrap(),
            &session.session_key,
            Activity::Inactive,
            true,
        )
        .unwrap();
        assert_eq!(detail.summary.id, "custom-id");
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn current_branch_excludes_abandoned_messages_and_keeps_tools_and_summaries() {
    let (root, paths, cwd) = fixture();
    let path = paths.sessions.join("project/session.jsonl");
    write_jsonl(
        &path,
        &[
            entry_header("branch-id", &cwd, 3),
            user("u1", None, "keep user"),
            assistant(
                "a1",
                Some("u1"),
                json!([
                    {"type":"thinking","thinking":"consider"},
                    {"type":"toolCall","id":"call-1","name":"read","arguments":{"path":"a.txt"}}
                ]),
            ),
            json!({"type":"message","id":"t1","parentId":"a1","timestamp":"2026-09-22T00:00:03Z","message":{"role":"toolResult","toolCallId":"call-1","toolName":"read","content":[{"type":"text","text":"tool output"}],"isError":false,"timestamp":3}}),
            user("old-u", Some("t1"), "abandoned user"),
            assistant(
                "old-a",
                Some("old-u"),
                json!([{"type":"text","text":"abandoned answer"}]),
            ),
            json!({"type":"branch_summary","id":"branch","parentId":"t1","fromId":"old-a","timestamp":"2026-09-22T00:00:04Z","summary":"old branch summary"}),
            user("new-u", Some("branch"), "current user"),
            json!({"type":"compaction","id":"compact","parentId":"new-u","timestamp":"2026-09-22T00:00:05Z","summary":"compact summary","firstKeptEntryId":"t1","tokensBefore":123}),
            assistant(
                "new-a",
                Some("compact"),
                json!([{"type":"text","text":"current answer"}]),
            ),
        ],
    );
    let parsed = parse_session_file(&path).unwrap();
    assert_eq!(parsed.message_count, 5);
    assert_eq!(parsed.last_message.as_deref(), Some("current answer"));
    let events = history_events(&parsed.entries, &parsed.active_entry_ids);
    let rendered = serde_json::to_string(&events).unwrap();
    for expected in [
        "keep user",
        "call-1",
        "tool output",
        "old branch summary",
        "current user",
        "compact summary",
        "current answer",
    ] {
        assert!(
            rendered.contains(expected),
            "missing {expected}: {rendered}"
        );
    }
    assert!(!rendered.contains("abandoned user"));
    assert!(!rendered.contains("abandoned answer"));
    assert!(events.iter().any(|event| event.kind == "tool_call"));
    assert!(events.iter().any(|event| event.kind == "tool_result"));
    assert!(events.iter().any(|event| event.kind == "compaction"));
    assert!(events.iter().any(|event| event.kind == "branch_summary"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn incomplete_tail_preserves_complete_history_and_disables_mutation() {
    let (root, paths, cwd) = fixture();
    let path = paths.sessions.join("tail.jsonl");
    write_jsonl(
        &path,
        &[
            entry_header("tail-id", &cwd, 3),
            user("u1", None, "complete"),
        ],
    );
    let mut bytes = fs::read(&path).unwrap();
    bytes.extend_from_slice(br#"{"type":"message","id":"partial"#);
    fs::write(&path, bytes).unwrap();
    let parsed = parse_session_file(&path).unwrap();
    assert_eq!(parsed.last_message.as_deref(), Some("complete"));
    assert!(parsed.incomplete_tail);
    let summary = decorate_summary(
        &path,
        &canonical_root(&paths.sessions).unwrap(),
        parsed,
        Activity::Inactive,
        true,
    );
    assert_eq!(summary.status, "incomplete");
    assert!(!summary.can_rename);
    assert!(!summary.can_delete);
    assert!(summary
        .read_error
        .as_deref()
        .unwrap()
        .contains("incomplete trailing line"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn legacy_sessions_are_converted_only_in_memory_and_unknown_versions_are_read_only() {
    let (root, paths, cwd) = fixture();
    let legacy = paths.sessions.join("legacy.jsonl");
    write_jsonl(
        &legacy,
        &[
            entry_header("legacy-id", &cwd, 1),
            json!({"type":"message","timestamp":"2026-09-22T00:00:01Z","message":{"role":"user","content":"legacy text"}}),
            json!({"type":"message","timestamp":"2026-09-22T00:00:02Z","message":{"role":"hookMessage","content":"legacy hook","display":true}}),
        ],
    );
    let before = fs::read(&legacy).unwrap();
    let parsed = parse_session_file(&legacy).unwrap();
    assert_eq!(parsed.active_entry_ids.len(), 2);
    assert_eq!(fs::read(&legacy).unwrap(), before);
    let summary = decorate_summary(
        &legacy,
        &canonical_root(&paths.sessions).unwrap(),
        parsed,
        Activity::Inactive,
        true,
    );
    assert!(!summary.can_rename);
    assert!(!summary.can_delete);

    let future = paths.sessions.join("future.jsonl");
    write_jsonl(
        &future,
        &[
            entry_header("future-id", &cwd, 99),
            user("f1", None, "future text"),
        ],
    );
    let parsed = parse_session_file(&future).unwrap();
    assert_eq!(parsed.last_message.as_deref(), Some("future text"));
    let summary = decorate_summary(
        &future,
        &canonical_root(&paths.sessions).unwrap(),
        parsed,
        Activity::Inactive,
        true,
    );
    assert!(!summary.can_delete);
    assert!(summary.read_error.unwrap().contains("Unsupported"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn corrupted_files_are_visible_with_an_explicit_error() {
    let (root, paths, _cwd) = fixture();
    fs::write(
        paths.sessions.join("broken.jsonl"),
        "not-json\nstill-broken\n",
    )
    .unwrap();
    let sessions = list_pi_sessions(&paths, 100, None).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].status, "error");
    assert!(sessions[0]
        .read_error
        .as_deref()
        .unwrap()
        .contains("malformed JSON"));
    assert!(!sessions[0].can_delete);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn cache_invalidates_when_a_session_changes() {
    let (root, paths, cwd) = fixture();
    let path = paths.sessions.join("cached.jsonl");
    write_jsonl(
        &path,
        &[entry_header("cache-id", &cwd, 3), user("u1", None, "one")],
    );
    assert_eq!(
        read_parsed_file(&path).unwrap().last_message.as_deref(),
        Some("one")
    );
    write_jsonl(
        &path,
        &[
            entry_header("cache-id", &cwd, 3),
            user("u1", None, "one"),
            assistant("a1", Some("u1"), json!([{"type":"text","text":"two"}])),
        ],
    );
    assert_eq!(
        read_parsed_file(&path).unwrap().last_message.as_deref(),
        Some("two")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_cli_disables_only_native_rename_for_an_inactive_valid_session() {
    let (root, paths, cwd) = fixture();
    let path = paths.sessions.join("no-cli.jsonl");
    write_jsonl(
        &path,
        &[
            entry_header("no-cli-id", &cwd, 3),
            user("u1", None, "readable"),
        ],
    );
    let summary = decorate_summary(
        &path,
        &canonical_root(&paths.sessions).unwrap(),
        parse_session_file(&path).unwrap(),
        Activity::Inactive,
        false,
    );
    assert!(!summary.can_rename);
    assert!(summary.can_delete);
    assert!(summary
        .rename_block_reason
        .as_deref()
        .unwrap()
        .contains("executable unavailable"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn activity_detection_is_root_scoped_and_conservative() {
    let (root, paths, _cwd) = fixture();
    let canonical = canonical_root(&paths.sessions).unwrap();
    assert_eq!(
        activity_from_processes(
            &canonical,
            [ProcessSnapshot {
                command: "pi".to_string(),
                args: format!("pi --session-dir {}", canonical.display()),
            }]
        ),
        Activity::Active
    );
    let other = root.join("other");
    fs::create_dir_all(&other).unwrap();
    assert_eq!(
        activity_from_processes(
            &canonical,
            [ProcessSnapshot {
                command: "pi".to_string(),
                args: format!("pi --session-dir {}", other.display()),
            }]
        ),
        Activity::Inactive
    );
    assert_eq!(
        activity_from_processes(
            &canonical,
            [ProcessSnapshot {
                command: "pi".to_string(),
                args: "pi".to_string(),
            }]
        ),
        Activity::Unknown
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn delete_rechecks_fingerprint_and_removes_only_the_selected_file() {
    let (root, paths, cwd) = fixture();
    let selected = paths.sessions.join("project/selected.jsonl");
    let sibling = paths.sessions.join("project/sibling.jsonl");
    write_jsonl(
        &selected,
        &[
            entry_header("selected", &cwd, 3),
            user("u1", None, "selected"),
        ],
    );
    write_jsonl(
        &sibling,
        &[
            entry_header("sibling", &cwd, 3),
            user("u2", None, "sibling"),
        ],
    );
    let preview =
        preview_pi_delete_with_activity(&paths, "project/selected.jsonl", inactive).unwrap();
    fs::write(
        &selected,
        format!("{} ", fs::read_to_string(&selected).unwrap()),
    )
    .unwrap();
    let error = execute_pi_delete_with_activity(
        &paths,
        PiDeleteRequest {
            session_key: preview.session_key.clone(),
            confirmed: true,
            fingerprint: preview.fingerprint,
        },
        inactive,
    )
    .unwrap_err();
    assert!(error.to_string().contains("changed"));
    assert!(selected.exists());
    assert!(sibling.exists());

    write_jsonl(
        &selected,
        &[
            entry_header("selected", &cwd, 3),
            user("u1", None, "selected"),
        ],
    );
    let preview =
        preview_pi_delete_with_activity(&paths, "project/selected.jsonl", inactive).unwrap();
    let result = execute_pi_delete_with_activity(
        &paths,
        PiDeleteRequest {
            session_key: preview.session_key,
            confirmed: true,
            fingerprint: preview.fingerprint,
        },
        inactive,
    )
    .unwrap();
    assert!(result.deleted);
    assert!(!selected.exists());
    assert!(sibling.exists());
    assert!(cwd.exists());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn symlink_and_path_traversal_are_rejected() {
    use std::os::unix::fs::symlink;

    let (root, paths, cwd) = fixture();
    let real = paths.sessions.join("real.jsonl");
    write_jsonl(
        &real,
        &[entry_header("real", &cwd, 3), user("u1", None, "real")],
    );
    symlink(&real, paths.sessions.join("link.jsonl")).unwrap();
    let canonical = canonical_root(&paths.sessions).unwrap();
    assert!(discover_session_path(&canonical, "link.jsonl").is_err());
    assert!(discover_session_path(&canonical, "../real.jsonl").is_err());
    assert!(discover_session_path(&canonical, real.to_str().unwrap()).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
fn make_rpc_fixture(root: &Path, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let executable = root.join("pi-fixture");
    fs::write(&executable, body).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    executable
}

#[cfg(unix)]
#[tokio::test]
async fn native_rename_uses_fixed_rpc_and_reads_back_disk_state() {
    let (root, paths, cwd) = fixture();
    let session = paths.sessions.join("project/session.jsonl");
    write_jsonl(
        &session,
        &[
            entry_header("rename-id", &cwd, 3),
            user("u1", None, "rename me"),
        ],
    );
    let executable = make_rpc_fixture(
        &root,
        r#"#!/usr/bin/env python3
import json, os, pathlib, sys
args = sys.argv[1:]
session = pathlib.Path(args[args.index("--session") + 1]).resolve()
(pathlib.Path(__file__).parent / "rpc-args.json").write_text(json.dumps(args))
entries = [json.loads(line) for line in session.read_text().splitlines() if line.strip()]
session_id = entries[0]["id"]
name = next((entry.get("name") for entry in reversed(entries) if entry.get("type") == "session_info"), None)
leaf = next((entry.get("id") for entry in reversed(entries[1:]) if entry.get("id")), None)
for line in sys.stdin:
    request = json.loads(line)
    request_id = request.get("id")
    kind = request["type"]
    if kind == "set_session_name":
        name = request["name"]
        record = {"type":"session_info","id":"native-name","parentId":leaf,"timestamp":"2026-09-22T00:01:00Z","name":name}
        with session.open("a") as handle:
            handle.write(json.dumps(record, separators=(",", ":")) + "\n")
        leaf = "native-name"
        response = {"id":request_id,"type":"response","command":kind,"success":True}
    else:
        data = {"sessionFile":str(session),"sessionId":session_id,"sessionName":name}
        response = {"id":request_id,"type":"response","command":kind,"success":True,"data":data}
    print(json.dumps(response, separators=(",", ":")), flush=True)
"#,
    );
    let renamed = rename_pi_session_with(
        &paths,
        "project/session.jsonl",
        " Native title ",
        &executable,
        Duration::from_secs(2),
        inactive,
    )
    .await
    .unwrap();
    assert_eq!(renamed.title, "Native title");
    assert_eq!(renamed.id, "rename-id");
    let entries = fs::read_to_string(&session).unwrap();
    assert_eq!(entries.matches("native-name").count(), 1);
    assert!(entries.contains("Native title"));
    let args: Vec<String> =
        serde_json::from_str(&fs::read_to_string(root.join("rpc-args.json")).unwrap()).unwrap();
    for required in [
        "--mode",
        "rpc",
        "--offline",
        "--no-context-files",
        "--no-tools",
        "--no-extensions",
        "--no-skills",
        "--no-prompt-templates",
        "--no-themes",
        "--no-approve",
        "--session",
        "--session-dir",
    ] {
        assert!(
            args.iter().any(|value| value == required),
            "missing {required}"
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn native_rename_rejects_identity_mismatch_before_mutation() {
    let (root, paths, cwd) = fixture();
    let session = paths.sessions.join("session.jsonl");
    write_jsonl(
        &session,
        &[
            entry_header("expected-id", &cwd, 3),
            user("u1", None, "unchanged"),
        ],
    );
    let before = fs::read(&session).unwrap();
    let executable = make_rpc_fixture(
        &root,
        r#"#!/usr/bin/env python3
import json, pathlib, sys
session = pathlib.Path(sys.argv[sys.argv.index("--session") + 1]).resolve()
for line in sys.stdin:
    request = json.loads(line)
    if request["type"] == "set_session_name":
        (pathlib.Path(__file__).parent / "unexpected-mutation").write_text("called")
    print(json.dumps({"id":request["id"],"type":"response","command":request["type"],"success":True,"data":{"sessionFile":str(session),"sessionId":"wrong-id"}}), flush=True)
"#,
    );
    let error = rename_pi_session_with(
        &paths,
        "session.jsonl",
        "Must not persist",
        &executable,
        Duration::from_secs(2),
        inactive,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("identity mismatch"));
    assert_eq!(fs::read(&session).unwrap(), before);
    assert!(!root.join("unexpected-mutation").exists());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn native_rename_timeout_reaps_the_child() {
    let (root, paths, cwd) = fixture();
    let session = paths.sessions.join("session.jsonl");
    write_jsonl(
        &session,
        &[
            entry_header("timeout-id", &cwd, 3),
            user("u1", None, "timeout"),
        ],
    );
    let executable = make_rpc_fixture(
        &root,
        r#"#!/bin/sh
printf '%s' $$ > "$(dirname "$0")/rpc-pid"
while IFS= read -r request; do sleep 30; done
"#,
    );
    let error = rename_pi_session_with(
        &paths,
        "session.jsonl",
        "New title",
        &executable,
        Duration::from_millis(200),
        inactive,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("resulting state is unknown"));
    let pid = fs::read_to_string(root.join("rpc-pid")).unwrap();
    assert!(!std::process::Command::new("kill")
        .args(["-0", pid.trim()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success());
    assert!(!fs::read_to_string(&session).unwrap().contains("New title"));
    fs::remove_dir_all(root).unwrap();
}
