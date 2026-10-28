//! Antigravity App native session discovery, reading, and mutations.
//! Uses live Connect-RPC when the local Antigravity `language_server` is running,
//! and seamlessly falls back to `~/.gemini/antigravity` SQLite / prototext / transcript files offline.
use anyhow::{bail, ensure, Context, Result};
use rusqlite::{params, Connection, OpenFlags};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{
    provider_sessions::{
        matches_query, paginate_provider_detail, ProviderDetailRequest, ProviderHistoryEvent,
        ProviderListRequest, ProviderSessionDetail, ProviderSessionSummary,
    },
    services::sessions::{SessionBatchItem, SessionBatchItemResult, SessionOperation},
    session_delete::{self, Outcome},
    session_storage::{self, SessionStorageSize, StorageScope},
};

const SERVICE_PREFIX: &str = "/exa.language_server_pb.LanguageServerService/";
const RESPONSE_LIMIT: usize = crate::resource_limits::REMOTE_RESPONSE_BYTES;
const TRANSCRIPT_LINE_LIMIT: usize = 8 * 1024 * 1024;
const MAX_TITLE_CHARS: usize = 200;

#[derive(Debug, Clone)]
pub struct AntigravityPaths {
    pub home: PathBuf,
    /// Explicit endpoint mode used by isolated tests so tests never probe real host processes.
    pub rpc_mode: RpcMode,
}

#[derive(Debug, Clone)]
pub enum RpcMode {
    AutoDiscover,
    OfflineOnly,
    Fixed { port: u16, token: String },
}

impl AntigravityPaths {
    pub fn default_for_user() -> Self {
        let home = env::var_os("ANTIGRAVITY_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(".gemini/antigravity")
            });
        Self {
            home,
            rpc_mode: RpcMode::AutoDiscover,
        }
    }

    pub fn offline(home: PathBuf) -> Self {
        Self {
            home,
            rpc_mode: RpcMode::OfflineOnly,
        }
    }

    pub fn with_fixed_rpc(home: PathBuf, port: u16, token: impl Into<String>) -> Self {
        Self {
            home,
            rpc_mode: RpcMode::Fixed {
                port,
                token: token.into(),
            },
        }
    }

    pub fn available(&self) -> bool {
        self.home.is_dir() || connect_endpoint(self).is_some()
    }

    pub fn summaries_db(&self) -> PathBuf {
        self.home.join("conversation_summaries.db")
    }

    pub fn conversations_dir(&self) -> PathBuf {
        self.home.join("conversations")
    }

    pub fn annotations_dir(&self) -> PathBuf {
        self.home.join("annotations")
    }

    pub fn brain_dir(&self) -> PathBuf {
        self.home.join("brain")
    }

    pub fn presence_dir(&self) -> PathBuf {
        self.home.join("presence")
    }

    pub fn proto_cache(&self) -> PathBuf {
        self.home.join("agyhub_summaries_proto.pb")
    }

    pub fn canonical_path(&self, id: &str) -> PathBuf {
        let db = self.conversations_dir().join(format!("{id}.db"));
        if db.exists() {
            return db;
        }
        let pb = self.conversations_dir().join(format!("{id}.pb"));
        if pb.exists() {
            return pb;
        }
        let brain = self.brain_dir().join(id);
        if brain.exists() {
            return brain;
        }
        db
    }
}

pub fn valid_cascade_id(id: &str) -> Result<()> {
    ensure!(
        uuid::Uuid::parse_str(id).is_ok(),
        "无效的 Antigravity 会话 ID"
    );
    Ok(())
}

#[derive(Clone)]
struct LiveRpc {
    port: u16,
    token: String,
}

fn discover_host_endpoints() -> Vec<(Vec<u16>, String)> {
    #[cfg(unix)]
    {
        use std::process::Command;
        let Ok(output) = Command::new("/bin/ps")
            .args(["-ww", "-axo", "pid=,uid=,command="])
            .output()
        else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }
        let uid = unsafe { libc::getuid() };
        let text = String::from_utf8_lossy(&output.stdout);
        let mut candidates = Vec::new();
        for line in text.lines() {
            let mut parts = line.split_whitespace();
            let Some(pid) = parts.next().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            let Some(owner) = parts.next().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            if owner != uid {
                continue;
            }
            let args: Vec<&str> = parts.collect();
            let Some(&exe) = args.first() else {
                continue;
            };
            let is_antigravity = exe
                .ends_with("/Antigravity.app/Contents/Resources/bin/language_server")
                || (exe.ends_with("/language_server")
                    && args
                        .windows(2)
                        .any(|w| w[0] == "--app_data_dir" && w[1] == "antigravity"));
            if !is_antigravity {
                continue;
            }
            let Some(token) = args
                .windows(2)
                .find(|w| w[0] == "--csrf_token")
                .map(|w| w[1])
            else {
                continue;
            };
            if token.is_empty()
                || token.len() > 256
                || !token
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
            {
                continue;
            }
            let lsof_bin = if Path::new("/usr/sbin/lsof").is_file() {
                "/usr/sbin/lsof"
            } else {
                "lsof"
            };
            let Ok(lsof) = Command::new(lsof_bin)
                .args([
                    "-Pan",
                    "-p",
                    &pid.to_string(),
                    "-iTCP",
                    "-sTCP:LISTEN",
                    "-Fn",
                ])
                .output()
            else {
                continue;
            };
            if !lsof.status.success() {
                continue;
            }
            let mut ports: Vec<u16> = String::from_utf8_lossy(&lsof.stdout)
                .lines()
                .filter_map(|line| {
                    line.strip_prefix("n127.0.0.1:")
                        .and_then(|port| port.parse::<u16>().ok())
                })
                .collect();
            ports.sort_unstable();
            ports.dedup();
            if !ports.is_empty() && ports.len() <= 8 {
                candidates.push((ports, token.to_owned()));
            }
        }
        candidates
    }
    #[cfg(not(unix))]
    {
        Vec::new()
    }
}

fn connect_rpc_post(port: u16, token: &str, method: &str, body: &Value) -> Result<Value> {
    ensure!(
        matches!(
            method,
            "GetAllCascadeTrajectories"
                | "GetCascadeTrajectory"
                | "GetCascadeTrajectorySteps"
                | "UpdateConversationAnnotations"
                | "ForceStopCascadeTree"
                | "DeleteCascadeTrajectory"
        ),
        "不支持的 Antigravity RPC 方法"
    );
    let payload = serde_json::to_vec(body)?;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(1500))
        .context("无法连接本机 Antigravity 服务")?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let request = format!(
        "POST {SERVICE_PREFIX}{method} HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Content-Type: application/json\r\n\
         Accept: application/json\r\n\
         Accept-Encoding: identity\r\n\
         Connect-Protocol-Version: 1\r\n\
         x-codeium-csrf-token: {token}\r\n\
         Connection: close\r\n\
         Content-Length: {}\r\n\r\n",
        payload.len()
    );
    stream.write_all(request.as_bytes())?;
    stream.write_all(&payload)?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut status_line = String::new();
    reader.read_line(&mut status_line)?;
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .context("Antigravity HTTP 响应头无效")?;
    let mut content_length: Option<usize> = None;
    let mut chunked = false;
    loop {
        let mut header = String::new();
        let read = reader.read_line(&mut header)?;
        if read == 0 || header == "\r\n" || header == "\n" {
            break;
        }
        if let Some((name, val)) = header.split_once(':') {
            let name = name.trim();
            let val = val.trim();
            if name.eq_ignore_ascii_case("content-length") {
                let len = val.parse::<usize>().context("无效的 Content-Length")?;
                ensure!(len <= RESPONSE_LIMIT, "Antigravity 响应超过读取上限");
                content_length = Some(len);
            } else if name.eq_ignore_ascii_case("transfer-encoding")
                && val.to_ascii_lowercase().contains("chunked")
            {
                chunked = true;
            }
        }
    }
    let body_bytes = if chunked {
        read_chunked_body(&mut reader, RESPONSE_LIMIT)?
    } else if let Some(len) = content_length {
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;
        buf
    } else {
        let mut buf = Vec::new();
        reader
            .take(RESPONSE_LIMIT as u64 + 1)
            .read_to_end(&mut buf)?;
        ensure!(buf.len() <= RESPONSE_LIMIT, "Antigravity 响应超过读取上限");
        buf
    };
    if !(200..300).contains(&status_code) {
        let detail = serde_json::from_slice::<Value>(&body_bytes)
            .ok()
            .and_then(|v| v["message"].as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("HTTP {status_code}"));
        bail!("Antigravity RPC 调用失败：{detail}");
    }
    if body_bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(json!({}));
    }
    serde_json::from_slice(&body_bytes).context("Antigravity 返回了无效 JSON")
}

