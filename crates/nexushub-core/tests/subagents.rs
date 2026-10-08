use nexushub_core::codex::{
    hidden_thread_ids, list_threads,
    subagents::{
        enrich_subagent_blocks, read_subagent_detail, SubagentActivity, SubagentDetailRequest,
        SubagentDetailResponse, SubagentStatus,
    },
    thread_detail, window_thread_detail, CodexPaths, ThreadDetail, ThreadStatus,
};
use nexushub_core::{
    platform::{PlatformKind, PlatformPaths},
    services::{sessions::SessionProvider, use_cases::NexusHubUseCases},
    user_message::{SessionAttachmentRequest, SessionAttachmentResponse},
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

const ROOT: &str = "root-thread";
const CHILD: &str = "child-thread";
const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aN1cAAAAASUVORK5CYII=";

struct NativeFixture {
    directory: PathBuf,
    paths: CodexPaths,
}

impl NativeFixture {
    fn new(root_events: &[Value]) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "nexushub-subagents-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&directory).unwrap();
        let directory = fs::canonicalize(directory).unwrap();
        let paths = CodexPaths::new(directory.join("codex"));
        fs::create_dir_all(paths.sessions_dir()).unwrap();
        fs::create_dir(paths.home.join("archived_sessions")).unwrap();
        fs::write(paths.session_index(), b"").unwrap();
        let fixture = Self { directory, paths };
        fixture
            .connection()
            .execute_batch(
                "CREATE TABLE threads (
                    id TEXT PRIMARY KEY,
                    rollout_path TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    source TEXT NOT NULL,
                    thread_source TEXT NOT NULL,
                    parent_thread_id TEXT,
                    agent_path TEXT,
                    agent_nickname TEXT,
                    agent_role TEXT,
                    cwd TEXT NOT NULL,
                    title TEXT NOT NULL,
                    archived INTEGER NOT NULL DEFAULT 0,
                    has_user_event INTEGER NOT NULL DEFAULT 1
                );
                CREATE TABLE thread_spawn_edges (
                    parent_thread_id TEXT NOT NULL,
                    child_thread_id TEXT NOT NULL
                );",
            )
            .unwrap();
        fixture.add_thread(ROOT, None, None, "Root task", root_events);
        fixture
    }

    fn connection(&self) -> Connection {
        Connection::open(self.paths.state_db()).unwrap()
    }

    fn add_thread(
        &self,
        id: &str,
        parent: Option<&str>,
        agent_path: Option<&str>,
        title: &str,
        events: &[Value],
    ) -> PathBuf {
        let rollout = self.paths.sessions_dir().join(format!("{id}.jsonl"));
        let source = match parent {
            Some(parent) => json!({"subagent": {"thread_spawn": {
                "parent_thread_id": parent,
                "agent_path": agent_path,
                "agent_nickname": title,
                "agent_role": "explorer"
            }}}),
            None => json!("cli"),
        };
        let mut records = vec![json!({
            "type": "session_meta",
            "payload": {
                "id": id,
                "source": source,
                "cwd": "/workspace/example"
            }
        })];
        records.extend_from_slice(events);
        write_jsonl(&rollout, &records);
        self.connection()
            .execute(
                "INSERT INTO threads (
                    id, rollout_path, created_at, updated_at, source, thread_source,
                    parent_thread_id, agent_path, agent_nickname, agent_role, cwd, title
                ) VALUES (?1, ?2, 1, 2, ?3, ?4, ?5, ?6, ?7, ?8, '/workspace/example', ?9)",
                params![
                    id,
                    rollout.to_str().unwrap(),
                    parent.map_or_else(|| "cli".to_owned(), |_| source.to_string()),
                    if parent.is_some() { "subagent" } else { "user" },
                    parent,
                    agent_path,
                    parent.map(|_| title),
                    parent.map(|_| "explorer"),
                    title,
                ],
            )
            .unwrap();
        rollout
    }

    fn child(&self, id: &str, parent: &str, agent_path: &str, events: &[Value]) -> PathBuf {
        self.add_thread(id, Some(parent), Some(agent_path), "Reviewer", events)
    }

    fn set_rollout(&self, id: &str, rollout: &Path) {
        self.connection()
            .execute(
                "UPDATE threads SET rollout_path = ?1 WHERE id = ?2",
                params![rollout.to_str().unwrap(), id],
            )
            .unwrap();
    }

    fn root_detail(&self) -> ThreadDetail {
        thread_detail(&self.paths, ROOT).unwrap().unwrap()
    }

    fn read(&self, id: &str) -> anyhow::Result<SubagentDetailResponse> {
        read_subagent_detail(&self.paths, &request(ROOT, id, None, None))
    }

    fn read_attachment(
        &self,
        request: SessionAttachmentRequest,
    ) -> anyhow::Result<SessionAttachmentResponse> {
        let platform =
            PlatformPaths::for_desktop_kind_with_home(PlatformKind::Linux, &self.directory);
        let mut sessions = NexusHubUseCases::new(&platform).sessions(self.paths.clone());
        sessions.grok.home = self.directory.join("grok");
        sessions.claude.config = self.directory.join("claude");
        sessions.claude.projects = sessions.claude.config.join("projects");
        sessions.attachment_read(request)
    }

    fn snapshot(&self) -> BTreeMap<PathBuf, [u8; 32]> {
        fn visit(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, [u8; 32]>) {
            for entry in fs::read_dir(directory).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                let kind = entry.file_type().unwrap();
                if kind.is_dir() {
                    visit(root, &path, files);
                } else {
                    let bytes = if kind.is_symlink() {
                        fs::read_link(&path)
                            .unwrap()
                            .as_os_str()
                            .as_encoded_bytes()
                            .to_vec()
                    } else {
                        fs::read(&path).unwrap()
                    };
                    files.insert(
                        path.strip_prefix(root).unwrap().to_owned(),
                        Sha256::digest(bytes).into(),
                    );
                }
            }
        }
        let mut files = BTreeMap::new();
        visit(&self.directory, &self.directory, &mut files);
        files
    }
}

