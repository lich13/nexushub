//! Direct deletion of selected native storage. Reading and activity never gate removal.
use super::*;
use crate::{
    codex::rollout_file::{self, RolloutName},
    session_delete,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct TreePreview {
    pub selected: SelectedCodexPreview,
    pub ids: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Default)]
pub struct DeleteOutcome {
    pub deleted_ids: Vec<String>,
    pub residuals: Vec<String>,
    pub error: Option<String>,
    pub deleted_files: u64,
}

fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub(crate) fn descendants(paths: &CodexPaths, roots: &[String]) -> Result<Vec<String>> {
    for id in roots {
        session_delete::valid_id(id)?;
    }
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    if paths.state_db().exists() {
        let conn = Connection::open_with_flags(paths.state_db(), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let available = crate::archive::table_columns(&conn, "threads")?;
        let selected = ["id", "parent_thread_id", "parentThreadId", "source"]
            .into_iter()
            .filter(|column| available.contains(*column))
            .map(quoted)
            .collect::<Vec<_>>();
        let mut statement = conn.prepare(&format!("SELECT {} FROM threads", selected.join(",")))?;
        let names: Vec<_> = statement
            .column_names()
            .into_iter()
            .map(str::to_owned)
            .collect();
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let value = read_row_value(row, &names)?;
            let Some(id) = value["id"].as_str() else {
                continue;
            };
            let source = value["source"]
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .unwrap_or(Value::Null);
            for parent in [
                value.get("parent_thread_id"),
                value.get("parentThreadId"),
                source.pointer("/subagent/thread_spawn/parent_thread_id"),
                source.pointer("/parent_thread_id"),
            ]
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            {
                edges.entry(parent.into()).or_default().insert(id.into());
            }
        }
        let columns = crate::archive::table_columns(&conn, "thread_spawn_edges")?;
        if columns.contains("parent_thread_id") && columns.contains("child_thread_id") {
            let mut stmt =
                conn.prepare("SELECT parent_thread_id, child_thread_id FROM thread_spawn_edges")?;
            for row in
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            {
                let (parent, child) = row?;
                edges.entry(parent).or_default().insert(child);
            }
        }
    }
    let mut pending = roots.to_vec();
    let mut seen = BTreeSet::new();
    while let Some(id) = pending.pop() {
        session_delete::valid_id(&id)?;
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(children) = edges.get(&id) {
            pending.extend(children.iter().cloned());
        }
    }
    Ok(seen.into_iter().collect())
}

struct Inventory {
    files: BTreeSet<PathBuf>,
    rollouts: BTreeSet<String>,
    errors: Vec<String>,
}

