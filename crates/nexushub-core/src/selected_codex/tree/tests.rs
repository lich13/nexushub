use super::*;
use serde_json::json;
use std::{cell::Cell, rc::Rc};

const ROOT: &str = "11111111-1111-4111-8111-111111111111";
const CHILD: &str = "22222222-2222-4222-8222-222222222222";
const OTHER: &str = "33333333-3333-4333-8333-333333333333";
const ROLLOUT: &str = "44444444-4444-4444-8444-444444444444";

struct Fixture {
    paths: CodexPaths,
}
impl Fixture {
    fn new() -> Self {
        let paths = CodexPaths::new(
            std::env::temp_dir().join(format!("nexushub-tree-{}", uuid::Uuid::new_v4())),
        );
        fs::create_dir_all(paths.sessions_dir()).unwrap();
        fs::create_dir(paths.home.join("thread-writer-locks")).unwrap();
        fs::write(
            paths.home.join("thread-writer-locks/.coordination.lock"),
            "",
        )
        .unwrap();
        let fixture = Self { paths };
        fixture.conn().execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY, title TEXT, source TEXT, rollout_path TEXT, archived INTEGER, parent_thread_id TEXT); CREATE TABLE thread_spawn_edges(parent_thread_id TEXT, child_thread_id TEXT); CREATE TABLE thread_attachments(thread_id TEXT, attachment_id TEXT);").unwrap();
        TEST_DELETE.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(|_, _| anyhow::bail!("synthetic native rejection")))
        });
        fixture
    }
    fn conn(&self) -> Connection {
        Connection::open(self.paths.state_db()).unwrap()
    }
    fn file(&self, id: &str, rollout: Option<&str>) -> PathBuf {
        self.paths.sessions_dir().join(format!(
            "rollout-2026-01-01T00-00-00-{id}{}.jsonl",
            rollout.map(|r| format!("_{r}")).unwrap_or_default()
        ))
    }
    fn add(&self, id: &str, parent: Option<&str>, with_file: bool) {
        let path = self.file(id, None);
        if with_file {
            self.write(&path, id, None);
        }
        self.conn()
            .execute(
                "INSERT INTO threads VALUES(?1,'Fixture','cli',?2,0,?3)",
                rusqlite::params![
                    id,
                    with_file.then(|| path.to_string_lossy().into_owned()),
                    parent
                ],
            )
            .unwrap();
        if let Some(parent) = parent {
            self.conn()
                .execute("INSERT INTO thread_spawn_edges VALUES(?1,?2)", [parent, id])
                .unwrap();
        }
    }
    fn write(&self, path: &Path, id: &str, base: Option<&str>) {
        fs::write(path, format!("{}\n", json!({"type":"session_meta","payload":{"id":id,"history_base":base.map(|b| json!({"thread_id":b,"turn_id":"fixture-turn"}))}}))).unwrap();
    }
    fn history(&self, key: &str) {
        let conn = Connection::open(self.paths.home.join("thread_history_1.sqlite")).unwrap();
        conn.execute_batch("CREATE TABLE IF NOT EXISTS thread_items(thread_id TEXT, item_id TEXT); CREATE TABLE IF NOT EXISTS thread_turns(thread_id TEXT, turn_id TEXT); CREATE TABLE IF NOT EXISTS thread_history_projection_state(thread_id TEXT);").unwrap();
        conn.execute("INSERT INTO thread_items VALUES(?1,'fixture-item')", [key])
            .unwrap();
    }
    fn native(&self, delete: impl FnMut(&CodexPaths, &TreePreview) -> Result<()> + 'static) {
        TEST_DELETE.with(|slot| *slot.borrow_mut() = Some(Box::new(delete)));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        TEST_DELETE.with(|slot| *slot.borrow_mut() = None);
        fs::remove_dir_all(&self.paths.home).unwrap();
    }
}

