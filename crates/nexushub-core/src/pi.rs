use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    env,
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Component, Path, PathBuf},
    process::Stdio,
    time::Duration,
};

#[cfg(test)]
mod tests;

const MAX_SESSION_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 200_000;
const MAX_EVENT_TEXT: usize = 256 * 1024;

static SESSION_CACHE: crate::read_cache::ReadCache<ParsedFile> =
    crate::read_cache::ReadCache::new(64 * 1024 * 1024);
static EVENT_CACHE: crate::read_cache::ReadCache<Vec<PiHistoryEvent>> =
    crate::read_cache::ReadCache::new(16 * 1024 * 1024);

#[derive(Debug, Clone)]
pub struct PiPaths {
    pub sessions: PathBuf,
    pub agent_dir: PathBuf,
}

impl PiPaths {
    pub fn default_for_user() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let agent_dir = env::var_os("PI_CODING_AGENT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".pi").join("agent"));
        let sessions = env::var_os("PI_CODING_AGENT_SESSION_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| agent_dir.join("sessions"));
        Self {
            sessions,
            agent_dir,
        }
    }

    #[cfg(test)]
    pub fn from_sessions(sessions: PathBuf) -> Self {
        let agent_dir = sessions
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| sessions.clone());
        Self {
            sessions,
            agent_dir,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PiSessionSummary {
    pub id: String,
    pub session_key: String,
    pub title: String,
    pub cwd: String,
    pub path: PathBuf,
    pub updated_at: Option<String>,
    pub message_count: usize,
    pub last_message: Option<String>,
    pub status: String,
    pub format_version: u32,
    pub can_rename: bool,
    pub rename_block_reason: Option<String>,
    pub can_delete: bool,
    pub delete_block_reason: Option<String>,
    pub read_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PiSessionDetail {
    pub summary: PiSessionSummary,
    pub events: Vec<PiHistoryEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PiHistoryEvent {
    pub timestamp: Option<String>,
    pub kind: String,
    pub role: Option<String>,
    pub text: Option<String>,
    pub call_id: Option<String>,
    pub status: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PiDeletePreview {
    pub session_key: String,
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub fingerprint: String,
    pub file_count: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PiDeleteRequest {
    pub session_key: String,
    pub confirmed: bool,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PiDeleteResult {
    pub session_key: String,
    pub deleted: bool,
    pub bytes: u64,
}

#[derive(Debug, Clone)]
struct ParsedFile {
    id: String,
    title: String,
    cwd: String,
    updated_at: Option<String>,
    message_count: usize,
    last_message: Option<String>,
    format_version: u32,
    entries: Vec<Value>,
    active_entry_ids: Vec<String>,
    issues: Vec<String>,
    incomplete_tail: bool,
}

#[derive(Debug, Clone)]
struct ParsedSession {
    summary: PiSessionSummary,
    file: ParsedFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Activity {
    Inactive,
    Active,
    Unknown,
}

#[derive(Debug, Clone)]
struct ProcessSnapshot {
    command: String,
    args: String,
}

pub fn list_pi_sessions(
    paths: &PiPaths,
    limit: usize,
    query: Option<&str>,
) -> Result<Vec<PiSessionSummary>> {
    let root = match fs::canonicalize(&paths.sessions) {
        Ok(root) => root,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Pi sessions unavailable: {}", paths.sessions.display()))
        }
    };
    ensure!(root.is_dir(), "Pi sessions root is not a directory");
    let activity = detect_pi_activity(&root);
    let cli_available = find_pi_executable().is_some();
    let needle = query
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);
    let mut result = Vec::new();
    for entry in walkdir::WalkDir::new(&root)
        .follow_links(false)
        .max_depth(8)
    {
        let entry = entry.with_context(|| format!("scan Pi sessions under {}", root.display()))?;
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("jsonl")
        {
            continue;
        }
        let summary = match read_parsed_file(entry.path()) {
            Ok(file) => decorate_summary(entry.path(), &root, file, activity, cli_available),
            Err(error) => unreadable_summary(entry.path(), &root, &error),
        };
        if needle.as_ref().is_some_and(|needle| {
            ![
                summary.title.as_str(),
                summary.id.as_str(),
                summary.cwd.as_str(),
                summary.session_key.as_str(),
                summary.read_error.as_deref().unwrap_or_default(),
            ]
            .iter()
            .any(|value| value.to_ascii_lowercase().contains(needle))
        }) {
            continue;
        }
        result.push(summary);
    }
    result.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then(left.session_key.cmp(&right.session_key))
    });
    result.truncate(limit.clamp(1, 200));
    Ok(result)
}

pub(crate) fn pi_session_summary(paths: &PiPaths, session_key: &str) -> Result<PiSessionSummary> {
    Ok(resolve_session(paths, session_key)?.summary)
}

pub fn pi_session_detail(paths: &PiPaths, session_key: &str) -> Result<PiSessionDetail> {
    let parsed = resolve_session(paths, session_key)?;
    let path = parsed.summary.path.clone();
    let active_entry_ids = parsed.file.active_entry_ids.clone();
    let entries = parsed.file.entries.clone();
    let events = EVENT_CACHE.read(
        &path,
        |events, size| size.saturating_add(events.iter().map(event_weight).sum::<u64>()),
        || Ok(history_events(&entries, &active_entry_ids)),
    )?;
    Ok(PiSessionDetail {
        summary: parsed.summary,
        events,
    })
}

pub(crate) fn notification_snapshots(paths: &PiPaths) -> Result<crate::native_probe::NativeScan> {
    use crate::native_probe::{
        file_identity, pi_events, NativeProvider, NativeScan, NativeStreamSnapshot,
    };
    let mut scan = NativeScan::default();
    if !paths.sessions.exists() {
        return Ok(scan);
    }
    let root = canonical_root(&paths.sessions)?;
    for entry in walkdir::WalkDir::new(&root)
        .follow_links(false)
        .max_depth(8)
    {
        let entry = entry?;
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|v| v.to_str()) != Some("jsonl")
        {
            continue;
        }
        let read = (|| -> Result<NativeStreamSnapshot> {
            let file = read_parsed_file(entry.path())?;
            ensure!(
                file.format_version == 3 && file.issues.is_empty() && !file.incomplete_tail,
                "Pi notification source is incomplete or unsupported"
            );
            let events = pi_events(&file.entries, &file.active_entry_ids);
            Ok(NativeStreamSnapshot {
                provider: NativeProvider::Pi,
                session_key: session_key_for(entry.path(), &root),
                id: file.id,
                title: file.title,
                identity: file_identity(entry.path())?,
                record_count: file.entries.len() as u64,
                settled_count: file.entries.len() as u64,
                events,
            })
        })();
        match read {
            Ok(stream) => scan.streams.push(stream),
            Err(_) => scan.errors += 1,
        }
    }
    Ok(scan)
}

pub fn preview_pi_delete(paths: &PiPaths, session_key: &str) -> Result<PiDeletePreview> {
    preview_pi_delete_with_activity(paths, session_key, detect_pi_activity)
}

fn preview_pi_delete_with_activity(
    paths: &PiPaths,
    session_key: &str,
    activity_check: impl Fn(&Path) -> Activity,
) -> Result<PiDeletePreview> {
    let root = canonical_root(&paths.sessions)?;
    let parsed = resolve_session_with_root(
        paths,
        &root,
        session_key,
        activity_check(&root),
        find_pi_executable().is_some(),
    )?;
    ensure_delete_allowed(&parsed.summary)?;
    ensure_parent_writable(&parsed.summary.path, "Pi")?;
    let (fingerprint, file_count, bytes) = file_fingerprint(&parsed.summary.path)?;
    Ok(PiDeletePreview {
        session_key: parsed.summary.session_key,
        id: parsed.summary.id,
        title: parsed.summary.title,
        path: parsed.summary.path,
        fingerprint,
        file_count,
        bytes,
    })
}

pub fn execute_pi_delete(paths: &PiPaths, request: PiDeleteRequest) -> Result<PiDeleteResult> {
    execute_pi_delete_with_activity(paths, request, detect_pi_activity)
}

fn execute_pi_delete_with_activity(
    paths: &PiPaths,
    request: PiDeleteRequest,
    activity_check: impl Fn(&Path) -> Activity + Copy,
) -> Result<PiDeleteResult> {
    ensure!(request.confirmed, "Pi deletion requires confirmation");
    let root = canonical_root(&paths.sessions)?;
    let preview = preview_pi_delete_with_activity(paths, &request.session_key, activity_check)?;
    ensure!(
        preview.fingerprint == request.fingerprint,
        "Pi session changed; preview again"
    );
    ensure!(
        activity_check(&root) == Activity::Inactive,
        "Pi activity became active or could no longer be attributed; the session was not deleted"
    );
    let parent = preview.path.parent().context("invalid Pi session parent")?;
    let retired = parent.join(format!(".nexushub-delete-{}.pending", uuid::Uuid::new_v4()));
    fs::rename(&preview.path, &retired).with_context(|| {
        storage_write_error("Pi", parent, "move the selected session into quarantine")
    })?;
    let result = (|| -> Result<()> {
        ensure!(
            file_fingerprint(&retired)?.0 == preview.fingerprint,
            "Pi session changed during deletion"
        );
        ensure!(
            activity_check(&root) == Activity::Inactive,
            "Pi activity became active or could no longer be attributed"
        );
        fs::remove_file(&retired)
            .with_context(|| storage_write_error("Pi", parent, "remove the quarantined session"))?;
        Ok(())
    })();
    if let Err(error) = result {
        let restored = if !preview.path.exists() && retired.exists() {
            fs::rename(&retired, &preview.path).is_ok()
        } else {
            preview.path.exists()
        };
        let location = if restored { &preview.path } else { &retired };
        return Err(error.context(format!(
            "Pi deletion cancelled; recoverable session remains at {}",
            location.display()
        )));
    }
    Ok(PiDeleteResult {
        session_key: request.session_key,
        deleted: true,
        bytes: preview.bytes,
    })
}

pub async fn rename_pi_session(
    paths: &PiPaths,
    session_key: &str,
    title: &str,
) -> Result<PiSessionSummary> {
    let executable = find_pi_executable()
        .context("Pi executable unavailable; install Pi or set PI_CODING_AGENT_BIN")?;
    rename_pi_session_with(
        paths,
        session_key,
        title,
        &executable,
        Duration::from_secs(12),
        detect_pi_activity,
    )
    .await
}

async fn rename_pi_session_with(
    paths: &PiPaths,
    session_key: &str,
    title: &str,
    executable: &Path,
    timeout: Duration,
    activity_check: impl Fn(&Path) -> Activity,
) -> Result<PiSessionSummary> {
    let root = canonical_root(&paths.sessions)?;
    let parsed = resolve_session_with_root(paths, &root, session_key, activity_check(&root), true)?;
    ensure_rename_allowed(&parsed.summary)?;
    let title = title.trim();
    ensure!(
        !title.is_empty() && title.chars().count() <= 200 && !title.chars().any(char::is_control),
        "Pi title must contain 1-200 characters without control characters"
    );
    if parsed.summary.title == title {
        return Ok(parsed.summary);
    }
    OpenOptions::new()
        .append(true)
        .open(&parsed.summary.path)
        .with_context(|| {
            storage_write_error(
                "Pi",
                &parsed.summary.path,
                "open the session for native rename",
            )
        })?;
    ensure!(
        activity_check(&root) == Activity::Inactive,
        "Pi activity became active or could no longer be attributed; rename was not started"
    );
    let before = file_fingerprint(&parsed.summary.path)?.0;
    let current = resolve_session_with_root(paths, &root, session_key, Activity::Inactive, true)?;
    ensure!(
        current.summary.id == parsed.summary.id && current.summary.path == parsed.summary.path,
        "Pi session identity changed before rename"
    );
    ensure!(
        file_fingerprint(&current.summary.path)?.0 == before,
        "Pi session changed before rename"
    );
    run_pi_name_protocol(
        PiRenameProtocol {
            executable,
            session: &parsed.summary.path,
            sessions_root: &root,
            agent_dir: &paths.agent_dir,
            cwd: &parsed.summary.cwd,
            expected_id: &parsed.summary.id,
            title,
        },
        timeout,
    )
    .await?;
    let final_activity = activity_check(&root);
    ensure!(
        final_activity == Activity::Inactive,
        "Pi activity changed during rename; refresh the session before another operation"
    );
    let renamed = resolve_session_with_root(paths, &root, session_key, final_activity, true)?;
    ensure!(
        renamed.summary.id == parsed.summary.id && renamed.summary.path == parsed.summary.path,
        "Pi session identity changed during rename"
    );
    ensure!(
        renamed.summary.title == title,
        "Pi native rename did not persist the requested title"
    );
    ensure!(
        file_fingerprint(&renamed.summary.path)?.0 != before,
        "Pi native rename returned without changing the session file"
    );
    Ok(renamed.summary)
}

fn canonical_root(path: &Path) -> Result<PathBuf> {
    let root = path
        .canonicalize()
        .with_context(|| format!("Pi sessions unavailable: {}", path.display()))?;
    ensure!(root.is_dir(), "Pi sessions root is not a directory");
    Ok(root)
}

fn resolve_session(paths: &PiPaths, session_key: &str) -> Result<ParsedSession> {
    let root = canonical_root(&paths.sessions)?;
    let activity = detect_pi_activity(&root);
    resolve_session_with_root(
        paths,
        &root,
        session_key,
        activity,
        find_pi_executable().is_some(),
    )
}

fn resolve_session_with_root(
    _paths: &PiPaths,
    root: &Path,
    session_key: &str,
    activity: Activity,
    cli_available: bool,
) -> Result<ParsedSession> {
    let path = discover_session_path(root, session_key)?;
    let file = read_parsed_file(&path)?;
    let summary = decorate_summary(&path, root, file.clone(), activity, cli_available);
    Ok(ParsedSession { summary, file })
}

fn discover_session_path(root: &Path, session_key: &str) -> Result<PathBuf> {
    let key_path = Path::new(session_key);
    ensure!(
        !session_key.is_empty()
            && !key_path.is_absolute()
            && key_path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "invalid Pi session key"
    );
    ensure!(
        key_path.extension().and_then(|value| value.to_str()) == Some("jsonl"),
        "Pi session key must identify a JSONL file"
    );
    for entry in walkdir::WalkDir::new(root).follow_links(false).max_depth(8) {
        let entry = entry.with_context(|| format!("scan Pi sessions under {}", root.display()))?;
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("jsonl")
        {
            continue;
        }
        let key = session_key_for(entry.path(), root);
        if key != session_key {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Pi session must be a regular file, not a symlink"
        );
        let canonical = entry.path().canonicalize()?;
        ensure!(
            canonical.starts_with(root) && canonical == entry.path(),
            "Pi session escaped its configured root or traversed a symlink"
        );
        return Ok(canonical);
    }
    bail!("Pi session not found")
}

fn session_key_for(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .trim_start_matches('/')
        .to_string()
}

fn read_parsed_file(path: &Path) -> Result<ParsedFile> {
    SESSION_CACHE.read(
        path,
        |_, size| size.saturating_mul(2),
        || parse_session_file(path),
    )
}

fn parse_session_file(path: &Path) -> Result<ParsedFile> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "Pi session must be a regular file"
    );
    ensure!(
        metadata.len() <= MAX_SESSION_BYTES,
        "Pi session exceeds size limit"
    );
    let bytes = fs::read(path)?;
    let last_segment = bytes.iter().filter(|byte| **byte == b'\n').count();
    let has_terminal_newline = bytes.ends_with(b"\n");
    let mut entries = Vec::new();
    let mut incomplete_tail = false;
    for (index, raw) in bytes.split(|byte| *byte == b'\n').enumerate() {
        let raw = raw.strip_suffix(b"\r").unwrap_or(raw);
        if raw.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match serde_json::from_slice::<Value>(raw) {
            Ok(value) => {
                ensure!(
                    entries.len() < MAX_ENTRIES,
                    "Pi session exceeds entry limit"
                );
                entries.push(value);
            }
            Err(_) if index == last_segment && !has_terminal_newline => {
                incomplete_tail = true;
                break;
            }
            Err(error) => {
                bail!(
                    "Pi session contains malformed JSON at line {}: {}",
                    index + 1,
                    error
                )
            }
        }
    }
    let header = entries.first().context("Pi session header missing")?;
    ensure!(
        header.get("type").and_then(Value::as_str) == Some("session"),
        "Pi session header missing"
    );
    let version_u64 = header.get("version").and_then(Value::as_u64).unwrap_or(1);
    let version = u32::try_from(version_u64).context("Pi session version is out of range")?;
    ensure!(version > 0, "Pi session version must be positive");
    let id = header
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .context("Pi session id missing")?
        .to_string();
    let cwd = header
        .get("cwd")
        .and_then(Value::as_str)
        .context("Pi session cwd missing")?
        .to_string();
    ensure!(
        Path::new(&cwd).is_absolute(),
        "Pi session cwd must be absolute"
    );
    let header_timestamp = header
        .get("timestamp")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    let mut entries = normalize_legacy_entries(entries, version);
    let mut issues = Vec::new();
    if version > 3 {
        issues.push(format!(
            "Unsupported Pi session format version {version}; management is disabled"
        ));
    }
    if incomplete_tail {
        issues.push(
            "Pi session has an incomplete trailing line; showing the last complete state"
                .to_string(),
        );
    }
    let (active_entry_ids, tree_issues) = active_branch(&entries);
    issues.extend(tree_issues);

    let active_entries = active_entry_ids
        .iter()
        .filter_map(|entry_id| {
            entries
                .iter()
                .find(|entry| entry.get("id").and_then(Value::as_str) == Some(entry_id.as_str()))
        })
        .collect::<Vec<_>>();
    let mut latest_name: Option<Option<String>> = None;
    let mut updated_at = header_timestamp;
    for value in entries.iter().skip(1) {
        if let Some(timestamp) = value.get("timestamp").and_then(Value::as_str) {
            updated_at = Some(timestamp.to_string());
        }
        if value.get("type").and_then(Value::as_str) == Some("session_info") {
            latest_name = Some(
                value
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(ToString::to_string),
            );
        }
    }
    let message_count = active_entries
        .iter()
        .filter(|entry| entry.get("type").and_then(Value::as_str) == Some("message"))
        .count();
    let last_message = active_entries.iter().rev().find_map(|entry| {
        (entry.get("type").and_then(Value::as_str) == Some("message"))
            .then(|| message_text(entry.get("message").unwrap_or(&Value::Null)))
            .flatten()
    });
    let fallback_title = first_user_text(&active_entries).unwrap_or_else(|| id.clone());
    let title = latest_name.flatten().unwrap_or(fallback_title);

    entries.shrink_to_fit();
    Ok(ParsedFile {
        id,
        title,
        cwd,
        updated_at,
        message_count,
        last_message,
        format_version: version,
        entries,
        active_entry_ids,
        issues,
        incomplete_tail,
    })
}

fn normalize_legacy_entries(mut entries: Vec<Value>, version: u32) -> Vec<Value> {
    let mut previous_id: Option<String> = None;
    for (index, entry) in entries.iter_mut().enumerate().skip(1) {
        let Some(object) = entry.as_object_mut() else {
            continue;
        };
        if version == 1 {
            let entry_id = object
                .get("id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("legacy-{index:08x}"));
            object.insert("id".to_string(), Value::String(entry_id.clone()));
            if !object.contains_key("parentId") {
                object.insert(
                    "parentId".to_string(),
                    previous_id
                        .as_ref()
                        .map_or(Value::Null, |value| Value::String(value.clone())),
                );
            }
            previous_id = Some(entry_id);
        }
        if version <= 2 {
            if let Some(message) = object.get_mut("message").and_then(Value::as_object_mut) {
                if message.get("role").and_then(Value::as_str) == Some("hookMessage") {
                    message.insert("role".to_string(), Value::String("custom".to_string()));
                }
            }
        }
    }
    entries
}

fn active_branch(entries: &[Value]) -> (Vec<String>, Vec<String>) {
    let mut by_id = HashMap::<String, &Value>::new();
    let mut leaf = None;
    let mut issues = Vec::new();
    for (index, entry) in entries.iter().enumerate().skip(1) {
        let Some(entry_id) = entry
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        else {
            issues.push(format!("Pi session entry {} has no id", index + 1));
            continue;
        };
        if by_id.insert(entry_id.to_string(), entry).is_some() {
            issues.push(format!("Pi session contains duplicate entry id {entry_id}"));
        }
        leaf = Some(entry_id.to_string());
    }
    for (entry_id, entry) in &by_id {
        if let Some(parent_id) = entry.get("parentId").and_then(Value::as_str) {
            if !by_id.contains_key(parent_id) {
                issues.push(format!(
                    "Pi session entry {entry_id} references missing parent {parent_id}"
                ));
            }
        } else if entry.get("parentId").is_none() {
            issues.push(format!("Pi session entry {entry_id} has no parentId field"));
        }
    }
    let mut chain = Vec::new();
    let mut current = leaf;
    let mut seen = HashSet::new();
    while let Some(entry_id) = current {
        if !seen.insert(entry_id.clone()) {
            issues.push(format!("Pi session branch contains a cycle at {entry_id}"));
            break;
        }
        let Some(entry) = by_id.get(&entry_id) else {
            break;
        };
        chain.push(entry_id);
        current = entry
            .get("parentId")
            .and_then(Value::as_str)
            .map(ToString::to_string);
    }
    chain.reverse();
    (chain, issues)
}

fn decorate_summary(
    path: &Path,
    root: &Path,
    file: ParsedFile,
    activity: Activity,
    cli_available: bool,
) -> PiSessionSummary {
    let common_block = if file.format_version != 3 {
        Some(if file.format_version < 3 {
            "Legacy Pi session format is read-only until Pi migrates it".to_string()
        } else {
            format!(
                "Unsupported Pi session format version {}; management is disabled",
                file.format_version
            )
        })
    } else if !file.issues.is_empty() {
        Some(file.issues.join("; "))
    } else {
        match activity {
            Activity::Inactive => None,
            Activity::Active => {
                Some("Pi session root is active; close Pi before changing it".to_string())
            }
            Activity::Unknown => Some(
                "Pi process ownership cannot be confirmed; close Pi before changing this root"
                    .to_string(),
            ),
        }
    };
    let rename_block_reason = common_block.clone().or_else(|| {
        if !cli_available {
            Some("Pi executable unavailable; native rename is disabled".to_string())
        } else if !Path::new(&file.cwd).is_dir() {
            Some("Pi working directory is unavailable; native rename is disabled".to_string())
        } else {
            None
        }
    });
    let status = if !file.issues.is_empty() {
        if file.incomplete_tail && file.issues.len() == 1 {
            "incomplete"
        } else {
            "error"
        }
    } else {
        match activity {
            Activity::Inactive => "recent",
            Activity::Active => "running",
            Activity::Unknown => "unknown",
        }
    };
    PiSessionSummary {
        id: file.id,
        session_key: session_key_for(path, root),
        title: file.title,
        cwd: file.cwd,
        path: path.to_path_buf(),
        updated_at: file.updated_at,
        message_count: file.message_count,
        last_message: file.last_message,
        status: status.to_string(),
        format_version: file.format_version,
        can_rename: rename_block_reason.is_none(),
        rename_block_reason,
        can_delete: common_block.is_none(),
        delete_block_reason: common_block,
        read_error: (!file.issues.is_empty()).then(|| file.issues.join("; ")),
    }
}

fn unreadable_summary(path: &Path, root: &Path, error: &anyhow::Error) -> PiSessionSummary {
    let message = format!("Unable to read Pi session: {error:#}");
    PiSessionSummary {
        id: path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("invalid-session")
            .to_string(),
        session_key: session_key_for(path, root),
        title: "无法读取的 Pi 会话".to_string(),
        cwd: String::new(),
        path: path.to_path_buf(),
        updated_at: None,
        message_count: 0,
        last_message: None,
        status: "error".to_string(),
        format_version: 0,
        can_rename: false,
        rename_block_reason: Some(message.clone()),
        can_delete: false,
        delete_block_reason: Some(message.clone()),
        read_error: Some(message),
    }
}

fn first_user_text(entries: &[&Value]) -> Option<String> {
    entries
        .iter()
        .find_map(|entry| {
            (entry.get("type").and_then(Value::as_str) == Some("message")
                && entry.pointer("/message/role").and_then(Value::as_str) == Some("user"))
            .then(|| message_text(entry.get("message").unwrap_or(&Value::Null)))
            .flatten()
        })
        .map(|text| text.chars().take(120).collect())
}

fn message_text(message: &Value) -> Option<String> {
    content_text(message.get("content").unwrap_or(message))
}

fn content_text(content: &Value) -> Option<String> {
    let mut text = match content {
        Value::String(value) => value.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|item| match item.get("type").and_then(Value::as_str) {
                Some("text") => item
                    .get("text")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
                Some("image") => Some(
                    item.get("mimeType")
                        .and_then(Value::as_str)
                        .map_or_else(|| "[image]".to_string(), |mime| format!("[image: {mime}]")),
                ),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    };
    truncate_utf8(&mut text, MAX_EVENT_TEXT);
    (!text.trim().is_empty()).then_some(text)
}

fn history_events(entries: &[Value], active_entry_ids: &[String]) -> Vec<PiHistoryEvent> {
    let by_id = entries
        .iter()
        .filter_map(|entry| {
            entry
                .get("id")
                .and_then(Value::as_str)
                .map(|entry_id| (entry_id, entry))
        })
        .collect::<HashMap<_, _>>();
    active_entry_ids
        .iter()
        .filter_map(|entry_id| by_id.get(entry_id.as_str()).copied())
        .flat_map(event_from_entry)
        .collect()
}

fn event_from_entry(entry: &Value) -> Vec<PiHistoryEvent> {
    let timestamp = entry
        .get("timestamp")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    match entry.get("type").and_then(Value::as_str) {
        Some("message") => message_events(timestamp, entry.get("message").unwrap_or(&Value::Null)),
        Some("compaction") => vec![PiHistoryEvent {
            timestamp,
            kind: "compaction".to_string(),
            role: Some("summary".to_string()),
            text: entry
                .get("summary")
                .and_then(Value::as_str)
                .map(bounded_text),
            call_id: None,
            status: None,
            detail: entry
                .get("tokensBefore")
                .map(|tokens| format!("压缩前 tokens: {}", tokens.as_u64().unwrap_or_default())),
        }],
        Some("branch_summary") => vec![PiHistoryEvent {
            timestamp,
            kind: "branch_summary".to_string(),
            role: Some("summary".to_string()),
            text: entry
                .get("summary")
                .and_then(Value::as_str)
                .map(bounded_text),
            call_id: None,
            status: None,
            detail: entry
                .get("fromId")
                .and_then(Value::as_str)
                .map(|from_id| format!("来自分支 {from_id}")),
        }],
        Some("custom_message") if entry.get("display").and_then(Value::as_bool) != Some(false) => {
            vec![PiHistoryEvent {
                timestamp,
                kind: "custom_message".to_string(),
                role: Some("custom".to_string()),
                text: content_text(entry.get("content").unwrap_or(&Value::Null)),
                call_id: None,
                status: None,
                detail: entry
                    .get("customType")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            }]
        }
        _ => Vec::new(),
    }
}

fn message_events(timestamp: Option<String>, message: &Value) -> Vec<PiHistoryEvent> {
    let role = message
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("event");
    if role == "system" {
        return Vec::new();
    }
    if role == "toolResult" {
        return vec![PiHistoryEvent {
            timestamp,
            kind: "tool_result".to_string(),
            role: Some(role.to_string()),
            text: message
                .get("toolName")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            call_id: message
                .get("toolCallId")
                .and_then(Value::as_str)
                .map(ToString::to_string),
            status: Some(
                if message.get("isError").and_then(Value::as_bool) == Some(true) {
                    "failed"
                } else {
                    "completed"
                }
                .to_string(),
            ),
            detail: content_text(message.get("content").unwrap_or(&Value::Null)),
        }];
    }
    if role == "bashExecution" {
        let failed = message
            .get("exitCode")
            .and_then(Value::as_i64)
            .is_some_and(|code| code != 0);
        return vec![PiHistoryEvent {
            timestamp,
            kind: "tool_result".to_string(),
            role: Some(role.to_string()),
            text: message
                .get("command")
                .and_then(Value::as_str)
                .map(bounded_text),
            call_id: None,
            status: Some(if failed { "failed" } else { "completed" }.to_string()),
            detail: message
                .get("output")
                .and_then(Value::as_str)
                .map(bounded_text),
        }];
    }
    if role == "custom" && message.get("display").and_then(Value::as_bool) == Some(false) {
        return Vec::new();
    }
    let content = message.get("content").unwrap_or(message);
    if let Value::String(text) = content {
        return vec![PiHistoryEvent {
            timestamp,
            kind: format!("{role}_message"),
            role: Some(role.to_string()),
            text: Some(bounded_text(text)),
            call_id: None,
            status: None,
            detail: None,
        }];
    }
    let Some(items) = content.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let kind = item.get("type").and_then(Value::as_str)?;
            let (event_kind, text, call_id, detail) =
                match kind {
                    "text" => (
                        format!("{role}_message"),
                        item.get("text").and_then(Value::as_str).map(bounded_text),
                        None,
                        None,
                    ),
                    "thinking" => (
                        "thinking".to_string(),
                        item.get("thinking")
                            .and_then(Value::as_str)
                            .map(bounded_text),
                        None,
                        None,
                    ),
                    "toolCall" => (
                        "tool_call".to_string(),
                        item.get("name")
                            .and_then(Value::as_str)
                            .map(ToString::to_string),
                        item.get("id")
                            .and_then(Value::as_str)
                            .map(ToString::to_string),
                        item.get("arguments").map(bounded_json),
                    ),
                    "image" => (
                        format!("{role}_message"),
                        Some(item.get("mimeType").and_then(Value::as_str).map_or_else(
                            || "[image]".to_string(),
                            |mime| format!("[image: {mime}]"),
                        )),
                        None,
                        None,
                    ),
                    _ => return None,
                };
            Some(PiHistoryEvent {
                timestamp: timestamp.clone(),
                kind: event_kind,
                role: Some(role.to_string()),
                text,
                call_id,
                status: None,
                detail,
            })
        })
        .collect()
}

fn bounded_text(value: &str) -> String {
    let mut text = value.to_string();
    truncate_utf8(&mut text, MAX_EVENT_TEXT);
    text
}

fn bounded_json(value: &Value) -> String {
    let mut text = value.to_string();
    truncate_utf8(&mut text, MAX_EVENT_TEXT);
    text
}

fn truncate_utf8(text: &mut String, max_bytes: usize) {
    if text.len() <= max_bytes {
        return;
    }
    let mut end = max_bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
}

fn event_weight(event: &PiHistoryEvent) -> u64 {
    192 + event.text.as_ref().map_or(0, String::len) as u64
        + event.detail.as_ref().map_or(0, String::len) as u64
}

fn ensure_rename_allowed(summary: &PiSessionSummary) -> Result<()> {
    ensure!(
        summary.can_rename,
        "{}",
        summary
            .rename_block_reason
            .as_deref()
            .unwrap_or("Pi session rename is disabled")
    );
    Ok(())
}

fn ensure_delete_allowed(summary: &PiSessionSummary) -> Result<()> {
    ensure!(
        summary.can_delete,
        "{}",
        summary
            .delete_block_reason
            .as_deref()
            .unwrap_or("Pi session deletion is disabled")
    );
    Ok(())
}

fn file_fingerprint(path: &Path) -> Result<(String, usize, u64)> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "Pi session must be a regular file"
    );
    let mut source = File::open(path)?;
    let mut hash = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        ensure!(
            bytes <= MAX_SESSION_BYTES,
            "Pi session exceeds deletion audit limit"
        );
        hash.update(&buffer[..count]);
    }
    Ok((hex::encode(hash.finalize()), 1, bytes))
}