fn inventory(paths: &CodexPaths, ids: &BTreeSet<String>) -> Inventory {
    let mut result = Inventory {
        files: BTreeSet::new(),
        rollouts: ids.clone(),
        errors: Vec::new(),
    };
    let roots = [paths.sessions_dir(), paths.home.join("archived_sessions")];
    let mut add = |path: PathBuf| {
        if (path.extension().is_some_and(|ext| ext == "jsonl")
            || rollout_file::is_compressed(&path))
            && roots.iter().any(|root| session_delete::within(root, &path))
        {
            if let Some(name) = RolloutName::parse(&path) {
                result.rollouts.insert(name.rollout_id);
            }
            for sibling in [
                rollout_file::plain_path(&path),
                rollout_file::compressed_path(&path),
            ] {
                result.files.insert(sibling);
            }
        }
    };
    if paths.state_db().exists() {
        let read = (|| -> Result<()> {
            let conn =
                Connection::open_with_flags(paths.state_db(), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            if crate::archive::table_columns(&conn, "threads")?.contains("rollout_path") {
                let mut stmt = conn.prepare(
                    "SELECT id, rollout_path FROM threads WHERE rollout_path IS NOT NULL",
                )?;
                for row in
                    stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                {
                    let (id, path) = row?;
                    if ids.contains(&id) {
                        add(PathBuf::from(path));
                    }
                }
            }
            Ok(())
        })();
        if let Err(e) = read {
            result.errors.push(format!("原生索引: {e}"));
        }
    }
    if let Ok(file) = fs::File::open(paths.session_index()) {
        for line in BufReader::new(file).lines() {
            match line {
                Ok(line) => {
                    if let Ok(row) = serde_json::from_str::<Value>(&line) {
                        if row["id"].as_str().is_some_and(|id| ids.contains(id)) {
                            if let Some(path) = row
                                .get("path")
                                .or_else(|| row.get("rollout_path"))
                                .and_then(Value::as_str)
                            {
                                add(path.into());
                            }
                        }
                    }
                }
                Err(e) => {
                    result.errors.push(format!("会话名称索引: {e}"));
                    break;
                }
            }
        }
    }
    for root in roots {
        if !root.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(root)
            .follow_links(false)
            .follow_root_links(false)
        {
            match entry {
                Ok(entry) => {
                    if let Some(name) = RolloutName::parse(entry.path()) {
                        if ids.contains(&name.thread_id) {
                            result.rollouts.insert(name.rollout_id);
                            result.files.insert(entry.into_path());
                        }
                    }
                }
                Err(e) => result.errors.push(e.to_string()),
            }
        }
    }
    for id in ids {
        result.files.insert(
            paths
                .home
                .join("thread-writer-locks")
                .join(format!("{id}.lock")),
        );
    }
    let snapshots = paths.home.join("shell_snapshots");
    match fs::read_dir(&snapshots) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        if entry.file_name().to_str().is_some_and(|name| {
                            ids.iter().any(|id| {
                                name.strip_prefix(id.as_str())
                                    .is_some_and(|suffix| suffix.starts_with('.'))
                            })
                        }) {
                            result.files.insert(entry.path());
                        }
                    }
                    Err(error) => result
                        .errors
                        .push(format!("{}: {error}", snapshots.display())),
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => result
            .errors
            .push(format!("{}: {error}", snapshots.display())),
    }
    let visualizations = paths.home.join("visualizations");
    if visualizations.exists() {
        let mut entries = walkdir::WalkDir::new(&visualizations)
            .follow_links(false)
            .follow_root_links(false)
            .into_iter();
        while let Some(entry) = entries.next() {
            match entry {
                Ok(entry)
                    if entry
                        .file_name()
                        .to_str()
                        .is_some_and(|id| ids.contains(id)) =>
                {
                    result.files.insert(entry.into_path());
                    entries.skip_current_dir();
                }
                Ok(_) => {}
                Err(e) => result.errors.push(e.to_string()),
            }
        }
    }
    result
}

pub fn preview(paths: &CodexPaths, id: &str, include_descendants: bool) -> Result<TreePreview> {
    session_delete::valid_id(id)?;
    let (ids, relation_error) = if include_descendants {
        match descendants(paths, &[id.into()]) {
            Ok(ids) => (ids, None),
            Err(error) => (vec![id.into()], Some(format!("父子关联无法读取: {error}"))),
        }
    } else {
        (vec![id.into()], None)
    };
    let mut found = inventory(paths, &ids.iter().cloned().collect());
    found.errors.extend(relation_error);
    let files: Vec<_> = found
        .files
        .into_iter()
        .filter(|p| fs::symlink_metadata(p).is_ok())
        .collect();
    let (_, bytes) = session_delete::size(&files);
    Ok(TreePreview {
        selected: SelectedCodexPreview {
            id: id.into(),
            title: id.into(),
            paths: files,
            bytes,
            fingerprint: String::new(),
        },
        ids,
        warnings: found.errors,
    })
}

