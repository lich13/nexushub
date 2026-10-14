use super::*;
use crate::scan_work::{MemoryLease, ScanWork};
use std::collections::{HashMap, HashSet};

/// Shared discovery only. Execution still constructs fresh per-item snapshots.
pub(crate) struct PreviewContext {
    pub(super) columns: HashSet<String>,
    pub(super) rows: HashMap<String, Value>,
    pub(super) index: HashMap<String, Vec<Value>>,
    pub(super) files: HashMap<String, BTreeSet<PathBuf>>,
    pub(super) references: HashMap<PathBuf, HashSet<String>>,
    pub(super) active: HashSet<String>,
    pub(crate) index_lines: u64,
    pub(crate) rollout_files: u64,
    _memory: MemoryLease,
}

impl PreviewContext {
    pub(crate) fn load(paths: &CodexPaths, conn: &Connection, work: &ScanWork) -> Result<Self> {
        work.check()?;
        let mut context = Self {
            columns: crate::archive::table_columns(conn, "threads")?,
            rows: HashMap::new(),
            index: HashMap::new(),
            files: HashMap::new(),
            references: HashMap::new(),
            active: HashSet::new(),
            index_lines: 0,
            rollout_files: 0,
            _memory: work.lease(),
        };
        let mut stmt = conn.prepare("SELECT * FROM threads")?;
        let names: Vec<_> = stmt.column_names().into_iter().map(str::to_owned).collect();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            work.check()?;
            let value = read_row_value(row, &names)?;
            context._memory.grow(
                serde_json::to_vec(&value)?
                    .len()
                    .saturating_mul(8)
                    .saturating_add(1024),
            )?;
            if let Some(id) = value.get("id").and_then(Value::as_str) {
                if let Some(path) = value
                    .get("rollout_path")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                {
                    context.add_reference(id, path);
                }
                context.rows.insert(id.to_owned(), value);
            }
        }
        for (table, key) in [
            ("jobs", "thread_id"),
            ("agent_job_items", "assigned_thread_id"),
        ] {
            let columns = crate::archive::table_columns(conn, table)?;
            if columns.contains(key) && columns.contains("status") {
                let mut stmt = conn.prepare(&format!("SELECT {key} FROM {table} WHERE {key} IS NOT NULL AND lower(status) IN ('running','active','in_progress','pending','submitting')"))?;
                for id in stmt.query_map([], |row| row.get::<_, String>(0))? {
                    work.check()?;
                    let id = id?;
                    context._memory.grow(id.len() + 128)?;
                    context.active.insert(id);
                }
            }
        }
        let index = paths.session_index();
        if index.try_exists()? {
            ensure_safe_path(&paths.home, &index)?;
            let mut reader = BufReader::new(work.reader(fs::File::open(&index)?));
            while let Some((line, _memory)) = crate::scan_work::read_record(&mut reader, work)? {
                context.index_lines += 1;
                if line.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                let value: Value =
                    serde_json::from_slice(&line).context("会话索引损坏，暂不能删除")?;
                context
                    ._memory
                    .grow(line.len().saturating_mul(8).saturating_add(512))?;
                if let Some(id) = value.get("id").and_then(Value::as_str) {
                    if let Some(path) = value
                        .get("path")
                        .or_else(|| value.get("rollout_path"))
                        .and_then(Value::as_str)
                    {
                        context.add_reference(id, path);
                    }
                    context.index.entry(id.to_owned()).or_default().push(value);
                }
            }
        }
        for root in [paths.sessions_dir(), paths.home.join("archived_sessions")] {
            if !root.try_exists()? {
                continue;
            }
            ensure_safe_path(&paths.home, &root)?;
            for entry in walkdir::WalkDir::new(&root).follow_links(false) {
                work.check()?;
                let entry = entry?;
                let name = entry.file_name().to_string_lossy();
                if root == paths.sessions_dir()
                    && entry.file_type().is_file()
                    && name.starts_with("rollout-")
                    && name.ends_with(".jsonl")
                {
                    context.rollout_files += 1;
                }
                let Some(stem) = name
                    .strip_prefix("rollout-")
                    .and_then(|name| name.strip_suffix(".jsonl"))
                else {
                    continue;
                };
                for start in
                    std::iter::once(0).chain(stem.match_indices('-').map(|(index, _)| index + 1))
                {
                    let id = &stem[start..];
                    if context.rows.contains_key(id) {
                        context
                            ._memory
                            .grow(entry.path().as_os_str().len() * 4 + id.len() + 256)?;
                        context
                            .files
                            .entry(id.to_owned())
                            .or_default()
                            .insert(entry.path().to_owned());
                    }
                }
            }
        }
        Ok(context)
    }

    fn add_reference(&mut self, id: &str, path: &str) {
        if let Ok(canonical) = fs::canonicalize(path) {
            self.references
                .entry(canonical)
                .or_default()
                .insert(id.to_owned());
        }
    }

    pub(crate) fn preview(
        &self,
        paths: &CodexPaths,
        conn: &Connection,
        id: &str,
        work: &ScanWork,
    ) -> Result<SelectedCodexPreview> {
        snapshot_with_context(paths, conn, id, Some(false), Some(self), work)
    }
}
