use crate::codex::{hidden_thread_metadata_category, CodexPaths, ThreadVisibilityMetadata};
use anyhow::{Context, Result};
use rusqlite::{Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveDeletePlan {
    pub total_threads: u64,
    pub active_threads: u64,
    pub archived_threads: u64,
    pub session_index_lines: u64,
    pub rollout_files: u64,
    pub archived_ids: Vec<String>,
    pub integrity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveDeleteResult {
    pub before: ArchiveDeletePlan,
    pub after_total_threads: u64,
    pub after_active_threads: u64,
    pub after_archived_threads: u64,
    pub after_integrity: String,
    pub deleted_rollout_files: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HiddenThreadSelection {
    pub id: String,
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiddenThreadCandidate {
    pub id: String,
    pub title: String,
    pub fingerprint: Option<String>,
    pub allowed: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiddenThreadItemResult {
    pub id: String,
    pub status: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiddenThreadDeletePlan {
    pub total_threads: u64,
    pub visible_threads: u64,
    pub hidden_threads: u64,
    pub archived_threads: u64,
    pub session_index_lines: u64,
    pub rollout_files: u64,
    pub hidden_ids: Vec<String>,
    pub candidates: Vec<HiddenThreadCandidate>,
    #[serde(rename = "hidden_source_counts")]
    pub source_counts: BTreeMap<String, u64>,
    pub integrity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiddenThreadDeleteResult {
    pub skipped_threads: u64,
    pub failed_threads: u64,
    pub items: Vec<HiddenThreadItemResult>,
    pub before: HiddenThreadDeletePlan,
    pub deleted_threads: u64,
    pub after_total_threads: u64,
    pub after_visible_threads: u64,
    pub after_hidden_threads: u64,
    pub after_archived_threads: u64,
    pub after_integrity: String,
    pub visible_threads: u64,
    pub hidden_threads: u64,
    pub integrity: String,
    pub deleted_rollout_files: u64,
}

pub fn plan_delete_archived(paths: &CodexPaths) -> Result<ArchiveDeletePlan> {
    let db = paths.state_db();
    let conn = Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("open {}", db.display()))?;
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        anyhow::bail!("sqlite integrity check failed: {integrity}");
    }
    let columns = table_columns(&conn, "threads")?;
    let archived_condition = archived_sql_condition(&columns);
    let (total, active, archived) = count_threads(&conn)?;
    let updated_expr =
        first_existing(&columns, &["updated_at", "last_activity_at", "created_at"]).unwrap_or("id");
    let sql = format!(
        "SELECT id FROM threads WHERE {archived_condition} ORDER BY {updated_expr} DESC, id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let archived_ids = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ArchiveDeletePlan {
        total_threads: total,
        active_threads: active,
        archived_threads: archived,
        session_index_lines: count_lines(&paths.session_index())?,
        rollout_files: count_rollout_files(&paths.sessions_dir())?,
        archived_ids,
        integrity,
    })
}

pub fn execute_delete_archived(paths: &CodexPaths) -> Result<ArchiveDeleteResult> {
    let before = plan_delete_archived(paths)?;
    if before.archived_threads == 0 {
        return Ok(ArchiveDeleteResult {
            after_total_threads: before.total_threads,
            after_active_threads: before.active_threads,
            after_archived_threads: before.archived_threads,
            after_integrity: before.integrity.clone(),
            before,
            deleted_rollout_files: 0,
        });
    }

    let db = paths.state_db();
    let mut conn = Connection::open(&db).with_context(|| format!("open {}", db.display()))?;
    delete_threads_by_id(&mut conn, &before.archived_ids)?;

    rewrite_session_index(&paths.session_index(), &before.archived_ids)?;
    let deleted_rollout_files = delete_rollouts(&paths.sessions_dir(), &before.archived_ids)?;
    let (after_total, after_active, after_archived) = count_threads(&conn)?;
    let after_integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if after_integrity != "ok" {
        anyhow::bail!("sqlite integrity check failed after deletion: {after_integrity}");
    }
    if after_active != before.active_threads {
        anyhow::bail!(
            "active thread count changed unexpectedly: before={} after={after_active}",
            before.active_threads
        );
    }
    if after_archived != 0 {
        anyhow::bail!("archived threads remain after deletion: {after_archived}");
    }
    Ok(ArchiveDeleteResult {
        before,
        after_total_threads: after_total,
        after_active_threads: after_active,
        after_archived_threads: after_archived,
        after_integrity,
        deleted_rollout_files,
    })
}

pub fn plan_delete_hidden(paths: &CodexPaths) -> Result<HiddenThreadDeletePlan> {
    hidden_delete_plan(paths)
}

pub fn execute_delete_hidden(
    paths: &CodexPaths,
    candidates: &[HiddenThreadSelection],
) -> Result<HiddenThreadDeleteResult> {
    let before = plan_delete_hidden(paths)?;
    let (items, deleted_rollout_files) =
        crate::selected_codex::execute_hidden_batch(paths, candidates)?;
    let after = plan_delete_hidden(paths)?;
    Ok(HiddenThreadDeleteResult {
        deleted_threads: items.iter().filter(|i| i.status == "deleted").count() as u64,
        skipped_threads: items.iter().filter(|i| i.status == "skipped").count() as u64,
        failed_threads: items.iter().filter(|i| i.status == "failed").count() as u64,
        items,
        before,
        after_total_threads: after.total_threads,
        after_visible_threads: after.visible_threads,
        after_hidden_threads: after.hidden_threads,
        after_archived_threads: after.archived_threads,
        after_integrity: after.integrity.clone(),
        visible_threads: after.visible_threads,
        hidden_threads: after.hidden_threads,
        integrity: after.integrity,
        deleted_rollout_files,
    })
}

fn count_threads(conn: &Connection) -> Result<(u64, u64, u64)> {
    let columns = table_columns(conn, "threads")?;
    let archived_condition = archived_sql_condition(&columns);
    let sql = format!(
        "SELECT count(*),
                coalesce(sum(CASE WHEN NOT ({archived_condition}) THEN 1 ELSE 0 END), 0),
                coalesce(sum(CASE WHEN {archived_condition} THEN 1 ELSE 0 END), 0)
         FROM threads"
    );
    Ok(conn.query_row(&sql, [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?)
}

fn hidden_delete_plan(paths: &CodexPaths) -> Result<HiddenThreadDeletePlan> {
    let db = paths.state_db();
    let conn = Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("open {}", db.display()))?;
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        anyhow::bail!("sqlite integrity check failed: {integrity}");
    }

    let rows = hidden_visibility_rows(&conn)?;
    let mut visible_threads = 0;
    let mut hidden_ids = Vec::new();
    let mut candidates = Vec::new();
    let mut source_counts = BTreeMap::new();
    let mut archived_threads = 0;
    for row in &rows {
        if row.archived {
            archived_threads += 1;
        } else if let Some(category) = &row.hidden_category {
            hidden_ids.push(row.id.clone());
            let candidate = match crate::selected_codex::preview_hidden(paths, &row.id) {
                Ok(preview) => HiddenThreadCandidate {
                    id: row.id.clone(),
                    title: preview.title,
                    fingerprint: Some(preview.fingerprint),
                    allowed: true,
                    reason: None,
                },
                Err(error) => HiddenThreadCandidate {
                    id: row.id.clone(),
                    title: row.title.clone(),
                    fingerprint: None,
                    allowed: false,
                    reason: Some(error.to_string()),
                },
            };
            candidates.push(candidate);
            *source_counts.entry(category.clone()).or_insert(0) += 1;
        } else {
            visible_threads += 1;
        }
    }
    Ok(HiddenThreadDeletePlan {
        total_threads: rows.len() as u64,
        visible_threads,
        hidden_threads: hidden_ids.len() as u64,
        archived_threads,
        session_index_lines: count_lines(&paths.session_index())?,
        rollout_files: count_rollout_files(&paths.sessions_dir())?,
        hidden_ids,
        candidates,
        source_counts,
        integrity,
    })
}

#[derive(Debug)]
struct HiddenVisibilityRow {
    id: String,
    title: String,
    archived: bool,
    hidden_category: Option<String>,
}

fn hidden_visibility_rows(conn: &Connection) -> Result<Vec<HiddenVisibilityRow>> {
    let columns = table_columns(conn, "threads")?;
    let archived_condition = archived_sql_condition(&columns);
    let updated_expr = first_existing(&columns, &["updated_at", "last_activity_at", "created_at"])
        .unwrap_or("id")
        .to_string();
    let thread_source_expr = first_existing(&columns, &["thread_source", "source_kind"])
        .or_else(|| first_existing(&columns, &["source"]))
        .unwrap_or("NULL")
        .to_string();
    let source_expr = first_existing(&columns, &["source"])
        .unwrap_or("NULL")
        .to_string();
    let parent_thread_expr = first_existing(&columns, &["parent_thread_id", "parentThreadId"])
        .unwrap_or("NULL")
        .to_string();
    let agent_path_expr = first_existing(&columns, &["agent_path", "agentPath"])
        .unwrap_or("NULL")
        .to_string();
    let agent_nickname_expr = first_existing(&columns, &["agent_nickname", "agentNickname"])
        .unwrap_or("NULL")
        .to_string();
    let agent_role_expr = first_existing(&columns, &["agent_role", "agentRole"])
        .unwrap_or("NULL")
        .to_string();
    let has_user_event_expr = first_existing(&columns, &["has_user_event"])
        .unwrap_or("NULL")
        .to_string();
    let title_expr = first_existing(&columns, &["title", "name"])
        .unwrap_or("NULL")
        .to_string();
    let first_user_message_expr = first_existing(&columns, &["first_user_message"])
        .unwrap_or("NULL")
        .to_string();
    let preview_expr = first_existing(&columns, &["preview"])
        .unwrap_or("NULL")
        .to_string();
    let sql = format!(
        "SELECT id, CASE WHEN {archived_condition} THEN 1 ELSE 0 END,
                {thread_source_expr}, {source_expr}, {parent_thread_expr}, {agent_path_expr},
                {agent_nickname_expr}, {agent_role_expr}, {has_user_event_expr}, {title_expr},
                {first_user_message_expr}, {preview_expr}
         FROM threads
         ORDER BY {updated_expr} DESC, id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let archived: i64 = row.get(1)?;
        let thread_source: Option<String> = row.get(2).ok();
        let source: Option<String> = row.get(3).ok();
        let parent_thread_id: Option<String> = row.get(4).ok();
        let agent_path: Option<String> = row.get(5).ok();
        let agent_nickname: Option<String> = row.get(6).ok();
        let agent_role: Option<String> = row.get(7).ok();
        let has_user_event: Option<i64> = row.get(8).ok();
        let title: Option<String> = row.get(9).ok();
        let first_user_message: Option<String> = row.get(10).ok();
        let preview: Option<String> = row.get(11).ok();
        let hidden_category = hidden_thread_metadata_category(ThreadVisibilityMetadata {
            thread_source: thread_source.as_deref(),
            source: source.as_deref(),
            parent_thread_id: parent_thread_id.as_deref(),
            agent_path: agent_path.as_deref(),
            agent_nickname: agent_nickname.as_deref(),
            agent_role: agent_role.as_deref(),
            has_user_event,
            title: title.as_deref(),
            first_user_message: first_user_message.as_deref(),
            preview: preview.as_deref(),
        });
        Ok(HiddenVisibilityRow {
            title: title.unwrap_or_else(|| id.clone()),
            id,
            archived: archived != 0,
            hidden_category,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

pub(crate) fn prepare_delete_threads(conn: &Connection, ids: &[String]) -> Result<()> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS temp.delete_threads;
         CREATE TEMP TABLE delete_threads(id TEXT PRIMARY KEY);",
    )?;
    let mut stmt = conn.prepare("INSERT INTO delete_threads(id) VALUES(?1)")?;
    for id in ids {
        stmt.execute([id])?;
    }
    Ok(())
}

fn delete_threads_by_id(conn: &mut Connection, ids: &[String]) -> Result<()> {
    conn.execute_batch("PRAGMA busy_timeout=10000;")?;
    prepare_delete_threads(conn, ids)?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    cleanup_selected_threads(&tx)?;
    tx.commit()?;
    conn.execute_batch("VACUUM")?;
    Ok(())
}

pub(crate) fn cleanup_selected_threads(conn: &Connection) -> Result<()> {
    if table_exists(conn, "thread_dynamic_tools")?
        && table_columns(conn, "thread_dynamic_tools")?.contains("thread_id")
    {
        conn.execute(
            "DELETE FROM thread_dynamic_tools WHERE thread_id IN (SELECT id FROM delete_threads)",
            [],
        )?;
    }
    if table_exists(conn, "thread_spawn_edges")? {
        let columns = table_columns(conn, "thread_spawn_edges")?;
        if columns.contains("parent_thread_id") && columns.contains("child_thread_id") {
            conn.execute(
                "DELETE FROM thread_spawn_edges
                 WHERE parent_thread_id IN (SELECT id FROM delete_threads)
                    OR child_thread_id IN (SELECT id FROM delete_threads)",
                [],
            )?;
        }
    }
    if table_exists(conn, "agent_job_items")? {
        let columns = table_columns(conn, "agent_job_items")?;
        if columns.contains("assigned_thread_id") {
            conn.execute(
                "UPDATE agent_job_items
                 SET assigned_thread_id = NULL
                 WHERE assigned_thread_id IN (SELECT id FROM delete_threads)",
                [],
            )?;
        }
    }
    conn.execute(
        "DELETE FROM threads WHERE id IN (SELECT id FROM delete_threads)",
        [],
    )?;
    Ok(())
}

pub(crate) fn hidden_candidate_ids(conn: &Connection) -> Result<HashSet<String>> {
    Ok(hidden_visibility_rows(conn)?
        .into_iter()
        .filter(|row| !row.archived && row.hidden_category.is_some())
        .map(|row| row.id)
        .collect())
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
        [table],
        |row| row.get::<_, i64>(0),
    )? > 0)
}

pub(crate) fn table_columns(conn: &Connection, table: &str) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    Ok(rows.collect::<rusqlite::Result<HashSet<_>>>()?)
}

fn first_existing<'a>(columns: &HashSet<String>, names: &'a [&str]) -> Option<&'a str> {
    names.iter().copied().find(|name| columns.contains(*name))
}

fn archived_sql_condition(columns: &HashSet<String>) -> String {
    let mut conditions = Vec::new();
    if columns.contains("archived") {
        conditions.push("coalesce(archived, 0) != 0".to_string());
    }
    if columns.contains("archived_at") {
        conditions.push("archived_at IS NOT NULL".to_string());
    }
    if conditions.is_empty() {
        "0".to_string()
    } else {
        conditions.join(" OR ")
    }
}

fn count_lines(path: &Path) -> Result<u64> {
    if !path.exists() {
        return Ok(0);
    }
    Ok(fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?
        .lines()
        .count() as u64)
}

fn count_rollout_files(path: &Path) -> Result<u64> {
    if !path.exists() {
        return Ok(0);
    }
    Ok(WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_file()
                && entry
                    .file_name()
                    .to_str()
                    .map(|name| name.starts_with("rollout-") && name.ends_with(".jsonl"))
                    .unwrap_or(false)
        })
        .count() as u64)
}

