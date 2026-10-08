use super::*;
use serde_json::json;
use std::io::Write;

struct Fixture {
    root: PathBuf,
    paths: ClaudePaths,
    file: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = env::temp_dir().join(format!("nexushub-claude-test-{}", uuid::Uuid::new_v4()));
        let config = root.join("config");
        let paths = ClaudePaths {
            projects: config.join("projects"),
            config,
        };
        let file = paths.projects.join("project/native.jsonl");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::create_dir_all(root.join("workspace")).unwrap();
        Self { root, paths, file }
    }
    fn row(&self, id: &str, parent: Option<&str>, kind: &str, content: Value) -> Value {
        json!({"type":kind,"uuid":id,"parentUuid":parent,"sessionId":"native-fixture","cwd":self.root.join("workspace"),"version":"2.1.284","timestamp":"2026-09-29T00:00:00Z","message":{"role":kind,"content":content,"stop_reason":if kind=="assistant" {"end_turn"} else {""}}})
    }
    fn write(&self, rows: &[Value]) {
        fs::write(
            &self.file,
            rows.iter().map(|v| format!("{v}\n")).collect::<String>(),
        )
        .unwrap();
    }
    fn append(&self, row: &Value) {
        writeln!(
            fs::OpenOptions::new()
                .append(true)
                .open(&self.file)
                .unwrap(),
            "{row}"
        )
        .unwrap();
    }
    fn complete(&self) {
        self.write(&[
            self.row("u1", None, "user", json!("Example request")),
            self.row(
                "a1",
                Some("u1"),
                "assistant",
                json!([{"type":"text","text":"Example complete"}]),
            ),
        ]);
    }
    fn key(&self) -> String {
        key_for(
            &self.paths.projects.canonicalize().unwrap(),
            &self
                .paths
                .projects
                .canonicalize()
                .unwrap()
                .join("project/native.jsonl"),
            "native-fixture",
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn discovers_multiple_projects_and_duplicate_native_ids_without_subagents() {
    let f = Fixture::new();
    f.complete();
    let duplicate = f.paths.projects.join("another/same.jsonl");
    fs::create_dir_all(duplicate.parent().unwrap()).unwrap();
    fs::copy(&f.file, &duplicate).unwrap();
    let child = f
        .paths
        .projects
        .join("project/native/subagents/child.jsonl");
    fs::create_dir_all(child.parent().unwrap()).unwrap();
    fs::copy(&f.file, child).unwrap();
    let all = list_claude_sessions(&f.paths, 100, None).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].id, all[1].id);
    assert_ne!(all[0].session_key, all[1].session_key);
    assert!(all.iter().all(|s| s.status == "recent"));
    assert!(list_claude_sessions(&f.paths, 100, Some("not-present"))
        .unwrap()
        .is_empty());
    assert!(resolve(&f.paths, "../../secret").is_err());
    let absent = ClaudePaths {
        projects: f.root.join("absent"),
        config: f.root.clone(),
    };
    assert!(!absent.available());
    assert!(list_claude_sessions(&absent, 100, None).unwrap().is_empty());
}

