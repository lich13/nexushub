use crate::{
    platform::{PlatformKind, PlatformPaths},
    security::redact_output,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{env, path::PathBuf, time::Duration};
use tokio::{process::Command, time::timeout};

const DEFAULT_LIMIT: usize = 10;
const MAX_LIMIT: usize = 100;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeFlavor {
    Server,
    Local,
    Unavailable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStatusAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl ProbeCommand {
    pub fn shell_command(&self) -> String {
        std::iter::once(self.program.to_string_lossy().to_string())
            .chain(self.args.iter().cloned())
            .map(|part| shell_quote(&part))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeCommandProfile {
    pub flavor: ProbeFlavor,
    pub binary_path: Option<PathBuf>,
    pub available: bool,
}

impl ProbeCommandProfile {
    pub fn detect() -> Self {
        if let Ok(path) = env::var("NEXUSHUB_PROBE_BINARY") {
            let path = PathBuf::from(path);
            if path.is_file() {
                return profile_for_existing_binary(path);
            }
        }

        detect_server_binary()
            .map(Self::server)
            .or_else(|| detect_local_binary().map(Self::local))
            .unwrap_or_else(Self::unavailable)
    }

    pub fn server(path: impl Into<PathBuf>) -> Self {
        Self {
            flavor: ProbeFlavor::Server,
            binary_path: Some(path.into()),
            available: true,
        }
    }

    pub fn local(path: impl Into<PathBuf>) -> Self {
        Self {
            flavor: ProbeFlavor::Local,
            binary_path: Some(path.into()),
            available: true,
        }
    }

    pub fn unavailable() -> Self {
        Self {
            flavor: ProbeFlavor::Unavailable,
            binary_path: None,
            available: false,
        }
    }

    pub fn command(&self, kind: ProbeCommandKind) -> Option<ProbeCommand> {
        let program = self.binary_path.clone()?;
        let args = kind.args_for(self.flavor)?;
        Some(ProbeCommand { program, args })
    }

    pub fn job_command(&self, kind: ProbeCommandKind) -> Option<String> {
        self.command(kind).map(|command| command.shell_command())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProbeCommandKind {
    Status,
    Doctor,
    LifecycleStatus,
    HookStatus,
    LogsDbStatus,
    LogsDbMaintainDryRun,
    Running { limit: usize },
    ReplyNeeded { limit: usize },
    Recoverable { limit: usize },
    DebugAppServerThread { thread_id: String },
    InstallHooksRootRestartAppServer,
    TestBark,
}

impl ProbeCommandKind {
    fn args_for(&self, flavor: ProbeFlavor) -> Option<Vec<String>> {
        let args = match self {
            Self::Status => vec!["status".to_string()],
            Self::Doctor => vec!["doctor".to_string()],
            Self::LifecycleStatus => match flavor {
                ProbeFlavor::Local => vec!["lifecycle-status".to_string()],
                ProbeFlavor::Server => vec!["status".to_string()],
                ProbeFlavor::Unavailable => return None,
            },
            Self::HookStatus => match flavor {
                ProbeFlavor::Local => vec!["hook-status".to_string()],
                ProbeFlavor::Server => vec!["status".to_string()],
                ProbeFlavor::Unavailable => return None,
            },
            Self::LogsDbStatus => vec!["logs-db-status".to_string()],
            Self::LogsDbMaintainDryRun => {
                vec!["logs-db-maintain".to_string(), "--dry-run".to_string()]
            }
            Self::Running { limit } => {
                vec!["running".to_string(), bounded_limit(*limit).to_string()]
            }
            Self::ReplyNeeded { limit } => {
                vec![
                    "reply-needed".to_string(),
                    bounded_limit(*limit).to_string(),
                ]
            }
            Self::Recoverable { limit } => {
                vec!["recoverable".to_string(), bounded_limit(*limit).to_string()]
            }
            Self::DebugAppServerThread { thread_id } => {
                if !safe_thread_id(thread_id) || !matches!(flavor, ProbeFlavor::Local) {
                    return None;
                }
                vec!["debug-app-server-thread".to_string(), thread_id.clone()]
            }
            Self::InstallHooksRootRestartAppServer => {
                if !matches!(flavor, ProbeFlavor::Server) {
                    return None;
                }
                vec![
                    "install-hooks-root".to_string(),
                    "--restart-app-server".to_string(),
                ]
            }
            Self::TestBark => {
                if !matches!(flavor, ProbeFlavor::Server) {
                    return None;
                }
                vec!["test-bark".to_string()]
            }
        };
        Some(args)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeStatus {
    pub label: String,
    pub flavor: ProbeFlavor,
    pub availability: ProbeStatusAvailability,
    pub platform: PlatformKind,
    pub service_kind: String,
    pub service_name: String,
    pub binary_path: Option<PathBuf>,
    pub hook_status: String,
    pub bark_status: String,
    pub logs_db_status: String,
    pub lifecycle_status: String,
    pub doctor_status: String,
    pub recent_event_count: usize,
    pub running_count: usize,
    pub reply_needed_count: usize,
    pub recoverable_count: usize,
    pub config_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeDashboard {
    pub status: ProbeStatus,
    pub profile: ProbeCommandProfile,
    pub doctor: ProbeCommandOutput,
    pub lifecycle: ProbeCommandOutput,
    pub hook: ProbeCommandOutput,
    pub logs_db: ProbeCommandOutput,
    pub logs_db_maintain_dry_run: ProbeCommandOutput,
    pub running: ProbeCommandOutput,
    pub reply_needed: ProbeCommandOutput,
    pub recoverable: ProbeCommandOutput,
    pub recent_events: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeCommandOutput {
    pub command: Option<ProbeCommand>,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub json: Option<Value>,
    pub error: Option<String>,
}

pub fn probe_status(paths: &PlatformPaths, profile: ProbeCommandProfile) -> ProbeStatus {
    let availability = if profile.available {
        ProbeStatusAvailability::Available
    } else {
        ProbeStatusAvailability::Unavailable
    };
    let unknown = "unknown".to_string();
    ProbeStatus {
        label: "Probe".to_string(),
        flavor: profile.flavor,
        availability,
        platform: paths.kind,
        service_kind: paths.service_kind.clone(),
        service_name: paths.service_name.clone(),
        binary_path: profile.binary_path,
        hook_status: unknown.clone(),
        bark_status: unknown.clone(),
        logs_db_status: unknown.clone(),
        lifecycle_status: unknown.clone(),
        doctor_status: unknown,
        recent_event_count: 0,
        running_count: 0,
        reply_needed_count: 0,
        recoverable_count: 0,
        config_path: paths.config_file.clone(),
    }
}

pub async fn probe_dashboard(
    paths: &PlatformPaths,
    profile: ProbeCommandProfile,
) -> ProbeDashboard {
    let doctor = run_probe_command(&profile, ProbeCommandKind::Doctor).await;
    let lifecycle = run_probe_command(&profile, ProbeCommandKind::LifecycleStatus).await;
    let hook = run_probe_command(&profile, ProbeCommandKind::HookStatus).await;
    let logs_db = run_probe_command(&profile, ProbeCommandKind::LogsDbStatus).await;
    let logs_db_maintain_dry_run =
        run_probe_command(&profile, ProbeCommandKind::LogsDbMaintainDryRun).await;
    let running = run_probe_command(
        &profile,
        ProbeCommandKind::Running {
            limit: DEFAULT_LIMIT,
        },
    )
    .await;
    let reply_needed = run_probe_command(
        &profile,
        ProbeCommandKind::ReplyNeeded {
            limit: DEFAULT_LIMIT,
        },
    )
    .await;
    let recoverable = run_probe_command(
        &profile,
        ProbeCommandKind::Recoverable {
            limit: DEFAULT_LIMIT,
        },
    )
    .await;

    let mut status = probe_status(paths, profile.clone());
    status.doctor_status = status_text(&doctor);
    status.lifecycle_status = status_text(&lifecycle);
    status.hook_status = status_text(&hook);
    status.logs_db_status = status_text(&logs_db);
    status.running_count = json_len(&running.json);
    status.reply_needed_count = json_len(&reply_needed.json);
    status.recoverable_count = json_len(&recoverable.json);
    status.recent_event_count = status
        .running_count
        .saturating_add(status.reply_needed_count)
        .saturating_add(status.recoverable_count);

    ProbeDashboard {
        status,
        profile,
        doctor,
        lifecycle,
        hook,
        logs_db,
        logs_db_maintain_dry_run,
        running,
        reply_needed,
        recoverable,
        recent_events: json!({ "source": "probe_command_outputs" }),
    }
}

pub async fn run_probe_command(
    profile: &ProbeCommandProfile,
    kind: ProbeCommandKind,
) -> ProbeCommandOutput {
    let Some(command) = profile.command(kind) else {
        return unavailable_output();
    };
    run_command(command, DEFAULT_TIMEOUT).await
}

async fn run_command(command: ProbeCommand, wait: Duration) -> ProbeCommandOutput {
    let mut child = Command::new(&command.program);
    child.args(&command.args);
    let output = match timeout(wait, child.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(err)) => {
            return ProbeCommandOutput {
                command: Some(command),
                success: false,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                json: None,
                error: Some(err.to_string()),
            }
        }
        Err(_) => {
            return ProbeCommandOutput {
                command: Some(command),
                success: false,
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                json: None,
                error: Some("probe command timed out".to_string()),
            }
        }
    };

    let stdout = redact_output(&String::from_utf8_lossy(&output.stdout));
    let stderr = redact_output(&String::from_utf8_lossy(&output.stderr));
    let json = parse_json_output(&stdout);
    ProbeCommandOutput {
        command: Some(command),
        success: output.status.success(),
        exit_code: output.status.code(),
        stdout,
        stderr,
        json,
        error: None,
    }
}

fn unavailable_output() -> ProbeCommandOutput {
    ProbeCommandOutput {
        command: None,
        success: false,
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        json: None,
        error: Some("probe CLI unavailable".to_string()),
    }
}

fn detect_server_binary() -> Option<PathBuf> {
    [
        PathBuf::from("/usr/local/bin/codex-sentinel-server"),
        PathBuf::from("/opt/codex-sentinel-server/codex-sentinel-server"),
    ]
    .into_iter()
    .chain(path_lookup("codex-sentinel-server"))
    .find(|path| path.is_file())
}

fn detect_local_binary() -> Option<PathBuf> {
    [
        PathBuf::from("/Applications/Codex Sentinel Lite.app/Contents/MacOS/codex-sentinel-lite"),
        PathBuf::from("/usr/local/bin/codex-sentinel-lite"),
    ]
    .into_iter()
    .chain(path_lookup("codex-sentinel-lite"))
    .find(|path| path.is_file())
}

fn path_lookup(binary: &str) -> Vec<PathBuf> {
    env::var_os("PATH")
        .map(|value| {
            env::split_paths(&value)
                .map(|dir| dir.join(binary))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn profile_for_existing_binary(path: PathBuf) -> ProbeCommandProfile {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if name.contains("server") {
        ProbeCommandProfile::server(path)
    } else {
        ProbeCommandProfile::local(path)
    }
}

fn parse_json_output(output: &str) -> Option<Value> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str::<Value>(trimmed).ok()
}

fn status_text(output: &ProbeCommandOutput) -> String {
    if output.success {
        "ok"
    } else if output.command.is_none() {
        "unavailable"
    } else {
        "error"
    }
    .to_string()
}

fn json_len(value: &Option<Value>) -> usize {
    match value {
        Some(Value::Array(items)) => items.len(),
        Some(Value::Object(map)) => map.len(),
        _ => 0,
    }
}

fn bounded_limit(limit: usize) -> usize {
    limit.clamp(1, MAX_LIMIT)
}

fn safe_thread_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '-' | '_' | ':'))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}