fn read_chunked_body<R: BufRead>(reader: &mut R, limit: usize) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let mut size_line = String::new();
        ensure!(reader.read_line(&mut size_line)? > 0, "分块响应意外结束");
        let hex_part = size_line.trim().split(';').next().unwrap_or("").trim();
        let chunk_size = usize::from_str_radix(hex_part, 16).context("无效的分块大小")?;
        if chunk_size == 0 {
            break;
        }
        ensure!(
            out.len().saturating_add(chunk_size) <= limit,
            "Antigravity 响应超过读取上限"
        );
        let offset = out.len();
        out.resize(offset + chunk_size, 0);
        reader.read_exact(&mut out[offset..])?;
        let mut crlf = [0u8; 2];
        reader.read_exact(&mut crlf)?;
    }
    Ok(out)
}

fn connect_endpoint(paths: &AntigravityPaths) -> Option<(LiveRpc, Value)> {
    match &paths.rpc_mode {
        RpcMode::OfflineOnly => None,
        RpcMode::Fixed { port, token } => {
            let value = connect_rpc_post(
                *port,
                token,
                "GetAllCascadeTrajectories",
                &json!({"excludeSubtrajectories": true}),
            )
            .ok()?;
            value["trajectorySummaries"].is_object().then_some((
                LiveRpc {
                    port: *port,
                    token: token.clone(),
                },
                value,
            ))
        }
        RpcMode::AutoDiscover => {
            for (ports, token) in discover_host_endpoints() {
                for port in ports {
                    if let Ok(value) = connect_rpc_post(
                        port,
                        &token,
                        "GetAllCascadeTrajectories",
                        &json!({"excludeSubtrajectories": true}),
                    ) {
                        if value["trajectorySummaries"].is_object() {
                            return Some((LiveRpc { port, token }, value));
                        }
                    }
                }
            }
            None
        }
    }
}

#[derive(Debug, Default, Clone)]
struct AnnotationOverlay {
    title: Option<String>,
    archived: Option<bool>,
}

