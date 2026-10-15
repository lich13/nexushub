use super::*;
use rusqlite::params;
use serde_json::json;
use std::{cell::RefCell, process::Command, rc::Rc};

const SELECTED: &str = "maintenance-selected";
const RETAINED: &str = "maintenance-retained";
const FIXTURE_PREFIX: &str = "nexushub-selected-maintenance-";
const CHILD_STAGE: &str = "NEXUSHUB_SELECTED_MAINTENANCE_EXIT_STAGE";
const CHILD_TOKEN: &str = "NEXUSHUB_SELECTED_MAINTENANCE_FIXTURE_TOKEN";

struct Fixture {
    root: PathBuf,
    token: String,
    paths: CodexPaths,
}

impl Fixture {
    fn new() -> Self {
        let token = uuid::Uuid::new_v4().to_string();
        let root = std::env::temp_dir().join(format!("{FIXTURE_PREFIX}{token}"));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("fixture-token"), &token).unwrap();
        let paths = CodexPaths::new(root.join("codex"));
        fs::create_dir_all(paths.sessions_dir()).unwrap();
        fs::create_dir(paths.home.join("archived_sessions")).unwrap();
        let fixture = Self { root, token, paths };
        fixture
            .conn()
            .execute_batch(
                "CREATE TABLE threads(
                    id TEXT PRIMARY KEY, title TEXT, source TEXT,
                    rollout_path TEXT, archived INTEGER, status TEXT,
                    parent_thread_id TEXT, archived_at INTEGER);
                 CREATE TABLE thread_spawn_edges(parent_thread_id TEXT, child_thread_id TEXT);
                 CREATE TABLE thread_dynamic_tools(thread_id TEXT);
                 CREATE TABLE agent_job_items(id TEXT, assigned_thread_id TEXT, status TEXT);",
            )
            .unwrap();
        for id in [SELECTED, RETAINED] {
            fs::write(fixture.file(id), completed_rollout(id)).unwrap();
            fixture
                .conn()
                .execute(
                    "INSERT INTO threads VALUES(?1,?1,'cli',?2,1,'idle',?3,1)",
                    params![
                        id,
                        fixture.file(id).to_string_lossy(),
                        (id == RETAINED).then_some(SELECTED)
                    ],
                )
                .unwrap();
            fixture
                .conn()
                .execute("INSERT INTO thread_dynamic_tools VALUES(?1)", [id])
                .unwrap();
            fixture
                .conn()
                .execute(
                    "INSERT INTO agent_job_items VALUES(?1,?1,'completed')",
                    [id],
                )
                .unwrap();
        }
        fixture
            .conn()
            .execute(
                "INSERT INTO thread_spawn_edges VALUES(?1,?2)",
                params![SELECTED, RETAINED],
            )
            .unwrap();
        fs::write(
            fixture.paths.session_index(),
            [SELECTED, RETAINED]
                .map(|id| json!({"id": id, "thread_name": id}).to_string() + "\n")
                .concat(),
        )
        .unwrap();
        fixture
    }

    fn file(&self, id: &str) -> PathBuf {
        self.paths
            .home
            .join("archived_sessions")
            .join(format!("rollout-fixture-{id}.jsonl"))
    }

    fn conn(&self) -> Connection {
        Connection::open(self.paths.state_db()).unwrap()
    }

    fn database_rows(&self) -> Vec<Vec<Value>> {
        let conn = self.conn();
        [
            ("threads", "id"),
            ("thread_dynamic_tools", "thread_id"),
            ("thread_spawn_edges", "parent_thread_id, child_thread_id"),
            ("agent_job_items", "id"),
        ]
        .into_iter()
        .map(|(table, order)| {
            let mut stmt = conn
                .prepare(&format!("SELECT * FROM {table} ORDER BY {order}"))
                .unwrap();
            let names = stmt
                .column_names()
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>();
            let rows = stmt
                .query_map([], |row| read_row_value(row, &names))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
            rows
        })
        .collect()
    }

    fn quarantines(&self) -> Vec<PathBuf> {
        fs::read_dir(&self.paths.home)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".nexushub-delete-")
            })
            .collect()
    }

    fn assert_retained_rollout_unchanged(&self) {
        assert_eq!(
            fs::read(self.file(RETAINED)).unwrap(),
            completed_rollout(RETAINED).as_bytes()
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // This unique root contains only this test's synthetic data.
        let cleanup = fs::remove_dir_all(&self.root);
        if !std::thread::panicking() {
            cleanup.expect("remove the owned synthetic maintenance fixture");
        }
    }
}