#[test]
fn preserves_tools_text_plan_and_native_metadata_with_bounded_paging() {
    let f = Fixture::new();
    let mut rows = vec![f.row("u1", None, "user", json!("Start"))];
    let mut parent = "u1".to_string();
    for i in 0..90 {
        let aid = format!("a{i}");
        let uid = format!("r{i}");
        let call = format!("call{i}");
        rows.push(f.row(&aid,Some(&parent),"assistant",json!([{"type":"tool_use","id":call,"name":"Read","input":{"file_path":"/workspace/AGENTS.md"}}])));
        rows.push(f.row(&uid,Some(&aid),"user",json!([{"type":"tool_result","tool_use_id":call,"content":"# AGENTS.md\nExample instructions","is_error":i==1}])));
        parent = uid;
    }
    rows.push(f.row("plan",Some(&parent),"assistant",json!([{"type":"tool_use","id":"plan-call","name":"ExitPlanMode","input":{"plan":"# Example plan\nDo work"}}])));
    rows.push(f.row(
        "plan-result",
        Some("plan"),
        "user",
        json!([{"type":"tool_result","tool_use_id":"plan-call","content":"approved"}]),
    ));
    rows.push(f.row(
        "final",
        Some("plan-result"),
        "assistant",
        json!([{"type":"text","text":"Done <oai-mem-citation>internal</oai-mem-citation>"}]),
    ));
    rows.push(json!({"type":"cost-state","sessionId":"native-fixture","totalCostUSD":0}));
    f.write(&rows);
    let parsed = reader::read(&f.file).unwrap();
    assert!(parsed.issues.is_empty(), "{:?}", parsed.issues);
    assert_eq!(
        parsed
            .events
            .iter()
            .filter(|e| e.kind == "tool_call")
            .count(),
        90
    );
    assert_eq!(
        parsed
            .events
            .iter()
            .filter(|e| e.status.as_deref() == Some("failed"))
            .count(),
        1
    );
    assert_eq!(
        parsed
            .events
            .iter()
            .filter(|e| e.kind == "user_message")
            .count(),
        1
    );
    assert!(parsed.events.iter().any(|e| e.kind == "plan"));
    let mut before = None;
    let mut event_ids = Vec::new();
    loop {
        let page = claude_session_detail(
            &f.paths,
            &ClaudeDetailRequest {
                session_key: f.key(),
                limit: Some(20),
                before,
            },
        )
        .unwrap();
        assert!(page.events.len() <= 20);
        event_ids.extend(page.events.iter().map(|e| e.id.clone()));
        if !page.has_more {
            break;
        }
        before = page.before_cursor;
    }
    assert_eq!(event_ids.len(), parsed.events.len());
    event_ids.sort();
    event_ids.dedup();
    assert_eq!(event_ids.len(), parsed.events.len());
}

#[test]
fn cache_recovers_partial_tail_truncation_replacement_and_keeps_unknown_records() {
    let f = Fixture::new();
    f.complete();
    let first = reader::read(&f.file).unwrap();
    assert_eq!(first.events.len(), 2);
    let tail = json!({"type":"custom-title","sessionId":"native-fixture","customTitle":"Renamed"})
        .to_string();
    let half = tail.len() / 2;
    fs::OpenOptions::new()
        .append(true)
        .open(&f.file)
        .unwrap()
        .write_all(&tail.as_bytes()[..half])
        .unwrap();
    let partial = reader::read(&f.file).unwrap();
    assert_eq!(partial.events.len(), 2);
    assert!(!partial.issues.is_empty());
    let mut file = fs::OpenOptions::new().append(true).open(&f.file).unwrap();
    writeln!(file, "{}", &tail[half..]).unwrap();
    let complete = reader::read(&f.file).unwrap();
    assert!(complete.issues.is_empty());
    assert_eq!(complete.title, "Renamed");
    f.append(&json!({"type":"future-record","sessionId":"native-fixture","example":"visible"}));
    let unknown = reader::read(&f.file).unwrap();
    assert_eq!(unknown.events.last().unwrap().kind, "unknown");
    assert!(unknown.issues.is_empty());
    assert!(!unknown.warnings.is_empty());
    f.complete();
    assert!(reader::read(&f.file).unwrap().issues.is_empty());
    fs::remove_file(&f.file).unwrap();
    f.write(&[f.row("new", None, "user", json!("Replacement"))]);
    assert_eq!(reader::read(&f.file).unwrap().title, "Replacement");
    fs::OpenOptions::new()
        .append(true)
        .open(&f.file)
        .unwrap()
        .write_all(b"broken\n")
        .unwrap();
    assert!(!reader::read(&f.file).unwrap().issues.is_empty());
}

#[test]
fn follows_rewound_branch_and_never_reports_tool_failure_as_terminal_failure() {
    let f = Fixture::new();
    f.write(&[
        f.row("u1", None, "user", json!("start")),
        f.row(
            "a1",
            Some("u1"),
            "assistant",
            json!([{"type":"text","text":"abandoned"}]),
        ),
        f.row("u2", Some("a1"), "user", json!("abandoned question")),
        f.row(
            "new",
            Some("u1"),
            "assistant",
            json!([{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"false"}}]),
        ),
        f.row(
            "tr",
            Some("new"),
            "user",
            json!([{"type":"tool_result","tool_use_id":"t1","is_error":true,"content":"exit 1"}]),
        ),
    ]);
    let parsed = reader::read(&f.file).unwrap();
    assert!(!parsed
        .events
        .iter()
        .any(|e| e.text.as_deref().is_some_and(|s| s.contains("abandoned"))));
    assert!(parsed.native_events.is_empty());
    assert!(parsed.turn_open);
}

