//! Narrow, recoverable deletion of one explicitly selected archived Codex thread.
use crate::codex::{rollout_has_running_signal, CodexPaths};
use anyhow::{ensure, Context, Result};
use rusqlite::{types::ValueRef, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    sync::Mutex,
};

static MUTATION_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedCodexPreview {
    pub id: String,
    pub title: String,
    pub paths: Vec<PathBuf>,
    pub bytes: u64,
    pub fingerprint: String,
}

fn open(paths: &CodexPaths, writable: bool) -> Result<Connection> {
    ensure_safe_path(&paths.home, &paths.state_db())?;
    let conn = Connection::open_with_flags(
        paths.state_db(),
        if writable {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        } else {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        },
    )?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

pub fn preview(paths: &CodexPaths, id: &str) -> Result<SelectedCodexPreview> {
    snapshot(paths, &open(paths, false)?, id, true)
}

fn snapshot(
    paths: &CodexPaths,
    conn: &Connection,
    id: &str,
    expected_archived: bool,
) -> Result<SelectedCodexPreview> {
    let columns = crate::archive::table_columns(conn, "threads")?;
    let row = row_value(conn, "threads", "id", id)?.context("线程不存在")?;
    let archived = row.get("archived").and_then(Value::as_i64).unwrap_or(0) != 0
        || row.get("archived_at").and_then(Value::as_i64).unwrap_or(0) > 0;
    ensure!(
        archived == expected_archived,
        "线程归档状态与操作不匹配，请刷新列表"
    );
    for field in [
        "active_turn_id",
        "activeTurnId",
        "active_job_id",
        "activeJobId",
        "running_job_id",
        "runningJobId",
    ] {
        ensure!(
            row.get(field)
                .is_none_or(|v| v.is_null() || v.as_str().is_some_and(|s| s.trim().is_empty())),
            "线程仍有活动任务，暂不能删除"
        );
    }
    for field in ["status", "state"] {
        ensure!(
            !row.get(field)
                .and_then(Value::as_str)
                .is_some_and(|s| matches!(
                    s.to_ascii_lowercase().as_str(),
                    "running" | "active" | "in_progress" | "pending" | "submitting"
                )),
            "线程仍在运行，暂不能删除"
        );
    }
    for (table, key) in [
        ("jobs", "thread_id"),
        ("agent_job_items", "assigned_thread_id"),
    ] {
        let cols = crate::archive::table_columns(conn, table)?;
        if cols.contains(key) && cols.contains("status") {
            let active: bool = conn.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE {key}=?1 AND lower(status) IN ('running','active','in_progress','pending','submitting'))"), [id], |r| r.get(0))?;
            ensure!(!active, "线程仍有关联的运行任务，暂不能删除");
        }
    }
    let index = read_index(paths)?;
    let index_rows = index
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .collect::<Vec<_>>();
    let mut files = BTreeSet::new();
    if let Some(path) = row
        .get("rollout_path")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        files.insert(PathBuf::from(path));
    }
    for entry in &index_rows {
        if entry.get("id").and_then(Value::as_str) == Some(id) {
            if let Some(path) = entry
                .get("path")
                .or_else(|| entry.get("rollout_path"))
                .and_then(Value::as_str)
            {
                files.insert(PathBuf::from(path));
            }
        }
    }
    // Only known native stores and exact filename suffixes may supply additional files.
    for root in [paths.sessions_dir(), paths.home.join("archived_sessions")] {
        if !root.exists() {
            continue;
        }
        ensure_safe_path(&paths.home, &root)?;
        for entry in walkdir::WalkDir::new(&root).follow_links(false) {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy();
            if name.starts_with("rollout-") && name.ends_with(&format!("-{id}.jsonl")) {
                files.insert(entry.path().to_path_buf());
            }
        }
    }
    let mut digest = Sha256::new();
    digest.update(serde_json::to_vec(&row)?);
    let mut bytes = 0;
    for entry in &index_rows {
        if entry.get("id").and_then(Value::as_str) == Some(id) {
            digest.update(serde_json::to_vec(entry)?);
        }
    }
    for file in &files {
        ensure!(
            file.starts_with(paths.sessions_dir())
                || file.starts_with(paths.home.join("archived_sessions")),
            "线程文件超出原生会话目录"
        );
        ensure_safe_path(&paths.home, file)?;
        let actual = fs::canonicalize(file)?;
        ensure!(
            session_header_id(file)?.as_deref() == Some(id),
            "线程文件身份与选中 ID 不一致"
        );
        ensure!(
            !rollout_has_running_signal(file)?,
            "线程文件仍有运行信号，暂不能删除"
        );
        if columns.contains("rollout_path") {
            let mut stmt = conn.prepare(
                "SELECT rollout_path FROM threads WHERE id != ?1 AND rollout_path IS NOT NULL",
            )?;
            for path in stmt.query_map([id], |r| r.get::<_, String>(0))? {
                ensure!(
                    !fs::canonicalize(path?).is_ok_and(|p| p == actual),
                    "文件仍被其他线程引用"
                );
            }
        }
        for entry in &index_rows {
            if entry.get("id").and_then(Value::as_str) == Some(id) {
                continue;
            }
            if let Some(path) = entry
                .get("path")
                .or_else(|| entry.get("rollout_path"))
                .and_then(Value::as_str)
            {
                ensure!(
                    !fs::canonicalize(path).is_ok_and(|p| p == actual),
                    "文件仍被其他线程索引引用"
                );
            }
        }
        let metadata = fs::metadata(file)?;
        ensure!(metadata.is_file(), "线程目标不是普通文件");
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            ensure!(metadata.nlink() == 1, "线程文件有其他硬链接，暂不能删除");
            digest.update(metadata.ino().to_le_bytes());
            digest.update(metadata.dev().to_le_bytes());
            digest.update(metadata.ctime().to_le_bytes());
            digest.update(metadata.ctime_nsec().to_le_bytes());
        }
        digest.update(file.as_os_str().as_encoded_bytes());
        digest.update(content_hash(file)?);
        bytes += metadata.len();
    }
    Ok(SelectedCodexPreview {
        id: id.to_string(),
        title: row
            .get("name")
            .or_else(|| row.get("title"))
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(id)
            .to_string(),
        paths: files.into_iter().collect(),
        bytes,
        fingerprint: hex::encode(digest.finalize()),
    })
}