fn completed_rollout(id: &str) -> String {
    format!(
        "{}\n{}\n{}\n",
        json!({"type":"session_meta","payload":{"id":id}}),
        json!({"type":"turn_started","turn_id":"fixture-turn"}),
        json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"fixture-turn"}}),
    )
}

struct MutationHook;

impl MutationHook {
    fn install(hook: impl FnMut(&str, &Path) -> Result<()> + 'static) -> Self {
        TEST_MUTATION_HOOK.with(|slot| {
            assert!(
                slot.borrow().is_none(),
                "a mutation hook is already installed"
            );
            *slot.borrow_mut() = Some(Box::new(hook));
        });
        Self
    }
}

impl Drop for MutationHook {
    fn drop(&mut self) {
        TEST_MUTATION_HOOK.with(|slot| *slot.borrow_mut() = None);
    }
}

#[test]
fn parent_components_in_database_and_index_paths_never_escape_the_fixture() {
    for source in ["database", "index"] {
        let fixture = Fixture::new();
        let preview = preview_delete(&fixture.paths, SELECTED).unwrap();
        let outside = fixture.root.join("outside.jsonl");
        let outside_bytes = completed_rollout(SELECTED);
        fs::write(&outside, &outside_bytes).unwrap();
        let traversal = fixture.paths.sessions_dir().join("../../outside.jsonl");
        if source == "database" {
            fixture
                .conn()
                .execute(
                    "UPDATE threads SET rollout_path=?1 WHERE id=?2",
                    params![traversal.to_string_lossy(), SELECTED],
                )
                .unwrap();
        } else {
            fs::write(
                fixture.paths.session_index(),
                format!(
                    "{}\n{}\n",
                    json!({"id":SELECTED,"path":traversal}),
                    json!({"id":RETAINED,"thread_name":RETAINED}),
                ),
            )
            .unwrap();
        }
        let rows = fixture.database_rows();
        let index = fs::read(fixture.paths.session_index()).unwrap();

        let preview_error = preview_delete(&fixture.paths, SELECTED).unwrap_err();
        assert!(preview_error.to_string().contains("不安全分量"));
        let execute_error =
            execute_delete(&fixture.paths, SELECTED, &preview.fingerprint).unwrap_err();
        assert!(execute_error.to_string().contains("不安全分量"));
        assert_eq!(fixture.database_rows(), rows);
        assert_eq!(fs::read(fixture.paths.session_index()).unwrap(), index);
        assert_eq!(fs::read(outside).unwrap(), outside_bytes.as_bytes());
        assert_eq!(
            fs::read(fixture.file(SELECTED)).unwrap(),
            completed_rollout(SELECTED).as_bytes()
        );
        fixture.assert_retained_rollout_unchanged();
        assert!(fixture.quarantines().is_empty());
    }
}

fn assert_quarantine_content_change_is_rejected(replacement: String) {
    let fixture = Fixture::new();
    let rows = fixture.database_rows();
    let index = fs::read(fixture.paths.session_index()).unwrap();
    let preview = preview_delete(&fixture.paths, SELECTED).unwrap();
    let new_bytes = replacement.clone();
    let reached = Rc::new(RefCell::new(false));
    let moved = reached.clone();
    let _hook = MutationHook::install(move |stage, quarantine| {
        if stage == "moved" {
            *moved.borrow_mut() = true;
            fs::write(quarantine.join("rollout-0.jsonl"), &new_bytes)?;
        }
        Ok(())
    });

    let result = execute_delete(&fixture.paths, SELECTED, &preview.fingerprint);

    assert!(
        *reached.borrow(),
        "the pre-verification moved hook must run"
    );
    let error = result.unwrap_err();
    assert!(format!("{error:#}").contains("隔离后"));
    assert_eq!(fixture.database_rows(), rows);
    assert_eq!(fs::read(fixture.paths.session_index()).unwrap(), index);
    // Rollback must preserve the externally changed bytes rather than deleting
    // the file on the strength of its pre-quarantine preview.
    assert_eq!(
        fs::read(fixture.file(SELECTED)).unwrap(),
        replacement.as_bytes()
    );
    fixture.assert_retained_rollout_unchanged();
}

#[test]
fn changed_header_identity_after_quarantine_blocks_deletion() {
    assert_quarantine_content_change_is_rejected(completed_rollout("fixture-other-identity"));
}

#[test]
fn appended_content_after_quarantine_blocks_deletion() {
    assert_quarantine_content_change_is_rejected(format!(
        "{}{}\n",
        completed_rollout(SELECTED),
        json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"fixture-turn","fixture_marker":"appended"}}),
    ));
}

