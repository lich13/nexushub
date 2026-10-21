//! Explicit subtree deletion. Archive and hidden-cleanup retain their separate
//! terminal-state rules. Native storage is read here, never migrated.
use super::*;
use std::collections::{BTreeMap, HashMap};

const MAX_FILES: usize = 20_000;
const MAX_BYTES: u64 = 512 * 1024 * 1024;

#[cfg(test)]
type NativeDelete = Box<dyn FnMut(&CodexPaths, &TreePreview) -> Result<()>>;
#[cfg(test)]
thread_local! { static TEST_DELETE: std::cell::RefCell<Option<NativeDelete>> = const { std::cell::RefCell::new(None) }; }

#[derive(Debug, Clone)]
pub struct TreePreview {
    pub selected: SelectedCodexPreview,
    pub ids: Vec<String>,
    pub warnings: Vec<String>,
    rollouts: BTreeSet<String>,
    files: BTreeMap<PathBuf, String>,
    legacy: bool,
}

#[derive(Debug, Default)]
pub struct DeleteOutcome {
    pub deleted_ids: Vec<String>,
    pub residuals: Vec<String>,
    pub error: Option<String>,
    pub storage_failure: bool,
}

#[derive(Debug)]
struct Rollout {
    path: PathBuf,
    thread: String,
    id: String,
    base: Option<String>,
}

fn rollout_name(path: &Path) -> Option<(String, String)> {
    let name = path.file_name()?.to_str()?;
    let plain = name
        .strip_suffix(".zst")
        .unwrap_or(name)
        .strip_suffix(".jsonl")?;
    let plain = plain.strip_prefix("rollout-")?;
    let (thread_part, rollout) = plain
        .rsplit_once('_')
        .map_or((plain, None), |(left, right)| (left, Some(right)));
    let thread = thread_part.get(thread_part.len().checked_sub(36)?..)?;
    uuid::Uuid::parse_str(thread).ok()?;
    let rollout = rollout.unwrap_or(thread);
    uuid::Uuid::parse_str(rollout).ok()?;
    Some((thread.into(), rollout.into()))
}

fn header(path: &Path) -> Result<Value> {
    let mut line = String::new();
    BufReader::new(fs::File::open(path)?)
        .take(1024 * 1024)
        .read_line(&mut line)?;
    let value: Value = serde_json::from_str(&line).context("原生历史文件头无法核验")?;
    ensure!(value["type"] == "session_meta", "未知原生历史格式");
    Ok(value)
}

