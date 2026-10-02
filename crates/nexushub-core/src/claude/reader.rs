use super::*;
use std::{
    collections::{HashSet, VecDeque},
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    sync::Mutex,
    time::SystemTime,
};

const MAX_BYTES: u64 = 512 * 1024 * 1024;
const MAX_LINE: u64 = 32 * 1024 * 1024;
const CACHE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Stamp {
    size: u64,
    modified: SystemTime,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64),
}
pub(super) fn stamp(path: &Path) -> Result<Stamp> {
    let m = fs::symlink_metadata(path)?;
    ensure!(
        m.is_file() && !m.file_type().is_symlink(),
        "Claude 会话必须是普通文件"
    );
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    Ok(Stamp {
        size: m.len(),
        modified: m.modified()?,
        #[cfg(unix)]
        identity: (m.dev(), m.ino(), m.ctime(), m.ctime_nsec()),
    })
}
fn same_file(a: &Stamp, b: &Stamp) -> bool {
    #[cfg(unix)]
    {
        a.identity.0 == b.identity.0 && a.identity.1 == b.identity.1
    }
    #[cfg(not(unix))]
    {
        a.modified == b.modified
    }
}
#[derive(Clone)]
struct Entry {
    path: PathBuf,
    stamp: Stamp,
    records: Vec<Arc<Value>>,
    offset: u64,
    anchor: String,
    issues: Vec<String>,
    parsed: Arc<Parsed>,
}
static CACHE: Mutex<VecDeque<Entry>> = Mutex::new(VecDeque::new());