#[cfg(unix)]
#[test]
fn same_content_with_replaced_inode_after_quarantine_blocks_deletion() {
    use std::os::unix::fs::MetadataExt;

    let fixture = Fixture::new();
    let rows = fixture.database_rows();
    let index = fs::read(fixture.paths.session_index()).unwrap();
    let preview = preview_delete(&fixture.paths, SELECTED).unwrap();
    let preserved_original = fixture.root.join("original-rollout.jsonl");
    let original_backup = preserved_original.clone();
    let identities = Rc::new(RefCell::new(None));
    let captured = identities.clone();
    let _hook = MutationHook::install(move |stage, quarantine| {
        if stage == "moved" {
            let staged = quarantine.join("rollout-0.jsonl");
            let original = fs::metadata(&staged)?;
            let bytes = fs::read(&staged)?;
            // Retaining the original inode makes the replacement deterministic;
            // unlink-and-recreate can immediately reuse an inode on some hosts.
            fs::rename(&staged, &original_backup)?;
            fs::write(&staged, &bytes)?;
            let replacement = fs::metadata(&staged)?;
            *captured.borrow_mut() = Some((
                (original.dev(), original.ino()),
                (replacement.dev(), replacement.ino()),
            ));
        }
        Ok(())
    });

    let result = execute_delete(&fixture.paths, SELECTED, &preview.fingerprint);

    let (original, replacement) = identities.borrow().expect("the moved hook must run");
    assert_ne!(original, replacement, "the fixture must replace the inode");
    let error = result.unwrap_err();
    assert!(format!("{error:#}").contains("隔离后"));
    assert_eq!(fixture.database_rows(), rows);
    assert_eq!(fs::read(fixture.paths.session_index()).unwrap(), index);
    assert_eq!(
        fs::read(fixture.file(SELECTED)).unwrap(),
        completed_rollout(SELECTED).as_bytes()
    );
    assert_eq!(
        fs::read(preserved_original).unwrap(),
        completed_rollout(SELECTED).as_bytes()
    );
    fixture.assert_retained_rollout_unchanged();
}

#[test]
fn index_candidate_write_failure_restores_rollouts_and_database() {
    let fixture = Fixture::new();
    let rows = fixture.database_rows();
    let index = fs::read(fixture.paths.session_index()).unwrap();
    let preview = preview_delete(&fixture.paths, SELECTED).unwrap();
    let reached = Rc::new(RefCell::new(Vec::new()));
    let stages = reached.clone();
    let _hook = MutationHook::install(move |stage, quarantine| {
        stages.borrow_mut().push(stage.to_string());
        if stage == "quarantined" {
            // A directory at the candidate path deterministically fails fs::write,
            // including when tests run with privileges that bypass permissions.
            fs::create_dir(quarantine.join("index.next"))?;
        }
        Ok(())
    });

    let error = execute_delete(&fixture.paths, SELECTED, &preview.fingerprint).unwrap_err();

    assert!(error.chain().any(|cause| cause.is::<std::io::Error>()));
    assert!(reached.borrow().iter().any(|stage| stage == "quarantined"));
    assert!(!reached
        .borrow()
        .iter()
        .any(|stage| stage == "index_replaced"));
    assert_eq!(fixture.database_rows(), rows);
    assert_eq!(fs::read(fixture.paths.session_index()).unwrap(), index);
    assert_eq!(
        fs::read(fixture.file(SELECTED)).unwrap(),
        completed_rollout(SELECTED).as_bytes()
    );
    fixture.assert_retained_rollout_unchanged();
    assert!(fixture.quarantines().is_empty());
}

#[test]
fn error_after_index_replacement_restores_the_exact_previous_index() {
    let fixture = Fixture::new();
    let rows = fixture.database_rows();
    let index = fs::read(fixture.paths.session_index()).unwrap();
    let preview = preview_delete(&fixture.paths, SELECTED).unwrap();
    let _hook = MutationHook::install(|stage, _| {
        if stage == "index_replaced" {
            anyhow::bail!("injected failure after index replacement");
        }
        Ok(())
    });

    let error = execute_delete(&fixture.paths, SELECTED, &preview.fingerprint).unwrap_err();

    assert!(format!("{error:#}").contains("injected failure after index replacement"));
    assert_eq!(fixture.database_rows(), rows);
    assert_eq!(fs::read(fixture.paths.session_index()).unwrap(), index);
    assert_eq!(
        fs::read(fixture.file(SELECTED)).unwrap(),
        completed_rollout(SELECTED).as_bytes()
    );
    fixture.assert_retained_rollout_unchanged();
    assert!(fixture.quarantines().is_empty());
}

