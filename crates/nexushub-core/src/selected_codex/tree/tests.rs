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
    fn compress(&self, path: &Path, keep_plain: bool) -> PathBuf {
        let compressed = rollout_file::compressed_path(path);
        fs::write(
            &compressed,
            zstd::stream::encode_all(fs::read(path).unwrap().as_slice(), 0).unwrap(),
        )
        .unwrap();
        if !keep_plain {
            fs::remove_file(path).unwrap();
        }
        compressed
    }
    fn sidecars(&self, id: &str) -> Vec<PathBuf> {
        let lock = self
            .paths
            .home
            .join(format!("thread-writer-locks/{id}.lock"));
        let snapshot = self.paths.home.join(format!("shell_snapshots/{id}.sh"));
        let visualization = self
            .paths
            .home
            .join(format!("visualizations/2026/01/01/{id}"));
        fs::create_dir_all(snapshot.parent().unwrap()).unwrap();
        fs::create_dir_all(visualization.join("assets")).unwrap();
        fs::write(&lock, "").unwrap();
        fs::write(&snapshot, "synthetic snapshot").unwrap();
        fs::write(visualization.join("assets/page.html"), "synthetic visual").unwrap();
        vec![lock, snapshot, visualization]
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
fn invalid_compression_hardlinked_corrupt_and_partial_files_are_protected() {
    for damage in ["invalid_compression", "hardlink", "corrupt", "partial"] {
        let f = Fixture::new();
        f.add(ROOT, None, true);
        let path = f.file(ROOT, None);
        match damage {
            "invalid_compression" => {
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
fn compressed_subtrees_and_siblings_are_fully_verified_and_removed() {
    for keep_plain in [false, true] {
        let f = Fixture::new();
        f.add(ROOT, None, true);
        f.add(CHILD, Some(ROOT), true);
        let archived = f.paths.home.join("archived_sessions");
        fs::create_dir(&archived).unwrap();
        let branch = archived.join(f.file(CHILD, Some(ROLLOUT)).file_name().unwrap());
        f.write(&branch, CHILD, Some(ROOT));
        f.compress(&branch, keep_plain);
        f.compress(&f.file(ROOT, None), keep_plain);
        f.history(ROLLOUT);
        let plan = preview(&f.paths, ROOT, true).unwrap();
        let physical_bytes: u64 = plan
            .files
            .keys()
            .filter_map(|p| fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();
        assert_eq!(plan.selected.bytes, physical_bytes);
        assert!(!plan.legacy);
        assert!(plan.rollouts.contains(ROLLOUT));
        f.native(remove_native);
        let outcome = execute(&f.paths, ROOT, true, &plan.selected.fingerprint).unwrap();
        assert!(outcome.error.is_none(), "{outcome:?}");
        assert_eq!(outcome.deleted_ids, [ROOT, CHILD]);
    }
}

#[test]
fn unrelated_bad_headers_warn_without_binding_the_target_or_breaking_verification() {
    let f = Fixture::new();
    f.add(ROOT, None, true);
    let clean = preview(&f.paths, ROOT, true).unwrap();
    f.write(&f.file(OTHER, None), OTHER, None);
    let compressed = f.compress(&f.file(OTHER, None), false);
    assert!(preview(&f.paths, ROOT, true).unwrap().warnings.is_empty());
    fs::write(&compressed, b"broken compressed history").unwrap();
    let dirty = preview(&f.paths, ROOT, true).unwrap();
    assert_eq!(clean.selected.fingerprint, dirty.selected.fingerprint);
    assert!(dirty.warnings.iter().any(|w| w.contains("1 份")));
    assert!(!dirty.legacy);
    f.native(remove_native);
    let result = execute(&f.paths, ROOT, true, &dirty.selected.fingerprint).unwrap();
    assert!(!result.storage_failure);
    assert!(result.error.is_none());
    assert!(compressed.exists());
}

#[test]
fn external_compressed_references_compare_rollout_ids_and_prefer_plaintext() {
    let f = Fixture::new();
    f.add(ROOT, None, true);
    f.write(&f.file(ROOT, Some(ROLLOUT)), ROOT, Some(ROOT));
    f.write(&f.file(OTHER, None), OTHER, Some(ROLLOUT));
    f.compress(&f.file(OTHER, None), false);
    assert!(preview(&f.paths, ROOT, true)
        .unwrap_err()
        .to_string()
        .contains("引用"));
    f.write(&f.file(OTHER, None), OTHER, None);
    assert!(preview(&f.paths, ROOT, true).is_ok());
}

#[test]
fn conversion_and_each_physical_sibling_invalidate_the_preview() {
    let f = Fixture::new();
    f.add(ROOT, None, true);
    let plan = preview(&f.paths, ROOT, true).unwrap();
    let compressed = f.compress(&f.file(ROOT, None), false);
    assert!(execute(&f.paths, ROOT, true, &plan.selected.fingerprint).is_err());
    f.write(&f.file(ROOT, None), ROOT, None);
    let plan = preview(&f.paths, ROOT, true).unwrap();
    fs::write(
        &compressed,
        zstd::stream::encode_all(
            b"{\"type\":\"session_meta\",\"payload\":{\"id\":\"wrong\"}}\n".as_slice(),
            0,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(execute(&f.paths, ROOT, true, &plan.selected.fingerprint).is_err());
}

#[test]
fn target_identity_collisions_and_duplicate_rollouts_are_rejected() {
    for conflict in ["header", "duplicate", "database"] {
        let f = Fixture::new();
        f.add(ROOT, None, true);
        match conflict {
            "header" => f.write(&f.file(ROOT, None), OTHER, None),
            "duplicate" => f.write(&f.file(OTHER, Some(ROOT)), OTHER, None),
            _ => {
                f.write(&f.file(OTHER, None), OTHER, None);
                f.conn()
                    .execute(
                        "UPDATE threads SET rollout_path=?1 WHERE id=?2",
                        rusqlite::params![f.file(OTHER, None).to_string_lossy(), ROOT],
                    )
                    .unwrap();
            }
        }
        assert!(preview(&f.paths, ROOT, true).is_err(), "{conflict}");
    }
}

#[test]
fn verification_finds_corrupt_new_target_and_compressed_sibling_residuals() {
    for leave_sibling in [false, true] {
        let f = Fixture::new();
        f.add(ROOT, None, true);
        f.native(move |paths, plan| {
            let path = if leave_sibling {
                rollout_file::compressed_path(&plan.selected.paths[0])
            } else {
                paths.home.join("archived_sessions").join(format!(
                    "rollout-2026-01-02T00-00-00-{ROOT}_{ROLLOUT}.jsonl"
                ))
            };
            remove_native(paths, plan)?;
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, b"corrupt residual")?;
            Ok(())
        });
        let plan = preview(&f.paths, ROOT, true).unwrap();
        let result = execute(&f.paths, ROOT, true, &plan.selected.fingerprint).unwrap();
        assert!(result.error.is_some());
        assert!(!result.storage_failure);
        assert_eq!(result.residuals.len(), 1);
    }
}

#[test]
fn compressed_questions_use_native_reply_semantics() {
    use std::io::Write;
    let f = Fixture::new();
    f.add(ROOT, None, true);
    let question = json!({"type":"response_item","turn_id":"fixture-turn","payload":{"type":"function_call","name":"request_user_input","call_id":"fixture-call","arguments":{"questions":[{"id":"choice","question":"Choose"}]}}});
    writeln!(
        fs::OpenOptions::new()
            .append(true)
            .open(f.file(ROOT, None))
            .unwrap(),
        "{question}"
    )
    .unwrap();
    f.compress(&f.file(ROOT, None), false);
    assert!(preview(&f.paths, ROOT, true)
        .unwrap()
        .warnings
        .iter()
        .any(|w| w.contains("待回复")));
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
fn sidecars_are_previewed_and_cleaned_for_the_whole_confirmed_tree() {
    let f = Fixture::new();
    f.add(ROOT, None, true);
    f.add(CHILD, Some(ROOT), true);
    f.add(OTHER, None, true);
    let selected: Vec<_> = f
        .sidecars(ROOT)
        .into_iter()
        .chain(f.sidecars(CHILD))
        .collect();
    let retained = f.sidecars(OTHER);
    let similar = f
        .paths
        .home
        .join(format!("shell_snapshots/{ROOT}-other.sh"));
    fs::write(&similar, "retained").unwrap();
    let plan = preview(&f.paths, ROOT, true).unwrap();
    assert!(selected
        .iter()
        .all(|path| plan.selected.paths.contains(path)));
    assert!(retained
        .iter()
        .all(|path| !plan.selected.paths.contains(path)));
    let bytes: u64 = plan
        .selected
        .paths
        .iter()
        .filter_map(|path| fs::metadata(path).ok())
        .filter(|meta| meta.is_file())
        .map(|meta| meta.len())
        .sum();
    assert_eq!(plan.selected.bytes, bytes);
    f.native(remove_native);
    let result = execute(&f.paths, ROOT, true, &plan.selected.fingerprint).unwrap();
    assert!(result.error.is_none(), "{result:?}");
    assert!(result.residuals.is_empty());
    assert_eq!(result.deleted_ids, [ROOT, CHILD]);
    assert!(selected.iter().all(|path| !path.exists()));
    assert!(retained.iter().all(|path| path.exists()));
    assert!(similar.exists());
    assert!(f
        .paths
        .home
        .join("thread-writer-locks/.coordination.lock")
        .exists());
    assert!(f.paths.home.join("visualizations/2026/01/01").is_dir());
}

#[test]
fn sidecar_changes_require_a_new_preview_before_any_official_request() {
    for change in ["modify", "new"] {
        let f = Fixture::new();
        f.add(ROOT, None, true);
        let files = f.sidecars(ROOT);
        let plan = preview(&f.paths, ROOT, true).unwrap();
        let target = if change == "modify" {
            files[1].clone()
        } else {
            files[1].with_extension("zsh")
        };
        fs::write(target, "changed snapshot").unwrap();
        f.native(|_, _| panic!("must not submit an invalidated preview"));
        assert!(execute(&f.paths, ROOT, true, &plan.selected.fingerprint).is_err());
        assert!(f.file(ROOT, None).exists());
    }
}

#[test]
fn batch_sidecar_failure_is_not_succeeded_and_does_not_block_other_items() {
    use crate::services::sessions::*;
    let f = Fixture::new();
    for id in [ROOT, OTHER] {
        f.add(id, None, true);
        f.sidecars(id);
    }
    let api = SessionUseCases {
        platform: crate::platform::PlatformPaths::for_kind(crate::platform::PlatformKind::Macos),
        codex: f.paths.clone(),
        grok: crate::grok::GrokPaths {
            home: f.paths.home.join("unused-grok"),
        },
        claude: crate::claude::ClaudePaths {
            config: f.paths.home.join("unused-claude"),
            projects: f.paths.home.join("unused-claude/projects"),
        },
    };
    let plan = api
        .bulk_preview(SessionBatchRequest {
            provider: SessionProvider::Codex,
            operation: SessionOperation::Delete,
            session_keys: vec![ROOT.into(), OTHER.into()],
            include_descendants: true,
        })
        .unwrap();
    assert!(plan.items.iter().all(|item| item.allowed));
    f.native(|paths, preview| {
        remove_native(paths, preview)?;
        if preview.selected.id == ROOT {
            fs::write(
                paths.home.join(format!("shell_snapshots/{ROOT}.sh")),
                "changed after request",
            )?;
        }
        Ok(())
    });
    let result = api
        .bulk_execute(SessionBatchExecuteRequest {
            provider: SessionProvider::Codex,
            operation: SessionOperation::Delete,
            confirmed: true,
            include_descendants: true,
            items: plan
                .items
                .into_iter()
                .map(|item| SessionBatchSelection {
                    session_key: item.session_key,
                    fingerprint: item.fingerprint.unwrap(),
                })
                .collect(),
        })
        .unwrap();
    assert_eq!(result.items[0].status, "failed");
    assert_eq!(result.items[0].deleted_ids, [ROOT]);
    assert!(result.items[0]
        .residuals
        .iter()
        .any(|path| path.ends_with(&format!("{ROOT}.sh"))));
    assert_eq!(result.items[1].status, "succeeded");
    assert!(result.items[1].residuals.is_empty());
}

#[test]
fn deep_visualizations_fail_closed_without_truncating_the_inventory() {
    let f = Fixture::new();
    f.add(ROOT, None, true);
    let mut path = f.paths.home.join("visualizations");
    for _ in 0..34 {
        path = path.join("nested");
    }
    fs::create_dir_all(path.join(ROOT)).unwrap();
    assert!(preview(&f.paths, ROOT, true)
        .unwrap_err()
        .to_string()
        .contains("上限"));
    assert!(f.file(ROOT, None).exists());
}

#[test]
fn official_failure_or_primary_residuals_never_clean_sidecars() {
    for failure in ["rejected", "partial", "residual", "unverifiable"] {
        let f = Fixture::new();
        f.add(ROOT, None, true);
        let sidecars = f.sidecars(ROOT);
        f.native(move |paths, preview| {
            match failure {
                "rejected" => anyhow::bail!("synthetic official failure"),
                "partial" => {
                    remove_native(paths, preview)?;
                    anyhow::bail!("synthetic unknown result")
                }
                "unverifiable" => {
                    remove_native(paths, preview)?;
                    fs::write(paths.state_db(), "unreadable native database")?;
                }
                _ => {}
            }
            Ok(())
        });
        let plan = preview(&f.paths, ROOT, true).unwrap();
        let result = execute(&f.paths, ROOT, true, &plan.selected.fingerprint).unwrap();
        assert!(result.error.is_some(), "{failure}: {result:?}");
        assert!(sidecars.iter().all(|path| path.exists()));
        assert!(sidecars
            .iter()
            .all(|path| result.residuals.contains(&path.display().to_string())));
        if failure == "rejected" || failure == "residual" {
            assert!(f.file(ROOT, None).exists());
        }
    }
}

#[test]
fn changed_and_new_sidecars_after_official_success_remain_failed_residuals() {
    for change in ["modify", "new"] {
        let f = Fixture::new();
        f.add(ROOT, None, true);
        let files = f.sidecars(ROOT);
        let target = if change == "modify" {
            files[1].clone()
        } else {
            files[1].with_extension("zsh")
        };
        let changed = target.clone();
        f.native(move |paths, preview| {
            remove_native(paths, preview)?;
            fs::write(&changed, "preserve post-request content")?;
            Ok(())
        });
        let plan = preview(&f.paths, ROOT, true).unwrap();
        let result = execute(&f.paths, ROOT, true, &plan.selected.fingerprint).unwrap();
        assert!(result.error.is_some());
        assert_eq!(result.deleted_ids, [ROOT]);
        assert!(result.residuals.contains(&target.display().to_string()));
        assert_eq!(
            fs::read_to_string(target).unwrap(),
            "preserve post-request content"
        );
        assert!(!files[0].exists());
        assert!(!files[2].exists());
    }
}

#[test]
fn writer_reappearing_after_official_success_protects_all_sidecars() {
    let f = Fixture::new();
    f.add(ROOT, None, true);
    let files = f.sidecars(ROOT);
    let lock_path = files[0].clone();
    let held_lock = Rc::new(std::cell::RefCell::new(None));
    let retained_lock = held_lock.clone();
    f.native(move |paths, preview| {
        remove_native(paths, preview)?;
        let lock = fs::File::options()
            .read(true)
            .write(true)
            .open(&lock_path)?;
        lock.lock()?;
        *retained_lock.borrow_mut() = Some(lock);
        Ok(())
    });
    let plan = preview(&f.paths, ROOT, true).unwrap();
    let result = execute(&f.paths, ROOT, true, &plan.selected.fingerprint).unwrap();
    assert!(result.error.unwrap().contains("活跃写入者"));
    assert_eq!(result.deleted_ids, [ROOT]);
    assert!(files.iter().all(|path| path.exists()));
    assert!(held_lock.borrow().is_some());
}

#[cfg(unix)]
#[test]
fn sidecar_links_and_replaced_ancestors_cannot_delete_outside_content() {
    use std::os::unix::fs::symlink;
    for replace_after_request in [false, true] {
        let f = Fixture::new();
        f.add(ROOT, None, true);
        let files = f.sidecars(ROOT);
        let outside = f.paths.home.join("retained-workspace");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("precious.txt"), "retained").unwrap();
        if !replace_after_request {
            symlink(&outside, files[2].join("linked")).unwrap();
            assert!(preview(&f.paths, ROOT, true).is_err());
        } else {
            let target = files[2].clone();
            let outside_copy = outside.clone();
            f.native(move |paths, preview| {
                remove_native(paths, preview)?;
                fs::rename(&target, target.with_extension("retained"))?;
                symlink(&outside_copy, &target)?;
                Ok(())
            });
            let plan = preview(&f.paths, ROOT, true).unwrap();
            let result = execute(&f.paths, ROOT, true, &plan.selected.fingerprint).unwrap();
            assert!(result.error.is_some());
            assert!(result.residuals.contains(&files[2].display().to_string()));
            assert!(fs::symlink_metadata(&files[2])
                .unwrap()
                .file_type()
                .is_symlink());
        }
        assert_eq!(
            fs::read_to_string(outside.join("precious.txt")).unwrap(),
            "retained"
        );
    }
}

#[test]
fn helper_unavailable_retains_safe_plaintext_fallback_but_never_compressed_fallback() {
    for compressed in [false, true] {
        let f = Fixture::new();
        f.add(ROOT, None, true);
        fs::remove_file(f.paths.home.join("thread-writer-locks/.coordination.lock")).unwrap();
        fs::remove_dir(f.paths.home.join("thread-writer-locks")).unwrap();
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(f.file(ROOT, None))
            .unwrap();
        use std::io::Write;
        writeln!(
            file,
            "{}",
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"fixture-turn"}})
        )
        .unwrap();
        drop(file);
        if compressed {
            f.compress(&f.file(ROOT, None), false);
        }
        let plan = preview(&f.paths, ROOT, true).unwrap();
        assert_eq!(plan.legacy, !compressed);
        let result = offline_delete(&f.paths, &plan);
        if compressed {
            assert!(result.is_err());
            assert!(f.file(ROOT, None).with_extension("jsonl.zst").exists());
        } else {
            assert!(!result.unwrap());
            assert!(!f.file(ROOT, None).exists());
            assert!(verify(&f.paths, &plan).unwrap().residuals.is_empty());
        }
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
