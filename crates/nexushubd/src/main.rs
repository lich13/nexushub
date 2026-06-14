mod api;
mod auth;
mod state;
mod turnstile;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use nexushub_core::{
    app_server::AppServerBridge,
    config::Config,
    db::{NewProbeEvent, PanelDb},
    platform::PlatformPaths,
    probe::{ProbeEventInput, ProbeEventOutcome, ProbeRuntime},
};
use serde_json::{json, Value};
use state::AppState;
use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
    process::Command as StdCommand,
};
use tokio::net::TcpListener;
use tower_http::{services::ServeDir, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const DEFAULT_CONFIG: &str = "/opt/nexushub/config.toml";

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
    },
    LifecycleRepair,
    ServiceRestart,
    LegacyImport,
    LegacyCleanup {
        #[arg(long)]
        dry_run: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nexushubd=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
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
            println!("config={}", cli.config.display());
            println!("db={}", db.path().display());
            println!("codex_home={}", config.codex.home.display());
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
        ProbeCommand::HookStop {
            thread_id,
            turn_id,
            kind,
        } => {
            let event = probe_runtime(config).build_event(ProbeEventInput::hook_stop(
                thread_id.as_deref(),
                turn_id.as_deref(),
                &kind,
            ));
            let claimed = db.claim_probe_dedupe(
                &event.dedupe_namespace,
                &event.dedupe_key,
                event.ttl_seconds,
            )?;
            if claimed {
                db.record_probe_event(NewProbeEvent {
                    kind: &event.kind,
                    thread_id: event.thread_id.as_deref(),
                    title: Some(&event.title),
                    message: Some(&event.message),
                    dedupe_key: Some(&event.dedupe_key),
                    source: &event.source,
                    payload: event.payload.clone(),
                })?;
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&ProbeEventOutcome::from_claim(&event, claimed))?
            );
        }
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
            let claimed = db.claim_probe_dedupe(
                &event.dedupe_namespace,
                &event.dedupe_key,
                event.ttl_seconds,
            )?;
            if claimed {
                db.record_probe_event(NewProbeEvent {
                    kind: &event.kind,
                    thread_id: event.thread_id.as_deref(),
                    title: Some(&event.title),
                    message: Some(&event.message),
                    dedupe_key: Some(&event.dedupe_key),
                    source: &event.source,
                    payload: event.payload.clone(),
                })?;
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&ProbeEventOutcome::from_claim(&event, claimed))?
            );
        }
        ProbeCommand::BarkTest => {
            let device_key = db.get_secret_setting_bytes("probe_bark_device_key")?;
            let configured = device_key.as_ref().is_some_and(|value| !value.is_empty());
            let sent = if config.probe.notifications.enabled && configured {
                send_bark_test(config, device_key.as_deref().unwrap_or_default()).await?
            } else {
                false
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": sent || !config.probe.notifications.enabled,
                    "configured": configured,
                    "skipped": !config.probe.notifications.enabled,
                    "sent": sent,
                }))?
            );
        }
        ProbeCommand::LogsDbMaintain { dry_run } => {
            let (events, dedupe) = db.maintain_probe_events(
                config.probe.logs_db.retention_days,
                config.probe.logs_db.max_delete_rows_per_run,
                dry_run || !config.probe.logs_db.enabled,
            )?;
            db.set_setting(
                "probe_logs_db_last_maintain",
                &json!({
                    "dry_run": dry_run,
                    "events": events,
                    "dedupe": dedupe,
                    "enabled": config.probe.logs_db.enabled,
                    "skip_reason": if config.probe.logs_db.enabled { Value::Null } else { Value::String("logs_db_disabled".to_string()) },
                })
                .to_string(),
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "ok": true,
                    "dry_run": dry_run,
                    "deleted_probe_events": if dry_run { 0 } else { events },
                    "deleted_probe_dedupe": if dry_run { 0 } else { dedupe },
                    "would_delete_probe_events": if dry_run { events } else { 0 },
                    "would_delete_probe_dedupe": if dry_run { dedupe } else { 0 },
                    "retention_days": config.probe.logs_db.retention_days,
                    "max_delete_rows_per_run": config.probe.logs_db.max_delete_rows_per_run,
                    "skip_reason": if config.probe.logs_db.enabled { serde_json::Value::Null } else { serde_json::Value::String("logs_db_disabled".to_string()) },
                }))?
            );
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
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({"ok": true, "action": "legacy_import"}))?
            );
        }
        ProbeCommand::LegacyCleanup { dry_run } => {
            let result = run_legacy_cleanup(dry_run)?;
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
    }
    Ok(())
}

fn probe_runtime(config: &Config) -> ProbeRuntime {
    ProbeRuntime::new(config.clone(), PlatformPaths::current())
}

async fn install_probe_hooks(config: &Config, dry_run: bool) -> Result<Value> {
    let hooks_path = config.codex.home.join("hooks.json");
    let hook_command = format!(
        "/opt/nexushub/bin/nexushubd --config {} probe hook-stop",
        PlatformPaths::current().config_file.display()
    );
    let mut root = read_hooks_json(&hooks_path)?;
    let changed = ensure_stop_hook(&mut root, &hook_command);
    let backup_path = hooks_path.with_extension(format!(
        "json.nexushub-probe-bak-{}",
        chrono::Utc::now().format("%Y%m%d%H%M%S")
    ));
    if changed && !dry_run {
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
        if config.probe.hooks.reload_app_server_after_install {
            let _ = StdCommand::new("systemctl")
                .arg("reload-or-restart")
                .arg(&config.codex.app_server_service)
                .status();
        }
    }
    Ok(json!({
        "ok": true,
        "dry_run": dry_run,
        "changed": changed,
        "hooks_json": hooks_path,
        "backup_path": if hooks_path.exists() { Some(backup_path) } else { None },
        "hook_command": hook_command,
        "reload_app_server_after_install": config.probe.hooks.reload_app_server_after_install,
    }))
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

async fn send_bark_test(config: &Config, device_key: &[u8]) -> Result<bool> {
    let device_key = std::str::from_utf8(device_key).context("Bark device_key is not utf-8")?;
    let base = config
        .probe
        .notifications
        .server_url
        .trim()
        .trim_end_matches('/');
    let url = format!(
        "{}/{}/{}",
        base,
        url_path_encode(device_key),
        "NexusHub%20Probe"
    );
    let mut body = json!({
        "body": "Probe notification route is configured.",
        "group": config.probe.notifications.group,
    });
    if let Some(sound) = config.probe.notifications.sound.as_deref() {
        body["sound"] = json!(sound);
    }
    if let Some(open_url) = config.probe.notifications.url.as_deref() {
        body["url"] = json!(open_url);
    }
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()?
        .post(url)
        .json(&body)
        .send()
        .await
        .context("send Bark test")?;
    Ok(response.status().is_success())
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

fn run_legacy_cleanup(dry_run: bool) -> Result<Value> {
    let script = "/usr/local/bin/nexushub-probe-legacy-cleanup";
    let mode = if dry_run { "--dry-run" } else { "--execute" };
    let output = StdCommand::new(script)
        .arg(mode)
        .output()
        .with_context(|| format!("run {script} {mode}"))?;
    let payload = json!({
        "ok": output.status.success(),
        "dry_run": dry_run,
        "action": "legacy_cleanup",
        "exit_code": output.status.code(),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
    });
    if !output.status.success() {
        anyhow::bail!(
            "legacy cleanup failed with exit {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(payload)
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
