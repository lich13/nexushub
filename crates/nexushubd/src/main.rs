mod api;
mod auth;
mod state;
mod turnstile;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use nexushub_core::{
    codex::{
        list_threads, resolve_codex_paths, rollout_completion_last_agent_message,
        rollout_latest_assistant_message,
    },
    config::{
        patch_probe_config_toml, valid_probe_notification_server_url, CodexProbeConfigPatch,
        Config, ProbeConfigFilePatch, ProbeHooksConfigPatch, ProbeLogsDbConfigPatch,
        ProbeNotificationsConfigPatch, ProbeObservabilityConfigPatch, ProbeSettingsPatch,
    },
    db::{NewProbeEvent, PanelDb},
    platform::PlatformPaths,
    probe::{
        redact_probe_event_for_output, ProbeEventInput, ProbeEventOutcome,
        ProbeLogsDbMaintenanceResult, ProbeRuntime,
        DEFAULT_LOGS_DB_COMPACT_QUICK_CHECK_TIMEOUT_SECONDS,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use state::AppState;
use std::{
    fs,
    io::{self, IsTerminal, Read},
    net::SocketAddr,
    path::{Path, PathBuf},
};
use tokio::{net::TcpListener, time};
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const DEFAULT_CONFIG: &str = "/opt/nexushub/config.toml";
const PROBE_LOGS_DB_LAST_MAINTAIN_SETTING: &str = "probe_logs_db_last_maintain";
const PROBE_LOGS_DB_LAST_COMPACT_SETTING: &str = "probe_logs_db_last_compact";
const PROBE_PASSIVE_SENT_MARKER_PREFIX: &str = "probe_passive_sent_marker:";
const PROBE_LOGS_DB_SCHEDULER_TICK_SECONDS: u64 = 300;
const PROBE_THREAD_SCAN_TICK_SECONDS: u64 = 120;
const PROBE_REPLY_NEEDED_FRESH_WINDOW_SECONDS: i64 = 10 * 60;
const PROBE_BARK_BODY_CHUNK_BYTES: usize = 2_400;
static PROBE_LOGS_DB_MAINTENANCE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static PROBE_THREAD_SCAN_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Parser)]
#[command(
    name = "nexushubd",
    version,
    about = "Headless Web panel for local Codex state and controlled jobs"
)]
struct Cli {
    #[arg(long, env = "NEXUSHUB_CONFIG", default_value = DEFAULT_CONFIG)]
    config: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Serve,
    Doctor,
    InitConfig,
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },
    Probe {
        #[command(subcommand)]
        command: ProbeCommand,
    },
}

#[derive(Debug, Subcommand)]
enum AdminCommand {
    Init {
        #[arg(long, default_value = "admin")]
        username: String,
        #[arg(long, env = "NEXUSHUB_ADMIN_PASSWORD")]
        password: String,
    },
    ResetPassword {
        #[arg(long, default_value = "admin")]
        username: String,
        #[arg(long, env = "NEXUSHUB_ADMIN_PASSWORD")]
        password: String,
    },
}

#[derive(Debug, Subcommand)]
enum ProbeCommand {
    Status,
    HookStatus,
    LogsDbStatus,
    Events {
        #[arg(long, default_value_t = 50)]
        limit: u32,
    },
    Running {
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    ReplyNeeded {
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    Recoverable {
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    HookStop {
        #[arg(long)]
        thread_id: Option<String>,
        #[arg(long)]
        turn_id: Option<String>,
        #[arg(long, default_value = "hook-stop")]
        kind: String,
    },
    HooksInstall {
        #[arg(long)]
        dry_run: bool,
    },
    NotifyCompletion {
        #[arg(long)]
        thread_id: Option<String>,
        #[arg(long)]
        turn_id: Option<String>,
    },
    BarkTest,
    LogsDbMaintain {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        compact: bool,
        #[arg(long, default_value_t = DEFAULT_LOGS_DB_COMPACT_QUICK_CHECK_TIMEOUT_SECONDS)]
        quick_check_timeout_seconds: u64,
    },
    LifecycleRepair,
    ServiceRestart,
    LegacyImport,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nexushubd=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::InitConfig => {
            Config::write_default(&cli.config)?;
            println!("wrote {}", cli.config.display());
        }
        Command::Doctor => {
            let config = Config::load(&cli.config)?;
            let db = open_panel_db(&config)?;
            let resolved = resolve_codex_paths(&config.codex.home);
            println!("config={}", cli.config.display());
            println!("db={}", db.path().display());
            println!("codex_home={}", resolved.home.display());
            println!(
                "configured_codex_home={}",
                resolved.configured_codex_home.as_deref().unwrap_or("auto")
            );
            println!("codex_home_source={}", resolved.codex_home_source);
            println!("listen={}", config.server.listen);
            println!("admin_count={}", db.admin_count()?);
            println!("codex_read_model=local_state_rollout_logs");
            let status = nexushub_core::system::system_status(&config).await?;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        Command::Admin { command } => {
            let config = Config::load(&cli.config)?;
            let db = open_panel_db(&config)?;
            match command {
                AdminCommand::Init { username, password } => {
                    init_admin(db, &username, &password, false)?
                }
                AdminCommand::ResetPassword { username, password } => {
                    init_admin(db, &username, &password, true)?
                }
            }
        }
        Command::Probe { command } => {
            let config = Config::load(&cli.config)?;
            let db = open_panel_db(&config)?;
            run_probe_command(command, &config, db).await?;
        }
        Command::Serve => serve(cli.config).await?,
    }
    Ok(())
}

async fn run_probe_command(command: ProbeCommand, config: &Config, db: PanelDb) -> Result<()> {
    match command {
        ProbeCommand::Status => {
            println!(
                "{}",
                serde_json::to_string_pretty(&probe_runtime(config).status().await?)?
            );
        }
        ProbeCommand::HookStatus => {
            println!(
                "{}",
                serde_json::to_string_pretty(&probe_runtime(config).hook_status())?
            );
        }
        ProbeCommand::LogsDbStatus => {
            println!(
                "{}",
                serde_json::to_string_pretty(&probe_runtime(config).logs_db_status())?
            );
        }
        ProbeCommand::Events { limit } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "events": db.list_probe_events(limit)?.into_iter().map(redact_probe_event).collect::<Vec<_>>()
                }))?
            );
        }
        ProbeCommand::Running { limit } => {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &probe_thread_snapshot(config, &db, "running", limit).await?
                )?
            );
        }
        ProbeCommand::ReplyNeeded { limit } => {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &probe_thread_snapshot(config, &db, "reply-needed", limit).await?
                )?
            );
        }
        ProbeCommand::Recoverable { limit } => {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &probe_thread_snapshot(config, &db, "recoverable", limit).await?
                )?
            );
        }
        ProbeCommand::HookStop {
            thread_id,
            turn_id,
            kind,
        } => match handle_hook_stop_command(config, &db, thread_id, turn_id, kind).await {
            Ok(result) => {
                let (stdout, stderr) = hook_stop_cli_output(&result)?;
                eprint!("{stderr}");
                print!("{stdout}");
            }
            Err(err) => {
                eprintln!(
                    "{}",
                    serde_json::to_string(&json!({
                        "ok": false,
                        "error": "probe_hook_stop_failed",
                    }))?
                );
                tracing::warn!("probe hook-stop failed before event recording");
                tracing::debug!("probe hook-stop diagnostic: {err:#}");
                println!("{}", serde_json::to_string(&codex_stop_continue_output())?);
            }
        },
        ProbeCommand::HooksInstall { dry_run } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&install_probe_hooks(config, dry_run).await?)?
            );
        }
        ProbeCommand::NotifyCompletion { thread_id, turn_id } => {
            let stdin_payload = read_optional_stdin_json()?;
            let input = notify_completion_context(
                config,
                stdin_payload.as_ref(),
                thread_id.as_deref(),
                turn_id.as_deref(),
            )?;
            let event = probe_runtime(config).build_event(input);
            let (outcome, bark) = record_probe_event_with_bark(config, &db, event).await?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "probe_event": outcome,
                    "bark": bark,
                }))?
            );
        }
        ProbeCommand::BarkTest => {
            let device_key = db.get_secret_setting_bytes("probe_bark_device_key")?;
            let configured = device_key.as_ref().is_some_and(|value| !value.is_empty());
            let bark = if config.probe.notifications.enabled && configured {
                send_bark_notification(
                    config,
                    device_key.as_deref().unwrap_or_default(),
                    &ProbeBarkRequest {
                        title: "Codex Sentinel Lite".to_string(),
                        body: "Bark 推送通道正常。".to_string(),
                        dedupe_key: "probe-bark-test".to_string(),
                    },
                    std::time::Duration::from_secs(8),
                )
                .await?
            } else {
                ProbeBarkOutcome::skipped(
                    if config.probe.notifications.enabled {
                        "device_key_missing"
                    } else {
                        "notifications_disabled"
                    },
                    config.probe.notifications.enabled,
                    true,
                    configured,
                )
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": bark.sent || (!config.probe.notifications.enabled && bark.skipped),
                    "configured": configured,
                    "skipped": bark.skipped,
                    "sent": bark.sent,
                    "reason": bark.reason,
                    "http_status": bark.http_status,
                }))?
            );
        }
        ProbeCommand::LogsDbMaintain {
            dry_run,
            compact,
            quick_check_timeout_seconds,
        } => {
            let result = probe_runtime(config).maintain_logs_db_with_compaction_timeout(
                dry_run,
                compact && !dry_run,
                std::time::Duration::from_secs(quick_check_timeout_seconds),
            )?;
            let (probe_events_deleted, probe_dedupe_deleted) = db.maintain_probe_events(
                config.probe.logs_db.retention_days,
                config.probe.logs_db.max_delete_rows_per_run,
                dry_run,
            )?;
            let mut stored = serde_json::to_value(&result)?;
            add_probe_events_maintenance_fields(
                &mut stored,
                probe_events_deleted,
                probe_dedupe_deleted,
                dry_run,
            );
            db.set_setting(
                PROBE_LOGS_DB_LAST_MAINTAIN_SETTING,
                &serde_json::to_string(&stored)?,
            )?;
            if result.vacuumed {
                db.set_setting(
                    PROBE_LOGS_DB_LAST_COMPACT_SETTING,
                    &serde_json::to_string(&result)?,
                )?;
            }
            println!("{}", serde_json::to_string_pretty(&stored)?);
        }
        ProbeCommand::LifecycleRepair => {
            anyhow::bail!(
                "unsupported_probe_action: lifecycle_repair has no fixed NexusHub implementation"
            );
        }
        ProbeCommand::ServiceRestart => {
            anyhow::bail!(
                "unsupported_probe_action: service_restart has no fixed NexusHub implementation"
            );
        }
        ProbeCommand::LegacyImport => {
            let result = import_legacy_sentinel_config(&db)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}

async fn handle_hook_stop_command(
    config: &Config,
    db: &PanelDb,
    thread_id: Option<String>,
    turn_id: Option<String>,
    kind: String,
) -> Result<HookStopResult> {
    let stdin_payload = read_optional_stdin_json()?;
    let payload_thread_id = stdin_payload.as_ref().and_then(|value| {
        read_string_field(value, &["thread_id", "threadId", "session_id", "sessionId"])
    });
    let payload_turn_id = stdin_payload
        .as_ref()
        .and_then(|value| read_string_field(value, &["turn_id", "turnId"]));
    let payload_session_id = stdin_payload
        .as_ref()
        .and_then(|value| read_string_field(value, &["session_id", "sessionId"]));
    let payload_transcript_path = stdin_payload
        .as_ref()
        .and_then(|value| read_string_field(value, &["transcript_path", "transcriptPath"]));
    let payload_last_assistant_message = stdin_payload.as_ref().and_then(|value| {
        read_string_field(value, &["last_assistant_message", "lastAssistantMessage"])
    });
    let transcript_last_assistant_message = payload_last_assistant_message
        .is_none()
        .then(|| {
            payload_transcript_path
                .as_deref()
                .map(Path::new)
                .and_then(|path| rollout_latest_assistant_message(path).ok().flatten())
        })
        .flatten();
    let last_assistant_message = payload_last_assistant_message
        .as_deref()
        .or(transcript_last_assistant_message.as_deref());
    let event_thread_id = thread_id
        .or(payload_thread_id.clone())
        .or(payload_session_id.clone());
    let event_turn_id = turn_id.or(payload_turn_id.clone());
    let event_kind = stdin_payload
        .as_ref()
        .and_then(|value| read_string_field(value, &["kind", "event_kind", "eventKind"]))
        .unwrap_or(kind);
    let event = probe_runtime(config).build_event(ProbeEventInput::hook_stop_with_context(
        event_thread_id.as_deref(),
        event_turn_id.as_deref(),
        payload_session_id.as_deref(),
        payload_transcript_path.as_deref(),
        last_assistant_message,
        &event_kind,
    ));
    handle_built_probe_event(config, db, event).await
}

fn read_optional_stdin_json() -> Result<Option<Value>> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        return Ok(None);
    }
    let mut buffer = String::new();
    let mut lock = stdin.lock();
    lock.read_to_string(&mut buffer)
        .context("read hook stop stdin")?;
    if buffer.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&buffer)
        .map(Some)
        .context("parse stdin json")
}

fn read_string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    })
}

fn notify_completion_context(
    config: &Config,
    payload: Option<&Value>,
    cli_thread_id: Option<&str>,
    cli_turn_id: Option<&str>,
) -> Result<ProbeEventInput> {
    let payload_thread_id = payload.and_then(|value| {
        read_string_field(value, &["thread_id", "threadId", "session_id", "sessionId"])
    });
    let payload_turn_id =
        payload.and_then(|value| read_string_field(value, &["turn_id", "turnId"]));
    let thread_id = cli_thread_id.map(str::to_string).or(payload_thread_id);
    let turn_id = cli_turn_id.map(str::to_string).or(payload_turn_id);
    let session_id =
        payload.and_then(|value| read_string_field(value, &["session_id", "sessionId"]));
    let transcript_path =
        payload.and_then(|value| read_string_field(value, &["transcript_path", "transcriptPath"]));
    let payload_thread_title = payload
        .and_then(|value| read_string_field(value, &["thread_title", "threadTitle", "title"]));
    let explicit_message = payload.and_then(|value| {
        read_string_field(
            value,
            &[
                "last_assistant_message",
                "lastAssistantMessage",
                "last_agent_message",
                "lastAgentMessage",
                "body",
                "message",
            ],
        )
    });
    let resolved_thread = if transcript_path.is_none() && thread_id.is_some() {
        notify_completion_thread_summary(config, thread_id.as_deref().unwrap())
            .ok()
            .flatten()
    } else {
        None
    };
    let resolved_transcript_path = transcript_path.or_else(|| {
        resolved_thread
            .as_ref()
            .and_then(|thread| thread.rollout_path.as_ref())
            .map(|path| path.to_string_lossy().to_string())
    });
    let thread_title = payload_thread_title.or_else(|| {
        resolved_thread
            .as_ref()
            .map(|thread| thread.title.clone())
            .filter(|title| !title.trim().is_empty())
    });
    let (message, body_source) = if let Some(message) = explicit_message {
        (Some(message), Some("stdin.last_agent_message".to_string()))
    } else if let Some(path) = resolved_transcript_path.as_deref() {
        match rollout_completion_last_agent_message(Path::new(path), turn_id.as_deref())? {
            Some(message) => (
                Some(message),
                Some("task_complete.last_agent_message".to_string()),
            ),
            None => (None, None),
        }
    } else if let Some(message) = resolved_thread
        .as_ref()
        .and_then(|thread| thread.latest_message.clone())
    {
        (Some(message), Some("thread.latest_message".to_string()))
    } else {
        (None, None)
    };
    Ok(ProbeEventInput::notify_completion_with_context(
        thread_id.as_deref().or(session_id.as_deref()),
        turn_id.as_deref(),
        session_id.as_deref(),
        resolved_transcript_path.as_deref(),
        message.as_deref(),
        body_source.as_deref(),
    )
    .with_thread_title(thread_title.as_deref()))
}

fn notify_completion_thread_summary(
    config: &Config,
    thread_id: &str,
) -> Result<Option<nexushub_core::codex::ThreadSummary>> {
    let resolved = resolve_codex_paths(&config.codex.home);
    Ok(
        list_threads(&resolved.codex_paths(), None, Some(thread_id), 1)?
            .into_iter()
            .find(|thread| thread.id == thread_id),
    )
}

fn probe_runtime(config: &Config) -> ProbeRuntime {
    ProbeRuntime::new(config.clone(), PlatformPaths::current())
}

fn redact_probe_event(event: nexushub_core::db::ProbeEvent) -> nexushub_core::db::ProbeEvent {
    redact_probe_event_for_output(event)
}

async fn probe_thread_snapshot(
    config: &Config,
    db: &PanelDb,
    status: &'static str,
    limit: usize,
) -> Result<Value> {
    let status = status.trim();
    let state = AppState::new(config.clone(), db.clone());
    let threads = api::load_probe_threads(&state, status, limit).await?;
    Ok(json!({
        "status": status,
        "count": threads.len(),
        "threads": threads,
    }))
}