fn ensure_parent_writable(path: &Path, provider: &str) -> Result<()> {
    let parent = path.parent().context("session parent is missing")?;
    let marker = parent.join(format!(".nexushub-write-check-{}", uuid::Uuid::new_v4()));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker)
        .with_context(|| storage_write_error(provider, parent, "create a write-check marker"))?;
    drop(file);
    fs::remove_file(&marker).with_context(|| {
        format!(
            "{}; temporary marker remains at {}",
            storage_write_error(provider, parent, "remove the write-check marker"),
            marker.display()
        )
    })?;
    Ok(())
}

fn storage_write_error(provider: &str, path: &Path, operation: &str) -> String {
    format!(
        "cannot {operation} at {}. {provider} session storage is not writable from NexusHub; if systemd reports a read-only filesystem or permission denial, add this exact session root to ReadWritePaths, run systemctl daemon-reload, and restart nexushub-webd",
        path.display()
    )
}

fn detect_pi_activity(root: &Path) -> Activity {
    #[cfg(unix)]
    {
        let output = std::process::Command::new("ps")
            .args(["-axo", "pid=,comm=,args="])
            .output();
        let Ok(output) = output else {
            return Activity::Unknown;
        };
        if !output.status.success() {
            return Activity::Unknown;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let current_pid = std::process::id().to_string();
        let processes = text.lines().filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?;
            let command = fields.next()?;
            if pid == current_pid {
                return None;
            }
            let args = fields.collect::<Vec<_>>().join(" ");
            Some(ProcessSnapshot {
                command: command.to_string(),
                args,
            })
        });
        activity_from_processes(root, processes)
    }
    #[cfg(not(unix))]
    {
        let _ = root;
        Activity::Unknown
    }
}

