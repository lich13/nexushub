use nexushub_core::{
    codex::{
        resolve_codex_paths, CodexAppServerClient, CodexPaths, ResolvedCodexPaths, ThreadDetail,
    },
    config::Config,
    db::PanelDb,
    jobs::JobRunner,
    platform::PlatformPaths,
    services::system::HostSurface,
};
use serde_json::Value;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct AppState {
    config: Arc<RwLock<Config>>,
    host_surface: HostSurface,
    platform: PlatformPaths,
    pub db: PanelDb,
    pub jobs: JobRunner,
    pub app_server_client: CodexAppServerClient,
    pub auth_limiter: Arc<Mutex<AuthLimiter>>,
    pub rollout_detail_cache: Arc<Mutex<HashMap<String, CachedThreadDetail>>>,
    pub probe_status_cache: Arc<Mutex<ProbeStatusCache>>,
}

impl AppState {
    pub fn new(config: Config, db: PanelDb) -> Self {
        Self::new_for_surface(config, db, HostSurface::LinuxServerApi)
    }

    pub fn new_for_surface(config: Config, db: PanelDb, host_surface: HostSurface) -> Self {
        Self::new_for_surface_with_app_server_client(
            config,
            db,
            host_surface,
            CodexAppServerClient::new(),
        )
    }

    fn new_for_surface_with_app_server_client(
        config: Config,
        db: PanelDb,
        host_surface: HostSurface,
        app_server_client: CodexAppServerClient,
    ) -> Self {
        let jobs = JobRunner::new(db.clone());
        let auth_rate_limit = config.security.auth_rate_limit_per_minute;
        let platform = match host_surface {
            HostSurface::LinuxServerApi => {
                PlatformPaths::for_kind(nexushub_core::platform::PlatformKind::Linux)
            }
            HostSurface::DesktopEmbeddedTauri => PlatformPaths::current(),
        };
        Self {
            config: Arc::new(RwLock::new(config)),
            host_surface,
            platform,
            db,
            jobs,
            app_server_client,
            auth_limiter: Arc::new(Mutex::new(AuthLimiter::new(auth_rate_limit))),
            rollout_detail_cache: Arc::new(Mutex::new(HashMap::new())),
            probe_status_cache: Arc::new(Mutex::new(ProbeStatusCache::default())),
        }
    }

    pub fn host_surface(&self) -> HostSurface {
        self.host_surface
    }

    pub fn platform(&self) -> &PlatformPaths {
        &self.platform
    }

    pub fn config(&self) -> Config {
        self.config.read().expect("config rwlock").clone()
    }

    pub fn replace_config(&self, config: Config) {
        *self.config.write().expect("config rwlock") = config;
    }

    pub fn resolved_codex_paths(&self) -> ResolvedCodexPaths {
        let config = self.config();
        resolve_codex_paths(&config.codex.home)
    }

    pub fn codex_paths(&self) -> CodexPaths {
        self.resolved_codex_paths().codex_paths()
    }
}

#[derive(Debug, Default)]
pub struct ProbeStatusCache {
    pub snapshot: Option<CachedProbeStatus>,
    pub refreshing: bool,
}

#[derive(Debug, Clone)]
pub struct CachedProbeStatus {
    pub value: Value,
    pub refreshed_at_unix: i64,
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

pub struct AuthLimiter {
    max_per_minute: u32,
    attempts: HashMap<String, Vec<Instant>>,
}

impl AuthLimiter {
    pub fn new(max_per_minute: u32) -> Self {
        Self {
            max_per_minute,
            attempts: HashMap::new(),
        }
    }

    pub fn check(&mut self, key: &str) -> bool {
        let now = Instant::now();
        let window = Duration::from_secs(60);
        self.attempts.retain(|_, times| {
            times.retain(|instant| now.duration_since(*instant) < window);
            !times.is_empty()
        });
        if self.attempts.len() >= 4096 && !self.attempts.contains_key(key) {
            return false;
        }
        let attempts = self.attempts.entry(key.to_string()).or_default();
        attempts.retain(|instant| now.duration_since(*instant) < window);
        if attempts.len() >= self.max_per_minute as usize {
            return false;
        }
        attempts.push(now);
        true
    }
}