pub fn preview_archive(
    paths: &CodexPaths,
    id: &str,
    archive: bool,
) -> Result<SelectedCodexPreview> {
    snapshot(paths, &open(paths, false)?, id, !archive)
}

pub fn execute_archive(
    paths: &CodexPaths,
    id: &str,
    archive: bool,
    fingerprint: &str,
) -> Result<()> {
    let _guard = MUTATION_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("线程管理锁不可用"))?;
    let mut conn = open(paths, true)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    ensure!(
        snapshot(paths, &tx, id, !archive)?.fingerprint == fingerprint,
        "线程或文件已变化，请重新预览"
    );
    let columns = crate::archive::table_columns(&tx, "threads")?;
    ensure!(columns.contains("archived_at"), "原生归档字段不可用");
    let at = archive.then(|| chrono::Utc::now().timestamp());
    let changed = if columns.contains("archived") {
        tx.execute(
            "UPDATE threads SET archived=?2,archived_at=?3 WHERE id=?1",
            rusqlite::params![id, archive, at],
        )?
    } else {
        tx.execute(
            "UPDATE threads SET archived_at=?2 WHERE id=?1",
            rusqlite::params![id, at],
        )?
    };
    ensure!(changed == 1, "线程身份已变化");
    tx.commit()?;
    Ok(())
}

fn content_hash(path: &Path) -> Result<Vec<u8>> {
    let mut reader = fs::File::open(path)?;
    let mut buffer = [0; 65536];
    let mut digest = Sha256::new();
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    Ok(digest.finalize().to_vec())
}

