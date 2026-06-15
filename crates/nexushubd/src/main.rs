mod api;
mod auth;
mod state;
mod turnstile;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use nexushub_core::{
    app_server::AppServerBridge,
    codex::resolve_codex_paths,
    config::{
        patch_probe_config_toml, CodexProbeConfigPatch, Config, ProbeConfigFilePatch,
        ProbeHooksConfigPatch, ProbeLogsDbConfigPatch, ProbeNotificationsConfigPatch,
        ProbeObservabilityConfigPatch, ProbeSettingsPatch,
    },
    db::{NewProbeEvent, PanelDb},
    platform::PlatformPaths,
    probe::{
        ProbeEventInput, ProbeEventOutcome, ProbeLogsDbMaintenanceResult, ProbeRuntime,
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
    process::Command as StdCommand,
};
use tokio::{net::TcpListener, time};
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const DEFAULT_CONFIG: &str = "/opt/nexushub/config.toml";
const PROBE_LOGS_DB_LAST_MAINTAIN_SETTING: &str = "probe_logs_db_last_maintain";
const PROBE_LOGS_DB_LAST_COMPACT_SETTING: &str = "probe_logs_db_last_compact";
const PROBE_LOGS_DB_SCHEDULER_TICK_SECONDS: u64 = 300;
const PROBE_THREAD_SCAN_TICK_SECONDS: u64 = 120;
const PROBE_BARK_BODY_CHUNK_BYTES: usize = 8_192;
static PROBE_LOGS_DB_MAINTENANCE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static PROBE_THREAD_SCAN_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Parser)]
#[command(
    name = "nexushubd",
    version,
    about = "Headless Web panel for cloud Codex app-server"
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
            let resolved = resolve_codex_paths(
                &config.codex.home,
                config.codex.app_server_socket.as_deref(),
            );
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
            let bridge = AppServerBridge::new(config.clone());
            if bridge.enabled() {
                match bridge.health_check().await {
                    Ok(()) => println!("app_server_bridge=ok"),
                    Err(err) => println!("app_server_bridge=error: {err}"),
                }
            } else {
                println!("app_server_bridge=disabled");
            }
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
                eprintln!(
                    "{}",
                    serde_json::to_string(&json!({
                        "probe_event": result.outcome,
                        "bark": result.bark,
                    }))?
                );
                println!("{}", serde_json::to_string(&result.stdout)?);
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
            let event = probe_runtime(config).build_event(ProbeEventInput::notify_completion(
                thread_id.as_deref(),
                turn_id.as_deref(),
            ));
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
                        title: "NexusHub Probe test".to_string(),
                        body: "Probe notification route is configured.".to_string(),
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
            db.set_setting(
                PROBE_LOGS_DB_LAST_MAINTAIN_SETTING,
                &serde_json::to_string(&result)?,
            )?;
            if result.vacuumed {
                db.set_setting(
                    PROBE_LOGS_DB_LAST_COMPACT_SETTING,
                    &serde_json::to_string(&result)?,
                )?;
            }
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        ProbeCommand::LifecycleRepair => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({"ok": true, "action": "lifecycle_repair"}))?
            );
        }
        ProbeCommand::ServiceRestart => {
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({"ok": true, "action": "service_restart"}))?
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
    let stdin_payload = read_hook_stop_stdin_json()?;
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
        payload_last_assistant_message.as_deref(),
        &event_kind,
    ));
    handle_built_probe_event(config, db, event).await
}

