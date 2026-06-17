use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    env, fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct ClaudePaths {
    pub home: PathBuf,
}

impl ClaudePaths {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self { home: home.into() }
    }

    pub fn default_for_user() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self::new(home.join(".claude"))
    }

    pub fn projects_dir(&self) -> PathBuf {
        self.home.join("projects")
    }

    pub fn settings_file(&self) -> PathBuf {
        self.home.join("settings.json")
    }

    pub fn settings_local_file(&self) -> PathBuf {
        self.home.join("settings.local.json")
    }

    pub fn user_config_file(&self) -> Option<PathBuf> {
        if self.home.file_name().and_then(|name| name.to_str()) == Some(".claude") {
            self.home.parent().map(|parent| parent.join(".claude.json"))
        } else {
            Some(self.home.join(".claude.json"))
        }
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.home.join("cache")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.home.join("logs")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaudeProject {
    pub id: String,
    pub display_name: String,
    pub path_hint: Option<String>,
    pub session_count: usize,
    pub sessions: Vec<ClaudeSessionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaudeSessionSummary {
    pub id: String,
    pub title: Option<String>,
    pub updated_at: Option<String>,
    pub message_count: usize,
    pub file: PathBuf,
    pub last_message_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaudeRecentSession {
    pub project_id: String,
    pub project_display_name: String,
    pub id: String,
    pub title: Option<String>,
    pub updated_at: Option<String>,
    pub message_count: usize,
    pub file: PathBuf,
    pub last_message_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaudeMcpSummary {
    pub config_files: Vec<PathBuf>,
    pub server_count: usize,
    pub servers: Vec<ClaudeMcpServerSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaudeMcpServerSummary {
    pub name: String,
    pub command: Option<String>,
    pub transport: Option<String>,
    pub args_count: usize,
    pub env_keys: Vec<String>,
    pub has_sensitive_env: bool,
    pub raw_config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaudeInstallationSummary {
    pub claude_home: PathBuf,
    pub settings_file: PathBuf,
    pub settings_exists: bool,
    pub settings_local_file: PathBuf,
    pub settings_local_exists: bool,
    pub user_config_file: Option<PathBuf>,
    pub user_config_exists: bool,
    pub executable_candidates: Vec<PathBuf>,
    pub version_hint: Option<String>,
    pub health_hints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaudeCacheLogStatus {
    pub cache_dir: PathBuf,
    pub cache_exists: bool,
    pub cache_file_count: usize,
    pub cache_total_bytes: u64,
    pub log_dir: PathBuf,
    pub log_exists: bool,
    pub log_file_count: usize,
    pub log_total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeOverview {
    pub home: PathBuf,
    pub settings_exists: bool,
    pub settings_preview: Option<Value>,
    pub projects: Vec<ClaudeProject>,
    pub recent_sessions: Vec<ClaudeRecentSession>,
    pub mcp: ClaudeMcpSummary,
    pub installation: ClaudeInstallationSummary,
    pub cache_status: ClaudeCacheLogStatus,
}

pub fn claude_overview(paths: &ClaudePaths) -> Result<ClaudeOverview> {
    let settings_file = paths.settings_file();
    let settings_preview = read_settings_preview(&settings_file);
    let projects = discover_claude_projects(paths)?;
    Ok(ClaudeOverview {
        home: paths.home.clone(),
        settings_exists: settings_file.exists(),
        settings_preview: settings_preview.clone(),
        recent_sessions: recent_sessions(&projects, 10),
        mcp: read_mcp_summary(paths, settings_preview.as_ref()),
        installation: read_installation_summary(paths),
        cache_status: read_cache_log_status(paths),
        projects,
    })
}

pub fn discover_claude_projects(paths: &ClaudePaths) -> Result<Vec<ClaudeProject>> {
    let projects_dir = paths.projects_dir();
    if !projects_dir.exists() {
        return Ok(Vec::new());
    }
    let mut projects = Vec::new();
    for entry in
        fs::read_dir(&projects_dir).with_context(|| format!("read {}", projects_dir.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if !file_type.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().to_string();
        let project_dir = entry.path();
        let mut sessions = discover_project_sessions(&project_dir)?;
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.id.cmp(&b.id)));
        let display_name = decode_project_id(&id);
        projects.push(ClaudeProject {
            id,
            path_hint: Some(display_name.clone()),
            display_name,
            session_count: sessions.len(),
            sessions,
        });
    }
    projects.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(projects)
}

fn discover_project_sessions(project_dir: &Path) -> Result<Vec<ClaudeSessionSummary>> {
    let mut sessions = Vec::new();
    for entry in WalkDir::new(project_dir).max_depth(2) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("jsonl") {
            continue;
        }
        sessions.push(read_session_summary(path)?);
    }
    Ok(sessions)
}

fn read_session_summary(path: &Path) -> Result<ClaudeSessionSummary> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut title = None;
    let mut updated_at = None;
    let mut message_count = 0usize;
    let mut last_message_preview = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        message_count += 1;
        if let Some(timestamp) = value.get("timestamp").and_then(Value::as_str) {
            updated_at = Some(timestamp.to_string());
        }
        if let Some(text) = first_text_fragment(&value) {
            last_message_preview = Some(text.clone());
            if title.is_none() {
                title = Some(text);
            }
        }
        if title.is_none() {
            title = value
                .get("summary")
                .and_then(Value::as_str)
                .or_else(|| value.get("title").and_then(Value::as_str))
                .map(str::to_string);
        }
    }
    Ok(ClaudeSessionSummary {
        id: path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("session")
            .to_string(),
        title,
        updated_at,
        message_count,
        file: path.to_path_buf(),
        last_message_preview,
    })
}

fn first_text_fragment(value: &Value) -> Option<String> {
    value
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(|content| {
            content.as_str().map(str::to_string).or_else(|| {
                content.as_array().and_then(|items| {
                    items.iter().find_map(|item| {
                        item.get("text").and_then(Value::as_str).map(str::to_string)
                    })
                })
            })
        })
        .map(|text| text.trim().chars().take(80).collect::<String>())
        .filter(|text| !text.is_empty())
}

fn read_settings_preview(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    let mut value = serde_json::from_str::<Value>(&text).ok()?;
    redact_sensitive_json(&mut value);
    Some(value)
}

fn recent_sessions(projects: &[ClaudeProject], limit: usize) -> Vec<ClaudeRecentSession> {
    let mut sessions = projects
        .iter()
        .flat_map(|project| {
            project.sessions.iter().map(|session| ClaudeRecentSession {
                project_id: project.id.clone(),
                project_display_name: project.display_name.clone(),
                id: session.id.clone(),
                title: session.title.clone(),
                updated_at: session.updated_at.clone(),
                message_count: session.message_count,
                file: session.file.clone(),
                last_message_preview: session.last_message_preview.clone(),
            })
        })
        .collect::<Vec<_>>();
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.id.cmp(&b.id)));
    sessions.truncate(limit);
    sessions
}

fn read_mcp_summary(paths: &ClaudePaths, settings_preview: Option<&Value>) -> ClaudeMcpSummary {
    let mut config_files = Vec::new();
    let mut servers = Vec::new();
    if paths.settings_file().exists() {
        config_files.push(paths.settings_file());
    }
    if paths.settings_local_file().exists() {
        config_files.push(paths.settings_local_file());
    }
    if let Some(user_config_file) = paths
        .user_config_file()
        .filter(|user_config_file| user_config_file.exists())
    {
        config_files.push(user_config_file);
    }

    if let Some(settings) = settings_preview {
        collect_mcp_servers(settings, &mut servers);
    }
    for path in [
        paths.settings_local_file(),
        paths.user_config_file().unwrap_or_default(),
    ] {
        if !path.exists() {
            continue;
        }
        if let Some(mut value) = read_json_file(&path) {
            redact_sensitive_json(&mut value);
            collect_mcp_servers(&value, &mut servers);
        }
    }
    servers.sort_by(|a, b| a.name.cmp(&b.name));
    servers.dedup_by(|a, b| a.name == b.name);
    ClaudeMcpSummary {
        server_count: servers.len(),
        config_files,
        servers,
    }
}

fn collect_mcp_servers(value: &Value, servers: &mut Vec<ClaudeMcpServerSummary>) {
    let Some(mcp_servers) = value.get("mcpServers").and_then(Value::as_object) else {
        return;
    };
    for (name, config) in mcp_servers {
        let command = config
            .get("command")
            .and_then(Value::as_str)
            .map(str::to_string);
        let transport = config
            .get("transport")
            .and_then(Value::as_str)
            .or_else(|| config.get("type").and_then(Value::as_str))
            .map(str::to_string);
        let args_count = config
            .get("args")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        let mut env_keys = config
            .get("env")
            .and_then(Value::as_object)
            .map(|env| env.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        env_keys.sort();
        let has_sensitive_env = env_keys.iter().any(|key| is_sensitive_key(key));
        servers.push(ClaudeMcpServerSummary {
            name: name.clone(),
            command,
            transport,
            args_count,
            env_keys,
            has_sensitive_env,
            raw_config: config.clone(),
        });
    }
}

fn read_installation_summary(paths: &ClaudePaths) -> ClaudeInstallationSummary {
    let settings_file = paths.settings_file();
    let settings_local_file = paths.settings_local_file();
    let user_config_file = paths.user_config_file();
    let user_config_exists = user_config_file
        .as_ref()
        .is_some_and(|user_config_file| user_config_file.exists());
    let executable_candidates = find_executable_candidates("claude");
    let version_hint = executable_candidates
        .iter()
        .find_map(|candidate| read_version_hint(candidate));
    let mut health_hints = Vec::new();
    if !paths.home.exists() {
        health_hints.push("claude_home_missing".to_string());
    }
    if !settings_file.exists() {
        health_hints.push("settings_missing".to_string());
    }
    if executable_candidates.is_empty() {
        health_hints.push("claude_executable_not_found_in_path".to_string());
    }
    if version_hint.is_none() {
        health_hints.push("version_hint_unavailable_without_running_claude".to_string());
    }
    ClaudeInstallationSummary {
        claude_home: paths.home.clone(),
        settings_exists: settings_file.exists(),
        settings_file,
        settings_local_exists: settings_local_file.exists(),
        settings_local_file,
        user_config_file,
        user_config_exists,
        executable_candidates,
        version_hint,
        health_hints,
    }
}

fn read_cache_log_status(paths: &ClaudePaths) -> ClaudeCacheLogStatus {
    let cache_dir = paths.cache_dir();
    let log_dir = paths.logs_dir();
    let (cache_file_count, cache_total_bytes) = file_count_and_bytes(&cache_dir);
    let (log_file_count, log_total_bytes) = file_count_and_bytes(&log_dir);
    ClaudeCacheLogStatus {
        cache_exists: cache_dir.exists(),
        cache_dir,
        cache_file_count,
        cache_total_bytes,
        log_exists: log_dir.exists(),
        log_dir,
        log_file_count,
        log_total_bytes,
    }
}

fn file_count_and_bytes(path: &Path) -> (usize, u64) {
    if !path.exists() {
        return (0, 0);
    }
    let mut file_count = 0usize;
    let mut total_bytes = 0u64;
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        file_count += 1;
        total_bytes += entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
    }
    (file_count, total_bytes)
}

fn read_json_file(path: &Path) -> Option<Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str::<Value>(&text).ok()
}

fn find_executable_candidates(name: &str) -> Vec<PathBuf> {
    let Some(paths) = env::var_os("PATH") else {
        return Vec::new();
    };
    let mut candidates = env::split_paths(&paths)
        .map(|path| path.join(name))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    candidates
}

fn read_version_hint(executable: &Path) -> Option<String> {
    let package_json = executable.parent().and_then(Path::parent).map(|prefix| {
        prefix
            .join("lib")
            .join("node_modules")
            .join("@anthropic-ai")
            .join("claude-code")
            .join("package.json")
    })?;
    read_json_file(&package_json)?
        .get("version")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn redact_sensitive_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if is_sensitive_key(key) {
                    *value = Value::String("[redacted]".to_string());
                } else {
                    redact_sensitive_json(value);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_sensitive_json(item);
            }
        }
        _ => {}
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let lowered = key.to_ascii_lowercase();
    lowered.contains("token")
        || lowered.contains("secret")
        || lowered.contains("key")
        || lowered.contains("password")
        || lowered.contains("credential")
        || lowered.contains("auth")
}

fn decode_project_id(id: &str) -> String {
    if id.starts_with('-') {
        id.replacen('-', "/", 1).replace('-', "/")
    } else {
        id.to_string()
    }
}