#[test]
fn recreated_source_is_preserved_with_original_data_left_in_quarantine() {
    let fixture = Fixture::new();
    let rows = fixture.database_rows();
    let original_index = fs::read(fixture.paths.session_index()).unwrap();
    let preview = preview_delete(&fixture.paths, SELECTED).unwrap();
    let source = fixture.file(SELECTED);
    let replacement = completed_rollout("fixture-replacement");
    let new_bytes = replacement.clone();
    let quarantine_seen = Rc::new(RefCell::new(None));
    let captured = quarantine_seen.clone();
    let _hook = MutationHook::install(move |stage, quarantine| {
        if stage == "quarantined" {
            fs::write(&source, &new_bytes)?;
            *captured.borrow_mut() = Some(quarantine.to_path_buf());
            anyhow::bail!("injected failure after source recreation");
        }
        Ok(())
    });

    let error = execute_delete(&fixture.paths, SELECTED, &preview.fingerprint).unwrap_err();

    assert!(error.to_string().contains("可恢复数据保留"));
    assert_eq!(fixture.database_rows(), rows);
    assert_eq!(
        fs::read(fixture.file(SELECTED)).unwrap(),
        replacement.as_bytes()
    );
    assert_eq!(
        fs::read(fixture.paths.session_index()).unwrap(),
        original_index
    );
    let quarantine = quarantine_seen.borrow().clone().unwrap();
    assert_eq!(fixture.quarantines(), vec![quarantine.clone()]);
    assert_eq!(
        fs::read(quarantine.join("rollout-0.jsonl")).unwrap(),
        completed_rollout(SELECTED).as_bytes()
    );
    assert_eq!(
        fs::read(quarantine.join("index.before")).unwrap(),
        original_index
    );
    fixture.assert_retained_rollout_unchanged();
}

#[test]
fn concurrent_index_change_is_preserved_after_replacement_failure() {
    let fixture = Fixture::new();
    let rows = fixture.database_rows();
    let original_index = fs::read(fixture.paths.session_index()).unwrap();
    let preview = preview_delete(&fixture.paths, SELECTED).unwrap();
    let index_path = fixture.paths.session_index();
    let concurrent_index = format!(
        "{}\n{}\n",
        json!({"id":RETAINED,"thread_name":RETAINED}),
        json!({"id":"fixture-new-thread","thread_name":"fixture new thread"}),
    );
    let new_bytes = concurrent_index.clone();
    let _hook = MutationHook::install(move |stage, _| {
        if stage == "index_replaced" {
            fs::write(&index_path, &new_bytes)?;
            anyhow::bail!("injected failure after concurrent index change");
        }
        Ok(())
    });

    let error = execute_delete(&fixture.paths, SELECTED, &preview.fingerprint).unwrap_err();

    assert!(error.to_string().contains("可恢复数据保留"));
    assert_eq!(fixture.database_rows(), rows);
    assert_eq!(
        fs::read(fixture.paths.session_index()).unwrap(),
        concurrent_index.as_bytes()
    );
    assert_eq!(
        fs::read(fixture.file(SELECTED)).unwrap(),
        completed_rollout(SELECTED).as_bytes()
    );
    let quarantines = fixture.quarantines();
    assert_eq!(quarantines.len(), 1);
    assert_eq!(
        fs::read(quarantines[0].join("index.before")).unwrap(),
        original_index
    );
    fixture.assert_retained_rollout_unchanged();
}

