use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeOverview {
    pub home: PathBuf,
    pub settings_exists: bool,
    pub settings_preview: Option<Value>,
    pub projects: Vec<ClaudeProject>,
}

pub fn claude_overview(paths: &ClaudePaths) -> Result<ClaudeOverview> {
    let settings_file = paths.settings_file();
    Ok(ClaudeOverview {
        home: paths.home.clone(),
        settings_exists: settings_file.exists(),
        settings_preview: read_settings_preview(&settings_file),
        projects: discover_claude_projects(paths)?,
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
        if title.is_none() {
            title = value
                .get("summary")
                .and_then(Value::as_str)
                .or_else(|| value.get("title").and_then(Value::as_str))
                .map(str::to_string)
                .or_else(|| first_text_fragment(&value));
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

fn redact_sensitive_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                let lowered = key.to_ascii_lowercase();
                if lowered.contains("token")
                    || lowered.contains("secret")
                    || lowered.contains("key")
                    || lowered.contains("password")
                {
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

fn decode_project_id(id: &str) -> String {
    if id.starts_with('-') {
        id.replacen('-', "/", 1).replace('-', "/")
    } else {
        id.to_string()
    }
}