fn session_header_id(path: &Path) -> Result<Option<String>> {
    let mut line = String::new();
    BufReader::new(fs::File::open(path)?)
        .take(1024 * 1024)
        .read_line(&mut line)?;
    let header: Value = serde_json::from_str(&line).context("线程文件头损坏")?;
    ensure!(
        header.get("type").and_then(Value::as_str) == Some("session_meta"),
        "未知线程文件格式"
    );
    Ok(header
        .pointer("/payload/id")
        .and_then(Value::as_str)
        .map(str::to_string))
}

fn ensure_safe_path(root: &Path, path: &Path) -> Result<()> {
    let relative = path.strip_prefix(root).context("目标超出 Codex 数据目录")?;
    let mut current = root.to_path_buf();
    ensure!(
        !fs::symlink_metadata(&current)?.file_type().is_symlink(),
        "Codex 数据目录不能是符号链接"
    );
    for component in relative.components() {
        ensure!(
            matches!(component, std::path::Component::Normal(_)),
            "路径包含不安全分量"
        );
        current.push(component);
        ensure!(
            !fs::symlink_metadata(&current)?.file_type().is_symlink(),
            "线程管理不接受符号链接"
        );
    }
    ensure!(
        fs::canonicalize(path)?.starts_with(fs::canonicalize(root)?),
        "目标超出 Codex 数据目录"
    );
    Ok(())
}

fn read_index(paths: &CodexPaths) -> Result<String> {
    let path = paths.session_index();
    if !path.try_exists()? {
        return Ok(String::new());
    }
    ensure_safe_path(&paths.home, &path)?;
    let text = fs::read_to_string(path)?;
    for line in text.lines().filter(|s| !s.trim().is_empty()) {
        serde_json::from_str::<Value>(line).context("会话索引损坏，暂不能删除")?;
    }
    Ok(text)
}

fn row_value(conn: &Connection, table: &str, key: &str, id: &str) -> Result<Option<Value>> {
    let mut stmt = conn.prepare(&format!("SELECT * FROM {table} WHERE {key}=?1"))?;
    let names = stmt
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    Ok(stmt
        .query_row([id], |r| {
            let mut values = serde_json::Map::new();
            for (i, name) in names.iter().enumerate() {
                let value = match r.get_ref(i)? {
                    ValueRef::Null => Value::Null,
                    ValueRef::Integer(n) => Value::from(n),
                    ValueRef::Real(n) => Value::from(n),
                    ValueRef::Text(s) => Value::from(String::from_utf8_lossy(s).into_owned()),
                    ValueRef::Blob(b) => serde_json::json!({"hex":hex::encode(b)}),
                };
                values.insert(name.clone(), value);
            }
            Ok(Value::Object(values))
        })
        .optional()?)
}