async fn install_probe_hooks(config: &Config, dry_run: bool) -> Result<Value> {
    let resolved = resolve_codex_paths(&config.codex.home);
    let hooks_path = resolved.home.join("hooks.json");
    let codex_config_path = resolved.home.join("config.toml");
    let hook_command = format!(
        "/opt/nexushub/bin/nexushubd --config {} probe hook-stop",
        PlatformPaths::current().config_file.display()
    );
    let mut root = read_hooks_json(&hooks_path)?;
    let hooks_json_changed = ensure_stop_hook(&mut root, &hook_command);
    let config_before = read_optional_text(&codex_config_path)?;
    let codex_config_after = ensure_codex_hooks_feature(&config_before)?;
    let codex_config_changed = codex_config_after != config_before;
    let backup_path = hooks_path.with_extension(format!(
        "json.nexushub-probe-bak-{}",
        chrono::Utc::now().format("%Y%m%d%H%M%S")
    ));
    let codex_config_backup_path = codex_config_path.with_extension(format!(
        "toml.nexushub-probe-bak-{}",
        chrono::Utc::now().format("%Y%m%d%H%M%S")
    ));
    if hooks_json_changed && !dry_run {
        if let Some(parent) = hooks_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create hooks dir {}", parent.display()))?;
        }
        if hooks_path.exists() {
            fs::copy(&hooks_path, &backup_path).with_context(|| {
                format!(
                    "backup hooks {} to {}",
                    hooks_path.display(),
                    backup_path.display()
                )
            })?;
        }
        fs::write(&hooks_path, serde_json::to_vec_pretty(&root)?)
            .with_context(|| format!("write hooks {}", hooks_path.display()))?;
    }
    if codex_config_changed && !dry_run {
        if let Some(parent) = codex_config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create codex config dir {}", parent.display()))?;
        }
        if codex_config_path.exists() {
            fs::copy(&codex_config_path, &codex_config_backup_path).with_context(|| {
                format!(
                    "backup codex config {} to {}",
                    codex_config_path.display(),
                    codex_config_backup_path.display()
                )
            })?;
        }
        fs::write(&codex_config_path, codex_config_after)
            .with_context(|| format!("write codex config {}", codex_config_path.display()))?;
    }
    let changed = hooks_json_changed || codex_config_changed;
    Ok(json!({
        "ok": true,
        "dry_run": dry_run,
        "changed": changed,
        "hooks_json_changed": hooks_json_changed,
        "codex_config_changed": codex_config_changed,
        "hooks_json": hooks_path,
        "codex_config": codex_config_path,
        "configured_codex_home": resolved.configured_codex_home,
        "resolved_codex_home": resolved.home,
        "codex_home_source": resolved.codex_home_source,
        "discovery_warnings": resolved.discovery_warnings,
        "backup_path": if hooks_path.exists() { Some(backup_path) } else { None },
        "codex_config_backup_path": if codex_config_path.exists() { Some(codex_config_backup_path) } else { None },
        "hook_command": hook_command,
        "reload_result": Value::Null,
    }))
}

fn read_optional_text(path: &Path) -> Result<String> {
    if path.exists() {
        fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
    } else {
        Ok(String::new())
    }
}

fn ensure_codex_hooks_feature(text: &str) -> Result<String> {
    let mut value: toml::Value = if text.trim().is_empty() {
        toml::Value::Table(toml::map::Map::new())
    } else {
        toml::from_str(text).context("parse Codex config.toml")?
    };
    let root = value
        .as_table_mut()
        .context("Codex config.toml root must be a table")?;
    let features = root
        .entry("features")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    if !features.is_table() {
        *features = toml::Value::Table(toml::map::Map::new());
    }
    let features = features
        .as_table_mut()
        .context("Codex config features must be a table")?;
    if matches!(features.get("hooks"), Some(toml::Value::Boolean(true))) {
        return Ok(text.to_string());
    }
    features.insert("hooks".to_string(), toml::Value::Boolean(true));
    toml::to_string_pretty(&value).context("serialize Codex config.toml")
}

fn read_hooks_json(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({"hooks": {}}));
    }
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

fn ensure_stop_hook(root: &mut Value, hook_command: &str) -> bool {
    if !root.is_object() {
        *root = json!({"hooks": {}});
    }
    let object = root.as_object_mut().expect("object initialized");
    let hooks = object.entry("hooks").or_insert_with(|| json!({}));
    if !hooks.is_object() {
        *hooks = json!({});
    }
    let hooks_object = hooks.as_object_mut().expect("hooks object initialized");
    let stop = hooks_object
        .entry("Stop")
        .or_insert_with(|| Value::Array(Vec::new()));
    if !stop.is_array() {
        *stop = Value::Array(Vec::new());
    }
    let groups = stop.as_array_mut().expect("stop array initialized");
    for group in groups.iter_mut() {
        let Some(items) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
            continue;
        };
        if items.iter().any(|item| {
            item.get("command")
                .and_then(Value::as_str)
                .is_some_and(|command| command == hook_command)
        }) {
            return false;
        }
        items.retain(|item| {
            !item
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(|command| command.contains("codex-sentinel"))
        });
    }
    groups.push(json!({
        "matcher": "*",
        "hooks": [{
            "type": "command",
            "command": hook_command
        }]
    }));
    true
}

#[derive(Debug, Clone, Serialize)]
struct HookStopResult {
    stdout: Value,
    outcome: ProbeEventOutcome,
    bark: ProbeBarkOutcome,
}

#[derive(Debug, Clone)]
struct ProbeBarkRequest {
    title: String,
    body: String,
    dedupe_key: String,
}

#[derive(Debug, Deserialize)]
struct BarkPushResponse {
    code: Option<i64>,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ProbeBarkOutcome {
    sent: bool,
    skipped: bool,
    reason: Option<String>,
    http_status: Option<u16>,
    server_url: Option<String>,
    request_url: Option<String>,
    request_count: usize,
    chunk_count: usize,
    notifications_enabled: bool,
    relevant_switch_enabled: bool,
    device_key_configured: bool,
    dedupe_hit: bool,
    dedupe_key: Option<String>,
}

impl ProbeBarkOutcome {
    fn sent(
        status: u16,
        notifications_enabled: bool,
        relevant_switch_enabled: bool,
        device_key_configured: bool,
        dedupe_key: Option<String>,
    ) -> Self {
        Self {
            sent: true,
            skipped: false,
            reason: None,
            http_status: Some(status),
            server_url: None,
            request_url: None,
            request_count: 0,
            chunk_count: 0,
            notifications_enabled,
            relevant_switch_enabled,
            device_key_configured,
            dedupe_hit: false,
            dedupe_key,
        }
    }

    fn skipped(
        reason: &str,
        notifications_enabled: bool,
        relevant_switch_enabled: bool,
        device_key_configured: bool,
    ) -> Self {
        Self {
            sent: false,
            skipped: true,
            reason: Some(reason.to_string()),
            http_status: None,
            server_url: None,
            request_url: None,
            request_count: 0,
            chunk_count: 0,
            notifications_enabled,
            relevant_switch_enabled,
            device_key_configured,
            dedupe_hit: reason == "dedupe",
            dedupe_key: None,
        }
    }

    fn failed_status(
        status: u16,
        notifications_enabled: bool,
        relevant_switch_enabled: bool,
        device_key_configured: bool,
        dedupe_key: Option<String>,
    ) -> Self {
        Self {
            sent: false,
            skipped: false,
            reason: Some("http_status".to_string()),
            http_status: Some(status),
            server_url: None,
            request_url: None,
            request_count: 0,
            chunk_count: 0,
            notifications_enabled,
            relevant_switch_enabled,
            device_key_configured,
            dedupe_hit: false,
            dedupe_key,
        }
    }

    fn failed_request(
        reason: &str,
        notifications_enabled: bool,
        relevant_switch_enabled: bool,
        device_key_configured: bool,
        dedupe_key: Option<String>,
    ) -> Self {
        Self {
            sent: false,
            skipped: false,
            reason: Some(reason.to_string()),
            http_status: None,
            server_url: None,
            request_url: None,
            request_count: 0,
            chunk_count: 0,
            notifications_enabled,
            relevant_switch_enabled,
            device_key_configured,
            dedupe_hit: false,
            dedupe_key,
        }
    }