fn activity_from_processes(
    root: &Path,
    processes: impl IntoIterator<Item = ProcessSnapshot>,
) -> Activity {
    let mut unattributed = false;
    for process in processes {
        if !is_pi_process(&process) {
            continue;
        }
        if let Some(session_dir) = process_flag_value(&process.args, "--session-dir") {
            if paths_refer_to_same_root(Path::new(session_dir), root) {
                return Activity::Active;
            }
            continue;
        }
        if let Some(session) = process_flag_value(&process.args, "--session") {
            let session = Path::new(session);
            if session.is_absolute() {
                if path_is_within(session, root) {
                    return Activity::Active;
                }
                continue;
            }
        }
        unattributed = true;
    }
    if unattributed {
        Activity::Unknown
    } else {
        Activity::Inactive
    }
}

fn is_pi_process(process: &ProcessSnapshot) -> bool {
    let command = Path::new(&process.command)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(&process.command)
        .to_ascii_lowercase();
    if matches!(command.as_str(), "pi" | "pi-rpc") {
        return true;
    }
    let args = process.args.to_ascii_lowercase();
    let first = args.split_whitespace().next().unwrap_or_default();
    let first_name = Path::new(first)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    matches!(first_name, "pi" | "pi-rpc")
        || args.contains("@earendil-works/pi-coding-agent")
        || args.contains("/pi-coding-agent/")
        || args.contains("/bin/pi ")
}