fn rewrite_session_index(path: &Path, archived_ids: &[String]) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let delete_ids = archived_ids
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    let mut kept = Vec::new();
    for line in fs::read_to_string(path)?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let keep = serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|v| {
                v.get("id")
                    .and_then(|id| id.as_str())
                    .map(|id| !delete_ids.contains(id))
            })
            .unwrap_or(true);
        if keep {
            kept.push(line.to_string());
        }
    }
    let tmp = tmp_path(path);
    fs::write(&tmp, kept.join("\n") + "\n")?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn delete_rollouts(path: &Path, archived_ids: &[String]) -> Result<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let mut deleted = 0;
    for entry in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        let name = p.file_name().and_then(|v| v.to_str()).unwrap_or("");
        if archived_ids.iter().any(|id| name.contains(id)) {
            fs::remove_file(p)?;
            deleted += 1;
        }
    }
    remove_empty_dirs(path)?;
    Ok(deleted)
}

fn remove_empty_dirs(path: &Path) -> Result<()> {
    let mut dirs = WalkDir::new(path)
        .min_depth(1)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_dir())
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    dirs.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for dir in dirs {
        let _ = fs::remove_dir(&dir);
    }
    Ok(())
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut tmp = path.to_path_buf();
    tmp.set_extension("tmp");
    tmp
}

