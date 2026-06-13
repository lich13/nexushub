use nexushub_core::{
    app_server::AppServerBridge, codex::ThreadDetail, config::Config, db::PanelDb, jobs::JobRunner,
};
use reqwest::Client;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: PanelDb,
    pub jobs: JobRunner,
    pub bridge: AppServerBridge,
    pub http: Client,
    pub login_limiter: std::sync::Arc<Mutex<LoginLimiter>>,
    pub rollout_detail_cache: std::sync::Arc<Mutex<HashMap<String, CachedThreadDetail>>>,
}

impl AppState {
    pub fn new(config: Config, db: PanelDb) -> Self {
        let jobs = JobRunner::new(db.clone());
        let bridge = AppServerBridge::new(config.clone());
        Self {
            config: config.clone(),
            db,
            jobs,
            bridge,
            http: Client::new(),
            login_limiter: std::sync::Arc::new(Mutex::new(LoginLimiter::new(
                config.security.login_rate_limit_per_minute,
            ))),
            rollout_detail_cache: std::sync::Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CachedThreadDetail {
    pub signature: ThreadDetailCacheSignature,
    pub detail: ThreadDetail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadDetailCacheSignature {
    pub rollout_path: Option<PathBuf>,
    pub rollout: Option<FileSignature>,
    pub state_db: Option<FileSignature>,
    pub session_index: Option<FileSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileSignature {
    pub len: u64,
    pub modified_ms: Option<u128>,
}

pub struct LoginLimiter {
    max_per_minute: u32,
    attempts: HashMap<String, Vec<Instant>>,
}

impl LoginLimiter {
    pub fn new(max_per_minute: u32) -> Self {
        Self {
            max_per_minute,
            attempts: HashMap::new(),
        }
    }

    pub fn check(&mut self, key: &str) -> bool {
        let now = Instant::now();
        let window = Duration::from_secs(60);
        let attempts = self.attempts.entry(key.to_string()).or_default();
        attempts.retain(|instant| now.duration_since(*instant) < window);
        if attempts.len() >= self.max_per_minute as usize {
            return false;
        }
        attempts.push(now);
        true
    }
}