    fn with_delivery_metadata(
        mut self,
        server_url: &str,
        request_count: usize,
        chunk_count: usize,
    ) -> Self {
        self.server_url = Some(server_url.to_string());
        self.request_url = (request_count > 0).then(|| "[redacted]".to_string());
        self.request_count = request_count;
        self.chunk_count = chunk_count;
        self
    }
}

async fn handle_built_probe_event(
    config: &Config,
    db: &PanelDb,
    event: nexushub_core::probe::ProbeBuiltEvent,
) -> Result<HookStopResult> {
    let (outcome, bark) = record_probe_event_with_bark(config, db, event).await?;
    Ok(HookStopResult {
        stdout: codex_stop_continue_output(),
        outcome,
        bark,
    })
}

fn hook_stop_cli_output(result: &HookStopResult) -> Result<(String, String)> {
    let stdout = format!("{}\n", serde_json::to_string(&result.stdout)?);
    let stderr = format!(
        "{}\n",
        serde_json::to_string(&json!({
            "probe_event": result.outcome,
            "bark": result.bark,
        }))?
    );
    Ok((stdout, stderr))
}

async fn record_probe_event_with_bark(
    config: &Config,
    db: &PanelDb,
    mut event: nexushub_core::probe::ProbeBuiltEvent,
) -> Result<(ProbeEventOutcome, ProbeBarkOutcome)> {
    normalize_probe_event_dedupe_key(&mut event);
    if passive_unresolved_action_sent(db, &event)? {
        let mut outcome = ProbeEventOutcome::from_claim(&event, false);
        outcome.recorded = false;
        outcome.duplicate = true;
        let relevant_switch_enabled = probe_event_bark_switch_enabled(config, &event.kind);
        let device_key = db.get_secret_setting_bytes("probe_bark_device_key")?;
        let configured = device_key.as_ref().is_some_and(|value| !value.is_empty());
        return Ok((
            outcome,
            ProbeBarkOutcome::skipped(
                "sent_marker",
                config.probe.notifications.enabled,
                relevant_switch_enabled,
                configured,
            ),
        ));
    }
    let claimed = db.claim_probe_dedupe(
        &event.dedupe_namespace,
        &event.dedupe_key,
        event.ttl_seconds,
    )?;
    let mut outcome = ProbeEventOutcome::from_claim(&event, claimed);
    let bark = handle_probe_event_bark(config, db, &event, claimed).await?;
    if claimed {
        let mut payload = event.payload.clone();
        merge_bark_outcome_payload(&mut payload, &bark)?;
        if payload["bark"]["chunk_count"].is_null() {
            let chunk_count = bark_body_chunks(&event.bark_body, PROBE_BARK_BODY_CHUNK_BYTES).len();
            payload["bark"]["chunk_count"] = json!(chunk_count);
        }
        payload["dedupe"] = json!({
            "namespace": &event.dedupe_namespace,
            "key": &event.dedupe_key,
            "claimed": claimed,
            "duplicate": !claimed,
            "status": if claimed { "claimed" } else { "duplicate" },
        });
        payload["bark_status"] = json!(probe_bark_status_label(&bark));
        payload["dedupe_status"] = json!(if claimed { "claimed" } else { "duplicate" });
        db.record_probe_event(NewProbeEvent {
            kind: &event.kind,
            thread_id: event.thread_id.as_deref(),
            title: Some(&event.title),
            message: Some(&event.message),
            dedupe_key: Some(&event.dedupe_key),
            source: &event.source,
            payload,
        })?;
        mark_passive_unresolved_action_sent(db, &event)?;
    } else {
        outcome.recorded = false;
        outcome.duplicate = true;
    }

    Ok((outcome, bark))
}

fn normalize_probe_event_dedupe_key(event: &mut nexushub_core::probe::ProbeBuiltEvent) {
    if event.kind != "reply-needed" {
        return;
    }
    match event.payload.get("body_source").and_then(Value::as_str) {
        Some("proposed_plan") => normalize_proposed_plan_dedupe_key(event),
        Some("request_user_input") => normalize_request_user_input_dedupe_key(event),
        _ => {}
    }
}

fn normalize_proposed_plan_dedupe_key(event: &mut nexushub_core::probe::ProbeBuiltEvent) {
    let thread_id = event
        .thread_id
        .as_deref()
        .or_else(|| event.payload.get("thread_id").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string();
    let turn_id = event
        .turn_id
        .as_deref()
        .or_else(|| event.payload.get("turn_id").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string();
    let item_or_call_id = event
        .payload
        .get("item_id")
        .and_then(Value::as_str)
        .or_else(|| event.payload.get("call_id").and_then(Value::as_str))
        .unwrap_or(turn_id.as_str())
        .to_string();
    let plan_hash = proposed_plan_hash_from_text(&event.bark_body).unwrap_or_else(|| {
        event
            .payload
            .get("body_sha256")
            .and_then(Value::as_str)
            .and_then(|value| value.get(..16))
            .unwrap_or("unknown")
            .to_string()
    });
    event.dedupe_key = format!(
        "{}:{}:{}:{}:{}",
        dedupe_component(&event.kind),
        dedupe_component(&thread_id),
        dedupe_component(&turn_id),
        dedupe_component(&item_or_call_id),
        dedupe_component(&format!("plan_hash:{plan_hash}")),
    );
    event.payload["dedupe_plan_hash"] = json!(plan_hash);
    event.payload["dedupe_item_or_call_id"] = json!(item_or_call_id);
}

fn normalize_request_user_input_dedupe_key(event: &mut nexushub_core::probe::ProbeBuiltEvent) {
    let thread_id = event
        .thread_id
        .as_deref()
        .or_else(|| event.payload.get("thread_id").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string();
    let turn_id = event
        .turn_id
        .as_deref()
        .or_else(|| event.payload.get("turn_id").and_then(Value::as_str))
        .unwrap_or("unknown")
        .to_string();
    let call_id = event
        .payload
        .get("call_id")
        .and_then(Value::as_str)
        .or_else(|| event.payload.get("item_id").and_then(Value::as_str))
        .unwrap_or(turn_id.as_str())
        .to_string();
    let input_hash = request_user_input_hash_from_text(&event.bark_body).unwrap_or_else(|| {
        event
            .payload
            .get("body_sha256")
            .and_then(Value::as_str)
            .and_then(|value| value.get(..16))
            .unwrap_or("unknown")
            .to_string()
    });
    event.dedupe_key = format!(
        "{}:{}:{}:{}:{}",
        dedupe_component(&event.kind),
        dedupe_component(&thread_id),
        dedupe_component(&turn_id),
        dedupe_component(&call_id),
        dedupe_component(&format!("input_hash:{input_hash}")),
    );
    event.payload["dedupe_input_hash"] = json!(input_hash);
    event.payload["dedupe_item_or_call_id"] = json!(call_id);
}

fn passive_unresolved_action_sent(
    db: &PanelDb,
    event: &nexushub_core::probe::ProbeBuiltEvent,
) -> Result<bool> {
    let Some(key) = passive_unresolved_action_marker_key(event) else {
        return Ok(false);
    };
    Ok(db.get_setting(&key)?.is_some())
}

fn mark_passive_unresolved_action_sent(
    db: &PanelDb,
    event: &nexushub_core::probe::ProbeBuiltEvent,
) -> Result<()> {
    let Some(key) = passive_unresolved_action_marker_key(event) else {
        return Ok(());
    };
    db.set_setting(
        &key,
        &json!({
            "dedupe_key": event.dedupe_key,
            "thread_id": event.thread_id,
            "turn_id": event.turn_id,
            "body_source": event.payload.get("body_source").and_then(Value::as_str),
            "body_sha256": event.payload.get("body_sha256").and_then(Value::as_str),
        })
        .to_string(),
    )
}

fn passive_unresolved_action_marker_key(
    event: &nexushub_core::probe::ProbeBuiltEvent,
) -> Option<String> {
    if event.kind != "reply-needed" {
        return None;
    }
    if event.payload.get("scan_source").and_then(Value::as_str) != Some("passive-scan") {
        return None;
    }
    let body_source = event.payload.get("body_source").and_then(Value::as_str)?;
    if !matches!(body_source, "proposed_plan" | "request_user_input") {
        return None;
    }
    let thread_id = event
        .thread_id
        .as_deref()
        .or_else(|| event.payload.get("thread_id").and_then(Value::as_str))
        .unwrap_or("unknown");
    let turn_id = event
        .turn_id
        .as_deref()
        .or_else(|| event.payload.get("turn_id").and_then(Value::as_str))
        .unwrap_or("unknown");
    let action_id = event
        .payload
        .get("item_id")
        .and_then(Value::as_str)
        .or_else(|| event.payload.get("call_id").and_then(Value::as_str))
        .unwrap_or(turn_id);
    let content_key = if body_source == "proposed_plan" {
        proposed_plan_hash_from_text(&event.bark_body)
            .or_else(|| {
                event
                    .payload
                    .get("body_sha256")
                    .and_then(Value::as_str)
                    .and_then(|value| value.get(..16))
                    .map(ToString::to_string)
            })
            .unwrap_or_else(|| "unknown".to_string())
    } else {
        request_user_input_hash_from_text(&event.bark_body).unwrap_or_else(|| {
            event
                .payload
                .get("body_sha256")
                .and_then(Value::as_str)
                .and_then(|value| value.get(..16))
                .unwrap_or("unknown")
                .to_string()
        })
    };
    Some(format!(
        "{}{}:{}:{}:{}:{}:{}",
        PROBE_PASSIVE_SENT_MARKER_PREFIX,
        dedupe_component(body_source),
        dedupe_component(thread_id),
        dedupe_component(turn_id),
        dedupe_component(action_id),
        dedupe_component(&content_key),
        dedupe_component(&event.kind),
    ))
}

fn merge_bark_outcome_payload(payload: &mut Value, bark: &ProbeBarkOutcome) -> Result<()> {
    let outcome = serde_json::to_value(bark)?;
    if let Some(existing) = payload.get_mut("bark").and_then(Value::as_object_mut) {
        if let Some(outcome_object) = outcome.as_object() {
            for (key, value) in outcome_object {
                existing.insert(key.clone(), value.clone());
            }
        }
    } else {
        payload["bark"] = outcome;
    }
    Ok(())
}

async fn handle_probe_event_bark(
    config: &Config,
    db: &PanelDb,
    event: &nexushub_core::probe::ProbeBuiltEvent,
    claimed: bool,
) -> Result<ProbeBarkOutcome> {
    let relevant_switch_enabled = probe_event_bark_switch_enabled(config, &event.kind);
    if !config.probe.notifications.enabled {
        return Ok(ProbeBarkOutcome::skipped(
            "notifications_disabled",
            false,
            relevant_switch_enabled,
            false,
        ));
    }
    if !relevant_switch_enabled {
        return Ok(ProbeBarkOutcome::skipped(
            "event_switch_disabled",
            true,
            false,
            false,
        ));
    }
    let device_key = db.get_secret_setting_bytes("probe_bark_device_key")?;
    let configured = device_key.as_ref().is_some_and(|value| !value.is_empty());
    if !configured {
        return Ok(ProbeBarkOutcome::skipped(
            "device_key_missing",
            true,
            true,
            false,
        ));
    }
    if !claimed {
        return Ok(ProbeBarkOutcome::skipped("dedupe", true, true, true));
    }
    send_bark_notification(
        config,
        device_key.as_deref().unwrap_or_default(),
        &ProbeBarkRequest {
            title: event.bark_title.clone(),
            body: event.bark_body.clone(),
            dedupe_key: event.dedupe_key.clone(),
        },
        std::time::Duration::from_secs(8),
    )
    .await
}

fn probe_event_bark_switch_enabled(config: &Config, kind: &str) -> bool {
    match kind {
        "completion" => config.probe.notifications.notify_completion,
        "reply-needed" => config.probe.notifications.notify_reply_needed,
        "recoverable" => config.probe.notifications.notify_recoverable,
        _ => config.probe.notifications.notify_completion,
    }
}

fn probe_bark_status_label(bark: &ProbeBarkOutcome) -> &'static str {
    if bark.sent {
        "sent"
    } else if bark.skipped && bark.reason.as_deref() == Some("dedupe") {
        "dedupe_hit"
    } else if bark.skipped {
        "skipped"
    } else {
        "failed"
    }
}

fn codex_stop_continue_output() -> Value {
    json!({
        "continue": true,
        "suppressOutput": false,
    })
}

async fn send_bark_notification(
    config: &Config,
    device_key: &[u8],
    request: &ProbeBarkRequest,
    timeout: std::time::Duration,
) -> Result<ProbeBarkOutcome> {
    let device_key = match std::str::from_utf8(device_key) {
        Ok(value) => value,
        Err(err) => {
            tracing::warn!("Bark device_key is not utf-8: {err}");
            return Ok(ProbeBarkOutcome::failed_request(
                "invalid_device_key_encoding",
                config.probe.notifications.enabled,
                true,
                true,
                Some(request.dedupe_key.clone()),
            ));
        }
    };
    let server_url = config.probe.notifications.server_url.trim();
    let server_url = if server_url.is_empty() {
        "https://api.day.app"
    } else {
        server_url
    };
    if !valid_probe_notification_server_url(server_url) {
        tracing::warn!("Bark notification server URL rejected by Probe policy");
        return Ok(ProbeBarkOutcome::failed_request(
            "invalid_server_url",
            config.probe.notifications.enabled,
            true,
            true,
            Some(request.dedupe_key.clone()),
        ));
    }
    let base = if server_url.ends_with('/') {
        server_url.to_string()
    } else {
        format!("{server_url}/")
    };
    let push_url = match reqwest::Url::parse(&base).and_then(|url| url.join("push")) {
        Ok(url) => url,
        Err(err) => {
            tracing::warn!("Bark notification push URL build failed: {err}");
            return Ok(ProbeBarkOutcome::failed_request(
                "invalid_server_url",
                config.probe.notifications.enabled,
                true,
                true,
                Some(request.dedupe_key.clone()),
            ));
        }
    };
    let client = match reqwest::Client::builder().timeout(timeout).build() {
        Ok(client) => client,
        Err(err) => {
            tracing::warn!("Bark notification client build failed: {err}");
            return Ok(ProbeBarkOutcome::failed_request(
                "client_build_error",
                config.probe.notifications.enabled,
                true,
                true,
                Some(request.dedupe_key.clone()),
            ));
        }
    };
    let chunks = bark_body_chunks(&request.body, PROBE_BARK_BODY_CHUNK_BYTES);
    let chunk_count = chunks.len();
    let mut last_status = None;
    let mut request_count = 0usize;
    for (index, chunk) in chunks.iter().enumerate() {
        let chunk_title = if chunk_count > 1 {
            format!("{} ({}/{})", request.title, index + 1, chunk_count)
        } else {
            request.title.clone()
        };
        let payload = json!({
            "device_key": device_key.trim(),
            "title": chunk_title,
            "body": chunk,
        });
        let response = client.post(push_url.clone()).json(&payload).send().await;
        request_count += 1;
        let response = match response {
            Ok(response) => response,
            Err(err) => {
                let reason = if err.is_timeout() {
                    "timeout"
                } else {
                    "request_error"
                };
                tracing::warn!("Bark notification request failed: {reason}");
                return Ok(ProbeBarkOutcome::failed_request(
                    reason,
                    config.probe.notifications.enabled,
                    true,
                    true,
                    Some(request.dedupe_key.clone()),
                )
                .with_delivery_metadata(server_url, request_count, chunk_count));
            }
        };
        let status = response.status().as_u16();
        last_status = Some(status);
        if !response.status().is_success() {
            return Ok(ProbeBarkOutcome::failed_status(
                status,
                config.probe.notifications.enabled,
                true,
                true,
                Some(request.dedupe_key.clone()),
            )
            .with_delivery_metadata(server_url, request_count, chunk_count));
        }
        let bark_response = match response.json::<BarkPushResponse>().await {
            Ok(response) => response,
            Err(err) => {
                tracing::warn!("Bark notification response decode failed: {err}");
                return Ok(ProbeBarkOutcome::failed_request(
                    "response_decode",
                    config.probe.notifications.enabled,
                    true,
                    true,
                    Some(request.dedupe_key.clone()),
                )
                .with_delivery_metadata(server_url, request_count, chunk_count));
            }
        };
        if bark_response.code != Some(200) {
            if let Some(message) = bark_response.message.as_deref() {
                tracing::warn!("Bark notification rejected: {message}");
            }
            let mut outcome = ProbeBarkOutcome::failed_request(
                "bark_response_code",
                config.probe.notifications.enabled,
                true,
                true,
                Some(request.dedupe_key.clone()),
            )
            .with_delivery_metadata(server_url, request_count, chunk_count);
            outcome.http_status = Some(status);
            return Ok(outcome);
        }
    }
    Ok(ProbeBarkOutcome::sent(
        last_status.unwrap_or(0),
        config.probe.notifications.enabled,
        true,
        true,
        Some(request.dedupe_key.clone()),
    )
    .with_delivery_metadata(server_url, request_count, chunk_count))
}

fn utf8_chunks(value: &str, max_bytes: usize) -> Vec<String> {
    if value.is_empty() || max_bytes == 0 {
        return vec![String::new()];
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < value.len() {
        let mut end = (start + max_bytes).min(value.len());
        while end > start && !value.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            end = value[start..]
                .char_indices()
                .nth(1)
                .map(|(offset, _)| start + offset)
                .unwrap_or(value.len());
        }
        chunks.push(value[start..end].to_string());
        start = end;
    }
    chunks
}

fn bark_body_chunks(value: &str, max_bytes: usize) -> Vec<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || max_bytes == 0 {
        return vec![String::new()];
    }
    let raw_chunks = utf8_chunks(trimmed, max_bytes);
    let chunk_count = raw_chunks.len();
    if chunk_count <= 1 {
        return raw_chunks;
    }
    raw_chunks
        .into_iter()
        .enumerate()
        .map(|(index, chunk)| format!("{}{chunk}", bark_chunk_prefix(index + 1, chunk_count)))
        .collect()
}

fn bark_chunk_prefix(index: usize, chunk_count: usize) -> String {
    format!("第 {index}/{chunk_count} 段\n\n")
}

fn dedupe_component(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if normalized.is_empty() {
        "unknown".to_string()
    } else {
        normalized
    }
}

fn proposed_plan_hash_from_text(value: &str) -> Option<String> {
    let plan = nexushub_core::codex::extract_proposed_plan_text(value)
        .or_else(|| {
            value
                .split_once("Plan 摘要:")
                .map(|(_, plan)| plan.trim().to_string())
                .filter(|plan| !plan.is_empty())
        })
        .or_else(|| {
            value
                .split_once("待回复内容：")
                .map(|(_, plan)| plan.trim().to_string())
                .filter(|plan| !plan.is_empty())
        })?;
    Some(stable_hash64_hex(plan.trim()))
}

fn request_user_input_hash_from_text(value: &str) -> Option<String> {
    let body = value
        .split_once("待回复内容：")
        .map(|(_, body)| body)
        .unwrap_or(value);
    let normalized = body
        .lines()
        .map(str::trim)
        .filter(|line| {
            !line.starts_with("时间：")
                && !line.starts_with("时间:")
                && !line.starts_with("事件时间：")
                && !line.starts_with("事件时间:")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let normalized = normalized.trim();
    (!normalized.is_empty()).then(|| stable_hash64_hex(normalized))
}

fn stable_hash64_hex(value: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacySentinelConfig {
    server: LegacySentinelServerSection,
    bark: LegacySentinelBarkSection,
    logs_db: LegacySentinelLogsDbSection,
    observability: LegacySentinelObservabilitySection,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacySentinelServerSection {
    host_label: String,
    codex_home: Option<PathBuf>,
    app_server_service: String,
    poll_seconds: Option<u64>,
    recent_limit: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacySentinelBarkSection {
    enabled: Option<bool>,
    server_url: String,
    device_key: String,
    sound: String,
    group: String,
    url: String,
    notify_completion: Option<bool>,
    notify_abnormal: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacySentinelLogsDbSection {
    enabled: Option<bool>,
    retention_days: Option<u32>,
    maintenance_interval_hours: Option<u32>,
    maintain_on_codex_exit: Option<bool>,
    codex_exit_grace_seconds: Option<u64>,
    codex_exit_max_wait_seconds: Option<u64>,
    delete_chunk_rows: Option<u32>,
    max_delete_rows_per_run: Option<u32>,
    busy_timeout_ms: Option<u64>,
    auto_compact_when_codex_closed: Option<bool>,
    compact_interval_hours: Option<u32>,
    compact_min_freelist_mb: Option<u64>,
    compact_min_freelist_ratio_percent: Option<u32>,
    minimum_free_space_mb: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacySentinelObservabilitySection {
    hook_event_max_lines: Option<usize>,
    hook_cooldown_max_lines: Option<usize>,
    log_max_bytes: Option<usize>,
}

fn import_legacy_sentinel_config(db: &PanelDb) -> Result<Value> {
    let legacy_path = PathBuf::from("/etc/codex-sentinel-server/config.toml");
    let config_path = std::env::var_os("NEXUSHUB_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PlatformPaths::current().config_file);
    import_legacy_sentinel_config_from_path(db, &legacy_path, &config_path)
}

fn import_legacy_sentinel_config_from_path(
    db: &PanelDb,
    legacy_path: &Path,
    config_path: &Path,
) -> Result<Value> {
    if !legacy_path.exists() {
        return Ok(json!({
            "ok": true,
            "action": "legacy_import",
            "imported": false,
            "legacy_config": legacy_path,
            "skip_reason": "legacy_config_missing",
        }));
    }

    let text = fs::read_to_string(legacy_path)
        .with_context(|| format!("read {}", legacy_path.display()))?;
    let legacy: LegacySentinelConfig =
        toml::from_str(&text).with_context(|| format!("parse {}", legacy_path.display()))?;
    let patch = legacy_sentinel_config_patch(&legacy);
    let current = fs::read_to_string(config_path)
        .with_context(|| format!("read {}", config_path.display()))?;
    let updated = patch_probe_config_toml(&current, &patch)?;
    fs::write(config_path, updated).with_context(|| format!("write {}", config_path.display()))?;

    let mut imported_secret = false;
    let device_key = legacy.bark.device_key.trim();
    if !device_key.is_empty() {
        db.set_secret_setting_bytes("probe_bark_device_key", device_key.as_bytes())?;
        imported_secret = true;
    }
    db.set_setting(
        "probe_legacy_import",
        &json!({
            "legacy_config": legacy_path,
            "config_path": config_path,
            "imported_bark_device_key": imported_secret,
            "imported_at": chrono::Utc::now().to_rfc3339(),
        })
        .to_string(),
    )?;

    Ok(json!({
        "ok": true,
        "action": "legacy_import",
        "imported": true,
        "legacy_config": legacy_path,
        "config_path": config_path,
        "imported_bark_device_key": imported_secret,
        "mapped": {
            "codex": ["home", "host_label"],
            "probe": ["enabled", "poll_seconds", "recent_limit", "notifications", "observability", "logs_db"],
        }
    }))
}

fn legacy_sentinel_config_patch(legacy: &LegacySentinelConfig) -> ProbeConfigFilePatch {
    let nonempty = |value: &str| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    };
    ProbeConfigFilePatch {
        codex: Some(CodexProbeConfigPatch {
            home: legacy
                .server
                .codex_home
                .as_ref()
                .map(|path| Some(path.to_string_lossy().to_string())),
            host_label: nonempty(&legacy.server.host_label),
            ..Default::default()
        }),
        probe: Some(ProbeSettingsPatch {
            enabled: Some(true),
            poll_seconds: legacy.server.poll_seconds,
            recent_limit: legacy.server.recent_limit,
            hooks: Some(ProbeHooksConfigPatch {
                manage_stop_hook: Some(true),
                reload_app_server_after_install: Some(true),
            }),
            notifications: Some(ProbeNotificationsConfigPatch {
                enabled: legacy.bark.enabled,
                server_url: nonempty(&legacy.bark.server_url),
                sound: Some(nonempty(&legacy.bark.sound)),
                group: nonempty(&legacy.bark.group),
                url: Some(nonempty(&legacy.bark.url)),
                notify_completion: legacy.bark.notify_completion,
                notify_reply_needed: legacy.bark.notify_completion,
                notify_recoverable: legacy.bark.notify_abnormal,
            }),
            observability: Some(ProbeObservabilityConfigPatch {
                hook_event_max_lines: legacy.observability.hook_event_max_lines,
                hook_cooldown_max_lines: legacy.observability.hook_cooldown_max_lines,
                log_max_bytes: legacy.observability.log_max_bytes,
            }),
            logs_db: Some(ProbeLogsDbConfigPatch {
                enabled: legacy.logs_db.enabled,
                retention_days: legacy.logs_db.retention_days,
                maintenance_interval_hours: legacy.logs_db.maintenance_interval_hours,
                maintain_on_codex_exit: legacy.logs_db.maintain_on_codex_exit,
                codex_exit_grace_seconds: legacy.logs_db.codex_exit_grace_seconds,
                codex_exit_max_wait_seconds: legacy.logs_db.codex_exit_max_wait_seconds,
                delete_chunk_rows: legacy.logs_db.delete_chunk_rows,
                max_delete_rows_per_run: legacy.logs_db.max_delete_rows_per_run,
                busy_timeout_ms: legacy.logs_db.busy_timeout_ms,
                auto_compact_when_codex_closed: legacy.logs_db.auto_compact_when_codex_closed,
                compact_interval_hours: legacy.logs_db.compact_interval_hours,
                compact_min_freelist_mb: legacy.logs_db.compact_min_freelist_mb,
                compact_min_freelist_ratio_percent: legacy
                    .logs_db
                    .compact_min_freelist_ratio_percent,
                minimum_free_space_mb: legacy.logs_db.minimum_free_space_mb,
            }),
        }),
    }
}

fn init_admin(db: PanelDb, username: &str, password: &str, allow_existing: bool) -> Result<()> {
    if password.len() < 12 {
        anyhow::bail!("password must be at least 12 characters");
    }
    if !allow_existing && db.admin_count()? > 0 {
        anyhow::bail!("admin already exists; use admin reset-password");
    }
    let hash = auth::hash_password(password)?;
    db.upsert_admin(&uuid::Uuid::new_v4().to_string(), username, &hash)?;
    println!("admin {} configured", username);
    Ok(())
}

async fn serve(config_path: PathBuf) -> Result<()> {
    let config = Config::load(&config_path)?;
    let db = open_panel_db(&config)?;
    let state = AppState::new(config.clone(), db);
    spawn_probe_logs_db_scheduler(state.clone());
    spawn_probe_thread_scan(state.clone());
    api::spawn_probe_status_refresh(state.clone());
    let webui_dir = config.paths.webui_dir.clone();
    let app = api::router(state)
        .fallback_service(ServeDir::new(webui_dir).append_index_html_on_directories(true))
        .layer(TraceLayer::new_for_http());
    let addr: SocketAddr = config.server.listen;
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {addr}"))?;
    tracing::info!("nexushub listening on {addr}");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

fn open_panel_db(config: &Config) -> Result<PanelDb> {
    PanelDb::open_with_secret_box(&config.paths.db_path, config.secret_box()?)
}

#[derive(Debug)]
struct ProbeLogsDbMaintenanceOutcome {
    ran: bool,
    result: Option<ProbeLogsDbMaintenanceResult>,
    skip_reason: Option<String>,
}

fn spawn_probe_logs_db_scheduler(state: AppState) {
    tokio::spawn(async move {
        loop {
            match run_probe_logs_db_maintenance_if_due(state.clone()).await {
                Ok(outcome) if outcome.ran => {
                    if let Some(result) = outcome.result {
                        tracing::info!(
                            target = result.target.as_str(),
                            deleted_rows = result.deleted_rows,
                            remaining_old_rows = result.remaining_old_rows,
                            "probe logs DB maintenance completed"
                        );
                    }
                }
                Ok(outcome) => {
                    tracing::debug!(
                        skip_reason = outcome.skip_reason.as_deref().unwrap_or("unknown"),
                        "probe logs DB maintenance skipped"
                    );
                }
                Err(err) => {
                    tracing::warn!("probe logs DB maintenance failed: {err}");
                }
            }
            time::sleep(std::time::Duration::from_secs(
                PROBE_LOGS_DB_SCHEDULER_TICK_SECONDS,
            ))
            .await;
        }
    });
}

fn spawn_probe_thread_scan(state: AppState) {
    tokio::spawn(async move {
        loop {
            match run_probe_thread_scan_if_due(state.clone()).await {
                Ok(count) if count > 0 => {
                    tracing::info!(count, "probe thread scan notifications recorded");
                }
                Ok(_) => {
                    tracing::debug!("probe thread scan skipped");
                }
                Err(err) => {
                    tracing::warn!("probe thread scan failed: {err}");
                }
            }
            time::sleep(std::time::Duration::from_secs(
                PROBE_THREAD_SCAN_TICK_SECONDS,
            ))
            .await;
        }
    });
}

async fn run_probe_logs_db_maintenance_if_due(
    state: AppState,
) -> Result<ProbeLogsDbMaintenanceOutcome> {
    let _guard = PROBE_LOGS_DB_MAINTENANCE_LOCK.lock().await;
    let config = state.config();
    if !config.probe.logs_db.enabled {
        return Ok(ProbeLogsDbMaintenanceOutcome {
            ran: false,
            result: None,
            skip_reason: Some("logs_db_disabled".to_string()),
        });
    }
    let interval_seconds =
        i64::from(config.probe.logs_db.maintenance_interval_hours.max(1)) * 3_600;
    if let Some((_raw, updated_at)) = state
        .db
        .get_setting_with_updated_at(PROBE_LOGS_DB_LAST_MAINTAIN_SETTING)?
    {
        if PanelDb::now().saturating_sub(updated_at) < interval_seconds {
            return Ok(ProbeLogsDbMaintenanceOutcome {
                ran: false,
                result: None,
                skip_reason: Some("not_due".to_string()),
            });
        }
    }

    let compact = probe_logs_db_compaction_due(&state.db, &config).await?;
    let worker_config = config.clone();
    let result = tokio::task::spawn_blocking(move || {
        probe_runtime(&worker_config).maintain_logs_db_with_compaction(false, compact)
    })
    .await
    .context("join probe logs DB maintenance worker")??;
    let (probe_events_deleted, probe_dedupe_deleted) = state.db.maintain_probe_events(
        config.probe.logs_db.retention_days,
        config.probe.logs_db.max_delete_rows_per_run,
        false,
    )?;
    let mut stored = serde_json::to_value(&result)?;
    add_probe_events_maintenance_fields(
        &mut stored,
        probe_events_deleted,
        probe_dedupe_deleted,
        false,
    );
    state.db.set_setting(
        PROBE_LOGS_DB_LAST_MAINTAIN_SETTING,
        &serde_json::to_string(&stored)?,
    )?;
    if result.vacuumed {
        state.db.set_setting(
            PROBE_LOGS_DB_LAST_COMPACT_SETTING,
            &serde_json::to_string(&result)?,
        )?;
    }
    Ok(ProbeLogsDbMaintenanceOutcome {
        ran: true,
        result: Some(result),
        skip_reason: None,
    })
}

async fn run_probe_thread_scan_if_due(state: AppState) -> Result<usize> {
    let _guard = PROBE_THREAD_SCAN_LOCK.lock().await;
    let config = state.config();
    if !config.probe.enabled || !config.probe.notifications.enabled {
        return Ok(0);
    }
    let mut recorded = 0usize;
    for status in ["reply-needed", "recoverable"] {
        let threads = api::load_probe_threads(&state, status, config.probe.recent_limit).await?;
        for thread in threads {
            let (body, body_source) = probe_thread_notification_body(&thread, status);
            if !probe_thread_passive_bark_fresh(&thread, status, body_source.as_deref()) {
                continue;
            }
            let transcript_path = thread
                .rollout_path
                .as_ref()
                .map(|path| path.to_string_lossy().to_string());
            let mut event = probe_runtime(&config).build_event(
                ProbeEventInput::hook_stop_with_context(
                    Some(thread.id.as_str()),
                    thread.active_turn_id.as_deref(),
                    Some(thread.id.as_str()),
                    transcript_path.as_deref(),
                    body.as_deref(),
                    status,
                )
                .with_thread_title(Some(thread.title.as_str()))
                .with_body_source(body_source.as_deref())
                .with_passive_scan_source(),
            );
            event.payload["thread_title"] = json!(thread.title.clone());
            event.payload["thread_id"] = json!(thread.id.clone());
            if let Some(active_turn_id) = thread.active_turn_id.as_deref() {
                event.payload["turn_id"] = json!(active_turn_id);
            }
            if let Some(elicitation) = &thread.pending_elicitation {
                if let Some(item_id) = elicitation.item_id.as_deref() {
                    event.payload["item_id"] = json!(item_id);
                    event.payload["call_id"] = json!(item_id);
                }
            }
            match status {
                "reply-needed" => {
                    event.payload["reason_label"] = json!("等待用户确认");
                }
                "recoverable" => {
                    event.payload["reason_label"] = json!("异常/可恢复");
                }
                _ => {}
            }
            let (outcome, bark) = record_probe_event_with_bark(&config, &state.db, event).await?;
            if outcome.recorded || bark.sent {
                recorded += 1;
            }
        }
    }
    Ok(recorded)
}

fn probe_thread_notification_body(
    thread: &nexushub_core::codex::ThreadSummary,
    status: &str,
) -> (Option<String>, Option<String>) {
    if status == "reply-needed" {
        if let Some(elicitation) = &thread.pending_elicitation {
            if !thread_rollout_still_request_user_input_needed(thread) {
                return (None, None);
            }
            return (
                Some(format_pending_elicitation(elicitation)),
                Some("request_user_input".to_string()),
            );
        }
        if let Some(message) = thread
            .latest_message
            .as_deref()
            .filter(|value| value.contains("<proposed_plan>") && value.contains("</proposed_plan>"))
        {
            if !thread_rollout_still_reply_needed(thread) {
                return (None, None);
            }
            return (
                Some(format_proposed_plan_reply_needed(
                    &thread.id,
                    thread.active_turn_id.as_deref().unwrap_or("-"),
                    message,
                )),
                Some("proposed_plan".to_string()),
            );
        }
        if let Some(path) = thread.rollout_path.as_deref() {
            if let Ok(Some(message)) =
                rollout_completion_last_agent_message(path, thread.active_turn_id.as_deref())
            {
                if message.contains("<proposed_plan>") && message.contains("</proposed_plan>") {
                    if !thread_rollout_still_reply_needed(thread) {
                        return (None, None);
                    }
                    return (
                        Some(format_proposed_plan_reply_needed(
                            &thread.id,
                            thread.active_turn_id.as_deref().unwrap_or("-"),
                            &message,
                        )),
                        Some("proposed_plan".to_string()),
                    );
                }
            }
        }
    }
    if status == "recoverable" {
        if let Some(message) = thread.latest_message.as_deref() {
            return (
                Some(message.to_string()),
                Some("latest_exception".to_string()),
            );
        }
    }
    (
        thread.latest_message.clone(),
        thread
            .latest_message
            .as_ref()
            .map(|_| "latest_message".to_string()),
    )
}

fn probe_thread_passive_bark_fresh(
    thread: &nexushub_core::codex::ThreadSummary,
    status: &str,
    body_source: Option<&str>,
) -> bool {
    if status != "reply-needed" {
        return true;
    }
    if !thread_updated_within(thread, PROBE_REPLY_NEEDED_FRESH_WINDOW_SECONDS) {
        return false;
    }
    match body_source {
        Some("request_user_input") => thread_rollout_still_request_user_input_needed(thread),
        Some("proposed_plan") => thread_rollout_still_reply_needed(thread),
        Some(_) | None => false,
    }
}

fn thread_updated_within(
    thread: &nexushub_core::codex::ThreadSummary,
    max_age_seconds: i64,
) -> bool {
    let Some(updated_at) = thread.updated_at.as_deref() else {
        return false;
    };
    let Ok(updated_at) = chrono::DateTime::parse_from_rfc3339(updated_at) else {
        return false;
    };
    let age_seconds = chrono::Utc::now()
        .signed_duration_since(updated_at.with_timezone(&chrono::Utc))
        .num_seconds();
    (0..=max_age_seconds).contains(&age_seconds)
}

fn thread_rollout_still_reply_needed(thread: &nexushub_core::codex::ThreadSummary) -> bool {
    if thread.rollout_path.is_none() {
        return true;
    }
    let mut refreshed = thread.clone();
    if nexushub_core::codex::enrich_thread_from_rollout(&mut refreshed).is_err() {
        return true;
    }
    let reply_needed = matches!(
        refreshed.status,
        nexushub_core::codex::ThreadStatus::ReplyNeeded
    );
    let has_plan_message = refreshed.latest_message.as_deref().is_some_and(|value| {
        value.contains("<proposed_plan>") && value.contains("</proposed_plan>")
    });
    let same_turn = refreshed
        .active_turn_id
        .as_deref()
        .is_some_and(|turn_id| Some(turn_id) == thread.active_turn_id.as_deref())
        || (thread.active_turn_id.is_none() && refreshed.active_turn_id.is_none());
    reply_needed && has_plan_message && same_turn
}

fn thread_rollout_still_request_user_input_needed(
    thread: &nexushub_core::codex::ThreadSummary,
) -> bool {
    if thread.rollout_path.is_none() {
        return thread.pending_elicitation.is_some();
    }
    let mut refreshed = thread.clone();
    if nexushub_core::codex::enrich_thread_from_rollout(&mut refreshed).is_err() {
        return false;
    }
    let reply_needed = matches!(
        refreshed.status,
        nexushub_core::codex::ThreadStatus::ReplyNeeded
    );
    let has_pending_elicitation = refreshed.pending_elicitation.is_some();
    let same_turn = refreshed
        .active_turn_id
        .as_deref()
        .is_some_and(|turn_id| Some(turn_id) == thread.active_turn_id.as_deref())
        || (thread.active_turn_id.is_none() && refreshed.active_turn_id.is_none());
    reply_needed && has_pending_elicitation && same_turn
}

fn format_pending_elicitation(elicitation: &nexushub_core::codex::PendingElicitation) -> String {
    let mut lines = Vec::new();
    for (index, question) in elicitation.questions.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        let number = index + 1;
        lines.push(format!("问题 {number}：{}", question.question.trim()));
        if let Some(header) = question
            .header
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            lines.push(format!("标题：{header}"));
        }
        for (option_index, option) in question.options.iter().enumerate() {
            let marker = option_index + 1;
            lines.push(format!("选项 {marker}：{}", option.label.trim()));
            if let Some(description) = option
                .description
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                lines.push(format!("说明：{description}"));
            }
        }
    }
    if lines.is_empty() {
        "Codex 请求用户输入。".to_string()
    } else {
        lines.join("\n")
    }
}

fn format_proposed_plan_reply_needed(_thread_id: &str, _turn_id: &str, raw: &str) -> String {
    let plan_text = nexushub_core::codex::extract_proposed_plan_text(raw)
        .unwrap_or_else(|| raw.trim().to_string());
    plan_text.trim().to_string()
}

fn add_probe_events_maintenance_fields(
    value: &mut Value,
    events: usize,
    dedupe: usize,
    dry_run: bool,
) {
    if let Value::Object(object) = value {
        object.insert(
            "probe_events_target".to_string(),
            Value::String("panel_probe_events".to_string()),
        );
        object.insert("probe_events_dry_run".to_string(), Value::Bool(dry_run));
        object.insert("probe_events_deleted".to_string(), json!(events));
        object.insert("probe_dedupe_deleted".to_string(), json!(dedupe));
    }
}

async fn probe_logs_db_compaction_due(db: &PanelDb, config: &Config) -> Result<bool> {
    if !config.probe.logs_db.auto_compact_when_codex_closed {
        return Ok(false);
    }
    let interval_seconds = i64::from(config.probe.logs_db.compact_interval_hours.max(1)) * 3_600;
    if let Some((_raw, updated_at)) =
        db.get_setting_with_updated_at(PROBE_LOGS_DB_LAST_COMPACT_SETTING)?
    {
        if PanelDb::now().saturating_sub(updated_at) < interval_seconds {
            return Ok(false);
        }
    }
    let _ = config;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::time::SystemTime;

    #[tokio::test]
    async fn unsupported_probe_cli_actions_do_not_report_fake_success() {
        let config = Config::default();

        for command in [ProbeCommand::LifecycleRepair, ProbeCommand::ServiceRestart] {
            let db = PanelDb::open(":memory:").unwrap();
            let err = run_probe_command(command, &config, db).await.unwrap_err();
            let message = format!("{err:#}");
            assert!(message.contains("unsupported"));
            assert!(!message.contains("\"ok\": true"));
        }
    }

    #[test]
    fn legacy_import_maps_server_bark_observability_and_logs_db_without_plaintext_secret() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("nexushub-legacy-import-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.toml");
        let legacy_path = dir.join("legacy.toml");
        let mut config = Config::default();
        config.security.secret_key = "7q9DCmCPyxnTrH3FhrV1sUJol1yqPgscQsBnR-mXA2E".to_string();
        fs::write(&config_path, toml::to_string_pretty(&config).unwrap()).unwrap();
        fs::write(
            &legacy_path,
            r#"
[server]
host_label = "tencent-wanka"
codex_home = "/root/.codex"
app_server_service = "codex-app-server-root.service"
poll_seconds = 60
recent_limit = 50

[bark]
enabled = true
server_url = "https://api.day.app"
device_key = "legacy-bark-secret"
sound = "bell"
group = "Codex"
url = "https://661313.xyz/nexushub/"
notify_completion = true
notify_abnormal = false

[logs_db]
enabled = true
retention_days = 2
maintenance_interval_hours = 6
maintain_on_codex_exit = true
codex_exit_grace_seconds = 0
codex_exit_max_wait_seconds = 1800
delete_chunk_rows = 5000
max_delete_rows_per_run = 100000
busy_timeout_ms = 500
auto_compact_when_codex_closed = true
compact_interval_hours = 24
compact_min_freelist_mb = 256
compact_min_freelist_ratio_percent = 20
minimum_free_space_mb = 1024

[observability]
hook_event_max_lines = 500
hook_cooldown_max_lines = 1000
log_max_bytes = 5242880
"#,
        )
        .unwrap();
        let db = PanelDb::open_with_secret_box(
            dir.join("nexushub.sqlite"),
            config.secret_box().unwrap(),
        )
        .unwrap();

        let result =
            import_legacy_sentinel_config_from_path(&db, &legacy_path, &config_path).unwrap();

        assert_eq!(result["imported"], true);
        assert_eq!(result["imported_bark_device_key"], true);
        let updated = fs::read_to_string(&config_path).unwrap();
        assert!(updated.contains("home = \"/root/.codex\""));
        assert!(updated.contains("host_label = \"tencent-wanka\""));
        assert!(updated.contains("poll_seconds = 60"));
        assert!(updated.contains("enabled = true"));
        assert!(updated.contains("sound = \"bell\""));
        assert!(updated.contains("notify_recoverable = false"));
        assert!(updated.contains("retention_days = 2"));
        assert!(updated.contains("busy_timeout_ms = 500"));
        assert!(updated.contains("hook_event_max_lines = 500"));
        assert!(updated.contains("log_max_bytes = 5242880"));
        assert!(!updated.contains("legacy-bark-secret"));
        assert!(!updated.contains("app_server_service"));
        assert!(!updated.contains("codex-app-server-root.service"));
        assert_eq!(
            db.get_secret_setting_bytes("probe_bark_device_key")
                .unwrap()
                .as_deref(),
            Some("legacy-bark-secret".as_bytes())
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn legacy_import_reports_missing_config_without_touching_current_config() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("nexushub-legacy-import-missing-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.toml");
        let legacy_path = dir.join("missing.toml");
        fs::write(&config_path, "sentinel = \"keep\"\n").unwrap();
        let db = PanelDb::open(dir.join("nexushub.sqlite")).unwrap();

        let result =
            import_legacy_sentinel_config_from_path(&db, &legacy_path, &config_path).unwrap();

        assert_eq!(result["imported"], false);
        assert_eq!(result["skip_reason"], "legacy_config_missing");
        assert_eq!(
            fs::read_to_string(&config_path).unwrap(),
            "sentinel = \"keep\"\n"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn codex_config_patch_enables_features_hooks_and_preserves_existing_values() {
        let updated = ensure_codex_hooks_feature(
            r#"
model = "gpt-5"

[features]
foo = true
hooks = false
"#,
        )
        .unwrap();

        assert!(updated.contains("hooks = true"));
        assert!(updated.contains("foo = true"));
        assert!(updated.contains("model = \"gpt-5\""));
    }

    #[test]
    fn codex_config_patch_creates_features_table_when_missing() {
        let updated = ensure_codex_hooks_feature("model = \"gpt-5\"\n").unwrap();

        assert!(updated.contains("[features]"));
        assert!(updated.contains("hooks = true"));
    }

    #[tokio::test]
    async fn install_probe_hooks_writes_hooks_json_and_codex_features_hooks() {
        let dir = temp_test_dir("nexushub-hooks-install");
        let codex_home = dir.join(".codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::create_dir_all(codex_home.join("sessions")).unwrap();
        fs::write(codex_home.join("config.toml"), "model = \"gpt-5\"\n").unwrap();
        let mut config = Config::default();
        config.codex.home = codex_home.clone();
        config.probe.hooks.reload_app_server_after_install = false;

        let result = install_probe_hooks(&config, false).await.unwrap();

        assert_eq!(result["ok"], true);
        assert_eq!(result["hooks_json_changed"], true);
        assert_eq!(result["codex_config_changed"], true);
        assert!(result["reload_result"].is_null());
        let hooks_json = fs::read_to_string(codex_home.join("hooks.json")).unwrap();
        assert!(hooks_json.contains("probe hook-stop"));
        let codex_config = fs::read_to_string(codex_home.join("config.toml")).unwrap();
        assert!(codex_config.contains("[features]"));
        assert!(codex_config.contains("hooks = true"));

        fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn hook_stop_records_probe_event_but_returns_codex_stop_json_and_redacted_bark_state() {
        let mut config = Config::default();
        config.probe.notifications.enabled = true;
        config.probe.notifications.notify_completion = true;
        config.probe.notifications.server_url = "http://127.0.0.1:9".to_string();
        let db = PanelDb::open(":memory:").unwrap();

        let event = probe_runtime(&config).build_event(ProbeEventInput::hook_stop(
            Some("thread-a"),
            Some("turn-1"),
            "hook-stop",
        ));
        let result = handle_built_probe_event(&config, &db, event).await.unwrap();

        assert_eq!(
            result.stdout,
            json!({"continue": true, "suppressOutput": false})
        );
        assert!(result.outcome.recorded);
        assert!(!result.bark.sent);
        assert!(result.bark.skipped);
        assert_eq!(result.bark.reason.as_deref(), Some("device_key_missing"));
        assert!(!result.bark.device_key_configured);
        let output = serde_json::to_string(&result).unwrap();
        assert!(!output.contains("device-key"));
        assert!(!output.contains("secret"));

        let events = db.list_probe_events(10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "hook-stop");
        assert_eq!(events[0].payload["bark"]["reason"], "device_key_missing");
        assert!(events[0].payload["bark"].get("device_key").is_none());
    }

    #[tokio::test]
    async fn hook_stop_records_stdin_compatible_context_fields() {
        let mut config = Config::default();
        config.probe.notifications.enabled = false;
        let db = PanelDb::open(":memory:").unwrap();
        let event = probe_runtime(&config).build_event(ProbeEventInput::hook_stop_with_context(
            None,
            Some("turn-stdin"),
            Some("session-stdin"),
            Some("/tmp/transcript.jsonl"),
            Some("assistant body"),
            "hook-stop",
        ));

        let result = handle_built_probe_event(&config, &db, event).await.unwrap();

        assert_eq!(
            result.stdout,
            json!({"continue": true, "suppressOutput": false})
        );
        let events = db.list_probe_events(10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].thread_id.as_deref(), Some("session-stdin"));
        assert_eq!(events[0].payload["session_id"], "session-stdin");
        assert_eq!(events[0].payload["turn_id"], "turn-stdin");
        assert_eq!(
            events[0].payload["transcript_path"],
            "/tmp/transcript.jsonl"
        );
        assert_eq!(
            events[0].payload["last_assistant_message"]["summary"],
            "assistant body"
        );
        assert_eq!(
            events[0].payload["last_assistant_message"]["classification"],
            "completion"
        );
        assert_eq!(events[0].payload["event_type"], "completion");
        assert_eq!(events[0].payload["raw_kind"], "hook-stop");
        assert_eq!(events[0].payload["dedupe"]["namespace"], "probe_event");
        assert_eq!(events[0].payload["dedupe"]["claimed"], true);
        assert_eq!(events[0].payload["dedupe"]["duplicate"], false);
        assert_eq!(events[0].kind, "completion");
    }

    #[tokio::test]
    async fn hook_stop_uses_transcript_latest_assistant_when_stdin_omits_body() {
        let mut config = Config::default();
        config.probe.notifications.enabled = false;
        let db = PanelDb::open(":memory:").unwrap();
        let dir = temp_test_dir("nexushub-hook-transcript-summary");
        fs::create_dir_all(&dir).unwrap();
        let transcript = dir.join("rollout.jsonl");
        fs::write(
            &transcript,
            [
                json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"first answer"}]}}).to_string(),
                json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"final answer"}]}}).to_string(),
            ]
            .join("\n"),
        )
        .unwrap();

        let event = probe_runtime(&config).build_event(ProbeEventInput::hook_stop_with_context(
            Some("thread-transcript"),
            Some("turn-transcript"),
            Some("session-transcript"),
            Some(transcript.to_string_lossy().as_ref()),
            rollout_latest_assistant_message(&transcript)
                .unwrap()
                .as_deref(),
            "hook-stop",
        ));

        handle_built_probe_event(&config, &db, event).await.unwrap();

        let events = db.list_probe_events(10).unwrap();
        assert_eq!(
            events[0].payload["last_assistant_message"]["summary"],
            "final answer"
        );
        assert_eq!(
            events[0].payload["last_assistant_message"]["classification"],
            "completion"
        );
        assert_eq!(events[0].payload["body_summary"], "final answer");
        fs::remove_dir_all(dir).unwrap();
    }