#[test]
fn explicit_question_waits_for_result_and_does_not_infer_questions_from_prose() {
    let f = Fixture::new();
    f.write(&[f.row("u1",None,"user",json!("start")),f.row("q",Some("u1"),"assistant",json!([{"type":"tool_use","id":"question","name":"AskUserQuestion","input":{"questions":[{"question":"Choose a color","options":[{"label":"Blue"},{"label":"Green"}]}]}}]))]);
    let waiting = reader::read(&f.file).unwrap();
    assert_eq!(waiting.native_events.len(), 1);
    assert_eq!(waiting.native_events[0].kind, "reply_needed");
    assert!(waiting.native_events[0].body.contains("Blue"));
    f.append(&f.row(
        "answered",
        Some("q"),
        "user",
        json!([{"type":"tool_result","tool_use_id":"question","content":"Blue"}]),
    ));
    assert!(reader::read(&f.file).unwrap().native_events.is_empty());
    f.append(&f.row(
        "a1",
        Some("answered"),
        "assistant",
        json!([{"type":"text","text":"Which color?"}]),
    ));
    assert!(reader::read(&f.file)
        .unwrap()
        .native_events
        .iter()
        .all(|e| e.kind == "completion"));
}

#[test]
fn unfinished_turn_without_a_live_owner_remains_guarded() {
    let f = Fixture::new();
    f.write(&[f.row("u1", None, "user", json!("Unfinished request"))]);
    let summary = claude_session_summary(&f.paths, &f.key()).unwrap();
    assert_eq!(summary.status, "unknown");
    assert!(!summary.can_rename && !summary.can_delete);
    assert!(rename_claude_session(&f.paths, &f.key(), "Blocked").is_err());
    assert!(preview_claude_delete(&f.paths, &f.key()).is_err());
}

#[test]
fn rename_and_delete_only_selected_file_recheck_fingerprint_and_identity() {
    let f = Fixture::new();
    f.complete();
    let sibling = f.file.with_file_name("other.jsonl");
    fs::copy(&f.file, &sibling).unwrap();
    let original = fs::read(&f.file).unwrap();
    let renamed = rename_claude_session(&f.paths, &f.key(), "A new title").unwrap();
    assert_eq!(renamed.title, "A new title");
    assert!(fs::read(&f.file).unwrap().starts_with(&original));
    assert_eq!(fs::read(&sibling).unwrap(), original);
    let preview = preview_claude_delete(&f.paths, &f.key()).unwrap();
    f.append(
        &json!({"type":"custom-title","sessionId":"native-fixture","customTitle":"Changed again"}),
    );
    assert!(execute_claude_delete(
        &f.paths,
        ClaudeDeleteRequest {
            session_key: f.key(),
            confirmed: true,
            fingerprint: preview.fingerprint
        }
    )
    .is_err());
    assert!(f.file.exists());
    let preview = preview_claude_delete(&f.paths, &f.key()).unwrap();
    let result = execute_claude_delete(
        &f.paths,
        ClaudeDeleteRequest {
            session_key: f.key(),
            confirmed: true,
            fingerprint: preview.fingerprint,
        },
    )
    .unwrap();
    assert!(result.deleted);
    assert!(!f.file.exists());
    assert!(sibling.exists());
    assert!(f.root.join("workspace").exists());
}

#[test]
fn rejects_unknown_version_symlinks_and_damaged_identity() {
    let f = Fixture::new();
    f.complete();
    f.append(&json!({"type":"future","sessionId":"native-fixture"}));
    assert!(preview_claude_delete(&f.paths, &f.key()).is_err());
    assert!(rename_claude_session(&f.paths, &f.key(), "blocked").is_err());
    f.complete();
    f.append(&json!({"type":"custom-title","sessionId":"other-id","customTitle":"wrong"}));
    assert!(rename_claude_session(&f.paths, &f.key(), "blocked").is_err());
    #[cfg(unix)]
    {
        fs::remove_file(&f.file).unwrap();
        let target = f.root.join("outside.jsonl");
        fs::write(&target, "protected").unwrap();
        std::os::unix::fs::symlink(&target, &f.file).unwrap();
        assert!(resolve(&f.paths, &f.key()).is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "protected");
    }
}

