use nexushub_core::{
    codex::{resolve_codex_paths, CodexPaths, ResolvedCodexPaths, ThreadDetail},
    config::Config,
    db::PanelDb,
    jobs::JobRunner,
};
use reqwest::Client;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct AppState {
    config: Arc<RwLock<Config>>,
    pub db: PanelDb,
    pub jobs: JobRunner,
    pub http: Client,
    pub login_limiter: Arc<Mutex<LoginLimiter>>,
    pub rollout_detail_cache: Arc<Mutex<HashMap<String, CachedThreadDetail>>>,
}

impl AppState {
    pub fn new(config: Config, db: PanelDb) -> Self {
        let jobs = JobRunner::new(db.clone());
        let login_rate_limit = config.security.login_rate_limit_per_minute;
        Self {
            config: Arc::new(RwLock::new(config)),
            db,
            jobs,
            http: Client::new(),
            login_limiter: Arc::new(Mutex::new(LoginLimiter::new(login_rate_limit))),
            rollout_detail_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn config(&self) -> Config {
        self.config.read().expect("config rwlock").clone()
    }

    pub fn replace_config(&self, config: Config) {
        *self.config.write().expect("config rwlock") = config;
    }

    pub fn resolved_codex_paths(&self) -> ResolvedCodexPaths {
        let config = self.config();
        resolve_codex_paths(
            &config.codex.home,
            config.codex.app_server_socket.as_deref(),
        )
    }

    pub fn codex_paths(&self) -> CodexPaths {
        self.resolved_codex_paths().codex_paths()
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