    #[tokio::test]
    async fn hook_stop_dedupe_skips_duplicate_bark_without_leaking_device_key() {
        let mut config = Config::default();
        config.probe.notifications.enabled = true;
        config.probe.notifications.server_url = "http://127.0.0.1:9".to_string();
        let db = PanelDb::open(":memory:").unwrap();
        db.set_secret_setting_bytes("probe_bark_device_key", b"super-secret-device")
            .unwrap();

        let first = handle_built_probe_event(
            &config,
            &db,
            probe_runtime(&config).build_event(ProbeEventInput::hook_stop(
                Some("thread-a"),
                Some("turn-1"),
                "hook-stop",
            )),
        )
        .await
        .unwrap();
        let duplicate = handle_built_probe_event(
            &config,
            &db,
            probe_runtime(&config).build_event(ProbeEventInput::hook_stop(
                Some("thread-a"),
                Some("turn-1"),
                "hook-stop",
            )),
        )
        .await
        .unwrap();

        assert_eq!(first.bark.reason.as_deref(), Some("request_error"));
        assert_eq!(
            duplicate.stdout,
            json!({"continue": true, "suppressOutput": false})
        );
        assert!(!duplicate.outcome.recorded);
        assert!(!duplicate.bark.sent);
        assert!(duplicate.bark.skipped);
        assert_eq!(duplicate.bark.reason.as_deref(), Some("dedupe"));
        assert!(duplicate.bark.device_key_configured);
        assert!(duplicate.bark.dedupe_hit);
        assert!(!serde_json::to_string(&duplicate)
            .unwrap()
            .contains("super-secret-device"));

        let events = db.list_probe_events(10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload["bark"]["device_key_configured"], true);
        assert!(events[0].payload["bark"].get("device_key").is_none());
    }