fn scan_rollouts(paths: &CodexPaths) -> Result<Vec<Rollout>> {
    let mut result = Vec::new();
    let mut seen = HashMap::new();
    let mut count = 0;
    for root in [paths.sessions_dir(), paths.home.join("archived_sessions")] {
        if !root.try_exists()? {
            continue;
        }
        ensure_safe_path(&paths.home, &root)?;
        for entry in walkdir::WalkDir::new(&root)
            .follow_links(false)
            .max_depth(34)
        {
            let entry = entry?;
            count += 1;
            ensure!(
                count <= MAX_FILES && entry.depth() < 34,
                "原生历史扫描超出上限，未执行删除"
            );
            ensure!(
                !entry.file_type().is_symlink(),
                "原生历史目录包含符号链接，引用无法完整核验"
            );
            if !entry.file_type().is_file() {
                continue;
            }
            let Some((named_thread, id)) = rollout_name(entry.path()) else {
                continue;
            };
            ensure!(
                entry.path().extension().is_none_or(|e| e != "zst"),
                "发现压缩历史，当前版本无法完整核验引用，请在原生应用中处理"
            );
            let value = header(entry.path())?;
            let thread = value
                .pointer("/payload/id")
                .and_then(Value::as_str)
                .context("原生历史缺少身份")?;
            ensure!(thread == named_thread, "原生历史文件名与线程身份不一致");
            ensure!(
                seen.insert(id.clone(), entry.path().to_path_buf())
                    .is_none(),
                "原生 rollout 身份重复，引用无法确认"
            );
            let base = value
                .pointer("/payload/history_base/thread_id")
                .and_then(Value::as_str)
                .map(str::to_string);
            if !value["payload"]["history_base"].is_null() {
                ensure!(base.is_some(), "未知历史分支引用格式");
            }
            result.push(Rollout {
                path: entry.path().into(),
                thread: thread.into(),
                id,
                base,
            });
        }
    }
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

fn file_digest(paths: &CodexPaths, path: &Path, expected: &str) -> Result<(String, u64)> {
    ensure_safe_path(&paths.home, path)?;
    let before = fs::symlink_metadata(path)?;
    ensure!(
        before.is_file() && before.len() <= MAX_BYTES,
        "线程文件类型或大小无法安全核验"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(before.nlink() == 1, "线程文件存在硬链接");
    }
    let mut reader = BufReader::new(fs::File::open(path)?);
    let mut digest = Sha256::new();
    let mut line = String::new();
    let mut first = true;
    loop {
        line.clear();
        let read = reader.by_ref().take(8 * 1024 * 1024).read_line(&mut line)?;
        if read == 0 {
            break;
        }
        ensure!(
            line.ends_with('\n'),
            "线程存在半行或超大记录，请关闭原生写入后重试"
        );
        let row: Value = serde_json::from_str(&line).context("线程记录损坏")?;
        if first {
            ensure!(
                row["type"] == "session_meta" && row["payload"]["id"] == expected,
                "线程文件身份不匹配"
            );
            first = false;
        }
        digest.update(line.as_bytes());
    }
    ensure!(!first, "线程缺少身份记录");
    let after = fs::symlink_metadata(path)?;
    ensure!(
        before.len() == after.len() && before.modified()? == after.modified()?,
        "线程在核验期间发生变化"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            (
                before.dev(),
                before.ino(),
                before.ctime(),
                before.ctime_nsec()
            ) == (after.dev(), after.ino(), after.ctime(), after.ctime_nsec()),
            "线程文件身份已变化"
        );
        digest.update(before.dev().to_le_bytes());
        digest.update(before.ino().to_le_bytes());
        digest.update(before.ctime().to_le_bytes());
        digest.update(before.ctime_nsec().to_le_bytes());
    }
    Ok((hex::encode(digest.finalize()), before.len()))
}

/// Native writers take the coordination lock before opening their per-thread
/// lock. Probe under that same coordination lock; never remove lock files.
fn check_writers(paths: &CodexPaths, ids: &[String], files: &[PathBuf]) -> Result<bool> {
    let directory = paths.home.join("thread-writer-locks");
    if directory.try_exists()? {
        ensure_safe_path(&paths.home, &directory)?;
        let coordination = directory.join(".coordination.lock");
        ensure_safe_path(&paths.home, &coordination)?;
        let guard = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(coordination)?;
        guard
            .try_lock()
            .map_err(|_| anyhow::anyhow!("原生线程归属正在变化，请稍后重新预览"))?;
        for id in ids {
            let path = directory.join(format!("{id}.lock"));
            if !path.try_exists()? {
                continue;
            }
            ensure_safe_path(&paths.home, &path)?;
            let lock = fs::OpenOptions::new().read(true).write(true).open(path)?;
            lock.try_lock()
                .map_err(|_| anyhow::anyhow!("线程树仍有原生活跃 owner 或写入者，请关闭后重试"))?;
        }
        return Ok(true);
    }
    // Old stores lack native writer locks. Do not infer inactivity from mtime.
    // lsof is scoped to the exact native files/database, not unrelated projects.
    let executable = if cfg!(target_os = "macos") {
        "/usr/sbin/lsof"
    } else {
        "lsof"
    };
    let output = std::process::Command::new(executable)
        .args(["-Fpc", "--"])
        .args(files)
        .arg(paths.state_db())
        .output()
        .context("原生占用信息不可用，未执行删除")?;
    ensure!(
        matches!(output.status.code(), Some(0 | 1)) && output.stderr.is_empty(),
        "线程归属无法确认，请关闭原生应用后重试"
    );
    let text = std::str::from_utf8(&output.stdout)?;
    for line in text.lines().filter_map(|line| line.strip_prefix('p')) {
        ensure!(
            line.parse::<u32>()? == std::process::id(),
            "线程仍被其他进程占用，请关闭后重试"
        );
    }
    Ok(false)
}

fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// Hash or count every known identity-bearing row, including rollout-keyed
/// paginated history. An unrecognized thread table fails closed.
fn related_rows(
    conn: &Connection,
    ids: &BTreeSet<String>,
    history: bool,
) -> Result<BTreeMap<String, Vec<Value>>> {
    let names = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut result = BTreeMap::new();
    for table in names {
        if table.starts_with("sqlite_") || table == "_sqlx_migrations" {
            continue;
        }
        if history {
            ensure!(
                matches!(
                    table.as_str(),
                    "thread_turns"
                        | "thread_items"
                        | "thread_history_projection_state"
                        | "thread_realtime_items"
                        | "thread_turn_ends"
                ),
                "分页历史格式不支持完整核验"
            );
        }
        let cols = crate::archive::table_columns(conn, &quoted(&table))?;
        let keys: Vec<_> = cols
            .iter()
            .filter(|col| {
                matches!(
                    col.as_str(),
                    "thread_id"
                        | "rollout_id"
                        | "parent_thread_id"
                        | "child_thread_id"
                        | "assigned_thread_id"
                ) || (table == "threads" && col.as_str() == "id")
            })
            .collect();
        if keys.is_empty() {
            ensure!(
                !history && (!table.starts_with("thread_") || table == "thread_sections"),
                "原生关联表格式不支持完整核验"
            );
            continue;
        }
        let mut selected = Vec::new();
        let predicate = keys
            .iter()
            .map(|key| format!("{} IN (SELECT value FROM json_each(?1))", quoted(key)))
            .collect::<Vec<_>>()
            .join(" OR ");
        let mut statement = conn.prepare(&format!(
            "SELECT * FROM {} WHERE {predicate} LIMIT 200001",
            quoted(&table)
        ))?;
        let columns: Vec<_> = statement
            .column_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut rows = statement.query([serde_json::to_string(ids)?])?;
        while let Some(row) = rows.next()? {
            selected.push(read_row_value(row, &columns)?);
            ensure!(selected.len() <= 200_000, "关联记录超出管理核验上限");
        }
        selected.sort_by_key(Value::to_string);
        if !selected.is_empty() {
            result.insert(table, selected);
        }
    }
    Ok(result)
}

fn history_db(paths: &CodexPaths) -> Result<Option<Connection>> {
    for entry in fs::read_dir(&paths.home)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        ensure!(
            !name.starts_with("thread_history_")
                || !name.ends_with(".sqlite")
                || name == "thread_history_1.sqlite",
            "发现不支持的分页历史数据库"
        );
    }
    let path = paths.home.join("thread_history_1.sqlite");
    if !path.try_exists()? {
        return Ok(None);
    }
    ensure_safe_path(&paths.home, &path)?;
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(Some(conn))
}

