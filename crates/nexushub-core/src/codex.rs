use anyhow::{Context, Result};
use chrono::{DateTime, Local, TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ThreadStatus {
    Recent,
    Running,
    ReplyNeeded,
    Recoverable,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub id: String,
    pub title: String,
    pub status: ThreadStatus,
    pub updated_at: Option<String>,
    pub archived_at: Option<String>,
    pub message_count: usize,
    pub latest_message: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub rollout_path: Option<PathBuf>,
    pub active_turn_id: Option<String>,
    pub active_job_id: Option<String>,
    pub pending_elicitation: Option<PendingElicitation>,
    pub last_event_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadDetail {
    pub summary: ThreadSummary,
    pub messages: Vec<CodexMessage>,
    pub blocks: Vec<MessageBlock>,
    pub raw_event_count: usize,
    pub total_blocks: usize,
    pub has_more_blocks: bool,
    pub before_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexMessage {
    pub role: String,
    pub kind: String,
    pub text: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PendingElicitation {
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub questions: Vec<UserInputQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserInputQuestion {
    pub id: String,
    pub header: Option<String>,
    pub question: String,
    pub options: Vec<UserInputOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserInputOption {
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserInputAnswer {
    pub question_id: String,
    pub answers: Vec<String>,
    pub note: Option<String>,
}

pub fn extract_proposed_plan_text(text: &str) -> Option<String> {
    let (_, after_start) = text.split_once("<proposed_plan>")?;
    let plan = after_start
        .split_once("</proposed_plan>")
        .map(|(body, _)| body)
        .unwrap_or(after_start)
        .trim();
    (!plan.is_empty()).then(|| plan.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageBlock {
    pub id: String,
    pub role: String,
    pub kind: String,
    pub display_kind: Option<String>,
    pub status: Option<String>,
    pub text: Option<String>,
    pub summary: Option<String>,
    pub input: Option<String>,
    pub truncated: Option<bool>,
    pub resolved: Option<bool>,
    pub answers: Vec<UserInputAnswer>,
    pub plan_status: Option<String>,
    pub group_id: Option<String>,
    pub tool_name: Option<String>,
    pub call_id: Option<String>,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub created_at: Option<String>,
    pub questions: Vec<UserInputQuestion>,
    pub payload: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct CodexPaths {
    pub home: PathBuf,
}

impl CodexPaths {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }

    pub fn state_db(&self) -> PathBuf {
        self.home.join("state_5.sqlite")
    }

    pub fn session_index(&self) -> PathBuf {
        self.home.join("session_index.jsonl")
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.home.join("sessions")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedCodexPaths {
    pub configured_codex_home: Option<String>,
    pub home: PathBuf,
    pub logs_db: PathBuf,
    pub state_db: PathBuf,
    pub session_index: PathBuf,
    pub sessions_dir: PathBuf,
    pub configured_app_server_socket: Option<PathBuf>,
    pub app_server_socket: Option<PathBuf>,
    pub codex_home_source: String,
    pub logs_db_source: String,
    pub app_server_socket_source: Option<String>,
    pub discovery_warnings: Vec<String>,
}

impl ResolvedCodexPaths {
    pub fn codex_paths(&self) -> CodexPaths {
        CodexPaths::new(&self.home)
    }
}

#[derive(Debug, Clone)]
pub struct CodexPathDiscoveryOptions {
    pub env_codex_home: Option<PathBuf>,
    pub current_user_home: Option<PathBuf>,
    pub root_codex_home: PathBuf,
    pub ubuntu_codex_home: PathBuf,
    pub home_scan_root: PathBuf,
}

impl Default for CodexPathDiscoveryOptions {
    fn default() -> Self {
        Self {
            env_codex_home: env::var_os("CODEX_HOME").map(PathBuf::from),
            current_user_home: dirs::home_dir(),
            root_codex_home: PathBuf::from("/root/.codex"),
            ubuntu_codex_home: PathBuf::from("/home/ubuntu/.codex"),
            home_scan_root: PathBuf::from("/home"),
        }
    }
}

pub fn resolve_codex_paths(
    configured_home: &Path,
    configured_app_server_socket: Option<&Path>,
) -> ResolvedCodexPaths {
    resolve_codex_paths_with_options(
        configured_home,
        configured_app_server_socket,
        &CodexPathDiscoveryOptions::default(),
    )
}

pub fn resolve_codex_paths_with_options(
    configured_home: &Path,
    configured_app_server_socket: Option<&Path>,
    options: &CodexPathDiscoveryOptions,
) -> ResolvedCodexPaths {
    let configured_codex_home = configured_path_value(configured_home);
    let mut warnings = Vec::new();
    let configured_candidate = (!is_auto_path(configured_home)).then(|| {
        (
            configured_home.to_path_buf(),
            "configured",
            "configured Codex home is not valid",
        )
    });
    let socket_home = configured_app_server_socket.and_then(codex_home_from_app_server_socket_path);

    let mut candidates: Vec<(PathBuf, &'static str, &'static str)> = Vec::new();
    if let Some(candidate) = configured_candidate {
        candidates.push(candidate);
    }
    if let Some(path) = options
        .env_codex_home
        .as_deref()
        .filter(|path| !is_auto_path(path))
    {
        candidates.push((
            path.to_path_buf(),
            "env:CODEX_HOME",
            "CODEX_HOME is not a valid Codex home",
        ));
    }
    if let Some(path) = socket_home {
        candidates.push((
            path,
            "socket",
            "app-server socket did not resolve to a valid Codex home",
        ));
    }
    if let Some(path) = options.current_user_home.as_ref() {
        candidates.push((
            path.join(".codex"),
            "current_user",
            "current user ~/.codex is not a valid Codex home",
        ));
    }
    candidates.push((
        options.root_codex_home.clone(),
        "root",
        "/root/.codex is not a valid Codex home",
    ));
    candidates.push((
        options.ubuntu_codex_home.clone(),
        "home_ubuntu",
        "/home/ubuntu/.codex is not a valid Codex home",
    ));
    candidates.extend(
        scanned_home_codex_dirs(&options.home_scan_root)
            .into_iter()
            .map(|path| {
                (
                    path,
                    "home_scan",
                    "/home/*/.codex is not a valid Codex home",
                )
            }),
    );

    let mut selected: Option<(PathBuf, &'static str)> = None;
    for (path, source, invalid_message) in &candidates {
        if is_valid_codex_home(path) {
            selected = Some((path.clone(), *source));
            break;
        }
        if matches!(*source, "configured" | "env:CODEX_HOME" | "socket") {
            warnings.push(format!("{invalid_message}: {}", path.display()));
        }
    }

    let (home, codex_home_source) = selected.unwrap_or_else(|| {
        if !is_auto_path(configured_home) {
            warnings.push(format!(
                "no valid Codex home discovered; using configured path {}",
                configured_home.display()
            ));
            (configured_home.to_path_buf(), "fallback_configured")
        } else {
            warnings.push(format!(
                "no valid Codex home discovered; using {}",
                options.root_codex_home.display()
            ));
            (options.root_codex_home.clone(), "fallback_root")
        }
    });
    let codex_home_source = codex_home_source.to_string();
    let configured_app_server_socket = configured_app_server_socket
        .filter(|path| !is_auto_path(path))
        .map(Path::to_path_buf);
    let (app_server_socket, app_server_socket_source) =
        resolve_app_server_socket(&home, configured_app_server_socket.as_deref());

    ResolvedCodexPaths {
        configured_codex_home,
        logs_db: home.join("logs_2.sqlite"),
        state_db: home.join("state_5.sqlite"),
        session_index: home.join("session_index.jsonl"),
        sessions_dir: home.join("sessions"),
        configured_app_server_socket,
        app_server_socket,
        codex_home_source: codex_home_source.clone(),
        logs_db_source: codex_home_source,
        app_server_socket_source,
        discovery_warnings: warnings,
        home,
    }
}

fn resolve_app_server_socket(
    home: &Path,
    configured_app_server_socket: Option<&Path>,
) -> (Option<PathBuf>, Option<String>) {
    if let Some(socket) = configured_app_server_socket {
        return (Some(socket.to_path_buf()), Some("configured".to_string()));
    }
    (
        Some(
            home.join("app-server-control")
                .join("app-server-control.sock"),
        ),
        Some("resolved_codex_home".to_string()),
    )
}

fn is_auto_path(path: &Path) -> bool {
    let value = path.to_string_lossy();
    let trimmed = value.trim();
    trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto")
}

fn configured_path_value(path: &Path) -> Option<String> {
    let value = path.to_string_lossy();
    let trimmed = value.trim();
    (!trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("auto")).then(|| trimmed.to_string())
}

fn is_valid_codex_home(path: &Path) -> bool {
    path.is_dir()
        && [
            path.join("logs_2.sqlite"),
            path.join("state_5.sqlite"),
            path.join("session_index.jsonl"),
            path.join("sessions"),
            path.join("hooks.json"),
            path.join("app-server-control"),
        ]
        .iter()
        .any(|artifact| artifact.exists())
}

fn codex_home_from_app_server_socket_path(socket: &Path) -> Option<PathBuf> {
    let parent = socket.parent()?;
    if parent.file_name().and_then(|value| value.to_str()) == Some("app-server-control") {
        return parent.parent().map(Path::to_path_buf);
    }
    None
}

fn scanned_home_codex_dirs(home_root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(home_root) else {
        return Vec::new();
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            file_type.is_dir().then(|| entry.path().join(".codex"))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub fn list_threads(
    paths: &CodexPaths,
    status: Option<&str>,
    q: Option<&str>,
    limit: usize,
) -> Result<Vec<ThreadSummary>> {
    let mut rows = read_thread_rows(paths)?;
    let rollout_map = read_session_index(paths).unwrap_or_default();
    let mut hidden_subagents = HashSet::new();
    for row in &mut rows {
        if row.rollout_path.is_none() {
            row.rollout_path = rollout_map.get(&row.id).cloned();
        }
        if row.rollout_path.is_none() {
            row.rollout_path = find_rollout_path(paths, &row.id);
        }
        if enrich_thread_from_rollout(row).unwrap_or(false) {
            hidden_subagents.insert(row.id.clone());
        }
    }
    rows.retain(|row| !hidden_subagents.contains(&row.id));

    let needle = q
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty());
    rows.retain(|row| {
        if let Some(status) = status {
            if !matches_status(row, status) {
                return false;
            }
        }
        if let Some(needle) = &needle {
            let title = row.title.to_ascii_lowercase();
            let id = row.id.to_ascii_lowercase();
            let latest = row
                .latest_message
                .clone()
                .unwrap_or_default()
                .to_ascii_lowercase();
            title.contains(needle) || id.contains(needle) || latest.contains(needle)
        } else {
            true
        }
    });
    rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    rows.truncate(limit.max(1));
    Ok(rows)
}

pub fn hidden_thread_ids(paths: &CodexPaths) -> Result<HashSet<String>> {
    let db = paths.state_db();
    if !db.exists() {
        return Ok(HashSet::new());
    }
    let conn = Connection::open(&db).with_context(|| format!("open {}", db.display()))?;
    let has_threads = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='threads'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    if !has_threads {
        return Ok(HashSet::new());
    }

    let columns = table_columns(&conn, "threads")?;
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
        "SELECT id, {thread_source_expr}, {source_expr}, {parent_thread_expr}, {agent_path_expr}, {agent_nickname_expr}, {agent_role_expr}, {has_user_event_expr}, {title_expr}, {first_user_message_expr}, {preview_expr} FROM threads"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let thread_source: Option<String> = row.get(1).ok();
        let source: Option<String> = row.get(2).ok();
        let parent_thread_id: Option<String> = row.get(3).ok();
        let agent_path: Option<String> = row.get(4).ok();
        let agent_nickname: Option<String> = row.get(5).ok();
        let agent_role: Option<String> = row.get(6).ok();
        let has_user_event: Option<i64> = row.get(7).ok();
        let title: Option<String> = row.get(8).ok();
        let first_user_message: Option<String> = row.get(9).ok();
        let preview: Option<String> = row.get(10).ok();
        let hidden = is_hidden_thread_metadata(ThreadVisibilityMetadata {
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
        Ok(hidden.then_some(id))
    })?;
    Ok(rows
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect())
}

pub fn thread_source_counts(paths: &CodexPaths) -> Result<HashMap<String, usize>> {
    let db = paths.state_db();
    if !db.exists() {
        return Ok(HashMap::new());
    }
    let conn = Connection::open(&db).with_context(|| format!("open {}", db.display()))?;
    let has_threads = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='threads'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    if !has_threads {
        return Ok(HashMap::new());
    }

    let columns = table_columns(&conn, "threads")?;
    let thread_source_expr = first_existing(&columns, &["thread_source", "source_kind"])
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
    let archived_expr = first_existing(&columns, &["archived"])
        .unwrap_or("0")
        .to_string();
    let archived_at_expr = first_existing(&columns, &["archived_at"])
        .unwrap_or("NULL")
        .to_string();
    let sql = format!(
        "SELECT {thread_source_expr}, {source_expr}, {parent_thread_expr}, {agent_path_expr}, {agent_nickname_expr}, {agent_role_expr}, {has_user_event_expr}, {title_expr}, {first_user_message_expr}, {preview_expr}, {archived_expr}, {archived_at_expr} FROM threads"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let thread_source: Option<String> = row.get(0).ok();
        let source: Option<String> = row.get(1).ok();
        let parent_thread_id: Option<String> = row.get(2).ok();
        let agent_path: Option<String> = row.get(3).ok();
        let agent_nickname: Option<String> = row.get(4).ok();
        let agent_role: Option<String> = row.get(5).ok();
        let has_user_event: Option<i64> = row.get(6).ok();
        let title: Option<String> = row.get(7).ok();
        let first_user_message: Option<String> = row.get(8).ok();
        let preview: Option<String> = row.get(9).ok();
        let archived_flag: i64 = row.get(10).unwrap_or(0);
        let archived_at: Option<ValueCell> = row.get(11).ok();
        Ok((
            thread_source,
            source,
            parent_thread_id,
            agent_path,
            agent_nickname,
            agent_role,
            has_user_event,
            title,
            first_user_message,
            preview,
            archived_flag,
            archived_at,
        ))
    })?;
    let mut counts = HashMap::new();
    for row in rows {
        let (
            thread_source,
            source,
            parent_thread_id,
            agent_path,
            agent_nickname,
            agent_role,
            has_user_event,
            title,
            first_user_message,
            preview,
            archived_flag,
            archived_at,
        ) = row?;
        let metadata = ThreadVisibilityMetadata {
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
        };
        let key = if is_internal_thread_metadata(metadata) {
            "internal".to_string()
        } else if is_subagent_metadata(
            thread_source.as_deref(),
            parent_thread_id.as_deref(),
            agent_path.as_deref(),
            agent_nickname.as_deref(),
            agent_role.as_deref(),
        ) || source.as_deref().is_some_and(source_text_contains_subagent)
        {
            "subagent".to_string()
        } else {
            thread_source_label(thread_source.as_deref().or(source.as_deref()))
        };
        *counts.entry(key).or_insert(0) += 1;
        if archived_flag != 0 || archived_at.as_ref().and_then(format_cell_time).is_some() {
            *counts.entry("archived".to_string()).or_insert(0) += 1;
        }
    }
    Ok(counts)
}

pub fn enrich_thread_from_rollout(row: &mut ThreadSummary) -> Result<bool> {
    let Some(path) = &row.rollout_path else {
        return Ok(false);
    };
    let scan = scan_rollout(path, 80)?;
    row.message_count = scan.message_count;
    row.latest_message = scan.latest_message;
    if !matches!(row.status, ThreadStatus::Archived) {
        if scan.recoverable {
            row.status = ThreadStatus::Recoverable;
        } else if scan.running {
            row.status = ThreadStatus::Running;
        } else if scan.reply_needed {
            row.status = ThreadStatus::ReplyNeeded;
        } else if matches!(
            row.status,
            ThreadStatus::Running | ThreadStatus::ReplyNeeded | ThreadStatus::Recoverable
        ) {
            row.status = ThreadStatus::Recent;
        }
    }
    row.cwd = scan.cwd;
    row.model = scan.model;
    row.active_turn_id = scan.active_turn_id;
    row.pending_elicitation = scan.pending_elicitation;
    row.last_event_kind = scan.last_event_kind;
    Ok(scan.is_subagent)
}

pub fn thread_detail(paths: &CodexPaths, id: &str) -> Result<Option<ThreadDetail>> {
    let Some(summary) = list_threads(paths, None, Some(id), 500)?
        .into_iter()
        .find(|thread| thread.id == id)
    else {
        return Ok(None);
    };
    thread_detail_from_summary(summary).map(Some)
}

pub fn thread_detail_from_summary(summary: ThreadSummary) -> Result<ThreadDetail> {
    let mut messages = Vec::new();
    let mut block_builder = MessageBlockBuilder::default();
    let mut raw_event_count = 0;
    if let Some(path) = &summary.rollout_path {
        let text =
            fs::read_to_string(path).with_context(|| format!("read rollout {}", path.display()))?;
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            raw_event_count += 1;
            let Ok(value) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if let Some(message) = parse_message_event(&value) {
                messages.push(message);
            }
            block_builder.push_event(&value, raw_event_count);
        }
    }
    let blocks = block_builder.finish();
    let total_blocks = blocks.len();
    Ok(ThreadDetail {
        summary,
        messages,
        blocks,
        raw_event_count,
        total_blocks,
        has_more_blocks: false,
        before_cursor: None,
    })
}

pub fn window_thread_detail(
    mut detail: ThreadDetail,
    limit: Option<usize>,
    before: Option<&str>,
) -> ThreadDetail {
    let total = detail.blocks.len();
    detail.total_blocks = total;
    let Some(limit) = limit.filter(|value| *value > 0) else {
        detail.has_more_blocks = false;
        detail.before_cursor = None;
        return detail;
    };
    let end = before
        .and_then(|cursor| cursor.strip_prefix("b:"))
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(total)
        .min(total);
    let start = end.saturating_sub(limit);
    detail.blocks = detail.blocks[start..end].to_vec();
    detail.messages.clear();
    detail.has_more_blocks = start > 0;
    detail.before_cursor = (start > 0).then(|| format!("b:{start}"));
    detail
}

pub fn message_blocks_from_events<'a>(
    events: impl IntoIterator<Item = &'a Value>,
) -> Vec<MessageBlock> {
    let mut block_builder = MessageBlockBuilder::default();
    for (index, value) in events.into_iter().enumerate() {
        block_builder.push_event(value, index + 1);
    }
    block_builder.finish()
}

fn read_thread_rows(paths: &CodexPaths) -> Result<Vec<ThreadSummary>> {
    let db = paths.state_db();
    if !db.exists() {
        return Ok(Vec::new());
    }
    let conn = Connection::open(&db).with_context(|| format!("open {}", db.display()))?;
    let has_threads = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='threads'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    if !has_threads {
        return Ok(Vec::new());
    }

    let columns = table_columns(&conn, "threads")?;
    let title_expr = first_existing(&columns, &["title", "name"])
        .map(|c| format!("COALESCE({c}, id)"))
        .unwrap_or_else(|| "id".to_string());
    let updated_expr = first_existing(&columns, &["updated_at", "last_activity_at", "created_at"])
        .unwrap_or("NULL")
        .to_string();
    let archived_expr = first_existing(&columns, &["archived_at"])
        .unwrap_or("NULL")
        .to_string();
    let archived_flag_expr = first_existing(&columns, &["archived"])
        .unwrap_or("0")
        .to_string();
    let rollout_expr = first_existing(&columns, &["rollout_path"])
        .unwrap_or("NULL")
        .to_string();
    let cwd_expr = first_existing(&columns, &["cwd"])
        .unwrap_or("NULL")
        .to_string();
    let model_expr = first_existing(&columns, &["model"])
        .unwrap_or("NULL")
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
    let first_user_message_expr = first_existing(&columns, &["first_user_message"])
        .unwrap_or("NULL")
        .to_string();
    let preview_expr = first_existing(&columns, &["preview"])
        .unwrap_or("NULL")
        .to_string();
    let sql = format!(
        "SELECT id, {title_expr}, {updated_expr}, {archived_expr}, {archived_flag_expr}, {rollout_expr}, {cwd_expr}, {model_expr}, {thread_source_expr}, {source_expr}, {parent_thread_expr}, {agent_path_expr}, {agent_nickname_expr}, {agent_role_expr}, {has_user_event_expr}, {first_user_message_expr}, {preview_expr} FROM threads"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let title: Option<String> = row.get(1).ok();
        let updated_raw: Option<ValueCell> = row.get(2).ok();
        let archived_raw: Option<ValueCell> = row.get(3).ok();
        let archived_flag: i64 = row.get(4).unwrap_or(0);
        let rollout_path: Option<String> = row.get(5).ok();
        let cwd: Option<String> = row.get(6).ok();
        let model: Option<String> = row.get(7).ok();
        let thread_source: Option<String> = row.get(8).ok();
        let source: Option<String> = row.get(9).ok();
        let parent_thread_id: Option<String> = row.get(10).ok();
        let agent_path: Option<String> = row.get(11).ok();
        let agent_nickname: Option<String> = row.get(12).ok();
        let agent_role: Option<String> = row.get(13).ok();
        let has_user_event: Option<i64> = row.get(14).ok();
        let first_user_message: Option<String> = row.get(15).ok();
        let preview: Option<String> = row.get(16).ok();
        if is_hidden_thread_metadata(ThreadVisibilityMetadata {
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
        }) {
            return Ok(None);
        }
        let archived_at = archived_raw.as_ref().and_then(format_cell_time);
        let status = if archived_at.is_some() || archived_flag != 0 {
            ThreadStatus::Archived
        } else {
            ThreadStatus::Recent
        };
        Ok(Some(ThreadSummary {
            id,
            title: title
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "未命名线程".to_string()),
            status,
            updated_at: updated_raw.as_ref().and_then(format_cell_time),
            archived_at,
            message_count: 0,
            latest_message: None,
            cwd,
            model,
            rollout_path: rollout_path
                .filter(|p| !p.trim().is_empty())
                .map(PathBuf::from),
            active_turn_id: None,
            active_job_id: None,
            pending_elicitation: None,
            last_event_kind: None,
        }))
    })?;
    Ok(rows
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect())
}

#[derive(Debug)]
enum ValueCell {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
}

impl rusqlite::types::FromSql for ValueCell {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        Ok(match value {
            rusqlite::types::ValueRef::Null => Self::Null,
            rusqlite::types::ValueRef::Integer(v) => Self::Integer(v),
            rusqlite::types::ValueRef::Real(v) => Self::Real(v),
            rusqlite::types::ValueRef::Text(v) => {
                Self::Text(String::from_utf8_lossy(v).to_string())
            }
            rusqlite::types::ValueRef::Blob(_) => Self::Null,
        })
    }
}

fn format_cell_time(cell: &ValueCell) -> Option<String> {
    match cell {
        ValueCell::Null => None,
        ValueCell::Integer(v) => format_timestamp(*v),
        ValueCell::Real(v) => format_timestamp(*v as i64),
        ValueCell::Text(v) => {
            if v.trim().is_empty() {
                None
            } else if let Ok(n) = v.parse::<i64>() {
                format_timestamp(n)
            } else {
                Some(v.clone())
            }
        }
    }
}

fn format_timestamp(value: i64) -> Option<String> {
    if value <= 0 {
        return None;
    }
    let seconds = if value > 10_000_000_000 {
        value / 1000
    } else {
        value
    };
    Utc.timestamp_opt(seconds, 0)
        .single()
        .map(|dt| DateTime::<Local>::from(dt).to_rfc3339())
}

fn table_columns(conn: &Connection, table: &str) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    Ok(rows.collect::<rusqlite::Result<HashSet<_>>>()?)
}

fn first_existing<'a>(columns: &HashSet<String>, names: &'a [&str]) -> Option<&'a str> {
    names.iter().copied().find(|name| columns.contains(*name))
}

fn read_session_index(paths: &CodexPaths) -> Result<HashMap<String, PathBuf>> {
    let index = paths.session_index();
    let text = fs::read_to_string(&index).with_context(|| format!("read {}", index.display()))?;
    let mut map = HashMap::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(id) = value.get("id").and_then(Value::as_str) else {
            continue;
        };
        let path = value
            .get("path")
            .or_else(|| value.get("rollout_path"))
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .or_else(|| find_rollout_path(paths, id));
        if let Some(path) = path {
            map.insert(id.to_string(), path);
        }
    }
    Ok(map)
}

fn find_rollout_path(paths: &CodexPaths, thread_id: &str) -> Option<PathBuf> {
    let sessions = paths.sessions_dir();
    if !sessions.exists() {
        return None;
    }
    WalkDir::new(sessions)
        .max_depth(8)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .find(|path| {
            path.file_name()
                .and_then(|v| v.to_str())
                .map(|name| name.contains(thread_id) && name.ends_with(".jsonl"))
                .unwrap_or(false)
        })
}

#[derive(Default)]
struct RolloutScan {
    message_count: usize,
    latest_message: Option<String>,
    reply_needed: bool,
    recoverable: bool,
    running: bool,
    cwd: Option<String>,
    model: Option<String>,
    active_turn_id: Option<String>,
    pending_elicitation: Option<PendingElicitation>,
    last_event_kind: Option<String>,
    is_subagent: bool,
}

#[derive(Debug, Clone)]
enum PendingAction {
    Plan {
        turn_id: Option<String>,
        item_id: Option<String>,
    },
    Elicitation {
        turn_id: Option<String>,
        item_id: Option<String>,
        call_id: Option<String>,
        elicitation: PendingElicitation,
    },
}

fn scan_rollout(path: &Path, max_messages: usize) -> Result<RolloutScan> {
    let text =
        fs::read_to_string(path).with_context(|| format!("read rollout {}", path.display()))?;
    let mut scan = RolloutScan::default();
    let mut pending_action: Option<PendingAction> = None;
    let mut current_plan_marker: Option<(Option<String>, Option<String>)> = None;
    let mut last_task_status: Option<String> = None;
    let mut active_tasks: Vec<Option<String>> = Vec::new();
    let mut pending_tool_turns: HashMap<String, Option<String>> = HashMap::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(payload) = value.get("session_meta").and_then(|v| v.get("payload")) {
            scan.is_subagent |= is_subagent_session_meta(payload);
            scan.cwd = payload
                .get("cwd")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or(scan.cwd);
            scan.model = payload
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or(scan.model);
        }
        let event_type = rollout_event_type(&value);
        if !event_type.is_empty() {
            scan.last_event_kind = Some(event_type.to_string());
        }
        update_turn_state(&value, &mut scan);
        if is_turn_terminal_event(event_type) {
            let mut active_tasks_changed = false;
            let completed_turn_id = event_turn_id(&value);
            if let Some(turn_id) = &completed_turn_id {
                if let Some(index) = active_tasks
                    .iter()
                    .position(|active| active.as_deref() == Some(turn_id.as_str()))
                {
                    active_tasks.drain(..=index);
                    active_tasks_changed = true;
                }
            } else if !active_tasks.is_empty() {
                active_tasks.clear();
                active_tasks_changed = true;
            }
            if active_tasks_changed {
                scan.active_turn_id = latest_active_task_turn(&active_tasks);
            }
            clear_pending_tools_for_turn(&mut pending_tool_turns, completed_turn_id.as_deref());
            if scan.active_turn_id.is_none() {
                scan.recoverable = false;
            }
        }
        if event_type == "task_started" {
            if let Some(turn_id) = event_turn_id(&value) {
                scan.active_turn_id = Some(turn_id.clone());
                if !active_tasks
                    .iter()
                    .any(|active| active.as_deref() == Some(turn_id.as_str()))
                {
                    active_tasks.push(Some(turn_id));
                }
            } else {
                active_tasks.push(None);
            }
            scan.running = true;
        }
        if event_type == "item_completed"
            && value
                .get("item")
                .and_then(|item| item.get("type"))
                .and_then(Value::as_str)
                == Some("Plan")
        {
            current_plan_marker = Some((event_turn_id(&value), event_item_id(&value)));
        }
        if event_type == "task_complete" {
            let completed_turn_id = event_turn_id(&value);
            if let Some(turn_id) = completed_turn_id.as_deref() {
                if let Some(index) = active_tasks
                    .iter()
                    .position(|active| active.as_deref() == Some(turn_id))
                {
                    active_tasks.drain(..=index);
                }
                scan.active_turn_id = latest_active_task_turn(&active_tasks);
            } else {
                if let Some(index) = active_tasks.iter().rposition(Option::is_none) {
                    active_tasks.remove(index);
                    scan.active_turn_id = latest_active_task_turn(&active_tasks);
                }
            }
            let payload = value.get("payload").unwrap_or(&value);
            last_task_status = value
                .get("status")
                .and_then(Value::as_str)
                .or_else(|| payload.get("status").and_then(Value::as_str))
                .or_else(|| value.get("turn_status").and_then(Value::as_str))
                .or_else(|| payload.get("turn_status").and_then(Value::as_str))
                .map(str::to_string);
            let last_agent_null = value
                .get("last_agent_message")
                .or_else(|| payload.get("last_agent_message"))
                .map(Value::is_null)
                .unwrap_or(false);
            let last_agent_message = value
                .get("last_agent_message")
                .or_else(|| payload.get("last_agent_message"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|message| !message.is_empty());
            if let Some(turn_id) = completed_turn_id.as_deref() {
                clear_pending_tools_for_turn(&mut pending_tool_turns, Some(turn_id));
            }
            if last_agent_message.is_some() && scan.active_turn_id.is_none() {
                clear_anonymous_pending_tools(&mut pending_tool_turns);
            }
            if last_agent_null {
                if let Some((turn_id, item_id)) =
                    plan_marker_for_event(&current_plan_marker, &value)
                {
                    pending_action = Some(PendingAction::Plan { turn_id, item_id });
                } else {
                    scan.recoverable = true;
                }
            }
            if last_agent_message.is_some() {
                pending_action = None;
                scan.recoverable = false;
            }
            current_plan_marker = None;
        }
        if is_request_user_input(&value) {
            if let Some(elicitation) = parse_pending_elicitation(&value) {
                pending_action = Some(PendingAction::Elicitation {
                    turn_id: elicitation.turn_id.clone(),
                    item_id: elicitation.item_id.clone(),
                    call_id: event_call_id(&value),
                    elicitation,
                });
            }
        }
        update_pending_tool_calls(&value, &mut pending_tool_turns);
        if clears_pending_action(&value, pending_action.as_ref()) {
            pending_action = None;
        }
        if let Some(message) = parse_message_event(&value) {
            scan.message_count += 1;
            let text = trim_text(&message.text, 500);
            if !text.is_empty() {
                if text.contains("<proposed_plan>") {
                    let (turn_id, item_id) = plan_marker_for_event(&current_plan_marker, &value)
                        .unwrap_or_else(|| (event_turn_id(&value), event_item_id(&value)));
                    pending_action = Some(PendingAction::Plan { turn_id, item_id });
                }
                scan.latest_message = Some(text);
            }
        }
    }
    if let Some(pending) = pending_action {
        scan.reply_needed = true;
        scan.recoverable = false;
        if let PendingAction::Elicitation { elicitation, .. } = pending {
            scan.pending_elicitation = Some(elicitation);
        }
    }
    let _ = max_messages;
    if scan.active_turn_id.is_none() {
        scan.active_turn_id = pending_tool_turns.values().find_map(Clone::clone);
    }
    scan.running = scan.active_turn_id.is_some()
        || !active_tasks.is_empty()
        || !pending_tool_turns.is_empty()
        || matches!(
            last_task_status.as_deref(),
            Some("running" | "in_progress" | "inProgress")
        );
    Ok(scan)
}

pub fn rollout_latest_assistant_message(path: &Path) -> Result<Option<String>> {
    let scan = scan_rollout(path, 1)?;
    Ok(scan.latest_message)
}

pub fn rollout_completion_last_agent_message(
    path: &Path,
    turn_id: Option<&str>,
) -> Result<Option<String>> {
    let text =
        fs::read_to_string(path).with_context(|| format!("read rollout {}", path.display()))?;
    let mut latest_assistant = None;
    let mut latest_task_complete = None;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(message) = parse_message_event(&value) {
            if message.role == "assistant" && !message.text.trim().is_empty() {
                latest_assistant = Some(message.text);
            }
        }
        if rollout_event_type(&value) != "task_complete" {
            continue;
        }
        if let Some(expected_turn_id) = turn_id {
            let event_turn = event_turn_id(&value);
            if event_turn.as_deref() != Some(expected_turn_id) {
                continue;
            }
        }
        let payload = value.get("payload").unwrap_or(&value);
        let last_agent_message = value
            .get("last_agent_message")
            .or_else(|| payload.get("last_agent_message"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|message| !message.is_empty())
            .map(str::to_string);
        if last_agent_message.is_some() {
            latest_task_complete = last_agent_message;
        }
    }
    Ok(latest_task_complete.or(latest_assistant))
}

pub fn rollout_has_completed_turn(path: &Path, turn_id: Option<&str>) -> Result<bool> {
    let text =
        fs::read_to_string(path).with_context(|| format!("read rollout {}", path.display()))?;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let event_type = rollout_event_type(&value);
        if !matches!(
            event_type,
            "task_complete" | "turn_completed" | "turn/completed"
        ) {
            continue;
        }
        if let Some(expected_turn_id) = turn_id {
            let event_turn = event_turn_id(&value);
            if event_turn.as_deref() != Some(expected_turn_id) {
                continue;
            }
        }
        if event_type != "task_complete" || task_complete_has_last_agent_message(&value) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn task_complete_has_last_agent_message(value: &Value) -> bool {
    let payload = value.get("payload").unwrap_or(value);
    value
        .get("last_agent_message")
        .or_else(|| payload.get("last_agent_message"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|message| !message.is_empty())
}

fn latest_active_task_turn(active_tasks: &[Option<String>]) -> Option<String> {
    active_tasks.iter().rev().find_map(Clone::clone)
}

fn is_turn_terminal_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "turn_completed" | "turn/completed" | "turn_aborted" | "turn/aborted"
    )
}

fn clear_pending_tools_for_turn(
    pending_tool_turns: &mut HashMap<String, Option<String>>,
    turn_id: Option<&str>,
) {
    match turn_id {
        Some(turn_id) => {
            pending_tool_turns.retain(|_, pending_turn| pending_turn.as_deref() != Some(turn_id))
        }
        None => pending_tool_turns.clear(),
    }
}

fn clear_anonymous_pending_tools(pending_tool_turns: &mut HashMap<String, Option<String>>) {
    pending_tool_turns.retain(|_, pending_turn| pending_turn.is_some());
}

fn rollout_event_type(value: &Value) -> &str {
    let top_level = value.get("type").and_then(Value::as_str).unwrap_or("");
    if top_level == "event_msg" {
        value
            .pointer("/payload/type")
            .or_else(|| value.pointer("/payload/event_type"))
            .or_else(|| value.pointer("/payload/event/type"))
            .or_else(|| value.pointer("/payload/payload/type"))
            .and_then(Value::as_str)
            .unwrap_or(top_level)
    } else {
        top_level
    }
}

fn update_pending_tool_calls(
    value: &Value,
    pending_tool_turns: &mut HashMap<String, Option<String>>,
) {
    let Some(payload) = event_payload(value) else {
        return;
    };
    let payload_type = event_payload_type(value, payload);
    if is_tool_output_kind(payload_type) {
        if let Some(call_id) = payload_call_id(payload) {
            pending_tool_turns.remove(&call_id);
        }
        return;
    }
    if !is_tool_call_kind(payload_type) || is_request_user_input(value) {
        return;
    }
    let Some(call_id) = payload_call_id(payload) else {
        return;
    };
    if payload_status(payload)
        .as_deref()
        .is_some_and(is_finished_status)
    {
        return;
    }
    pending_tool_turns.insert(call_id, event_turn_id(value));
}

fn plan_marker_for_event(
    marker: &Option<(Option<String>, Option<String>)>,
    value: &Value,
) -> Option<(Option<String>, Option<String>)> {
    let (turn_id, item_id) = marker.clone()?;
    let event_turn = event_turn_id(value);
    if turn_id.is_some() && event_turn.is_some() && turn_id != event_turn {
        return None;
    }
    Some((turn_id, item_id))
}

fn clears_pending_action(value: &Value, pending: Option<&PendingAction>) -> bool {
    let Some(pending) = pending else {
        return false;
    };
    if user_input_answer_matches(value, pending) {
        return true;
    }
    if function_output_matches(value, pending) {
        return true;
    }
    let Some(message) = parse_message_event(value) else {
        return false;
    };
    let role = message.role.as_str();
    if role == "user" {
        return true;
    }
    if role == "assistant" && !message.text.contains("<proposed_plan>") {
        return match pending {
            PendingAction::Plan { turn_id, .. } | PendingAction::Elicitation { turn_id, .. } => {
                event_turn_id(value).as_deref() != turn_id.as_deref()
            }
        };
    }
    false
}

fn user_input_answer_matches(value: &Value, pending: &PendingAction) -> bool {
    if !is_user_input_answer(value) {
        return false;
    }
    match pending {
        PendingAction::Elicitation {
            call_id: Some(expected),
            ..
        } => event_call_id(value).as_deref() == Some(expected),
        PendingAction::Elicitation {
            turn_id, item_id, ..
        }
        | PendingAction::Plan { turn_id, item_id } => {
            turn_id
                .as_deref()
                .is_some_and(|expected| event_turn_id(value).as_deref() == Some(expected))
                || item_id
                    .as_deref()
                    .is_some_and(|expected| event_item_id(value).as_deref() == Some(expected))
        }
    }
}

fn function_output_matches(value: &Value, pending: &PendingAction) -> bool {
    let payload = value.get("payload").unwrap_or(value);
    let payload_type = payload
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| value.get("type").and_then(Value::as_str));
    if payload_type != Some("function_call_output") {
        return false;
    }
    let call_id = payload
        .get("call_id")
        .or_else(|| payload.get("callId"))
        .and_then(Value::as_str);
    match pending {
        PendingAction::Elicitation {
            call_id: Some(expected),
            ..
        } => call_id == Some(expected),
        PendingAction::Elicitation {
            turn_id, item_id, ..
        }
        | PendingAction::Plan { turn_id, item_id } => {
            turn_id
                .as_deref()
                .is_some_and(|expected| event_turn_id(value).as_deref() == Some(expected))
                || item_id
                    .as_deref()
                    .is_some_and(|expected| event_item_id(value).as_deref() == Some(expected))
        }
    }
}

fn event_turn_id(value: &Value) -> Option<String> {
    value
        .get("turn_id")
        .or_else(|| value.get("turnId"))
        .or_else(|| value.pointer("/payload/turn_id"))
        .or_else(|| value.pointer("/payload/turnId"))
        .or_else(|| value.pointer("/payload/event/turn_id"))
        .or_else(|| value.pointer("/payload/event/turnId"))
        .or_else(|| value.pointer("/payload/payload/turn_id"))
        .or_else(|| value.pointer("/payload/payload/turnId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn event_item_id(value: &Value) -> Option<String> {
    value
        .get("item_id")
        .or_else(|| value.get("itemId"))
        .or_else(|| value.pointer("/item/id"))
        .or_else(|| value.pointer("/payload/id"))
        .or_else(|| value.pointer("/payload/item_id"))
        .or_else(|| value.pointer("/payload/itemId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn event_call_id(value: &Value) -> Option<String> {
    value
        .get("call_id")
        .or_else(|| value.get("callId"))
        .or_else(|| value.pointer("/payload/call_id"))
        .or_else(|| value.pointer("/payload/callId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn parse_message_event(value: &Value) -> Option<CodexMessage> {
    let event_type = value.get("type").and_then(Value::as_str).unwrap_or("");
    let payload = value
        .get("payload")
        .or_else(|| value.get("message"))
        .or_else(|| value.get("item"));
    let payload_type = payload
        .and_then(|p| p.get("type"))
        .and_then(Value::as_str)
        .unwrap_or(event_type);

    let role = payload
        .and_then(|p| p.get("role"))
        .and_then(Value::as_str)
        .or_else(|| value.get("role").and_then(Value::as_str))
        .unwrap_or_else(|| {
            if payload_type.contains("function") {
                "tool"
            } else {
                "assistant"
            }
        });

    let mut text = String::new();
    collect_text(value, &mut text);
    let text = trim_text(&text, 4000);
    if text.is_empty() {
        return None;
    }
    if payload_type == "message" && !matches!(role, "user" | "assistant") {
        return None;
    }
    if is_internal_display_kind(payload_type) || is_internal_context_message_text(&text) {
        return None;
    }
    Some(CodexMessage {
        role: role.to_string(),
        kind: payload_type.to_string(),
        text,
        created_at: value
            .get("timestamp")
            .or_else(|| value.get("created_at"))
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

const MESSAGE_TEXT_LIMIT: usize = 12_000;
const TOOL_TEXT_LIMIT: usize = 8_000;
const TOOL_INPUT_LIMIT: usize = 4_000;
const TOOL_SUMMARY_LIMIT: usize = 220;
const COMPLETED_TOOL_HISTORY_LIMIT: usize = 40;
const CHAT_HISTORY_LIMIT: usize = 80;

#[derive(Default)]
struct MessageBlockBuilder {
    blocks: Vec<MessageBlock>,
    pending_tools: HashMap<String, PendingToolCall>,
    suppressed_call_ids: HashSet<String>,
    current_plan_marker: Option<(Option<String>, Option<String>)>,
    plan_block_indexes: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
struct PendingToolCall {
    raw_index: usize,
    id: String,
    kind: String,
    status: Option<String>,
    tool_name: Option<String>,
    call_id: Option<String>,
    turn_id: Option<String>,
    item_id: Option<String>,
    created_at: Option<String>,
    input: Option<String>,
    summary: Option<String>,
    payload: Option<Value>,
}

impl MessageBlockBuilder {
    fn push_event(&mut self, value: &Value, raw_index: usize) {
        let event_type = rollout_event_type(value);
        if event_type == "task_complete" || is_turn_terminal_event(event_type) {
            self.clear_pending_tools_for_turn(event_turn_id(value).as_deref());
        }

        if is_plan_item_completed(value) {
            self.current_plan_marker = Some((event_turn_id(value), event_item_id(value)));
            return;
        }

        if let Some(block) = plan_delta_block(value, raw_index) {
            self.push_plan_block(block);
            return;
        }

        if let Some(block) = user_input_answer_block(value, raw_index) {
            self.push_answer_block(block);
            return;
        }

        if let Some(block) = pending_elicitation_block(value, raw_index) {
            if let Some(call_id) = &block.call_id {
                self.suppressed_call_ids.insert(call_id.clone());
            }
            self.blocks.push(block);
            return;
        }

        let Some(payload) = event_payload(value) else {
            return;
        };
        let payload_type = event_payload_type(value, payload);

        if is_tool_call_kind(payload_type) {
            if let Some(call_id) = payload_call_id(payload) {
                self.pending_tools.insert(
                    call_id.clone(),
                    PendingToolCall::from_payload(value, payload, payload_type, raw_index, call_id),
                );
            }
            return;
        }

        if is_tool_output_kind(payload_type) {
            let call_id = payload_call_id(payload);
            if call_id
                .as_ref()
                .is_some_and(|id| self.suppressed_call_ids.remove(id))
            {
                self.push_answer_block(user_input_output_block(value, payload, raw_index, call_id));
                return;
            }
            let pending = call_id
                .as_ref()
                .and_then(|id| self.pending_tools.remove(id));
            self.blocks.push(tool_output_block(
                value,
                payload,
                payload_type,
                raw_index,
                pending,
            ));
            return;
        }

        if let Some(mut block) = parse_message_block(value, raw_index) {
            if block.text.as_deref().is_some_and(contains_proposed_plan) {
                block.kind = "plan".to_string();
                block.status = block.status.or_else(|| Some("pending".to_string()));
                block.display_kind = Some("plan".to_string());
                block.plan_status = block.status.clone();
                block.resolved = Some(false);
                if let Some((turn_id, item_id)) =
                    plan_marker_for_event(&self.current_plan_marker, value)
                {
                    block.turn_id = block.turn_id.or(turn_id);
                    block.item_id = block.item_id.or(item_id);
                }
                block.group_id = Some(plan_group_key(&block, raw_index));
                self.current_plan_marker = None;
                self.push_plan_block(block);
                return;
            }
            self.blocks.push(block);
        }
    }

    fn push_plan_block(&mut self, mut block: MessageBlock) {
        let key = plan_group_key(&block, self.blocks.len());
        block.group_id = Some(key.clone());
        block.display_kind = Some("plan".to_string());
        block.plan_status = block.plan_status.or_else(|| block.status.clone());
        if let Some(index) = self.plan_block_indexes.get(&key).copied() {
            let existing = &mut self.blocks[index];
            if block.status.as_deref() == Some("streaming") {
                let next_text = block.text.unwrap_or_default();
                if !next_text.trim().is_empty() {
                    let current = existing.text.get_or_insert_with(String::new);
                    current.push_str(&next_text);
                }
                existing.status = Some("streaming".to_string());
                existing.plan_status = Some("streaming".to_string());
                existing.resolved = Some(false);
            } else {
                if block
                    .text
                    .as_deref()
                    .is_some_and(|text| !text.trim().is_empty())
                {
                    existing.text = block.text;
                }
                existing.status = block.status.or_else(|| Some("pending".to_string()));
                existing.plan_status = block.plan_status.or_else(|| existing.status.clone());
                existing.resolved = block.resolved.or(Some(false));
                existing.item_id = block.item_id.or_else(|| existing.item_id.clone());
                existing.turn_id = block.turn_id.or_else(|| existing.turn_id.clone());
                existing.created_at = block.created_at.or_else(|| existing.created_at.clone());
                existing.kind = "plan".to_string();
            }
            return;
        }
        self.plan_block_indexes.insert(key, self.blocks.len());
        self.blocks.push(block);
    }

    fn push_answer_block(&mut self, block: MessageBlock) {
        let key = block
            .call_id
            .as_deref()
            .or(block.item_id.as_deref())
            .map(str::to_string);
        if let Some(key) = key {
            if let Some(index) = self.blocks.iter().rposition(|candidate| {
                candidate.call_id.as_deref() == Some(key.as_str())
                    || candidate.item_id.as_deref() == Some(key.as_str())
            }) {
                let pending = &mut self.blocks[index];
                if pending.kind == "request_user_input" {
                    pending.kind = "request_user_input_result".to_string();
                    pending.status = Some("completed".to_string());
                    pending.display_kind = Some("question_result".to_string());
                    pending.resolved = Some(true);
                    pending.answers = block.answers.clone();
                    pending.summary = Some(answer_summary(&block.answers));
                    pending.text = block.text;
                    return;
                }
            }
        }
        self.blocks.push(block);
    }

    fn clear_pending_tools_for_turn(&mut self, turn_id: Option<&str>) {
        match turn_id {
            Some(turn_id) => self
                .pending_tools
                .retain(|_, call| call.turn_id.as_deref() != Some(turn_id)),
            None => self.pending_tools.clear(),
        }
    }

    fn finish(mut self) -> Vec<MessageBlock> {
        let mut pending = self.pending_tools.into_values().collect::<Vec<_>>();
        pending.sort_by_key(|call| call.raw_index);
        self.blocks
            .extend(pending.into_iter().map(PendingToolCall::into_running_block));
        let blocks = compact_completed_tool_history(self.blocks, COMPLETED_TOOL_HISTORY_LIMIT);
        compact_chat_history(blocks, CHAT_HISTORY_LIMIT)
    }
}

fn compact_completed_tool_history(
    blocks: Vec<MessageBlock>,
    max_completed_tools: usize,
) -> Vec<MessageBlock> {
    let completed_tool_indexes = blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            (block.role == "tool"
                && !is_history_collapsed_block(block)
                && !block.status.as_deref().is_some_and(is_running_status))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    if completed_tool_indexes.len() <= max_completed_tools {
        return blocks;
    }

    let hidden = completed_tool_indexes
        .len()
        .saturating_sub(max_completed_tools);
    let keep_start = completed_tool_indexes
        .len()
        .saturating_sub(max_completed_tools);
    let keep_completed = completed_tool_indexes[keep_start..]
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let hide_completed = completed_tool_indexes[..keep_start]
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let collapsed = MessageBlock {
        id: "completed-tool-history-collapsed".to_string(),
        role: "tool".to_string(),
        kind: "tool_history_collapsed".to_string(),
        display_kind: Some("tool_group".to_string()),
        status: Some("completed".to_string()),
        text: Some(format!("{hidden} 个历史工具调用已折叠")),
        summary: Some(format!("{hidden} 个历史工具调用已折叠")),
        input: None,
        truncated: Some(false),
        resolved: Some(true),
        answers: Vec::new(),
        plan_status: None,
        group_id: Some("tool_history".to_string()),
        tool_name: Some("tool_history".to_string()),
        call_id: None,
        turn_id: None,
        item_id: None,
        created_at: None,
        questions: Vec::new(),
        payload: None,
    };

    let mut compacted = Vec::with_capacity(blocks.len().saturating_sub(hidden).saturating_add(1));
    let mut inserted = false;
    for (index, block) in blocks.into_iter().enumerate() {
        if hide_completed.contains(&index) && !keep_completed.contains(&index) {
            if !inserted {
                compacted.push(collapsed.clone());
                inserted = true;
            }
            continue;
        }
        compacted.push(block);
    }
    compacted
}

fn compact_chat_history(blocks: Vec<MessageBlock>, max_chat_messages: usize) -> Vec<MessageBlock> {
    let chat_indexes = blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| is_chat_history_block(block).then_some(index))
        .collect::<Vec<_>>();
    if chat_indexes.len() <= max_chat_messages {
        return blocks;
    }

    let hidden = chat_indexes.len().saturating_sub(max_chat_messages);
    let keep_start = chat_indexes.len().saturating_sub(max_chat_messages);
    let keep_chat = chat_indexes[keep_start..]
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let hide_chat = chat_indexes[..keep_start]
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let collapsed = MessageBlock {
        id: "chat-history-collapsed".to_string(),
        role: "tool".to_string(),
        kind: "chat_history_collapsed".to_string(),
        display_kind: Some("history_group".to_string()),
        status: Some("completed".to_string()),
        text: Some(format!("{hidden} 条历史对话已折叠")),
        summary: Some(format!("{hidden} 条历史对话已折叠")),
        input: None,
        truncated: Some(false),
        resolved: Some(true),
        answers: Vec::new(),
        plan_status: None,
        group_id: Some("chat_history".to_string()),
        tool_name: Some("chat_history".to_string()),
        call_id: None,
        turn_id: None,
        item_id: None,
        created_at: None,
        questions: Vec::new(),
        payload: None,
    };

    let mut compacted = Vec::with_capacity(blocks.len().saturating_sub(hidden).saturating_add(1));
    let mut inserted = false;
    for (index, block) in blocks.into_iter().enumerate() {
        if hide_chat.contains(&index) && !keep_chat.contains(&index) {
            if !inserted {
                compacted.push(collapsed.clone());
                inserted = true;
            }
            continue;
        }
        compacted.push(block);
    }
    compacted
}

fn is_chat_history_block(block: &MessageBlock) -> bool {
    if !matches!(block.role.as_str(), "user" | "assistant") {
        return false;
    }
    if !block.questions.is_empty() {
        return false;
    }
    if block.status.as_deref().is_some_and(is_running_status) {
        return false;
    }
    let kind = block.kind.trim().to_ascii_lowercase();
    !matches!(
        kind.as_str(),
        "reasoning"
            | "agent_reasoning"
            | "session_meta"
            | "request_user_input"
            | "requestuserinput"
    ) && !kind.contains("plan")
        && !kind.contains("approval")
        && !kind.contains("tool")
        && !kind.contains("function_call")
        && !kind.contains("command")
}

fn is_history_collapsed_block(block: &MessageBlock) -> bool {
    matches!(
        block.kind.trim().to_ascii_lowercase().as_str(),
        "chat_history_collapsed" | "tool_history_collapsed"
    )
}

impl PendingToolCall {
    fn from_payload(
        value: &Value,
        payload: &Value,
        payload_type: &str,
        raw_index: usize,
        call_id: String,
    ) -> Self {
        let input = tool_input_text(payload);
        let summary = input.as_deref().map(tool_summary);
        Self {
            raw_index,
            id: block_id(value, raw_index),
            kind: normalize_kind(payload_type).to_string(),
            status: payload_status(payload),
            tool_name: tool_name(payload, payload_type),
            call_id: Some(call_id),
            turn_id: event_turn_id(value),
            item_id: payload_item_id(value, payload),
            created_at: event_time(value),
            input,
            summary,
            payload: None,
        }
    }

    fn into_running_block(self) -> MessageBlock {
        MessageBlock {
            id: self.id,
            role: "tool".to_string(),
            kind: self.kind,
            display_kind: Some("tool".to_string()),
            status: self.status.or_else(|| Some("running".to_string())),
            text: None,
            summary: self.summary,
            input: self.input,
            truncated: Some(false),
            resolved: Some(false),
            answers: Vec::new(),
            plan_status: None,
            group_id: self.call_id.clone(),
            tool_name: self.tool_name,
            call_id: self.call_id,
            turn_id: self.turn_id,
            item_id: self.item_id,
            created_at: self.created_at,
            questions: Vec::new(),
            payload: self.payload,
        }
    }
}

fn parse_message_block(value: &Value, raw_index: usize) -> Option<MessageBlock> {
    if let Some(block) = pending_elicitation_block(value, raw_index) {
        return Some(block);
    }

    let payload = event_payload(value)?;
    let payload_type = event_payload_type(value, payload);
    if is_plan_delta_kind(payload_type) || is_user_input_answer(value) {
        return None;
    }
    if is_tool_call_kind(payload_type) || is_tool_output_kind(payload_type) {
        return None;
    }
    let role = payload
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or_else(|| role_for_kind(payload_type));
    if payload_type == "message" && !matches!(role, "user" | "assistant") {
        return None;
    }
    if is_internal_display_kind(payload_type) {
        return None;
    }
    if payload_type != "message" && !is_action_display_kind(payload_type) {
        return None;
    }
    let text = structured_text(payload)
        .or_else(|| structured_text(value))
        .or_else(|| parse_message_event(value).map(|message| message.text));
    if text
        .as_deref()
        .is_some_and(is_internal_context_message_text)
    {
        return None;
    }
    if text.as_deref().unwrap_or("").trim().is_empty() && !is_action_display_kind(payload_type) {
        return None;
    }
    Some(MessageBlock {
        id: block_id(value, raw_index),
        role: role.to_string(),
        kind: normalize_kind(payload_type).to_string(),
        display_kind: display_kind_for_payload(payload_type).map(str::to_string),
        status: payload_status(payload),
        text,
        summary: None,
        input: None,
        truncated: Some(false),
        resolved: resolved_for_payload(payload_type, payload),
        answers: Vec::new(),
        plan_status: plan_status(payload),
        group_id: event_turn_id(value).or_else(|| payload_item_id(value, payload)),
        tool_name: tool_name(payload, payload_type),
        call_id: payload_call_id(payload),
        turn_id: event_turn_id(value),
        item_id: payload_item_id(value, payload),
        created_at: event_time(value),
        questions: Vec::new(),
        payload: None,
    })
}

fn is_plan_item_completed(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("item_completed")
        && value
            .get("item")
            .and_then(|item| item.get("type"))
            .and_then(Value::as_str)
            == Some("Plan")
}

fn pending_elicitation_block(value: &Value, raw_index: usize) -> Option<MessageBlock> {
    let elicitation = parse_pending_elicitation(value)?;
    Some(MessageBlock {
        id: block_id(value, raw_index),
        role: "assistant".to_string(),
        kind: "request_user_input".to_string(),
        display_kind: Some("question".to_string()),
        status: Some("pending".to_string()),
        text: elicitation
            .questions
            .first()
            .map(|question| question.question.clone()),
        summary: elicitation
            .questions
            .first()
            .map(|question| tool_summary(&question.question)),
        input: None,
        truncated: Some(false),
        resolved: Some(false),
        answers: Vec::new(),
        plan_status: None,
        group_id: event_call_id(value).or_else(|| elicitation.item_id.clone()),
        tool_name: Some("request_user_input".to_string()),
        call_id: event_call_id(value),
        turn_id: elicitation.turn_id.clone(),
        item_id: elicitation.item_id.clone(),
        created_at: event_time(value),
        questions: elicitation.questions,
        payload: Some(value.clone()),
    })
}

fn plan_delta_block(value: &Value, raw_index: usize) -> Option<MessageBlock> {
    let payload = event_payload(value).unwrap_or(value);
    let payload_type = event_payload_type(value, payload);
    if !is_plan_delta_kind(payload_type) {
        return None;
    }
    let text = structured_text_raw(payload)
        .or_else(|| structured_text_raw(value))
        .or_else(|| {
            payload
                .get("delta")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .get("markdown")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    let status = if is_final_plan_kind(payload_type) {
        payload_status(payload).unwrap_or_else(|| "pending".to_string())
    } else {
        "streaming".to_string()
    };
    Some(MessageBlock {
        id: plan_block_id(value, payload, raw_index),
        role: "assistant".to_string(),
        kind: "plan".to_string(),
        display_kind: Some("plan".to_string()),
        status: Some(status.clone()),
        text,
        summary: Some("Proposed Plan".to_string()),
        input: None,
        truncated: Some(false),
        resolved: Some(is_finished_status(&status)),
        answers: Vec::new(),
        plan_status: Some(status),
        group_id: plan_group_id(value, payload),
        tool_name: None,
        call_id: None,
        turn_id: event_turn_id(value)
            .or_else(|| {
                payload
                    .get("turnId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .get("turn_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
        item_id: payload_item_id(value, payload),
        created_at: event_time(value),
        questions: Vec::new(),
        payload: None,
    })
}

fn user_input_answer_block(value: &Value, raw_index: usize) -> Option<MessageBlock> {
    if !is_user_input_answer(value) {
        return None;
    }
    let payload = value.get("payload").unwrap_or(value);
    let answers = parse_user_input_answers(payload);
    let call_id = event_call_id(value)
        .or_else(|| payload_call_id(payload))
        .or_else(|| payload_item_id(value, payload));
    Some(MessageBlock {
        id: block_id(value, raw_index),
        role: "assistant".to_string(),
        kind: "request_user_input_result".to_string(),
        display_kind: Some("question_result".to_string()),
        status: Some("completed".to_string()),
        text: Some(answer_summary(&answers)),
        summary: Some(answer_summary(&answers)),
        input: None,
        truncated: Some(false),
        resolved: Some(true),
        answers,
        plan_status: None,
        group_id: call_id.clone(),
        tool_name: Some("request_user_input".to_string()),
        call_id,
        turn_id: event_turn_id(value)
            .or_else(|| {
                payload
                    .get("turnId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .get("turn_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
        item_id: payload_item_id(value, payload),
        created_at: event_time(value),
        questions: Vec::new(),
        payload: Some(value.clone()),
    })
}

fn user_input_output_block(
    value: &Value,
    payload: &Value,
    raw_index: usize,
    call_id: Option<String>,
) -> MessageBlock {
    let answers = parse_user_input_output_answers(payload);
    MessageBlock {
        id: block_id(value, raw_index),
        role: "assistant".to_string(),
        kind: "request_user_input_result".to_string(),
        display_kind: Some("question_result".to_string()),
        status: Some("completed".to_string()),
        text: Some(answer_summary(&answers)),
        summary: Some(answer_summary(&answers)),
        input: None,
        truncated: Some(false),
        resolved: Some(true),
        answers,
        plan_status: None,
        group_id: call_id.clone(),
        tool_name: Some("request_user_input".to_string()),
        call_id,
        turn_id: event_turn_id(value),
        item_id: payload_item_id(value, payload),
        created_at: event_time(value),
        questions: Vec::new(),
        payload: Some(value.clone()),
    }
}

fn tool_output_block(
    value: &Value,
    payload: &Value,
    payload_type: &str,
    raw_index: usize,
    pending: Option<PendingToolCall>,
) -> MessageBlock {
    let raw_text = structured_text_raw(payload)
        .or_else(|| structured_text_raw(value))
        .or_else(|| pretty_json(payload));
    let (text, truncated) = raw_text
        .as_deref()
        .map(|text| trim_text_with_limit(text, TOOL_TEXT_LIMIT))
        .unwrap_or_else(|| (String::new(), false));
    let call_id =
        payload_call_id(payload).or_else(|| pending.as_ref().and_then(|call| call.call_id.clone()));
    let input = pending
        .as_ref()
        .and_then(|call| call.input.clone())
        .or_else(|| tool_input_text(payload));
    let summary = if text.trim().is_empty() {
        input.as_deref().map(tool_summary)
    } else {
        Some(tool_summary(&text))
    };
    MessageBlock {
        id: pending
            .as_ref()
            .map(|call| call.id.clone())
            .unwrap_or_else(|| block_id(value, raw_index)),
        role: "tool".to_string(),
        kind: normalize_kind(payload_type).to_string(),
        display_kind: Some("tool".to_string()),
        status: payload_status(payload)
            .or_else(|| pending.as_ref().and_then(|call| call.status.clone()))
            .or_else(|| Some("completed".to_string())),
        text: if text.is_empty() { None } else { Some(text) },
        summary,
        input,
        truncated: Some(truncated),
        resolved: Some(true),
        answers: Vec::new(),
        plan_status: None,
        group_id: call_id.clone(),
        tool_name: pending
            .as_ref()
            .and_then(|call| call.tool_name.clone())
            .or_else(|| tool_name(payload, payload_type)),
        call_id,
        turn_id: event_turn_id(value)
            .or_else(|| pending.as_ref().and_then(|call| call.turn_id.clone())),
        item_id: payload_item_id(value, payload)
            .or_else(|| pending.as_ref().and_then(|call| call.item_id.clone())),
        created_at: event_time(value)
            .or_else(|| pending.as_ref().and_then(|call| call.created_at.clone())),
        questions: Vec::new(),
        payload: None,
    }
}

fn parse_pending_elicitation(value: &Value) -> Option<PendingElicitation> {
    let payload = value.get("payload").unwrap_or(value);
    if !is_request_user_input(value) {
        return None;
    }
    let args = payload
        .get("arguments")
        .and_then(parse_arguments_value)
        .or_else(|| {
            payload
                .pointer("/input/arguments")
                .and_then(parse_arguments_value)
        })
        .or_else(|| payload.get("params").cloned())
        .or_else(|| value.get("params").cloned())
        .or_else(|| Some(payload.clone()))?;
    let question_values = args
        .get("questions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let questions = question_values
        .iter()
        .enumerate()
        .map(|(index, question)| {
            let options = question
                .get("options")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .map(|option| UserInputOption {
                            label: option
                                .get("label")
                                .or_else(|| option.get("text"))
                                .or_else(|| option.get("value"))
                                .and_then(Value::as_str)
                                .unwrap_or("选项")
                                .to_string(),
                            description: option
                                .get("description")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            UserInputQuestion {
                id: question
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("q{}", index + 1)),
                header: question
                    .get("header")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                question: question
                    .get("question")
                    .and_then(Value::as_str)
                    .unwrap_or("需要回复")
                    .to_string(),
                options,
            }
        })
        .collect::<Vec<_>>();
    if questions.is_empty() {
        return None;
    }
    Some(PendingElicitation {
        turn_id: payload
            .get("turnId")
            .or_else(|| payload.get("turn_id"))
            .or_else(|| value.get("turnId"))
            .or_else(|| value.get("turn_id"))
            .or_else(|| value.pointer("/params/turnId"))
            .or_else(|| value.pointer("/params/turn_id"))
            .and_then(Value::as_str)
            .map(str::to_string),
        item_id: payload
            .get("itemId")
            .or_else(|| payload.get("item_id"))
            .or_else(|| value.get("itemId"))
            .or_else(|| value.get("item_id"))
            .or_else(|| value.pointer("/params/itemId"))
            .or_else(|| value.pointer("/params/item_id"))
            .and_then(Value::as_str)
            .map(str::to_string),
        questions,
    })
}

fn parse_arguments_value(value: &Value) -> Option<Value> {
    match value {
        Value::String(text) => serde_json::from_str::<Value>(text).ok(),
        other => Some(other.clone()),
    }
}

fn update_turn_state(value: &Value, scan: &mut RolloutScan) {
    let event_type = rollout_event_type(value);
    let turn_id = event_turn_id(value);
    if matches!(event_type, "turn_started" | "turn/started") {
        scan.active_turn_id = turn_id.clone();
        scan.running = true;
    }
    if is_turn_terminal_event(event_type) && (turn_id.is_none() || scan.active_turn_id == turn_id) {
        scan.active_turn_id = None;
    }
}

fn block_id(value: &Value, raw_index: usize) -> String {
    value
        .pointer("/payload/id")
        .or_else(|| value.pointer("/item/id"))
        .or_else(|| value.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("raw-{raw_index}"))
}

fn event_time(value: &Value) -> Option<String> {
    value
        .get("timestamp")
        .or_else(|| value.get("created_at"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn role_for_kind(kind: &str) -> &'static str {
    match kind {
        "userMessage" | "input_text" => "user",
        "function_call"
        | "function_call_output"
        | "custom_tool_call"
        | "custom_tool_call_output"
        | "tool_search_call"
        | "tool_search_output"
        | "web_search_call"
        | "web_search_end"
        | "commandExecution"
        | "mcpToolCall"
        | "dynamicToolCall"
        | "fileChange" => "tool",
        _ => "assistant",
    }
}

fn normalize_kind(kind: &str) -> &str {
    match kind {
        "Plan" => "plan",
        "PlanDelta" | "planDelta" | "plan_delta" | "item/plan/delta" => "plan",
        "turn/plan/updated" => "plan",
        other => other,
    }
}

fn is_plan_delta_kind(kind: &str) -> bool {
    let normalized = kind.to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "plan" | "plandelta" | "plan_delta" | "item/plan/delta" | "turn/plan/updated"
    )
}

fn is_final_plan_kind(kind: &str) -> bool {
    matches!(
        kind,
        "Plan" | "plan" | "turn/plan/updated" | "plan/updated" | "planUpdated"
    )
}

fn plan_block_id(value: &Value, payload: &Value, raw_index: usize) -> String {
    payload_item_id(value, payload)
        .or_else(|| event_turn_id(value).map(|turn| format!("plan-{turn}")))
        .unwrap_or_else(|| block_id(value, raw_index))
}

fn plan_group_key(block: &MessageBlock, fallback_index: usize) -> String {
    block
        .turn_id
        .as_deref()
        .map(|turn| format!("plan-turn-{turn}"))
        .or_else(|| {
            block
                .item_id
                .as_deref()
                .map(|item| format!("plan-item-{item}"))
        })
        .or_else(|| block.group_id.clone())
        .or_else(|| Some(format!("plan-{fallback_index}")))
        .unwrap()
}

fn plan_group_id(value: &Value, payload: &Value) -> Option<String> {
    event_turn_id(value)
        .or_else(|| {
            payload
                .get("turnId")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .get("turn_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .map(|turn| format!("plan-turn-{turn}"))
        .or_else(|| payload_item_id(value, payload).map(|item| format!("plan-item-{item}")))
}

fn event_payload(value: &Value) -> Option<&Value> {
    value
        .get("payload")
        .or_else(|| value.get("item"))
        .or_else(|| value.get("message"))
}

fn event_payload_type<'a>(value: &'a Value, payload: &'a Value) -> &'a str {
    payload
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| value.get("type").and_then(Value::as_str))
        .unwrap_or("event")
}

fn payload_status(payload: &Value) -> Option<String> {
    payload
        .get("status")
        .and_then(|status| status.get("type").or(Some(status)))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn plan_status(payload: &Value) -> Option<String> {
    payload_status(payload)
        .or_else(|| {
            payload
                .get("plan_status")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .get("planStatus")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn display_kind_for_payload(kind: &str) -> Option<&'static str> {
    let normalized = normalize_kind(kind).to_ascii_lowercase();
    if normalized.contains("plan") {
        Some("plan")
    } else if normalized.contains("approval") || normalized.contains("permissions/request") {
        Some("approval")
    } else {
        None
    }
}

fn resolved_for_payload(kind: &str, payload: &Value) -> Option<bool> {
    let normalized = normalize_kind(kind).to_ascii_lowercase();
    if normalized.contains("plan") {
        return payload_status(payload)
            .as_deref()
            .map(is_finished_status)
            .or(Some(false));
    }
    None
}

fn payload_call_id(payload: &Value) -> Option<String> {
    payload
        .get("call_id")
        .or_else(|| payload.get("callId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn payload_item_id(value: &Value, payload: &Value) -> Option<String> {
    payload
        .get("id")
        .or_else(|| value.get("item_id"))
        .or_else(|| value.get("itemId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn is_tool_call_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_call" | "custom_tool_call" | "tool_search_call" | "web_search_call"
    )
}

fn is_tool_output_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_call_output"
            | "custom_tool_call_output"
            | "tool_search_output"
            | "web_search_end"
            | "web_search_output"
    )
}

fn is_action_display_kind(kind: &str) -> bool {
    let normalized = kind.to_ascii_lowercase();
    matches!(normalized.as_str(), "plan" | "plandelta" | "plan_delta")
        || normalized.contains("plan/delta")
        || normalized.contains("turn/plan/updated")
        || normalized.contains("approval")
        || normalized.contains("permissions/request")
}

fn is_internal_display_kind(kind: &str) -> bool {
    let normalized = kind.to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "reasoning"
            | "reasoning_delta"
            | "agent_reasoning"
            | "internal"
            | "subagent"
            | "system"
            | "developer"
            | "session_meta"
    ) || normalized.contains("reasoning")
        || normalized.contains("internal")
        || normalized.contains("subagent")
}

fn is_internal_context_message_text(text: &str) -> bool {
    let lower = text.trim_start().to_ascii_lowercase();
    [
        "<environment_context>",
        "<permissions instructions>",
        "<app-context>",
        "<collaboration_mode>",
        "<skills_instructions>",
        "<plugins_instructions>",
        "<subagent_notification>",
        "<subagent_context>",
        "<codex_internal_context",
        "<goal_context>",
        "<additional_context>",
        "<user_instructions>",
        "<turn_aborted>",
        "<user_shell_command>",
        "<legacy_unified_exec_process_limit_warning>",
        "<legacy_apply_patch_exec_command_warning>",
        "<legacy_model_mismatch_warning>",
        "========= memory_summary begins =========",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn contains_proposed_plan(text: &str) -> bool {
    text.contains("<proposed_plan>")
}

fn is_subagent_session_meta(payload: &Value) -> bool {
    let source_value = payload
        .get("thread_source")
        .or_else(|| payload.get("threadSource"))
        .or_else(|| payload.get("source"))
        .or_else(|| payload.get("sourceKind"))
        .or_else(|| payload.get("source_kind"));
    let source = source_value.and_then(Value::as_str);
    is_subagent_metadata(
        source,
        payload
            .get("parent_thread_id")
            .or_else(|| payload.get("parentThreadId"))
            .and_then(Value::as_str),
        payload
            .get("agent_path")
            .or_else(|| payload.get("agentPath"))
            .and_then(Value::as_str),
        payload
            .get("agent_nickname")
            .or_else(|| payload.get("agentNickname"))
            .and_then(Value::as_str),
        payload
            .get("agent_role")
            .or_else(|| payload.get("agentRole"))
            .and_then(Value::as_str),
    ) || source_value.is_some_and(value_contains_subagent)
}

fn is_subagent_metadata(
    source: Option<&str>,
    parent_thread_id: Option<&str>,
    agent_path: Option<&str>,
    agent_nickname: Option<&str>,
    agent_role: Option<&str>,
) -> bool {
    non_empty(parent_thread_id).is_some()
        || non_empty(agent_path).is_some()
        || source.is_some_and(source_text_contains_subagent)
        || non_empty(agent_nickname).is_some()
        || non_empty(agent_role).is_some()
}

#[derive(Clone, Copy)]
pub(crate) struct ThreadVisibilityMetadata<'a> {
    pub(crate) thread_source: Option<&'a str>,
    pub(crate) source: Option<&'a str>,
    pub(crate) parent_thread_id: Option<&'a str>,
    pub(crate) agent_path: Option<&'a str>,
    pub(crate) agent_nickname: Option<&'a str>,
    pub(crate) agent_role: Option<&'a str>,
    pub(crate) has_user_event: Option<i64>,
    pub(crate) title: Option<&'a str>,
    pub(crate) first_user_message: Option<&'a str>,
    pub(crate) preview: Option<&'a str>,
}

pub(crate) fn is_hidden_thread_metadata(metadata: ThreadVisibilityMetadata<'_>) -> bool {
    hidden_thread_metadata_category(metadata).is_some()
}

pub(crate) fn hidden_thread_metadata_category(
    metadata: ThreadVisibilityMetadata<'_>,
) -> Option<String> {
    if is_internal_thread_metadata(metadata) {
        return Some("internal".to_string());
    }
    if is_subagent_metadata(
        metadata.thread_source,
        metadata.parent_thread_id,
        metadata.agent_path,
        metadata.agent_nickname,
        metadata.agent_role,
    ) || metadata.source.is_some_and(source_text_contains_subagent)
    {
        return Some("subagent".to_string());
    }
    None
}

pub(crate) fn rollout_has_running_signal(path: &Path) -> Result<bool> {
    let scan = scan_rollout(path, 80)?;
    Ok(scan.running)
}

fn is_internal_thread_metadata(metadata: ThreadVisibilityMetadata<'_>) -> bool {
    let source = metadata
        .source
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let thread_source = metadata
        .thread_source
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if source != "exec" || metadata.has_user_event.unwrap_or(0) != 0 {
        return false;
    }
    if !thread_source.is_empty() && thread_source != "user" {
        return false;
    }
    [
        metadata.title,
        metadata.first_user_message,
        metadata.preview,
    ]
    .into_iter()
    .flatten()
    .any(is_internal_thread_prompt_text)
}

fn is_internal_thread_prompt_text(text: &str) -> bool {
    let text = text.trim().to_ascii_lowercase();
    if text.is_empty() {
        return false;
    }
    let readonly_probe = text.contains("只读验证")
        || text.contains("只读核查")
        || text.contains("不要修改文件")
        || text.contains("不改文件")
        || text.contains("read-only")
        || text.contains("readonly");
    let agent_probe = text.contains("spawn_agent")
        || text.contains("子代理")
        || text.contains("subagent")
        || text.contains("model_reasoning_effort=xhigh");
    if readonly_probe && agent_probe {
        return true;
    }

    let strong_subagent_instruction = text.contains("你是子代理")
        || text.contains("你是并行子代理")
        || text.contains("you are a subagent");
    let fixed_agent_config = text.contains("gpt-5.5")
        || text.contains("xhigh")
        || text.contains("model_reasoning_effort=xhigh");
    strong_subagent_instruction && fixed_agent_config
}

fn source_text_contains_subagent(value: &str) -> bool {
    value.to_ascii_lowercase().contains("subagent")
        || serde_json::from_str::<Value>(value)
            .ok()
            .as_ref()
            .is_some_and(value_contains_subagent)
}

fn thread_source_label(value: Option<&str>) -> String {
    let Some(value) = non_empty(value) else {
        return "unknown".to_string();
    };
    if source_text_contains_subagent(value) {
        return "subagent".to_string();
    }
    serde_json::from_str::<Value>(value)
        .ok()
        .and_then(|value| {
            value
                .get("kind")
                .or_else(|| value.get("type"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| value.to_ascii_lowercase())
}

fn value_contains_subagent(value: &Value) -> bool {
    match value {
        Value::String(text) => text.to_ascii_lowercase().contains("subagent"),
        Value::Array(items) => items.iter().any(value_contains_subagent),
        Value::Object(map) => map.iter().any(|(key, value)| {
            key.to_ascii_lowercase().contains("subagent") || value_contains_subagent(value)
        }),
        _ => false,
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn tool_name(payload: &Value, kind: &str) -> Option<String> {
    payload
        .get("name")
        .or_else(|| payload.get("tool"))
        .or_else(|| payload.get("server"))
        .or_else(|| payload.get("method"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| match kind {
            "tool_search_call" | "tool_search_output" => Some("tool_search".to_string()),
            "web_search_call" | "web_search_end" | "web_search_output" => {
                Some("web_search".to_string())
            }
            _ => None,
        })
}

fn tool_input_text(payload: &Value) -> Option<String> {
    let value = payload
        .get("arguments")
        .or_else(|| payload.get("input"))
        .or_else(|| payload.get("params"))
        .or_else(|| payload.get("command"))?;
    let raw = match value {
        Value::String(text) => text.clone(),
        other => pretty_json(other)?,
    };
    let (text, _) = trim_preserving_indentation(&raw, TOOL_INPUT_LIMIT);
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn structured_text(value: &Value) -> Option<String> {
    structured_text_raw(value).map(|text| trim_text(&text, MESSAGE_TEXT_LIMIT))
}

fn structured_text_raw(value: &Value) -> Option<String> {
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = value.get("output").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = value.get("aggregatedOutput").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(command) = value.get("command").and_then(Value::as_str) {
        return Some(command.to_string());
    }
    if let Some(content) = value.get("content").and_then(Value::as_array) {
        let text = content
            .iter()
            .filter_map(|item| {
                item.get("text")
                    .or_else(|| item.get("input_text"))
                    .and_then(Value::as_str)
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    None
}

fn pretty_json(value: &Value) -> Option<String> {
    serde_json::to_string_pretty(value).ok()
}

fn collect_text(value: &Value, out: &mut String) {
    match value {
        Value::String(s) => {
            if looks_like_message(s) {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(s);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_text(value, out);
            }
        }
        Value::Object(map) => {
            for key in [
                "payload", "item", "text", "message", "content", "summary", "output",
            ] {
                if let Some(value) = map.get(key) {
                    collect_text(value, out);
                }
            }
        }
        _ => {}
    }
}

fn looks_like_message(s: &str) -> bool {
    let s = s.trim();
    s.len() > 1
        && !s.starts_with("019")
        && !s.starts_with("call_")
        && !matches!(s, "assistant" | "user" | "system" | "function_call")
}

fn is_request_user_input(value: &Value) -> bool {
    let payload = value.get("payload").unwrap_or(value);
    [
        value.get("name").and_then(Value::as_str),
        value.get("toolName").and_then(Value::as_str),
        value.get("tool_name").and_then(Value::as_str),
        value.get("method").and_then(Value::as_str),
        payload.get("name").and_then(Value::as_str),
        payload.get("toolName").and_then(Value::as_str),
        payload.get("tool_name").and_then(Value::as_str),
        payload.get("method").and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    .any(|name| {
        matches!(
            name,
            "request_user_input" | "requestUserInput" | "item/tool/requestUserInput"
        )
    })
}

fn is_user_input_answer(value: &Value) -> bool {
    let payload = value.get("payload").unwrap_or(value);
    [
        value.get("type").and_then(Value::as_str),
        value.get("name").and_then(Value::as_str),
        value.get("method").and_then(Value::as_str),
        payload.get("type").and_then(Value::as_str),
        payload.get("name").and_then(Value::as_str),
        payload.get("method").and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    .any(|name| {
        matches!(
            name,
            "UserInputAnswer" | "userInputAnswer" | "user_input_answer"
        )
    })
}

fn parse_user_input_answers(payload: &Value) -> Vec<UserInputAnswer> {
    let source = payload
        .get("answers")
        .or_else(|| {
            payload
                .get("response")
                .and_then(|response| response.get("answers"))
        })
        .or_else(|| {
            payload
                .get("result")
                .and_then(|result| result.get("answers"))
        })
        .unwrap_or(payload);
    match source {
        Value::Object(map) => map
            .iter()
            .map(|(question_id, value)| UserInputAnswer {
                question_id: question_id.clone(),
                answers: answer_values(value),
                note: answer_note(value),
            })
            .collect(),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let question_id = item
                    .get("question_id")
                    .or_else(|| item.get("questionId"))
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("q{}", index + 1));
                UserInputAnswer {
                    question_id,
                    answers: answer_values(
                        item.get("answers")
                            .or_else(|| item.get("answer"))
                            .unwrap_or(item),
                    ),
                    note: answer_note(item),
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_user_input_output_answers(payload: &Value) -> Vec<UserInputAnswer> {
    payload
        .get("output")
        .and_then(Value::as_str)
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .map(|value| parse_user_input_answers(&value))
        .filter(|answers| !answers.is_empty())
        .unwrap_or_else(|| parse_user_input_answers(payload))
}

fn answer_values(value: &Value) -> Vec<String> {
    match value {
        Value::String(text) => vec![text.clone()],
        Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(text) => Some(text.clone()),
                Value::Object(map) => map
                    .get("label")
                    .or_else(|| map.get("text"))
                    .or_else(|| map.get("value"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                _ => None,
            })
            .collect(),
        Value::Object(map) => map
            .get("answers")
            .or_else(|| map.get("answer"))
            .map(answer_values)
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn answer_note(value: &Value) -> Option<String> {
    value
        .get("note")
        .or_else(|| value.get("user_note"))
        .or_else(|| value.get("userNote"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn answer_summary(answers: &[UserInputAnswer]) -> String {
    if answers.is_empty() {
        return "Questions answered".to_string();
    }
    let answered = answers
        .iter()
        .filter(|answer| !answer.answers.is_empty() || answer.note.is_some())
        .count();
    format!("{answered}/{} answered", answers.len())
}

fn is_finished_status(status: &str) -> bool {
    let status = status.trim().to_ascii_lowercase();
    matches!(
        status.as_str(),
        "completed"
            | "complete"
            | "done"
            | "finished"
            | "success"
            | "succeeded"
            | "failed"
            | "error"
            | "cancelled"
            | "canceled"
            | "interrupted"
    )
}

fn is_running_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "pending" | "running" | "in_progress" | "inprogress" | "active"
    )
}

fn trim_text(text: &str, max: usize) -> String {
    trim_text_with_limit(text, max).0
}

fn trim_text_with_limit(text: &str, max: usize) -> (String, bool) {
    let compact = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if compact.chars().count() <= max {
        (compact, false)
    } else {
        (compact.chars().take(max).collect::<String>() + "...", true)
    }
}

fn trim_preserving_indentation(text: &str, max: usize) -> (String, bool) {
    let compact = text.trim().to_string();
    if compact.chars().count() <= max {
        (compact, false)
    } else {
        (compact.chars().take(max).collect::<String>() + "...", true)
    }
}

fn tool_summary(text: &str) -> String {
    let compact = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    if compact.chars().count() <= TOOL_SUMMARY_LIMIT {
        compact.to_string()
    } else {
        compact.chars().take(TOOL_SUMMARY_LIMIT).collect::<String>() + "..."
    }
}

fn matches_status(row: &ThreadSummary, status: &str) -> bool {
    match status {
        "recent" => matches!(row.status, ThreadStatus::Recent),
        "running" => matches!(row.status, ThreadStatus::Running),
        "reply-needed" | "reply_needed" => matches!(row.status, ThreadStatus::ReplyNeeded),
        "recoverable" => matches!(row.status, ThreadStatus::Recoverable),
        "archived" => matches!(row.status, ThreadStatus::Archived),
        _ => true,
    }
}

pub fn set_thread_archived(paths: &CodexPaths, id: &str, archived: bool) -> Result<()> {
    let db = paths.state_db();
    let conn = Connection::open(&db).with_context(|| format!("open {}", db.display()))?;
    let columns = table_columns(&conn, "threads")?;
    if !columns.contains("archived_at") {
        anyhow::bail!("threads.archived_at column not found");
    }
    let archived_at = if archived {
        Some(Utc::now().timestamp())
    } else {
        None
    };
    if columns.contains("archived") {
        conn.execute(
            "UPDATE threads SET archived=?2, archived_at=?3 WHERE id=?1",
            rusqlite::params![id, if archived { 1 } else { 0 }, archived_at],
        )?;
    } else {
        conn.execute(
            "UPDATE threads SET archived_at=?2 WHERE id=?1",
            rusqlite::params![id, archived_at],
        )?;
    }
    Ok(())
}

pub fn set_thread_title(paths: &CodexPaths, id: &str, title: &str) -> Result<()> {
    let name = title.trim();
    if name.is_empty() {
        anyhow::bail!("thread title cannot be empty");
    }
    let db = paths.state_db();
    let conn = Connection::open(&db).with_context(|| format!("open {}", db.display()))?;
    let columns = table_columns(&conn, "threads")?;
    let Some(title_column) = first_existing(&columns, &["title", "name"]) else {
        anyhow::bail!("threads title/name column not found");
    };
    let sql = format!("UPDATE threads SET {title_column}=?2 WHERE id=?1");
    conn.execute(&sql, rusqlite::params![id, name])?;
    Ok(())
}

pub fn db_integrity(paths: &CodexPaths) -> Result<String> {
    let db = paths.state_db();
    let conn = Connection::open(&db).with_context(|| format!("open {}", db.display()))?;
    conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .optional()?
        .context("integrity_check returned no rows")
}

#[cfg(test)]
mod tests {
    use super::{
        hidden_thread_ids, is_request_user_input, list_threads, parse_message_event,
        resolve_codex_paths_with_options, scan_rollout, set_thread_title, thread_detail,
        thread_source_counts, window_thread_detail, CodexPathDiscoveryOptions, CodexPaths,
        ThreadStatus,
    };
    use rusqlite::Connection;
    use serde_json::json;
    use std::{
        env, fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
    };

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn resolved_codex_paths_prefers_valid_configured_home_before_auto_candidates() {
        let root = unique_temp_dir("resolved-codex-configured");
        let configured = root.join("configured/.codex");
        let env_home = root.join("env/.codex");
        let socket_home = root.join("socket/.codex");
        mark_codex_home(&configured);
        mark_codex_home(&env_home);
        mark_codex_home(&socket_home);
        let socket = socket_home
            .join("app-server-control")
            .join("app-server-control.sock");
        let options = CodexPathDiscoveryOptions {
            env_codex_home: Some(env_home.clone()),
            current_user_home: None,
            root_codex_home: root.join("root/.codex"),
            ubuntu_codex_home: root.join("ubuntu/.codex"),
            home_scan_root: root.join("home"),
        };

        let resolved = resolve_codex_paths_with_options(&configured, Some(&socket), &options);

        assert_eq!(resolved.home, configured);
        assert_eq!(resolved.codex_home_source, "configured");
        assert_eq!(resolved.logs_db, resolved.home.join("logs_2.sqlite"));
        assert_eq!(resolved.state_db, resolved.home.join("state_5.sqlite"));
        assert_eq!(
            resolved.session_index,
            resolved.home.join("session_index.jsonl")
        );
        assert_eq!(resolved.sessions_dir, resolved.home.join("sessions"));
        assert_eq!(resolved.logs_db_source, "configured");
        assert!(resolved.discovery_warnings.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolved_codex_paths_treats_auto_and_empty_config_as_discovery() {
        let root = unique_temp_dir("resolved-codex-auto");
        let env_home = root.join("env/.codex");
        mark_codex_home(&env_home);
        let options = CodexPathDiscoveryOptions {
            env_codex_home: Some(env_home.clone()),
            current_user_home: None,
            root_codex_home: root.join("root/.codex"),
            ubuntu_codex_home: root.join("ubuntu/.codex"),
            home_scan_root: root.join("home"),
        };

        let auto = resolve_codex_paths_with_options(Path::new("auto"), None, &options);
        let empty = resolve_codex_paths_with_options(Path::new(""), None, &options);

        assert_eq!(auto.home, env_home);
        assert_eq!(auto.configured_codex_home, None);
        assert_eq!(auto.codex_home_source, "env:CODEX_HOME");
        assert_eq!(empty.home, env_home);
        assert_eq!(empty.configured_codex_home, None);
        assert_eq!(empty.codex_home_source, "env:CODEX_HOME");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolved_codex_paths_uses_socket_then_current_root_ubuntu_and_home_scan() {
        let root = unique_temp_dir("resolved-codex-order");
        let invalid_config = root.join("missing/.codex");
        let socket_home = root.join("socket-owner/.codex");
        let current_home = root.join("current-user");
        let root_home = root.join("root/.codex");
        let ubuntu_home = root.join("ubuntu/.codex");
        let scanned_home = root.join("home/alice/.codex");
        mark_codex_home(&socket_home);
        mark_codex_home(&current_home.join(".codex"));
        mark_codex_home(&root_home);
        mark_codex_home(&ubuntu_home);
        mark_codex_home(&scanned_home);
        let socket = socket_home
            .join("app-server-control")
            .join("app-server-control.sock");

        let socket_resolved = resolve_codex_paths_with_options(
            &invalid_config,
            Some(&socket),
            &CodexPathDiscoveryOptions {
                env_codex_home: None,
                current_user_home: Some(current_home.clone()),
                root_codex_home: root_home.clone(),
                ubuntu_codex_home: ubuntu_home.clone(),
                home_scan_root: root.join("home"),
            },
        );
        assert_eq!(socket_resolved.home, socket_home);
        assert_eq!(socket_resolved.codex_home_source, "socket");
        assert_eq!(socket_resolved.app_server_socket, Some(socket));
        assert_eq!(
            socket_resolved.app_server_socket_source.as_deref(),
            Some("configured")
        );
        assert!(socket_resolved
            .discovery_warnings
            .iter()
            .any(|warning| warning.contains("configured Codex home is not valid")));

        let current_resolved = resolve_codex_paths_with_options(
            Path::new("auto"),
            None,
            &CodexPathDiscoveryOptions {
                env_codex_home: None,
                current_user_home: Some(current_home.clone()),
                root_codex_home: root_home.clone(),
                ubuntu_codex_home: ubuntu_home.clone(),
                home_scan_root: root.join("home"),
            },
        );
        assert_eq!(current_resolved.home, current_home.join(".codex"));
        assert_eq!(current_resolved.codex_home_source, "current_user");

        let root_resolved = resolve_codex_paths_with_options(
            Path::new("auto"),
            None,
            &CodexPathDiscoveryOptions {
                env_codex_home: None,
                current_user_home: None,
                root_codex_home: root_home.clone(),
                ubuntu_codex_home: ubuntu_home.clone(),
                home_scan_root: root.join("home"),
            },
        );
        assert_eq!(root_resolved.home, root_home);
        assert_eq!(root_resolved.codex_home_source, "root");

        let ubuntu_resolved = resolve_codex_paths_with_options(
            Path::new("auto"),
            None,
            &CodexPathDiscoveryOptions {
                env_codex_home: None,
                current_user_home: None,
                root_codex_home: root.join("missing-root/.codex"),
                ubuntu_codex_home: ubuntu_home,
                home_scan_root: root.join("home"),
            },
        );
        assert_eq!(ubuntu_resolved.codex_home_source, "home_ubuntu");

        let scan_resolved = resolve_codex_paths_with_options(
            Path::new("auto"),
            None,
            &CodexPathDiscoveryOptions {
                env_codex_home: None,
                current_user_home: None,
                root_codex_home: root.join("missing-root/.codex"),
                ubuntu_codex_home: root.join("missing-ubuntu/.codex"),
                home_scan_root: root.join("home"),
            },
        );
        assert_eq!(scan_resolved.home, scanned_home);
        assert_eq!(scan_resolved.codex_home_source, "home_scan");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolved_codex_paths_preserves_configured_socket_outside_resolved_home() {
        let root = unique_temp_dir("resolved-codex-custom-socket");
        let env_home = root.join("env/.codex");
        let custom_socket = root.join("run/codex.sock");
        mark_codex_home(&env_home);
        let options = CodexPathDiscoveryOptions {
            env_codex_home: Some(env_home.clone()),
            current_user_home: None,
            root_codex_home: root.join("root/.codex"),
            ubuntu_codex_home: root.join("ubuntu/.codex"),
            home_scan_root: root.join("home"),
        };

        let resolved =
            resolve_codex_paths_with_options(Path::new("auto"), Some(&custom_socket), &options);

        assert_eq!(resolved.home, env_home);
        assert_eq!(resolved.app_server_socket, Some(custom_socket.clone()));
        assert_eq!(resolved.configured_app_server_socket, Some(custom_socket));
        assert_eq!(
            resolved.app_server_socket_source.as_deref(),
            Some("configured")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn detects_request_user_input_function_call() {
        let value = json!({"payload":{"type":"function_call","name":"request_user_input"}});
        assert!(is_request_user_input(&value));
        assert!(is_request_user_input(&json!({
            "method":"item/tool/requestUserInput",
            "params":{"questions":[]}
        })));
        assert!(is_request_user_input(&json!({
            "payload":{"type":"function_call","toolName":"requestUserInput"}
        })));
    }

    #[test]
    fn parses_message_text() {
        let value =
            json!({"payload":{"type":"message","role":"assistant","content":[{"text":"hello"}]}});
        let msg = parse_message_event(&value).unwrap();
        assert_eq!(msg.text, "hello");
    }

    #[test]
    fn clears_old_plan_pending_after_later_user_and_assistant_progress() {
        let scan = scan_fixture(&[
            json!({"type":"item_completed","turn_id":"turn-1","item":{"type":"Plan"}}),
            json!({"type":"response_item","turn_id":"turn-1","payload":{"type":"message","role":"assistant","content":[{"text":"<proposed_plan>先做 A，再做 B。</proposed_plan>"}]}}),
            json!({"type":"response_item","turn_id":"turn-2","payload":{"type":"message","role":"user","content":[{"text":"执行"}]}}),
            json!({"type":"response_item","turn_id":"turn-2","payload":{"type":"message","role":"assistant","content":[{"text":"开始执行计划。"}]}}),
            json!({"type":"task_complete","turn_id":"turn-2","status":"completed","last_agent_message":"开始执行计划。"}),
        ]);

        assert!(!scan.reply_needed);
        assert!(scan.pending_elicitation.is_none());
    }

    #[test]
    fn clears_request_user_input_after_matching_function_call_output() {
        let scan = scan_fixture(&[
            json!({"type":"response_item","turn_id":"turn-1","payload":{"type":"function_call","name":"request_user_input","call_id":"call-1","arguments":{"questions":[{"id":"choice","question":"选择方案","options":[{"label":"选项 1"},{"label":"选项 2"}]}]}}}),
            json!({"type":"response_item","turn_id":"turn-1","payload":{"type":"function_call_output","call_id":"call-1","output":"{\"choice\":[\"选项 1\"]}"}}),
            json!({"type":"response_item","turn_id":"turn-1","payload":{"type":"message","role":"assistant","content":[{"text":"已收到选择，继续执行。"}]}}),
            json!({"type":"task_complete","turn_id":"turn-1","status":"completed","last_agent_message":"已收到选择，继续执行。"}),
        ]);

        assert!(!scan.reply_needed);
        assert!(scan.pending_elicitation.is_none());
    }

    #[test]
    fn clears_request_user_input_after_user_input_answer_item() {
        let scan = scan_fixture(&[
            json!({"type":"response_item","turn_id":"turn-1","payload":{"type":"function_call","name":"request_user_input","call_id":"call-1","arguments":{"questions":[{"id":"choice","question":"选择方案","options":[{"label":"选项 1"},{"label":"选项 2"}]}]}}}),
            json!({"type":"response_item","turn_id":"turn-1","payload":{"type":"UserInputAnswer","call_id":"call-1","answers":{"choice":["选项 1"]}}}),
            json!({"type":"response_item","turn_id":"turn-1","payload":{"type":"message","role":"assistant","content":[{"text":"已收到选择。"}]}}),
        ]);

        assert!(!scan.reply_needed);
        assert!(scan.pending_elicitation.is_none());
    }

    #[test]
    fn keeps_latest_request_user_input_pending_without_resolution() {
        let scan = scan_fixture(&[
            json!({"type":"response_item","turn_id":"turn-1","payload":{"type":"function_call","name":"request_user_input","call_id":"call-1","arguments":{"questions":[{"id":"choice","question":"选择方案","options":[{"label":"选项 1"},{"label":"选项 2"}]}]}}}),
        ]);

        assert!(scan.reply_needed);
        assert_eq!(
            scan.pending_elicitation.unwrap().questions[0].question,
            "选择方案"
        );
    }

    #[test]
    fn old_plan_marker_does_not_reclassify_later_silent_completion() {
        let scan = scan_fixture(&[
            json!({"type":"item_completed","turn_id":"turn-plan","item":{"type":"Plan"}}),
            json!({"type":"response_item","turn_id":"turn-plan","payload":{"type":"message","role":"assistant","content":[{"text":"<proposed_plan>旧计划</proposed_plan>"}]}}),
            json!({"type":"response_item","turn_id":"turn-work","payload":{"type":"message","role":"user","content":[{"text":"执行"}]}}),
            json!({"type":"response_item","turn_id":"turn-work","payload":{"type":"message","role":"assistant","content":[{"text":"开始执行。"}]}}),
            json!({"type":"task_complete","turn_id":"turn-work","status":"completed","last_agent_message":"开始执行。"}),
            json!({"type":"task_complete","turn_id":"turn-later","status":"completed","last_agent_message":null}),
        ]);

        assert!(!scan.reply_needed);
        assert!(scan.recoverable);
    }

    #[test]
    fn unrelated_function_output_without_ids_does_not_clear_pending_choice() {
        let scan = scan_fixture(&[
            json!({"type":"response_item","payload":{"type":"function_call","name":"request_user_input","arguments":{"questions":[{"id":"choice","question":"选择方案","options":[{"label":"选项 1"},{"label":"选项 2"}]}]}}}),
            json!({"type":"response_item","payload":{"type":"function_call_output","output":"{}"}}),
        ]);

        assert!(scan.reply_needed);
        assert_eq!(
            scan.pending_elicitation.unwrap().questions[0].question,
            "选择方案"
        );
    }

    #[test]
    fn thread_detail_blocks_hide_internal_context_and_keep_chat_messages() {
        let detail = detail_fixture(&[
            json!({"type":"response_item","payload":{"type":"message","role":"developer","content":[{"text":"internal instructions"}]}}),
            json!({"type":"response_item","payload":{"type":"message","role":"system","content":[{"text":"system context"}]}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"<environment_context>\n  <cwd>/tmp</cwd>\n</environment_context>"}]}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"hello"}]}}),
            json!({"type":"response_item","payload":{"type":"reasoning","summary":[{"text":"hidden reasoning"}]}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"world"}]}}),
        ]);

        assert_eq!(detail.blocks.len(), 2);
        assert_eq!(detail.blocks[0].role, "user");
        assert_eq!(detail.blocks[0].text.as_deref(), Some("hello"));
        assert_eq!(detail.blocks[1].role, "assistant");
        assert_eq!(detail.blocks[1].text.as_deref(), Some("world"));
    }

    #[test]
    fn thread_detail_blocks_hide_event_msg_progress_rows() {
        let detail = detail_fixture(&[
            json!({"type":"event_msg","payload":{"type":"agent_message","message":"progress update"}}),
            json!({"type":"event_msg","payload":{"type":"user_message","message":"duplicate user text"}}),
            json!({"type":"turn_context","payload":{"cwd":"/tmp"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"final answer"}]}}),
        ]);

        assert_eq!(detail.blocks.len(), 1);
        assert_eq!(detail.blocks[0].role, "assistant");
        assert_eq!(detail.blocks[0].text.as_deref(), Some("final answer"));
    }

    #[test]
    fn thread_detail_blocks_hide_subagent_context_fragments() {
        let detail = detail_fixture(&[
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"<subagent_notification>{\"agent_path\":\"/tmp/child\",\"status\":{\"completed\":\"done\"}}</subagent_notification>"}]}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"<subagent_context>\n- /tmp/child: worker\n</subagent_context>"}]}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"真实用户消息"}]}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"可见回复"}]}}),
        ]);

        assert_eq!(detail.blocks.len(), 2);
        assert_eq!(detail.blocks[0].role, "user");
        assert_eq!(detail.blocks[0].text.as_deref(), Some("真实用户消息"));
        assert_eq!(detail.blocks[1].role, "assistant");
        assert_eq!(detail.blocks[1].text.as_deref(), Some("可见回复"));
    }

    #[test]
    fn proposed_plan_message_becomes_action_block_only() {
        let detail = detail_fixture(&[
            json!({"type":"item_completed","turn_id":"turn-plan","item":{"id":"plan-item","type":"Plan"}}),
            json!({"type":"response_item","turn_id":"turn-plan","payload":{"type":"message","role":"assistant","content":[{"text":"<proposed_plan>\n# Summary\n- Fix it\n</proposed_plan>"}]}}),
        ]);

        assert_eq!(detail.blocks.len(), 1);
        let block = &detail.blocks[0];
        assert_eq!(block.role, "assistant");
        assert_eq!(block.kind, "plan");
        assert_eq!(block.turn_id.as_deref(), Some("turn-plan"));
        assert_eq!(block.item_id.as_deref(), Some("plan-item"));
        assert!(block
            .text
            .as_deref()
            .is_some_and(|text| text.contains("<proposed_plan>")));
    }

    #[test]
    fn plan_protocol_variants_merge_into_stable_plan_block() {
        let blocks = super::message_blocks_from_events(
            [
                json!({"type":"PlanDelta","turn_id":"turn-plan","delta":"- inspect\n"}),
                json!({"type":"item/plan/delta","turn_id":"turn-plan","item_id":"plan-1","delta":"- patch\n"}),
                json!({"type":"turn/plan/updated","turn_id":"turn-plan","plan":{"text":"- inspect\n- patch\n- test"}}),
                json!({"type":"response_item","turn_id":"turn-plan","payload":{"type":"Plan","id":"plan-1","text":"- inspect\n- patch\n- test","status":"completed"}}),
            ]
            .iter(),
        );

        assert_eq!(blocks.len(), 1);
        let block = &blocks[0];
        assert_eq!(block.kind, "plan");
        assert_eq!(block.display_kind.as_deref(), Some("plan"));
        assert_eq!(block.group_id.as_deref(), Some("plan-turn-turn-plan"));
        assert_eq!(block.item_id.as_deref(), Some("plan-1"));
        assert_eq!(block.status.as_deref(), Some("completed"));
        assert_eq!(block.resolved, Some(true));
        assert_eq!(block.text.as_deref(), Some("- inspect\n- patch\n- test"));
    }

    #[test]
    fn user_input_answer_clears_pending_but_turn_completion_and_progress_do_not() {
        let answered = scan_fixture(&[
            json!({"type":"turn_started","turn_id":"turn-choice"}),
            json!({"type":"response_item","turn_id":"turn-choice","payload":{"type":"function_call","name":"request_user_input","call_id":"call-choice","arguments":{"questions":[{"id":"choice","question":"选择方案","options":[{"label":"A"}]}]}}}),
            json!({"type":"response_item","turn_id":"turn-choice","payload":{"type":"UserInputAnswer","call_id":"call-choice","answers":{"choice":["A"]}}}),
        ]);
        assert!(!answered.reply_needed);
        assert!(answered.pending_elicitation.is_none());

        let completed = scan_fixture(&[
            json!({"type":"turn_started","turn_id":"turn-choice"}),
            json!({"type":"response_item","turn_id":"turn-choice","payload":{"type":"function_call","name":"request_user_input","call_id":"call-choice","arguments":{"questions":[{"id":"choice","question":"选择方案","options":[{"label":"A"}]}]}}}),
            json!({"type":"turn_completed","turn_id":"turn-choice"}),
        ]);
        assert!(completed.reply_needed);
        assert!(completed.pending_elicitation.is_some());

        let progressed = scan_fixture(&[
            json!({"type":"turn_started","turn_id":"turn-choice"}),
            json!({"type":"response_item","turn_id":"turn-choice","payload":{"type":"function_call","name":"request_user_input","call_id":"call-choice","arguments":{"questions":[{"id":"choice","question":"选择方案","options":[{"label":"A"}]}]}}}),
            json!({"type":"event_msg","payload":{"type":"progress","turn_id":"turn-choice","message":"continuing"}}),
        ]);
        assert!(progressed.reply_needed);
        assert!(progressed.pending_elicitation.is_some());
    }

    #[test]
    fn request_user_input_output_becomes_resolved_history_block() {
        let blocks = super::message_blocks_from_events(
            [
                json!({"type":"turn_started","turn_id":"turn-choice"}),
                json!({"type":"response_item","turn_id":"turn-choice","payload":{"type":"function_call","name":"request_user_input","call_id":"call-choice","arguments":{"questions":[{"id":"choice","question":"选择方案","options":[{"label":"A"}]}]}}}),
                json!({"type":"response_item","turn_id":"turn-choice","payload":{"type":"function_call_output","call_id":"call-choice","output":"{\"choice\":[\"A\"]}"}}),
            ]
            .iter(),
        );

        assert_eq!(blocks.len(), 1);
        let block = &blocks[0];
        assert_eq!(block.kind, "request_user_input_result");
        assert_eq!(block.display_kind.as_deref(), Some("question_result"));
        assert_eq!(block.status.as_deref(), Some("completed"));
        assert_eq!(block.resolved, Some(true));
        assert_eq!(block.questions[0].question, "选择方案");
        assert_eq!(block.answers[0].question_id, "choice");
        assert_eq!(block.answers[0].answers, vec!["A".to_string()]);
    }

    #[test]
    fn internal_roles_and_reasoning_protocol_blocks_are_filtered() {
        let detail = detail_fixture(&[
            json!({"type":"response_item","payload":{"type":"message","role":"system","content":[{"text":"system context"}]}}),
            json!({"type":"response_item","payload":{"type":"message","role":"developer","content":[{"text":"developer context"}]}}),
            json!({"type":"response_item","payload":{"type":"reasoning_delta","summary":[{"text":"hidden reasoning"}]}}),
            json!({"type":"response_item","payload":{"type":"internal","text":"hidden internal"}}),
            json!({"type":"response_item","payload":{"type":"subagent","text":"hidden subagent"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"visible user"}]}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"visible assistant"}]}}),
        ]);

        assert_eq!(detail.blocks.len(), 2);
        assert_eq!(detail.messages.len(), 2);
        assert_eq!(detail.blocks[0].text.as_deref(), Some("visible user"));
        assert_eq!(detail.blocks[1].text.as_deref(), Some("visible assistant"));
    }

    #[test]
    fn message_blocks_parse_request_user_input_protocol_shapes() {
        let events = [
            json!({
                "method":"item/tool/requestUserInput",
                "params":{
                    "turnId":"turn-choice",
                    "itemId":"item-choice",
                    "questions":[{
                        "id":"q1",
                        "header":"选择",
                        "question":"选择方案",
                        "options":[{"label":"A","description":"执行 A"}]
                    }]
                }
            }),
            json!({
                "type":"response_item",
                "payload":{
                    "type":"function_call",
                    "toolName":"requestUserInput",
                    "callId":"call-choice",
                    "input":{
                        "arguments":{
                            "questions":[{
                                "id":"q2",
                                "question":"继续吗",
                                "options":[{"value":"继续"}]
                            }]
                        }
                    }
                }
            }),
        ];

        let blocks = super::message_blocks_from_events(events.iter());

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind, "request_user_input");
        assert_eq!(blocks[0].turn_id.as_deref(), Some("turn-choice"));
        assert_eq!(blocks[0].item_id.as_deref(), Some("item-choice"));
        assert_eq!(blocks[0].questions[0].question, "选择方案");
        assert_eq!(blocks[0].questions[0].options[0].label, "A");
        assert_eq!(blocks[1].call_id.as_deref(), Some("call-choice"));
        assert_eq!(blocks[1].questions[0].options[0].label, "继续");
    }

    #[test]
    fn thread_detail_blocks_merge_function_call_with_output() {
        let detail = detail_fixture(&[
            json!({"type":"response_item","timestamp":"2026-06-07T10:00:00Z","payload":{"type":"function_call","name":"exec_command","call_id":"call-1","status":"completed","arguments":{"cmd":"pwd"}}}),
            json!({"type":"response_item","timestamp":"2026-06-07T10:00:01Z","payload":{"type":"function_call_output","call_id":"call-1","output":"Output:\n/home/ubuntu"}}),
        ]);

        assert_eq!(detail.blocks.len(), 1);
        let block = &detail.blocks[0];
        assert_eq!(block.role, "tool");
        assert_eq!(block.kind, "function_call_output");
        assert_eq!(block.tool_name.as_deref(), Some("exec_command"));
        assert_eq!(block.call_id.as_deref(), Some("call-1"));
        assert_eq!(block.input.as_deref(), Some("{\n  \"cmd\": \"pwd\"\n}"));
        assert_eq!(block.text.as_deref(), Some("Output:\n/home/ubuntu"));
    }

    #[test]
    fn thread_detail_blocks_do_not_emit_empty_function_call_shells() {
        let detail = detail_fixture(&[
            json!({"type":"response_item","payload":{"type":"function_call","name":"exec_command","call_id":"call-1","arguments":{"cmd":"pwd"}}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"done"}]}}),
        ]);

        assert_eq!(detail.blocks.len(), 2);
        assert_eq!(detail.blocks[0].role, "assistant");
        assert_eq!(detail.blocks[0].text.as_deref(), Some("done"));
        assert_eq!(detail.blocks[1].role, "tool");
        assert_eq!(detail.blocks[1].status.as_deref(), Some("running"));
        assert_eq!(detail.blocks[1].tool_name.as_deref(), Some("exec_command"));
        assert!(detail.blocks[1]
            .text
            .as_deref()
            .unwrap_or_default()
            .is_empty());
    }

    #[test]
    fn thread_detail_blocks_clear_wait_agent_after_task_complete_same_turn() {
        let detail = detail_fixture(&[
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-main"}}),
            json!({"type":"response_item","turn_id":"turn-main","payload":{"type":"function_call","name":"wait_agent","call_id":"wait-agent-1","arguments":{"targets":["agent-1"]}}}),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-main","last_agent_message":"主线程完成。"}}),
        ]);

        assert_eq!(detail.summary.status, ThreadStatus::Recent);
        assert!(detail.summary.active_turn_id.is_none());
        assert!(!detail.blocks.iter().any(|block| {
            block.call_id.as_deref() == Some("wait-agent-1")
                && block.status.as_deref().is_some_and(|status| {
                    matches!(
                        status,
                        "pending" | "running" | "in_progress" | "inProgress" | "active"
                    )
                })
        }));
    }

    #[test]
    fn thread_detail_blocks_merge_custom_tool_call_with_output() {
        let detail = detail_fixture(&[
            json!({"type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","call_id":"call-2","status":"completed","input":"*** Begin Patch"}}),
            json!({"type":"response_item","payload":{"type":"custom_tool_call_output","call_id":"call-2","output":"Success. Updated files."}}),
        ]);

        assert_eq!(detail.blocks.len(), 1);
        let block = &detail.blocks[0];
        assert_eq!(block.role, "tool");
        assert_eq!(block.kind, "custom_tool_call_output");
        assert_eq!(block.tool_name.as_deref(), Some("apply_patch"));
        assert_eq!(block.input.as_deref(), Some("*** Begin Patch"));
        assert_eq!(block.text.as_deref(), Some("Success. Updated files."));
    }

    #[test]
    fn thread_detail_blocks_keep_orphan_tool_outputs() {
        let detail = detail_fixture(&[json!({
            "type":"response_item",
            "payload":{"type":"function_call_output","call_id":"missing-call","output":"orphan output"}
        })]);

        assert_eq!(detail.blocks.len(), 1);
        let block = &detail.blocks[0];
        assert_eq!(block.role, "tool");
        assert_eq!(block.kind, "function_call_output");
        assert_eq!(block.call_id.as_deref(), Some("missing-call"));
        assert_eq!(block.text.as_deref(), Some("orphan output"));
    }

    #[test]
    fn scan_rollout_ignores_subagent_notifications_for_latest_message() {
        let scan = scan_fixture(&[
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"<subagent_notification>{\"agent_path\":\"/tmp/child\",\"status\":{\"completed\":\"done\"}}</subagent_notification>"}]}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"真实回复"}]}}),
        ]);

        assert_eq!(scan.message_count, 1);
        assert_eq!(scan.latest_message.as_deref(), Some("真实回复"));
    }

    #[test]
    fn rollout_latest_assistant_message_reads_last_visible_assistant_message() {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "nexushub-rollout-latest-assistant-{}-{counter}.jsonl",
            std::process::id()
        ));
        let events = [
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"first answer"}]}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"next"}]}}),
            json!({"type":"subagent_notification","message":"worker done"}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"final answer"}]}}),
        ];
        fs::write(
            &path,
            events
                .iter()
                .map(serde_json::Value::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();

        assert_eq!(
            super::rollout_latest_assistant_message(&path)
                .unwrap()
                .as_deref(),
            Some("final answer")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn list_threads_preserves_db_rollout_path_when_session_index_misses_thread() {
        let root = unique_temp_dir("db-rollout-path");
        fs::create_dir_all(&root).unwrap();
        let rollout = root.join("db-only-rollout.jsonl");
        fs::write(
            &rollout,
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"from db rollout"}]}})
                .to_string(),
        )
        .unwrap();
        fs::write(
            root.join("session_index.jsonl"),
            json!({"id":"other-thread","path":root.join("other.jsonl")}).to_string(),
        )
        .unwrap();
        write_thread_db(&root, "test-thread", &rollout, 1, 0);

        let rows = list_threads(&CodexPaths::new(&root), None, None, 10).unwrap();

        let row = rows
            .iter()
            .find(|thread| thread.id == "test-thread")
            .unwrap();
        assert_eq!(row.rollout_path.as_deref(), Some(rollout.as_path()));
        assert_eq!(row.latest_message.as_deref(), Some("from db rollout"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rollout_session_meta_subagent_thread_is_hidden_from_list() {
        let root = unique_temp_dir("rollout-subagent-hidden");
        fs::create_dir_all(&root).unwrap();
        let rollout = root.join("rollout-subagent.jsonl");
        fs::write(
            &rollout,
            [
                json!({"session_meta":{"payload":{"thread_source":"subagent","parent_thread_id":"parent","agent_nickname":"worker","agent_role":"explorer"}}}).to_string(),
                json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"worker result"}]}}).to_string(),
            ]
            .join("\n"),
        )
        .unwrap();
        write_thread_db(&root, "child-thread", &rollout, 1, 0);

        let rows = list_threads(&CodexPaths::new(&root), None, None, 10).unwrap();

        assert!(rows.iter().all(|row| row.id != "child-thread"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn db_metadata_subagent_thread_is_hidden_from_list() {
        let root = unique_temp_dir("db-subagent-hidden");
        fs::create_dir_all(&root).unwrap();
        let rollout = root.join("rollout-subagent.jsonl");
        fs::write(&rollout, "").unwrap();
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        conn.execute_batch(
            "CREATE TABLE threads(
                id TEXT PRIMARY KEY,
                rollout_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                thread_source TEXT NOT NULL,
                parent_thread_id TEXT,
                agent_nickname TEXT,
                agent_role TEXT,
                cwd TEXT NOT NULL,
                title TEXT NOT NULL
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads(id, rollout_path, created_at, updated_at, thread_source, parent_thread_id, agent_nickname, agent_role, cwd, title)
             VALUES('child-thread', ?1, 1, 1, 'subagent', 'parent-thread', 'worker', 'explorer', '/tmp', 'worker')",
            [rollout.display().to_string()],
        )
        .unwrap();

        let rows = list_threads(&CodexPaths::new(&root), None, None, 10).unwrap();

        assert!(rows.iter().all(|row| row.id != "child-thread"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn hidden_thread_ids_exports_state_db_subagent_metadata_for_app_server_pruning() {
        let root = unique_temp_dir("db-hidden-thread-ids");
        fs::create_dir_all(&root).unwrap();
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        conn.execute_batch(
            "CREATE TABLE threads(
                id TEXT PRIMARY KEY,
                thread_source TEXT NOT NULL,
                source TEXT,
                parent_thread_id TEXT,
                agent_nickname TEXT,
                agent_role TEXT
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads(id, thread_source, source, parent_thread_id, agent_nickname, agent_role)
             VALUES('main-thread', 'user', 'vscode', NULL, NULL, NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads(id, thread_source, source, parent_thread_id, agent_nickname, agent_role)
             VALUES('child-thread', 'subagent', '{\"subagent\":{\"thread_spawn\":{\"parent_thread_id\":\"main-thread\"}}}', 'main-thread', 'worker', 'explorer')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads(id, thread_source, source, parent_thread_id, agent_nickname, agent_role)
             VALUES('child-source-json', 'user', '{\"subagent\":{\"thread_spawn\":{\"parent_thread_id\":\"main-thread\"}}}', NULL, NULL, NULL)",
            [],
        )
        .unwrap();

        let hidden = hidden_thread_ids(&CodexPaths::new(&root)).unwrap();

        assert!(hidden.contains("child-thread"));
        assert!(hidden.contains("child-source-json"));
        assert!(!hidden.contains("main-thread"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn main_thread_with_interactive_source_metadata_remains_visible() {
        let root = unique_temp_dir("db-main-source-visible");
        fs::create_dir_all(&root).unwrap();
        let rollout = root.join("rollout-main.jsonl");
        fs::write(
            &rollout,
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"main answer"}]}})
                .to_string(),
        )
        .unwrap();
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        conn.execute_batch(
            "CREATE TABLE threads(
                id TEXT PRIMARY KEY,
                rollout_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                source_kind TEXT NOT NULL,
                cwd TEXT NOT NULL,
                title TEXT NOT NULL
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads(id, rollout_path, created_at, updated_at, source_kind, cwd, title)
             VALUES('main-thread', ?1, 1, 1, 'cli', '/tmp', 'main')",
            [rollout.display().to_string()],
        )
        .unwrap();

        let rows = list_threads(&CodexPaths::new(&root), None, None, 10).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "main-thread");
        assert_eq!(rows[0].latest_message.as_deref(), Some("main answer"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn exec_readonly_verification_threads_are_hidden_from_main_list() {
        let root = unique_temp_dir("db-internal-exec-hidden");
        fs::create_dir_all(&root).unwrap();
        let rollout = root.join("rollout-internal.jsonl");
        fs::write(&rollout, "").unwrap();
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        conn.execute_batch(
            "CREATE TABLE threads(
                id TEXT PRIMARY KEY,
                rollout_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                source TEXT NOT NULL,
                thread_source TEXT NOT NULL,
                has_user_event INTEGER NOT NULL,
                first_user_message TEXT NOT NULL,
                preview TEXT NOT NULL,
                cwd TEXT NOT NULL,
                title TEXT NOT NULL
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads(id, rollout_path, created_at, updated_at, source, thread_source, has_user_event, first_user_message, preview, cwd, title)
             VALUES('internal-exec', ?1, 1, 2, 'exec', 'user', 0, '只读验证任务。不要修改文件。使用 tool_search 查询 spawn_agent。', '只读验证任务。', '/tmp', '只读验证任务。不要修改文件。')",
            [rollout.display().to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads(id, rollout_path, created_at, updated_at, source, thread_source, has_user_event, first_user_message, preview, cwd, title)
             VALUES('internal-subagent-prompt', ?1, 1, 2, 'exec', 'user', 0, '', '', '/tmp', '你是子代理 A，必须使用 gpt-5.5 和 xhigh。')",
            [rollout.display().to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads(id, rollout_path, created_at, updated_at, source, thread_source, has_user_event, first_user_message, preview, cwd, title)
             VALUES('main-thread', ?1, 1, 1, 'vscode', 'user', 0, '接手这个线程的工作，修复项目。', '接手这个线程的工作，修复项目。', '/tmp', 'wanka')",
            [rollout.display().to_string()],
        )
        .unwrap();

        let rows = list_threads(&CodexPaths::new(&root), None, None, 10).unwrap();
        let hidden = hidden_thread_ids(&CodexPaths::new(&root)).unwrap();
        let counts = thread_source_counts(&CodexPaths::new(&root)).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "main-thread");
        assert!(hidden.contains("internal-exec"));
        assert!(hidden.contains("internal-subagent-prompt"));
        assert_eq!(counts.get("internal").copied(), Some(2));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn thread_detail_collapses_old_completed_tool_history() {
        let mut events = Vec::new();
        events.push(json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"starting"}]}}));
        for index in 0..90 {
            events.push(json!({"type":"response_item","payload":{"type":"function_call","name":"exec_command","call_id":format!("call-{index}"),"arguments":{"cmd":"pwd"}}}));
            events.push(json!({"type":"response_item","payload":{"type":"function_call_output","call_id":format!("call-{index}"),"output":format!("out-{index}")}}));
        }

        let detail = detail_fixture(&events);

        assert!(detail.blocks.len() < 90);
        assert!(detail
            .blocks
            .iter()
            .any(|block| block.id == "completed-tool-history-collapsed"));
        assert!(detail
            .blocks
            .iter()
            .any(|block| block.text.as_deref() == Some("out-89")));
    }

    #[test]
    fn thread_detail_collapses_old_chat_history_but_keeps_recent_and_running_blocks() {
        let mut events = Vec::new();
        events.push(json!({"type":"response_item","payload":{"type":"function_call","name":"exec_command","call_id":"live-call","arguments":{"cmd":"tail -f log"}}}));
        for index in 0..180 {
            events.push(json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":format!("assistant message {index}")}]}}));
        }

        let detail = detail_fixture(&events);

        assert!(detail.blocks.len() < 100);
        assert!(detail
            .blocks
            .iter()
            .any(|block| block.id == "chat-history-collapsed"));
        assert!(!detail
            .blocks
            .iter()
            .any(|block| block.text.as_deref() == Some("assistant message 0")));
        assert!(detail
            .blocks
            .iter()
            .any(|block| block.text.as_deref() == Some("assistant message 179")));
        assert!(detail.blocks.iter().any(|block| {
            block.role == "tool"
                && block.status.as_deref() == Some("running")
                && block.call_id.as_deref() == Some("live-call")
        }));
    }

    #[test]
    fn window_thread_detail_returns_latest_window_with_cursor() {
        let events = (0..6)
            .map(|index| json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":format!("message-{index}")}]}}))
            .collect::<Vec<_>>();
        let window = window_thread_detail(detail_fixture(&events), Some(2), None);

        assert_eq!(window.total_blocks, 6);
        assert!(window.has_more_blocks);
        assert_eq!(window.before_cursor.as_deref(), Some("b:4"));
        assert!(window.messages.is_empty());
        assert_eq!(window.blocks.len(), 2);
        assert_eq!(window.blocks[0].text.as_deref(), Some("message-4"));
        assert_eq!(window.blocks[1].text.as_deref(), Some("message-5"));
    }

    #[test]
    fn window_thread_detail_uses_before_cursor_for_older_window() {
        let events = (0..6)
            .map(|index| json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":format!("message-{index}")}]}}))
            .collect::<Vec<_>>();
        let window = window_thread_detail(detail_fixture(&events), Some(2), Some("b:4"));

        assert_eq!(window.total_blocks, 6);
        assert!(window.has_more_blocks);
        assert_eq!(window.before_cursor.as_deref(), Some("b:2"));
        assert_eq!(window.blocks.len(), 2);
        assert_eq!(window.blocks[0].text.as_deref(), Some("message-2"));
        assert_eq!(window.blocks[1].text.as_deref(), Some("message-3"));
    }

    #[test]
    fn window_thread_detail_returns_empty_at_start_cursor() {
        let events = (0..3)
            .map(|index| json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":format!("message-{index}")}]}}))
            .collect::<Vec<_>>();
        let window = window_thread_detail(detail_fixture(&events), Some(2), Some("b:0"));

        assert_eq!(window.total_blocks, 3);
        assert!(!window.has_more_blocks);
        assert_eq!(window.before_cursor, None);
        assert!(window.messages.is_empty());
        assert!(window.blocks.is_empty());
    }

    #[test]
    fn window_thread_detail_ignores_invalid_before_cursor() {
        let events = (0..5)
            .map(|index| json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":format!("message-{index}")}]}}))
            .collect::<Vec<_>>();
        let window = window_thread_detail(detail_fixture(&events), Some(2), Some("not-a-cursor"));

        assert_eq!(window.total_blocks, 5);
        assert!(window.has_more_blocks);
        assert_eq!(window.before_cursor.as_deref(), Some("b:3"));
        assert_eq!(window.blocks.len(), 2);
        assert_eq!(window.blocks[0].text.as_deref(), Some("message-3"));
        assert_eq!(window.blocks[1].text.as_deref(), Some("message-4"));
    }

    #[test]
    fn archived_thread_keeps_archived_status_despite_running_rollout() {
        let root = unique_temp_dir("archived-priority");
        fs::create_dir_all(&root).unwrap();
        let rollout = root.join("archived-rollout.jsonl");
        fs::write(
            &rollout,
            [
                json!({"type":"turn_started","turn_id":"turn-active"}).to_string(),
                json!({"type":"response_item","turn_id":"turn-active","payload":{"type":"function_call","name":"request_user_input","call_id":"call-1","arguments":{"questions":[{"id":"q","question":"continue?","options":[{"label":"yes"}]}]}}}).to_string(),
            ].join("\n"),
        )
        .unwrap();
        write_thread_db(&root, "test-thread", &rollout, 1, 1);

        let row = list_threads(&CodexPaths::new(&root), None, None, 10)
            .unwrap()
            .into_iter()
            .find(|thread| thread.id == "test-thread")
            .unwrap();

        assert_eq!(row.status, ThreadStatus::Archived);
        assert_eq!(row.active_turn_id.as_deref(), Some("turn-active"));
        assert!(row.pending_elicitation.is_some());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn scan_rollout_latest_message_uses_last_message_beyond_window() {
        let mut events = Vec::new();
        for index in 0..100 {
            events.push(json!({
                "type":"response_item",
                "payload":{"type":"message","role":"assistant","content":[{"text":format!("message-{index}")}]}
            }));
        }

        let scan = scan_fixture(&events);

        assert_eq!(scan.message_count, 100);
        assert_eq!(scan.latest_message.as_deref(), Some("message-99"));
    }

    #[test]
    fn list_threads_full_fetch_keeps_running_thread_beyond_small_page() {
        let root = unique_temp_dir("full-fetch-running");
        fs::create_dir_all(&root).unwrap();
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        conn.execute_batch(
            "CREATE TABLE threads(
                id TEXT PRIMARY KEY,
                rollout_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                source TEXT NOT NULL,
                model_provider TEXT NOT NULL,
                cwd TEXT NOT NULL,
                title TEXT NOT NULL,
                sandbox_policy TEXT NOT NULL,
                approval_mode TEXT NOT NULL,
                archived INTEGER NOT NULL DEFAULT 0,
                preview TEXT NOT NULL DEFAULT ''
            );",
        )
        .unwrap();
        for index in 0..20 {
            let rollout = root.join(format!("rollout-recent-{index}.jsonl"));
            fs::write(&rollout, "").unwrap();
            conn.execute(
                "INSERT INTO threads(id, rollout_path, created_at, updated_at, source, model_provider, cwd, title, sandbox_policy, approval_mode, archived, preview)
                 VALUES(?1, ?2, 1, ?3, 'codex', '', '/tmp', ?1, '', '', 0, '')",
                (
                    format!("recent-{index}"),
                    rollout.display().to_string(),
                    10_000 + index,
                ),
            )
            .unwrap();
        }
        let running_rollout = root.join("rollout-running-old.jsonl");
        fs::write(
            &running_rollout,
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-live"}})
                .to_string(),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads(id, rollout_path, created_at, updated_at, source, model_provider, cwd, title, sandbox_policy, approval_mode, archived, preview)
             VALUES('running-old', ?1, 1, 1, 'codex', '', '/tmp', 'running-old', '', '', 0, '')",
            [running_rollout.display().to_string()],
        )
        .unwrap();

        let first_page = list_threads(&CodexPaths::new(&root), None, None, 10).unwrap();
        assert!(!first_page.iter().any(|thread| thread.id == "running-old"));
        let running =
            list_threads(&CodexPaths::new(&root), Some("running"), None, usize::MAX).unwrap();
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].id, "running-old");
        assert_eq!(running[0].active_turn_id.as_deref(), Some("turn-live"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn scan_rollout_running_when_turn_started_without_completion() {
        let scan = scan_fixture(&[
            json!({"type":"turn_started","turn_id":"turn-running"}),
            json!({"type":"response_item","turn_id":"turn-running","payload":{"type":"message","role":"assistant","content":[{"text":"working"}]}}),
        ]);

        assert!(scan.running);
        assert_eq!(scan.active_turn_id.as_deref(), Some("turn-running"));
    }

    #[test]
    fn scan_rollout_running_when_event_msg_task_started_without_turn_started() {
        let scan = scan_fixture(&[
            json!({"type":"event_msg","payload":{"type":"task_complete"}}),
            json!({"type":"event_msg","payload":{"type":"task_started"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"text":"continue"}]}}),
        ]);

        assert!(scan.running);
        assert!(scan.active_turn_id.is_none());
    }

    #[test]
    fn scan_rollout_event_msg_task_started_uses_payload_turn_id_as_active_turn() {
        let scan = scan_fixture(&[
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-old","last_agent_message":"done"}}),
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-live"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"working"}]}}),
        ]);

        assert!(scan.running);
        assert_eq!(scan.active_turn_id.as_deref(), Some("turn-live"));
    }

    #[test]
    fn scan_rollout_task_complete_for_prior_turn_does_not_clear_newer_task() {
        let scan = scan_fixture(&[
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-old"}}),
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-live"}}),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-old","last_agent_message":"done"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"still working"}]}}),
        ]);

        assert!(scan.running);
        assert_eq!(scan.active_turn_id.as_deref(), Some("turn-live"));
    }

    #[test]
    fn scan_rollout_latest_task_complete_clears_stale_older_task() {
        let scan = scan_fixture(&[
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-stale"}}),
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-latest"}}),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-latest","last_agent_message":"done"}}),
        ]);

        assert!(!scan.running);
        assert!(scan.active_turn_id.is_none());
    }

    #[test]
    fn scan_rollout_named_task_complete_clears_prior_anonymous_task_started() {
        let scan = scan_fixture(&[
            json!({"type":"event_msg","payload":{"type":"task_started"}}),
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-live"}}),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-live","last_agent_message":"done"}}),
        ]);

        assert!(!scan.running);
        assert!(scan.active_turn_id.is_none());
    }

    #[test]
    fn scan_rollout_prior_named_task_complete_preserves_newer_anonymous_task_started() {
        let scan = scan_fixture(&[
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-old"}}),
            json!({"type":"event_msg","payload":{"type":"task_started"}}),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-old","last_agent_message":"done"}}),
        ]);

        assert!(scan.running);
        assert!(scan.active_turn_id.is_none());
    }

    #[test]
    fn scan_rollout_turn_completed_for_prior_turn_does_not_clear_active_turn() {
        let scan = scan_fixture(&[
            json!({"type":"turn_started","turn_id":"turn-live"}),
            json!({"type":"turn_completed","turn_id":"turn-old"}),
            json!({"type":"response_item","turn_id":"turn-live","payload":{"type":"message","role":"assistant","content":[{"text":"still working"}]}}),
        ]);

        assert!(scan.running);
        assert_eq!(scan.active_turn_id.as_deref(), Some("turn-live"));
    }

    #[test]
    fn scan_rollout_xianbao_style_live_task_stays_running_after_completed_tools() {
        let scan = scan_fixture(&[
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-old"}}),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-old","last_agent_message":null}}),
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-live","model_context_window":258400}}),
            json!({"type":"turn_context","payload":{"turn_id":"turn-live","cwd":"/home/ubuntu/codex-workspace","model":"gpt-5.5"}}),
            json!({"type":"response_item","turn_id":"turn-live","payload":{"type":"custom_tool_call","status":"completed","call_id":"call-edit","name":"apply_patch","input":"*** Begin Patch"}}),
            json!({"type":"event_msg","payload":{"type":"patch_apply_end","turn_id":"turn-live","call_id":"call-edit","success":true}}),
            json!({"type":"response_item","turn_id":"turn-live","payload":{"type":"custom_tool_call_output","call_id":"call-edit","output":"Success"}}),
            json!({"type":"response_item","turn_id":"turn-live","payload":{"type":"function_call","name":"exec_command","call_id":"call-test","arguments":{"cmd":"python3 -m unittest"}}}),
            json!({"type":"response_item","turn_id":"turn-live","payload":{"type":"function_call_output","call_id":"call-test","output":"OK"}}),
            json!({"type":"event_msg","payload":{"type":"agent_message","message":"继续处理中","phase":"commentary"}}),
        ]);

        assert!(scan.running);
        assert_eq!(scan.active_turn_id.as_deref(), Some("turn-live"));
        assert_eq!(scan.latest_message.as_deref(), Some("继续处理中"));
    }

    #[test]
    fn scan_rollout_anonymous_task_complete_does_not_clear_explicit_active_turn() {
        let scan = scan_fixture(&[
            json!({"type":"turn_started","turn_id":"turn-live"}),
            json!({"type":"event_msg","payload":{"type":"task_complete","last_agent_message":"old done"}}),
            json!({"type":"response_item","turn_id":"turn-live","payload":{"type":"message","role":"assistant","content":[{"text":"still working"}]}}),
        ]);

        assert!(scan.running);
        assert_eq!(scan.active_turn_id.as_deref(), Some("turn-live"));
    }

    #[test]
    fn scan_rollout_nested_event_msg_payload_paths_provide_task_turn_id() {
        let scan = scan_fixture(&[
            json!({"type":"event_msg","payload":{"event":{"type":"task_started","turnId":"turn-live"}}}),
            json!({"type":"event_msg","payload":{"event_type":"token_count","payload":{"turn_id":"turn-live"}}}),
        ]);

        assert!(scan.running);
        assert_eq!(scan.active_turn_id.as_deref(), Some("turn-live"));
    }

    #[test]
    fn scan_rollout_external_xianbao_fixture_when_provided() {
        let Ok(path) = env::var("XIANBAO_ROLLOUT_FIXTURE") else {
            return;
        };
        let scan = scan_rollout(Path::new(&path), 80).unwrap();

        assert!(
            scan.running,
            "expected external xianbao fixture to be running"
        );
        assert_eq!(
            scan.active_turn_id.as_deref(),
            Some("019ea8d1-d740-7233-8488-cd06d0b0ea57")
        );
    }

    #[test]
    fn scan_rollout_external_ld_fixture_when_provided() {
        let Ok(path) = env::var("LD_ROLLOUT_FIXTURE") else {
            return;
        };
        let scan = scan_rollout(Path::new(&path), 80).unwrap();

        assert!(
            !scan.running,
            "expected external LD fixture to be completed"
        );
        assert!(
            !scan.recoverable,
            "expected external LD fixture to end with a successful task_complete"
        );
        assert_eq!(scan.active_turn_id, None);
        assert!(scan.pending_elicitation.is_none());
    }

    #[test]
    fn scan_rollout_later_successful_task_complete_clears_stale_recoverable() {
        let scan = scan_fixture(&[
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-old"}}),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-old","last_agent_message":null}}),
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-latest"}}),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-latest","last_agent_message":"done"}}),
        ]);

        assert!(!scan.running);
        assert!(!scan.recoverable);
        assert!(scan.active_turn_id.is_none());
    }

    #[test]
    fn scan_rollout_event_msg_task_complete_clears_task_started_running() {
        let scan = scan_fixture(&[
            json!({"type":"event_msg","payload":{"type":"task_started"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"working"}]}}),
            json!({"type":"event_msg","payload":{"type":"task_complete","last_agent_message":"done"}}),
        ]);

        assert!(!scan.running);
        assert!(scan.active_turn_id.is_none());
    }

    #[test]
    fn scan_rollout_event_msg_turn_aborted_clears_task_started_running() {
        let scan = scan_fixture(&[
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-abort"}}),
            json!({"type":"turn_started","turn_id":"turn-abort"}),
            json!({"type":"response_item","turn_id":"turn-abort","payload":{"type":"function_call","name":"exec_command","call_id":"call-abort","arguments":{"cmd":"sleep 10"}}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<turn_aborted>"}]}}),
            json!({"type":"event_msg","payload":{"type":"turn_aborted","turn_id":"turn-abort","reason":"interrupted"}}),
        ]);

        assert!(!scan.running);
        assert!(scan.active_turn_id.is_none());
    }

    #[test]
    fn scan_rollout_slash_turn_aborted_clears_running() {
        let scan = scan_fixture(&[
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-abort"}}),
            json!({"type":"turn_started","turn_id":"turn-abort"}),
            json!({"type":"turn/aborted","turn_id":"turn-abort","reason":"interrupted"}),
        ]);

        assert!(!scan.running);
        assert!(scan.active_turn_id.is_none());
    }

    #[test]
    fn scan_rollout_running_when_tool_call_has_no_output_without_turn_started() {
        let scan = scan_fixture(&[
            json!({"type":"response_item","turn_id":"turn-live","payload":{"type":"function_call","name":"exec_command","call_id":"call-live","arguments":{"cmd":"sleep 10"}}}),
        ]);

        assert!(scan.running);
        assert_eq!(scan.active_turn_id.as_deref(), Some("turn-live"));
    }

    #[test]
    fn scan_rollout_task_complete_clears_wait_agent_pending_tool_for_same_turn() {
        let scan = scan_fixture(&[
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-main"}}),
            json!({"type":"response_item","turn_id":"turn-main","payload":{"type":"function_call","name":"wait_agent","call_id":"wait-agent-1","arguments":{"targets":["agent-1"]}}}),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-main","last_agent_message":"主线程完成。"}}),
        ]);

        assert!(!scan.running);
        assert_eq!(scan.active_turn_id, None);
        assert!(!scan.recoverable);
    }

    #[test]
    fn scan_rollout_turn_completed_clears_wait_agent_pending_tool_for_same_turn() {
        let scan = scan_fixture(&[
            json!({"type":"turn_started","turn_id":"turn-main"}),
            json!({"type":"response_item","turn_id":"turn-main","payload":{"type":"function_call","name":"wait_agent","call_id":"wait-agent-1","arguments":{"targets":["agent-1"]}}}),
            json!({"type":"turn_completed","turn_id":"turn-main"}),
        ]);

        assert!(!scan.running);
        assert_eq!(scan.active_turn_id, None);
    }

    #[test]
    fn scan_rollout_task_complete_does_not_clear_newer_running_turn() {
        let scan = scan_fixture(&[
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-old"}}),
            json!({"type":"event_msg","payload":{"type":"task_started","turn_id":"turn-live"}}),
            json!({"type":"response_item","turn_id":"turn-live","payload":{"type":"function_call","name":"wait_agent","call_id":"wait-agent-live","arguments":{"targets":["agent-live"]}}}),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-old","last_agent_message":"旧 turn 完成。"}}),
        ]);

        assert!(scan.running);
        assert_eq!(scan.active_turn_id.as_deref(), Some("turn-live"));
    }

    #[test]
    fn scan_rollout_request_user_input_still_reply_needed_until_cleared() {
        let waiting = scan_fixture(&[
            json!({"type":"turn_started","turn_id":"turn-choice"}),
            json!({"type":"response_item","turn_id":"turn-choice","payload":{"type":"function_call","name":"request_user_input","call_id":"choice-1","arguments":{"questions":[{"id":"choice","question":"选择方案","options":[{"label":"A"}]}]}}}),
            json!({"type":"response_item","turn_id":"turn-choice","payload":{"type":"function_call","name":"wait_agent","call_id":"wait-agent-1","arguments":{"targets":["agent-1"]}}}),
        ]);
        assert!(waiting.reply_needed);
        assert!(waiting.pending_elicitation.is_some());

        let cleared = scan_fixture(&[
            json!({"type":"turn_started","turn_id":"turn-choice"}),
            json!({"type":"response_item","turn_id":"turn-choice","payload":{"type":"function_call","name":"request_user_input","call_id":"choice-1","arguments":{"questions":[{"id":"choice","question":"选择方案","options":[{"label":"A"}]}]}}}),
            json!({"type":"response_item","turn_id":"turn-choice","payload":{"type":"function_call","name":"wait_agent","call_id":"wait-agent-1","arguments":{"targets":["agent-1"]}}}),
            json!({"type":"response_item","turn_id":"turn-choice","payload":{"type":"function_call_output","call_id":"choice-1","output":"{\"choice\":[\"A\"]}"}}),
            json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-choice","last_agent_message":"选择已处理。"}}),
        ]);
        assert!(!cleared.reply_needed);
        assert!(cleared.pending_elicitation.is_none());
        assert!(!cleared.running);
    }

    #[test]
    fn scan_rollout_completed_tool_call_without_turn_started_is_recent() {
        let scan = scan_fixture(&[
            json!({"type":"response_item","turn_id":"turn-done","payload":{"type":"function_call","name":"exec_command","call_id":"call-done","arguments":{"cmd":"pwd"}}}),
            json!({"type":"response_item","turn_id":"turn-done","payload":{"type":"function_call_output","call_id":"call-done","output":"/tmp"}}),
        ]);

        assert!(!scan.running);
        assert!(scan.active_turn_id.is_none());
    }

    #[test]
    fn set_thread_title_updates_title_column_as_rename_fallback() {
        let root = unique_temp_dir("set-title");
        fs::create_dir_all(&root).unwrap();
        let rollout = root.join("rollout-thread-a.jsonl");
        fs::write(&rollout, "").unwrap();
        write_thread_db(&root, "thread-a", &rollout, 1, 0);

        set_thread_title(&CodexPaths::new(&root), "thread-a", "wanka").unwrap();

        let rows = list_threads(&CodexPaths::new(&root), None, Some("thread-a"), 10).unwrap();
        assert_eq!(rows[0].title, "wanka");
        let _ = fs::remove_dir_all(root);
    }

    fn scan_fixture(events: &[serde_json::Value]) -> super::RolloutScan {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "nexushub-rollout-test-{}-{}-{}.jsonl",
            std::process::id(),
            counter,
            events.len()
        ));
        let text = events
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, text).unwrap();
        let scan = scan_rollout(&path, 80).unwrap();
        let _ = fs::remove_file(path);
        scan
    }

    fn detail_fixture(events: &[serde_json::Value]) -> super::ThreadDetail {
        let root = unique_temp_dir("detail");
        fs::create_dir_all(&root).unwrap();
        let rollout = root.join("rollout-test-thread.jsonl");
        let text = events
            .iter()
            .map(serde_json::Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&rollout, text).unwrap();
        fs::write(
            root.join("session_index.jsonl"),
            json!({"id":"test-thread","path":rollout}).to_string(),
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
                model_provider TEXT NOT NULL,
                cwd TEXT NOT NULL,
                title TEXT NOT NULL,
                sandbox_policy TEXT NOT NULL,
                approval_mode TEXT NOT NULL,
                preview TEXT NOT NULL DEFAULT ''
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads(id, rollout_path, created_at, updated_at, source, model_provider, cwd, title, sandbox_policy, approval_mode, preview)
             VALUES('test-thread', '', 1, 1, 'codex', '', '/tmp', 'test', '', '', '')",
            [],
        )
        .unwrap();

        let detail = thread_detail(&CodexPaths::new(&root), "test-thread")
            .unwrap()
            .unwrap();
        let _ = fs::remove_dir_all(root);
        detail
    }

    fn write_thread_db(
        root: &Path,
        thread_id: &str,
        rollout: &std::path::Path,
        updated_at: i64,
        archived: i64,
    ) {
        let conn = Connection::open(root.join("state_5.sqlite")).unwrap();
        conn.execute_batch(
            "CREATE TABLE threads(
                id TEXT PRIMARY KEY,
                rollout_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                source TEXT NOT NULL,
                model_provider TEXT NOT NULL,
                cwd TEXT NOT NULL,
                title TEXT NOT NULL,
                sandbox_policy TEXT NOT NULL,
                approval_mode TEXT NOT NULL,
                archived INTEGER NOT NULL DEFAULT 0,
                preview TEXT NOT NULL DEFAULT ''
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads(id, rollout_path, created_at, updated_at, source, model_provider, cwd, title, sandbox_policy, approval_mode, archived, preview)
             VALUES(?1, ?2, 1, ?3, 'codex', '', '/tmp', 'test', '', '', ?4, '')",
            (thread_id, rollout.display().to_string(), updated_at, archived),
        )
        .unwrap();
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        env::temp_dir().join(format!(
            "nexushub-{label}-{}-{}-{}",
            std::process::id(),
            counter,
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ))
    }

    fn mark_codex_home(home: &Path) {
        fs::create_dir_all(home.join("sessions")).unwrap();
        fs::write(home.join("state_5.sqlite"), b"").unwrap();
        fs::write(home.join("session_index.jsonl"), b"").unwrap();
        fs::create_dir_all(home.join("app-server-control")).unwrap();
    }
}