pub fn execute(paths: &CodexPaths, id: &str, fingerprint: &str) -> Result<u64> {
    let _guard = MUTATION_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("线程管理锁不可用"))?;
    let mut conn = open(paths, true)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let selected = snapshot(paths, &tx, id, true)?;
    ensure!(
        selected.fingerprint == fingerprint,
        "线程或文件已变化，请重新预览"
    );
    let hashes = selected
        .paths
        .iter()
        .map(|p| content_hash(p))
        .collect::<Result<Vec<_>>>()?;
    let original_index = read_index(paths)?;
    ensure!(
        snapshot(paths, &tx, id, true)?.fingerprint == fingerprint,
        "线程在执行检查期间变化，请重新预览"
    );
    let index_existed = paths.session_index().exists();
    let rewritten_index = original_index
        .split_inclusive('\n')
        .filter(|line| {
            serde_json::from_str::<Value>(line)
                .ok()
                .and_then(|v| v.get("id").and_then(Value::as_str).map(|s| s != id))
                .unwrap_or(true)
        })
        .collect::<String>();
    let quarantine = paths
        .home
        .join(format!(".nexushub-delete-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&quarantine)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&quarantine, fs::Permissions::from_mode(0o700))?;
    }
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut index_replaced = false;
    let result = (|| -> Result<()> {
        fs::write(
            quarantine.join("thread.json"),
            serde_json::to_vec_pretty(&row_value(&tx, "threads", "id", id)?)?,
        )?;
        fs::write(quarantine.join("index.before"), original_index.as_bytes())?;
        if index_existed {
            fs::set_permissions(
                quarantine.join("index.before"),
                fs::metadata(paths.session_index())?.permissions(),
            )?;
        }
        fs::write(
            quarantine.join("manifest.json"),
            serde_json::to_vec_pretty(&selected)?,
        )?;
        for (i, file) in selected.paths.iter().enumerate() {
            ensure_safe_path(&paths.home, file)?;
            let staged = quarantine.join(format!("rollout-{i}.jsonl"));
            fs::rename(file, &staged)?;
            moved.push((file.clone(), staged));
        }
        for (i, (_, staged)) in moved.iter().enumerate() {
            ensure!(
                !fs::symlink_metadata(staged)?.file_type().is_symlink(),
                "隔离后文件变为符号链接"
            );
            ensure!(
                content_hash(staged)? == hashes[i],
                "隔离后文件发生变化，请重新预览"
            );
            ensure!(
                !rollout_has_running_signal(staged)?,
                "隔离后发现运行中的线程"
            );
        }
        ensure!(
            read_index(paths)? == original_index,
            "会话索引已被其他进程修改，请重新预览"
        );
        if index_existed {
            let candidate = quarantine.join("index.next");
            fs::write(&candidate, rewritten_index.as_bytes())?;
            fs::set_permissions(
                &candidate,
                fs::metadata(paths.session_index())?.permissions(),
            )?;
            fs::File::open(&candidate)?.sync_all()?;
            ensure_safe_path(&paths.home, &paths.session_index())?;
            ensure!(
                read_index(paths)? == original_index,
                "会话索引已变化，请重新预览"
            );
            fs::rename(candidate, paths.session_index())?;
            index_replaced = true;
        }
        crate::archive::prepare_delete_threads(&tx, &[id.to_string()])?;
        crate::archive::cleanup_selected_threads(&tx)?;
        ensure!(
            read_index(paths)? == rewritten_index,
            "删除期间会话索引发生变化"
        );
        tx.commit()?;
        Ok(())
    })();
    if let Err(error) = result {
        let mut restored = true;
        for (original, staged) in moved.iter().rev() {
            if original.exists() || fs::rename(staged, original).is_err() {
                restored = false;
            }
        }
        if index_replaced {
            if read_index(paths).is_ok_and(|s| s == rewritten_index) {
                if fs::rename(quarantine.join("index.before"), paths.session_index()).is_err() {
                    restored = false;
                }
            } else {
                restored = false;
            }
        }
        if restored {
            fs::remove_dir_all(&quarantine)?;
            return Err(error.context("删除失败，文件及数据库已恢复"));
        }
        return Err(error.context(format!(
            "删除未完成；可恢复数据保留在 {}",
            quarantine.display()
        )));
    }
    fs::remove_dir_all(&quarantine).with_context(|| {
        format!(
            "线程记录已删除，待清理的可恢复文件保留在 {}",
            quarantine.display()
        )
    })?;
    Ok(selected.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use serde_json::json;
    struct Fixture {
        paths: CodexPaths,
    }
    impl Fixture {
        fn new() -> Self {
            let home =
                std::env::temp_dir().join(format!("selected-codex-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(home.join("archived_sessions")).unwrap();
            let paths = CodexPaths { home };
            let conn = Connection::open(paths.state_db()).unwrap();
            conn.execute_batch("CREATE TABLE threads(id TEXT PRIMARY KEY,title TEXT,source TEXT,rollout_path TEXT,archived INTEGER,status TEXT,parent_thread_id TEXT,archived_at INTEGER);
                CREATE TABLE thread_spawn_edges(parent_thread_id TEXT,child_thread_id TEXT);
                CREATE TABLE thread_dynamic_tools(thread_id TEXT);
                CREATE TABLE agent_job_items(id TEXT,assigned_thread_id TEXT,status TEXT);").unwrap();
            for id in ["one", "two", "child", "active"] {
                let file = paths
                    .home
                    .join("archived_sessions")
                    .join(format!("rollout-test-{id}.jsonl"));
                fs::write(
                    &file,
                    format!(
                        "{}\n{}\n",
                        json!({"type":"session_meta","payload":{"id":id}}),
                        json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"t"}})
                    ),
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO threads VALUES(?1,?1,'cli',?2,?3,'idle',?4,NULL)",
                    params![
                        id,
                        file.display().to_string(),
                        if id == "active" { 0 } else { 1 },
                        if id == "child" { Some("one") } else { None }
                    ],
                )
                .unwrap();
            }
            conn.execute("INSERT INTO thread_spawn_edges VALUES('one','child')", [])
                .unwrap();
            conn.execute("INSERT INTO thread_dynamic_tools VALUES('one')", [])
                .unwrap();
            conn.execute(
                "INSERT INTO agent_job_items VALUES('j','one','completed')",
                [],
            )
            .unwrap();
            fs::write(
                paths.session_index(),
                ["one", "two", "child", "active"]
                    .map(|id| json!({"id":id,"thread_name":id}).to_string() + "\n")
                    .concat(),
            )
            .unwrap();
            Self { paths }
        }
        fn file(&self, id: &str) -> PathBuf {
            self.paths
                .home
                .join("archived_sessions")
                .join(format!("rollout-test-{id}.jsonl"))
        }
        fn conn(&self) -> Connection {
            Connection::open(self.paths.state_db()).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.paths.home).unwrap();
        }
    }
    #[test]
    fn batch_archive_restore_delete_rechecks_state_and_returns_per_item_results() {
        use crate::services::sessions::*;
        let f = Fixture::new();
        let api = SessionUseCases {
            platform: crate::platform::PlatformPaths::for_kind(
                crate::platform::PlatformKind::Macos,
            ),
            codex: f.paths.clone(),
            grok: crate::grok::GrokPaths {
                home: f.paths.home.join("unused-grok"),
            },
            pi: crate::pi::PiPaths::from_sessions(f.paths.home.join("unused-pi")),
        };
        let first = api
            .bulk_preview(SessionBatchRequest {
                provider: SessionProvider::Codex,
                operation: SessionOperation::Archive,
                session_keys: vec!["active".into(), "one".into()],
            })
            .unwrap();
        assert!(first.items[0].allowed);
        assert!(!first.items[1].allowed);
        let request = SessionBatchExecuteRequest {
            provider: SessionProvider::Codex,
            operation: SessionOperation::Archive,
            confirmed: true,
            items: vec![SessionBatchSelection {
                session_key: "active".into(),
                fingerprint: first.items[0].fingerprint.clone().unwrap(),
            }],
        };
        assert_eq!(
            api.bulk_execute(request.clone()).unwrap().items[0].status,
            "succeeded"
        );
        assert_eq!(
            api.bulk_execute(request).unwrap().items[0].status,
            "blocked"
        );
        let restore = preview_archive(&f.paths, "active", false).unwrap();
        execute_archive(&f.paths, "active", false, &restore.fingerprint).unwrap();
        assert!(preview(&f.paths, "active").is_err());
        let deletion = api
            .bulk_preview(SessionBatchRequest {
                provider: SessionProvider::Codex,
                operation: SessionOperation::Delete,
                session_keys: vec!["one".into(), "two".into(), "active".into()],
            })
            .unwrap();
        assert_eq!(deletion.items.iter().filter(|i| i.allowed).count(), 2);
        fs::write(f.file("two"), "changed").unwrap();
        let result = api
            .bulk_execute(SessionBatchExecuteRequest {
                provider: SessionProvider::Codex,
                operation: SessionOperation::Delete,
                confirmed: true,
                items: deletion
                    .items
                    .into_iter()
                    .filter(|i| i.allowed)
                    .map(|i| SessionBatchSelection {
                        session_key: i.session_key,
                        fingerprint: i.fingerprint.unwrap(),
                    })
                    .collect(),
            })
            .unwrap();
        assert_eq!(result.items[0].status, "succeeded");
        assert_eq!(result.items[1].status, "blocked");
        assert!(f.file("child").is_file());
        assert!(f.file("active").is_file());
        assert!(f.file("two").is_file());
        assert!(api
            .bulk_preview(SessionBatchRequest {
                provider: SessionProvider::Codex,
                operation: SessionOperation::Delete,
                session_keys: vec!["one".into(); 101]
            })
            .is_err());
        assert!(api
            .bulk_preview(SessionBatchRequest {
                provider: SessionProvider::Pi,
                operation: SessionOperation::Archive,
                session_keys: vec!["one".into()]
            })
            .is_err());
    }

    #[test]
    fn selected_deletion_preserves_unselected_child_and_active_threads() {
        let f = Fixture::new();
        for id in ["one", "two"] {
            let plan = preview(&f.paths, id).unwrap();
            execute(&f.paths, id, &plan.fingerprint).unwrap();
        }
        let ids = f
            .conn()
            .prepare("SELECT id FROM threads ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(ids, vec!["active", "child"]);
        assert!(f.file("child").is_file());
        assert!(f.file("active").is_file());
        assert!(!f.file("one").exists());
        assert!(!f.file("two").exists());
        assert!(read_index(&f.paths).unwrap().contains("child"));
        assert!(!read_index(&f.paths).unwrap().contains("one"));
        assert!(f
            .conn()
            .query_row("SELECT assigned_thread_id FROM agent_job_items", [], |r| {
                r.get::<_, Option<String>>(0)
            })
            .unwrap()
            .is_none());
        assert!(preview(&f.paths, "active").is_err());
        assert!(preview(&f.paths, "one").is_err());
    }
    #[test]
    fn changed_fingerprint_shared_reference_and_live_jobs_are_blocked() {
        let f = Fixture::new();
        let p = preview(&f.paths, "one").unwrap();
        use std::io::Write;
        fs::OpenOptions::new()
            .append(true)
            .open(f.file("one"))
            .unwrap()
            .write_all(b"{\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\"}}\n")
            .unwrap();
        assert!(execute(&f.paths, "one", &p.fingerprint).is_err());
        assert!(f.file("one").exists());
        f.conn()
            .execute(
                "UPDATE threads SET rollout_path=?1 WHERE id='two'",
                [f.file("one").display().to_string()],
            )
            .unwrap();
        assert!(preview(&f.paths, "one")
            .unwrap_err()
            .to_string()
            .contains("引用"));
        f.conn()
            .execute(
                "UPDATE threads SET rollout_path=?1 WHERE id='two'",
                [f.file("two").display().to_string()],
            )
            .unwrap();
        f.conn()
            .execute("UPDATE agent_job_items SET status='running'", [])
            .unwrap();
        assert!(preview(&f.paths, "one")
            .unwrap_err()
            .to_string()
            .contains("运行"));
    }
    #[test]
    fn database_failure_restores_files_index_and_relations() {
        let f = Fixture::new();
        let original = read_index(&f.paths).unwrap();
        let bytes = fs::read(f.file("one")).unwrap();
        let p = preview(&f.paths, "one").unwrap();
        f.conn().execute_batch("CREATE TRIGGER fail_delete BEFORE DELETE ON threads BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        assert!(execute(&f.paths, "one", &p.fingerprint).is_err());
        assert_eq!(fs::read(f.file("one")).unwrap(), bytes);
        assert_eq!(read_index(&f.paths).unwrap(), original);
        assert_eq!(
            f.conn()
                .query_row("SELECT count(*) FROM thread_dynamic_tools", [], |r| r
                    .get::<_, u32>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            f.conn()
                .query_row("SELECT count(*) FROM threads", [], |r| r.get::<_, u32>(0))
                .unwrap(),
            4
        );
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_and_outside_rollouts_are_never_deleted() {
        let f = Fixture::new();
        fs::remove_file(f.file("one")).unwrap();
        std::os::unix::fs::symlink(f.file("two"), f.file("one")).unwrap();
        assert!(preview(&f.paths, "one").is_err());
        assert!(f.file("two").exists());
        f.conn()
            .execute(
                "UPDATE threads SET rollout_path='/etc/passwd' WHERE id='two'",
                [],
            )
            .unwrap();
        assert!(preview(&f.paths, "two").is_err());
    }
}