pub fn preview(paths: &CodexPaths, id: &str, include_descendants: bool) -> Result<TreePreview> {
    let conn = open(paths, false)?;
    let ids = crate::codex::subagents::deletion_tree(paths, id)?;
    ensure!(
        include_descendants || ids.len() == 1,
        "线程包含后代，请升级客户端并明确确认整树删除"
    );
    ensure!(
        crate::codex::codex_task_identity(paths, id)? == crate::codex::CodexTaskIdentity::Main,
        "主线程身份无法核验"
    );
    let scanned = scan_rollouts(paths)?;
    let mut rollouts: BTreeSet<_> = ids.iter().cloned().collect();
    let mut files = BTreeMap::new();
    for file in &scanned {
        if ids.contains(&file.thread) {
            rollouts.insert(file.id.clone());
            files.insert(file.path.clone(), file.thread.clone());
        }
    }
    let mut digest = Sha256::new();
    digest.update(paths.home.as_os_str().as_encoded_bytes());
    digest.update([u8::from(include_descendants)]);
    digest.update(serde_json::to_vec(&ids)?);
    for file in &scanned {
        if !ids.contains(&file.thread) {
            ensure!(
                !file
                    .base
                    .as_ref()
                    .is_some_and(|base| rollouts.contains(base)),
                "其他线程仍引用目标历史分支，请先在原生应用中处理引用"
            );
        }
        // Unrelated trees may change (including earlier batch deletions).
        // Incoming references are rechecked above, target lineage is bound here.
        if ids.contains(&file.thread) {
            digest.update(serde_json::to_vec(&(
                &file.path,
                &file.thread,
                &file.id,
                &file.base,
            ))?);
        }
    }
    let index = read_index(paths)?;
    for line in index.lines().filter(|l| !l.trim().is_empty()) {
        let row: Value = serde_json::from_str(line)?;
        if ids.iter().any(|id| row["id"] == *id) {
            digest.update(line.as_bytes());
            if let Some(path) = row
                .get("path")
                .or_else(|| row.get("rollout_path"))
                .and_then(Value::as_str)
                .filter(|p| !p.is_empty())
            {
                let owner = row["id"].as_str().context("会话索引身份缺失")?;
                files.insert(PathBuf::from(path), owner.into());
            }
        }
    }
    let mut title = String::new();
    for member in &ids {
        let row = row_value(&conn, "threads", "id", member)?.context("子线程记录已变化")?;
        if member == id {
            title = row
                .get("name")
                .or_else(|| row.get("title"))
                .and_then(Value::as_str)
                .unwrap_or("未命名线程")
                .into();
        }
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
                    .is_none_or(|v| v.is_null() || v.as_str() == Some("")),
                "线程仍有关联活动任务"
            );
        }
        if let Some(path) = row["rollout_path"].as_str().filter(|p| !p.is_empty()) {
            files.insert(PathBuf::from(path), member.clone());
        }
    }
    let mut statement =
        conn.prepare("SELECT id, rollout_path FROM threads WHERE rollout_path IS NOT NULL")?;
    for row in statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })? {
        let (other, path) = row?;
        ensure!(
            ids.contains(&other) || !files.contains_key(Path::new(&path)),
            "文件仍被其他线程元数据引用"
        );
    }
    for line in index.lines().filter(|line| !line.trim().is_empty()) {
        let row: Value = serde_json::from_str(line)?;
        if let Some(path) = row
            .get("path")
            .or_else(|| row.get("rollout_path"))
            .and_then(Value::as_str)
        {
            ensure!(
                row["id"]
                    .as_str()
                    .is_some_and(|id| ids.iter().any(|member| member == id))
                    || !files.contains_key(Path::new(path)),
                "文件仍被其他会话索引引用"
            );
        }
    }
    let mut bytes = 0;
    let mut warnings = Vec::new();
    for (path, owner) in &files {
        ensure!(
            path.starts_with(paths.sessions_dir())
                || path.starts_with(paths.home.join("archived_sessions")),
            "线程路径超出原生历史目录"
        );
        ensure!(
            path.extension().is_none_or(|e| e != "zst"),
            "目标包含压缩历史，本轮不支持删除"
        );
        // Metadata-only threads may legitimately have no rollout. Missing
        // files do not make traversal components or dangling links safe.
        let relative = path.strip_prefix(&paths.home)?;
        ensure!(
            relative
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_))),
            "线程路径包含不安全分量"
        );
        let mut component = paths.home.clone();
        for part in relative.components() {
            component.push(part);
            match fs::symlink_metadata(&component) {
                Ok(meta) => ensure!(!meta.file_type().is_symlink(), "线程路径包含符号链接"),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                Err(e) => return Err(e.into()),
            }
        }
        if !path.try_exists()? {
            digest.update(serde_json::to_vec(&(path, "missing"))?);
            continue;
        }
        let (hash, size) = file_digest(paths, path, owner)?;
        digest.update(serde_json::to_vec(&(path, hash))?);
        bytes += size;
        if crate::codex::rollout_waiting_for_reply(path)? {
            warnings.push("将丢弃待回复的问题".into());
        }
    }
    let modern = check_writers(
        paths,
        &ids,
        &files
            .keys()
            .filter(|p| p.exists())
            .cloned()
            .collect::<Vec<_>>(),
    )?;
    let related = related_rows(&conn, &rollouts, false)?;
    for row in related.get("agent_job_items").into_iter().flatten() {
        ensure!(
            !row["status"]
                .as_str()
                .is_some_and(|s| matches!(s, "running" | "active" | "pending" | "submitting")),
            "线程树仍有关联的活动任务"
        );
    }
    digest.update(serde_json::to_vec(&related)?);
    let history = history_db(paths)?;
    if let Some(history) = &history {
        digest.update(serde_json::to_vec(&related_rows(
            history, &rollouts, true,
        )?)?);
    }
    let legacy = !modern
        && history.is_none()
        && ids.len() == 1
        && scanned.iter().all(|r| r.base.is_none() && r.id == r.thread)
        && related.keys().all(|t| {
            matches!(
                t.as_str(),
                "threads" | "thread_dynamic_tools" | "thread_spawn_edges" | "agent_job_items"
            )
        });
    warnings.sort();
    warnings.dedup();
    Ok(TreePreview {
        selected: SelectedCodexPreview {
            id: id.into(),
            title,
            paths: files.keys().cloned().collect(),
            bytes,
            fingerprint: hex::encode(digest.finalize()),
        },
        ids,
        warnings,
        rollouts,
        files,
        legacy,
    })
}