#[test]
fn deferred_foreign_key_commit_failure_restores_all_native_rows_and_files() {
    let fixture = Fixture::new();
    let conn = open(&fixture.paths, true).unwrap();
    let foreign_keys: bool = conn
        .pragma_query_value(None, "foreign_keys", |r| r.get(0))
        .unwrap();
    assert!(
        foreign_keys,
        "the deletion connection must enforce foreign keys"
    );
    conn.execute_batch(
        "CREATE TABLE fixture_deferred_reference(
            id TEXT PRIMARY KEY,
            thread_id TEXT NOT NULL REFERENCES threads(id)
                DEFERRABLE INITIALLY DEFERRED);",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO fixture_deferred_reference VALUES('fixture-reference',?1)",
        [SELECTED],
    )
    .unwrap();
    drop(conn);
    let rows = fixture.database_rows();
    let index = fs::read(fixture.paths.session_index()).unwrap();
    let preview = preview_delete(&fixture.paths, SELECTED).unwrap();
    let reached = Rc::new(RefCell::new(Vec::new()));
    let stages = reached.clone();
    let _hook = MutationHook::install(move |stage, _| {
        stages.borrow_mut().push(stage.to_string());
        Ok(())
    });

    let error = execute_delete(&fixture.paths, SELECTED, &preview.fingerprint).unwrap_err();

    assert!(reached
        .borrow()
        .iter()
        .any(|stage| stage == "before_commit"));
    assert!(error.chain().any(|cause| matches!(
        cause.downcast_ref::<rusqlite::Error>(),
        Some(rusqlite::Error::SqliteFailure(code, _))
            if code.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY
    )));
    assert_eq!(fixture.database_rows(), rows);
    assert_eq!(fs::read(fixture.paths.session_index()).unwrap(), index);
    assert_eq!(
        fs::read(fixture.file(SELECTED)).unwrap(),
        completed_rollout(SELECTED).as_bytes()
    );
    assert_eq!(
        fixture
            .conn()
            .query_row(
                "SELECT thread_id FROM fixture_deferred_reference WHERE id='fixture-reference'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
        SELECTED
    );
    fixture.assert_retained_rollout_unchanged();
    assert!(fixture.quarantines().is_empty());
}

#[test]
fn process_exit_keeps_recoverable_files_and_rolls_back_on_database_reopen() {
    if let Ok(stage) = std::env::var(CHILD_STAGE) {
        assert!(matches!(
            stage.as_str(),
            "quarantined" | "index_replaced" | "before_commit"
        ));
        let token = std::env::var(CHILD_TOKEN).unwrap();
        assert_eq!(uuid::Uuid::parse_str(&token).unwrap().to_string(), token);
        let root = std::env::temp_dir().join(format!("{FIXTURE_PREFIX}{token}"));
        assert!(!fs::symlink_metadata(&root)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            fs::read_to_string(root.join("fixture-token")).unwrap(),
            token
        );
        let paths = CodexPaths::new(root.join("codex"));
        assert!(!fs::symlink_metadata(&paths.home)
            .unwrap()
            .file_type()
            .is_symlink());
        let preview = preview_delete(&paths, SELECTED).unwrap();
        let _hook = MutationHook::install(move |label, _| {
            if label == stage {
                // Unlike a panic, exit does not run the transaction or fixture drops.
                std::process::exit(86);
            }
            Ok(())
        });
        let result = execute_delete(&paths, SELECTED, &preview.fingerprint);
        panic!("the child did not reach its exit hook: {}", result.is_ok());
    }

    for stage in ["quarantined", "index_replaced", "before_commit"] {
        let fixture = Fixture::new();
        let rows = fixture.database_rows();
        let old_index = fs::read(fixture.paths.session_index()).unwrap();
        let selected_row = row_value(&fixture.conn(), "threads", "id", SELECTED)
            .unwrap()
            .unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "selected_codex::maintenance_tests::process_exit_keeps_recoverable_files_and_rolls_back_on_database_reopen",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD_STAGE, stage)
            .env(CHILD_TOKEN, &fixture.token)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(86),
            "child failed to exit at {stage}: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Opening a fresh connection must roll back the interrupted SQLite
        // transaction. It does not restore filesystem or index state.
        assert_eq!(fixture.database_rows(), rows, "exit stage: {stage}");
        assert!(!fixture.file(SELECTED).exists());
        fixture.assert_retained_rollout_unchanged();
        let quarantines = fixture.quarantines();
        assert_eq!(quarantines.len(), 1);
        let quarantine = &quarantines[0];
        assert_eq!(
            fs::read(quarantine.join("rollout-0.jsonl")).unwrap(),
            completed_rollout(SELECTED).as_bytes()
        );
        assert_eq!(
            fs::read(quarantine.join("index.before")).unwrap(),
            old_index
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(quarantine.join("thread.json")).unwrap())
                .unwrap(),
            selected_row
        );
        let manifest: SelectedCodexPreview =
            serde_json::from_slice(&fs::read(quarantine.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest.id, SELECTED);
        assert_eq!(manifest.paths, vec![fixture.file(SELECTED)]);
        if stage == "quarantined" {
            assert_eq!(fs::read(fixture.paths.session_index()).unwrap(), old_index);
        } else {
            assert_eq!(
                fs::read_to_string(fixture.paths.session_index()).unwrap(),
                json!({"id":RETAINED,"thread_name":RETAINED}).to_string() + "\n"
            );
        }
    }
}