fn anchor(file: &mut File, offset: u64) -> Result<String> {
    let mut bytes = vec![0; offset.min(4096) as usize];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut bytes)?;
    let mut tail = vec![0; offset.min(4096) as usize];
    file.seek(SeekFrom::Start(offset.saturating_sub(4096)))?;
    file.read_exact(&mut tail)?;
    bytes.extend(tail);
    Ok(digest(bytes))
}
pub(super) fn read(path: &Path) -> Result<Arc<Parsed>> {
    let before = stamp(path)?;
    ensure!(before.size <= MAX_BYTES, "Claude 会话超出 512 MiB 读取上限");
    let old = CACHE.lock().ok().and_then(|mut cache| {
        let index = cache.iter().position(|e| e.path == path)?;
        cache.remove(index)
    });
    if let Some(entry) = old.as_ref().filter(|e| e.stamp == before) {
        let result = entry.parsed.clone();
        if let Ok(mut cache) = CACHE.lock() {
            cache.push_back(entry.clone());
        }
        return Ok(result);
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let opened = file.metadata()?;
        ensure!(
            (opened.dev(), opened.ino()) == (before.identity.0, before.identity.1),
            "Claude 文件身份已变化"
        );
    }
    let reuse = old.filter(|e| {
        before.size > e.stamp.size
            && same_file(&before, &e.stamp)
            && anchor(&mut file, e.offset).ok().as_ref() == Some(&e.anchor)
    });
    let (mut records, mut offset, mut issues) = reuse
        .map(|e| (e.records, e.offset, e.issues))
        .unwrap_or_default();
    file.seek(SeekFrom::Start(offset))?;
    let mut input = BufReader::new(file.take(before.size - offset));
    let mut partial = false;
    loop {
        let mut line = Vec::new();
        let length = input
            .by_ref()
            .take(MAX_LINE + 1)
            .read_until(b'\n', &mut line)?;
        if length == 0 {
            break;
        }
        ensure!(
            length as u64 <= MAX_LINE && records.len() < 200_000,
            "Claude 会话记录超出读取上限"
        );
        if !line.ends_with(b"\n") {
            partial = true;
            break;
        }
        offset += length as u64;
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match serde_json::from_slice::<Value>(&line) {
            Ok(value) if value.is_object() => records.push(Arc::new(value)),
            _ => {
                if !issues.iter().any(|i| i == "会话含损坏记录，管理操作已禁用") {
                    issues.push("会话含损坏记录，管理操作已禁用".into());
                }
            }
        }
    }
    let mut all_issues = issues.clone();
    if partial {
        all_issues.push("会话尾行尚未写完，请稍后刷新".into());
    }
    let parsed = Arc::new(parse(&records, path, all_issues));
    ensure!(before == stamp(path)?, "Claude 会话正在写入，请稍后刷新");
    if before.size.saturating_mul(4) <= CACHE_BYTES {
        let mut file = File::open(path)?;
        let entry = Entry {
            path: path.into(),
            stamp: before,
            records,
            offset,
            anchor: anchor(&mut file, offset)?,
            issues,
            parsed: parsed.clone(),
        };
        if let Ok(mut cache) = CACHE.lock() {
            cache.retain(|e| e.path != path);
            while cache.len() >= 64
                || cache
                    .iter()
                    .map(|e| e.stamp.size.saturating_mul(4))
                    .sum::<u64>()
                    + entry.stamp.size.saturating_mul(4)
                    > CACHE_BYTES
            {
                if cache.pop_front().is_none() {
                    break;
                }
            }
            cache.push_back(entry);
        }
    }
    Ok(parsed)
}
fn bounded(text: &str) -> String {
    crate::security::redact_output(&text.chars().take(256 * 1024).collect::<String>())
}
fn printable(value: &Value) -> String {
    fn clean(v: &Value) -> Value {
        match v {
            Value::Object(fields) => Value::Object(
                fields
                    .iter()
                    .filter(|(k, _)| {
                        !matches!(k.as_str(), "data" | "image_url" | "base64" | "signature")
                    })
                    .map(|(k, v)| (k.clone(), clean(v)))
                    .collect(),
            ),
            Value::Array(rows) => Value::Array(rows.iter().map(clean).collect()),
            Value::String(s) if s.starts_with("data:image/") => Value::String("[图片]".into()),
            _ => v.clone(),
        }
    }
    if let Some(s) = value.as_str() {
        bounded(s)
    } else {
        bounded(&serde_json::to_string_pretty(&clean(value)).unwrap_or_default())
    }
}
fn content_text(content: &Value) -> String {
    if let Some(s) = content.as_str() {
        return bounded(s);
    }
    content
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| match field(p, "type") {
                    "text" => Some(field(p, "text").to_owned()),
                    "tool_reference" => {
                        let name = if field(p, "tool_name").is_empty() {
                            field(p, "name")
                        } else {
                            field(p, "tool_name")
                        };
                        (!name.is_empty()).then(|| format!("工具引用：{name}"))
                    }
                    "tool_search_tool_result" => p
                        .get("content")
                        .map(content_text)
                        .or_else(|| p.get("tool_references").map(content_text))
                        .filter(|text| !text.is_empty()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .map(|s| bounded(&s))
        .unwrap_or_default()
}
fn text_content_only(content: &Value) -> String {
    if let Some(s) = content.as_str() {
        return bounded(s);
    }
    content
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter(|p| field(p, "type") == "text")
                .map(|p| field(p, "text"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .map(|s| bounded(&s))
        .unwrap_or_default()
}
fn millis(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|t| t.timestamp_millis())
}
fn terminal_stop_reason(value: &str) -> bool {
    matches!(value, "end_turn" | "stop_sequence" | "max_tokens")
}
fn event(
    id: String,
    record: &Value,
    kind: &str,
    text: Option<String>,
    turn: &Option<String>,
) -> ClaudeHistoryEvent {
    ClaudeHistoryEvent {
        id,
        timestamp: record
            .get("timestamp")
            .and_then(Value::as_str)
            .map(str::to_owned),
        kind: kind.into(),
        text,
        turn_id: turn.clone(),
        role: None,
        call_id: None,
        status: None,
        detail: None,
        result: None,
        user_message: None,
    }
}
fn issue(parsed: &mut Parsed, message: &str) {
    if !parsed.issues.iter().any(|i| i == message) {
        parsed.issues.push(message.into());
    }
}
fn warning(parsed: &mut Parsed, message: &str) {
    if !parsed.warnings.iter().any(|i| i == message) {
        parsed.warnings.push(message.into());
    }
}
fn alias_field<'a>(value: &'a Value, keys: &[&str]) -> &'a str {
    keys.iter()
        .find_map(|key| {
            value
                .get(*key)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })
        .unwrap_or("")
}
fn alias_conflict(record: &Value, keys: &[&str]) -> bool {
    let mut value = None;
    for key in keys {
        let Some(candidate) = record
            .get(*key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        if let Some(prior) = value {
            if prior != candidate {
                return true;
            }
        } else {
            value = Some(candidate);
        }
    }
    false
}

fn parse(records: &[Arc<Value>], path: &Path, issues: Vec<String>) -> Parsed {
    let mut parsed = Parsed {
        issues,
        record_count: records.len() as u64,
        settled_count: records.len() as u64,
        ..Default::default()
    };
    let mut custom_title = None;
    let mut auto_title = None;
    let mut first_user = None;
    let mut turn = None;
    let mut calls = HashMap::<String, usize>::new();
    let mut pending = HashSet::<String>::new();
    let mut questions = HashMap::<String, crate::native_probe::NativeTurnEvent>::new();
    let mut permission_calls = HashMap::<String, String>::new();
    let mut seen = HashMap::<String, String>::new();
    let mut last_assistant_text = None;
    let mut last_stop_reason = String::new();
    // Main messages only. A rewind replaces the prior suffix; compaction with
    // a null parent preserves earlier history and supplies its own boundary.
    let mut main = Vec::<usize>::new();
    let mut positions = HashMap::<String, usize>::new();
    for (i, r) in records.iter().enumerate() {
        if r.get("isSidechain").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let uuid = field(r, "uuid");
        if matches!(field(r, "type"), "user" | "assistant") && !uuid.is_empty() {
            if let Some(parent) = positions.get(field(r, "parentUuid")).copied() {
                if !positions.contains_key(uuid) {
                    main.truncate(parent + 1);
                    positions.retain(|_, v| *v <= parent);
                }
            }
            if !positions.contains_key(uuid) {
                positions.insert(uuid.into(), main.len());
                main.push(i);
            }
        }
    }
    let allowed: HashSet<_> = main.iter().map(|i| field(&records[*i], "uuid")).collect();
    for (position, record) in records.iter().enumerate() {
        if record.get("isSidechain").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let kind = field(record, "type");
        let uuid = field(record, "uuid");
        if matches!(kind, "user" | "assistant") && !uuid.is_empty() && !allowed.contains(uuid) {
            continue;
        }
        if alias_conflict(record, &["sessionId", "session_id"]) {
            issue(&mut parsed, "会话身份字段冲突，管理操作已禁用");
        }
        if alias_conflict(record, &["version", "claude_code_version"]) {
            issue(&mut parsed, "会话版本字段冲突，管理操作已禁用");
        }
        let session = alias_field(record, &["sessionId", "session_id"]);
        if !session.is_empty() {
            if parsed.id.is_empty() {
                parsed.id = session.into();
            } else if parsed.id != session {
                issue(&mut parsed, "会话身份冲突，管理操作已禁用");
                continue;
            }
        }
        if !field(record, "cwd").is_empty() {
            parsed.cwd = field(record, "cwd").into();
        }
        let version = alias_field(record, &["version", "claude_code_version"]);
        if !version.is_empty() {
            parsed.version = version.into();
        }
        if !field(record, "timestamp").is_empty() {
            parsed.updated_at = Some(field(record, "timestamp").into());
        }
        let key = if uuid.is_empty() {
            format!("record-{}", position + 1)
        } else {
            uuid.into()
        };
        if !uuid.is_empty() {
            let hash = digest(record.to_string());
            if let Some(prior) = seen.insert(uuid.into(), hash.clone()) {
                if prior != hash {
                    issue(&mut parsed, "记录 ID 冲突，管理操作已禁用");
                }
                continue;
            }
        }
        match kind {
            "custom-title" => {
                custom_title = record
                    .get("customTitle")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .map(bounded)
            }
            "ai-title" => {
                auto_title = record
                    .get("aiTitle")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty())
                    .map(bounded)
            }
            "user" | "assistant" => {
                let Some(message) = record.get("message").filter(|m| m.is_object()) else {
                    issue(&mut parsed, "消息格式未知，管理操作已禁用");
                    continue;
                };
                let Some(content) = message.get("content") else {
                    issue(&mut parsed, "消息正文缺失，管理操作已禁用");
                    continue;
                };
                if !content.is_array() && !content.is_string() {
                    issue(&mut parsed, "消息正文格式未知，管理操作已禁用");
                    let mut e = event(key, record, "unknown", Some("未知消息正文".into()), &turn);
                    e.detail = Some(printable(content));
                    parsed.events.push(e);
                    continue;
                }
                let parts = content.as_array().cloned().unwrap_or_else(|| {
                    vec![serde_json::json!({"type":"text","text":content.as_str().unwrap_or("")})]
                });
                let visible_user = kind == "user"
                    && parts
                        .iter()
                        .any(|p| matches!(field(p, "type"), "text" | "image" | "document"));
                if visible_user {
                    // A new request supersedes any undelivered event from the
                    // previous turn. The persisted delivery ledger keeps history.
                    parsed.native_events.clear();
                    turn = Some(key.clone());
                    parsed.turn_open = true;
                    parsed.last_turn_at = millis(field(record, "timestamp"));
                    pending.clear();
                    questions.clear();
                    permission_calls.clear();
                    calls.clear();
                    last_assistant_text = None;
                    last_stop_reason.clear();
                    if uuid.is_empty() {
                        issue(&mut parsed, "原生消息身份缺失，通知和管理操作已禁用");
                    }
                    let user_parts = Value::Array(
                        parts
                            .iter()
                            .filter(|p| field(p, "type") != "tool_result")
                            .cloned()
                            .collect(),
                    );
                    let user = crate::user_message::parse_user_message(&key, &user_parts);
                    if first_user.is_none() && !user.text.trim().is_empty() {
                        first_user = Some(user.text.chars().take(120).collect::<String>());
                    }
                    let mut e = event(
                        key.clone(),
                        record,
                        "user_message",
                        Some(bounded(&user.text)),
                        &turn,
                    );
                    e.user_message = Some(user);
                    parsed.events.push(e);
                }
                if kind == "user" {
                    if let Some(plan) = record
                        .get("planContent")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                    {
                        parsed.events.push(event(
                            format!("{key}:plan"),
                            record,
                            "plan",
                            Some(bounded(plan)),
                            &turn,
                        ));
                    }
                }
                for (index, part) in parts.iter().enumerate() {
                    let mut e = event(format!("{key}:{index}"), record, "", None, &turn);
                    match field(part, "type") {
                        "text" if kind == "assistant" => {
                            e.kind = "assistant_message".into();
                            e.text = Some(bounded(field(part, "text")));
                        }
                        "thinking" if kind == "assistant" => {
                            e.kind = "thinking".into();
                            e.text = Some(bounded(field(part, "thinking")));
                        }
                        "tool_reference" => {
                            let name = if field(part, "tool_name").is_empty() {
                                field(part, "name")
                            } else {
                                field(part, "tool_name")
                            };
                            e.kind = "tool_reference".into();
                            e.text = Some(if name.is_empty() {
                                "工具引用".into()
                            } else {
                                name.into()
                            });
                            e.role = (!name.is_empty()).then(|| name.into());
                            e.detail = Some(printable(part));
                            if name.is_empty() {
                                warning(&mut parsed, "工具引用缺少工具名称，已保留为警告");
                            }
                        }
                        "tool_search_tool_result" => {
                            e.kind = "tool_reference".into();
                            e.text = Some("工具引用结果".into());
                            e.detail = part.get("content").map(printable);
                        }
                        "redacted_thinking" => {
                            continue;
                        }
                        "tool_use" if kind == "assistant" => {
                            let call = field(part, "id");
                            let name = field(part, "name");
                            e.kind = "tool_call".into();
                            e.text = Some(name.into());
                            e.role = Some(name.into());
                            e.call_id = Some(call.into());
                            e.status = Some("in_progress".into());
                            e.detail = part.get("input").map(printable);
                            if call.is_empty() {
                                issue(&mut parsed, "工具调用身份缺失，管理操作已禁用");
                            }
                            pending.insert(call.into());
                            calls.insert(call.into(), parsed.events.len());
                            if name == "AskUserQuestion" && !call.is_empty() {
                                if let (Some(turn), Some(ts), Some(qs)) = (
                                    &turn,
                                    millis(field(record, "timestamp")),
                                    part.pointer("/input/questions").and_then(Value::as_array),
                                ) {
                                    let body = qs
                                        .iter()
                                        .filter_map(|q| {
                                            let question = field(q, "question");
                                            if question.is_empty() {
                                                return None;
                                            }
                                            let options = q
                                                .get("options")
                                                .and_then(Value::as_array)
                                                .map(|opts| {
                                                    opts.iter()
                                                        .filter_map(|o| {
                                                            o.as_str().or_else(|| {
                                                                o.get("label")
                                                                    .and_then(Value::as_str)
                                                            })
                                                        })
                                                        .collect::<Vec<_>>()
                                                        .join(" / ")
                                                })
                                                .unwrap_or_default();
                                            Some(format!("{question}\n{options}"))
                                        })
                                        .collect::<Vec<_>>()
                                        .join("\n\n");
                                    if !body.trim().is_empty() {
                                        questions.insert(
                                            call.into(),
                                            crate::native_probe::NativeTurnEvent {
                                                position: position as u64 + 1,
                                                turn_id: format!("{turn}:{call}"),
                                                kind: "reply_needed".into(),
                                                body: bounded(&body),
                                                timestamp_ms: ts,
                                            },
                                        );
                                    }
                                }
                            }
                            if name == "ExitPlanMode" {
                                if let Some(plan) = part
                                    .pointer("/input/plan")
                                    .and_then(Value::as_str)
                                    .filter(|p| !p.trim().is_empty())
                                {
                                    e.kind = "plan".into();
                                    e.text = Some(bounded(plan));
                                    e.detail = None;
                                }
                            }
                        }
                        "tool_result" => {
                            let call = field(part, "tool_use_id");
                            pending.remove(call);
                            questions.remove(call);
                            let result = part
                                .get("content")
                                .map(|c| {
                                    if c.is_array() {
                                        content_text(c)
                                    } else {
                                        printable(c)
                                    }
                                })
                                .unwrap_or_default();
                            let status =
                                if part.get("is_error").and_then(Value::as_bool) == Some(true) {
                                    "failed"
                                } else {
                                    "completed"
                                };
                            if let Some(index) = calls.get(call) {
                                let prior = &mut parsed.events[*index];
                                prior.status = Some(status.into());
                                prior.result = Some(result);
                                continue;
                            }
                            e.kind = "tool_result".into();
                            e.text = Some("工具结果".into());
                            e.call_id = Some(call.into());
                            e.status = Some(status.into());
                            e.result = Some(result);
                        }
                        "text" | "image" | "document" if kind == "user" => continue,
                        _ => {
                            e.kind = "unknown".into();
                            e.text = Some("未知内容块".into());
                            e.detail = Some(printable(part));
                            warning(&mut parsed, "存在未知内容块，已保留为警告");
                        }
                    }
                    parsed.events.push(e);
                }
                if kind == "assistant" {
                    let text = text_content_only(content);
                    if !text.trim().is_empty() {
                        last_assistant_text = Some(text);
                    }
                    last_stop_reason = field(message, "stop_reason").into();
                    if !terminal_stop_reason(&last_stop_reason) || !pending.is_empty() {
                        parsed.turn_open = turn.is_some();
                        parsed.native_events.clear();
                    }
                }
                let assistant_terminal = terminal_stop_reason(field(message, "stop_reason"));
                if kind == "assistant"
                    && assistant_terminal
                    && pending.is_empty()
                    && questions.is_empty()
                {
                    finish(
                        &mut parsed,
                        &turn,
                        position,
                        record,
                        "completion",
                        last_assistant_text.clone().unwrap_or_default(),
                    );
                }
            }
            "result" => {
                let subtype = field(record, "subtype");
                let is_failure = record.get("is_error").and_then(Value::as_bool) == Some(true)
                    && matches!(
                        subtype,
                        "error_during_execution"
                            | "error_max_turns"
                            | "error_max_budget_usd"
                            | "error_max_structured_output_retries"
                    );
                let stop_reason = record.get("stop_reason").and_then(Value::as_str);
                let explicit_terminal = stop_reason.is_some_and(terminal_stop_reason);
                let inferred_terminal = stop_reason.is_none()
                    && (last_stop_reason.is_empty() || terminal_stop_reason(&last_stop_reason))
                    && record.get("is_error").and_then(Value::as_bool) == Some(false);
                if subtype == "success"
                    && stop_reason.is_some_and(|reason| !terminal_stop_reason(reason))
                {
                    parsed
                        .native_events
                        .retain(|event| event.kind != "completion");
                    parsed.turn_open = turn.is_some();
                }
                if subtype == "success"
                    && record.get("is_error").and_then(Value::as_bool) != Some(true)
                    && pending.is_empty()
                    && questions.is_empty()
                    && (explicit_terminal || inferred_terminal)
                {
                    let result = bounded(field(record, "result"));
                    finish(
                        &mut parsed,
                        &turn,
                        position,
                        record,
                        "completion",
                        if result.trim().is_empty() {
                            last_assistant_text.clone().unwrap_or_default()
                        } else {
                            result
                        },
                    );
                } else if is_failure {
                    questions.clear();
                    pending.clear();
                    finish(
                        &mut parsed,
                        &turn,
                        position,
                        record,
                        "failure",
                        record
                            .get("errors")
                            .map(printable)
                            .unwrap_or_else(|| subtype.into()),
                    );
                }
            }
            "system" => {
                let subtype = field(record, "subtype");
                if matches!(subtype, "interrupted" | "cancelled" | "canceled") {
                    parsed.turn_open = false;
                    parsed.native_events.clear();
                    turn = None;
                    last_stop_reason.clear();
                    last_assistant_text = None;
                    questions.clear();
                    for call in &pending {
                        if let Some(index) = calls.get(call) {
                            parsed.events[*index].status = Some("cancelled".into());
                        }
                    }
                    pending.clear();
                }
                if subtype == "init" {
                    // Identity/cwd/version were extracted above. Startup
                    // configuration is metadata, not a conversation activity.
                    continue;
                }
                if subtype == "compact_boundary" {
                    parsed.events.push(event(
                        key,
                        record,
                        "compaction",
                        Some(content_text(record.get("content").unwrap_or(&Value::Null))),
                        &turn,
                    ));
                } else {
                    let known = matches!(
                        subtype,
                        "init"
                            | "turn_duration"
                            | "stop_hook_summary"
                            | "interrupted"
                            | "cancelled"
                            | "canceled"
                            | "api_error"
                    );
                    let mut e = event(
                        key,
                        record,
                        if known { "system" } else { "unknown" },
                        Some(format!("系统活动：{subtype}")),
                        &turn,
                    );
                    e.detail = Some(printable(record));
                    parsed.events.push(e);
                    if !known {
                        warning(&mut parsed, "存在新格式系统活动，管理操作已禁用");
                    }
                }
            }
            "permission_request" => {
                let call = field(record, "tool_use_id");
                let request = field(record, "request_id");
                let mut e = event(
                    key,
                    record,
                    "permission_request",
                    Some("权限请求".into()),
                    &turn,
                );
                e.detail = Some(printable(record));
                parsed.events.push(e);
                // Only a pending primary call with explicit transcript identity
                // proves this is our permission request, not a remote subagent's.
                if !request.is_empty()
                    && pending.contains(call)
                    && field(record, "agent_id").is_empty()
                    && alias_field(record, &["sessionId", "session_id"]) == parsed.id
                {
                    if let (Some(turn), Some(ts)) = (&turn, millis(field(record, "timestamp"))) {
                        let body = format!(
                            "{}\n{}",
                            field(record, "description"),
                            field(record, "tool_name")
                        );
                        if !body.trim().is_empty() {
                            permission_calls.insert(request.into(), call.into());
                            questions.entry(call.into()).or_insert(
                                crate::native_probe::NativeTurnEvent {
                                    position: position as u64 + 1,
                                    turn_id: format!("{turn}:permission:{request}"),
                                    kind: "reply_needed".into(),
                                    body: bounded(&body),
                                    timestamp_ms: ts,
                                },
                            );
                        }
                    }
                }
            }
            "permission_response" => {
                if let Some(call) = permission_calls.remove(field(record, "request_id")) {
                    questions.remove(&call);
                }
                let mut e = event(
                    key,
                    record,
                    "permission_response",
                    Some("权限回复".into()),
                    &turn,
                );
                e.detail = Some(printable(record));
                parsed.events.push(e);
            }
            "summary" => parsed.events.push(event(
                key,
                record,
                "compaction",
                Some(bounded(field(record, "summary"))),
                &turn,
            )),
            "tool_reference" | "tool_search_tool_result" => {
                let name = if field(record, "tool_name").is_empty() {
                    field(record, "name")
                } else {
                    field(record, "tool_name")
                };
                let mut e = event(
                    key,
                    record,
                    "tool_reference",
                    Some(if name.is_empty() {
                        if kind == "tool_search_tool_result" {
                            "工具引用结果".into()
                        } else {
                            "工具引用".into()
                        }
                    } else {
                        name.into()
                    }),
                    &turn,
                );
                e.role = (!name.is_empty()).then(|| name.into());
                e.detail = Some(printable(record));
                parsed.events.push(e);
                let nested_reference = ["content", "tool_references"].iter().any(|key| {
                    record
                        .get(*key)
                        .and_then(Value::as_array)
                        .is_some_and(|parts| {
                            parts.iter().any(|part| {
                                field(part, "type") == "tool_reference"
                                    && !alias_field(part, &["tool_name", "name"]).is_empty()
                            })
                        })
                });
                if name.is_empty() && !nested_reference {
                    warning(&mut parsed, "工具引用缺少工具名称，已保留为警告");
                }
            }
            "file-history-snapshot"
            | "file-history-delta"
            | "queue-operation"
            | "attachment"
            | "atis-latch"
            | "cost-state" => {
                if kind == "attachment"
                    && record.pointer("/attachment/type").and_then(Value::as_str)
                        == Some("plan_file_reference")
                {
                    if let Some(plan) = record
                        .pointer("/attachment/planContent")
                        .and_then(Value::as_str)
                        .filter(|s| !s.trim().is_empty())
                    {
                        parsed
                            .events
                            .push(event(key, record, "plan", Some(bounded(plan)), &turn));
                        continue;
                    }
                }
                let mut e = event(
                    key,
                    record,
                    kind,
                    Some(
                        match kind {
                            "file-history-snapshot" | "file-history-delta" => "文件历史快照",
                            "queue-operation" => "队列活动",
                            "atis-latch" => "会话设置",
                            "cost-state" => "用量记录",
                            _ => "附件",
                        }
                        .into(),
                    ),
                    &turn,
                );
                e.detail = Some(printable(record));
                parsed.events.push(e);
            }
            "progress" | "last-prompt" | "agent-name" | "tag" | "pr-link"
            | "saved_hook_context" | "mode" | "permission-mode" => {}
            _ => {
                let mut e = event(
                    key,
                    record,
                    "unknown",
                    Some(format!("未识别的活动：{kind}")),
                    &turn,
                );
                e.detail = Some(printable(record));
                parsed.events.push(e);
                warning(&mut parsed, "存在新格式活动，管理操作已禁用");
            }
        }
    }
    if parsed.id.is_empty() {
        parsed.id = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into();
        issue(&mut parsed, "未确认原生会话 ID，管理操作已禁用");
    }
    if parsed.cwd.is_empty() {
        issue(&mut parsed, "未确认会话工作目录，管理操作已禁用");
    }
    parsed.title = custom_title
        .or(auto_title)
        .or(first_user)
        .unwrap_or_else(|| parsed.id.clone());
    if parsed.version.is_empty() {
        parsed.version = "unknown".into();
        issue(&mut parsed, "未确认 Claude 会话版本，管理操作已禁用");
    }
    if !parsed.version.starts_with("2.") {
        issue(&mut parsed, "此 Claude 版本尚未验证，管理操作已禁用");
    }
    for q in questions.into_values() {
        parsed.native_events.push(q);
    }
    parsed.native_events.sort_by_key(|e| e.position);
    parsed
}
fn finish(
    parsed: &mut Parsed,
    turn: &Option<String>,
    position: usize,
    record: &Value,
    kind: &str,
    body: String,
) {
    parsed.turn_open = false;
    if body.trim().is_empty() {
        return;
    }
    let (Some(turn), Some(ts)) = (turn, millis(field(record, "timestamp"))) else {
        return;
    };
    if parsed
        .native_events
        .iter()
        .any(|e| e.turn_id == *turn && e.kind == kind)
    {
        return;
    }
    parsed.native_events.retain(|e| e.turn_id != *turn);
    parsed
        .native_events
        .push(crate::native_probe::NativeTurnEvent {
            position: position as u64 + 1,
            turn_id: turn.clone(),
            kind: kind.into(),
            body,
            timestamp_ms: ts,
        });
}