#[cfg(test)]
mod tests {
    use super::{
        execute_delete_hidden, plan_delete_hidden, ArchiveDeletePlan, HiddenThreadDeleteResult,
        HiddenThreadSelection,
    };
    use crate::codex::CodexPaths;
    use rusqlite::Connection;
    use serde_json::json;
    use std::{
        env, fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn plan_serializes_counts() {
        let plan = ArchiveDeletePlan {
            total_threads: 1,
            active_threads: 1,
            archived_threads: 0,
            session_index_lines: 1,
            rollout_files: 1,
            archived_ids: Vec::new(),
            integrity: "ok".to_string(),
        };
        assert!(serde_json::to_string(&plan)
            .unwrap()
            .contains("total_threads"));
    }

    #[test]
    fn plan_delete_hidden_selects_non_archived_hidden_threads() {
        let root = hidden_cleanup_fixture("plan-hidden");

        let plan = plan_delete_hidden(&CodexPaths::new(&root)).unwrap();

        assert_eq!(plan.total_threads, 4);
        assert_eq!(plan.visible_threads, 1);
        assert_eq!(plan.hidden_threads, 2);
        assert_eq!(plan.archived_threads, 1);
        assert_eq!(
            plan.hidden_ids,
            vec!["hidden-internal".to_string(), "hidden-subagent".to_string()]
        );
        assert_eq!(plan.source_counts.get("internal").copied(), Some(1));
        assert_eq!(plan.source_counts.get("subagent").copied(), Some(1));
        assert!(!plan.hidden_ids.contains(&"archived-hidden".to_string()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn execute_delete_hidden_removes_only_hidden_and_preserves_visible_count() {
        let root = hidden_cleanup_fixture("execute-hidden");
        let paths = CodexPaths::new(&root);
        let plan = plan_delete_hidden(&paths).unwrap();
        let candidates = selections_for_plan(&plan);

        let result = execute_delete_hidden(&paths, &candidates).unwrap();

        assert_hidden_result_preserved_visible(&result);
        assert_eq!(result.before.hidden_threads, 2);
        assert_eq!(result.after_hidden_threads, 0);
        assert_eq!(result.after_archived_threads, 1);
        assert_eq!(result.deleted_rollout_files, 2);

        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        assert!(thread_exists(&conn, "main-visible"));
        assert!(!thread_exists(&conn, "hidden-subagent"));
        assert!(!thread_exists(&conn, "hidden-internal"));
        assert!(thread_exists(&conn, "archived-hidden"));
        assert_eq!(count_rows(&conn, "thread_dynamic_tools"), 1);
        assert_eq!(count_rows(&conn, "thread_spawn_edges"), 0);
        let assigned: Option<String> = conn
            .query_row(
                "SELECT assigned_thread_id FROM agent_job_items WHERE id='job-hidden-subagent'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(assigned, None);
        let index = fs::read_to_string(root.join("session_index.jsonl")).unwrap();
        assert!(index.contains("main-visible"));
        assert!(index.contains("archived-hidden"));
        assert!(!index.contains("hidden-subagent"));
        assert!(!index.contains("hidden-internal"));
        assert!(root
            .join("sessions")
            .join("rollout-main-visible.jsonl")
            .exists());
        assert!(root
            .join("sessions")
            .join("rollout-archived-hidden.jsonl")
            .exists());
        assert!(!root
            .join("sessions")
            .join("rollout-hidden-subagent.jsonl")
            .exists());
        assert!(!root
            .join("sessions")
            .join("rollout-hidden-internal.jsonl")
            .exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn execute_delete_hidden_skips_running_candidate_and_deletes_safe_candidate() {
        let root = hidden_cleanup_fixture("running-hidden");
        fs::write(
            root.join("sessions").join("rollout-hidden-subagent.jsonl"),
            [
                json!({"type":"session_meta","payload":{"id":"hidden-subagent"}}).to_string(),
                json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-live"}})
                    .to_string(),
            ]
            .join("\n"),
        )
        .unwrap();
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        conn.execute(
            "UPDATE threads SET updated_at=?1 WHERE id='hidden-subagent'",
            [current_unix_seconds()],
        )
        .unwrap();
        let paths = CodexPaths::new(&root);
        let plan = plan_delete_hidden(&paths).unwrap();
        assert!(!candidate(&plan, "hidden-subagent").allowed);
        let candidates = selections_for_plan(&plan);

        let result = execute_delete_hidden(&paths, &candidates).unwrap();

        assert_eq!(result.deleted_threads, 1);
        assert_eq!(result.skipped_threads, 1);
        assert_eq!(result.failed_threads, 0);
        assert!(thread_exists(&conn, "hidden-subagent"));
        assert!(!thread_exists(&conn, "hidden-internal"));
        assert!(root
            .join("sessions")
            .join("rollout-hidden-subagent.jsonl")
            .exists());

        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn execute_delete_hidden_protects_stale_running_rollout() {
        let root = hidden_cleanup_fixture("stale-running-hidden");
        fs::write(
            root.join("sessions").join("rollout-hidden-subagent.jsonl"),
            [
                json!({"type":"session_meta","payload":{"id":"hidden-subagent"}}).to_string(),
                json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-stale"}}).to_string(),
                json!({"type":"response_item","payload":{"type":"tool_search_call","call_id":"call-stale"}}).to_string(),
                json!({"type":"response_item","payload":{"type":"tool_search_output","call_id":"call-stale"}}).to_string(),
            ]
            .join("\n"),
        )
        .unwrap();
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        conn.execute(
            "UPDATE threads SET updated_at=?1 WHERE id='hidden-subagent'",
            [1_i64],
        )
        .unwrap();
        let paths = CodexPaths::new(&root);
        let plan = plan_delete_hidden(&paths).unwrap();
        assert!(!candidate(&plan, "hidden-subagent").allowed);
        let candidates = selections_for_plan(&plan);

        let result = execute_delete_hidden(&paths, &candidates).unwrap();

        assert_hidden_result_preserved_visible(&result);
        assert_eq!(result.before.hidden_threads, 2);
        assert_eq!(result.deleted_threads, 1);
        assert_eq!(result.skipped_threads, 1);
        assert_eq!(result.after_hidden_threads, 1);
        assert!(thread_exists(&conn, "hidden-subagent"));
        assert!(root
            .join("sessions")
            .join("rollout-hidden-subagent.jsonl")
            .exists());

        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn execute_delete_hidden_allows_aborted_candidate() {
        let root = hidden_cleanup_fixture("aborted-hidden");
        fs::write(
            root.join("sessions").join("rollout-hidden-subagent.jsonl"),
            [
                json!({"type":"session_meta","payload":{"id":"hidden-subagent"}}).to_string(),
                json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-abort"}}).to_string(),
                json!({"type":"turn_started","turn_id":"turn-abort"}).to_string(),
                json!({"type":"response_item","turn_id":"turn-abort","payload":{"type":"function_call","name":"exec_command","call_id":"call-abort","arguments":{"cmd":"sleep 10"}}}).to_string(),
                json!({"type":"event_msg","payload":{"type":"turn_aborted","turn_id":"turn-abort","reason":"interrupted"}}).to_string(),
            ]
            .join("\n"),
        )
        .unwrap();
        let paths = CodexPaths::new(&root);
        let plan = plan_delete_hidden(&paths).unwrap();
        let candidates = selections_for_plan(&plan);

        let result = execute_delete_hidden(&paths, &candidates).unwrap();

        assert_hidden_result_preserved_visible(&result);
        assert_eq!(result.before.hidden_threads, 2);
        assert_eq!(result.after_hidden_threads, 0);
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        assert!(!thread_exists(&conn, "hidden-subagent"));
        assert!(!root
            .join("sessions")
            .join("rollout-hidden-subagent.jsonl")
            .exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hidden_delete_rechecks_changed_fingerprint_and_continues_with_other_candidates() {
        let root = hidden_cleanup_fixture("changed-fingerprint");
        let paths = CodexPaths::new(&root);
        let plan = plan_delete_hidden(&paths).unwrap();
        let candidates = vec![
            selection(&plan, "hidden-internal"),
            selection(&plan, "hidden-subagent"),
        ];
        fs::write(
            root.join("sessions").join("rollout-hidden-internal.jsonl"),
            valid_completed_rollout("hidden-internal") + "\n",
        )
        .unwrap();

        let result = execute_delete_hidden(&paths, &candidates).unwrap();

        assert_eq!(result.deleted_threads, 1);
        assert_eq!(result.skipped_threads, 1);
        assert_eq!(
            result
                .items
                .iter()
                .find(|item| item.id == "hidden-internal")
                .unwrap()
                .status,
            "skipped"
        );
        assert_eq!(
            result
                .items
                .iter()
                .find(|item| item.id == "hidden-subagent")
                .unwrap()
                .status,
            "deleted"
        );
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        assert!(thread_exists(&conn, "hidden-internal"));
        assert!(!thread_exists(&conn, "hidden-subagent"));
        assert!(root
            .join("sessions")
            .join("rollout-hidden-internal.jsonl")
            .exists());
        assert!(!root
            .join("sessions")
            .join("rollout-hidden-subagent.jsonl")
            .exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hidden_delete_skips_a_candidate_thread_deleted_after_preview() {
        let root = hidden_cleanup_fixture("missing-candidate-id");
        let paths = CodexPaths::new(&root);
        let plan = plan_delete_hidden(&paths).unwrap();
        let candidate = selection(&plan, "hidden-subagent");
        Connection::open(root.join("state_5.sqlite"))
            .unwrap()
            .execute("DELETE FROM threads WHERE id='hidden-subagent'", [])
            .unwrap();

        let result = execute_delete_hidden(&paths, &[candidate]).unwrap();

        assert_eq!(result.skipped_threads, 1);
        assert_eq!(result.deleted_threads, 0);
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        assert!(!thread_exists(&conn, "hidden-subagent"));
        assert!(thread_exists(&conn, "hidden-internal"));
        assert!(root
            .join("sessions")
            .join("rollout-hidden-subagent.jsonl")
            .exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hidden_delete_skips_missing_corrupt_and_shared_rollouts() {
        for case in ["missing", "corrupt", "shared"] {
            let root = hidden_cleanup_fixture(case);
            let paths = CodexPaths::new(&root);
            let plan = plan_delete_hidden(&paths).unwrap();
            let candidate = selection(&plan, "hidden-subagent");
            let rollout = root.join("sessions").join("rollout-hidden-subagent.jsonl");
            match case {
                "missing" => fs::remove_file(&rollout).unwrap(),
                "corrupt" => fs::write(&rollout, "{broken jsonl").unwrap(),
                "shared" => {
                    Connection::open(root.join("state_5.sqlite"))
                        .unwrap()
                        .execute(
                            "UPDATE threads SET rollout_path=?1 WHERE id='main-visible'",
                            [rollout.display().to_string()],
                        )
                        .unwrap();
                }
                _ => unreachable!(),
            }

            let result = execute_delete_hidden(&paths, &[candidate]).unwrap();

            assert_eq!(result.skipped_threads, 1, "{case}");
            assert_eq!(result.deleted_threads, 0, "{case}");
            let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
            assert!(thread_exists(&conn, "hidden-subagent"), "{case}");
            assert_eq!(rollout.exists(), case != "missing", "{case}");
            let _ = fs::remove_dir_all(root);
        }
    }

    #[cfg(unix)]
    #[test]
    fn hidden_delete_skips_symlink_rollouts_without_touching_the_target() {
        use std::os::unix::fs::symlink;
        let root = hidden_cleanup_fixture("symlink");
        let paths = CodexPaths::new(&root);
        let plan = plan_delete_hidden(&paths).unwrap();
        let candidate = selection(&plan, "hidden-subagent");
        let rollout = root.join("sessions").join("rollout-hidden-subagent.jsonl");
        let target = root.join("sessions").join("rollout-main-visible.jsonl");
        fs::remove_file(&rollout).unwrap();
        symlink(&target, &rollout).unwrap();

        let result = execute_delete_hidden(&paths, &[candidate]).unwrap();

        assert_eq!(result.skipped_threads, 1);
        assert!(thread_exists(
            &Connection::open(root.join("state_5.sqlite")).unwrap(),
            "hidden-subagent"
        ));
        assert!(fs::symlink_metadata(&rollout)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(target.is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hidden_delete_does_not_add_threads_created_after_preview_to_candidates() {
        let root = hidden_cleanup_fixture("late-hidden");
        let paths = CodexPaths::new(&root);
        let plan = plan_delete_hidden(&paths).unwrap();
        let candidates = selections_for_plan(&plan);
        append_hidden_thread(&root, "late-hidden", 50);

        let result = execute_delete_hidden(&paths, &candidates).unwrap();

        assert_eq!(result.deleted_threads, 2);
        assert_eq!(result.after_hidden_threads, 1);
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        assert!(thread_exists(&conn, "late-hidden"));
        assert!(root
            .join("sessions")
            .join("rollout-late-hidden.jsonl")
            .exists());
        assert!(fs::read_to_string(root.join("session_index.jsonl"))
            .unwrap()
            .contains("late-hidden"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hidden_delete_accepts_more_than_one_hundred_preview_candidates() {
        let root = hidden_cleanup_fixture("over-one-hundred");
        for index in 0..101 {
            append_hidden_thread(&root, &format!("extra-{index:03}"), 100 + index as i64);
        }
        let paths = CodexPaths::new(&root);
        let plan = plan_delete_hidden(&paths).unwrap();
        let candidates = selections_for_plan(&plan);
        assert!(candidates.len() > 100);

        let result = execute_delete_hidden(&paths, &candidates).unwrap();

        assert_eq!(result.deleted_threads as usize, candidates.len());
        assert_eq!(result.skipped_threads, 0);
        assert_eq!(result.failed_threads, 0);
        assert_eq!(result.after_hidden_threads, 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hidden_delete_continues_after_a_single_item_database_failure() {
        let root = hidden_cleanup_fixture("item-database-failure");
        let paths = CodexPaths::new(&root);
        let plan = plan_delete_hidden(&paths).unwrap();
        let candidates = vec![
            selection(&plan, "hidden-internal"),
            selection(&plan, "hidden-subagent"),
        ];
        Connection::open(root.join("state_5.sqlite"))
            .unwrap()
            .execute_batch("CREATE TRIGGER fail_one_hidden BEFORE DELETE ON threads WHEN OLD.id='hidden-internal' BEGIN SELECT RAISE(ABORT,'injected item failure'); END;")
            .unwrap();

        let result = execute_delete_hidden(&paths, &candidates).unwrap();

        assert_eq!(result.failed_threads, 1);
        assert_eq!(result.deleted_threads, 1);
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        assert!(thread_exists(&conn, "hidden-internal"));
        assert!(!thread_exists(&conn, "hidden-subagent"));
        assert!(root
            .join("sessions")
            .join("rollout-hidden-internal.jsonl")
            .exists());
        assert!(!root
            .join("sessions")
            .join("rollout-hidden-subagent.jsonl")
            .exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hidden_delete_rolls_back_all_candidates_on_a_system_database_failure() {
        let root = hidden_cleanup_fixture("system-database-failure");
        let paths = CodexPaths::new(&root);
        let plan = plan_delete_hidden(&paths).unwrap();
        let candidates = vec![
            selection(&plan, "hidden-internal"),
            selection(&plan, "hidden-subagent"),
        ];
        let index_before = fs::read_to_string(paths.session_index()).unwrap();
        Connection::open(root.join("state_5.sqlite"))
            .unwrap()
            .execute_batch("CREATE TRIGGER rollback_hidden_batch BEFORE DELETE ON threads WHEN OLD.id='hidden-subagent' BEGIN SELECT RAISE(ROLLBACK,'injected batch failure'); END;")
            .unwrap();

        let error = execute_delete_hidden(&paths, &candidates).unwrap_err();

        assert!(error.to_string().contains("回滚"));
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        assert!(thread_exists(&conn, "hidden-internal"));
        assert!(thread_exists(&conn, "hidden-subagent"));
        assert!(root
            .join("sessions")
            .join("rollout-hidden-internal.jsonl")
            .exists());
        assert!(root
            .join("sessions")
            .join("rollout-hidden-subagent.jsonl")
            .exists());
        assert_eq!(
            fs::read_to_string(paths.session_index()).unwrap(),
            index_before
        );
        let _ = fs::remove_dir_all(root);
    }

    fn assert_hidden_result_preserved_visible(result: &HiddenThreadDeleteResult) {
        assert_eq!(
            result.after_visible_threads, result.before.visible_threads,
            "visible thread count changed"
        );
    }

    fn hidden_cleanup_fixture(label: &str) -> PathBuf {
        let root = unique_temp_dir(label);
        let sessions = root.join("sessions");
        fs::create_dir_all(&sessions).unwrap();
        for id in [
            "main-visible",
            "hidden-subagent",
            "hidden-internal",
            "archived-hidden",
        ] {
            fs::write(
                sessions.join(format!("rollout-{id}.jsonl")),
                valid_completed_rollout(id),
            )
            .unwrap();
        }
        fs::write(
            root.join("session_index.jsonl"),
            [
                session_index_line(&root, "main-visible"),
                session_index_line(&root, "hidden-subagent"),
                session_index_line(&root, "hidden-internal"),
                session_index_line(&root, "archived-hidden"),
            ]
            .join("\n"),
        )
        .unwrap();
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        conn.execute_batch(
            "CREATE TABLE threads(
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
                has_user_event INTEGER NOT NULL,
                first_user_message TEXT NOT NULL,
                preview TEXT NOT NULL,
                cwd TEXT NOT NULL,
                title TEXT NOT NULL,
                archived INTEGER NOT NULL DEFAULT 0,
                archived_at INTEGER
            );
            CREATE TABLE thread_dynamic_tools(thread_id TEXT NOT NULL);
            CREATE TABLE thread_spawn_edges(parent_thread_id TEXT NOT NULL, child_thread_id TEXT NOT NULL);
            CREATE TABLE agent_job_items(id TEXT PRIMARY KEY, assigned_thread_id TEXT, status TEXT NOT NULL);",
        )
        .unwrap();
        insert_thread(
            &conn,
            &root,
            ThreadFixtureRow {
                id: "main-visible",
                source: "codex",
                thread_source: "user",
                parent_thread_id: None,
                updated_at: 10,
                archived: 0,
            },
        );
        insert_thread(
            &conn,
            &root,
            ThreadFixtureRow {
                id: "hidden-subagent",
                source: "codex",
                thread_source: "subagent",
                parent_thread_id: Some("main-visible"),
                updated_at: 20,
                archived: 0,
            },
        );
        insert_thread(
            &conn,
            &root,
            ThreadFixtureRow {
                id: "hidden-internal",
                source: "exec",
                thread_source: "user",
                parent_thread_id: None,
                updated_at: 30,
                archived: 0,
            },
        );
        insert_thread(
            &conn,
            &root,
            ThreadFixtureRow {
                id: "archived-hidden",
                source: "codex",
                thread_source: "subagent",
                parent_thread_id: Some("main-visible"),
                updated_at: 40,
                archived: 1,
            },
        );
        conn.execute(
            "INSERT INTO thread_dynamic_tools(thread_id) VALUES('main-visible'), ('hidden-subagent'), ('hidden-internal')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO thread_spawn_edges(parent_thread_id, child_thread_id) VALUES('main-visible', 'hidden-subagent'), ('hidden-subagent', 'hidden-internal')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO agent_job_items(id, assigned_thread_id, status) VALUES('job-hidden-subagent', 'hidden-subagent', 'completed')",
            [],
        )
        .unwrap();
        root
    }

    struct ThreadFixtureRow<'a> {
        id: &'a str,
        source: &'a str,
        thread_source: &'a str,
        parent_thread_id: Option<&'a str>,
        updated_at: i64,
        archived: i64,
    }

    fn insert_thread(conn: &Connection, root: &Path, row: ThreadFixtureRow<'_>) {
        let rollout = root
            .join("sessions")
            .join(format!("rollout-{}.jsonl", row.id));
        let first_user_message = if row.id == "hidden-internal" {
            "read-only subagent verification"
        } else {
            "user request"
        };
        conn.execute(
            "INSERT INTO threads(
                id, rollout_path, created_at, updated_at, source, thread_source, parent_thread_id,
                agent_path, agent_nickname, agent_role, has_user_event, first_user_message,
                preview, cwd, title, archived, archived_at
             ) VALUES(?1, ?2, 1, ?3, ?4, ?5, ?6, NULL, NULL, NULL, ?7, ?8, ?8, '/tmp', ?1, ?9, ?10)",
            (
                row.id,
                rollout.display().to_string(),
                row.updated_at,
                row.source,
                row.thread_source,
                row.parent_thread_id,
                if row.id == "hidden-internal" { 0 } else { 1 },
                first_user_message,
                row.archived,
                if row.archived == 0 {
                    None
                } else {
                    Some(1_i64)
                },
            ),
        )
        .unwrap();
    }

    fn valid_completed_rollout(id: &str) -> String {
        [
            json!({"type":"session_meta","payload":{"id":id}}).to_string(),
            json!({"type":"turn_started","turn_id":"turn-fixture"}).to_string(),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-fixture"}})
                .to_string(),
        ]
        .join("\n")
    }

    fn append_hidden_thread(root: &Path, id: &str, updated_at: i64) {
        let rollout = root.join("sessions").join(format!("rollout-{id}.jsonl"));
        fs::write(&rollout, valid_completed_rollout(id)).unwrap();
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        insert_thread(
            &conn,
            root,
            ThreadFixtureRow {
                id,
                source: "codex",
                thread_source: "subagent",
                parent_thread_id: Some("main-visible"),
                updated_at,
                archived: 0,
            },
        );
        let index_path = root.join("session_index.jsonl");
        let mut index = fs::read_to_string(&index_path).unwrap();
        if !index.is_empty() {
            index.push('\n');
        }
        index.push_str(&session_index_line(root, id));
        fs::write(index_path, index).unwrap();
    }

    fn candidate<'a>(
        plan: &'a super::HiddenThreadDeletePlan,
        id: &str,
    ) -> &'a super::HiddenThreadCandidate {
        plan.candidates
            .iter()
            .find(|candidate| candidate.id == id)
            .unwrap()
    }

    fn selection(plan: &super::HiddenThreadDeletePlan, id: &str) -> HiddenThreadSelection {
        let candidate = candidate(plan, id);
        HiddenThreadSelection {
            id: candidate.id.clone(),
            fingerprint: candidate.fingerprint.clone(),
        }
    }

    fn selections_for_plan(plan: &super::HiddenThreadDeletePlan) -> Vec<HiddenThreadSelection> {
        plan.candidates
            .iter()
            .map(|candidate| HiddenThreadSelection {
                id: candidate.id.clone(),
                fingerprint: candidate.fingerprint.clone(),
            })
            .collect()
    }

    fn session_index_line(root: &Path, id: &str) -> String {
        json!({
            "id": id,
            "path": root.join("sessions").join(format!("rollout-{id}.jsonl")),
        })
        .to_string()
    }

    fn thread_exists(conn: &Connection, id: &str) -> bool {
        conn.query_row("SELECT count(*) FROM threads WHERE id=?1", [id], |row| {
            row.get::<_, u64>(0)
        })
        .unwrap()
            > 0
    }

    fn count_rows(conn: &Connection, table: &str) -> u64 {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
    }

    fn current_unix_seconds() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        env::temp_dir().join(format!(
            "nexushub-archive-{label}-{}-{counter}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }
}