fn process_flag_value<'a>(args: &'a str, flag: &str) -> Option<&'a str> {
    let mut parts = args.split_whitespace();
    while let Some(part) = parts.next() {
        if part == flag {
            return parts.next();
        }
        if let Some(value) = part.strip_prefix(&format!("{flag}=")) {
            return Some(value);
        }
    }
    None
}

fn paths_refer_to_same_root(candidate: &Path, root: &Path) -> bool {
    candidate
        .canonicalize()
        .map_or_else(|_| candidate == root, |path| path == root)
}

fn path_is_within(candidate: &Path, root: &Path) -> bool {
    candidate.canonicalize().map_or_else(
        |_| candidate.starts_with(root),
        |path| path.starts_with(root),
    )
}

fn find_pi_executable() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("PI_CODING_AGENT_BIN") {
        candidates.push(PathBuf::from(path));
    }
    if let Some(path) = env::var_os("PATH") {
        candidates.extend(env::split_paths(&path).map(|directory| directory.join("pi")));
    }
    candidates.extend([
        PathBuf::from("/opt/homebrew/bin/pi"),
        PathBuf::from("/usr/local/bin/pi"),
    ]);
    if let Some(home) = dirs::home_dir() {
        candidates.extend([
            home.join(".local/bin/pi"),
            home.join(".volta/bin/pi"),
            home.join(".bun/bin/pi"),
            home.join("Library/pnpm/pi"),
        ]);
        let nvm_versions = home.join(".nvm/versions/node");
        if let Ok(entries) = fs::read_dir(nvm_versions) {
            let mut versions = entries
                .filter_map(Result::ok)
                .map(|entry| entry.path().join("bin/pi"))
                .collect::<Vec<_>>();
            versions.sort_by(|left, right| right.cmp(left));
            candidates.extend(versions);
        }
    }
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .find(|path| seen.insert(path.clone()) && is_executable_file(path))
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