fn read_hook_stop_stdin_json() -> Result<Option<Value>> {
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
        .context("parse hook stop stdin json")
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

fn probe_runtime(config: &Config) -> ProbeRuntime {
    ProbeRuntime::new(config.clone(), PlatformPaths::current())
}

fn redact_probe_event(mut event: nexushub_core::db::ProbeEvent) -> nexushub_core::db::ProbeEvent {
    redact_sensitive_json(&mut event.payload);
    event
}

fn redact_sensitive_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                let lower = key.to_ascii_lowercase();
                if lower.contains("device_key")
                    || lower.contains("secret")
                    || lower.contains("token")
                    || lower.contains("password")
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
    let resolved = resolve_codex_paths(
        &config.codex.home,
        config.codex.app_server_socket.as_deref(),
    );
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
    let reload_result = if !dry_run && config.probe.hooks.reload_app_server_after_install {
        Some(reload_app_server(&config.codex.app_server_service))
    } else {
        None
    };
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
        "reload_app_server_after_install": config.probe.hooks.reload_app_server_after_install,
        "reload_result": reload_result,
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

fn reload_app_server(service_name: &str) -> Value {
    let service_name = service_name.trim();
    if service_name.is_empty() {
        return json!({
            "attempted": false,
            "status": "skipped",
            "reason": "empty_service_name",
        });
    }
    match StdCommand::new("systemctl")
        .arg("reload-or-restart")
        .arg(service_name)
        .status()
    {
        Ok(status) => json!({
            "attempted": true,
            "service": service_name,
            "success": status.success(),
            "code": status.code(),
        }),
        Err(err) => json!({
            "attempted": true,
            "service": service_name,
            "success": false,
            "error": err.to_string(),
        }),
    }
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

#[derive(Debug, Clone, Serialize)]
struct ProbeBarkOutcome {
    sent: bool,
    skipped: bool,
    reason: Option<String>,
    http_status: Option<u16>,
    notifications_enabled: bool,
    relevant_switch_enabled: bool,
    device_key_configured: bool,
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
            notifications_enabled,
            relevant_switch_enabled,
            device_key_configured,
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
            notifications_enabled,
            relevant_switch_enabled,
            device_key_configured,
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
            notifications_enabled,
            relevant_switch_enabled,
            device_key_configured,
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
            notifications_enabled,
            relevant_switch_enabled,
            device_key_configured,
            dedupe_key,
        }
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

async fn record_probe_event_with_bark(
    config: &Config,
    db: &PanelDb,
    event: nexushub_core::probe::ProbeBuiltEvent,
) -> Result<(ProbeEventOutcome, ProbeBarkOutcome)> {
    let claimed = db.claim_probe_dedupe(
        &event.dedupe_namespace,
        &event.dedupe_key,
        event.ttl_seconds,
    )?;
    let mut outcome = ProbeEventOutcome::from_claim(&event, claimed);
    let bark = handle_probe_event_bark(config, db, &event, claimed).await?;
    if claimed {
        let mut payload = event.payload.clone();
        payload["bark"] = serde_json::to_value(&bark)?;
        db.record_probe_event(NewProbeEvent {
            kind: &event.kind,
            thread_id: event.thread_id.as_deref(),
            title: Some(&event.title),
            message: Some(&event.message),
            dedupe_key: Some(&event.dedupe_key),
            source: &event.source,
            payload,
        })?;
    } else {
        outcome.recorded = false;
        outcome.duplicate = true;
    }

    Ok((outcome, bark))
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
            title: event.title.clone(),
            body: event.message.clone(),
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
    let base = config
        .probe
        .notifications
        .server_url
        .trim()
        .trim_end_matches('/');
    let mut body = json!({
        "body": request.body,
        "group": config.probe.notifications.group,
    });
    if let Some(sound) = config.probe.notifications.sound.as_deref() {
        body["sound"] = json!(sound);
    }
    if let Some(open_url) = config.probe.notifications.url.as_deref() {
        body["url"] = json!(open_url);
    }
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
    let chunks = utf8_chunks(&request.body, PROBE_BARK_BODY_CHUNK_BYTES);
    let chunk_count = chunks.len();
    let mut last_status = None;
    for (index, chunk) in chunks.iter().enumerate() {
        let chunk_title = if chunk_count > 1 {
            format!("{} ({}/{})", request.title, index + 1, chunk_count)
        } else {
            request.title.clone()
        };
        let chunk_url = format!(
            "{}/{}/{}",
            base,
            url_path_encode(device_key),
            url_path_encode(&chunk_title)
        );
        body["body"] = json!(chunk);
        let response = client.post(chunk_url).json(&body).send().await;
        let response = match response {
            Ok(response) => response,
            Err(err) => {
                tracing::warn!("Bark notification request failed: {err}");
                return Ok(ProbeBarkOutcome::failed_request(
                    if err.is_timeout() {
                        "timeout"
                    } else {
                        "request_error"
                    },
                    config.probe.notifications.enabled,
                    true,
                    true,
                    Some(request.dedupe_key.clone()),
                ));
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
            ));
        }
    }
    Ok(ProbeBarkOutcome::sent(
        last_status.unwrap_or(0),
        config.probe.notifications.enabled,
        true,
        true,
        Some(request.dedupe_key.clone()),
    ))
}

fn url_path_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
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
            "codex": ["home", "app_server_service", "host_label"],
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
            app_server_service: nonempty(&legacy.server.app_server_service),
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
    let result = tokio::task::spawn_blocking(move || {
        probe_runtime(&config).maintain_logs_db_with_compaction(false, compact)
    })
    .await
    .context("join probe logs DB maintenance worker")??;
    state.db.set_setting(
        PROBE_LOGS_DB_LAST_MAINTAIN_SETTING,
        &serde_json::to_string(&result)?,
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
            if thread.active_turn_id.as_deref().is_none()
                && thread.active_job_id.as_deref().is_none()
            {
                continue;
            }
            let mut event =
                probe_runtime(&config).build_event(ProbeEventInput::hook_stop_with_context(
                    Some(thread.id.as_str()),
                    thread.active_turn_id.as_deref(),
                    Some(thread.id.as_str()),
                    None,
                    None,
                    status,
                ));
            match status {
                "reply-needed" => {
                    event.title = "需要回复".to_string();
                    event.message = format!("Codex thread {} needs a reply", thread.id);
                    event.source = "nexushubd probe passive-scan".to_string();
                }
                "recoverable" => {
                    event.title = "可恢复任务".to_string();
                    event.message = format!("Codex thread {} is recoverable", thread.id);
                    event.source = "nexushubd probe passive-scan".to_string();
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
    Ok(codex_app_server_is_inactive(&config.codex.app_server_service).await)
}

async fn codex_app_server_is_inactive(service_name: &str) -> bool {
    let service_name = service_name.trim();
    if service_name.is_empty() {
        return true;
    }
    match tokio::process::Command::new("systemctl")
        .arg("is-active")
        .arg("--quiet")
        .arg(service_name)
        .status()
        .await
    {
        Ok(status) => !status.success(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::time::SystemTime;

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
            events[0].payload["last_assistant_message"],
            "assistant body"
        );
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
        assert!(!serde_json::to_string(&duplicate)
            .unwrap()
            .contains("super-secret-device"));

        let events = db.list_probe_events(10).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].payload["bark"]["device_key_configured"], true);
        assert!(events[0].payload["bark"].get("device_key").is_none());
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
    async fn send_bark_builds_redacted_request_and_reports_non_success() {
        let server = TestHttpServer::start_n(
            1,
            "HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
        );
        let mut config = Config::default();
        config.probe.notifications.server_url = server.url();
        config.probe.notifications.group = "Probe Group".to_string();
        config.probe.notifications.sound = Some("bell".to_string());
        config.probe.notifications.url = Some("https://661313.xyz/nexushub/".to_string());

        let request = ProbeBarkRequest {
            title: "NexusHub Probe test".to_string(),
            body: "Probe notification route is configured.".to_string(),
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
        assert_eq!(result.reason.as_deref(), Some("http_status"));
        assert_eq!(result.http_status, Some(503));
        assert!(result.device_key_configured);
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("device key"));
        let raw = server.request();
        assert!(raw.starts_with("POST /device%20key%2Fwith%20spaces/NexusHub%20Probe%20test "));
        assert!(raw.contains(r#""body":"Probe notification route is configured.""#));
        assert!(raw.contains(r#""group":"Probe Group""#));
        assert!(raw.contains(r#""sound":"bell""#));
        assert!(raw.contains(r#""url":"https://661313.xyz/nexushub/""#));
    }

    #[tokio::test]
    async fn send_bark_splits_long_body_on_utf8_boundaries() {
        let server = TestHttpServer::start_n(
            3,
            "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
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
        let requests = server.requests(3);
        assert_eq!(requests.len(), 3);
        for (index, raw) in requests.iter().enumerate() {
            assert!(raw.starts_with(&format!(
                "POST /device-key/NexusHub%20Probe%20long%20body%20%28{}%2F3%29 ",
                index + 1
            )));
            let body = raw.split("\r\n\r\n").nth(1).unwrap();
            let payload: Value = serde_json::from_str(body).unwrap();
            let chunk = payload["body"].as_str().unwrap();
            assert!(chunk.len() <= PROBE_BARK_BODY_CHUNK_BYTES);
            assert!(chunk.is_char_boundary(chunk.len()));
        }
        let combined = requests
            .iter()
            .map(|raw| {
                let body = raw.split("\r\n\r\n").nth(1).unwrap();
                serde_json::from_str::<Value>(body).unwrap()["body"]
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect::<String>();
        assert_eq!(combined, request.body);
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
        config.codex.home = PathBuf::from("auto");
        config.codex.app_server_socket = Some(
            codex_home
                .join("app-server-control")
                .join("app-server-control.sock"),
        );
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
        assert!(stored["configured_codex_home"].is_null());
        assert_eq!(
            stored["resolved_codex_home"],
            codex_home.to_string_lossy().as_ref()
        );
        assert_eq!(stored["codex_home_source"], "socket");
        assert_eq!(stored["logs_db_source"], "socket");

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
        config.codex.home = PathBuf::from("auto");
        config.codex.app_server_socket = Some(
            codex_home
                .join("app-server-control")
                .join("app-server-control.sock"),
        );
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
        assert_eq!(result.configured_codex_home, None);
        assert_eq!(result.resolved_codex_home, codex_home);
        assert_eq!(result.codex_home_source, "socket");
        assert_eq!(result.logs_db_source, "socket");
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
    async fn probe_thread_scan_suppresses_stale_reply_needed_when_thread_is_running() {
        let dir = temp_test_dir("nexushub-thread-scan-running-suppression");
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
                        "question": "Continue?",
                        "options": []
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
        db.create_job("job-live", "codex_chat", "running visible followup")
            .unwrap();
        db.link_job_thread("job-live", Some("stale-reply"), Some("turn-live"))
            .unwrap();

        let count = run_probe_thread_scan_if_due(state).await.unwrap();

        assert_eq!(count, 0);
        assert!(db.list_probe_events(10).unwrap().is_empty());
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
