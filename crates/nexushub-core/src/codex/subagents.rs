//! Parent-bound, read-only access to native Codex child sessions.
use anyhow::{bail, ensure, Context, Result};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};

use super::{CodexPaths, ThreadDetail, ThreadStatus, ThreadSummary};

const MAX_DEPTH: usize = 32;
const MAX_RECORDS: usize = 20_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    Running,
    Completed,
    Failed,
    Interrupted,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubagentActivity {
    pub agent_id: Option<String>,
    pub name: String,
    pub role: Option<String>,
    pub status: SubagentStatus,
    pub available: bool,
    pub unavailable_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubagentDetailRequest {
    pub root_thread_id: String,
    pub agent_id: String,
    pub limit: Option<usize>,
    pub before: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentDetailResponse {
    pub root_thread_id: String,
    pub parent_thread_id: String,
    pub agent: SubagentActivity,
    pub detail: ThreadDetail,
}

#[derive(Clone)]
struct AgentRecord {
    id: String,
    parents: HashSet<String>,
    path: Option<String>,
    title: String,
    role: Option<String>,
    rollout: PathBuf,
    cwd: Option<String>,
    updated_at: Option<String>,
}

fn field(value: &Value, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn spawn_source(value: &Value) -> &Value {
    value.pointer("/subagent/thread_spawn").unwrap_or(value)
}

fn columns(conn: &Connection, table: &str) -> Result<HashSet<String>> {
    Ok(conn
        .prepare(&format!("PRAGMA table_info({table})"))?
        .query_map([], |row| row.get(1))?
        .collect::<rusqlite::Result<_>>()?)
}

fn read_graph(paths: &CodexPaths) -> Result<HashMap<String, AgentRecord>> {
    let conn = Connection::open_with_flags(paths.state_db(), OpenFlags::SQLITE_OPEN_READ_ONLY)
        .context("子智能体索引暂时不可用")?;
    let cols = columns(&conn, "threads")?;
    ensure!(
        cols.contains("id") && cols.contains("rollout_path"),
        "子智能体索引格式不支持"
    );
    let select = |names: &[&str]| {
        names
            .iter()
            .find(|name| cols.contains(**name))
            .map(|s| s.to_string())
            .unwrap_or_else(|| "NULL".into())
    };
    let sql = format!(
        "SELECT id, rollout_path, {}, {}, {}, {}, {}, {}, {}, {} FROM threads LIMIT {}",
        select(&["source"]),
        select(&["parent_thread_id", "parentThreadId"]),
        select(&["agent_path", "agentPath"]),
        select(&["title", "name"]),
        select(&["agent_nickname", "agentNickname"]),
        select(&["agent_role", "agentRole"]),
        select(&["cwd"]),
        select(&["updated_at", "created_at"]),
        MAX_RECORDS + 1
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |row| {
            let source: Option<String> = row.get(2)?;
            let source: Value = source
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(Value::Null);
            let source = spawn_source(&source);
            let mut parents = HashSet::new();
            if let Some(id) = row.get::<_, Option<String>>(3)?.filter(|s| !s.is_empty()) {
                parents.insert(id);
            }
            if let Some(id) = field(source, &["parent_thread_id", "parentThreadId"]) {
                parents.insert(id);
            }
            Ok(AgentRecord {
                id: row.get(0)?,
                rollout: row.get::<_, Option<String>>(1)?.unwrap_or_default().into(),
                parents,
                path: row
                    .get::<_, Option<String>>(4)?
                    .or_else(|| field(source, &["agent_path", "agentPath"])),
                title: row
                    .get::<_, Option<String>>(5)?
                    .filter(|title| !title.trim().is_empty())
                    .or(row.get::<_, Option<String>>(6)?)
                    .or_else(|| field(source, &["agent_nickname"]))
                    .unwrap_or_else(|| "子智能体".into()),
                role: row
                    .get::<_, Option<String>>(7)?
                    .or_else(|| field(source, &["agent_role"])),
                cwd: row.get(8)?,
                updated_at: row.get::<_, Option<String>>(9).unwrap_or_default(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(
        rows.len() <= MAX_RECORDS,
        "子智能体索引过大，暂时无法验证关联"
    );
    let mut graph: HashMap<_, _> = rows.into_iter().map(|r| (r.id.clone(), r)).collect();
    let edge_cols = columns(&conn, "thread_spawn_edges")?;
    if edge_cols.contains("parent_thread_id") && edge_cols.contains("child_thread_id") {
        let mut stmt =
            conn.prepare("SELECT parent_thread_id, child_thread_id FROM thread_spawn_edges")?;
        let mut rows = stmt.query([])?;
        let mut count = 0;
        while let Some(row) = rows.next()? {
            count += 1;
            ensure!(count <= MAX_RECORDS, "子智能体关联过多");
            let parent: String = row.get(0)?;
            if let Some(child) = graph.get_mut(&row.get::<_, String>(1)?) {
                child.parents.insert(parent);
            }
        }
    }
    Ok(graph)
}

fn validate_key(key: &str) -> Result<()> {
    ensure!(
        !key.is_empty()
            && key.len() <= 256
            && !key.chars().any(|c| c.is_control() || c == '/' || c == '\\'),
        "无效的线程身份"
    );
    Ok(())
}

fn validate_rollout(paths: &CodexPaths, record: &AgentRecord) -> Result<PathBuf> {
    let home = fs::canonicalize(&paths.home).context("会话根目录不可用")?;
    let path = fs::canonicalize(&record.rollout).context("子智能体记录已清理或不存在")?;
    ensure!(
        path.starts_with(home.join("sessions")) || path.starts_with(home.join("archived_sessions")),
        "子智能体记录不在会话目录内"
    );
    let mut component = record.rollout.as_path();
    while component != paths.home && component != home {
        ensure!(
            !fs::symlink_metadata(component)?.file_type().is_symlink(),
            "子智能体记录包含符号链接"
        );
        component = component.parent().context("子智能体路径无法验证")?;
    }
    ensure!(path.is_file(), "子智能体记录不是文件");
    let file = File::open(&path)?;
    let mut reader = BufReader::new(file.take(512 * 1024));
    let mut line = String::new();
    while reader.read_line(&mut line)? > 0 {
        if let Ok(value) = serde_json::from_str::<Value>(&line) {
            let meta = if value.get("type").and_then(Value::as_str) == Some("session_meta") {
                value.get("payload")
            } else {
                value.pointer("/session_meta/payload")
            };
            if let Some(meta) = meta {
                ensure!(
                    field(meta, &["id", "session_id"]).as_deref() == Some(&record.id),
                    "子智能体文件身份已变化"
                );
                if let Some(parent) = meta
                    .get("source")
                    .map(spawn_source)
                    .and_then(|s| field(s, &["parent_thread_id", "parentThreadId"]))
                    .or_else(|| field(meta, &["parent_thread_id", "parentThreadId"]))
                {
                    ensure!(
                        record.parents.len() == 1 && record.parents.contains(&parent),
                        "子智能体父线程身份冲突"
                    );
                }
                return Ok(path);
            }
        }
        line.clear();
    }
    bail!("子智能体文件缺少可验证身份")
}

fn validate_relation(
    paths: &CodexPaths,
    graph: &HashMap<String, AgentRecord>,
    root: &str,
    agent: &str,
) -> Result<()> {
    validate_key(root)?;
    validate_key(agent)?;
    ensure!(root != agent, "请选择子智能体");
    ensure!(
        super::local_thread_summary(paths, root)?.is_some(),
        "主线程已变化或不可读取"
    );
    let mut visited = HashSet::new();
    let mut id = agent;
    for _ in 0..MAX_DEPTH {
        if id == root {
            return Ok(());
        }
        ensure!(visited.insert(id), "子智能体关联存在循环");
        let record = graph.get(id).context("子智能体关联记录不存在")?;
        ensure!(record.parents.len() == 1, "子智能体父线程关联不明确");
        validate_rollout(paths, record)?;
        id = record.parents.iter().next().expect("one parent");
    }
    bail!("子智能体嵌套层级过深")
}

fn native_status(path: &Path) -> Result<SubagentStatus> {
    static CACHE: crate::read_cache::ReadCache<SubagentStatus> =
        crate::read_cache::ReadCache::new(1024 * 1024);
    CACHE.read(path, |_, _| 256, || {
        let mut status = SubagentStatus::Unknown;
        let mut turn = None;
        let mut pending = HashSet::new();
        let mut reader = BufReader::new(File::open(path)?);
        let mut line = String::new();
        while reader.read_line(&mut line)? > 0 {
            if !line.ends_with('\n') {
                return Ok(SubagentStatus::Unknown);
            }
            if !line.trim().is_empty() {
                let Ok(value) = serde_json::from_str::<Value>(&line) else {
                    return Ok(SubagentStatus::Unknown);
                };
                let event = value.get("payload").unwrap_or(&value);
                let kind = event.get("type").and_then(Value::as_str).unwrap_or("");
                let event_turn = field(event, &["turn_id", "turnId"]);
                match kind {
                    "task_started" | "turn_started" | "turn/started" => {
                        turn = event_turn;
                        pending.clear();
                        status = SubagentStatus::Running;
                    }
                    "task_complete" | "turn_completed" | "turn/completed" | "turn_aborted"
                    | "turn/aborted" | "turn_error" | "turn_failed"
                        if event_turn.is_none() || turn.is_none() || event_turn == turn =>
                    {
                        let terminal = field(event, &["status", "turn_status"]).unwrap_or_default();
                        status = if kind.contains("aborted")
                            || matches!(terminal.as_str(), "cancelled" | "canceled" | "interrupted")
                        {
                            SubagentStatus::Interrupted
                        } else if matches!(kind, "turn_error" | "turn_failed")
                            || matches!(terminal.as_str(), "failed" | "error")
                        {
                            SubagentStatus::Failed
                        } else if pending.is_empty() {
                            SubagentStatus::Completed
                        } else {
                            SubagentStatus::Unknown
                        };
                        pending.clear();
                    }
                    "function_call" | "custom_tool_call" => {
                        if let Some(call) = field(event, &["call_id", "callId"]) {
                            pending.insert(call);
                        }
                    }
                    "function_call_output" | "custom_tool_call_output" => {
                        if let Some(call) = field(event, &["call_id", "callId"]) {
                            pending.remove(&call);
                        }
                    }
                    "message"
                        if event.get("role").and_then(Value::as_str) == Some("user")
                            && status != SubagentStatus::Running =>
                    {
                        status = SubagentStatus::Unknown;
                    }
                    _ => {}
                }
            }
            line.clear();
        }
        Ok(status)
    })
}

fn activity(
    record: &AgentRecord,
    status: SubagentStatus,
    delegation: Option<String>,
) -> SubagentActivity {
    SubagentActivity {
        agent_id: Some(record.id.clone()),
        name: record.title.clone(),
        role: record.role.clone(),
        status,
        available: true,
        unavailable_reason: None,
        delegation,
    }
}

fn tool_leaf(name: &str) -> &str {
    name.rsplit(['.', ':', '/']).next().unwrap_or(name)
}

// Capture structured delegation before the generic tool preview truncates JSON.
pub(super) fn creation_activity(name: &str, payload: &Value) -> Option<SubagentActivity> {
    if tool_leaf(name) != "spawn_agent" {
        return None;
    }
    let raw = payload
        .get("arguments")
        .or_else(|| payload.get("input"))
        .or_else(|| payload.get("params"))?;
    let parsed;
    let input = if let Some(text) = raw.as_str() {
        parsed = serde_json::from_str::<Value>(text).ok()?;
        &parsed
    } else {
        raw
    };
    Some(SubagentActivity {
        agent_id: None,
        name: field(input, &["task_name", "name"]).unwrap_or_else(|| "子智能体".into()),
        role: field(input, &["agent_type"]),
        status: SubagentStatus::Unknown,
        available: false,
        unavailable_reason: Some("子智能体关联尚未确认或记录已清理".into()),
        delegation: field(input, &["message", "prompt"]),
    })
}

/// Enrich only structural tool calls. Quoted tool names and user prose never create agents.
pub fn enrich_subagent_blocks(paths: &CodexPaths, detail: &mut ThreadDetail) {
    if !detail.blocks.iter().any(|b| {
        b.tool_name
            .as_deref()
            .is_some_and(|n| tool_leaf(n) == "spawn_agent")
    }) {
        return;
    }
    let graph = read_graph(paths);
    let mut seen = HashSet::new();
    for block in &mut detail.blocks {
        if block.role != "tool"
            || block
                .tool_name
                .as_deref()
                .is_none_or(|n| tool_leaf(n) != "spawn_agent")
        {
            continue;
        }
        if !seen.insert(block.call_id.clone().unwrap_or_else(|| block.id.clone())) {
            continue;
        }
        let input: Value = block
            .input
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(Value::Null);
        let output: Value = block
            .text
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(Value::Null);
        let mut view = block.subagent.clone().unwrap_or_else(|| SubagentActivity {
            agent_id: None,
            name: field(&input, &["task_name", "name"]).unwrap_or_else(|| "子智能体".into()),
            role: field(&input, &["agent_type"]),
            status: SubagentStatus::Unknown,
            available: false,
            unavailable_reason: Some("子智能体关联尚未确认或记录已清理".into()),
            delegation: field(&input, &["message", "prompt"]),
        });
        view.agent_id = None;
        view.status = SubagentStatus::Unknown;
        view.available = false;
        view.unavailable_reason = Some("子智能体关联尚未确认或记录已清理".into());
        if let Ok(graph) = &graph {
            let explicit = field(&output, &["agent_id", "thread_id", "id"]);
            let task_path = field(&output, &["task_name", "agent_path"]);
            let matches: Vec<_> = graph
                .values()
                .filter(|record| {
                    record.parents.contains(&detail.summary.id)
                        && if let Some(id) = &explicit {
                            record.id == *id
                        } else {
                            task_path
                                .as_ref()
                                .is_some_and(|path| record.path.as_ref() == Some(path))
                        }
                })
                .collect();
            if matches.len() == 1 {
                let record = matches[0];
                let validated = if record.parents.len() == 1 {
                    validate_rollout(paths, record)
                } else {
                    Err(anyhow::anyhow!("子智能体父线程关联不明确"))
                };
                match validated {
                    Ok(path) => {
                        view = activity(record, native_status(&path).unwrap_or_default(), view.delegation);
                    }
                    Err(_) => {
                        view.unavailable_reason = Some("子智能体记录缺失或身份无法验证".into());
                    }
                }
            }
        }
        block.subagent = Some(view);
    }
    detail.subagent_updates = detail
        .blocks
        .iter()
        .filter_map(|block| {
            block.subagent.clone().map(|mut agent| {
                agent.delegation = None;
                (block.id.clone(), agent)
            })
        })
        .collect();
}

fn verified_detail(paths: &CodexPaths, request: &SubagentDetailRequest) -> Result<SubagentDetailResponse> {
    let graph = read_graph(paths)?;
    validate_relation(paths, &graph, &request.root_thread_id, &request.agent_id)?;
    let record = graph
        .get(&request.agent_id)
        .context("子智能体记录不存在")?;
    let path = validate_rollout(paths, record)?;
    let status = native_status(&path)?;
    let before = fs::metadata(&path)?;
    let summary = ThreadSummary {
        id: record.id.clone(),
        title: record.title.clone(),
        status: if status == SubagentStatus::Running {
            ThreadStatus::Running
        } else {
            ThreadStatus::Recent
        },
        updated_at: record.updated_at.clone(),
        archived_at: None,
        message_count: 0,
        latest_message: None,
        cwd: record.cwd.clone(),
        model: None,
        rollout_path: Some(path.clone()),
        active_turn_id: None,
        active_job_id: None,
        pending_elicitation: None,
        last_event_kind: None,
    };
    static CACHE: crate::read_cache::ReadCache<ThreadDetail> =
        crate::read_cache::ReadCache::new(16 * 1024 * 1024);
    let mut detail = CACHE.read(
        &path,
        |_, size| size,
        || super::rollout_events::subagent_detail_from_summary(summary.clone()),
    )?;
    detail.summary = summary;
    let after = fs::metadata(validate_rollout(paths, record)?)?;
    ensure!(
        before.len() == after.len() && before.modified()? == after.modified()?,
        "子智能体记录已变化，请重试"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        ensure!(
            before.dev() == after.dev()
                && before.ino() == after.ino()
                && before.ctime() == after.ctime()
                && before.ctime_nsec() == after.ctime_nsec(),
            "子智能体文件身份已变化，请重试"
        );
    }
    let current_graph = read_graph(paths)?;
    validate_relation(paths, &current_graph, &request.root_thread_id, &request.agent_id)?;
    let current = current_graph
        .get(&request.agent_id)
        .context("子智能体关联已变化")?;
    ensure!(
        current.parents == record.parents && validate_rollout(paths, current)? == path,
        "子智能体关联已变化，请重试"
    );
    enrich_subagent_blocks(paths, &mut detail);
    Ok(SubagentDetailResponse {
        root_thread_id: request.root_thread_id.clone(),
        parent_thread_id: record.parents.iter().next().expect("verified parent").clone(),
        agent: activity(record, status, None),
        detail,
    })
}

/// Page only after checking the full parent chain against the current machine.
pub fn read_subagent_detail(
    paths: &CodexPaths,
    request: &SubagentDetailRequest,
) -> Result<SubagentDetailResponse> {
    if let Some(cursor) = &request.before {
        ensure!(
            cursor
                .strip_prefix("b:")
                .and_then(|s| s.parse::<usize>().ok())
                .is_some(),
            "无效的历史游标"
        );
    }
    let mut response = verified_detail(paths, request)?;
    response.detail = super::window_thread_detail(
        response.detail,
        Some(request.limit.unwrap_or(120).clamp(1, 500)),
        request.before.as_deref(),
    );
    Ok(response)
}

/// Attachment identity is resolved from the verified native message, never a caller path.
pub(crate) fn read_attachment(
    paths: &CodexPaths,
    request: &crate::user_message::SessionAttachmentRequest,
) -> Result<crate::user_message::SessionAttachmentResponse> {
    let root = request
        .root_thread_id
        .as_ref()
        .context("缺少主线程上下文")?;
    let response = verified_detail(
        paths,
        &SubagentDetailRequest {
            root_thread_id: root.clone(),
            agent_id: request.session_key.clone(),
            limit: None,
            before: None,
        },
    )?;
    let message = response
        .detail
        .blocks
        .into_iter()
        .filter_map(|block| block.user_message)
        .find(|message| message.id == request.message_id)
        .context("子智能体附件已变化或不存在")?;
    message.read_attachment(&request.attachment_id)
}