    #[tokio::test]
    async fn passive_reply_needed_scan_uses_sent_marker_while_hook_window_stays_short() {
        let mut config = Config::default();
        config.probe.notifications.enabled = true;
        config.probe.notifications.server_url = "http://127.0.0.1:9".to_string();
        let db = PanelDb::open(":memory:").unwrap();
        db.set_secret_setting_bytes("probe_bark_device_key", b"super-secret-device")
            .unwrap();

        let passive_input = || {
            ProbeEventInput::hook_stop_with_context(
                Some("thread-passive"),
                Some("turn-passive"),
                Some("thread-passive"),
                None,
                Some("等待用户选择：Plan Mode 已请求用户选择后继续。\n\nCall ID：call-passive\nTurn ID：turn-passive\n时间：2026-06-16 12:00:00 北京时间\n状态说明：这一轮正在等待用户选择，不是异常停止。\n\n待选择内容：\n问题 1：继续吗？\n选项 1：继续"),
                "reply-needed",
            )
            .with_body_source(Some("request_user_input"))
            .with_passive_scan_source()
        };

        let first = record_probe_event_with_bark(
            &config,
            &db,
            probe_runtime(&config).build_event(passive_input()),
        )
        .await
        .unwrap();
        let duplicate = record_probe_event_with_bark(
            &config,
            &db,
            probe_runtime(&config).build_event(passive_input()),
        )
        .await
        .unwrap();

        assert!(first.0.recorded);
        assert!(!duplicate.0.recorded);
        assert_eq!(duplicate.1.reason.as_deref(), Some("sent_marker"));
        let events = db.list_probe_events(10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload["dedupe_ttl_seconds"], 6 * 60 * 60);
        assert_eq!(events[0].payload["scan_source"], "passive-scan");

        let hook = probe_runtime(&config).build_event(ProbeEventInput::hook_stop_with_context(
            Some("thread-hook"),
            Some("turn-hook"),
            Some("thread-hook"),
            None,
            Some("等待用户选择：Plan Mode 已请求用户选择后继续。\n\nCall ID：call-hook\nTurn ID：turn-hook"),
            "reply-needed",
        ));

        assert_eq!(
            hook.ttl_seconds,
            nexushub_core::probe::PROBE_EVENT_TTL_SECONDS
        );
        assert_eq!(
            hook.payload["dedupe_ttl_seconds"],
            nexushub_core::probe::PROBE_EVENT_TTL_SECONDS
        );
        assert_eq!(hook.payload["scan_source"], "hook-stop");
    }

    #[tokio::test]
    async fn notify_completion_uses_completion_bark_switch() {
        let mut config = Config::default();
        config.probe.notifications.enabled = true;
        config.probe.notifications.notify_completion = false;
        let db = PanelDb::open(":memory:").unwrap();
        let event = probe_runtime(&config).build_event(ProbeEventInput::notify_completion(
            Some("thread-a"),
            Some("turn-a"),
        ));

        let (_outcome, bark) = record_probe_event_with_bark(&config, &db, event)
            .await
            .unwrap();

        assert!(bark.skipped);
        assert_eq!(bark.reason.as_deref(), Some("event_switch_disabled"));
        let events = db.list_probe_events(10).unwrap();
        assert_eq!(events[0].kind, "completion");
        assert_eq!(events[0].payload["bark"]["relevant_switch_enabled"], false);
    }

