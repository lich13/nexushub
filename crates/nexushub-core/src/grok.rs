use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    env,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone)]
pub struct GrokPaths {
    pub home: PathBuf,
}

impl GrokPaths {
    pub fn default_for_user() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            home: env::var_os("GROK_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".grok")),
        }
    }
    pub fn sessions(&self) -> PathBuf {
        self.home.join("sessions")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GrokSessionSummary {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub path: PathBuf,
    pub updated_at: Option<String>,
    pub message_count: usize,
    pub last_message: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GrokSessionDetail {
    pub summary: GrokSessionSummary,
    pub events: Vec<GrokHistoryEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GrokHistoryEvent {
    pub timestamp: Option<String>,
    pub kind: String,
    pub text: Option<String>,
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

pub fn list_grok_sessions(
    paths: &GrokPaths,
    limit: usize,
    query: Option<&str>,
) -> Result<Vec<GrokSessionSummary>> {
    let root = paths.sessions();
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let needle = query
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_ascii_lowercase);
    let active = read_active_sessions(paths).ok();
    let mut result = Vec::new();
    for workspace in fs::read_dir(&root).with_context(|| format!("read {}", root.display()))? {
        let workspace = workspace?;
        if !workspace.file_type()?.is_dir() {
            continue;
        }
        for session in fs::read_dir(workspace.path())? {
            let session = session?;
            if !session.file_type()?.is_dir() {
                continue;
            }
            if let Ok(Some(mut summary)) = read_summary(&session.path()) {
                summary.status = status_from_snapshot(active.as_ref(), &summary.id).to_string();
                if needle.as_ref().is_some_and(|needle| {
                    !summary.title.to_ascii_lowercase().contains(needle)
                        && !summary.id.to_ascii_lowercase().contains(needle)
                        && !summary.cwd.to_ascii_lowercase().contains(needle)
                }) {
                    continue;
                }
                result.push(summary);
            }
        }
    }
    result.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.id.cmp(&b.id)));
    result.truncate(limit.clamp(1, 200));
    Ok(result)
}

pub(crate) fn notification_snapshots(paths: &GrokPaths) -> Result<crate::native_probe::NativeScan> {
    use crate::native_probe::{
        file_identity, grok_events, read_jsonl, NativeProvider, NativeScan, NativeStreamSnapshot,
    };
    let mut scan = NativeScan::default();
    if !paths.sessions().exists() {
        return Ok(scan);
    }
    let root = paths.sessions().canonicalize()?;
    let mut seen = std::collections::HashSet::new();
    for workspace in fs::read_dir(&root)? {
        let workspace = workspace?;
        if !workspace.file_type()?.is_dir() {
            continue;
        }
        for entry in fs::read_dir(workspace.path())? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !seen.insert(id.clone()) {
                continue;
            }
            let read = (|| -> Result<NativeStreamSnapshot> {
                let summary = resolve_session(paths, &id)?;
                let event_path = summary.path.join("events.jsonl");
                let updates_path = summary.path.join("updates.jsonl");
                let identity = format!(
                    "{}:{}",
                    file_identity(&event_path)?,
                    file_identity(&updates_path)?
                );
                let events = read_jsonl(&event_path)?;
                let updates = read_jsonl(&updates_path)?;
                let (turns, settled_count) = grok_events(&id, &events, &updates);
                Ok(NativeStreamSnapshot {
                    provider: NativeProvider::Grok,
                    session_key: id.clone(),
                    id,
                    title: summary.title,
                    identity,
                    record_count: events.len() as u64,
                    settled_count,
                    events: turns,
                })
            })();
            match read {
                Ok(stream) => scan.streams.push(stream),
                Err(_) => scan.errors += 1,
            }
        }
    }
    Ok(scan)
}

pub(crate) fn grok_session_summary(paths: &GrokPaths, id: &str) -> Result<GrokSessionSummary> {
    resolve_session(paths, id)
}

