mod api;
mod auth;
mod state;
mod turnstile;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use nexushub_core::{app_server::AppServerBridge, config::Config, db::PanelDb};
use state::AppState;
use std::{net::SocketAddr, path::PathBuf};
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
        Command::Serve => serve(cli.config).await?,
    }
    Ok(())
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
