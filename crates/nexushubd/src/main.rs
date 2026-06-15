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
use serde::Deserialize;
use serde_json::{json, Value};
use state::AppState;
use std::{
    fs,
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
static PROBE_LOGS_DB_MAINTENANCE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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

fn probe_runtime(config: &Config) -> ProbeRuntime {
    ProbeRuntime::new(config.clone(), PlatformPaths::current())
}

async fn install_probe_hooks(config: &Config, dry_run: bool) -> Result<Value> {
    let resolved = resolve_codex_paths(
        &config.codex.home,
        config.codex.app_server_socket.as_deref(),
    );
    let hooks_path = resolved.home.join("hooks.json");
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
        "configured_codex_home": resolved.configured_codex_home,
        "resolved_codex_home": resolved.home,
        "codex_home_source": resolved.codex_home_source,
        "discovery_warnings": resolved.discovery_warnings,
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