fn unescape_prototext_string(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' || i + 1 >= bytes.len() {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        let next = bytes[i + 1];
        match next {
            b'n' => {
                out.push(b'\n');
                i += 2;
            }
            b'r' => {
                out.push(b'\r');
                i += 2;
            }
            b't' => {
                out.push(b'\t');
                i += 2;
            }
            b'\\' => {
                out.push(b'\\');
                i += 2;
            }
            b'"' => {
                out.push(b'"');
                i += 2;
            }
            b'\'' => {
                out.push(b'\'');
                i += 2;
            }
            b'0'..=b'7' => {
                let mut val = (next - b'0') as u32;
                let mut consumed = 2;
                for offset in 2..=3 {
                    if i + offset < bytes.len() && (b'0'..=b'7').contains(&bytes[i + offset]) {
                        val = val * 8 + (bytes[i + offset] - b'0') as u32;
                        consumed += 1;
                    } else {
                        break;
                    }
                }
                out.push((val & 0xff) as u8);
                i += consumed;
            }
            b'x' | b'X' if i + 3 < bytes.len() => {
                if let Ok(hex_byte) =
                    u8::from_str_radix(std::str::from_utf8(&bytes[i + 2..i + 4]).unwrap_or(""), 16)
                {
                    out.push(hex_byte);
                    i += 4;
                } else {
                    out.push(next);
                    i += 2;
                }
            }
            _ => {
                out.push(next);
                i += 2;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn escape_prototext_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    for &byte in value.as_bytes() {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(byte as char),
            _ => out.push_str(&format!("\\{:03o}", byte)),
        }
    }
    out
}

fn parse_pbtxt_annotations(content: &str) -> AnnotationOverlay {
    let mut overlay = AnnotationOverlay::default();
    let mut depth = 0usize;
    for line in content.lines() {
        let trimmed = line.trim();
        if depth == 0 {
            if let Some(rest) = trimmed.strip_prefix("title:") {
                let rest = rest.trim();
                if rest.len() >= 2 && rest.starts_with('"') && rest.ends_with('"') {
                    let parsed = unescape_prototext_string(&rest[1..rest.len() - 1]);
                    if !parsed.trim().is_empty() {
                        overlay.title = Some(parsed);
                    }
                }
            } else if let Some(rest) = trimmed.strip_prefix("archived:") {
                match rest.trim() {
                    "true" => overlay.archived = Some(true),
                    "false" => overlay.archived = Some(false),
                    _ => {}
                }
            }
        }
        for ch in trimmed.chars() {
            if ch == '{' {
                depth = depth.saturating_add(1);
            } else if ch == '}' {
                depth = depth.saturating_sub(1);
            }
        }
    }
    overlay
}

fn read_annotation_overlay(paths: &AntigravityPaths, id: &str) -> AnnotationOverlay {
    let path = paths.annotations_dir().join(format!("{id}.pbtxt"));
    fs::read_to_string(path)
        .map(|text| parse_pbtxt_annotations(&text))
        .unwrap_or_default()
}

fn write_annotation_overlay(
    paths: &AntigravityPaths,
    id: &str,
    new_title: Option<&str>,
    new_archived: Option<bool>,
) -> Result<()> {
    fs::create_dir_all(paths.annotations_dir())?;
    let path = paths.annotations_dir().join(format!("{id}.pbtxt"));
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let current = parse_pbtxt_annotations(&existing);
    let title = new_title.map(str::to_owned).or(current.title);
    let archived = new_archived.or(current.archived);

    let mut preserved = Vec::new();
    let mut skip_block_depth = 0usize;
    for line in existing.lines() {
        let trimmed = line.trim();
        if skip_block_depth > 0 {
            for ch in trimmed.chars() {
                if ch == '{' {
                    skip_block_depth += 1;
                } else if ch == '}' {
                    skip_block_depth = skip_block_depth.saturating_sub(1);
                }
            }
            continue;
        }
        if trimmed.starts_with("title:") || trimmed.starts_with("archived:") {
            continue;
        }
        if trimmed.starts_with("archival_status_timestamp") && new_archived.is_some() {
            let opens = trimmed.chars().filter(|&c| c == '{').count();
            let closes = trimmed.chars().filter(|&c| c == '}').count();
            skip_block_depth = opens.saturating_sub(closes);
            continue;
        }
        if !trimmed.is_empty() {
            preserved.push(line.to_owned());
        }
    }

    let mut output = String::new();
    if let Some(title) = title.filter(|t| !t.trim().is_empty()) {
        output.push_str(&format!("title: \"{}\"\n", escape_prototext_string(&title)));
    }
    if let Some(flag) = archived {
        output.push_str(&format!(
            "archived: {}\n",
            if flag { "true" } else { "false" }
        ));
        if new_archived.is_some() {
            let secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            output.push_str(&format!(
                "archival_status_timestamp {{\n  seconds: {secs}\n}}\n"
            ));
        }
    }
    for line in preserved {
        output.push_str(&line);
        output.push('\n');
    }

    let tmp = paths
        .annotations_dir()
        .join(format!(".nexushub-ann-{}.tmp", uuid::Uuid::new_v4()));
    {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(output.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(&tmp, &path)?;
    Ok(())
}

fn invalidate_proto_cache(paths: &AntigravityPaths) {
    let cache = paths.proto_cache();
    if cache.exists() {
        let _ = fs::remove_file(cache);
    }
}

fn decode_file_uri(uri: &str) -> String {
    let raw = uri.trim();
    let stripped = raw.strip_prefix("file://").unwrap_or(raw);
    let bytes = stripped.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
            {
                out.push(hex);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn attach_storage_size(paths: &AntigravityPaths, summary: &mut ProviderSessionSummary) {
    if !paths.home.is_dir() {
        return;
    }
    let brain = paths.brain_dir().join(&summary.id);
    let primary = paths.conversations_dir().join(format!("{}.db", summary.id));
    let legacy_pb = paths.conversations_dir().join(format!("{}.pb", summary.id));
    let primary_file = if primary.is_file() {
        Some(primary)
    } else if legacy_pb.is_file() {
        Some(legacy_pb)
    } else {
        None
    };
    let size = if brain.is_dir() {
        session_storage::directory_size(
            &paths.home,
            &brain,
            primary_file.as_deref(),
            &format!("antigravity:{}", summary.id),
        )
    } else if let Some(file) = &primary_file {
        session_storage::file_size(&paths.home, file)
    } else {
        SessionStorageSize::unavailable(StorageScope::File)
    };
    summary.storage_size = Some(size);
}

fn summary_from_live_json(
    paths: &AntigravityPaths,
    id: &str,
    item: &Value,
) -> Option<ProviderSessionSummary> {
    if uuid::Uuid::parse_str(id).is_err() {
        return None;
    }
    let parent = item["parentConversationId"]
        .as_str()
        .or_else(|| item["trajectoryMetadata"]["parentConversationId"].as_str())
        .unwrap_or("");
    let depth = item["nestingDepth"]
        .as_i64()
        .or_else(|| item["trajectoryMetadata"]["nestingDepth"].as_i64())
        .unwrap_or(0);
    let root = item["trajectoryMetadata"]["rootConversationId"]
        .as_str()
        .unwrap_or("");
    if !parent.is_empty() || depth > 0 || (!root.is_empty() && root != id) {
        return None;
    }
    let overlay = read_annotation_overlay(paths, id);
    let ann_title = item["annotations"]["title"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .or(overlay.title);
    let fallback_summary = item["summary"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let title = ann_title
        .or_else(|| fallback_summary.clone())
        .unwrap_or_else(|| "未命名会话".into());
    let archived = item["annotations"]
        .get("archived")
        .and_then(Value::as_bool)
        .or(overlay.archived)
        .unwrap_or(false);
    let cwd = item["workspaces"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|ws| ws["workspaceFolderAbsoluteUri"].as_str())
        .or_else(|| {
            item["trajectoryMetadata"]["workspaceUris"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(Value::as_str)
        })
        .map(decode_file_uri)
        .unwrap_or_default();
    let updated_at = item["lastModifiedTime"]
        .as_str()
        .or_else(|| item["createdTime"].as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let step_count = item["stepCount"].as_u64().unwrap_or(0) as usize;
    let waiting = item["waitingSteps"]
        .as_array()
        .is_some_and(|arr| !arr.is_empty());
    let run_status = item["status"].as_str().unwrap_or("");
    let running = !archived
        && (waiting
            || matches!(
                run_status,
                "CASCADE_RUN_STATUS_RUNNING" | "CASCADE_RUN_STATUS_CANCELING"
            )
            || item["notFullyIdle"].as_bool() == Some(true)
            || item["hasRunningSubagents"].as_bool() == Some(true));
    let status = if archived {
        "archived"
    } else if running {
        "running"
    } else {
        "idle"
    };
    let mut summary = ProviderSessionSummary {
        storage_size: None,
        id: id.to_owned(),
        session_key: id.to_owned(),
        title,
        cwd,
        path: paths.canonical_path(id),
        archived,
        updated_at,
        message_count: step_count,
        last_message: fallback_summary,
        status: status.into(),
        format_version: "cascade-v2".into(),
        can_rename: true,
        rename_block_reason: None,
        can_archive: true,
        archive_block_reason: None,
        can_delete: !running,
        delete_block_reason: running
            .then(|| "会话正在运行，请先在 Antigravity 中停止后再删除".into()),
        read_error: None,
        read_warning: None,
    };
    attach_storage_size(paths, &mut summary);
    Some(summary)
}

fn list_offline_summaries(paths: &AntigravityPaths) -> Result<Vec<ProviderSessionSummary>> {
    if !paths.home.is_dir() {
        return Ok(Vec::new());
    }
    let mut by_id: BTreeMap<String, ProviderSessionSummary> = BTreeMap::new();
    let db_path = paths.summaries_db();
    if db_path.is_file() {
        if let Ok(conn) = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            let mut fts_archived: BTreeMap<String, bool> = BTreeMap::new();
            if let Ok(mut stmt) = conn.prepare(
                "SELECT conversation_id, archived FROM conversations_fts WHERE chunk_index = -1",
            ) {
                if let Ok(rows) = stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, ValueOrString>(1).ok(),
                    ))
                }) {
                    for row in rows.flatten() {
                        if let Some(flag) = row.1 {
                            fts_archived.insert(row.0, flag.0);
                        }
                    }
                }
            }
            if let Ok(mut stmt) = conn.prepare(
                "SELECT conversation_id, COALESCE(title, ''), COALESCE(preview, ''), \
                 COALESCE(step_count, 0), COALESCE(last_modified_time, ''), \
                 COALESCE(workspace_uris, ''), COALESCE(parent_conversation_id, ''), \
                 COALESCE(nesting_depth, 0) \
                 FROM conversation_summaries",
            ) {
                if let Ok(rows) = stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                    ))
                }) {
                    for row in rows.flatten() {
                        let (id, db_title, preview, steps, modified, ws_raw, parent, depth) = row;
                        if uuid::Uuid::parse_str(&id).is_err() || !parent.is_empty() || depth > 0 {
                            continue;
                        }
                        let overlay = read_annotation_overlay(paths, &id);
                        let archived = overlay
                            .archived
                            .or_else(|| fts_archived.get(&id).copied())
                            .unwrap_or(false);
                        let title = overlay
                            .title
                            .filter(|s| !s.trim().is_empty())
                            .or_else(|| {
                                let t = db_title.trim();
                                (!t.is_empty()).then(|| t.to_owned())
                            })
                            .or_else(|| {
                                let p = preview.trim();
                                (!p.is_empty()).then(|| p.chars().take(80).collect())
                            })
                            .unwrap_or_else(|| "未命名会话".into());
                        let cwd = serde_json::from_str::<Vec<String>>(&ws_raw)
                            .ok()
                            .and_then(|uris| uris.into_iter().next())
                            .map(|uri| decode_file_uri(&uri))
                            .unwrap_or_default();
                        let mut summary = ProviderSessionSummary {
                            storage_size: None,
                            id: id.clone(),
                            session_key: id.clone(),
                            title,
                            cwd,
                            path: paths.canonical_path(&id),
                            archived,
                            updated_at: (!modified.trim().is_empty()).then_some(modified),
                            message_count: steps.max(0) as usize,
                            last_message: (!preview.trim().is_empty()).then_some(preview),
                            status: if archived { "archived" } else { "idle" }.into(),
                            format_version: "sqlite-offline".into(),
                            can_rename: true,
                            rename_block_reason: None,
                            can_archive: true,
                            archive_block_reason: None,
                            can_delete: true,
                            delete_block_reason: None,
                            read_error: None,
                            read_warning: None,
                        };
                        attach_storage_size(paths, &mut summary);
                        by_id.insert(id, summary);
                    }
                }
            }
        }
    }

    // Fallback scan of brain/<uuid> when conversation_summaries.db is absent or incomplete
    if by_id.is_empty() && paths.brain_dir().is_dir() {
        if let Ok(entries) = fs::read_dir(paths.brain_dir()) {
            for entry in entries.flatten() {
                let id = entry.file_name().to_string_lossy().into_owned();
                if uuid::Uuid::parse_str(&id).is_err() || by_id.contains_key(&id) {
                    continue;
                }
                let overlay = read_annotation_overlay(paths, &id);
                let archived = overlay.archived.unwrap_or(false);
                let updated_at = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|time| chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339());
                let mut summary = ProviderSessionSummary {
                    storage_size: None,
                    id: id.clone(),
                    session_key: id.clone(),
                    title: overlay.title.unwrap_or_else(|| id.clone()),
                    cwd: String::new(),
                    path: paths.canonical_path(&id),
                    archived,
                    updated_at,
                    message_count: 0,
                    last_message: None,
                    status: if archived { "archived" } else { "idle" }.into(),
                    format_version: "brain-offline".into(),
                    can_rename: true,
                    rename_block_reason: None,
                    can_archive: true,
                    archive_block_reason: None,
                    can_delete: true,
                    delete_block_reason: None,
                    read_error: None,
                    read_warning: None,
                };
                attach_storage_size(paths, &mut summary);
                by_id.insert(id, summary);
            }
        }
    }

    Ok(by_id.into_values().collect())
}