impl Drop for NativeFixture {
    fn drop(&mut self) {
        // This UUID directory and every file below it belong to this fixture.
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn write_jsonl(path: &Path, events: &[Value]) {
    let mut text = events
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    text.push('\n');
    fs::write(path, text).unwrap();
}

fn message(role: &str, text: &str) -> Value {
    json!({"type":"response_item","payload":{
        "type":"message","role":role,"content":[{"type":"input_text","text":text}]
    }})
}

fn image_message(id: &str) -> Value {
    json!({"type":"response_item","payload":{
        "id":id,"type":"message","role":"user","content":[
            {"type":"input_text","text":"Inspect the attached example image."},
            {"type":"input_image","image_url":format!("data:image/png;base64,{PNG}")}
        ]
    }})
}

fn attachment_request(detail: &ThreadDetail) -> SessionAttachmentRequest {
    let message = detail
        .blocks
        .iter()
        .filter_map(|block| block.user_message.as_ref())
        .find(|message| !message.attachments.is_empty())
        .unwrap();
    assert_eq!(message.attachments.len(), 1);
    assert!(message.attachments[0].reason.is_none());
    SessionAttachmentRequest {
        root_thread_id: Some(ROOT.to_owned()),
        provider: SessionProvider::Codex,
        session_key: detail.summary.id.clone(),
        message_id: message.id.clone(),
        attachment_id: message.attachments[0].id.clone(),
    }
}

fn event(kind: &str, turn: &str) -> Value {
    json!({"type":"event_msg","payload":{"type":kind,"turn_id":turn}})
}

fn tool(call_id: &str, name: &str, input: Value, output: Value) -> Vec<Value> {
    vec![
        json!({"type":"response_item","payload":{
            "type":"function_call","call_id":call_id,"name":name,"arguments":input.to_string()
        }}),
        json!({"type":"response_item","payload":{
            "type":"function_call_output","call_id":call_id,"output":output.to_string()
        }}),
    ]
}

fn spawn(call_id: &str, task_name: &str, output: Value) -> Vec<Value> {
    tool(
        call_id,
        "collaboration.spawn_agent",
        json!({"task_name":task_name,"agent_type":"explorer","message":"Inspect the example fixture."}),
        output,
    )
}

fn request(
    root: &str,
    agent: &str,
    limit: Option<usize>,
    before: Option<&str>,
) -> SubagentDetailRequest {
    SubagentDetailRequest {
        root_thread_id: root.to_owned(),
        agent_id: agent.to_owned(),
        limit,
        before: before.map(str::to_owned),
    }
}

fn cards(detail: &ThreadDetail) -> Vec<&SubagentActivity> {
    detail
        .blocks
        .iter()
        .filter_map(|block| block.subagent.as_ref())
        .collect()
}

fn assert_unavailable(fixture: &NativeFixture) {
    let mut detail = fixture.root_detail();
    enrich_subagent_blocks(&fixture.paths, &mut detail);
    let cards = cards(&detail);
    assert_eq!(cards.len(), 1);
    assert!(!cards[0].available);
    assert_eq!(cards[0].status, SubagentStatus::Unknown);
    assert!(cards[0].unavailable_reason.is_some());
    assert!(fixture.read(CHILD).is_err());
}

#[test]
fn returned_agent_id_binds_native_identity_at_the_original_tool_position() {
    let mut events = vec![message("user", "Review the example.")];
    events.extend(spawn("spawn-review", "review", json!({"agent_id":CHILD})));
    events.push(message("assistant", "The review was delegated."));
    let fixture = NativeFixture::new(&events);
    fixture.child(
        CHILD,
        ROOT,
        "/root/review",
        &[
            event("task_started", "review-turn"),
            message("assistant", "Example reviewed."),
            event("task_complete", "review-turn"),
        ],
    );
    let before = fixture.snapshot();
    let mut detail = fixture.root_detail();
    let block_ids: Vec<_> = detail.blocks.iter().map(|block| block.id.clone()).collect();

    enrich_subagent_blocks(&fixture.paths, &mut detail);

    assert_eq!(
        detail.blocks.iter().map(|block| block.id.clone()).collect::<Vec<_>>(),
        block_ids
    );
    assert_eq!(detail.blocks.len(), 3);
    assert_eq!(detail.blocks[1].call_id.as_deref(), Some("spawn-review"));
    let card = detail.blocks[1].subagent.as_ref().unwrap();
    assert_eq!(card.agent_id.as_deref(), Some(CHILD));
    assert_eq!(card.name, "Reviewer");
    assert_eq!(card.role.as_deref(), Some("explorer"));
    assert_eq!(card.status, SubagentStatus::Completed);
    assert!(card.available);
    assert_eq!(card.unavailable_reason, None);
    assert_eq!(card.delegation.as_deref(), Some("Inspect the example fixture."));
    let child = fixture.read(CHILD).unwrap();
    assert_eq!(child.root_thread_id, ROOT);
    assert_eq!(child.parent_thread_id, ROOT);
    assert_eq!(child.detail.summary.id, CHILD);
    assert_eq!(child.detail.blocks[0].text.as_deref(), Some("Example reviewed."));
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn returned_task_paths_resolve_distinct_agents_with_the_same_display_name() {
    let mut events = spawn("spawn-first", "review", json!({"task_name":"/root/review"}));
    events.extend(spawn("spawn-second", "review", json!({"task_name":"/root/review_again"})));
    let fixture = NativeFixture::new(&events);
    fixture.child(CHILD, ROOT, "/root/review", &[message("assistant", "First result.")]);
    fixture.child("second-child", ROOT, "/root/review_again", &[message("assistant", "Second result.")]);
    // Exercise the native structured source when dedicated association columns are absent.
    fixture.connection().execute(
        "UPDATE threads SET parent_thread_id = NULL, agent_path = NULL WHERE id != ?1",
        [ROOT],
    ).unwrap();
    let before = fixture.snapshot();
    let mut detail = fixture.root_detail();

    enrich_subagent_blocks(&fixture.paths, &mut detail);

    let cards = cards(&detail);
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].agent_id.as_deref(), Some(CHILD));
    assert_eq!(cards[1].agent_id.as_deref(), Some("second-child"));
    assert_eq!(cards[0].name, cards[1].name);
    assert!(cards.iter().all(|card| card.available));
    assert_eq!(fixture.read(CHILD).unwrap().detail.blocks[0].text.as_deref(), Some("First result."));
    assert_eq!(fixture.read("second-child").unwrap().detail.blocks[0].text.as_deref(), Some("Second result."));
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn identical_names_or_paths_do_not_bind_another_parent_or_guess_an_identity() {
    let mut events = spawn("foreign-id", "review", json!({"agent_id":"foreign-child"}));
    events.extend(spawn("name-only", "Reviewer", json!({"name":"Reviewer"})));
    events.extend(spawn("valid-path", "review", json!({"task_name":"/root/review"})));
    let fixture = NativeFixture::new(&events);
    fixture.child(CHILD, ROOT, "/root/review", &[]);
    fixture.add_thread("other-root", None, None, "Other task", &[]);
    fixture.child("foreign-child", "other-root", "/root/review", &[]);
    let mut detail = fixture.root_detail();

    enrich_subagent_blocks(&fixture.paths, &mut detail);

    let cards = cards(&detail);
    assert_eq!(cards.len(), 3);
    assert!(!cards[0].available);
    assert!(!cards[1].available);
    assert_eq!(cards[2].agent_id.as_deref(), Some(CHILD));
    assert!(cards[2].available);
    assert!(fixture.read("foreign-child").is_err());
}

#[test]
fn duplicate_native_paths_are_unavailable_until_the_returned_id_disambiguates() {
    let mut events = spawn("ambiguous-path", "review", json!({"task_name":"/root/review"}));
    events.extend(spawn("explicit-id", "review", json!({"agent_id":CHILD})));
    let fixture = NativeFixture::new(&events);
    fixture.child(CHILD, ROOT, "/root/review", &[]);
    fixture.child("second-child", ROOT, "/root/review", &[]);
    let mut detail = fixture.root_detail();

    enrich_subagent_blocks(&fixture.paths, &mut detail);

    let cards = cards(&detail);
    assert_eq!(cards.len(), 2);
    assert!(!cards[0].available);
    assert_eq!(cards[1].agent_id.as_deref(), Some(CHILD));
    assert!(cards[1].available);
}

#[test]
fn nested_children_keep_the_original_root_and_their_immediate_parent() {
    let fixture = NativeFixture::new(&spawn("spawn-parent", "review", json!({"agent_id":CHILD})));
    fixture.child(CHILD, ROOT, "/root/review", &spawn(
        "spawn-grandchild", "details", json!({"task_name":"/root/review/details"}),
    ));
    fixture.child("grandchild", CHILD, "/root/review/details", &[message("assistant", "Nested result.")]);
    fixture.add_thread("other-root", None, None, "Other task", &[]);
    let before = fixture.snapshot();

    let child = fixture.read(CHILD).unwrap();
    let grandchild = fixture.read("grandchild").unwrap();

    assert_eq!(cards(&child.detail)[0].agent_id.as_deref(), Some("grandchild"));
    assert!(cards(&child.detail)[0].available);
    assert_eq!(grandchild.root_thread_id, ROOT);
    assert_eq!(grandchild.parent_thread_id, CHILD);
    assert_eq!(grandchild.detail.blocks[0].text.as_deref(), Some("Nested result."));
    assert!(read_subagent_detail(&fixture.paths, &request("other-root", "grandchild", None, None)).is_err());
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn user_prose_code_examples_and_other_tools_do_not_create_subagent_cards() {
    let mut events = vec![
        message("user", "Please explain collaboration.spawn_agent({\"task_name\":\"review\"})."),
        message("assistant", "```json\n{\"type\":\"function_call\",\"name\":\"spawn_agent\",\"agent_id\":\"child-thread\"}\n```"),
    ];
    events.extend(tool("regular-tool", "functions.exec_command", json!({"cmd":"print example"}), json!({"agent_id":CHILD})));
    let fixture = NativeFixture::new(&events);
    fixture.child(CHILD, ROOT, "/root/review", &[]);
    let mut detail = fixture.root_detail();
    let original = serde_json::to_value(&detail).unwrap();

    enrich_subagent_blocks(&fixture.paths, &mut detail);

    assert!(cards(&detail).is_empty());
    assert_eq!(serde_json::to_value(detail).unwrap(), original);
}

#[test]
fn status_uses_native_turn_evidence_instead_of_spawn_tool_completion() {
    let cases = [
        ("running", vec![event("task_started", "turn")], SubagentStatus::Running),
        ("completed", vec![event("task_started", "turn"), event("task_complete", "turn")], SubagentStatus::Completed),
        ("failed", vec![event("task_started", "turn"), event("turn_error", "turn")], SubagentStatus::Failed),
        ("interrupted", vec![event("task_started", "turn"), event("turn_aborted", "turn")], SubagentStatus::Interrupted),
        ("unknown", vec![message("assistant", "An answer without terminal evidence.")], SubagentStatus::Unknown),
        ("other-turn", vec![event("task_started", "current"), event("task_complete", "previous")], SubagentStatus::Running),
        ("new-turn", vec![event("task_started", "old"), event("task_complete", "old"), event("task_started", "new")], SubagentStatus::Running),
        ("failed-status", vec![event("task_started", "turn"), json!({"type":"event_msg","payload":{"type":"turn_completed","turn_id":"turn","status":"failed"}})], SubagentStatus::Failed),
        ("cancelled-status", vec![event("task_started", "turn"), json!({"type":"event_msg","payload":{"type":"turn_completed","turn_id":"turn","status":"cancelled"}})], SubagentStatus::Interrupted),
    ];
    for (case, events, expected) in cases {
        let fixture = NativeFixture::new(&spawn("spawn-status", "review", json!({"agent_id":CHILD})));
        fixture.child(CHILD, ROOT, "/root/review", &events);
        let before = fixture.snapshot();
        let mut detail = fixture.root_detail();
        enrich_subagent_blocks(&fixture.paths, &mut detail);
        assert_eq!(cards(&detail)[0].status, expected, "{case}");
        assert!(cards(&detail)[0].available, "{case}");
        let child = fixture.read(CHILD).unwrap();
        assert_eq!(child.agent.status, expected, "{case}");
        assert_eq!(child.detail.summary.status == ThreadStatus::Running, expected == SubagentStatus::Running, "{case}");
        assert_eq!(fixture.snapshot(), before, "{case}");
    }
}

#[test]
fn unresolved_tools_prevent_completed_status_and_matched_results_allow_it() {
    for resolved in [false, true] {
        let mut events = vec![event("task_started", "turn")];
        let tool_events = tool("native-tool", "functions.exec_command", json!({"cmd":"print example"}), json!("Example output."));
        events.push(tool_events[0].clone());
        if resolved {
            events.push(tool_events[1].clone());
        }
        events.push(event("task_complete", "turn"));
        let fixture = NativeFixture::new(&spawn("spawn-status", "review", json!({"agent_id":CHILD})));
        fixture.child(CHILD, ROOT, "/root/review", &events);
        let child = fixture.read(CHILD).unwrap();
        assert_eq!(child.agent.status, if resolved { SubagentStatus::Completed } else { SubagentStatus::Unknown });
    }
}

#[test]
fn a_partial_or_malformed_trailing_record_keeps_readable_history_with_unknown_status() {
    for tail in ["{\"type\":\"event_msg\",\"payload\":", "{not-json}\n", "{\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"turn\"}}"] {
        let fixture = NativeFixture::new(&spawn("spawn-status", "review", json!({"agent_id":CHILD})));
        let rollout = fixture.child(CHILD, ROOT, "/root/review", &[
            event("task_started", "turn"), message("assistant", "A readable earlier result."), event("task_complete", "turn"),
        ]);
        fs::OpenOptions::new().append(true).open(rollout).unwrap().write_all(tail.as_bytes()).unwrap();
        let before = fixture.snapshot();

        let child = fixture.read(CHILD).unwrap();

        assert_eq!(child.agent.status, SubagentStatus::Unknown);
        assert!(child.agent.available);
        assert!(child.detail.blocks.iter().any(|block| block.text.as_deref() == Some("A readable earlier result.")));
        assert_eq!(fixture.snapshot(), before);
    }
}

#[test]
fn missing_native_index_does_not_get_recreated_by_subagent_reads() {
    let fixture = NativeFixture::new(&spawn("spawn-missing", "review", json!({"agent_id":CHILD})));
    fixture.child(CHILD, ROOT, "/root/review", &[]);
    let mut detail = fixture.root_detail();
    fs::remove_file(fixture.paths.state_db()).unwrap();
    let before = fixture.snapshot();

    enrich_subagent_blocks(&fixture.paths, &mut detail);

    assert!(!cards(&detail)[0].available);
    assert!(fixture.read(CHILD).is_err());
    assert!(!fixture.paths.state_db().exists());
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn missing_child_index_or_rollout_and_unverifiable_file_identity_are_unavailable() {
    for case in ["missing-row", "missing-file", "missing-meta", "different-id", "different-parent"] {
        let fixture = NativeFixture::new(&spawn("spawn-missing", "review", json!({"agent_id":CHILD})));
        let rollout = fixture.child(CHILD, ROOT, "/root/review", &[]);
        match case {
            "missing-row" => { fixture.connection().execute("DELETE FROM threads WHERE id = ?1", [CHILD]).unwrap(); }
            "missing-file" => fs::remove_file(rollout).unwrap(),
            "missing-meta" => write_jsonl(&rollout, &[message("assistant", "No native identity.")]),
            "different-id" => write_jsonl(&rollout, &[json!({"type":"session_meta","payload":{"id":"replacement-thread","parent_thread_id":ROOT}})]),
            "different-parent" => write_jsonl(&rollout, &[json!({"type":"session_meta","payload":{"id":CHILD,"parent_thread_id":"other-root"}})]),
            _ => unreachable!(),
        }
        let before = fixture.snapshot();
        assert_unavailable(&fixture);
        assert_eq!(fixture.snapshot(), before, "{case}");
    }
}

#[test]
fn contradictory_native_parent_columns_and_edges_are_rejected() {
    for via_edge in [false, true] {
        let fixture = NativeFixture::new(&spawn("spawn-conflict", "review", json!({"agent_id":CHILD})));
        fixture.child(CHILD, ROOT, "/root/review", &[]);
        if via_edge {
            fixture.connection().execute("INSERT INTO thread_spawn_edges VALUES ('other-root', ?1)", [CHILD]).unwrap();
        } else {
            fixture.connection().execute("UPDATE threads SET parent_thread_id = 'other-root' WHERE id = ?1", [CHILD]).unwrap();
        }
        let before = fixture.snapshot();
        assert_unavailable(&fixture);
        assert_eq!(fixture.snapshot(), before);
    }
}

#[test]
fn native_spawn_edge_can_supply_the_only_parent_association() {
    let fixture = NativeFixture::new(&spawn("spawn-edge", "review", json!({"agent_id":CHILD})));
    fixture.child(CHILD, ROOT, "/root/review", &[]);
    fixture.connection().execute("UPDATE threads SET parent_thread_id = NULL, source = 'cli' WHERE id = ?1", [CHILD]).unwrap();
    fixture.connection().execute("INSERT INTO thread_spawn_edges VALUES (?1, ?2)", [ROOT, CHILD]).unwrap();
    let before = fixture.snapshot();
    let mut detail = fixture.root_detail();

    enrich_subagent_blocks(&fixture.paths, &mut detail);

    assert!(cards(&detail)[0].available);
    assert_eq!(fixture.read(CHILD).unwrap().parent_thread_id, ROOT);
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn cycles_and_missing_ancestors_cannot_be_read_through_an_unrelated_root() {
    for cycle in [false, true] {
        let fixture = NativeFixture::new(&[]);
        fixture.child(CHILD, "missing-parent", "/root/review", &[]);
        if cycle {
            fixture.child("missing-parent", CHILD, "/root/review/loop", &[]);
        }
        let before = fixture.snapshot();
        assert!(fixture.read(CHILD).is_err());
        assert_eq!(fixture.snapshot(), before);
    }
}

#[test]
fn rollout_paths_outside_session_roots_and_directory_paths_are_rejected() {
    for case in ["outside-home", "outside-sessions", "directory", "traversal"] {
        let fixture = NativeFixture::new(&spawn("spawn-path", "review", json!({"agent_id":CHILD})));
        let original = fixture.child(CHILD, ROOT, "/root/review", &[]);
        let destination = match case {
            "outside-home" | "traversal" => fixture.directory.join("outside.jsonl"),
            "outside-sessions" => fixture.paths.home.join("outside.jsonl"),
            "directory" => fixture.paths.sessions_dir().join("directory"),
            _ => unreachable!(),
        };
        if case == "directory" {
            fs::create_dir(&destination).unwrap();
        } else {
            fs::copy(original, &destination).unwrap();
        }
        let indexed = if case == "traversal" {
            fixture.paths.sessions_dir().join("../../outside.jsonl")
        } else {
            destination
        };
        fixture.set_rollout(CHILD, &indexed);
        let before = fixture.snapshot();
        assert_unavailable(&fixture);
        assert_eq!(fixture.snapshot(), before, "{case}");
    }
}

#[cfg(unix)]
#[test]
fn symlinked_files_and_intermediate_directories_are_rejected() {
    use std::os::unix::fs::symlink;

    for directory_link in [false, true] {
        let fixture = NativeFixture::new(&spawn("spawn-symlink", "review", json!({"agent_id":CHILD})));
        let original = fixture.child(CHILD, ROOT, "/root/review", &[]);
        let indexed = if directory_link {
            let actual = fixture.paths.sessions_dir().join("actual");
            fs::create_dir(&actual).unwrap();
            fs::rename(&original, actual.join("child.jsonl")).unwrap();
            let link = fixture.paths.sessions_dir().join("linked");
            symlink(actual, &link).unwrap();
            link.join("child.jsonl")
        } else {
            let target = fixture.paths.sessions_dir().join("target.jsonl");
            fs::rename(&original, &target).unwrap();
            symlink(target, &original).unwrap();
            original
        };
        fixture.set_rollout(CHILD, &indexed);
        let before = fixture.snapshot();
        assert_unavailable(&fixture);
        assert_eq!(fixture.snapshot(), before);
    }
}

#[test]
fn archived_child_rollouts_remain_readable_through_their_verified_parent() {
    let fixture = NativeFixture::new(&spawn("spawn-archived", "review", json!({"agent_id":CHILD})));
    let original = fixture.child(CHILD, ROOT, "/root/review", &[message("assistant", "Archived result.")]);
    let archived = fixture.paths.home.join("archived_sessions/child.jsonl");
    fs::rename(original, &archived).unwrap();
    fixture.set_rollout(CHILD, &archived);
    fixture.connection().execute("UPDATE threads SET archived = 1 WHERE id = ?1", [CHILD]).unwrap();
    let before = fixture.snapshot();

    let child = fixture.read(CHILD).unwrap();

    assert_eq!(child.detail.blocks[0].text.as_deref(), Some("Archived result."));
    assert!(child.agent.available);
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn children_stay_hidden_from_the_main_list_but_are_readable_from_the_parent() {
    let fixture = NativeFixture::new(&spawn("spawn-list", "review", json!({"agent_id":CHILD})));
    fixture.child(CHILD, ROOT, "/root/review", &[]);
    fixture.child("grandchild", CHILD, "/root/review/details", &[]);
    let before = fixture.snapshot();

    for status in [None, Some("all")] {
        let rows = list_threads(&fixture.paths, status, None, 100).unwrap();
        assert_eq!(rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(), vec![ROOT]);
    }
    let hidden = hidden_thread_ids(&fixture.paths).unwrap();
    assert!(hidden.contains(CHILD));
    assert!(hidden.contains("grandchild"));
    assert!(thread_detail(&fixture.paths, CHILD).unwrap().is_none());
    assert!(fixture.read(CHILD).is_ok());
    assert!(fixture.read("grandchild").is_ok());
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn detail_pagination_preserves_order_cursors_and_bounded_limits() {
    let fixture = NativeFixture::new(&[]);
    let mut events = vec![event("task_started", "paged-turn")];
    for index in 0..505 {
        events.extend(tool(
            &format!("page-tool-{index}"),
            "functions.exec_command",
            json!({"cmd":"print example"}),
            json!({"sequence":index}),
        ));
    }
    events.push(event("task_complete", "paged-turn"));
    fixture.child(CHILD, ROOT, "/root/review", &events);
    let before = fixture.snapshot();
    let read_page = |limit, before| read_subagent_detail(&fixture.paths, &request(ROOT, CHILD, limit, before)).unwrap().detail;

    let latest = read_page(Some(5), None);
    assert_eq!(latest.total_blocks, 505);
    assert_eq!(latest.blocks.len(), 5);
    assert_eq!(latest.blocks[0].call_id.as_deref(), Some("page-tool-500"));
    assert_eq!(latest.blocks[4].call_id.as_deref(), Some("page-tool-504"));
    assert!(latest.has_more_blocks);
    assert_eq!(latest.before_cursor.as_deref(), Some("b:500"));
    assert!(latest.messages.is_empty());
    let older = read_page(Some(5), latest.before_cursor.as_deref());
    assert_eq!(older.blocks[0].call_id.as_deref(), Some("page-tool-495"));
    assert_eq!(older.blocks[4].call_id.as_deref(), Some("page-tool-499"));
    assert_eq!(older.before_cursor.as_deref(), Some("b:495"));
    let first = read_page(Some(5), Some("b:3"));
    assert_eq!(first.blocks.len(), 3);
    assert_eq!(first.blocks[0].call_id.as_deref(), Some("page-tool-0"));
    assert!(!first.has_more_blocks);
    assert_eq!(first.before_cursor, None);
    assert!(read_page(Some(5), Some("b:0")).blocks.is_empty());
    assert_eq!(read_page(None, None).blocks.len(), 120);
    assert_eq!(read_page(Some(0), None).blocks.len(), 1);
    assert_eq!(read_page(Some(usize::MAX), None).blocks.len(), 500);
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn invalid_thread_keys_and_history_cursors_are_rejected() {
    let fixture = NativeFixture::new(&[]);
    fixture.child(CHILD, ROOT, "/root/review", &[]);
    let before = fixture.snapshot();
    for invalid in ["", "../child", "child/other", "child\\other", "child\nother"] {
        assert!(read_subagent_detail(&fixture.paths, &request(ROOT, invalid, None, None)).is_err());
        assert!(read_subagent_detail(&fixture.paths, &request(invalid, CHILD, None, None)).is_err());
    }
    assert!(fixture.read(&"a".repeat(257)).is_err());
    assert!(fixture.read(ROOT).is_err());
    for cursor in ["", "500", "b:-1", "b:invalid", "b:1suffix"] {
        assert!(read_subagent_detail(&fixture.paths, &request(ROOT, CHILD, Some(5), Some(cursor))).is_err());
    }
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn parent_context_reads_direct_and_nested_child_attachments_without_polling_bytes() {
    let fixture = NativeFixture::new(&[]);
    fixture.child(CHILD, ROOT, "/root/review", &[image_message("child-image")]);
    fixture.child(
        "grandchild",
        CHILD,
        "/root/review/nested",
        &[image_message("nested-image")],
    );
    let before = fixture.snapshot();

    for child in [CHILD, "grandchild"] {
        let detail = fixture.read(child).unwrap().detail;
        let wire = serde_json::to_string(&detail).unwrap();
        assert!(!wire.contains(PNG));
        assert!(!wire.contains("data:image/"));
        let response = fixture.read_attachment(attachment_request(&detail)).unwrap();
        assert_eq!(response.mime_type, "image/png");
        assert_eq!(response.base64, PNG);
    }

    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn child_attachments_reject_unrelated_missing_and_omitted_parent_contexts() {
    let fixture = NativeFixture::new(&[]);
    fixture.child(CHILD, ROOT, "/root/review", &[image_message("child-image")]);
    fixture.add_thread("other-root", None, None, "Other task", &[]);
    let valid = attachment_request(&fixture.read(CHILD).unwrap().detail);
    assert!(fixture.read_attachment(valid.clone()).is_ok());
    let before = fixture.snapshot();

    for root in [Some("other-root"), Some("missing-root"), Some(CHILD), None] {
        let mut request = valid.clone();
        request.root_thread_id = root.map(str::to_owned);
        assert!(fixture.read_attachment(request).is_err(), "root: {root:?}");
    }

    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn parent_context_is_rejected_for_non_codex_attachment_providers() {
    let fixture = NativeFixture::new(&[]);
    fixture.child(CHILD, ROOT, "/root/review", &[image_message("child-image")]);
    let valid = attachment_request(&fixture.read(CHILD).unwrap().detail);
    assert!(fixture.read_attachment(valid.clone()).is_ok());
    let before = fixture.snapshot();

    for provider in [SessionProvider::Claude, SessionProvider::Grok] {
        let mut request = valid.clone();
        request.provider = provider;
        let error = fixture.read_attachment(request).unwrap_err();
        assert!(error.to_string().contains("子智能体附件仅支持 Codex"));
    }

    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn child_attachment_identity_rejects_paths_unknown_ids_and_malformed_keys() {
    let fixture = NativeFixture::new(&[]);
    let rollout = fixture.child(
        CHILD,
        ROOT,
        "/root/review",
        &[image_message("child-image")],
    );
    let valid = attachment_request(&fixture.read(CHILD).unwrap().detail);
    assert!(fixture.read_attachment(valid.clone()).is_ok());
    let before = fixture.snapshot();

    for attachment_id in [
        String::new(),
        "missing-attachment".to_owned(),
        "../child-image.png".to_owned(),
        "child\\image.png".to_owned(),
        "attachment\nother".to_owned(),
        "a".repeat(513),
        rollout.to_string_lossy().into_owned(),
    ] {
        let mut request = valid.clone();
        request.attachment_id = attachment_id;
        assert!(fixture.read_attachment(request).is_err());
    }
    let mut missing_message = valid;
    missing_message.message_id = "missing-message".to_owned();
    assert!(fixture.read_attachment(missing_message).is_err());

    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn child_attachment_reads_revalidate_the_parent_after_detail_is_cached() {
    let fixture = NativeFixture::new(&[]);
    fixture.child(CHILD, ROOT, "/root/review", &[image_message("child-image")]);
    fixture.add_thread("other-root", None, None, "Other task", &[]);
    let request = attachment_request(&fixture.read(CHILD).unwrap().detail);
    assert!(fixture.read_attachment(request.clone()).is_ok());
    fixture
        .connection()
        .execute(
            "UPDATE threads SET parent_thread_id = 'other-root' WHERE id = ?1",
            [CHILD],
        )
        .unwrap();
    let before = fixture.snapshot();

    assert!(fixture.read_attachment(request).is_err());

    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn child_attachments_from_older_detail_pages_remain_readable() {
    let fixture = NativeFixture::new(&[]);
    let mut events = vec![image_message("older-image")];
    for index in 0..130 {
        events.extend(tool(
            &format!("attachment-page-tool-{index}"),
            "functions.exec_command",
            json!({"cmd":"print example"}),
            json!({"sequence":index}),
        ));
    }
    fixture.child(CHILD, ROOT, "/root/review", &events);
    let before = fixture.snapshot();
    let latest = fixture.read(CHILD).unwrap().detail;
    assert_eq!(latest.blocks.len(), 120);
    assert!(latest.has_more_blocks);
    assert!(latest.blocks.iter().all(|block| block.user_message.is_none()));
    let older = read_subagent_detail(
        &fixture.paths,
        &request(ROOT, CHILD, None, latest.before_cursor.as_deref()),
    )
    .unwrap()
    .detail;
    assert!(!older.has_more_blocks);

    let response = fixture.read_attachment(attachment_request(&older)).unwrap();

    assert_eq!(response.mime_type, "image/png");
    assert_eq!(response.base64, PNG);
    assert_eq!(fixture.snapshot(), before);
}

#[test]
fn older_spawn_status_updates_survive_latest_page_without_reintroducing_blocks() {
    let mut events = spawn("older-spawn", "review", json!({"agent_id":CHILD}));
    for index in 0..5 {
        events.extend(tool(
            &format!("newer-tool-{index}"),
            "functions.exec_command",
            json!({"cmd":"print example"}),
            json!({"sequence":index}),
        ));
    }
    let fixture = NativeFixture::new(&events);
    let child_rollout = fixture.child(
        CHILD,
        ROOT,
        "/root/review",
        &[event("task_started", "review-turn")],
    );
    let before = fixture.snapshot();
    let mut detail = fixture.root_detail();
    let spawn_id = detail
        .blocks
        .iter()
        .find(|block| block.call_id.as_deref() == Some("older-spawn"))
        .unwrap()
        .id
        .clone();
    enrich_subagent_blocks(&fixture.paths, &mut detail);
    let running = window_thread_detail(detail, Some(2), None);
    assert_eq!(running.blocks.len(), 2);
    assert!(running.has_more_blocks);
    assert!(running.blocks.iter().all(|block| block.id != spawn_id));
    let wire = serde_json::to_value(&running).unwrap();
    let updates = wire["subagent_updates"].as_object().unwrap();
    assert_eq!(updates.len(), 1);
    let update = updates.get(&spawn_id).unwrap();
    assert_eq!(update["agentId"], CHILD);
    assert_eq!(update["status"], "running");
    assert_eq!(update["available"], true);
    assert!(update["delegation"].is_null());
    assert_eq!(fixture.snapshot(), before);

    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(child_rollout)
        .unwrap();
    writeln!(file, "{}", event("task_complete", "review-turn")).unwrap();
    drop(file);
    let after_completion = fixture.snapshot();
    let mut detail = fixture.root_detail();
    enrich_subagent_blocks(&fixture.paths, &mut detail);
    let completed = window_thread_detail(detail, Some(2), None);

    assert_eq!(
        completed.blocks.iter().map(|block| &block.id).collect::<Vec<_>>(),
        running.blocks.iter().map(|block| &block.id).collect::<Vec<_>>()
    );
    assert_eq!(completed.total_blocks, running.total_blocks);
    assert_eq!(completed.before_cursor, running.before_cursor);
    let wire = serde_json::to_value(&completed).unwrap();
    let updates = wire["subagent_updates"].as_object().unwrap();
    assert_eq!(updates.len(), 1);
    let update = updates.get(&spawn_id).unwrap();
    assert_eq!(update["agentId"], CHILD);
    assert_eq!(update["status"], "completed");
    assert_eq!(update["available"], true);
    assert!(update["delegation"].is_null());
    assert_eq!(fixture.snapshot(), after_completion);
}

#[test]
fn long_delegations_survive_pending_and_completed_tool_input_previews() {
    let delegation = format!(
        "{}Final instruction: preserve this ending.",
        "Inspect the example fixture.\n".repeat(180)
    );
    assert!(delegation.chars().count() > 4_000);
    let input = json!({
        "task_name": "long-review",
        "agent_type": "explorer",
        "message": delegation,
    });
    let original_input = input.to_string();

    for resolved in [false, true] {
        let mut events = tool(
            "spawn-long-review",
            "collaboration.spawn_agent",
            input.clone(),
            json!({"agent_id": CHILD}),
        );
        if !resolved {
            events.pop();
        }
        let fixture = NativeFixture::new(&events);
        fixture.child(
            CHILD,
            ROOT,
            "/root/long-review",
            &[
                event("task_started", "review-turn"),
                event("task_complete", "review-turn"),
            ],
        );
        let before = fixture.snapshot();
        let mut detail = fixture.root_detail();
        assert_eq!(detail.blocks.len(), 1);
        let block = &detail.blocks[0];
        assert_eq!(block.resolved, Some(resolved));
        let preview = block.input.as_deref().unwrap();
        assert_eq!(preview.chars().count(), 4_003);
        assert!(original_input.starts_with(preview.strip_suffix("...").unwrap()));
        assert!(serde_json::from_str::<Value>(preview).is_err());
        assert!(!preview.contains("Final instruction: preserve this ending."));
        let card = block.subagent.as_ref().unwrap();
        assert_eq!(card.name, "long-review");
        assert_eq!(card.role.as_deref(), Some("explorer"));
        assert_eq!(card.delegation.as_deref(), Some(delegation.as_str()));
        assert!(!card.available);
        let original_preview = block.input.clone();

        enrich_subagent_blocks(&fixture.paths, &mut detail);

        let block = &detail.blocks[0];
        assert_eq!(block.input, original_preview);
        let card = block.subagent.as_ref().unwrap();
        assert_eq!(card.delegation.as_deref(), Some(delegation.as_str()));
        assert_eq!(card.available, resolved);
        assert_eq!(card.agent_id.as_deref(), resolved.then_some(CHILD));
        assert_eq!(
            card.status,
            if resolved {
                SubagentStatus::Completed
            } else {
                SubagentStatus::Unknown
            }
        );
        assert_eq!(fixture.snapshot(), before);
    }
}

#[test]
fn repeated_enrichment_revokes_stale_availability_after_native_association_changes() {
    for case in [
        "missing-index",
        "missing-row",
        "missing-rollout",
        "changed-parent",
        "changed-identity",
    ] {
        let fixture = NativeFixture::new(&spawn(
            "spawn-revalidate",
            "review",
            json!({"agent_id": CHILD}),
        ));
        let rollout = fixture.child(
            CHILD,
            ROOT,
            "/root/review",
            &[
                event("task_started", "review-turn"),
                event("task_complete", "review-turn"),
            ],
        );
        let mut detail = fixture.root_detail();
        enrich_subagent_blocks(&fixture.paths, &mut detail);
        let block_id = detail.blocks[0].id.clone();
        let card = detail.blocks[0].subagent.as_ref().unwrap();
        assert!(card.available, "{case}");
        assert_eq!(card.agent_id.as_deref(), Some(CHILD), "{case}");
        assert_eq!(card.status, SubagentStatus::Completed, "{case}");
        assert!(detail.subagent_updates[&block_id].available, "{case}");

        match case {
            "missing-index" => fs::remove_file(fixture.paths.state_db()).unwrap(),
            "missing-row" => {
                fixture
                    .connection()
                    .execute("DELETE FROM threads WHERE id = ?1", [CHILD])
                    .unwrap();
            }
            "missing-rollout" => fs::remove_file(&rollout).unwrap(),
            "changed-parent" => {
                fixture
                    .connection()
                    .execute(
                        "UPDATE threads SET parent_thread_id = 'other-root', source = 'cli' WHERE id = ?1",
                        [CHILD],
                    )
                    .unwrap();
            }
            "changed-identity" => write_jsonl(
                &rollout,
                &[json!({"type": "session_meta", "payload": {
                    "id": "replacement-thread", "parent_thread_id": ROOT,
                }})],
            ),
            _ => unreachable!(),
        }
        let after_change = fixture.snapshot();

        for _ in 0..2 {
            enrich_subagent_blocks(&fixture.paths, &mut detail);

            let card = detail.blocks[0].subagent.as_ref().unwrap();
            assert!(!card.available, "{case}");
            assert_eq!(card.agent_id, None, "{case}");
            assert_eq!(card.status, SubagentStatus::Unknown, "{case}");
            assert!(card.unavailable_reason.is_some(), "{case}");
            assert_eq!(
                card.delegation.as_deref(),
                Some("Inspect the example fixture."),
                "{case}"
            );
            let update = &detail.subagent_updates[&block_id];
            assert!(!update.available, "{case}");
            assert_eq!(update.agent_id, None, "{case}");
            assert_eq!(update.status, SubagentStatus::Unknown, "{case}");
            assert_eq!(update.unavailable_reason, card.unavailable_reason, "{case}");
            assert_eq!(update.delegation, None, "{case}");
        }
        assert!(fixture.read(CHILD).is_err(), "{case}");
        assert_eq!(fixture.snapshot(), after_change, "{case}");
    }
}