fn delete_database(
    path: &Path,
    ids: &BTreeSet<String>,
    rollouts: &BTreeSet<String>,
    files: &BTreeSet<PathBuf>,
) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    ensure!(
        !fs::symlink_metadata(path)?.file_type().is_symlink(),
        "原生数据库不能是链接"
    );
    let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut tables = tx
        .prepare("SELECT name FROM sqlite_master WHERE type='table'")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    tables.sort_by_key(|name| name == "threads");
    let ids_json = serde_json::to_string(ids)?;
    let rollouts_json = serde_json::to_string(rollouts)?;
    let files_json = serde_json::to_string(files)?;
    for table in tables {
        if table.starts_with("sqlite_") || table == "_sqlx_migrations" {
            continue;
        }
        let columns = crate::archive::table_columns(&tx, &quoted(&table))?;
        if table == "agent_job_items" && columns.contains("assigned_thread_id") {
            tx.execute("UPDATE agent_job_items SET assigned_thread_id=NULL WHERE assigned_thread_id IN (SELECT value FROM json_each(?1))",[&ids_json])?;
            continue;
        }
        let mut predicates = Vec::new();
        for column in &columns {
            if column == "rollout_id" {
                predicates.push(format!(
                    "{} IN (SELECT value FROM json_each(?2))",
                    quoted(column)
                ));
            } else if column == "thread_id"
                || (table == "threads" && column == "id")
                || (table == "thread_spawn_edges"
                    && matches!(column.as_str(), "parent_thread_id" | "child_thread_id"))
            {
                predicates.push(format!(
                    "{} IN (SELECT value FROM json_each(?1))",
                    quoted(column)
                ));
            } else if table == "rollout_migration_skipped_rollouts" && column == "rollout_path" {
                predicates.push(format!(
                    "{} IN (SELECT value FROM json_each(?3))",
                    quoted(column)
                ));
            }
        }
        if !predicates.is_empty() {
            // Numbered parameters have a fixed arity, including identities absent from this table.
            tx.execute(&format!("DELETE FROM {} WHERE ({}) AND (?1 IS NOT NULL AND ?2 IS NOT NULL AND ?3 IS NOT NULL)",quoted(&table),predicates.join(" OR ")),rusqlite::params![ids_json,rollouts_json,files_json])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn execute_ids(paths: &CodexPaths, selected: &[String]) -> Result<DeleteOutcome> {
    let _guard = MUTATION_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("线程管理锁不可用"))?;
    for id in selected {
        session_delete::valid_id(id)?;
    }
    let ids: BTreeSet<_> = selected.iter().cloned().collect();
    let inventory = inventory(paths, &ids);
    let mut outcome = session_delete::Outcome {
        residuals: inventory.errors,
        ..Default::default()
    };
    for path in &inventory.files {
        if session_delete::within(&paths.home, path) {
            outcome.remove(path);
        } else {
            outcome
                .residuals
                .push(format!("无法定位线程文件: {}", path.display()));
        }
    }
    let index = paths.session_index();
    outcome.attempt(&index, || {
        session_delete::filter_jsonl(&index, |row| {
            row["id"].as_str().is_some_and(|id| ids.contains(id))
        })
    });
    let history = paths.home.join("history.jsonl");
    outcome.attempt(&history, || {
        session_delete::filter_jsonl(&history, |row| session_delete::has_id(row, &ids))
    });
    let database = paths.state_db();
    outcome.attempt(&database, || {
        delete_database(&database, &ids, &inventory.rollouts, &inventory.files)
    });
    match fs::read_dir(&paths.home) {
        Ok(entries) => {
            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => {
                        outcome.residuals.push(error.to_string());
                        continue;
                    }
                };
                if entry.file_name().to_str().is_some_and(|name| {
                    name.starts_with("thread_history_") && name.ends_with(".sqlite")
                }) {
                    outcome.attempt(&entry.path(), || {
                        delete_database(&entry.path(), &ids, &inventory.rollouts, &inventory.files)
                    });
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => outcome.residuals.push(e.to_string()),
    }
    outcome.finish();
    let deleted_ids = if database.exists() {
        Connection::open_with_flags(&database, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .ok()
            .map(|conn| {
                ids.iter()
                    .filter(|id| {
                        row_value(&conn, "threads", "id", id).is_ok_and(|row| row.is_none())
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    } else {
        ids.into_iter().collect()
    };
    Ok(DeleteOutcome {
        deleted_ids,
        error: (!outcome.residuals.is_empty()).then(|| "部分内容未删除，请查看逐项结果".into()),
        residuals: outcome.residuals,
        deleted_files: outcome.files,
    })
}

#[cfg(test)]
mod tests;