#[test]
fn embedded_images_load_lazily_and_invalid_ids_are_rejected() {
    let f = Fixture::new();
    let data="iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j8ioAAAAASUVORK5CYII=";
    f.write(&[f.row("u1",None,"user",json!([{"type":"text","text":"Example image"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":data}}]))]);
    let detail = claude_session_detail(
        &f.paths,
        &ClaudeDetailRequest {
            session_key: f.key(),
            before: None,
            limit: None,
        },
    )
    .unwrap();
    let user = detail.events[0].user_message.as_ref().unwrap();
    assert_eq!(user.attachments.len(), 1);
    assert!(!serde_json::to_string(&detail).unwrap().contains(data));
    let mut request = crate::user_message::SessionAttachmentRequest {
        root_thread_id: None,
        provider: crate::services::sessions::SessionProvider::Claude,
        session_key: f.key(),
        message_id: user.id.clone(),
        attachment_id: user.attachments[0].id.clone(),
    };
    assert_eq!(
        read_attachment(&f.paths, &request).unwrap().mime_type,
        "image/png"
    );
    request.attachment_id = "../../outside".into();
    assert!(read_attachment(&f.paths, &request).is_err());
}

#[test]
fn batch_delete_has_partial_results_and_never_removes_an_unselected_sibling() {
    use crate::services::sessions::*;
    let f = Fixture::new();
    f.complete();
    let sibling = f.file.with_file_name("keep.jsonl");
    fs::copy(&f.file, &sibling).unwrap();
    let cases = SessionUseCases {
        platform: crate::platform::PlatformPaths::desktop_current(),
        codex: crate::codex::CodexPaths::new(f.root.join("codex")),
        grok: crate::grok::GrokPaths::default_for_user(),
        claude: f.paths.clone(),
    };
    let unknown = format!("claude:{}", "f".repeat(64));
    let preview = cases
        .bulk_preview(SessionBatchRequest {
            provider: SessionProvider::Claude,
            operation: SessionOperation::Delete,
            session_keys: vec![f.key(), unknown.clone()],
        })
        .unwrap();
    assert!(preview.items[0].allowed);
    assert!(!preview.items[1].allowed);
    let result = cases
        .bulk_execute(SessionBatchExecuteRequest {
            provider: SessionProvider::Claude,
            operation: SessionOperation::Delete,
            confirmed: true,
            items: vec![
                SessionBatchSelection {
                    session_key: f.key(),
                    fingerprint: preview.items[0].fingerprint.clone().unwrap(),
                },
                SessionBatchSelection {
                    session_key: unknown,
                    fingerprint: "invalid".into(),
                },
            ],
        })
        .unwrap();
    assert_eq!(result.items[0].status, "succeeded");
    assert_eq!(result.items[1].status, "blocked");
    assert!(sibling.exists());
}

#[test]
fn claude_notifications_baseline_dedupe_and_terminal_evidence_survive_restart() {
    use crate::{
        config::Config,
        db::PanelDb,
        native_probe::{NativeProvider, NativeScan},
    };
    let f = Fixture::new();
    f.complete();
    let mut config = Config::default();
    config.probe.notifications.enabled = true;
    let dbpath = f.root.join("panel.sqlite");
    let db = PanelDb::open(&dbpath).unwrap();
    let scan = notification_snapshots(&f.paths).unwrap();
    assert_eq!(
        db.stage_native_notifications(NativeProvider::Claude, &scan, &config)
            .unwrap(),
        0
    );
    let mut user = f.row("next", Some("a1"), "user", json!("Next request"));
    user["timestamp"] = json!((chrono::Utc::now() + chrono::Duration::seconds(1)).to_rfc3339());
    f.append(&user);
    let mut final_row = f.row(
        "done",
        Some("next"),
        "assistant",
        json!([{"type":"text","text":"Finished"}]),
    );
    final_row["timestamp"] = user["timestamp"].clone();
    f.append(&final_row);
    let scan = notification_snapshots(&f.paths).unwrap();
    assert_eq!(
        db.stage_native_notifications(NativeProvider::Claude, &scan, &config)
            .unwrap(),
        1
    );
    let pending = db.pending_native_deliveries(10).unwrap();
    assert_eq!(pending.len(), 1);
    assert!(db.claim_native_delivery(&pending[0].event_key).unwrap());
    db.finish_native_delivery(&pending[0], json!({"sent":true}))
        .unwrap();
    drop(db);
    let db = PanelDb::open(&dbpath).unwrap();
    assert_eq!(
        db.stage_native_notifications(NativeProvider::Claude, &scan, &config)
            .unwrap(),
        0
    );
    assert!(db.pending_native_deliveries(10).unwrap().is_empty());
    let mut failure = json!({"type":"result","sessionId":"native-fixture","subtype":"error_max_turns","is_error":true,"errors":["Terminal fixture failure"],"timestamp":user["timestamp"]});
    user["uuid"] = json!("failure-turn");
    user["parentUuid"] = json!("done");
    f.append(&user);
    failure["timestamp"] = user["timestamp"].clone();
    f.append(&failure);
    assert!(notification_snapshots(&f.paths).unwrap().streams[0]
        .events
        .iter()
        .any(|e| e.kind == "failure"));
    config.probe.notifications.notify_claude = false;
    assert_eq!(
        db.stage_native_notifications(NativeProvider::Claude, &NativeScan::default(), &config)
            .unwrap(),
        0
    );
}

#[test]
fn native_saved_plans_and_bound_permission_requests_are_read_without_guessing() {
    let f = Fixture::new();
    let user = f.row("u1", None, "user", json!("Start"));
    let call = f.row(
        "a1",
        Some("u1"),
        "assistant",
        json!([{"type":"tool_use","id":"call","name":"Bash","input":{"command":"pwd"}}]),
    );
    let permission = json!({"type":"permission_request","request_id":"approval","tool_use_id":"call","tool_name":"Bash","description":"Allow this command","sessionId":"native-fixture","timestamp":"2026-09-29T00:00:01Z"});
    f.write(&[user, call, permission.clone()]);
    let waiting = reader::read(&f.file).unwrap();
    assert!(waiting.issues.is_empty());
    assert_eq!(waiting.native_events.len(), 1);
    assert_eq!(waiting.native_events[0].kind, "reply_needed");
    f.append(&json!({"type":"permission_response","request_id":"approval","subtype":"error","sessionId":"native-fixture"}));
    assert!(reader::read(&f.file).unwrap().native_events.is_empty());
    f.append(&json!({"type":"attachment","uuid":"plan","sessionId":"native-fixture","attachment":{"type":"plan_file_reference","planContent":"# Saved plan\nKeep content"}}));
    let p = reader::read(&f.file).unwrap();
    assert_eq!(p.events.last().unwrap().kind, "plan");
    assert_eq!(
        p.events.last().unwrap().text.as_deref(),
        Some("# Saved plan\nKeep content")
    );
    let mut unbound = permission;
    unbound["tool_use_id"] = json!("another-call");
    f.append(&unbound);
    assert!(reader::read(&f.file).unwrap().native_events.is_empty());
}

#[test]
fn oversized_unrelated_file_does_not_hide_readable_sessions_and_cache_is_reused() {
    let f = Fixture::new();
    f.complete();
    let first = reader::read(&f.file).unwrap();
    let second = reader::read(&f.file).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    let huge = f.file.with_file_name("oversized.jsonl");
    fs::File::create(huge)
        .unwrap()
        .set_len(512 * 1024 * 1024 + 1)
        .unwrap();
    let all = list_claude_sessions(&f.paths, 100, None).unwrap();
    assert_eq!(all.len(), 2);
    assert!(all.iter().any(|s| s.id == "native-fixture" && s.can_rename));
    let blocked = all.iter().find(|s| s.id == "oversized").unwrap();
    assert!(blocked.read_error.is_some());
    assert!(!blocked.can_delete);
    assert_eq!(resolve(&f.paths, &f.key()).unwrap().1.id, "native-fixture");
}

#[test]
fn reads_stream_init_aliases_tool_references_and_empty_result_falls_back() {
    let f = Fixture::new();
    f.write(&[
        json!({
            "type": "system",
            "subtype": "init",
            "uuid": "init",
            "session_id": "snake-session",
            "claude_code_version": "2.1.284",
            "cwd": f.root.join("workspace"),
            "timestamp": "2026-09-29T00:00:00Z"
        }),
        json!({
            "type": "user",
            "uuid": "u1",
            "session_id": "snake-session",
            "cwd": f.root.join("workspace"),
            "claude_code_version": "2.1.284",
            "timestamp": "2026-09-29T00:00:01Z",
            "message": {"role":"user","content":"Start","stop_reason":""}
        }),
        json!({
            "type": "assistant",
            "uuid": "a1",
            "parentUuid": "u1",
            "session_id": "snake-session",
            "timestamp": "2026-09-29T00:00:02Z",
            "message": {
                "role":"assistant",
                "content":[
                    {"type":"tool_reference","tool_name":"mcp__fixture__search"},
                    {"type":"text","text":"Fallback answer"}
                ],
                "stop_reason":"end_turn"
            }
        }),
        json!({
            "type": "result",
            "uuid": "result",
            "session_id": "snake-session",
            "subtype": "success",
            "is_error": false,
            "result": "",
            "timestamp": "2026-09-29T00:00:03Z"
        }),
    ]);
    let parsed = reader::read(&f.file).unwrap();
    assert!(parsed.issues.is_empty(), "{:?}", parsed.issues);
    assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
    assert_eq!(parsed.id, "snake-session");
    assert_eq!(parsed.version, "2.1.284");
    assert!(parsed.events.iter().any(|event| {
        event.kind == "tool_reference" && event.role.as_deref() == Some("mcp__fixture__search")
    }));
    assert_eq!(parsed.native_events.len(), 1);
    assert_eq!(parsed.native_events[0].body, "Fallback answer");
    assert!(!parsed.turn_open);
}

#[test]
fn success_result_does_not_finish_with_pending_tool_and_unknown_system_is_warning() {
    let f = Fixture::new();
    f.write(&[
        f.row("u1", None, "user", json!("Start")),
        f.row(
            "a1",
            Some("u1"),
            "assistant",
            json!([{"type":"tool_use","id":"call","name":"Search","input":{}}]),
        ),
        json!({
            "type":"result",
            "uuid":"result",
            "sessionId":"native-fixture",
            "subtype":"success",
            "is_error":false,
            "result":"Finished too early",
            "timestamp":"2026-09-29T00:00:01Z"
        }),
        json!({
            "type":"system",
            "subtype":"future_activity",
            "uuid":"future",
            "sessionId":"native-fixture",
            "timestamp":"2026-09-29T00:00:02Z"
        }),
    ]);
    let parsed = reader::read(&f.file).unwrap();
    assert!(parsed.issues.is_empty(), "{:?}", parsed.issues);
    assert!(parsed
        .warnings
        .iter()
        .any(|warning| warning.contains("新格式系统活动")));
    assert!(parsed.turn_open);
    assert!(parsed.native_events.is_empty());
    let summary = list_claude_sessions(&f.paths, 10, None)
        .unwrap()
        .into_iter()
        .find(|summary| summary.id == "native-fixture")
        .expect("fixture summary");
    assert_eq!(summary.status, "unknown");
    assert!(summary.read_error.is_none());
    assert!(summary.read_warning.is_some());
    assert!(!summary.can_rename && !summary.can_delete);
    let scan = notification_snapshots(&f.paths).unwrap();
    assert_eq!(scan.errors, 0);
    assert_eq!(scan.streams.len(), 1);
}

#[test]
fn compatible_auxiliary_records_are_readable_but_unknown_formats_stay_mutation_guarded() {
    let f = Fixture::new();
    f.complete();
    for kind in [
        "mode",
        "permission-mode",
        "file-history-delta",
        "future-fixture-record",
    ] {
        f.append(&json!({"type":kind,"sessionId":"native-fixture"}));
    }
    f.append(&json!({"type":"system","subtype":"informational","session_id":"native-fixture"}));
    let detail = claude_session_detail(
        &f.paths,
        &ClaudeDetailRequest {
            session_key: f.key(),
            limit: Some(200),
            before: None,
        },
    )
    .unwrap();
    assert!(detail.summary.read_error.is_none());
    assert!(detail.summary.read_warning.is_some());
    assert_eq!(detail.summary.status, "recent");
    assert!(!detail.summary.can_rename && !detail.summary.can_delete);
    assert!(rename_claude_session(&f.paths, &f.key(), "Blocked").is_err());
    assert!(preview_claude_delete(&f.paths, &f.key()).is_err());
    let scan = notification_snapshots(&f.paths).unwrap();
    assert_eq!(scan.errors, 0);
    assert_eq!(scan.streams[0].events.len(), 1);
    assert_eq!(scan.streams[0].events[0].kind, "completion");
}

#[test]
fn empty_success_result_uses_this_turn_text_and_never_tool_use_or_cancelled_turns() {
    let f = Fixture::new();
    let user = f.row("u1", None, "user", json!("Fixture request"));
    let mut answer = f.row(
        "a1",
        Some("u1"),
        "assistant",
        json!([{"type":"text","text":"Fixture final reply"}]),
    );
    answer["message"]["stop_reason"] = Value::Null;
    f.write(&[user.clone(), answer.clone()]);
    assert!(reader::read(&f.file).unwrap().native_events.is_empty());
    let result = json!({"type":"result","session_id":"native-fixture","subtype":"success","is_error":false,"result":"","timestamp":"2026-09-29T00:00:02Z"});
    f.append(&result);
    assert_eq!(
        reader::read(&f.file).unwrap().native_events[0].body,
        "Fixture final reply"
    );
    for reason in ["tool_use", "pause_turn", "refusal"] {
        answer["message"]["stop_reason"] = json!(reason);
        f.write(&[user.clone(), answer.clone(), result.clone()]);
        assert!(
            reader::read(&f.file).unwrap().native_events.is_empty(),
            "implicit {reason}"
        );
        let mut explicit = result.clone();
        explicit["stop_reason"] = json!(reason);
        f.write(&[user.clone(), answer.clone(), explicit.clone()]);
        assert!(
            reader::read(&f.file).unwrap().native_events.is_empty(),
            "{reason}"
        );
        answer["message"]["stop_reason"] = json!("end_turn");
        f.write(&[user.clone(), answer.clone(), explicit]);
        assert!(
            reader::read(&f.file).unwrap().native_events.is_empty(),
            "later result overrides completion: {reason}"
        );
    }
    answer["message"]["stop_reason"] = json!("end_turn");
    f.write(&[
        user.clone(),
        answer,
        json!({"type":"system","subtype":"cancelled"}),
        result,
    ]);
    assert!(reader::read(&f.file).unwrap().native_events.is_empty());
    f.complete();
    f.append(&f.row("u2", Some("a1"), "user", json!("New request")));
    assert!(reader::read(&f.file).unwrap().native_events.is_empty());
}

#[test]
fn terminal_evidence_respects_pending_tools_identity_and_record_integrity() {
    let f = Fixture::new();
    for reason in ["end_turn", "stop_sequence"] {
        let mut answer = f.row(
            "a1",
            Some("u1"),
            "assistant",
            json!([{"type":"text","text":"Finished"}]),
        );
        answer["message"]["stop_reason"] = json!(reason);
        f.write(&[f.row("u1", None, "user", json!("Start")), answer]);
        assert_eq!(
            notification_snapshots(&f.paths).unwrap().streams[0]
                .events
                .len(),
            1
        );
        let mut conflict = json!({"type":"system","subtype":"init","sessionId":"native-fixture","session_id":"other-fixture"});
        f.append(&conflict);
        assert!(notification_snapshots(&f.paths).unwrap().streams.is_empty());
        f.complete();
        conflict["session_id"] = json!("native-fixture");
        conflict["claude_code_version"] = json!("3.0.0");
        f.append(&conflict);
        assert!(notification_snapshots(&f.paths).unwrap().streams.is_empty());
    }
    f.write(&[
        f.row("u1", None, "user", json!("Start")),
        f.row(
            "a1",
            Some("u1"),
            "assistant",
            json!([{"type":"tool_use","id":"call","name":"Bash","input":{"command":"pwd"}}]),
        ),
        f.row(
            "a2",
            Some("a1"),
            "assistant",
            json!([{"type":"text","text":"Premature answer"}]),
        ),
    ]);
    assert!(notification_snapshots(&f.paths).unwrap().streams[0]
        .events
        .is_empty());
    fs::OpenOptions::new()
        .append(true)
        .open(&f.file)
        .unwrap()
        .write_all(b"{\"type\":")
        .unwrap();
    assert!(notification_snapshots(&f.paths).unwrap().streams.is_empty());
}