fn remove_native(paths: &CodexPaths, preview: &TreePreview) -> Result<()> {
    for path in preview.files.keys() {
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    let conn = open(paths, true)?;
    for id in &preview.ids {
        conn.execute(
            "DELETE FROM thread_spawn_edges WHERE parent_thread_id=?1 OR child_thread_id=?1",
            [id],
        )?;
        conn.execute("DELETE FROM thread_attachments WHERE thread_id=?1", [id])?;
        conn.execute("DELETE FROM threads WHERE id=?1", [id])?;
    }
    if paths.home.join("thread_history_1.sqlite").exists() {
        let history = Connection::open(paths.home.join("thread_history_1.sqlite"))?;
        for id in &preview.rollouts {
            history.execute("DELETE FROM thread_items WHERE thread_id=?1", [id])?;
        }
    }
    fs::write(paths.session_index(), "")?;
    Ok(())
}

#[test]
fn metadata_only_empty_and_header_only_threads_can_be_deleted() {
    for with_file in [false, true] {
        let f = Fixture::new();
        f.add(ROOT, None, with_file);
        f.native(remove_native);
        let plan = preview(&f.paths, ROOT, true).unwrap();
        assert_eq!(plan.selected.paths.len(), usize::from(with_file));
        let result = execute(&f.paths, ROOT, true, &plan.selected.fingerprint).unwrap();
        assert!(result.error.is_none(), "{result:?}");
        assert_eq!(result.deleted_ids, [ROOT]);
    }
}

#[test]
fn missing_rollouts_still_reject_symlinks_and_parent_traversal() {
    for link in [false, true] {
        let f = Fixture::new();
        f.add(ROOT, None, false);
        let path = if link {
            let link = f.paths.sessions_dir().join("alias");
            #[cfg(unix)]
            std::os::unix::fs::symlink(f.paths.home.join("missing"), &link).unwrap();
            link.join(f.file(ROOT, None).file_name().unwrap())
        } else {
            f.paths
                .sessions_dir()
                .join("../missing")
                .join(f.file(ROOT, None).file_name().unwrap())
        };
        f.conn()
            .execute(
                "UPDATE threads SET rollout_path=?1 WHERE id=?2",
                rusqlite::params![path.to_string_lossy(), ROOT],
            )
            .unwrap();
        assert!(preview(&f.paths, ROOT, true).is_err());
    }
}

#[test]
fn inactive_waiting_threads_warn_and_delete_in_both_locations() {
    use std::io::Write;
    for archived in [false, true] {
        let f = Fixture::new();
        f.add(ROOT, None, true);
        let mut path = f.file(ROOT, None);
        let question = json!({"type":"response_item","turn_id":"fixture-turn","payload":{"type":"function_call","name":"request_user_input","call_id":"fixture-call","arguments":{"questions":[{"id":"choice","question":"Choose a fixture option"}]}}});
        writeln!(
            fs::OpenOptions::new().append(true).open(&path).unwrap(),
            "{question}"
        )
        .unwrap();
        if archived {
            let dir = f.paths.home.join("archived_sessions");
            fs::create_dir(&dir).unwrap();
            let archived_path = dir.join(path.file_name().unwrap());
            fs::rename(&path, &archived_path).unwrap();
            path = archived_path;
            f.conn()
                .execute(
                    "UPDATE threads SET archived=1,rollout_path=?1 WHERE id=?2",
                    rusqlite::params![path.to_string_lossy(), ROOT],
                )
                .unwrap();
        }
        f.native(remove_native);
        let plan = preview(&f.paths, ROOT, true).unwrap();
        assert!(plan.warnings.iter().any(|w| w.contains("待回复")));
        let result = execute(&f.paths, ROOT, true, &plan.selected.fingerprint).unwrap();
        assert!(result.error.is_none(), "{result:?}");
        assert!(!path.exists());
    }
}

#[test]
fn trees_beyond_the_existing_depth_budget_are_rejected_without_truncation() {
    let f = Fixture::new();
    f.add(ROOT, None, false);
    let mut parent = ROOT.to_string();
    for n in 1..=33 {
        let id = format!("55555555-5555-4555-8555-{n:012}");
        f.add(&id, Some(&parent), false);
        parent = id;
    }
    assert!(preview(&f.paths, ROOT, true).is_err());
    assert_eq!(
        f.conn()
            .query_row("SELECT count(*) FROM threads", [], |r| r.get::<_, usize>(0))
            .unwrap(),
        34
    );
}

#[test]
fn explicit_tree_includes_archived_rollout_variants_and_paged_history() {
    let f = Fixture::new();
    f.add(ROOT, None, true);
    f.add(CHILD, Some(ROOT), true);
    f.add(OTHER, None, true);
    let alternate = f.file(ROOT, Some(ROLLOUT));
    f.write(&alternate, ROOT, Some(ROOT));
    let archived = f.paths.home.join("archived_sessions");
    fs::create_dir(&archived).unwrap();
    fs::rename(&alternate, archived.join(alternate.file_name().unwrap())).unwrap();
    f.history(ROLLOUT);
    f.conn()
        .execute("UPDATE threads SET archived=1 WHERE id=?1", [ROOT])
        .unwrap();
    assert!(preview(&f.paths, ROOT, false)
        .unwrap_err()
        .to_string()
        .contains("后代"));
    let plan = preview(&f.paths, ROOT, true).unwrap();
    assert_eq!(plan.ids, [ROOT, CHILD]);
    assert_eq!(plan.selected.paths.len(), 3);
    f.native(remove_native);
    let result = execute(&f.paths, ROOT, true, &plan.selected.fingerprint).unwrap();
    assert!(result.error.is_none(), "{result:?}");
    assert_eq!(result.deleted_ids, [ROOT, CHILD]);
    assert!(f.file(OTHER, None).exists());
}

#[test]
fn preview_is_invalidated_by_a_new_descendant_or_reference() {
    let f = Fixture::new();
    f.add(ROOT, None, true);
    let plan = preview(&f.paths, ROOT, true).unwrap();
    f.add(CHILD, Some(ROOT), true);
    assert!(execute(&f.paths, ROOT, true, &plan.selected.fingerprint).is_err());
    f.add(OTHER, None, true);
    f.write(&f.file(OTHER, None), OTHER, Some(ROOT));
    assert!(preview(&f.paths, ROOT, true)
        .unwrap_err()
        .to_string()
        .contains("引用"));
    assert!(f.file(ROOT, None).exists());
}

#[test]
fn compressed_hardlinked_corrupt_and_partial_files_are_protected() {
    for damage in ["compressed", "hardlink", "corrupt", "partial"] {
        let f = Fixture::new();
        f.add(ROOT, None, true);
        let path = f.file(ROOT, None);
        match damage {
            "compressed" => {
                fs::rename(&path, path.with_extension("jsonl.zst")).unwrap();
            }
            "hardlink" => {
                fs::hard_link(&path, f.paths.home.join("other-link")).unwrap();
            }
            "corrupt" => {
                fs::write(&path, "invalid\n").unwrap();
            }
            _ => {
                use std::io::Write;
                fs::OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .unwrap()
                    .write_all(b"{")
                    .unwrap();
            }
        }
        assert!(preview(&f.paths, ROOT, true).is_err(), "{damage}");
    }
}

#[test]
fn active_native_writer_and_ambiguous_graphs_are_blocked() {
    let f = Fixture::new();
    f.add(ROOT, None, true);
    let lock = fs::File::create(
        f.paths
            .home
            .join(format!("thread-writer-locks/{ROOT}.lock")),
    )
    .unwrap();
    lock.lock().unwrap();
    assert!(
        preview(&f.paths, ROOT, true)
            .unwrap_err()
            .to_string()
            .contains("占用")
            || preview(&f.paths, ROOT, true)
                .unwrap_err()
                .to_string()
                .contains("活跃")
    );
    drop(lock);
    f.add(CHILD, Some(ROOT), true);
    f.add(OTHER, None, true);
    f.conn()
        .execute(
            "INSERT INTO thread_spawn_edges VALUES(?1,?2)",
            [OTHER, CHILD],
        )
        .unwrap();
    assert!(preview(&f.paths, ROOT, true)
        .unwrap_err()
        .to_string()
        .contains("多个"));
}

#[test]
fn missing_edges_and_cycles_do_not_become_partial_previews() {
    for cycle in [false, true] {
        let f = Fixture::new();
        f.add(ROOT, None, true);
        if cycle {
            f.add(CHILD, Some(ROOT), true);
            f.conn()
                .execute(
                    "INSERT INTO thread_spawn_edges VALUES(?1,?2)",
                    [CHILD, ROOT],
                )
                .unwrap();
        } else {
            f.conn()
                .execute(
                    "INSERT INTO thread_spawn_edges VALUES(?1,?2)",
                    [ROOT, CHILD],
                )
                .unwrap();
        }
        assert!(preview(&f.paths, ROOT, true).is_err());
    }
}

#[test]
fn success_with_residual_paged_rows_is_reported_as_failure() {
    let f = Fixture::new();
    f.add(ROOT, None, true);
    f.history(ROOT);
    f.native(|paths, plan| {
        fs::remove_file(&plan.selected.paths[0])?;
        open(paths, true)?.execute("DELETE FROM threads", [])?;
        Ok(())
    });
    let plan = preview(&f.paths, ROOT, true).unwrap();
    let result = execute(&f.paths, ROOT, true, &plan.selected.fingerprint).unwrap();
    assert!(result.error.is_some());
    assert!(result.residuals.iter().any(|r| r.contains("thread_items")));
}

#[test]
fn a_successful_native_request_with_unverifiable_storage_never_reports_success() {
    let f = Fixture::new();
    f.add(ROOT, None, true);
    let plan = preview(&f.paths, ROOT, true).unwrap();
    f.native(|paths, preview| {
        remove_native(paths, preview)?;
        fs::write(paths.session_index(), "{incomplete")?;
        Ok(())
    });
    let result = execute(&f.paths, ROOT, true, &plan.selected.fingerprint).unwrap();
    assert!(result.storage_failure);
    assert!(result
        .error
        .unwrap()
        .contains("无法完整核验文件、原生索引和分页历史"));
    assert!(result.deleted_ids.is_empty());
}

#[test]
fn partial_native_failure_and_unknown_result_never_retry_or_fallback() {
    for partial in [false, true] {
        let f = Fixture::new();
        f.add(ROOT, None, true);
        f.add(CHILD, Some(ROOT), true);
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        f.native(move |paths, _| {
            observed.set(observed.get() + 1);
            if partial {
                fs::remove_file(
                    paths
                        .sessions_dir()
                        .join(format!("rollout-2026-01-01T00-00-00-{CHILD}.jsonl")),
                )?;
                open(paths, true)?.execute("DELETE FROM threads WHERE id=?1", [CHILD])?;
            }
            anyhow::bail!("synthetic timeout or partial failure")
        });
        let plan = preview(&f.paths, ROOT, true).unwrap();
        let result = execute(&f.paths, ROOT, true, &plan.selected.fingerprint).unwrap();
        assert!(result.error.is_some());
        assert_eq!(calls.get(), 1);
        assert!(f.file(ROOT, None).exists());
    }
}

#[test]
fn unknown_history_schema_is_not_deleted() {
    let f = Fixture::new();
    f.add(ROOT, None, true);
    f.history(ROOT);
    Connection::open(f.paths.home.join("thread_history_1.sqlite"))
        .unwrap()
        .execute("CREATE TABLE future_history(identity TEXT)", [])
        .unwrap();
    assert!(preview(&f.paths, ROOT, true)
        .unwrap_err()
        .to_string()
        .contains("格式"));
}

#[test]
fn independent_batch_trees_keep_their_preview_after_another_tree_is_deleted() {
    let f = Fixture::new();
    f.add(ROOT, None, true);
    f.add(OTHER, None, true);
    let root = preview(&f.paths, ROOT, true).unwrap();
    let other = preview(&f.paths, OTHER, true).unwrap();
    f.native(remove_native);
    assert!(execute(&f.paths, ROOT, true, &root.selected.fingerprint)
        .unwrap()
        .error
        .is_none());
    assert!(execute(&f.paths, OTHER, true, &other.selected.fingerprint)
        .unwrap()
        .error
        .is_none());
}