    #[tokio::test]
    async fn notify_completion_uses_complete_last_agent_message_from_rollout_without_storing_body()
    {
        let mut config = Config::default();
        config.probe.notifications.enabled = false;
        let db = PanelDb::open(":memory:").unwrap();
        let dir = temp_test_dir("nexushub-notify-completion-rollout");
        fs::create_dir_all(&dir).unwrap();
        let transcript = dir.join("rollout.jsonl");
        let final_message = format!(
            "最终反馈第一行\n{}\nAuthorization: Bearer secret-token\n末尾唯一完整反馈",
            "完整正文".repeat(900)
        );
        fs::write(
            &transcript,
            [
                json!({"type":"response_item","turn_id":"turn-complete","payload":{"type":"message","role":"assistant","content":[{"text":"short assistant fallback"}]}}).to_string(),
                json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-complete","last_agent_message":final_message}}).to_string(),
            ]
            .join("\n"),
        )
        .unwrap();

        let context = notify_completion_context(
            &config,
            Some(&json!({
                "thread_id": "thread-complete",
                "turn_id": "turn-complete",
                "transcript_path": transcript,
            })),
            None,
            None,
        )
        .unwrap();
        let event = probe_runtime(&config).build_event(context);
        let (_outcome, _bark) = record_probe_event_with_bark(&config, &db, event.clone())
            .await
            .unwrap();

        assert_eq!(event.kind, "completion");
        assert!(event.bark_body.contains("最终反馈第一行"));
        assert!(event.bark_body.contains("完整正文完整正文"));
        assert!(event.bark_body.contains("末尾唯一完整反馈"));
        assert!(!event.bark_body.contains("secret-token"));
        assert!(!event.bark_body.contains("[truncated]"));
        assert!(!event.bark_body.contains("最后反馈："));
        assert!(event.payload["bark"].get("body").is_none());
        assert_eq!(
            event.payload["body_source"],
            "task_complete.last_agent_message"
        );
        let stored = db.list_probe_events(10).unwrap();
        let stored_json = serde_json::to_string(&stored[0]).unwrap();
        assert!(!stored_json.contains("末尾唯一完整反馈"));
        assert!(!stored_json.contains("secret-token"));
        assert_eq!(
            stored[0].payload["body_summary"],
            event.payload["body_summary"]
        );
        assert_eq!(
            stored[0].payload["body_sha256"],
            event.payload["body_sha256"]
        );
        assert!(stored[0].payload["bark"].get("body").is_none());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn passive_reply_needed_body_formats_questions_and_extracts_plan_text() {
        let thread = nexushub_core::codex::ThreadSummary {
            id: "thread-question".to_string(),
            title: "问题线程".to_string(),
            status: nexushub_core::codex::ThreadStatus::ReplyNeeded,
            updated_at: None,
            archived_at: None,
            message_count: 1,
            latest_message: Some("fallback should not be used".to_string()),
            cwd: None,
            model: None,
            rollout_path: None,
            active_turn_id: Some("turn-question".to_string()),
            active_job_id: None,
            pending_elicitation: Some(nexushub_core::codex::PendingElicitation {
                turn_id: Some("turn-question".to_string()),
                item_id: Some("call-question".to_string()),
                questions: vec![nexushub_core::codex::UserInputQuestion {
                    id: "q1".to_string(),
                    header: Some("Mode".to_string()),
                    question: "怎么继续？".to_string(),
                    options: vec![
                        nexushub_core::codex::UserInputOption {
                            label: "直接执行".to_string(),
                            description: Some("按当前计划继续".to_string()),
                        },
                        nexushub_core::codex::UserInputOption {
                            label: "先调整".to_string(),
                            description: Some("补充约束后再执行".to_string()),
                        },
                    ],
                }],
            }),
            last_event_kind: None,
        };

        let (body, source) = probe_thread_notification_body(&thread, "reply-needed");

        let body = body.expect("request_user_input body");
        assert_eq!(source.as_deref(), Some("request_user_input"));
        assert!(!body.contains("等待用户选择"));
        assert!(!body.contains("Call ID："));
        assert!(!body.contains("Turn ID："));
        assert!(!body.contains("待选择内容"));
        assert!(body.contains("问题 1：怎么继续？"));
        assert!(body.contains("选项 1：直接执行"));
        assert!(body.contains("说明：按当前计划继续"));
        assert!(body.contains("选项 2：先调整"));
        assert!(!body.contains("fallback should not be used"));

        let event = probe_runtime(&Config::default()).build_event(
            ProbeEventInput::hook_stop_with_context(
                Some("thread-question"),
                Some("turn-question"),
                Some("thread-question"),
                None,
                Some(&body),
                "reply-needed",
            )
            .with_thread_title(Some("问题线程"))
            .with_body_source(source.as_deref()),
        );
        assert_eq!(event.bark_title, "等待回复：问题线程");
        assert_eq!(event.bark_body, body);
        assert!(!event.bark_body.contains("thread-question"));

        let plan_thread = nexushub_core::codex::ThreadSummary {
            pending_elicitation: None,
            latest_message: Some(
                "<proposed_plan>\n# 修复计划\n- 等待确认\n</proposed_plan>".to_string(),
            ),
            ..thread
        };

        let (body, source) = probe_thread_notification_body(&plan_thread, "reply-needed");

        let body = body.expect("plan body");
        assert_eq!(source.as_deref(), Some("proposed_plan"));
        assert_eq!(body, "# 修复计划\n- 等待确认");
        assert!(!body.contains("<proposed_plan>"));
        assert!(!body.contains("</proposed_plan>"));
    }

    #[test]
    fn passive_reply_needed_body_suppresses_plan_after_later_reply_or_completion() {
        let dir = temp_test_dir("nexushub-plan-suppressed-after-reply");
        fs::create_dir_all(&dir).unwrap();
        let rollout = dir.join("rollout-plan-replied.jsonl");
        fs::write(
            &rollout,
            [
                json!({"type":"response_item","turn_id":"turn-plan","item_id":"item-plan","payload":{"type":"message","role":"assistant","content":[{"text":"<proposed_plan>\n# 旧计划\n- 等待确认\n</proposed_plan>"}]}}).to_string(),
                json!({"type":"response_item","turn_id":"turn-plan","payload":{"type":"message","role":"user","content":[{"text":"批准，继续。"}]}}).to_string(),
                json!({"type":"response_item","turn_id":"turn-work","payload":{"type":"message","role":"assistant","content":[{"text":"正在执行计划。"}]}}).to_string(),
                json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-work","last_agent_message":"已完成。"}}).to_string(),
            ]
            .join("\n"),
        )
        .unwrap();
        let thread = nexushub_core::codex::ThreadSummary {
            id: "thread-plan-replied".to_string(),
            title: "计划已回复线程".to_string(),
            status: nexushub_core::codex::ThreadStatus::ReplyNeeded,
            updated_at: None,
            archived_at: None,
            message_count: 4,
            latest_message: Some(
                "<proposed_plan>\n# 旧计划\n- 等待确认\n</proposed_plan>".to_string(),
            ),
            cwd: None,
            model: None,
            rollout_path: Some(rollout),
            active_turn_id: Some("turn-plan".to_string()),
            active_job_id: None,
            pending_elicitation: None,
            last_event_kind: Some("task_complete".to_string()),
        };

        let (body, source) = probe_thread_notification_body(&thread, "reply-needed");

        assert_eq!(body, None);
        assert_eq!(source, None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn passive_reply_needed_scan_rejects_old_reply_needed_threads() {
        let thread = nexushub_core::codex::ThreadSummary {
            id: "thread-old-plan".to_string(),
            title: "旧计划线程".to_string(),
            status: nexushub_core::codex::ThreadStatus::ReplyNeeded,
            updated_at: Some(
                (chrono::Utc::now()
                    - chrono::Duration::seconds(PROBE_REPLY_NEEDED_FRESH_WINDOW_SECONDS + 60))
                .to_rfc3339(),
            ),
            archived_at: None,
            message_count: 1,
            latest_message: Some(
                "<proposed_plan>\n# 旧计划\n- 等待确认\n</proposed_plan>".to_string(),
            ),
            cwd: None,
            model: None,
            rollout_path: None,
            active_turn_id: Some("turn-old".to_string()),
            active_job_id: None,
            pending_elicitation: None,
            last_event_kind: Some("task_complete".to_string()),
        };

        assert!(!probe_thread_passive_bark_fresh(
            &thread,
            "reply-needed",
            Some("proposed_plan")
        ));
    }

    #[tokio::test]
    async fn passive_reply_needed_plan_dedupe_key_changes_with_plan_hash_and_no_ttl_resend() {
        let mut config = Config::default();
        config.probe.notifications.enabled = true;
        config.probe.notifications.notify_reply_needed = true;
        config.probe.notifications.server_url = "http://127.0.0.1:9".to_string();
        let dir = temp_test_dir("nexushub-plan-ttl-marker");
        fs::create_dir_all(&dir).unwrap();
        let db = PanelDb::open(dir.join("panel.sqlite")).unwrap();
        db.set_secret_setting_bytes("probe_bark_device_key", b"super-secret-device")
            .unwrap();

        let body_one = format_proposed_plan_reply_needed(
            "thread-plan",
            "turn-plan",
            "<proposed_plan>\n# 第一版\n- A\n</proposed_plan>",
        );
        let body_two = format_proposed_plan_reply_needed(
            "thread-plan",
            "turn-plan",
            "<proposed_plan>\n# 第二版\n- B\n</proposed_plan>",
        );
        let make_event = |body: &str| {
            probe_runtime(&config).build_event(
                ProbeEventInput::hook_stop_with_context(
                    Some("thread-plan"),
                    Some("turn-plan"),
                    Some("thread-plan"),
                    None,
                    Some(body),
                    "reply-needed",
                )
                .with_thread_title(Some("真实计划线程"))
                .with_body_source(Some("proposed_plan"))
                .with_passive_scan_source(),
            )
        };

        let first_event = make_event(&body_one);
        let first_key = first_event.dedupe_key.clone();
        let first = record_probe_event_with_bark(&config, &db, first_event)
            .await
            .unwrap();
        let duplicate = record_probe_event_with_bark(&config, &db, make_event(&body_one))
            .await
            .unwrap();
        let changed_event = make_event(&body_two);
        let changed_key = changed_event.dedupe_key.clone();
        let changed = record_probe_event_with_bark(&config, &db, changed_event)
            .await
            .unwrap();

        assert!(first.0.recorded);
        assert_eq!(duplicate.1.reason.as_deref(), Some("sent_marker"));
        assert!(!duplicate.0.recorded);
        assert!(changed.0.recorded);
        assert_ne!(first_key, changed_key);
        assert!(first_key.contains("thread-plan"));
        assert!(first_key.contains("turn-plan"));
        assert_ne!(
            first_key,
            "reply-needed:thread-plan:turn-plan:reply_needed:turn:turn-plan"
        );
        let events = db.list_probe_events(10).unwrap();
        assert_eq!(events.len(), 2);
    }

    #[tokio::test]
    async fn passive_reply_needed_plan_does_not_resend_after_ttl_expires() {
        let mut config = Config::default();
        config.probe.notifications.enabled = true;
        config.probe.notifications.notify_reply_needed = true;
        config.probe.notifications.server_url = "http://127.0.0.1:9".to_string();
        let dir = temp_test_dir("nexushub-plan-ttl-marker-single");
        fs::create_dir_all(&dir).unwrap();
        let db = PanelDb::open(dir.join("panel.sqlite")).unwrap();
        db.set_secret_setting_bytes("probe_bark_device_key", b"super-secret-device")
            .unwrap();
        let body = format_proposed_plan_reply_needed(
            "thread-plan-ttl",
            "turn-plan-ttl",
            "<proposed_plan>\n# 旧计划\n- 等待确认\n</proposed_plan>",
        );
        let make_event = || {
            probe_runtime(&config).build_event(
                ProbeEventInput::hook_stop_with_context(
                    Some("thread-plan-ttl"),
                    Some("turn-plan-ttl"),
                    Some("thread-plan-ttl"),
                    None,
                    Some(&body),
                    "reply-needed",
                )
                .with_thread_title(Some("旧计划 TTL 线程"))
                .with_body_source(Some("proposed_plan"))
                .with_passive_scan_source(),
            )
        };

        let first = record_probe_event_with_bark(&config, &db, make_event())
            .await
            .unwrap();
        Connection::open(db.path())
            .unwrap()
            .execute("DELETE FROM probe_dedupe", [])
            .unwrap();
        let after_ttl = record_probe_event_with_bark(&config, &db, make_event())
            .await
            .unwrap();

        assert!(first.0.recorded);
        assert_eq!(after_ttl.1.reason.as_deref(), Some("sent_marker"));
        assert!(!after_ttl.0.recorded);
        assert!(!after_ttl.1.sent);
        assert_eq!(db.list_probe_events(10).unwrap().len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn passive_request_user_input_event_does_not_resend_after_ttl_expires() {
        let mut config = Config::default();
        config.probe.notifications.enabled = true;
        config.probe.notifications.notify_reply_needed = true;
        config.probe.notifications.server_url = "http://127.0.0.1:9".to_string();
        let dir = temp_test_dir("nexushub-input-ttl-marker");
        fs::create_dir_all(&dir).unwrap();
        let db = PanelDb::open(dir.join("panel.sqlite")).unwrap();
        db.set_secret_setting_bytes("probe_bark_device_key", b"super-secret-device")
            .unwrap();
        let body = "等待用户选择：Plan Mode 已请求用户选择后继续。\n\nCall ID：call-question\nTurn ID：turn-question\n时间：2026-06-16 12:00:00 北京时间\n状态说明：这一轮正在等待用户选择，不是异常停止。\n\n待选择内容：\n问题 1：Continue?\n选项 1：继续";
        let make_event = || {
            probe_runtime(&config).build_event(
                ProbeEventInput::hook_stop_with_context(
                    Some("thread-question-ttl"),
                    Some("turn-question"),
                    Some("thread-question-ttl"),
                    None,
                    Some(body),
                    "reply-needed",
                )
                .with_thread_title(Some("问题 TTL 线程"))
                .with_body_source(Some("request_user_input"))
                .with_passive_scan_source(),
            )
        };

        let first = record_probe_event_with_bark(&config, &db, make_event())
            .await
            .unwrap();
        Connection::open(db.path())
            .unwrap()
            .execute("DELETE FROM probe_dedupe", [])
            .unwrap();
        let after_ttl = record_probe_event_with_bark(&config, &db, make_event())
            .await
            .unwrap();

        assert!(first.0.recorded);
        assert_eq!(after_ttl.1.reason.as_deref(), Some("sent_marker"));
        assert!(!after_ttl.0.recorded);
        assert!(!after_ttl.1.sent);
        assert_eq!(db.list_probe_events(10).unwrap().len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn passive_request_user_input_marker_ignores_scan_time_changes() {
        let mut config = Config::default();
        config.probe.notifications.enabled = true;
        config.probe.notifications.notify_reply_needed = true;
        config.probe.notifications.server_url = "http://127.0.0.1:9".to_string();
        let db = PanelDb::open(":memory:").unwrap();
        db.set_secret_setting_bytes("probe_bark_device_key", b"super-secret-device")
            .unwrap();
        let make_body = |time: &str| {
            format!(
                "等待用户选择：Plan Mode 已请求用户选择后继续。\n\nCall ID：call-question\nTurn ID：turn-question\n时间：{time}\n状态说明：这一轮正在等待用户选择，不是异常停止。\n\n待选择内容：\n问题 1：Continue?\n选项 1：继续"
            )
        };
        let make_event = |body: String| {
            probe_runtime(&config).build_event(
                ProbeEventInput::hook_stop_with_context(
                    Some("thread-question-time"),
                    Some("turn-question"),
                    Some("thread-question-time"),
                    None,
                    Some(&body),
                    "reply-needed",
                )
                .with_thread_title(Some("问题时间线程"))
                .with_body_source(Some("request_user_input"))
                .with_passive_scan_source(),
            )
        };

        let first = record_probe_event_with_bark(
            &config,
            &db,
            make_event(make_body("2026-06-16 12:00:00 北京时间")),
        )
        .await
        .unwrap();
        let second = record_probe_event_with_bark(
            &config,
            &db,
            make_event(make_body("2026-06-16 12:05:00 北京时间")),
        )
        .await
        .unwrap();

        assert!(first.0.recorded);
        assert_eq!(second.1.reason.as_deref(), Some("sent_marker"));
        assert!(!second.0.recorded);
        assert_eq!(db.list_probe_events(10).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn send_bark_posts_lite_payload_and_reports_non_success_response_code() {
        let server = TestHttpServer::start_n(
            1,
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 39\r\n\r\n{\"code\":400,\"message\":\"bad bark token\"}",
        );
        let mut config = Config::default();
        config.probe.notifications.server_url = server.url();
        config.probe.notifications.group = "Probe Group".to_string();
        config.probe.notifications.sound = Some("bell".to_string());
        config.probe.notifications.url = Some("https://661313.xyz/nexushub/".to_string());

        let request = ProbeBarkRequest {
            title: "Codex Sentinel Lite".to_string(),
            body: "Bark 推送通道正常。".to_string(),
            dedupe_key: "hook-stop:thread-a:turn-1".to_string(),
        };
        let result = send_bark_notification(
            &config,
            b"device key/with spaces",
            &request,
            std::time::Duration::from_secs(2),
        )
        .await
        .unwrap();

        assert!(!result.sent);
        assert!(!result.skipped);
        assert_eq!(result.reason.as_deref(), Some("bark_response_code"));
        assert_eq!(result.http_status, Some(200));
        assert!(result.device_key_configured);
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("device key"));
        let raw = server.request();
        assert!(raw.starts_with("POST /push "));
        let body = raw.split("\r\n\r\n").nth(1).unwrap();
        let payload: Value = serde_json::from_str(body).unwrap();
        assert_eq!(payload["device_key"], "device key/with spaces");
        assert_eq!(payload["title"], "Codex Sentinel Lite");
        assert_eq!(payload["body"], "Bark 推送通道正常。");
        assert_eq!(payload.as_object().unwrap().len(), 3);
        assert!(payload.get("group").is_none());
        assert!(payload.get("sound").is_none());
        assert!(payload.get("url").is_none());
    }

    #[tokio::test]
    async fn send_bark_splits_long_body_on_utf8_boundaries_with_segment_prefix() {
        let server = TestHttpServer::start_n(
            20,
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 12\r\n\r\n{\"code\":200}",
        );
        let mut config = Config::default();
        config.probe.notifications.server_url = server.url();
        config.probe.notifications.group = "Probe Group".to_string();

        let request = ProbeBarkRequest {
            title: "NexusHub Probe long body".to_string(),
            body: "完成".repeat(4_000),
            dedupe_key: "hook-stop:thread-a:turn-long".to_string(),
        };
        let result = send_bark_notification(
            &config,
            b"device-key",
            &request,
            std::time::Duration::from_secs(2),
        )
        .await
        .unwrap();

        assert!(result.sent);
        assert_eq!(result.http_status, Some(200));
        let requests = server.requests(result.chunk_count);
        assert_eq!(requests.len(), result.chunk_count);
        for (index, raw) in requests.iter().enumerate() {
            assert!(raw.starts_with("POST /push "));
            let body = raw.split("\r\n\r\n").nth(1).unwrap();
            let payload: Value = serde_json::from_str(body).unwrap();
            assert_eq!(payload["device_key"], "device-key");
            assert_eq!(
                payload["title"],
                format!(
                    "NexusHub Probe long body ({}/{})",
                    index + 1,
                    result.chunk_count
                )
            );
            let chunk = payload["body"].as_str().unwrap();
            let prefix = format!("第 {}/{} 段\n\n", index + 1, result.chunk_count);
            assert!(chunk.starts_with(&prefix));
            let body_part = chunk.strip_prefix(&prefix).unwrap();
            assert!(body_part.len() <= PROBE_BARK_BODY_CHUNK_BYTES);
            assert!(chunk.is_char_boundary(chunk.len()));
        }
        let combined = requests
            .iter()
            .enumerate()
            .map(|(index, raw)| {
                let body = raw.split("\r\n\r\n").nth(1).unwrap();
                let chunk = serde_json::from_str::<Value>(body).unwrap()["body"]
                    .as_str()
                    .unwrap()
                    .to_string();
                let prefix = format!("第 {}/{} 段\n\n", index + 1, result.chunk_count);
                chunk.strip_prefix(&prefix).unwrap().to_string()
            })
            .collect::<String>();
        assert_eq!(combined, request.body);
    }

    #[tokio::test]
    async fn bark_delivery_uses_full_event_body_not_truncated_summary() {
        let server = TestHttpServer::start_n(
            20,
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 12\r\n\r\n{\"code\":200}",
        );
        let mut config = Config::default();
        config.probe.notifications.enabled = true;
        config.probe.notifications.notify_completion = true;
        config.probe.notifications.server_url = server.url();
        let db = PanelDb::open(":memory:").unwrap();
        db.set_secret_setting_bytes("probe_bark_device_key", b"device-key")
            .unwrap();
        let full_body = format!("开头\n{}\n末尾唯一完整反馈", "完整正文".repeat(900));
        let event = probe_runtime(&config).build_event(
            ProbeEventInput::notify_completion_with_context(
                Some("thread-long"),
                Some("turn-long"),
                Some("thread-long"),
                None,
                Some(&full_body),
                Some("task_complete.last_agent_message"),
            )
            .with_thread_title(Some("真实长正文线程")),
        );
        assert_eq!(event.bark_title, "线程正常完成：真实长正文线程");
        assert!(event.payload["body_truncated"].as_bool().unwrap());
        assert!(!event.payload["body_summary"]
            .as_str()
            .unwrap()
            .contains("末尾唯一完整反馈"));

        let (_outcome, bark) = record_probe_event_with_bark(&config, &db, event)
            .await
            .unwrap();

        assert!(bark.sent);
        let requests = server.requests(bark.request_count);
        assert_eq!(requests.len(), bark.request_count);
        let combined = requests
            .iter()
            .enumerate()
            .map(|(index, raw)| {
                let body = raw.split("\r\n\r\n").nth(1).unwrap();
                let payload: Value = serde_json::from_str(body).unwrap();
                assert_eq!(
                    payload["title"],
                    format!(
                        "线程正常完成：真实长正文线程 ({}/{})",
                        index + 1,
                        bark.request_count
                    )
                );
                let chunk = payload["body"].as_str().unwrap();
                let prefix = format!("第 {}/{} 段\n\n", index + 1, bark.request_count);
                let body_part = chunk.strip_prefix(&prefix).unwrap();
                assert!(body_part.len() <= PROBE_BARK_BODY_CHUNK_BYTES);
                body_part.to_string()
            })
            .collect::<String>();
        assert!(combined.contains("开头"));
        assert!(combined.contains("末尾唯一完整反馈"));
        assert!(combined.contains(&full_body));
        let stored = db.list_probe_events(10).unwrap();
        let stored_json = serde_json::to_string(&stored[0]).unwrap();
        assert!(!stored_json.contains("末尾唯一完整反馈"));
        assert!(stored[0].payload["bark"].get("body").is_none());
    }

    #[test]
    fn bark_body_chunks_match_lite_split_and_trim_body() {
        let body = format!("  {}  \n", "完成".repeat(4_000));
        let chunks = bark_body_chunks(&body, PROBE_BARK_BODY_CHUNK_BYTES);

        assert!(!chunks[0].contains("  完成"));
        for (index, chunk) in chunks.iter().enumerate() {
            let prefix = format!("第 {}/{} 段\n\n", index + 1, chunks.len());
            assert!(chunk.starts_with(&prefix));
            let body_part = chunk.strip_prefix(&prefix).unwrap();
            assert!(body_part.len() <= PROBE_BARK_BODY_CHUNK_BYTES);
            assert!(chunk.is_char_boundary(chunk.len()));
        }
        let joined = chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| {
                let prefix = format!("第 {}/{} 段\n\n", index + 1, chunks.len());
                chunk.strip_prefix(&prefix).unwrap()
            })
            .collect::<String>();
        assert_eq!(joined, body.trim());
    }

    #[test]
    fn bark_body_chunks_keeps_body_budget_when_prefix_digit_count_grows() {
        let body = "a".repeat((PROBE_BARK_BODY_CHUNK_BYTES * 9) - 100);
        let chunks = bark_body_chunks(&body, PROBE_BARK_BODY_CHUNK_BYTES);

        assert_eq!(chunks.len(), 9);
        let joined = chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| {
                let prefix = format!("第 {}/{} 段\n\n", index + 1, chunks.len());
                let body_part = chunk.strip_prefix(&prefix).unwrap();
                assert!(body_part.len() <= PROBE_BARK_BODY_CHUNK_BYTES);
                body_part
            })
            .collect::<String>();
        assert_eq!(joined, body);
    }

    #[test]
    fn bark_body_chunks_match_lite_exact_2400_body_bytes_before_prefix() {
        let body = "a".repeat(PROBE_BARK_BODY_CHUNK_BYTES * 2);
        let chunks = bark_body_chunks(&body, PROBE_BARK_BODY_CHUNK_BYTES);

        assert_eq!(chunks.len(), 2);
        for (index, chunk) in chunks.iter().enumerate() {
            let prefix = format!("第 {}/{} 段\n\n", index + 1, chunks.len());
            let body_part = chunk.strip_prefix(&prefix).unwrap();
            assert_eq!(body_part.len(), PROBE_BARK_BODY_CHUNK_BYTES);
            assert!(body_part.is_char_boundary(body_part.len()));
        }
        let joined = chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| {
                let prefix = format!("第 {}/{} 段\n\n", index + 1, chunks.len());
                chunk.strip_prefix(&prefix).unwrap()
            })
            .collect::<String>();
        assert_eq!(joined, body);
    }

    #[tokio::test]
    async fn record_probe_event_stores_body_metadata_but_not_full_bark_body_or_tokens() {
        let server = TestHttpServer::start_n(
            20,
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 12\r\n\r\n{\"code\":200}",
        );
        let mut config = Config::default();
        config.probe.notifications.enabled = true;
        config.probe.notifications.notify_completion = true;
        config.probe.notifications.server_url = server.url();
        let db = PanelDb::open(":memory:").unwrap();
        db.set_secret_setting_bytes("probe_bark_device_key", b"secret-device-token")
            .unwrap();
        let full_body = format!(
            "开头\nAuthorization: Bearer secret-token\n{}\n<proposed_plan>不要存这个标签</proposed_plan>\n末尾唯一完整反馈",
            "完整正文".repeat(900)
        );
        let event = probe_runtime(&config).build_event(
            ProbeEventInput::notify_completion_with_context(
                Some("thread-safe-store"),
                Some("turn-safe-store"),
                Some("thread-safe-store"),
                None,
                Some(&full_body),
                Some("task_complete.last_agent_message"),
            )
            .with_thread_title(Some("安全存储线程")),
        );

        let (_outcome, bark) = record_probe_event_with_bark(&config, &db, event)
            .await
            .unwrap();
        assert!(bark.sent);
        let _requests = server.requests(bark.request_count);

        let stored = db.list_probe_events(10).unwrap();
        assert_eq!(stored.len(), 1);
        let payload = &stored[0].payload;
        assert!(payload["body_summary"].as_str().unwrap().contains("开头"));
        assert!(payload["body_sha256"]
            .as_str()
            .is_some_and(|value| value.len() == 64));
        assert_eq!(payload["body_length"], full_body.len() as u64);
        assert_eq!(payload["bark"]["body_length"], full_body.len() as u64);
        assert_eq!(payload["bark"]["body_sha256"], payload["body_sha256"]);
        assert!(payload["bark"]["chunk_count"].as_u64().is_some());
        assert!(payload["bark"]["request_count"].as_u64().is_some());
        assert!(payload["bark"].get("body").is_none());
        assert!(payload.get("bark_body").is_none() || payload["bark_body"].is_null());
        let stored_json = serde_json::to_string(&stored[0]).unwrap();
        assert!(!stored_json.contains("末尾唯一完整反馈"));
        assert!(!stored_json.contains("secret-token"));
        assert!(!stored_json.contains("secret-device-token"));
        assert!(!stored_json.contains("<proposed_plan>"));
        assert!(!stored_json.contains("</proposed_plan>"));
        assert!(!stored_json.contains("/push"));
    }

    #[tokio::test]
    async fn passive_reply_needed_plan_dedupe_key_includes_thread_turn_item_and_plan_hash() {
        let server = TestHttpServer::start_n(
            1,
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 12\r\n\r\n{\"code\":200}",
        );
        let mut config = Config::default();
        config.probe.notifications.enabled = true;
        config.probe.notifications.notify_reply_needed = true;
        config.probe.notifications.server_url = server.url();
        let db = PanelDb::open(":memory:").unwrap();
        db.set_secret_setting_bytes("probe_bark_device_key", b"super-secret-device")
            .unwrap();
        let raw_plan = "<proposed_plan>\n# 稳定计划\n- A\n</proposed_plan>";
        let body =
            format_proposed_plan_reply_needed("thread-plan-stable", "turn-plan-stable", raw_plan);
        let event = probe_runtime(&config).build_event(
            ProbeEventInput::hook_stop_with_context(
                Some("thread-plan-stable"),
                Some("turn-plan-stable"),
                Some("thread-plan-stable"),
                None,
                Some(&body),
                "reply-needed",
            )
            .with_thread_title(Some("稳定计划线程"))
            .with_body_source(Some("proposed_plan"))
            .with_passive_scan_source(),
        );
        let mut event = event;
        event.payload["item_id"] = json!("item-plan-stable");

        let (outcome, bark) = record_probe_event_with_bark(&config, &db, event)
            .await
            .unwrap();
        assert!(bark.sent);
        let _raw = server.request();
        let dedupe_key = outcome.dedupe_key;

        assert!(dedupe_key.contains("thread-plan-stable"));
        assert!(dedupe_key.contains("turn-plan-stable"));
        assert!(dedupe_key.contains("item-plan-stable"));
        assert!(dedupe_key.contains("plan_hash"));
        assert_ne!(
            dedupe_key,
            "reply-needed:thread-plan-stable:turn-plan-stable:reply_needed:turn:turn-plan-stable"
        );
        let events = db.list_probe_events(10).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn passive_reply_needed_body_suppresses_plan_after_tool_output_assistant_or_turn_completed() {
        for (name, tail) in [
            (
                "tool-output",
                vec![
                    json!({"type":"response_item","turn_id":"turn-plan","payload":{"type":"function_call_output","call_id":"call-plan","output":"完成选择"}}),
                ],
            ),
            (
                "assistant-progress",
                vec![
                    json!({"type":"response_item","turn_id":"turn-work","payload":{"type":"message","role":"assistant","content":[{"text":"继续执行计划。"}]}}),
                ],
            ),
            (
                "turn-completed",
                vec![json!({"type":"turn_completed","turn_id":"turn-plan"})],
            ),
        ] {
            let dir = temp_test_dir(&format!("nexushub-plan-suppressed-{name}"));
            fs::create_dir_all(&dir).unwrap();
            let rollout = dir.join("rollout.jsonl");
            let mut lines = vec![
                json!({"type":"response_item","turn_id":"turn-plan","item_id":"item-plan","payload":{"type":"message","role":"assistant","content":[{"text":"<proposed_plan>\n# 旧计划\n- 等待确认\n</proposed_plan>"}]}}),
            ];
            lines.extend(tail);
            fs::write(
                &rollout,
                lines
                    .into_iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
            .unwrap();
            let thread = nexushub_core::codex::ThreadSummary {
                id: format!("thread-{name}"),
                title: "计划应被抑制线程".to_string(),
                status: nexushub_core::codex::ThreadStatus::ReplyNeeded,
                updated_at: None,
                archived_at: None,
                message_count: 2,
                latest_message: Some(
                    "<proposed_plan>\n# 旧计划\n- 等待确认\n</proposed_plan>".to_string(),
                ),
                cwd: None,
                model: None,
                rollout_path: Some(rollout),
                active_turn_id: Some("turn-plan".to_string()),
                active_job_id: None,
                pending_elicitation: None,
                last_event_kind: None,
            };

            let (body, source) = probe_thread_notification_body(&thread, "reply-needed");

            assert_eq!(body, None, "{name}");
            assert_eq!(source, None, "{name}");
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn passive_reply_needed_fallback_suppresses_old_plan_after_later_completion() {
        let dir = temp_test_dir("nexushub-fallback-old-plan-completed");
        fs::create_dir_all(&dir).unwrap();
        let rollout = dir.join("rollout.jsonl");
        fs::write(
            &rollout,
            [
                json!({"type":"response_item","turn_id":"turn-plan","item_id":"item-plan","payload":{"type":"message","role":"assistant","content":[{"text":"<proposed_plan>\n# 旧 fallback 计划\n- 等待确认\n</proposed_plan>"}]}}).to_string(),
                json!({"type":"response_item","turn_id":"turn-plan","payload":{"type":"message","role":"user","content":[{"text":"继续"}]}}).to_string(),
                json!({"type":"response_item","turn_id":"turn-work","payload":{"type":"message","role":"assistant","content":[{"text":"执行完成。"}]}}).to_string(),
                json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-work","last_agent_message":"执行完成。"}}).to_string(),
            ]
            .join("\n"),
        )
        .unwrap();
        let thread = nexushub_core::codex::ThreadSummary {
            id: "thread-fallback-old-plan".to_string(),
            title: "fallback 旧计划线程".to_string(),
            status: nexushub_core::codex::ThreadStatus::ReplyNeeded,
            updated_at: None,
            archived_at: None,
            message_count: 4,
            latest_message: None,
            cwd: None,
            model: None,
            rollout_path: Some(rollout),
            active_turn_id: None,
            active_job_id: None,
            pending_elicitation: None,
            last_event_kind: Some("task_complete".to_string()),
        };

        let (body, source) = probe_thread_notification_body(&thread, "reply-needed");

        assert_eq!(body, None);
        assert_eq!(source, None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn passive_reply_needed_fallback_suppresses_old_plan_when_completion_has_no_body() {
        let dir = temp_test_dir("nexushub-fallback-old-plan-empty-complete");
        fs::create_dir_all(&dir).unwrap();
        let rollout = dir.join("rollout.jsonl");
        fs::write(
            &rollout,
            [
                json!({"type":"response_item","turn_id":"turn-plan","item_id":"item-plan","payload":{"type":"message","role":"assistant","content":[{"text":"<proposed_plan>\n# 旧 fallback 计划\n- 等待确认\n</proposed_plan>"}]}}).to_string(),
                json!({"type":"response_item","turn_id":"turn-plan","payload":{"type":"message","role":"user","content":[{"text":"继续"}]}}).to_string(),
                json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-plan","last_agent_message":null}}).to_string(),
            ]
            .join("\n"),
        )
        .unwrap();
        let fallback = rollout_completion_last_agent_message(&rollout, Some("turn-plan"))
            .unwrap()
            .expect("fallback plan");
        assert!(fallback.contains("<proposed_plan>"));
        let thread = nexushub_core::codex::ThreadSummary {
            id: "thread-fallback-empty-complete".to_string(),
            title: "fallback 空完成线程".to_string(),
            status: nexushub_core::codex::ThreadStatus::ReplyNeeded,
            updated_at: None,
            archived_at: None,
            message_count: 3,
            latest_message: None,
            cwd: None,
            model: None,
            rollout_path: Some(rollout),
            active_turn_id: Some("turn-plan".to_string()),
            active_job_id: None,
            pending_elicitation: None,
            last_event_kind: Some("task_complete".to_string()),
        };

        let (body, source) = probe_thread_notification_body(&thread, "reply-needed");

        assert_eq!(body, None);
        assert_eq!(source, None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn probe_thread_scan_does_not_resend_completed_old_plan_when_app_server_is_offline() {
        let dir = temp_test_dir("nexushub-thread-scan-old-plan-offline");
        let codex_home = dir.join(".codex");
        fs::create_dir_all(codex_home.join("sessions")).unwrap();
        let rollout = codex_home.join("sessions").join("rollout-old-plan.jsonl");
        fs::write(
            &rollout,
            [
                json!({"type":"response_item","turn_id":"turn-plan","item_id":"item-plan","payload":{"type":"message","role":"assistant","content":[{"text":"<proposed_plan>\n# 旧计划\n- 等待确认\n</proposed_plan>"}]}}).to_string(),
                json!({"type":"response_item","turn_id":"turn-plan","payload":{"type":"message","role":"user","content":[{"text":"同意，继续。"}]}}).to_string(),
                json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-work","last_agent_message":"已完成。"}}).to_string(),
            ]
            .join("\n"),
        )
        .unwrap();
        let conn = Connection::open(codex_home.join("state_5.sqlite")).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                title TEXT,
                updated_at INTEGER,
                archived_at INTEGER,
                rollout_path TEXT
            );
            "#,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads(id, title, updated_at, rollout_path) VALUES(?1, ?2, ?3, ?4)",
            params![
                "thread-old-plan",
                "真实旧计划线程",
                chrono::Utc::now().timestamp_millis(),
                rollout.to_string_lossy().as_ref()
            ],
        )
        .unwrap();
        fs::write(codex_home.join("session_index.jsonl"), b"").unwrap();
        let mut config = Config::default();
        config.codex.home = codex_home.clone();
        config.codex.app_server_socket = Some(dir.join("missing.sock"));
        config.codex.bridge_enabled = true;
        config.probe.notifications.enabled = true;
        config.probe.notifications.notify_reply_needed = true;
        config.probe.notifications.server_url = "http://127.0.0.1:9".to_string();
        let db = PanelDb::open(":memory:").unwrap();
        let state = AppState::new(config, db.clone());

        let count = run_probe_thread_scan_if_due(state).await.unwrap();

        assert_eq!(count, 0);
        assert!(db.list_probe_events(10).unwrap().is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn passive_request_user_input_does_not_resend_after_ttl_or_revive_after_answer() {
        let dir = temp_test_dir("nexushub-request-user-input-no-revive");
        let codex_home = dir.join(".codex");
        fs::create_dir_all(codex_home.join("sessions")).unwrap();
        let rollout = codex_home.join("sessions").join("rollout-question.jsonl");
        fs::write(
            &rollout,
            json!({
                "type": "response_item",
                "turn_id": "turn-question",
                "payload": {
                    "type": "function_call",
                    "name": "request_user_input",
                    "status": "pending",
                    "call_id": "call-question",
                    "questions": [{
                        "id": "choice",
                        "header": "Mode",
                        "question": "Continue?",
                        "options": [
                            {"label": "继续", "description": "按计划执行"},
                            {"label": "停止"}
                        ]
                    }]
                }
            })
            .to_string(),
        )
        .unwrap();
        let conn = Connection::open(codex_home.join("state_5.sqlite")).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                title TEXT,
                updated_at INTEGER,
                archived_at INTEGER,
                rollout_path TEXT
            );
            "#,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads(id, title, updated_at, rollout_path) VALUES(?1, ?2, ?3, ?4)",
            params![
                "thread-question-no-repeat",
                "待选择线程",
                chrono::Utc::now().timestamp_millis(),
                rollout.to_string_lossy().as_ref()
            ],
        )
        .unwrap();
        fs::write(codex_home.join("session_index.jsonl"), b"").unwrap();
        let mut config = Config::default();
        config.codex.home = codex_home.clone();
        config.codex.app_server_socket = Some(dir.join("missing.sock"));
        config.codex.bridge_enabled = true;
        config.probe.notifications.enabled = true;
        config.probe.notifications.notify_reply_needed = true;
        config.probe.notifications.server_url = "http://127.0.0.1:9".to_string();
        let db = PanelDb::open(":memory:").unwrap();
        db.set_secret_setting_bytes("probe_bark_device_key", b"super-secret-device")
            .unwrap();

        let first_count = run_probe_thread_scan_if_due(AppState::new(config.clone(), db.clone()))
            .await
            .unwrap();
        db.maintain_probe_events(1, 100, false).unwrap();
        let second_count = run_probe_thread_scan_if_due(AppState::new(config.clone(), db.clone()))
            .await
            .unwrap();
        fs::write(
            &rollout,
            [
                json!({"type":"response_item","turn_id":"turn-question","payload":{"type":"function_call","name":"request_user_input","status":"pending","call_id":"call-question","questions":[{"id":"choice","header":"Mode","question":"Continue?","options":[{"label":"继续","description":"按计划执行"},{"label":"停止"}]}]}}).to_string(),
                json!({"type":"response_item","turn_id":"turn-question","payload":{"type":"UserInputAnswer","call_id":"call-question","answers":{"choice":["继续"]}}}).to_string(),
                json!({"type":"event_msg","payload":{"type":"task_complete","turn_id":"turn-question","last_agent_message":"已继续。"}}).to_string(),
            ]
            .join("\n"),
        )
        .unwrap();
        let answered_count = run_probe_thread_scan_if_due(AppState::new(config, db.clone()))
            .await
            .unwrap();

        assert_eq!(first_count, 1);
        assert_eq!(second_count, 0);
        assert_eq!(answered_count, 0);
        assert_eq!(db.list_probe_events(10).unwrap().len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn send_bark_success_reports_redacted_request_metadata() {
        let server = TestHttpServer::start_n(
            1,
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 12\r\n\r\n{\"code\":200}",
        );
        let mut config = Config::default();
        config.probe.notifications.enabled = true;
        config.probe.notifications.server_url = server.url();
        config.probe.notifications.group = "Probe Group".to_string();

        let result = send_bark_notification(
            &config,
            b"device-key-secret",
            &ProbeBarkRequest {
                title: "Codex Sentinel Lite".to_string(),
                body: "ok".to_string(),
                dedupe_key: "hook-stop:thread-a:turn-1".to_string(),
            },
            std::time::Duration::from_secs(2),
        )
        .await
        .unwrap();

        assert!(result.sent);
        assert_eq!(result.reason, None);
        assert_eq!(result.http_status, Some(200));
        assert_eq!(result.request_count, 1);
        assert_eq!(result.chunk_count, 1);
        assert_eq!(result.server_url.as_deref(), Some(server.url().as_str()));
        assert!(result.request_url.as_deref().is_some_and(|value| {
            value == "[redacted]" || !value.contains("device-key-secret")
        }));
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("device-key-secret"));
        let _ = server.request();
    }

    #[tokio::test]
    async fn bark_test_uses_codex_sentinel_lite_title_and_body() {
        let server = TestHttpServer::start_n(
            1,
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 12\r\n\r\n{\"code\":200}",
        );
        let mut config = Config::default();
        config.probe.notifications.enabled = true;
        config.probe.notifications.server_url = server.url();
        let db = PanelDb::open(":memory:").unwrap();
        db.set_secret_setting_bytes("probe_bark_device_key", b"device-key")
            .unwrap();

        run_probe_command(ProbeCommand::BarkTest, &config, db)
            .await
            .unwrap();

        let raw = server.request();
        assert!(raw.starts_with("POST /push "));
        let body = raw.split("\r\n\r\n").nth(1).unwrap();
        let payload: Value = serde_json::from_str(body).unwrap();
        assert_eq!(payload["device_key"], "device-key");
        assert_eq!(payload["title"], "Codex Sentinel Lite");
        assert_eq!(payload["body"], "Bark 推送通道正常。");
    }

    #[tokio::test]
    async fn hook_stop_cli_payload_keeps_stdout_codex_only_and_stderr_diagnostics() {
        let mut config = Config::default();
        config.probe.notifications.enabled = false;
        let db = PanelDb::open(":memory:").unwrap();
        let result = handle_built_probe_event(
            &config,
            &db,
            probe_runtime(&config).build_event(ProbeEventInput::hook_stop(
                Some("thread-a"),
                Some("turn-a"),
                "hook-stop",
            )),
        )
        .await
        .unwrap();

        let (stdout, stderr) = hook_stop_cli_output(&result).unwrap();
        assert_eq!(stdout.trim(), r#"{"continue":true,"suppressOutput":false}"#);
        let diagnostics: Value = serde_json::from_str(stderr.trim()).unwrap();
        assert_eq!(diagnostics["probe_event"]["recorded"], true);
        assert_eq!(diagnostics["bark"]["skipped"], true);
        assert_eq!(diagnostics["bark"]["reason"], "notifications_disabled");
        assert!(!stdout.contains("bark"));
        assert!(!stdout.contains("probe_event"));
    }

    #[test]
    fn utf8_chunks_preserve_multibyte_boundaries_and_content() {
        let value = "完成a".repeat(10);
        let chunks = utf8_chunks(&value, 7);

        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|chunk| chunk.len() <= 7));
        assert!(chunks
            .iter()
            .all(|chunk| chunk.is_char_boundary(chunk.len())));
        assert_eq!(chunks.concat(), value);
    }

    struct TestHttpServer {
        address: std::net::SocketAddr,
        request: std::sync::mpsc::Receiver<String>,
    }

    impl TestHttpServer {
        fn start_n(expected_requests: usize, response: &'static str) -> Self {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                for _ in 0..expected_requests {
                    let (mut stream, _) = listener.accept().unwrap();
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut request = String::new();
                    loop {
                        let mut line = String::new();
                        reader.read_line(&mut line).unwrap();
                        request.push_str(&line);
                        if line == "\r\n" || line.is_empty() {
                            break;
                        }
                    }
                    let content_length = request
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    let mut body = vec![0_u8; content_length];
                    reader.read_exact(&mut body).unwrap();
                    request.push_str(&String::from_utf8_lossy(&body));
                    tx.send(request).unwrap();
                    stream.write_all(response.as_bytes()).unwrap();
                }
            });
            Self {
                address,
                request: rx,
            }
        }

        fn url(&self) -> String {
            format!("http://{}", self.address)
        }

        fn request(self) -> String {
            self.request
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
        }

        fn requests(self, count: usize) -> Vec<String> {
            (0..count)
                .map(|_| {
                    self.request
                        .recv_timeout(std::time::Duration::from_secs(2))
                        .unwrap()
                })
                .collect()
        }
    }

    #[tokio::test]
    async fn probe_logs_db_scheduler_runs_when_due_and_persists_result() {
        let dir = temp_test_dir("nexushub-scheduler-due");
        let codex_home = dir.join(".codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::create_dir_all(codex_home.join("app-server-control")).unwrap();
        let logs_path = codex_home.join("logs_2.sqlite");
        let now = chrono::Utc::now().timestamp();
        seed_codex_logs_db(&logs_path, &[now - 300_000, now - 100]);

        let mut config = Config::default();
        config.codex.home = codex_home.clone();
        config.probe.logs_db.retention_days = 2;
        config.probe.logs_db.delete_chunk_rows = 10;
        config.probe.logs_db.max_delete_rows_per_run = 10;
        let db = PanelDb::open(":memory:").unwrap();
        let state = AppState::new(config, db.clone());

        let outcome = run_probe_logs_db_maintenance_if_due(state).await.unwrap();

        assert!(outcome.ran);
        assert_eq!(outcome.result.as_ref().unwrap().target, "codex_logs_2");
        assert_eq!(outcome.result.as_ref().unwrap().deleted_rows, 1);
        let stored = db
            .get_setting(PROBE_LOGS_DB_LAST_MAINTAIN_SETTING)
            .unwrap()
            .unwrap();
        let stored: Value = serde_json::from_str(&stored).unwrap();
        assert_eq!(stored["target"], "codex_logs_2");
        assert_eq!(stored["deleted_rows"], 1);
        assert_eq!(stored["path"], logs_path.to_string_lossy().as_ref());
        assert_eq!(
            stored["configured_codex_home"],
            codex_home.to_string_lossy().as_ref()
        );
        assert_eq!(
            stored["resolved_codex_home"],
            codex_home.to_string_lossy().as_ref()
        );
        assert_eq!(stored["codex_home_source"], "configured");
        assert_eq!(stored["logs_db_source"], "configured");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn probe_logs_db_scheduler_also_prunes_panel_probe_events_separately() {
        let dir = temp_test_dir("nexushub-scheduler-panel-probe-events");
        let codex_home = dir.join(".codex");
        fs::create_dir_all(&codex_home).unwrap();
        let logs_path = codex_home.join("logs_2.sqlite");
        let now = chrono::Utc::now().timestamp();
        seed_codex_logs_db(&logs_path, &[now - 100]);

        let mut config = Config::default();
        config.codex.home = codex_home;
        config.probe.logs_db.retention_days = 2;
        config.probe.logs_db.delete_chunk_rows = 10;
        config.probe.logs_db.max_delete_rows_per_run = 10;
        let db = PanelDb::open(dir.join("panel.sqlite")).unwrap();
        {
            let conn = Connection::open(db.path()).unwrap();
            conn.execute(
                r#"
                INSERT INTO probe_events(
                  id, kind, thread_id, title, message, dedupe_key, source, payload_json, created_at
                )
                VALUES('old-event', 'hook-stop', 'thread-a', 'old', 'old', 'old-event', 'test', '{}', ?1)
                "#,
                params![now - 300_000],
            )
            .unwrap();
            conn.execute(
                r#"
                INSERT INTO probe_dedupe(namespace, dedupe_key, expires_at, created_at)
                VALUES('probe_event', 'expired', ?1, ?2)
                "#,
                params![now - 1, now - 300_000],
            )
            .unwrap();
        }
        let state = AppState::new(config, db.clone());

        let outcome = run_probe_logs_db_maintenance_if_due(state).await.unwrap();

        assert!(outcome.ran);
        assert_eq!(outcome.result.as_ref().unwrap().target, "codex_logs_2");
        assert_eq!(outcome.result.as_ref().unwrap().deleted_rows, 0);
        let counts = db.probe_logs_db_counts(2).unwrap();
        assert_eq!(counts.event_count, 0);
        assert_eq!(counts.dedupe_count, 0);
        let stored = db
            .get_setting(PROBE_LOGS_DB_LAST_MAINTAIN_SETTING)
            .unwrap()
            .unwrap();
        let stored: Value = serde_json::from_str(&stored).unwrap();
        assert_eq!(stored["target"], "codex_logs_2");
        assert_eq!(stored["probe_events_target"], "panel_probe_events");
        assert_eq!(stored["probe_events_deleted"], 1);
        assert_eq!(stored["probe_dedupe_deleted"], 1);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn probe_logs_db_scheduler_skips_when_last_run_is_recent() {
        let dir = temp_test_dir("nexushub-scheduler-recent");
        let codex_home = dir.join(".codex");
        fs::create_dir_all(&codex_home).unwrap();
        let logs_path = codex_home.join("logs_2.sqlite");
        let now = chrono::Utc::now().timestamp();
        seed_codex_logs_db(&logs_path, &[now - 300_000, now - 100]);

        let mut config = Config::default();
        config.codex.home = codex_home;
        config.probe.logs_db.retention_days = 2;
        config.probe.logs_db.maintenance_interval_hours = 6;
        let db = PanelDb::open(":memory:").unwrap();
        db.set_setting(
            PROBE_LOGS_DB_LAST_MAINTAIN_SETTING,
            &json!({"target": "codex_logs_2", "deleted_rows": 0}).to_string(),
        )
        .unwrap();
        let state = AppState::new(config, db);

        let outcome = run_probe_logs_db_maintenance_if_due(state).await.unwrap();

        assert!(!outcome.ran);
        assert_eq!(outcome.skip_reason.as_deref(), Some("not_due"));
        let conn = Connection::open(&logs_path).unwrap();
        let old_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM logs WHERE ts < ?1",
                params![now - 172_800],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(old_rows, 1);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn probe_logs_db_scheduler_compacts_when_codex_service_is_not_active() {
        let dir = temp_test_dir("nexushub-scheduler-compact");
        let codex_home = dir.join(".codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::create_dir_all(codex_home.join("app-server-control")).unwrap();
        let logs_path = codex_home.join("logs_2.sqlite");
        let now = chrono::Utc::now().timestamp();
        seed_codex_logs_db(&logs_path, &[now - 300_000, now - 100]);
        {
            let conn = Connection::open(&logs_path).unwrap();
            conn.execute_batch(
                r#"
                CREATE TABLE bulky_payloads(body BLOB NOT NULL);
                INSERT INTO bulky_payloads(body) VALUES(zeroblob(1048576));
                DROP TABLE bulky_payloads;
                "#,
            )
            .unwrap();
        }

        let mut config = Config::default();
        config.codex.home = codex_home.clone();
        config.codex.app_server_service.clear();
        config.probe.logs_db.retention_days = 2;
        config.probe.logs_db.delete_chunk_rows = 10;
        config.probe.logs_db.max_delete_rows_per_run = 10;
        config.probe.logs_db.compact_min_freelist_mb = 0;
        config.probe.logs_db.compact_min_freelist_ratio_percent = 0;
        config.probe.logs_db.minimum_free_space_mb = 0;
        let db = PanelDb::open(":memory:").unwrap();
        let state = AppState::new(config, db);

        let outcome = run_probe_logs_db_maintenance_if_due(state).await.unwrap();

        assert!(outcome.ran);
        let result = outcome.result.as_ref().unwrap();
        assert_eq!(result.target, "codex_logs_2");
        assert!(result.vacuumed);
        assert_eq!(result.quick_check_before_vacuum.as_deref(), Some("ok"));
        assert_eq!(result.path, logs_path);
        assert_eq!(
            result.configured_codex_home.as_deref(),
            Some(codex_home.to_string_lossy().as_ref())
        );
        assert_eq!(result.resolved_codex_home, codex_home);
        assert_eq!(result.codex_home_source, "configured");
        assert_eq!(result.logs_db_source, "configured");
        assert!(result
            .checkpoint_result
            .as_deref()
            .is_some_and(|value| value.starts_with("mode=TRUNCATE")));
        assert!(result.database_size_before >= result.database_size_after);
        assert!(result.page_count_before >= result.page_count_after);
        assert!(result.freelist_count_before >= result.freelist_count_after);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn probe_thread_scan_records_request_user_input_body_for_reply_needed() {
        let dir = temp_test_dir("nexushub-thread-scan-request-user-input");
        let codex_home = dir.join(".codex");
        fs::create_dir_all(&codex_home).unwrap();
        fs::create_dir_all(codex_home.join("sessions")).unwrap();
        fs::write(
            codex_home
                .join("sessions")
                .join("rollout-stale-reply.jsonl"),
            json!({
                "type": "response_item",
                "turn_id": "turn-stale",
                "payload": {
                    "type": "function_call",
                    "name": "request_user_input",
                    "status": "pending",
                    "call_id": "call-choice",
                    "questions": [{
                        "id": "q1",
                        "header": "确认",
                        "question": "Continue?",
                        "options": [
                            {"label": "继续", "description": "按计划执行"},
                            {"label": "停止"}
                        ]
                    }]
                }
            })
            .to_string(),
        )
        .unwrap();
        let conn = Connection::open(codex_home.join("state_5.sqlite")).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                title TEXT,
                updated_at INTEGER,
                archived_at INTEGER,
                rollout_path TEXT
            );
            "#,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads(id, title, updated_at, rollout_path) VALUES(?1, ?2, ?3, ?4)",
            params![
                "stale-reply",
                "stale reply",
                chrono::Utc::now().timestamp_millis(),
                codex_home
                    .join("sessions")
                    .join("rollout-stale-reply.jsonl")
                    .to_string_lossy()
                    .as_ref()
            ],
        )
        .unwrap();
        fs::write(codex_home.join("session_index.jsonl"), b"").unwrap();
        let mut config = Config::default();
        config.codex.home = codex_home.clone();
        config.probe.notifications.enabled = true;
        config.probe.notifications.server_url = "http://127.0.0.1:9".to_string();
        config.probe.notifications.notify_reply_needed = true;
        config.probe.notifications.notify_recoverable = true;
        let db = PanelDb::open(":memory:").unwrap();
        let state = AppState::new(config, db.clone());

        let count = run_probe_thread_scan_if_due(state).await.unwrap();

        assert_eq!(count, 1);
        let events = db.list_probe_events(10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "reply-needed");
        assert_eq!(events[0].payload["thread_title"], "stale reply");
        assert_eq!(events[0].payload["body_source"], "request_user_input");
        let body_summary = events[0].payload["body_summary"].as_str().unwrap();
        assert!(!body_summary.contains("等待用户选择"));
        assert!(!body_summary.contains("Turn ID："));
        assert!(!body_summary.contains("Call ID："));
        assert!(!body_summary.contains("状态说明："));
        assert!(body_summary.contains("问题 1：Continue?"));
        assert!(body_summary.contains("选项 1：继续"));
        assert!(body_summary.contains("说明：按计划执行"));
        assert!(body_summary.contains("选项 2：停止"));
        assert!(events[0].payload["bark"].get("body").is_none());
        assert!(events[0].payload["bark"]["title"]
            .as_str()
            .unwrap()
            .contains("等待回复"));
        fs::remove_dir_all(&dir).unwrap();
    }

    fn temp_test_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{unique}"))
    }

    fn seed_codex_logs_db(path: &Path, timestamps: &[i64]) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts INTEGER NOT NULL,
                ts_nanos INTEGER NOT NULL,
                level TEXT NOT NULL,
                target TEXT NOT NULL,
                estimated_bytes INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX idx_logs_ts ON logs(ts DESC, ts_nanos DESC, id DESC);
            "#,
        )
        .unwrap();
        for ts in timestamps {
            conn.execute(
                "INSERT INTO logs(ts, ts_nanos, level, target, estimated_bytes) VALUES(?1, 0, 'INFO', 'test', 1)",
                params![ts],
            )
            .unwrap();
        }
    }
}