fn verify(paths: &CodexPaths, preview: &TreePreview) -> Result<DeleteOutcome> {
    let conn = open(paths, false)?;
    let related = related_rows(&conn, &preview.rollouts, false)?;
    let history = history_db(paths)?
        .map(|conn| related_rows(&conn, &preview.rollouts, true))
        .transpose()?
        .unwrap_or_default();
    let scanned = scan_rollouts(paths)?;
    let index = read_index(paths)?;
    let mut result = DeleteOutcome::default();
    for path in preview.files.keys() {
        if path.try_exists()? {
            result.residuals.push(path.display().to_string());
        }
    }
    for file in scanned {
        if preview.ids.contains(&file.thread) && !preview.files.contains_key(&file.path) {
            result.residuals.push(file.path.display().to_string());
        }
    }
    for table in related.keys() {
        result.residuals.push(format!("原生索引：{table}"));
    }
    for table in history.keys() {
        result.residuals.push(format!("分页历史：{table}"));
    }
    for id in &preview.ids {
        let indexed = index
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .any(|row| row["id"] == *id);
        if indexed {
            result.residuals.push(format!("会话名称索引：{id}"));
        }
        if row_value(&conn, "threads", "id", id)?.is_none() {
            result.deleted_ids.push(id.clone());
        }
    }
    Ok(result)
}

pub fn execute(
    paths: &CodexPaths,
    id: &str,
    include_descendants: bool,
    fingerprint: &str,
) -> Result<DeleteOutcome> {
    let _guard = MUTATION_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("线程管理锁不可用"))?;
    let preview = preview(paths, id, include_descendants)?;
    ensure!(
        preview.selected.fingerprint == fingerprint,
        "线程树、历史或引用已变化，请重新预览"
    );
    let operation: Result<()> = native_delete(paths, &preview, include_descendants);
    let mut outcome = match verify(paths, &preview) {
        Ok(result) => result,
        Err(_) => DeleteOutcome {
            error: Some("删除可能已部分完成；无法完整核验文件、原生索引和分页历史。请刷新原生应用后重新预览，勿直接重试".into()),
            storage_failure: true,
            ..Default::default()
        },
    };
    if outcome.storage_failure {
        return Ok(outcome);
    }
    if operation.is_err() {
        outcome.error =
            Some("原生删除未确认完整完成；已复检可核验的数据，请检查残留后重新预览".into());
    }
    if !outcome.residuals.is_empty() {
        outcome.error =
            Some("删除未完成，仍有文件或原生记录残留；请在原生应用中检查并重新预览".into());
    }
    Ok(outcome)
}

fn native_delete(
    paths: &CodexPaths,
    preview: &TreePreview,
    include_descendants: bool,
) -> Result<()> {
    #[cfg(test)]
    if let Some(result) = TEST_DELETE.with(|slot| {
        slot.borrow_mut()
            .as_mut()
            .map(|delete| delete(paths, preview))
    }) {
        return result;
    }
    #[cfg(unix)]
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let helper = runtime.block_on(crate::codex::desktop::helper::DesktopHelper::management(
            paths.home.clone(),
        ));
        if let Ok(helper) = helper {
            let latest = self::preview(paths, &preview.selected.id, include_descendants)?;
            ensure!(
                latest.selected.fingerprint == preview.selected.fingerprint,
                "提交前线程树已变化"
            );
            return runtime
                .block_on(helper.delete_thread(&preview.selected.id))
                .map_err(Into::into);
        }
    }
    ensure!(preview.legacy, "官方删除接口不可用，当前存储不允许离线回退");
    // Keep the previous transaction/quarantine implementation only for fully
    // verified legacy terminal singletons. Release our in-process lock before
    // invoking it would create a race, so use its already-locked inner path.
    let old = snapshot(paths, &open(paths, false)?, &preview.selected.id, None)?;
    super::execute_with_expected_archive_locked(paths, &preview.selected.id, &old.fingerprint, None)
        .map(|_| ())
}

#[cfg(test)]
mod tests;