struct ValueOrString(bool);
impl rusqlite::types::FromSql for ValueOrString {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value {
            rusqlite::types::ValueRef::Integer(i) => Ok(Self(i != 0)),
            rusqlite::types::ValueRef::Text(bytes) => {
                let s = std::str::from_utf8(bytes).unwrap_or("");
                Ok(Self(s == "1" || s.eq_ignore_ascii_case("true")))
            }
            _ => Ok(Self(false)),
        }
    }
}

fn list_all_summaries(paths: &AntigravityPaths) -> Result<Vec<ProviderSessionSummary>> {
    let mut items = if let Some((_, payload)) = connect_endpoint(paths) {
        payload["trajectorySummaries"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(id, item)| summary_from_live_json(paths, id, item))
            .collect::<Vec<_>>()
    } else {
        list_offline_summaries(paths)?
    };
    items.sort_by(|a, b| {
        b.updated_at
            .as_deref()
            .unwrap_or("")
            .cmp(a.updated_at.as_deref().unwrap_or(""))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(items)
}

pub fn list_sessions(
    paths: &AntigravityPaths,
    request: &ProviderListRequest,
) -> Result<Vec<ProviderSessionSummary>> {
    let limit = request.limit.unwrap_or(200).clamp(1, 1000);
    let all = list_all_summaries(paths)?;
    Ok(all
        .into_iter()
        .filter(|item| match request.archived {
            Some(flag) => item.archived == flag,
            None => true,
        })
        .filter(|item| matches_query(item, request.q.as_deref()))
        .take(limit)
        .collect())
}

pub fn session_summary(paths: &AntigravityPaths, key: &str) -> Result<ProviderSessionSummary> {
    valid_cascade_id(key)?;
    list_all_summaries(paths)?
        .into_iter()
        .find(|item| item.id == key)
        .with_context(|| format!("未找到 Antigravity 会话：{key}"))
}

fn map_cortex_status(status: &str) -> &'static str {
    match status {
        "CORTEX_STEP_STATUS_RUNNING"
        | "CORTEX_STEP_STATUS_WAITING"
        | "CORTEX_STEP_STATUS_PENDING" => "running",
        "CORTEX_STEP_STATUS_ERROR"
        | "CORTEX_STEP_STATUS_CANCELED"
        | "CORTEX_STEP_STATUS_INTERRUPTED"
        | "CORTEX_STEP_STATUS_HALTED" => "failed",
        _ => "completed",
    }
}

fn extract_live_step_events(id: &str, steps: &[Value]) -> Vec<ProviderHistoryEvent> {
    let mut events = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        let source = &step["metadata"]["sourceTrajectoryStepInfo"];
        if source["cascadeId"]
            .as_str()
            .is_some_and(|orig| !orig.is_empty() && orig != id)
        {
            continue;
        }
        let step_type = step["type"].as_str().unwrap_or("");
        let timestamp = step["metadata"]["createdAt"]
            .as_str()
            .or_else(|| step["metadata"]["completedAt"].as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        let turn_id = step["metadata"]["executionId"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        let status = map_cortex_status(step["status"].as_str().unwrap_or(""));

        match step_type {
            "CORTEX_STEP_TYPE_USER_INPUT" => {
                let mut parts = Vec::new();
                if let Some(items) = step["userInput"]["items"].as_array() {
                    for item in items {
                        if let Some(text) = item["text"].as_str().filter(|t| !t.is_empty()) {
                            parts.push(json!({"type": "text", "text": text}));
                        }
                    }
                }
                if parts.is_empty() {
                    if let Some(text) = step["userInput"]["userResponse"]
                        .as_str()
                        .or_else(|| step["userInput"]["query"].as_str())
                        .filter(|t| !t.trim().is_empty())
                    {
                        parts.push(json!({"type": "text", "text": text}));
                    }
                }
                if let Some(media) = step["userInput"]["media"].as_array() {
                    for item in media {
                        if let Some(uri) =
                            item["uri"].as_str().or_else(|| item["inlineData"].as_str())
                        {
                            let path = decode_file_uri(uri);
                            parts.push(
                                json!({"type": "text", "text": format!("<image path=\"{path}\">")}),
                            );
                            parts.push(json!({
                                "type": "image",
                                "url": path,
                                "mimeType": item["mimeType"].as_str().unwrap_or("image/png")
                            }));
                            parts.push(json!({"type": "text", "text": "</image>"}));
                        }
                    }
                }
                if parts.is_empty() {
                    continue;
                }
                let event_id = format!("step-{index}-user");
                let user_msg =
                    crate::user_message::parse_user_message(&event_id, &Value::Array(parts));
                if user_msg.text.trim().is_empty() && user_msg.attachments.is_empty() {
                    continue;
                }
                events.push(ProviderHistoryEvent {
                    id: event_id,
                    timestamp,
                    kind: "user_message".into(),
                    role: Some("user".into()),
                    text: Some(user_msg.text.clone()),
                    call_id: None,
                    turn_id,
                    status: Some(status.into()),
                    detail: None,
                    result: None,
                    user_message: Some(user_msg),
                });
            }
            "CORTEX_STEP_TYPE_PLANNER_RESPONSE" => {
                let pr = &step["plannerResponse"];
                if let Some(thinking) = pr["thinking"].as_str().filter(|s| !s.trim().is_empty()) {
                    events.push(ProviderHistoryEvent {
                        id: format!("step-{index}-thinking"),
                        timestamp: timestamp.clone(),
                        kind: "thinking".into(),
                        role: Some("assistant".into()),
                        text: Some(thinking.to_owned()),
                        call_id: None,
                        turn_id: turn_id.clone(),
                        status: Some(status.into()),
                        detail: None,
                        result: None,
                        user_message: None,
                    });
                }
                if let Some(calls) = pr["toolCalls"].as_array() {
                    for (ci, call) in calls.iter().enumerate() {
                        let name = call["name"].as_str().unwrap_or("tool");
                        let args = if call["argumentsJson"].is_string() {
                            call["argumentsJson"].as_str().unwrap_or("").to_owned()
                        } else if !call["arguments"].is_null() {
                            serde_json::to_string_pretty(&call["arguments"]).unwrap_or_default()
                        } else {
                            String::new()
                        };
                        let call_id = call["id"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .map(str::to_owned)
                            .unwrap_or_else(|| format!("step-{index}-call-{ci}"));
                        events.push(ProviderHistoryEvent {
                            id: format!("step-{index}-toolcall-{ci}"),
                            timestamp: timestamp.clone(),
                            kind: "tool_call".into(),
                            role: Some(name.to_owned()),
                            text: Some(name.to_owned()),
                            call_id: Some(call_id),
                            turn_id: turn_id.clone(),
                            status: Some(status.into()),
                            detail: (!args.trim().is_empty()).then_some(args),
                            result: None,
                            user_message: None,
                        });
                    }
                }
                let reply = pr["modifiedResponse"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| pr["response"].as_str().filter(|s| !s.trim().is_empty()));
                if let Some(text) = reply {
                    events.push(ProviderHistoryEvent {
                        id: format!("step-{index}-assistant"),
                        timestamp,
                        kind: "assistant_message".into(),
                        role: Some("assistant".into()),
                        text: Some(text.to_owned()),
                        call_id: None,
                        turn_id,
                        status: Some(status.into()),
                        detail: None,
                        result: None,
                        user_message: None,
                    });
                }
            }
            "CORTEX_STEP_TYPE_FINISH"
            | "CORTEX_STEP_TYPE_EPHEMERAL_MESSAGE"
            | "CORTEX_STEP_TYPE_CHECKPOINT"
            | "" => {}
            other => {
                let tool_name = step["metadata"]["toolCall"]["name"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        other
                            .strip_prefix("CORTEX_STEP_TYPE_")
                            .unwrap_or(other)
                            .to_ascii_lowercase()
                    });
                let call_id = step["metadata"]["toolCall"]["id"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned);
                let (detail, result) = summarize_cortex_tool_step(step);
                if detail.is_none() && result.is_none() && call_id.is_none() {
                    continue;
                }
                let kind = if call_id.is_some() {
                    "tool_result"
                } else {
                    "tool_call"
                };
                events.push(ProviderHistoryEvent {
                    id: format!("step-{index}-tool"),
                    timestamp,
                    kind: kind.into(),
                    role: Some(tool_name.clone()),
                    text: Some(tool_name),
                    call_id,
                    turn_id,
                    status: Some(status.into()),
                    detail,
                    result,
                    user_message: None,
                });
            }
        }
    }
    events
}

fn summarize_cortex_tool_step(step: &Value) -> (Option<String>, Option<String>) {
    if let Some(cmd) = step.get("runCommand").and_then(Value::as_object) {
        let command_line = cmd
            .get("commandLine")
            .or_else(|| cmd.get("command"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let output = cmd
            .get("combinedOutput")
            .and_then(|v| v.as_str().or_else(|| v["full"].as_str()))
            .or_else(|| cmd.get("stdout").and_then(Value::as_str))
            .filter(|s| !s.trim().is_empty())
            .map(str::to_owned);
        return (command_line, output);
    }
    if let Some(vf) = step.get("viewFile").and_then(Value::as_object) {
        let uri = vf
            .get("absolutePathUri")
            .and_then(Value::as_str)
            .map(decode_file_uri);
        let content = vf
            .get("content")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(str::to_owned);
        return (uri, content);
    }
    if let Some(ca) = step.get("codeAction").and_then(Value::as_object) {
        let spec = ca
            .get("actionSpec")
            .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
            .filter(|s| !s.trim().is_empty());
        let res = ca
            .get("actionResult")
            .map(|v| serde_json::to_string_pretty(v).unwrap_or_default())
            .filter(|s| !s.trim().is_empty());
        return (spec, res);
    }
    let detail = step["metadata"]["toolCall"]["argumentsJson"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned);
    let result = step
        .as_object()
        .and_then(|obj| {
            obj.iter()
                .find(|(k, _)| {
                    !matches!(k.as_str(), "type" | "status" | "metadata" | "permissions")
                })
                .map(|(_, v)| {
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| serde_json::to_string_pretty(v).unwrap_or_default())
                })
        })
        .filter(|s| !s.trim().is_empty() && s != "{}" && s != "null");
    (detail, result)
}

fn read_offline_transcript_events(
    paths: &AntigravityPaths,
    id: &str,
) -> Result<Vec<ProviderHistoryEvent>> {
    let logs_dir = paths.brain_dir().join(id).join(".system_generated/logs");
    let full = logs_dir.join("transcript_full.jsonl");
    let compact = logs_dir.join("transcript.jsonl");
    let chosen = if full.is_file() {
        Some(full)
    } else if compact.is_file() {
        Some(compact)
    } else {
        None
    };

    if let Some(path) = chosen {
        let file = fs::File::open(&path)?;
        let mut reader = BufReader::new(file);
        let mut line = Vec::new();
        let mut events = Vec::new();
        let mut idx = 0usize;
        loop {
            line.clear();
            let read = (&mut reader)
                .take(TRANSCRIPT_LINE_LIMIT as u64 + 1)
                .read_until(b'\n', &mut line)?;
            if read == 0 {
                break;
            }
            if line.len() > TRANSCRIPT_LINE_LIMIT || line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let Ok(row) = serde_json::from_slice::<Value>(&line) else {
                continue;
            };
            let step_index = row["step_index"].as_u64().unwrap_or(idx as u64);
            idx += 1;
            let step_type = row["type"].as_str().unwrap_or("");
            let source = row["source"].as_str().unwrap_or("");
            let timestamp = row["created_at"].as_str().map(str::to_owned);
            let status_raw = row["status"].as_str().unwrap_or("DONE");
            let status = match status_raw {
                "RUNNING" | "WAITING" | "PENDING" => "running",
                "ERROR" | "FAILED" | "CANCELED" => "failed",
                _ => "completed",
            };

            if step_type == "USER_INPUT" || source == "USER_EXPLICIT" {
                let text = row["content"].as_str().unwrap_or("");
                let mut parts = Vec::new();
                if !text.trim().is_empty() {
                    parts.push(json!({"type": "text", "text": text}));
                }
                if let Some(media) = row["media"].as_array() {
                    for item in media {
                        if let Some(uri) = item["uri"].as_str() {
                            let decoded = decode_file_uri(uri);
                            parts.push(json!({"type": "text", "text": format!("<image path=\"{decoded}\">")}));
                            parts.push(json!({
                                "type": "image",
                                "url": decoded,
                                "mimeType": item["mime_type"].as_str().unwrap_or("image/png")
                            }));
                            parts.push(json!({"type": "text", "text": "</image>"}));
                        }
                    }
                }
                if parts.is_empty() {
                    continue;
                }
                let event_id = format!("transcript-{step_index}-user");
                let user_msg =
                    crate::user_message::parse_user_message(&event_id, &Value::Array(parts));
                events.push(ProviderHistoryEvent {
                    id: event_id,
                    timestamp,
                    kind: "user_message".into(),
                    role: Some("user".into()),
                    text: Some(user_msg.text.clone()),
                    call_id: None,
                    turn_id: None,
                    status: Some(status.into()),
                    detail: None,
                    result: None,
                    user_message: Some(user_msg),
                });
            } else if step_type == "PLANNER_RESPONSE" || source == "MODEL" {
                if let Some(thinking) = row["thinking"].as_str().filter(|s| !s.trim().is_empty()) {
                    events.push(ProviderHistoryEvent {
                        id: format!("transcript-{step_index}-thinking"),
                        timestamp: timestamp.clone(),
                        kind: "thinking".into(),
                        role: Some("assistant".into()),
                        text: Some(thinking.to_owned()),
                        call_id: None,
                        turn_id: None,
                        status: Some(status.into()),
                        detail: None,
                        result: None,
                        user_message: None,
                    });
                }
                if let Some(calls) = row["tool_calls"].as_array() {
                    for (ci, call) in calls.iter().enumerate() {
                        let name = call["name"]
                            .as_str()
                            .or_else(|| call["tool_name"].as_str())
                            .unwrap_or("tool");
                        let args = call
                            .get("arguments")
                            .or_else(|| call.get("args"))
                            .map(|v| {
                                v.as_str().map(str::to_owned).unwrap_or_else(|| {
                                    serde_json::to_string_pretty(v).unwrap_or_default()
                                })
                            })
                            .filter(|s| !s.trim().is_empty());
                        let result = call
                            .get("output")
                            .or_else(|| call.get("result"))
                            .and_then(Value::as_str)
                            .filter(|s| !s.trim().is_empty())
                            .map(str::to_owned);
                        events.push(ProviderHistoryEvent {
                            id: format!("transcript-{step_index}-tool-{ci}"),
                            timestamp: timestamp.clone(),
                            kind: "tool_call".into(),
                            role: Some(name.to_owned()),
                            text: Some(name.to_owned()),
                            call_id: call["id"].as_str().map(str::to_owned),
                            turn_id: None,
                            status: Some(status.into()),
                            detail: args,
                            result,
                            user_message: None,
                        });
                    }
                }
                if let Some(content) = row["content"].as_str().filter(|s| !s.trim().is_empty()) {
                    events.push(ProviderHistoryEvent {
                        id: format!("transcript-{step_index}-assistant"),
                        timestamp,
                        kind: "assistant_message".into(),
                        role: Some("assistant".into()),
                        text: Some(content.to_owned()),
                        call_id: None,
                        turn_id: None,
                        status: Some(status.into()),
                        detail: None,
                        result: None,
                        user_message: None,
                    });
                }
            } else if let Some(content) = row["content"].as_str().filter(|s| !s.trim().is_empty()) {
                let name = if step_type.is_empty() {
                    "tool"
                } else {
                    step_type
                };
                events.push(ProviderHistoryEvent {
                    id: format!("transcript-{step_index}-step"),
                    timestamp,
                    kind: "tool_call".into(),
                    role: Some(name.to_ascii_lowercase()),
                    text: Some(name.to_ascii_lowercase()),
                    call_id: None,
                    turn_id: None,
                    status: Some(status.into()),
                    detail: None,
                    result: Some(content.to_owned()),
                    user_message: None,
                });
            }
        }
        if !events.is_empty() {
            return Ok(events);
        }
    }

    // Secondary offline fallback: extract UTF-8 strings from conversations/<id>.db steps table
    let conv_db = paths.conversations_dir().join(format!("{id}.db"));
    if conv_db.is_file() {
        if let Ok(conn) = Connection::open_with_flags(
            &conv_db,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) {
            if let Ok(mut stmt) =
                conn.prepare("SELECT idx, step_type, step_payload FROM steps ORDER BY idx ASC")
            {
                if let Ok(rows) = stmt.query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                }) {
                    let mut events = Vec::new();
                    for row in rows.flatten() {
                        let (idx, step_type, blob) = row;
                        let strings = extract_protobuf_utf8_strings(&blob);
                        let joined = strings.join("\n").trim().to_owned();
                        if joined.is_empty() {
                            continue;
                        }
                        if step_type == 14 {
                            let event_id = format!("db-{idx}-user");
                            let user_msg = crate::user_message::parse_user_message(
                                &event_id,
                                &Value::String(joined.clone()),
                            );
                            events.push(ProviderHistoryEvent {
                                id: event_id,
                                timestamp: None,
                                kind: "user_message".into(),
                                role: Some("user".into()),
                                text: Some(user_msg.text.clone()),
                                call_id: None,
                                turn_id: None,
                                status: Some("completed".into()),
                                detail: None,
                                result: None,
                                user_message: Some(user_msg),
                            });
                        } else if step_type == 15 {
                            events.push(ProviderHistoryEvent {
                                id: format!("db-{idx}-assistant"),
                                timestamp: None,
                                kind: "assistant_message".into(),
                                role: Some("assistant".into()),
                                text: Some(joined),
                                call_id: None,
                                turn_id: None,
                                status: Some("completed".into()),
                                detail: None,
                                result: None,
                                user_message: None,
                            });
                        }
                    }
                    return Ok(events);
                }
            }
        }
    }

    Ok(Vec::new())
}

fn extract_protobuf_utf8_strings(bytes: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    collect_proto_strings(bytes, 0, &mut out);
    out
}

fn read_varint(bytes: &[u8], pos: &mut usize) -> Option<u64> {
    let mut result = 0u64;
    let mut shift = 0u32;
    while *pos < bytes.len() && shift < 64 {
        let b = bytes[*pos];
        *pos += 1;
        result |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
    }
    None
}

fn collect_proto_strings(bytes: &[u8], depth: usize, out: &mut Vec<String>) {
    if depth > 4 {
        return;
    }
    let mut pos = 0usize;
    while pos < bytes.len() {
        let Some(tag) = read_varint(bytes, &mut pos) else {
            return;
        };
        let wire = tag & 0x07;
        match wire {
            0 => {
                if read_varint(bytes, &mut pos).is_none() {
                    return;
                }
            }
            1 => {
                if pos + 8 > bytes.len() {
                    return;
                }
                pos += 8;
            }
            5 => {
                if pos + 4 > bytes.len() {
                    return;
                }
                pos += 4;
            }
            2 => {
                let Some(len) = read_varint(bytes, &mut pos) else {
                    return;
                };
                let len = len as usize;
                if pos + len > bytes.len() {
                    return;
                }
                let slice = &bytes[pos..pos + len];
                pos += len;
                if let Ok(text) = std::str::from_utf8(slice) {
                    let trimmed = text.trim();
                    if trimmed.len() >= 2
                        && !trimmed
                            .chars()
                            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                        && uuid::Uuid::parse_str(trimmed).is_err()
                    {
                        out.push(trimmed.to_owned());
                        continue;
                    }
                }
                collect_proto_strings(slice, depth + 1, out);
            }
            _ => return,
        }
    }
}

pub fn load_all_events(paths: &AntigravityPaths, id: &str) -> Result<Vec<ProviderHistoryEvent>> {
    valid_cascade_id(id)?;
    if let Some((rpc, _)) = connect_endpoint(paths) {
        if let Ok(resp) = connect_rpc_post(
            rpc.port,
            &rpc.token,
            "GetCascadeTrajectory",
            &json!({"cascadeId": id}),
        ) {
            let mut steps = resp["trajectory"]["steps"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let total = resp["numTotalSteps"].as_u64().unwrap_or(steps.len() as u64) as usize;
            while steps.len() < total && steps.len() < 50_000 {
                let offset = steps.len();
                let Ok(more) = connect_rpc_post(
                    rpc.port,
                    &rpc.token,
                    "GetCascadeTrajectorySteps",
                    &json!({"cascadeId": id, "stepOffset": offset}),
                ) else {
                    break;
                };
                let Some(batch) = more["steps"].as_array().filter(|b| !b.is_empty()) else {
                    break;
                };
                steps.extend(batch.iter().cloned());
            }
            let events = extract_live_step_events(id, &steps);
            if !events.is_empty() || steps.is_empty() {
                return Ok(events);
            }
        }
    }
    read_offline_transcript_events(paths, id)
}

pub fn session_detail(
    paths: &AntigravityPaths,
    request: &ProviderDetailRequest,
) -> Result<ProviderSessionDetail> {
    let summary = session_summary(paths, &request.session_key)?;
    let events = load_all_events(paths, &summary.id)?;
    paginate_provider_detail(summary, &events, request)
}

pub fn read_attachment(
    paths: &AntigravityPaths,
    request: &crate::user_message::SessionAttachmentRequest,
) -> Result<crate::user_message::SessionAttachmentResponse> {
    let summary = session_summary(paths, &request.session_key)?;
    let events = load_all_events(paths, &summary.id)?;
    let mut message = events
        .into_iter()
        .filter_map(|event| event.user_message)
        .find(|msg| msg.id == request.message_id)
        .context("会话消息已变化或不存在，请刷新会话")?;
    message.refresh_files((!summary.cwd.is_empty()).then_some(summary.cwd.as_str()));
    message.read_attachment(&request.attachment_id)
}

pub fn rename_session(
    paths: &AntigravityPaths,
    key: &str,
    raw_title: &str,
) -> Result<ProviderSessionSummary> {
    valid_cascade_id(key)?;
    let title: String = raw_title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect();
    ensure!(!title.is_empty(), "会话名称不能为空");
    let _existing = session_summary(paths, key)?;

    if let Some((rpc, _)) = connect_endpoint(paths) {
        connect_rpc_post(
            rpc.port,
            &rpc.token,
            "UpdateConversationAnnotations",
            &json!({
                "cascadeIds": [key],
                "annotations": { "title": title },
                "mergeAnnotations": true
            }),
        )?;
    } else {
        write_annotation_overlay(paths, key, Some(&title), None)?;
        let db_path = paths.summaries_db();
        if db_path.is_file() {
            if let Ok(conn) = Connection::open(&db_path) {
                let _ = conn.execute(
                    "UPDATE conversation_summaries SET title = ?1 WHERE conversation_id = ?2",
                    params![title, key],
                );
                let _ = conn.execute(
                    "UPDATE conversations_fts SET title = ?1 WHERE conversation_id = ?2 AND chunk_index = -1",
                    params![title, key],
                );
            }
        }
        invalidate_proto_cache(paths);
    }
    session_summary(paths, key)
}

pub fn set_archived(
    paths: &AntigravityPaths,
    key: &str,
    archived: bool,
) -> Result<ProviderSessionSummary> {
    valid_cascade_id(key)?;
    let _existing = session_summary(paths, key)?;

    if let Some((rpc, _)) = connect_endpoint(paths) {
        connect_rpc_post(
            rpc.port,
            &rpc.token,
            "UpdateConversationAnnotations",
            &json!({
                "cascadeIds": [key],
                "annotations": { "archived": archived },
                "mergeAnnotations": true
            }),
        )?;
        if archived {
            let _ = connect_rpc_post(
                rpc.port,
                &rpc.token,
                "ForceStopCascadeTree",
                &json!({ "conversationId": key }),
            );
        }
    } else {
        write_annotation_overlay(paths, key, None, Some(archived))?;
        let db_path = paths.summaries_db();
        if db_path.is_file() {
            if let Ok(conn) = Connection::open(&db_path) {
                let flag = if archived { "1" } else { "0" };
                let _ = conn.execute(
                    "UPDATE conversations_fts SET archived = ?1 WHERE conversation_id = ?2 AND chunk_index = -1",
                    params![flag, key],
                );
            }
        }
        invalidate_proto_cache(paths);
    }
    session_summary(paths, key)
}

fn find_descendant_ids(paths: &AntigravityPaths, root_id: &str) -> Vec<String> {
    let db_path = paths.summaries_db();
    if !db_path.is_file() {
        return Vec::new();
    }
    let Ok(conn) = Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return Vec::new();
    };
    let Ok(mut stmt) = conn.prepare(
        "SELECT conversation_id, COALESCE(parent_conversation_id, '') FROM conversation_summaries",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return Vec::new();
    };
    let mut children_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (cid, pid) in rows.flatten() {
        if !pid.is_empty() && uuid::Uuid::parse_str(&cid).is_ok() {
            children_map.entry(pid).or_default().push(cid);
        }
    }
    let mut result = BTreeSet::new();
    let mut stack = vec![root_id.to_owned()];
    while let Some(curr) = stack.pop() {
        if let Some(kids) = children_map.get(&curr) {
            for kid in kids {
                if kid != root_id && result.insert(kid.clone()) {
                    stack.push(kid.clone());
                }
            }
        }
    }
    result.into_iter().collect()
}

fn owned_disk_paths(paths: &AntigravityPaths, id: &str) -> Vec<PathBuf> {
    let candidates = [
        paths.conversations_dir().join(format!("{id}.db")),
        paths.conversations_dir().join(format!("{id}.db-wal")),
        paths.conversations_dir().join(format!("{id}.db-shm")),
        paths.conversations_dir().join(format!("{id}.pb")),
        paths.annotations_dir().join(format!("{id}.pbtxt")),
        paths.brain_dir().join(id),
        paths.presence_dir().join(format!("{id}.lock")),
    ];
    candidates
        .into_iter()
        .filter(|path| path.exists() && session_delete::within(&paths.home, path))
        .collect()
}

pub fn preview_batch_item(
    paths: &AntigravityPaths,
    key: &str,
    operation: SessionOperation,
    include_descendants: bool,
) -> Result<SessionBatchItem> {
    let summary = session_summary(paths, key)?;
    if operation == SessionOperation::Delete && summary.status == "running" {
        bail!(
            "{}",
            summary
                .delete_block_reason
                .as_deref()
                .unwrap_or("会话正在运行，无法删除")
        );
    }
    let descendants = if operation == SessionOperation::Delete && include_descendants {
        find_descendant_ids(paths, &summary.id)
    } else {
        Vec::new()
    };
    let mut target_paths = owned_disk_paths(paths, &summary.id);
    for child in &descendants {
        target_paths.extend(owned_disk_paths(paths, child));
    }
    if target_paths.is_empty() {
        target_paths.push(summary.path.clone());
    }
    let (_, bytes) = session_delete::size(&target_paths);
    let fingerprint = hex::encode(Sha256::digest(format!(
        "antigravity:{}:{}:{}:{}",
        summary.id,
        summary.archived,
        bytes,
        summary.updated_at.as_deref().unwrap_or("")
    )));
    Ok(SessionBatchItem {
        session_key: summary.session_key,
        id: summary.id,
        title: summary.title,
        paths: target_paths,
        bytes,
        allowed: true,
        reason: None,
        fingerprint: Some(fingerprint),
        descendants,
        warnings: Vec::new(),
    })
}

pub fn execute_batch_item(
    paths: &AntigravityPaths,
    key: &str,
    operation: SessionOperation,
    descendants: &[String],
    include_descendants: bool,
) -> Result<SessionBatchItemResult> {
    valid_cascade_id(key)?;
    match operation {
        SessionOperation::Archive => {
            set_archived(paths, key, true)?;
            Ok(SessionBatchItemResult {
                session_key: key.to_owned(),
                status: "succeeded".into(),
                message: None,
                deleted_ids: Vec::new(),
                residuals: Vec::new(),
            })
        }
        SessionOperation::Restore => {
            set_archived(paths, key, false)?;
            Ok(SessionBatchItemResult {
                session_key: key.to_owned(),
                status: "succeeded".into(),
                message: None,
                deleted_ids: Vec::new(),
                residuals: Vec::new(),
            })
        }
        SessionOperation::Delete => {
            let summary = session_summary(paths, key)?;
            ensure!(
                summary.status != "running",
                "会话正在运行，请先在 Antigravity 中停止后再删除"
            );
            let mut all_ids = vec![key.to_owned()];
            if include_descendants {
                let discovered = if descendants.is_empty() {
                    find_descendant_ids(paths, key)
                } else {
                    descendants.to_vec()
                };
                for child in discovered {
                    valid_cascade_id(&child)?;
                    if !all_ids.contains(&child) {
                        all_ids.push(child);
                    }
                }
            }
            if let Some((rpc, _)) = connect_endpoint(paths) {
                for id in all_ids.iter().rev() {
                    let res = connect_rpc_post(
                        rpc.port,
                        &rpc.token,
                        "DeleteCascadeTrajectory",
                        &json!({ "cascadeId": id }),
                    );
                    if id == key {
                        res?;
                    }
                }
            } else {
                let db_path = paths.summaries_db();
                if db_path.is_file() {
                    let conn = Connection::open(&db_path)
                        .context("无法打开 Antigravity 索引数据库执行删除")?;
                    for id in &all_ids {
                        conn.execute(
                            "DELETE FROM conversation_summaries WHERE conversation_id = ?1",
                            params![id],
                        )?;
                        let _ = conn.execute(
                            "DELETE FROM conversations_fts WHERE conversation_id = ?1",
                            params![id],
                        );
                    }
                }
                invalidate_proto_cache(paths);
            }

            let mut outcome = Outcome::default();
            for id in &all_ids {
                for path in owned_disk_paths(paths, id) {
                    outcome.remove(&path);
                }
            }
            outcome.finish();
            Ok(SessionBatchItemResult {
                session_key: key.to_owned(),
                status: if outcome.residuals.is_empty() {
                    "succeeded"
                } else {
                    "failed"
                }
                .into(),
                message: (!outcome.residuals.is_empty())
                    .then(|| "部分 Antigravity 会话文件未清理，请重试".into()),
                deleted_ids: all_ids,
                residuals: outcome.residuals,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    #[test]
    fn offline_sqlite_pbtxt_and_transcript_support_all_five_actions() {
        let root = env::temp_dir().join(format!("nexushub-ag-offline-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("conversations")).unwrap();
        fs::create_dir_all(root.join("annotations")).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let sub_id = uuid::Uuid::new_v4().to_string();
        let logs_dir = root.join("brain").join(&id).join(".system_generated/logs");
        fs::create_dir_all(&logs_dir).unwrap();
        fs::create_dir_all(root.join("brain").join(&sub_id)).unwrap();
        fs::write(
            root.join("conversations").join(format!("{id}.db")),
            b"sqlite-placeholder",
        )
        .unwrap();
        fs::write(
            root.join("conversations").join(format!("{sub_id}.db")),
            b"sub-placeholder",
        )
        .unwrap();
        fs::write(root.join("agyhub_summaries_proto.pb"), b"cached").unwrap();
        fs::write(
            logs_dir.join("transcript_full.jsonl"),
            concat!(
                r#"{"step_index":0,"source":"USER_EXPLICIT","type":"USER_INPUT","status":"DONE","created_at":"2026-10-28T01:00:00Z","content":"重构前端界面"}"#,
                "\n",
                r#"{"step_index":1,"source":"MODEL","type":"PLANNER_RESPONSE","status":"DONE","created_at":"2026-10-28T01:00:05Z","thinking":"先分析样式系统","tool_calls":[{"name":"view_file","arguments":{"Path":"/tmp/demo.css"}}],"content":"已完成样式重构。"}"#,
                "\n"
            ),
        )
        .unwrap();

        let conn = Connection::open(root.join("conversation_summaries.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE conversation_summaries (
                conversation_id TEXT PRIMARY KEY,
                title TEXT,
                preview TEXT,
                step_count INTEGER,
                last_modified_time TEXT,
                workspace_uris TEXT,
                parent_conversation_id TEXT,
                nesting_depth INTEGER
            );
            CREATE TABLE conversations_fts (
                conversation_id TEXT,
                chunk_index INTEGER,
                archived TEXT,
                title TEXT
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO conversation_summaries VALUES (?1, '初始标题', '已完成样式重构', 2, '2026-10-28T01:00:05Z', '[\"file:///workspace/demo\"]', '', 0)",
            params![id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO conversation_summaries VALUES (?1, '子代理', 'sub', 1, '2026-10-28T01:00:04Z', '', ?2, 1)",
            params![sub_id, id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO conversations_fts VALUES (?1, -1, '0', '初始标题')",
            params![id],
        )
        .unwrap();
        drop(conn);

        let paths = AntigravityPaths::offline(root.clone());
        let active = list_sessions(
            &paths,
            &ProviderListRequest {
                archived: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, id);
        assert_eq!(active[0].title, "初始标题");
        assert_eq!(active[0].cwd, "/workspace/demo");

        // Detail & timeline
        let detail = session_detail(
            &paths,
            &ProviderDetailRequest {
                session_key: id.clone(),
                limit: Some(50),
                before: None,
            },
        )
        .unwrap();
        assert_eq!(detail.events.len(), 4);
        assert_eq!(detail.events[0].kind, "user_message");
        assert_eq!(detail.events[1].kind, "thinking");
        assert_eq!(detail.events[2].kind, "tool_call");
        assert_eq!(detail.events[3].kind, "assistant_message");
        assert_eq!(detail.timeline.as_ref().unwrap().len(), 1);

        // Rename (with non-ASCII UTF-8 octal prototext round-trip)
        let renamed = rename_session(&paths, &id, "ChatGPT 风格新标题").unwrap();
        assert_eq!(renamed.title, "ChatGPT 风格新标题");
        assert!(!paths.proto_cache().exists());

        // Archive & Restore
        let archived_item = set_archived(&paths, &id, true).unwrap();
        assert!(archived_item.archived);
        assert_eq!(
            list_sessions(
                &paths,
                &ProviderListRequest {
                    archived: Some(false),
                    ..Default::default()
                }
            )
            .unwrap()
            .len(),
            0
        );
        assert_eq!(
            list_sessions(
                &paths,
                &ProviderListRequest {
                    archived: Some(true),
                    ..Default::default()
                }
            )
            .unwrap()
            .len(),
            1
        );

        let restored = set_archived(&paths, &id, false).unwrap();
        assert!(!restored.archived);

        // Permanent delete (including subagent descendant)
        let preview = preview_batch_item(&paths, &id, SessionOperation::Delete, true).unwrap();
        assert!(preview.allowed);
        assert_eq!(preview.descendants, vec![sub_id.clone()]);
        let del = execute_batch_item(
            &paths,
            &id,
            SessionOperation::Delete,
            &preview.descendants,
            true,
        )
        .unwrap();
        assert_eq!(del.status, "succeeded");
        assert!(del.residuals.is_empty());
        assert!(!root.join("brain").join(&id).exists());
        assert!(!root.join("brain").join(&sub_id).exists());
        assert!(!root
            .join("annotations")
            .join(format!("{id}.pbtxt"))
            .exists());
        assert!(list_sessions(&paths, &ProviderListRequest::default())
            .unwrap()
            .is_empty());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn live_connect_rpc_mode_handles_list_detail_rename_archive_and_delete_sweep() {
        let root = env::temp_dir().join(format!("nexushub-ag-live-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("annotations")).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        // Simulate an orphaned .pbtxt left on disk that post-RPC sweep must clean up
        fs::write(
            root.join("annotations").join(format!("{id}.pbtxt")),
            "title: \"orphan\"\n",
        )
        .unwrap();

        let state = Arc::new(Mutex::new(("初始会话".to_string(), false, false)));
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let server_shutdown = shutdown.clone();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server_state = state.clone();
        let server_id = id.clone();
        let handle = std::thread::spawn(move || {
            while !server_shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                if server_shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut req_line = String::new();
                if reader.read_line(&mut req_line).unwrap_or(0) == 0 {
                    continue;
                }
                let mut len = 0usize;
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':') {
                        if k.trim().eq_ignore_ascii_case("content-length") {
                            len = v.trim().parse().unwrap_or(0);
                        }
                    }
                }
                let mut body = vec![0u8; len];
                let _ = reader.read_exact(&mut body);
                let json_body: Value = serde_json::from_slice(&body).unwrap_or_default();
                let mut guard = server_state.lock().unwrap();
                let resp = if req_line.contains("GetAllCascadeTrajectories") {
                    if guard.2 {
                        json!({ "trajectorySummaries": {} })
                    } else {
                        json!({
                            "trajectorySummaries": {
                                (&server_id): {
                                    "summary": "默认摘要",
                                    "stepCount": 2,
                                    "lastModifiedTime": "2026-10-28T02:00:00Z",
                                    "status": "CASCADE_RUN_STATUS_IDLE",
                                    "annotations": {
                                        "title": guard.0,
                                        "archived": guard.1
                                    },
                                    "workspaces": [{ "workspaceFolderAbsoluteUri": "file:///tmp/proj" }]
                                }
                            }
                        })
                    }
                } else if req_line.contains("GetCascadeTrajectory ") {
                    json!({
                        "numTotalSteps": 2,
                        "trajectory": {
                            "cascadeId": server_id,
                            "steps": [
                                {
                                    "type": "CORTEX_STEP_TYPE_USER_INPUT",
                                    "status": "CORTEX_STEP_STATUS_DONE",
                                    "userInput": { "items": [{ "text": "你好 Antigravity" }] }
                                },
                                {
                                    "type": "CORTEX_STEP_TYPE_PLANNER_RESPONSE",
                                    "status": "CORTEX_STEP_STATUS_DONE",
                                    "plannerResponse": { "response": "你好！我是 Antigravity。" }
                                }
                            ]
                        }
                    })
                } else if req_line.contains("UpdateConversationAnnotations") {
                    if let Some(t) = json_body["annotations"]["title"].as_str() {
                        guard.0 = t.to_owned();
                    }
                    if let Some(a) = json_body["annotations"]["archived"].as_bool() {
                        guard.1 = a;
                    }
                    json!({})
                } else if req_line.contains("DeleteCascadeTrajectory") {
                    guard.2 = true;
                    json!({})
                } else {
                    json!({})
                };
                drop(guard);
                let bytes = serde_json::to_vec(&resp).unwrap();
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    bytes.len()
                );
                let _ = stream.write_all(header.as_bytes());
                let _ = stream.write_all(&bytes);
                if req_line.contains("DeleteCascadeTrajectory") {
                    // One more GetAllCascadeTrajectories check may follow
                }
            }
        });

        let paths = AntigravityPaths::with_fixed_rpc(root.clone(), port, "test_token_123");
        let list = list_sessions(&paths, &ProviderListRequest::default()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "初始会话");

        let detail = session_detail(
            &paths,
            &ProviderDetailRequest {
                session_key: id.clone(),
                limit: None,
                before: None,
            },
        )
        .unwrap();
        assert_eq!(detail.events.len(), 2);

        let renamed = rename_session(&paths, &id, "RPC 改名成功").unwrap();
        assert_eq!(renamed.title, "RPC 改名成功");

        let archived = set_archived(&paths, &id, true).unwrap();
        assert!(archived.archived);

        let del = execute_batch_item(&paths, &id, SessionOperation::Delete, &[], true).unwrap();
        assert_eq!(del.status, "succeeded");
        assert!(!root
            .join("annotations")
            .join(format!("{id}.pbtxt"))
            .exists());
        drop(paths);
        shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = TcpStream::connect(("127.0.0.1", port));
        let _ = handle.join();
        fs::remove_dir_all(root).unwrap();
    }
}