struct PiRenameProtocol<'a> {
    executable: &'a Path,
    session: &'a Path,
    sessions_root: &'a Path,
    agent_dir: &'a Path,
    cwd: &'a str,
    expected_id: &'a str,
    title: &'a str,
}

async fn run_pi_name_protocol(request: PiRenameProtocol<'_>, timeout: Duration) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};

    let expected_session = request.session.canonicalize()?;
    let mut command = tokio::process::Command::new(request.executable);
    command.args([
        "--mode",
        "rpc",
        "--offline",
        "--no-context-files",
        "--no-tools",
        "--no-extensions",
        "--no-skills",
        "--no-prompt-templates",
        "--no-themes",
        "--no-approve",
        "--session",
    ]);
    command
        .arg(request.session)
        .arg("--session-dir")
        .arg(request.sessions_root);
    command
        .env("PI_OFFLINE", "1")
        .env("PI_CODING_AGENT_DIR", request.agent_dir)
        .env("PI_CODING_AGENT_SESSION_DIR", request.sessions_root)
        .current_dir(request.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    if let Some(path) = augmented_path(request.executable) {
        command.env("PATH", path);
    }
    let mut child = command.spawn().context("start Pi native RPC")?;
    let mut input = child.stdin.take().context("Pi stdin unavailable")?;
    let output = child.stdout.take().context("Pi stdout unavailable")?;
    let mut output = tokio::io::BufReader::new(output.take(2 * 1024 * 1024));
    let requests = [
        serde_json::json!({"id":"state-before","type":"get_state"}),
        serde_json::json!({"id":"set-name","type":"set_session_name","name":request.title}),
        serde_json::json!({"id":"state-after","type":"get_state"}),
    ];
    let exchange = async {
        for rpc_request in &requests {
            let mut bytes = serde_json::to_vec(rpc_request)?;
            bytes.push(b'\n');
            input.write_all(&bytes).await?;
            input.flush().await?;
            loop {
                let mut line = String::new();
                ensure!(
                    output.read_line(&mut line).await? > 0,
                    "Pi RPC ended before responding to {}",
                    rpc_request["id"].as_str().unwrap_or("request")
                );
                let value: Value =
                    serde_json::from_str(&line).context("invalid Pi RPC response")?;
                if value.get("id").and_then(Value::as_str)
                    != rpc_request.get("id").and_then(Value::as_str)
                {
                    continue;
                }
                ensure!(
                    value.get("type").and_then(Value::as_str) == Some("response"),
                    "Pi RPC returned a non-response for a correlated request"
                );
                if value.get("success").and_then(Value::as_bool) != Some(true) {
                    bail!(
                        "Pi RPC rejected {}: {}",
                        rpc_request["type"].as_str().unwrap_or("request"),
                        value
                            .get("error")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown error")
                    );
                }
                let command_name = rpc_request["type"].as_str().unwrap_or_default();
                ensure!(
                    value.get("command").and_then(Value::as_str) == Some(command_name),
                    "Pi RPC response command mismatch"
                );
                if command_name == "get_state" {
                    let data = value.get("data").context("Pi state response has no data")?;
                    ensure!(
                        data.get("sessionId").and_then(Value::as_str) == Some(request.expected_id),
                        "Pi RPC session identity mismatch"
                    );
                    let response_path = data
                        .get("sessionFile")
                        .and_then(Value::as_str)
                        .context("Pi RPC session file missing")?;
                    ensure!(
                        Path::new(response_path).canonicalize()? == expected_session,
                        "Pi RPC opened a different session file"
                    );
                    if rpc_request["id"] == "state-after" {
                        ensure!(
                            data.get("sessionName").and_then(Value::as_str) == Some(request.title),
                            "Pi RPC did not read back the requested title"
                        );
                    }
                }
                break;
            }
        }
        Ok::<_, anyhow::Error>(())
    };
    let result = match tokio::time::timeout(timeout, exchange).await {
        Ok(result) => result,
        Err(_) => Err(anyhow::anyhow!(
            "Pi rename timed out; resulting state is unknown, refresh before retrying"
        )),
    };
    drop(input);
    if tokio::time::timeout(Duration::from_secs(2), child.wait())
        .await
        .is_err()
    {
        let _ = child.start_kill();
        let _ = child.wait().await;
    }
    result
}

fn augmented_path(executable: &Path) -> Option<std::ffi::OsString> {
    let mut paths = Vec::new();
    if let Some(parent) = executable.parent() {
        paths.push(parent.to_path_buf());
    }
    if let Some(path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&path));
    }
    paths.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    env::join_paths(paths).ok()
}