pub fn grok_session_detail(
    paths: &GrokPaths,
    id: &str,
    cwd: Option<&str>,
) -> Result<GrokSessionDetail> {
    let summary = resolve_session(paths, id)?;
    ensure!(
        cwd.is_none_or(|cwd| summary.cwd == cwd),
        "Grok workspace identity changed"
    );
    let history = summary.path.join("updates.jsonl");
    static CACHE: crate::read_cache::ReadCache<Vec<GrokHistoryEvent>> =
        crate::read_cache::ReadCache::new(8 * 1024 * 1024);
    let events = CACHE.read(
        &history,
        |events, _| {
            events
                .iter()
                .map(|event| {
                    256 + event.text.as_ref().map_or(0, String::len) as u64
                        + event.detail.as_ref().map_or(0, String::len) as u64
                })
                .sum()
        },
        || read_history(&history),
    )?;
    Ok(GrokSessionDetail { summary, events })
}

#[cfg(test)]
thread_local! { static HISTORY_READS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

fn read_history(history: &Path) -> Result<Vec<GrokHistoryEvent>> {
    #[cfg(test)]
    HISTORY_READS.with(|reads| reads.set(reads.get() + 1));
    let mut events = Vec::new();
    let mut tool_indices: HashMap<String, usize> = HashMap::new();
    if history.is_file() {
        let mut file = fs::File::open(history)?;
        let offset = file.metadata()?.len().saturating_sub(8 * 1024 * 1024);
        file.seek(SeekFrom::Start(offset))?;
        let mut reader = BufReader::new(file);
        if offset > 0 {
            let mut partial = String::new();
            reader.read_line(&mut partial)?;
        }
        for line in reader.lines() {
            let line = line?;
            let Ok(value) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let params = value.get("params").unwrap_or(&value);
            let update = params.get("update").unwrap_or(params);
            let kind = update
                .get("sessionUpdate")
                .and_then(Value::as_str)
                .unwrap_or("event")
                .to_string();
            if !matches!(
                kind.as_str(),
                "user_message_chunk"
                    | "agent_message_chunk"
                    | "tool_call"
                    | "tool_call_update"
                    | "plan"
            ) {
                continue;
            }
            let text = update
                .get("content")
                .and_then(|v| v.get("text"))
                .and_then(Value::as_str)
                .or_else(|| update.get("text").and_then(Value::as_str))
                .or_else(|| update.get("title").and_then(Value::as_str))
                .map(ToString::to_string);
            let call_id = update
                .get("toolCallId")
                .and_then(Value::as_str)
                .map(str::to_string);
            let status = update
                .get("status")
                .and_then(Value::as_str)
                .map(str::to_string);
            let detail = update
                .get("content")
                .and_then(Value::as_array)
                .map(|content| {
                    content
                        .iter()
                        .filter_map(|item| item.pointer("/content/text").and_then(Value::as_str))
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .filter(|text| !text.is_empty());
            if kind.starts_with("tool_") {
                if let Some(index) = call_id.as_ref().and_then(|id| tool_indices.get(id)) {
                    let previous: &mut GrokHistoryEvent = &mut events[*index];
                    if text.is_some() {
                        previous.text = text;
                    }
                    if status.is_some() {
                        previous.status = status;
                    }
                    if detail.is_some() {
                        previous.detail = detail;
                    }
                    continue;
                }
                if let Some(id) = &call_id {
                    tool_indices.insert(id.clone(), events.len());
                }
            }
            if kind.ends_with("message_chunk") {
                if let Some(previous) = events
                    .last_mut()
                    .filter(|event: &&mut GrokHistoryEvent| event.kind == kind)
                {
                    previous
                        .text
                        .get_or_insert_with(String::new)
                        .push_str(text.as_deref().unwrap_or_default());
                    continue;
                }
            }
            events.push(GrokHistoryEvent {
                timestamp: value.get("timestamp").map(|value| {
                    value
                        .as_str()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| value.to_string())
                }),
                kind,
                text,
                method: value
                    .get("method")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                call_id,
                status,
                detail,
            });
        }
    }
    Ok(events)
}

fn read_summary(path: &Path) -> Result<Option<GrokSessionSummary>> {
    let file = path.join("summary.json");
    if !file.is_file() {
        return Ok(None);
    }
    ensure!(
        !fs::symlink_metadata(&file)?.file_type().is_symlink(),
        "Grok summary must not be a symlink"
    );
    ensure!(
        fs::metadata(&file)?.len() <= 1024 * 1024,
        "Grok summary exceeds size limit"
    );
    static CACHE: crate::read_cache::ReadCache<Value> =
        crate::read_cache::ReadCache::new(2 * 1024 * 1024);
    let value = CACHE.read(
        &file,
        |_, size| size.saturating_mul(4),
        || Ok(serde_json::from_str::<Value>(&fs::read_to_string(&file)?)?),
    )?;
    let id = path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_string();
    let info = value.get("info").unwrap_or(&Value::Null);
    ensure!(
        info.get("id").and_then(Value::as_str) == Some(&id),
        "Grok session identity mismatch"
    );
    let cwd = info
        .get("cwd")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    ensure!(
        Path::new(&cwd).is_absolute(),
        "Grok session has no absolute workspace"
    );
    let title = value
        .get("generated_title")
        .and_then(Value::as_str)
        .or_else(|| value.get("last_turn_summary").and_then(Value::as_str))
        .unwrap_or("未命名会话")
        .trim()
        .to_string();
    let updated_at = value
        .get("updated_at")
        .and_then(Value::as_str)
        .or_else(|| value.get("last_active_at").and_then(Value::as_str))
        .map(ToString::to_string);
    let message_count = value
        .get("num_messages")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    Ok(Some(GrokSessionSummary {
        id,
        title,
        cwd,
        path: path.to_path_buf(),
        updated_at,
        message_count,
        last_message: value
            .get("last_turn_summary")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        status: "recent".to_string(),
    }))
}

fn resolve_session(paths: &GrokPaths, id: &str) -> Result<GrokSessionSummary> {
    uuid::Uuid::parse_str(id).context("invalid Grok session ID")?;
    let root = paths
        .sessions()
        .canonicalize()
        .context("Grok sessions unavailable")?;
    let mut found = None;
    for workspace in fs::read_dir(&root)? {
        let workspace = workspace?;
        if !workspace.file_type()?.is_dir() {
            continue;
        }
        let path = workspace.path().join(id);
        if !path.try_exists()? {
            continue;
        }
        ensure!(
            !fs::symlink_metadata(&path)?.file_type().is_symlink(),
            "Grok session must not be a symlink"
        );
        let canonical = path.canonicalize()?;
        ensure!(
            canonical.parent().and_then(Path::parent) == Some(root.as_path()),
            "Grok session escaped sessions root"
        );
        if let Some(mut summary) = read_summary(&canonical)? {
            ensure!(found.is_none(), "ambiguous Grok session ID");
            summary.status = session_status(paths, id).to_string();
            found = Some(summary);
        }
    }
    found.context("Grok session not found")
}

fn read_active_sessions(paths: &GrokPaths) -> Result<Value> {
    let path = paths.home.join("active_sessions.json");
    ensure!(
        fs::metadata(&path)?.len() <= 1024 * 1024,
        "Grok active state exceeds size limit"
    );
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn session_status(paths: &GrokPaths, id: &str) -> &'static str {
    status_from_snapshot(read_active_sessions(paths).ok().as_ref(), id)
}

fn status_from_snapshot(value: Option<&Value>, id: &str) -> &'static str {
    let Some(value) = value else {
        return "unknown";
    };
    match value {
        Value::Array(entries) => {
            if entries.iter().any(|entry| {
                entry
                    .get("sessionId")
                    .or_else(|| entry.get("session_id"))
                    .and_then(Value::as_str)
                    == Some(id)
            }) {
                "running"
            } else {
                "recent"
            }
        }
        Value::Object(entries) => {
            if entries.contains_key(id)
                || entries.values().any(|entry| {
                    entry
                        .get("sessionId")
                        .or_else(|| entry.get("session_id"))
                        .and_then(Value::as_str)
                        == Some(id)
                })
            {
                "running"
            } else if entries.is_empty() || entries.values().all(Value::is_object) {
                "recent"
            } else {
                "unknown"
            }
        }
        _ => "unknown",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokDeletePreview {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub fingerprint: String,
    pub file_count: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokDeleteRequest {
    pub id: String,
    pub confirmed: bool,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrokDeleteResult {
    pub id: String,
    pub deleted: bool,
    pub bytes: u64,
}

fn directory_fingerprint(path: &Path) -> Result<(String, usize, u64)> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(path).follow_links(false) {
        let entry = entry?;
        ensure!(
            entry.depth() <= 16,
            "Grok session exceeds directory depth limit"
        );
        ensure!(
            !entry.file_type().is_symlink(),
            "Grok session contains a symlink"
        );
        ensure!(
            entry.file_type().is_file() || entry.file_type().is_dir(),
            "Grok session contains a special file"
        );
        if entry.file_type().is_file() {
            files.push(entry.into_path());
        }
        ensure!(files.len() <= 10_000, "Grok session exceeds file limit");
    }
    files.sort();
    let mut hash = Sha256::new();
    let mut bytes = 0;
    for file in &files {
        let relative = file.strip_prefix(path)?;
        hash.update(relative.as_os_str().as_encoded_bytes());
        hash.update([0]);
        let size = fs::metadata(file)?.len();
        bytes += size;
        ensure!(
            bytes <= 512 * 1024 * 1024,
            "Grok session exceeds deletion audit limit"
        );
        hash.update(size.to_le_bytes());
        let mut source = fs::File::open(file)?;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = source.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
    }
    Ok((hex::encode(hash.finalize()), files.len(), bytes))
}

pub fn preview_grok_delete(paths: &GrokPaths, id: &str) -> Result<GrokDeletePreview> {
    let session = resolve_session(paths, id)?;
    ensure!(
        session.status == "recent",
        "Grok task is running or its activity cannot be confirmed"
    );
    ensure_grok_storage_writable(
        session
            .path
            .parent()
            .context("invalid Grok session parent")?,
        "delete the selected session directory",
    )?;
    let (fingerprint, file_count, bytes) = directory_fingerprint(&session.path)?;
    Ok(GrokDeletePreview {
        id: session.id,
        title: session.title,
        path: session.path,
        fingerprint,
        file_count,
        bytes,
    })
}

pub fn execute_grok_delete(
    paths: &GrokPaths,
    request: GrokDeleteRequest,
) -> Result<GrokDeleteResult> {
    ensure!(request.confirmed, "Grok deletion requires confirmation");
    let preview = preview_grok_delete(paths, &request.id)?;
    ensure!(
        preview.fingerprint == request.fingerprint,
        "Grok task files changed; preview again"
    );
    let parent = preview
        .path
        .parent()
        .context("invalid Grok session parent")?;
    let retired = parent.join(format!(".nexushub-delete-{}", uuid::Uuid::new_v4()));
    // Detach the selected directory so newly created session files cannot be deleted.
    fs::rename(&preview.path, &retired).with_context(|| {
        grok_storage_write_error(parent, "move the selected session into quarantine")
    })?;
    let check = (|| -> Result<()> {
        ensure!(
            session_status(paths, &request.id) == "recent",
            "Grok task became active"
        );
        ensure!(
            directory_fingerprint(&retired)?.0 == preview.fingerprint,
            "Grok task files changed during deletion"
        );
        Ok(())
    })();
    if let Err(error) = check {
        let restored = if !preview.path.exists() && retired.exists() {
            fs::rename(&retired, &preview.path).is_ok()
        } else {
            preview.path.exists()
        };
        let location = if restored { &preview.path } else { &retired };
        return Err(error.context(format!(
            "Grok deletion cancelled; recoverable files remain at {}",
            location.display()
        )));
    }
    if let Err(error) = fs::remove_dir_all(&retired) {
        let restored = if !preview.path.exists() && retired.exists() {
            fs::rename(&retired, &preview.path).is_ok()
        } else {
            preview.path.exists()
        };
        let location = if restored { &preview.path } else { &retired };
        return Err(error).with_context(|| {
            format!(
                "{}; Grok deletion did not complete and recoverable files remain at {}",
                grok_storage_write_error(parent, "remove the quarantined session"),
                location.display()
            )
        });
    }
    Ok(GrokDeleteResult {
        id: request.id,
        deleted: true,
        bytes: preview.bytes,
    })
}

pub async fn rename_grok_session(
    paths: &GrokPaths,
    id: &str,
    title: &str,
) -> Result<GrokSessionSummary> {
    let session = resolve_session(paths, id)?;
    let title = title.trim();
    ensure!(
        !title.is_empty() && title.chars().count() <= 200 && !title.chars().any(char::is_control),
        "Grok title must contain 1-200 characters without control characters"
    );
    ensure_grok_storage_writable(&session.path, "persist the native session title")?;
    let executable = env::var_os("PATH")
        .into_iter()
        .flat_map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .map(|path| path.join("grok"))
        .chain(
            dirs::home_dir()
                .into_iter()
                .flat_map(|home| [home.join(".local/bin/grok"), home.join(".grok/bin/grok")]),
        )
        .find(|path| path.is_file())
        .context("Grok executable unavailable")?;
    rename_native(&executable, paths, &session, title, Duration::from_secs(10)).await?;
    let updated = resolve_session(paths, id)?;
    ensure!(
        updated.title == title,
        "Grok rename returned without persisting the requested title"
    );
    Ok(updated)
}

fn ensure_grok_storage_writable(directory: &Path, operation: &str) -> Result<()> {
    let marker = directory.join(format!(".nexushub-write-check-{}", uuid::Uuid::new_v4()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker)
        .with_context(|| grok_storage_write_error(directory, operation))?;
    drop(file);
    fs::remove_file(&marker).with_context(|| {
        format!(
            "{}; temporary marker remains at {}",
            grok_storage_write_error(directory, operation),
            marker.display()
        )
    })?;
    Ok(())
}

fn grok_storage_write_error(path: &Path, operation: &str) -> String {
    format!(
        "cannot {operation} at {}. Grok session storage is not writable from NexusHub; if systemd reports a read-only filesystem or permission denial, add this exact session root to ReadWritePaths, run systemctl daemon-reload, and restart nexushub-webd",
        path.display()
    )
}

async fn rename_native(
    executable: &Path,
    paths: &GrokPaths,
    session: &GrokSessionSummary,
    title: &str,
    timeout: Duration,
) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
    let mut child = tokio::process::Command::new(executable)
        .args(["agent", "stdio"])
        .env("GROK_HOME", &paths.home)
        .current_dir(&session.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("start Grok agent stdio")?;
    let mut input = child.stdin.take().context("Grok stdin unavailable")?;
    let output = child.stdout.take().context("Grok stdout unavailable")?;
    let mut output = tokio::io::BufReader::new(output.take(1024 * 1024));
    let exchange = async {
        for (id, method, params) in [
            (
                1,
                "initialize",
                serde_json::json!({"protocolVersion":1,"clientCapabilities":{},"clientInfo":{"name":"nexushub","version":env!("CARGO_PKG_VERSION")}}),
            ),
            (
                2,
                "_x.ai/session/rename",
                serde_json::json!({"sessionId":session.id,"cwd":session.cwd,"title":title}),
            ),
        ] {
            let mut request = serde_json::to_vec(
                &serde_json::json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
            )?;
            request.push(b'\n');
            input.write_all(&request).await?;
            input.flush().await?;
            loop {
                let mut line = String::new();
                ensure!(
                    output.read_line(&mut line).await? > 0,
                    "Grok protocol ended before response"
                );
                let value: Value = serde_json::from_str(&line).context("invalid Grok response")?;
                if value.get("id").and_then(Value::as_u64) != Some(id) {
                    continue;
                }
                if value.get("error").is_some() {
                    bail!("Grok rejected {method}; no local fallback was applied");
                }
                ensure!(value.get("result").is_some(), "Grok response has no result");
                break;
            }
        }
        Ok::<_, anyhow::Error>(())
    };
    let result = tokio::time::timeout(timeout, exchange)
        .await
        .context("Grok rename timed out; result unknown, refresh before retrying");
    drop(input);
    let _ = child.kill().await;
    let _ = child.wait().await;
    result?
}
