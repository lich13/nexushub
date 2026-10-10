//! Native Claude transcripts. Reading never starts the CLI or changes its configuration.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

mod activity;
mod mutations;
mod reader;
#[cfg(test)]
mod tests;
pub use mutations::{execute_claude_delete, preview_claude_delete, rename_claude_session};

#[derive(Debug, Clone)]
pub struct ClaudePaths {
    pub config: PathBuf,
    pub projects: PathBuf,
}
impl ClaudePaths {
    pub fn default_for_user() -> Self {
        let config = env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".claude"));
        Self {
            projects: config.join("projects"),
            config,
        }
    }
    pub fn available(&self) -> bool {
        self.projects.is_dir()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSessionSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_size: Option<crate::session_storage::SessionStorageSize>,
    pub id: String,
    pub session_key: String,
    pub title: String,
    pub cwd: String,
    pub path: PathBuf,
    pub updated_at: Option<String>,
    pub message_count: usize,
    pub last_message: Option<String>,
    pub status: String,
    pub format_version: String,
    pub can_rename: bool,
    pub rename_block_reason: Option<String>,
    pub can_delete: bool,
    pub delete_block_reason: Option<String>,
    pub read_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_warning: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeHistoryEvent {
    pub id: String,
    pub timestamp: Option<String>,
    pub kind: String,
    pub role: Option<String>,
    pub text: Option<String>,
    pub call_id: Option<String>,
    pub turn_id: Option<String>,
    pub status: Option<String>,
    pub detail: Option<String>,
    pub result: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_message: Option<crate::user_message::UserMessage>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeSessionDetail {
    pub summary: ClaudeSessionSummary,
    pub events: Vec<ClaudeHistoryEvent>,
    pub total_events: usize,
    pub has_more: bool,
    pub before_cursor: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaudeDetailRequest {
    pub session_key: String,
    pub limit: Option<usize>,
    pub before: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDeletePreview {
    pub session_key: String,
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub fingerprint: String,
    pub file_count: usize,
    pub bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClaudeDeleteRequest {
    pub session_key: String,
    pub confirmed: bool,
    pub fingerprint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDeleteResult {
    pub session_key: String,
    pub deleted: bool,
    pub bytes: u64,
}

#[derive(Clone, Default)]
struct Parsed {
    id: String,
    title: String,
    cwd: String,
    version: String,
    updated_at: Option<String>,
    events: Vec<ClaudeHistoryEvent>,
    issues: Vec<String>,
    warnings: Vec<String>,
    last_turn_at: Option<i64>,
    turn_open: bool,
    native_events: Vec<crate::native_probe::NativeTurnEvent>,
    record_count: u64,
    settled_count: u64,
}
fn field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}
fn digest(value: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(value.as_ref()))
}
fn key_for(root: &Path, path: &Path, id: &str) -> String {
    format!(
        "claude:{}",
        digest(format!(
            "{}\0{id}",
            path.strip_prefix(root).unwrap_or(path).to_string_lossy()
        ))
    )
}

fn files(paths: &ClaudePaths) -> Result<Vec<PathBuf>> {
    if !paths.projects.exists() {
        return Ok(vec![]);
    }
    ensure!(
        !fs::symlink_metadata(&paths.projects)?
            .file_type()
            .is_symlink(),
        "Claude 会话根不能是符号链接"
    );
    let root = paths.projects.canonicalize()?;
    ensure!(root.is_dir(), "Claude 会话根不是目录");
    let mut result = Vec::new();
    for entry in walkdir::WalkDir::new(&root)
        .follow_links(false)
        .max_depth(8)
        .into_iter()
        .filter_entry(|e| !matches!(e.file_name().to_str(), Some("subagents" | "tool-results")))
    {
        let entry = entry.context("无法扫描 Claude 会话")?;
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|x| x == "jsonl") {
            ensure!(result.len() < 20_000, "Claude 会话数量超出读取上限");
            result.push(entry.into_path());
        }
    }
    result.sort();
    Ok(result)
}
fn read_or_warning(path: &Path) -> Arc<Parsed> {
    reader::read(path).unwrap_or_else(|_| {
        let id = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        Arc::new(Parsed {
            title: id.clone(),
            id,
            version: "unknown".into(),
            issues: vec!["会话无法读取或超出读取上限，管理操作已禁用".into()],
            ..Default::default()
        })
    })
}
fn resolve(paths: &ClaudePaths, key: &str) -> Result<(PathBuf, Arc<Parsed>)> {
    ensure!(
        key.starts_with("claude:")
            && key.len() == 71
            && key[7..].bytes().all(|c| c.is_ascii_hexdigit()),
        "无效的 Claude 会话定位键"
    );
    let root = paths
        .projects
        .canonicalize()
        .context("未发现 Claude 会话")?;
    for path in files(paths)? {
        let parsed = read_or_warning(&path);
        if key_for(&root, &path, &parsed.id) == key {
            return Ok((path, parsed));
        }
    }
    anyhow::bail!("Claude 会话已变化或不存在，请刷新列表")
}
fn summary(
    paths: &ClaudePaths,
    path: &Path,
    parsed: &Parsed,
    snapshot: &activity::Snapshot,
) -> Result<ClaudeSessionSummary> {
    let owner = snapshot.ownership(parsed, path);
    let fatal_reason = (!parsed.issues.is_empty()).then(|| parsed.issues.join("；"));
    let warning_reason = (!parsed.warnings.is_empty()).then(|| parsed.warnings.join("；"));
    let reason = if let Some(reason) = fatal_reason.clone() {
        Some(reason)
    } else if let Some(reason) = warning_reason.clone() {
        Some(reason)
    } else if parsed.turn_open || owner != activity::Ownership::Inactive {
        Some("Claude 会话仍被原生进程占用或归属无法确认，请关闭后重试".into())
    } else {
        None
    };
    let status = if !parsed.issues.is_empty() {
        "unknown"
    } else if !parsed.turn_open {
        "recent"
    } else if owner == activity::Ownership::Open {
        "running"
    } else {
        "unknown"
    };
    /*
     * Fatal parse errors remain the read error and force an unknown status.
     * Compatibility warnings are shown separately and still guard mutations,
     * while notification scanning may use their terminal evidence.
     */
    let read_error = (!parsed.issues.is_empty()).then(|| parsed.issues.join("；"));
    let read_warning = warning_reason;
    let messages: Vec<_> = parsed
        .events
        .iter()
        .filter(|e| matches!(e.kind.as_str(), "user_message" | "assistant_message"))
        .collect();
    Ok(ClaudeSessionSummary {
        storage_size: None,
        id: parsed.id.clone(),
        session_key: key_for(&paths.projects.canonicalize()?, path, &parsed.id),
        title: parsed.title.clone(),
        cwd: parsed.cwd.clone(),
        path: path.to_owned(),
        updated_at: parsed.updated_at.clone(),
        message_count: messages.len(),
        last_message: messages
            .last()
            .and_then(|e| e.text.as_ref())
            .map(|s| s.chars().take(240).collect()),
        status: status.into(),
        format_version: parsed.version.clone(),
        can_rename: reason.is_none(),
        can_delete: reason.is_none(),
        rename_block_reason: reason.clone(),
        delete_block_reason: reason,
        read_error,
        read_warning,
    })
}
pub fn list_claude_sessions(
    paths: &ClaudePaths,
    limit: usize,
    query: Option<&str>,
) -> Result<Vec<ClaudeSessionSummary>> {
    let snapshot = activity::Snapshot::capture();
    let needle = query.unwrap_or("").trim().to_lowercase();
    let mut result = Vec::new();
    for path in files(paths)? {
        let parsed = read_or_warning(&path);
        let s = summary(paths, &path, &parsed, &snapshot)?;
        if [&s.title, &s.id, &s.cwd]
            .iter()
            .any(|v| v.to_lowercase().contains(&needle))
        {
            result.push(s);
        }
    }
    result.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then(a.session_key.cmp(&b.session_key))
    });
    result.truncate(limit.clamp(1, 200));
    for summary in &mut result {
        attach_storage_size(paths, summary);
    }
    Ok(result)
}

fn attach_storage_size(paths: &ClaudePaths, summary: &mut ClaudeSessionSummary) {
    use crate::session_storage::{self, StorageScope, StorageStatus};
    let mut size = session_storage::file_size(&paths.projects, &summary.path);
    if summary.read_error.is_some() {
        summary.storage_size = Some(session_storage::SessionStorageSize::unavailable(
            StorageScope::File,
        ));
        return;
    }
    // A same-ID directory belongs to this transcript only when the native ID,
    // native filename and project all agree. Duplicate IDs in other files do not inherit it.
    if summary.path.file_stem().and_then(|s| s.to_str()) == Some(&summary.id) {
        let directory = summary.path.with_extension("");
        match fs::symlink_metadata(&directory) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {
                size = session_storage::directory_size(
                    &paths.projects,
                    &directory,
                    Some(&summary.path),
                    &summary.session_key,
                );
            }
            Ok(_) => {
                size.scope = StorageScope::Directory;
                size.status = StorageStatus::Partial;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                size.scope = StorageScope::Directory;
                size.status = StorageStatus::Partial;
            }
        }
    }
    summary.storage_size = Some(size);
}
pub fn claude_session_summary(paths: &ClaudePaths, key: &str) -> Result<ClaudeSessionSummary> {
    let (path, parsed) = resolve(paths, key)?;
    let mut summary = summary(paths, &path, &parsed, &activity::Snapshot::capture())?;
    attach_storage_size(paths, &mut summary);
    Ok(summary)
}
pub fn claude_session_detail(
    paths: &ClaudePaths,
    request: &ClaudeDetailRequest,
) -> Result<ClaudeSessionDetail> {
    let (path, parsed) = resolve(paths, &request.session_key)?;
    let end = match &request.before {
        None => parsed.events.len(),
        Some(id) => parsed
            .events
            .iter()
            .position(|e| &e.id == id)
            .context("历史位置已变化，请刷新会话")?,
    };
    let start = end.saturating_sub(request.limit.unwrap_or(160).clamp(1, 500));
    let mut events = parsed.events[start..end].to_vec();
    for event in &mut events {
        if let Some(user) = &mut event.user_message {
            user.refresh_files(Some(&parsed.cwd));
        }
    }
    let mut summary = summary(paths, &path, &parsed, &activity::Snapshot::capture())?;
    attach_storage_size(paths, &mut summary);
    Ok(ClaudeSessionDetail {
        summary,
        events,
        total_events: parsed.events.len(),
        has_more: start > 0,
        before_cursor: (start > 0).then(|| parsed.events[start].id.clone()),
    })
}
pub fn read_attachment(
    paths: &ClaudePaths,
    request: &crate::user_message::SessionAttachmentRequest,
) -> Result<crate::user_message::SessionAttachmentResponse> {
    let (path, parsed) = resolve(paths, &request.session_key)?;
    let before = reader::stamp(&path)?;
    let mut user = parsed
        .events
        .iter()
        .filter_map(|e| e.user_message.as_ref())
        .find(|m| m.id == request.message_id)
        .cloned()
        .context("Claude 附件消息已变化或不存在")?;
    user.refresh_files(Some(&parsed.cwd));
    let result = user.read_attachment(&request.attachment_id)?;
    ensure!(
        before == reader::stamp(&path)?,
        "Claude 会话已变化，请刷新后重试"
    );
    Ok(result)
}
pub(crate) fn notification_snapshots(
    paths: &ClaudePaths,
) -> Result<crate::native_probe::NativeScan> {
    use crate::native_probe::{file_identity, NativeProvider, NativeScan, NativeStreamSnapshot};
    let mut scan = NativeScan::default();
    for path in files(paths)? {
        let parsed = match reader::read(&path) {
            Ok(p) => p,
            Err(_) => {
                scan.errors += 1;
                continue;
            }
        };
        // Compatibility warnings are safe to scan; only fatal parse/identity
        // issues suppress terminal evidence and increment the scan error count.
        if !parsed.issues.is_empty() {
            scan.errors += 1;
            continue;
        }
        scan.streams.push(NativeStreamSnapshot {
            provider: NativeProvider::Claude,
            session_key: key_for(&paths.projects.canonicalize()?, &path, &parsed.id),
            id: parsed.id.clone(),
            title: parsed.title.clone(),
            identity: file_identity(&path)?,
            record_count: parsed.record_count,
            settled_count: parsed.settled_count,
            events: parsed.native_events.clone(),
        });
    }
    Ok(scan)
}
