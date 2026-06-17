use crate::{
    crypto::SecretBox,
    platform::{PlatformKind, PlatformPaths},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
};

pub const DEFAULT_HOST_LABEL: &str = "43.155.235.227";
pub const DEFAULT_SESSION_TTL_SECONDS: u64 = 31_536_000;
pub const LEGACY_SESSION_TTL_SECONDS: u64 = 604_800;
pub const DEFAULT_TURNSTILE_SITE_KEY: &str = "0x4AAAAAADPfCPB_O-N3j6ON";
pub const DEFAULT_TURNSTILE_EXPECTED_HOSTNAME: &str = "661313.xyz";
pub const DEFAULT_TURNSTILE_EXPECTED_ACTION: &str = "login";
const DEFAULT_LOOPBACK_PORT: u16 = 15742;
const LEGACY_LOOPBACK_PORT: u16 = 15732;
const LEGACY_PRECHECK_COMMAND_SIMPLE: &str = "codex --version && sudo -n codex --version && /usr/local/bin/codex-raw --version && sqlite3 /root/.codex/state_5.sqlite 'pragma integrity_check;' && /home/ubuntu/codex-admin/bin/codex-cloud-doctor";
const LEGACY_PRECHECK_COMMAND_WITH_AUDIT: &str = "codex --version && sudo -n codex --version && /usr/local/bin/codex-raw --version && readlink -f /usr/local/bin/codex && readlink -f /usr/local/bin/codex-raw && sqlite3 /root/.codex/state_5.sqlite 'pragma integrity_check;' && sqlite3 /root/.codex/state_5.sqlite \"select count(*) total, sum(archived_at is null) active, sum(archived_at is not null) archived from threads;\" && /home/ubuntu/codex-admin/bin/codex-cloud-doctor";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub codex: CodexConfig,
    #[serde(default)]
    pub probe: ProbeConfig,
    pub security: SecurityConfig,
    pub paths: PathConfig,
    pub update: UpdateConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub listen: SocketAddr,
    pub public_base_url: Option<String>,
    pub trust_forwarded_headers: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodexConfig {
    #[serde(default = "default_codex_home")]
    pub home: PathBuf,
    pub workspace: PathBuf,
    #[serde(default, skip_serializing)]
    pub app_server_service: String,
    #[serde(default, skip_serializing)]
    pub app_server_socket: Option<PathBuf>,
    #[serde(default, skip_serializing)]
    pub bridge_enabled: bool,
    #[serde(default, skip_serializing)]
    pub bridge_transport: Option<String>,
    #[serde(default, skip_serializing)]
    pub bridge_timeout_seconds: Option<u64>,
    pub host_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_probe_poll_seconds")]
    pub poll_seconds: u64,
    #[serde(default = "default_probe_recent_limit")]
    pub recent_limit: usize,
    #[serde(default)]
    pub hooks: ProbeHooksConfig,
    #[serde(default)]
    pub notifications: ProbeNotificationsConfig,
    #[serde(default)]
    pub observability: ProbeObservabilityConfig,
    #[serde(default)]
    pub logs_db: ProbeLogsDbConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeHooksConfig {
    #[serde(default = "default_true")]
    pub manage_stop_hook: bool,
    #[serde(default = "default_true")]
    pub reload_app_server_after_install: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeNotificationsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_bark_server_url")]
    pub server_url: String,
    #[serde(default)]
    pub sound: Option<String>,
    #[serde(default = "default_probe_notification_group")]
    pub group: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default = "default_true")]
    pub notify_completion: bool,
    #[serde(default = "default_true")]
    pub notify_reply_needed: bool,
    #[serde(default = "default_true")]
    pub notify_recoverable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeObservabilityConfig {
    #[serde(default = "default_hook_event_max_lines")]
    pub hook_event_max_lines: usize,
    #[serde(default = "default_hook_cooldown_max_lines")]
    pub hook_cooldown_max_lines: usize,
    #[serde(default = "default_log_max_bytes")]
    pub log_max_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeLogsDbConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_logs_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_logs_maintenance_interval_hours")]
    pub maintenance_interval_hours: u32,
    #[serde(default = "default_true")]
    pub maintain_on_codex_exit: bool,
    #[serde(default = "default_codex_exit_grace_seconds")]
    pub codex_exit_grace_seconds: u64,
    #[serde(default = "default_codex_exit_max_wait_seconds")]
    pub codex_exit_max_wait_seconds: u64,
    #[serde(default = "default_delete_chunk_rows")]
    pub delete_chunk_rows: u32,
    #[serde(default = "default_max_delete_rows_per_run")]
    pub max_delete_rows_per_run: u32,
    #[serde(default = "default_busy_timeout_ms")]
    pub busy_timeout_ms: u64,
    #[serde(default = "default_true")]
    pub auto_compact_when_codex_closed: bool,
    #[serde(default = "default_compact_interval_hours")]
    pub compact_interval_hours: u32,
    #[serde(default = "default_compact_min_freelist_mb")]
    pub compact_min_freelist_mb: u64,
    #[serde(default = "default_compact_min_freelist_ratio_percent")]
    pub compact_min_freelist_ratio_percent: u32,
    #[serde(default = "default_minimum_free_space_mb")]
    pub minimum_free_space_mb: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeConfigFilePatch {
    pub codex: Option<CodexProbeConfigPatch>,
    pub probe: Option<ProbeSettingsPatch>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CodexProbeConfigPatch {
    #[serde(default, deserialize_with = "deserialize_optional_string_field")]
    pub home: Option<Option<String>>,
    pub workspace: Option<String>,
    pub host_label: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeSettingsPatch {
    pub enabled: Option<bool>,
    pub poll_seconds: Option<u64>,
    pub recent_limit: Option<usize>,
    pub hooks: Option<ProbeHooksConfigPatch>,
    pub notifications: Option<ProbeNotificationsConfigPatch>,
    pub observability: Option<ProbeObservabilityConfigPatch>,
    pub logs_db: Option<ProbeLogsDbConfigPatch>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeHooksConfigPatch {
    pub manage_stop_hook: Option<bool>,
    pub reload_app_server_after_install: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeNotificationsConfigPatch {
    pub enabled: Option<bool>,
    pub server_url: Option<String>,
    pub sound: Option<Option<String>>,
    pub group: Option<String>,
    pub url: Option<Option<String>>,
    pub notify_completion: Option<bool>,
    pub notify_reply_needed: Option<bool>,
    pub notify_recoverable: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeObservabilityConfigPatch {
    pub hook_event_max_lines: Option<usize>,
    pub hook_cooldown_max_lines: Option<usize>,
    pub log_max_bytes: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeLogsDbConfigPatch {
    pub enabled: Option<bool>,
    pub retention_days: Option<u32>,
    pub maintenance_interval_hours: Option<u32>,
    pub maintain_on_codex_exit: Option<bool>,
    pub codex_exit_grace_seconds: Option<u64>,
    pub codex_exit_max_wait_seconds: Option<u64>,
    pub delete_chunk_rows: Option<u32>,
    pub max_delete_rows_per_run: Option<u32>,
    pub busy_timeout_ms: Option<u64>,
    pub auto_compact_when_codex_closed: Option<bool>,
    pub compact_interval_hours: Option<u32>,
    pub compact_min_freelist_mb: Option<u64>,
    pub compact_min_freelist_ratio_percent: Option<u32>,
    pub minimum_free_space_mb: Option<u64>,
}

fn default_codex_home() -> PathBuf {
    PathBuf::from("auto")
}

fn default_true() -> bool {
    true
}

fn default_probe_poll_seconds() -> u64 {
    15
}

fn default_probe_recent_limit() -> usize {
    50
}

fn default_bark_server_url() -> String {
    "https://api.day.app".to_string()
}

fn default_probe_notification_group() -> String {
    "NexusHub".to_string()
}

fn default_hook_event_max_lines() -> usize {
    500
}

fn default_hook_cooldown_max_lines() -> usize {
    1_000
}

fn default_log_max_bytes() -> usize {
    5 * 1024 * 1024
}

fn default_logs_retention_days() -> u32 {
    2
}

fn default_logs_maintenance_interval_hours() -> u32 {
    6
}

fn default_codex_exit_grace_seconds() -> u64 {
    5
}

fn default_codex_exit_max_wait_seconds() -> u64 {
    1_800
}

fn default_delete_chunk_rows() -> u32 {
    5_000
}

fn default_max_delete_rows_per_run() -> u32 {
    100_000
}

fn default_busy_timeout_ms() -> u64 {
    500
}

fn default_compact_interval_hours() -> u32 {
    24
}

fn default_compact_min_freelist_mb() -> u64 {
    256
}

fn default_compact_min_freelist_ratio_percent() -> u32 {
    20
}

fn default_minimum_free_space_mb() -> u64 {
    1_024
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            poll_seconds: default_probe_poll_seconds(),
            recent_limit: default_probe_recent_limit(),
            hooks: ProbeHooksConfig::default(),
            notifications: ProbeNotificationsConfig::default(),
            observability: ProbeObservabilityConfig::default(),
            logs_db: ProbeLogsDbConfig::default(),
        }
    }
}

impl Default for ProbeHooksConfig {
    fn default() -> Self {
        Self {
            manage_stop_hook: true,
            reload_app_server_after_install: false,
        }
    }
}

impl Default for ProbeNotificationsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: default_bark_server_url(),
            sound: None,
            group: default_probe_notification_group(),
            url: None,
            notify_completion: true,
            notify_reply_needed: true,
            notify_recoverable: true,
        }
    }
}

impl Default for ProbeObservabilityConfig {
    fn default() -> Self {
        Self {
            hook_event_max_lines: default_hook_event_max_lines(),
            hook_cooldown_max_lines: default_hook_cooldown_max_lines(),
            log_max_bytes: default_log_max_bytes(),
        }
    }
}

impl Default for ProbeLogsDbConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: default_logs_retention_days(),
            maintenance_interval_hours: default_logs_maintenance_interval_hours(),
            maintain_on_codex_exit: true,
            codex_exit_grace_seconds: default_codex_exit_grace_seconds(),
            codex_exit_max_wait_seconds: default_codex_exit_max_wait_seconds(),
            delete_chunk_rows: default_delete_chunk_rows(),
            max_delete_rows_per_run: default_max_delete_rows_per_run(),
            busy_timeout_ms: default_busy_timeout_ms(),
            auto_compact_when_codex_closed: true,
            compact_interval_hours: default_compact_interval_hours(),
            compact_min_freelist_mb: default_compact_min_freelist_mb(),
            compact_min_freelist_ratio_percent: default_compact_min_freelist_ratio_percent(),
            minimum_free_space_mb: default_minimum_free_space_mb(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub secret_key: String,
    pub cookie_secure: bool,
    pub session_ttl_seconds: u64,
    pub login_rate_limit_per_minute: u32,
    #[serde(default = "default_turnstile_expected_hostname")]
    pub turnstile_expected_hostname: Option<String>,
    #[serde(default = "default_turnstile_expected_action")]
    pub turnstile_expected_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathConfig {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub webui_dir: PathBuf,
    pub log_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    pub precheck_command: String,
    pub update_command: String,
    pub prune_command: String,
    pub doctor_command: String,
    #[serde(default = "default_panel_update_command")]
    pub panel_update_command: String,
    #[serde(default = "default_panel_precheck_command")]
    pub panel_precheck_command: String,
}

fn default_turnstile_expected_hostname() -> Option<String> {
    Some(DEFAULT_TURNSTILE_EXPECTED_HOSTNAME.to_string())
}

fn default_turnstile_expected_action() -> Option<String> {
    Some(DEFAULT_TURNSTILE_EXPECTED_ACTION.to_string())
}

fn default_update_command() -> String {
    "/usr/local/bin/nexushub-codex-update".to_string()
}

fn default_precheck_command() -> String {
    "/usr/local/bin/nexushub-codex-precheck".to_string()
}

fn default_prune_command() -> String {
    "/usr/local/bin/nexushub-codex-prune".to_string()
}

fn default_panel_update_command() -> String {
    "/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest".to_string()
}

fn default_panel_precheck_command() -> String {
    default_panel_precheck_command_for_platform(&PlatformPaths::current())
}

fn default_panel_precheck_command_for_platform(platform: &PlatformPaths) -> String {
    match platform.kind {
        PlatformKind::Linux => format!(
            "test -x /usr/local/bin/nexushub-update && systemctl is-active nexushub && curl -fsS http://127.0.0.1:{DEFAULT_LOOPBACK_PORT}/healthz"
        ),
        PlatformKind::Macos => format!(
            "test -d {}",
            shell_quote(&platform.data_dir.display().to_string())
        ),
        PlatformKind::Windows => format!("curl -fsS http://127.0.0.1:{DEFAULT_LOOPBACK_PORT}/healthz"),
    }
}

fn default_doctor_command_for_platform(platform: &PlatformPaths) -> String {
    match platform.kind {
        PlatformKind::Linux => "/home/ubuntu/codex-admin/bin/codex-cloud-doctor".to_string(),
        _ => format!(
            "{} --config {} doctor",
            shell_quote(&platform.daemon_binary().display().to_string()),
            shell_quote(&platform.config_file.display().to_string())
        ),
    }
}

fn default_workspace_for_platform(platform: &PlatformPaths, home: &Path) -> PathBuf {
    match platform.kind {
        PlatformKind::Linux => PathBuf::from("/home/ubuntu/codex-workspace"),
        PlatformKind::Macos => home.join("nexushub-workspace"),
        PlatformKind::Windows => PathBuf::from(r"%USERPROFILE%\NexusHub\workspace"),
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn current_home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"))
}

fn current_platform_kind() -> PlatformKind {
    #[cfg(target_os = "macos")]
    {
        PlatformKind::Macos
    }
    #[cfg(target_os = "windows")]
    {
        PlatformKind::Windows
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        PlatformKind::Linux
    }
}

impl Config {
    pub fn for_platform_kind(kind: PlatformKind) -> Self {
        Self::for_platform_kind_with_home(kind, current_home_dir())
    }

    pub fn for_platform_kind_with_home(kind: PlatformKind, home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        let listen = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_LOOPBACK_PORT);
        let platform = PlatformPaths::for_kind_with_home(kind, &home);
        Self {
            server: ServerConfig {
                listen,
                public_base_url: None,
                trust_forwarded_headers: true,
            },
            codex: CodexConfig {
                home: default_codex_home(),
                workspace: default_workspace_for_platform(&platform, &home),
                app_server_service: String::new(),
                app_server_socket: None,
                bridge_enabled: false,
                bridge_transport: None,
                bridge_timeout_seconds: None,
                host_label: DEFAULT_HOST_LABEL.to_string(),
            },
            probe: ProbeConfig::default(),
            security: SecurityConfig {
                secret_key: String::new(),
                cookie_secure: true,
                session_ttl_seconds: DEFAULT_SESSION_TTL_SECONDS,
                login_rate_limit_per_minute: 8,
                turnstile_expected_hostname: default_turnstile_expected_hostname(),
                turnstile_expected_action: default_turnstile_expected_action(),
            },
            paths: PathConfig {
                data_dir: platform.data_dir.clone(),
                db_path: platform.data_dir.join("nexushub.sqlite"),
                webui_dir: platform.webui_dir.clone(),
                log_dir: platform.log_dir.clone(),
            },
            update: UpdateConfig {
                precheck_command: default_precheck_command(),
                update_command: default_update_command(),
                prune_command: default_prune_command(),
                doctor_command: default_doctor_command_for_platform(&platform),
                panel_update_command: default_panel_update_command(),
                panel_precheck_command: default_panel_precheck_command_for_platform(&platform),
            },
        }
    }

    pub fn current_default_config_path() -> PathBuf {
        PlatformPaths::current().config_file
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::for_platform_kind(current_platform_kind())
    }
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let text =
            fs::read_to_string(path).with_context(|| format!("read config {}", path.display()))?;
        let mut config: Self =
            toml::from_str(&text).with_context(|| format!("parse config {}", path.display()))?;
        config.load_sibling_env(path)?;
        config.normalize();
        Ok(config)
    }

    pub fn write_default(path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create config dir {}", parent.display()))?;
        }
        let text = toml::to_string_pretty(&Self::default()).context("serialize default config")?;
        fs::write(path, text).with_context(|| format!("write config {}", path.display()))
    }

    pub fn normalize(&mut self) {
        if self.server.listen
            == SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), LEGACY_LOOPBACK_PORT)
        {
            self.server.listen =
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_LOOPBACK_PORT);
        }
        if let Ok(value) = env::var("NEXUSHUB_SECRET_KEY") {
            self.security.secret_key = value;
        }
        if self.security.session_ttl_seconds == LEGACY_SESSION_TTL_SECONDS {
            self.security.session_ttl_seconds = DEFAULT_SESSION_TTL_SECONDS;
        } else if self.security.session_ttl_seconds < 300 {
            self.security.session_ttl_seconds = 300;
        }
        if self.security.login_rate_limit_per_minute == 0 {
            self.security.login_rate_limit_per_minute = 8;
        }
        if self.codex.host_label.trim().is_empty() || self.codex.host_label == legacy_host_label() {
            self.codex.host_label = DEFAULT_HOST_LABEL.to_string();
        }
        self.probe.normalize();
        if self.security.turnstile_expected_action.is_none() {
            self.security.turnstile_expected_action = default_turnstile_expected_action();
        }
        if is_legacy_precheck_command(&self.update.precheck_command) {
            self.update.precheck_command = default_precheck_command();
        }
        if self.update.precheck_command == "/usr/local/bin/codex-cloud-panel-codex-precheck" {
            self.update.precheck_command = default_precheck_command();
        }
        if self.update.update_command == "/usr/local/bin/codex-cloud-panel-codex-update" {
            self.update.update_command = default_update_command();
        }
        if self.update.prune_command == "/usr/local/bin/codex-cloud-panel-codex-prune" {
            self.update.prune_command = default_prune_command();
        }
        if self.update.panel_update_command.trim().is_empty() {
            self.update.panel_update_command = default_panel_update_command();
        }
        if self
            .update
            .panel_update_command
            .contains("codex-cloud-panel-update")
            || self
                .update
                .panel_update_command
                .contains("lich13/codex-cloud-panel")
        {
            self.update.panel_update_command = default_panel_update_command();
        }
        if self.update.panel_precheck_command.trim().is_empty() {
            self.update.panel_precheck_command = default_panel_precheck_command();
        }
        if self
            .update
            .panel_precheck_command
            .contains("codex-cloud-panel")
            || self
                .update
                .panel_precheck_command
                .contains("http://127.0.0.1:15732/healthz")
        {
            self.update.panel_precheck_command = default_panel_precheck_command();
        }
    }

    pub fn secret_box(&self) -> Result<SecretBox> {
        let value = self.security.secret_key.trim();
        if value.is_empty() {
            anyhow::bail!("NEXUSHUB_SECRET_KEY is required");
        }
        SecretBox::from_key_material(value)
    }

    fn load_sibling_env(&mut self, config_path: &Path) -> Result<()> {
        if !self.security.secret_key.trim().is_empty() {
            return Ok(());
        }
        let Some(parent) = config_path.parent() else {
            return Ok(());
        };
        let env_path = parent.join("env");
        if !env_path.exists() {
            return Ok(());
        }
        let text = fs::read_to_string(&env_path)
            .with_context(|| format!("read env {}", env_path.display()))?;
        if let Some(value) = env_file_value(&text, "NEXUSHUB_SECRET_KEY") {
            self.security.secret_key = value;
        }
        Ok(())
    }
}

impl ProbeConfig {
    pub fn normalize(&mut self) {
        self.poll_seconds = self.poll_seconds.clamp(5, 3_600);
        self.recent_limit = self.recent_limit.clamp(1, 500);
        if !valid_probe_notification_server_url(&self.notifications.server_url) {
            self.notifications.server_url = default_bark_server_url();
        }
        if self.notifications.group.trim().is_empty() {
            self.notifications.group = default_probe_notification_group();
        }
        self.observability.hook_event_max_lines =
            self.observability.hook_event_max_lines.clamp(10, 10_000);
        self.observability.hook_cooldown_max_lines =
            self.observability.hook_cooldown_max_lines.clamp(10, 10_000);
        self.observability.log_max_bytes = self.observability.log_max_bytes.clamp(4_096, 8_388_608);
        self.logs_db.retention_days = self.logs_db.retention_days.clamp(1, 3_650);
        self.logs_db.maintenance_interval_hours =
            self.logs_db.maintenance_interval_hours.clamp(1, 24 * 365);
        self.logs_db.codex_exit_grace_seconds =
            self.logs_db.codex_exit_grace_seconds.clamp(0, 3_600);
        self.logs_db.codex_exit_max_wait_seconds =
            self.logs_db.codex_exit_max_wait_seconds.clamp(1, 86_400);
        self.logs_db.delete_chunk_rows = self.logs_db.delete_chunk_rows.clamp(1, 1_000_000);
        self.logs_db.max_delete_rows_per_run =
            self.logs_db.max_delete_rows_per_run.clamp(1, 10_000_000);
        self.logs_db.busy_timeout_ms = self.logs_db.busy_timeout_ms.clamp(100, 120_000);
        self.logs_db.compact_interval_hours =
            self.logs_db.compact_interval_hours.clamp(1, 24 * 365);
        self.logs_db.compact_min_freelist_ratio_percent = self
            .logs_db
            .compact_min_freelist_ratio_percent
            .clamp(1, 100);
    }
}

fn legacy_host_label() -> String {
    ["tencent", "wanka"].join("-")
}

fn is_legacy_precheck_command(value: &str) -> bool {
    let normalized = value.trim();
    normalized == LEGACY_PRECHECK_COMMAND_SIMPLE || normalized == LEGACY_PRECHECK_COMMAND_WITH_AUDIT
}

fn env_file_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let (line_key, value) = line.split_once('=')?;
        if line_key.trim() != key {
            return None;
        }
        Some(
            value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string(),
        )
    })
}

pub fn valid_probe_notification_server_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value.trim()) else {
        return false;
    };
    match url.scheme() {
        "https" => true,
        "http" => url.host_str().is_some_and(is_loopback_host),
        _ => false,
    }
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host == "127.0.0.1"
        || host == "::1"
        || host.starts_with("127.")
}

fn deserialize_optional_string_field<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<Option<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}

fn non_auto_string(value: Option<&str>) -> Option<&str> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("auto")).then_some(value)
    })
}

pub fn patch_probe_config_toml(text: &str, patch: &ProbeConfigFilePatch) -> Result<String> {
    let mut editor = TomlPatchEditor::new(text);
    remove_legacy_app_server_patch_keys(&mut editor);
    if let Some(codex) = patch.codex.as_ref() {
        if let Some(home) = codex.home.as_ref() {
            match non_auto_string(home.as_deref()) {
                Some(value) => editor.set_string("codex", "home", Some(value)),
                None => editor.remove_key("codex", "home"),
            }
        }
        editor.set_string("codex", "workspace", codex.workspace.as_deref());
        editor.set_string("codex", "host_label", codex.host_label.as_deref());
    }
    if let Some(probe) = patch.probe.as_ref() {
        editor.set_bool("probe", "enabled", probe.enabled);
        editor.set_u64("probe", "poll_seconds", probe.poll_seconds);
        editor.set_usize("probe", "recent_limit", probe.recent_limit);
        if let Some(hooks) = probe.hooks.as_ref() {
            editor.set_bool("probe.hooks", "manage_stop_hook", hooks.manage_stop_hook);
        }
        if let Some(notifications) = probe.notifications.as_ref() {
            editor.set_bool("probe.notifications", "enabled", notifications.enabled);
            editor.set_string(
                "probe.notifications",
                "server_url",
                notifications.server_url.as_deref(),
            );
            if let Some(value) = notifications.sound.as_ref() {
                editor.set_string("probe.notifications", "sound", value.as_deref());
            }
            editor.set_string(
                "probe.notifications",
                "group",
                notifications.group.as_deref(),
            );
            if let Some(value) = notifications.url.as_ref() {
                editor.set_string("probe.notifications", "url", value.as_deref());
            }
            editor.set_bool(
                "probe.notifications",
                "notify_completion",
                notifications.notify_completion,
            );
            editor.set_bool(
                "probe.notifications",
                "notify_reply_needed",
                notifications.notify_reply_needed,
            );
            editor.set_bool(
                "probe.notifications",
                "notify_recoverable",
                notifications.notify_recoverable,
            );
        }
        if let Some(observability) = probe.observability.as_ref() {
            editor.set_usize(
                "probe.observability",
                "hook_event_max_lines",
                observability.hook_event_max_lines,
            );
            editor.set_usize(
                "probe.observability",
                "hook_cooldown_max_lines",
                observability.hook_cooldown_max_lines,
            );
            editor.set_usize(
                "probe.observability",
                "log_max_bytes",
                observability.log_max_bytes,
            );
        }
        if let Some(logs_db) = probe.logs_db.as_ref() {
            editor.set_bool("probe.logs_db", "enabled", logs_db.enabled);
            editor.set_u32("probe.logs_db", "retention_days", logs_db.retention_days);
            editor.set_u32(
                "probe.logs_db",
                "maintenance_interval_hours",
                logs_db.maintenance_interval_hours,
            );
            editor.set_bool(
                "probe.logs_db",
                "maintain_on_codex_exit",
                logs_db.maintain_on_codex_exit,
            );
            editor.set_u64(
                "probe.logs_db",
                "codex_exit_grace_seconds",
                logs_db.codex_exit_grace_seconds,
            );
            editor.set_u64(
                "probe.logs_db",
                "codex_exit_max_wait_seconds",
                logs_db.codex_exit_max_wait_seconds,
            );
            editor.set_u32(
                "probe.logs_db",
                "delete_chunk_rows",
                logs_db.delete_chunk_rows,
            );
            editor.set_u32(
                "probe.logs_db",
                "max_delete_rows_per_run",
                logs_db.max_delete_rows_per_run,
            );
            editor.set_u64("probe.logs_db", "busy_timeout_ms", logs_db.busy_timeout_ms);
            editor.set_bool(
                "probe.logs_db",
                "auto_compact_when_codex_closed",
                logs_db.auto_compact_when_codex_closed,
            );
            editor.set_u32(
                "probe.logs_db",
                "compact_interval_hours",
                logs_db.compact_interval_hours,
            );
            editor.set_u64(
                "probe.logs_db",
                "compact_min_freelist_mb",
                logs_db.compact_min_freelist_mb,
            );
            editor.set_u32(
                "probe.logs_db",
                "compact_min_freelist_ratio_percent",
                logs_db.compact_min_freelist_ratio_percent,
            );
            editor.set_u64(
                "probe.logs_db",
                "minimum_free_space_mb",
                logs_db.minimum_free_space_mb,
            );
        }
    }
    Ok(editor.finish())
}

fn remove_legacy_app_server_patch_keys(editor: &mut TomlPatchEditor) {
    for key in [
        "app_server_service",
        "app_server_socket",
        "bridge_enabled",
        "bridge_transport",
        "bridge_timeout_seconds",
    ] {
        editor.remove_key("codex", key);
    }
    editor.remove_key("probe.hooks", "reload_app_server_after_install");
}

struct TomlPatchEditor {
    lines: Vec<String>,
}

impl TomlPatchEditor {
    fn new(text: &str) -> Self {
        Self {
            lines: text.lines().map(ToString::to_string).collect(),
        }
    }

    fn finish(self) -> String {
        let mut text = self.lines.join("\n");
        text.push('\n');
        text
    }

    fn set_string(&mut self, section: &str, key: &str, value: Option<&str>) {
        self.set_value(section, key, value.map(toml_string));
    }

    fn set_bool(&mut self, section: &str, key: &str, value: Option<bool>) {
        self.set_value(section, key, value.map(|value| value.to_string()));
    }

    fn set_u32(&mut self, section: &str, key: &str, value: Option<u32>) {
        self.set_value(section, key, value.map(|value| value.to_string()));
    }

    fn set_u64(&mut self, section: &str, key: &str, value: Option<u64>) {
        self.set_value(section, key, value.map(|value| value.to_string()));
    }

    fn set_usize(&mut self, section: &str, key: &str, value: Option<usize>) {
        self.set_value(section, key, value.map(|value| value.to_string()));
    }

    fn set_value(&mut self, section: &str, key: &str, value: Option<String>) {
        let Some(value) = value else {
            return;
        };
        let (start, end) = self.ensure_section(section);
        for index in start + 1..end {
            let stripped = self.lines[index].trim();
            if stripped.starts_with('#') || !stripped.contains('=') {
                continue;
            }
            let Some((line_key, _)) = stripped.split_once('=') else {
                continue;
            };
            if line_key.trim() == key {
                self.lines[index] = format!("{key} = {value}");
                return;
            }
        }
        self.lines.insert(end, format!("{key} = {value}"));
    }

    fn remove_key(&mut self, section: &str, key: &str) {
        let Some((start, end)) = self.section_range(section) else {
            return;
        };
        if let Some(index) = (start + 1..end).find(|index| {
            let stripped = self.lines[*index].trim();
            if stripped.starts_with('#') || !stripped.contains('=') {
                return false;
            }
            stripped
                .split_once('=')
                .is_some_and(|(line_key, _)| line_key.trim() == key)
        }) {
            self.lines.remove(index);
        }
    }

    fn ensure_section(&mut self, section: &str) -> (usize, usize) {
        if let Some(range) = self.section_range(section) {
            return range;
        }
        if self
            .lines
            .last()
            .is_some_and(|line| !line.trim().is_empty())
        {
            self.lines.push(String::new());
        }
        let start = self.lines.len();
        self.lines.push(format!("[{section}]"));
        (start, self.lines.len())
    }

    fn section_range(&self, section: &str) -> Option<(usize, usize)> {
        let header = format!("[{section}]");
        let start = self.lines.iter().position(|line| line.trim() == header)?;
        let end = self
            .lines
            .iter()
            .enumerate()
            .skip(start + 1)
            .find_map(|(index, line)| {
                let trimmed = line.trim();
                (trimmed.starts_with('[') && trimmed.ends_with(']')).then_some(index)
            })
            .unwrap_or(self.lines.len());
        Some((start, end))
    }
}

fn toml_string(value: &str) -> String {
    let escaped = value
        .chars()
        .flat_map(|ch| match ch {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            _ => vec![ch],
        })
        .collect::<String>();
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::{Config, ProbeConfigFilePatch, ProbeSettingsPatch};
    use crate::platform::PlatformKind;
    use std::{fs, time::SystemTime};

    #[test]
    fn linux_config_uses_loopback_panel_port() {
        let config = Config::for_platform_kind(PlatformKind::Linux);
        assert_eq!(config.server.listen.to_string(), "127.0.0.1:15742");
        assert!(config
            .update
            .panel_precheck_command
            .contains("http://127.0.0.1:15742/healthz"));
        assert_eq!(config.codex.home.to_string_lossy(), "auto");
    }

    #[test]
    fn macos_config_uses_tauri_app_precheck() {
        let config = Config::for_platform_kind_with_home(PlatformKind::Macos, "/Users/example");

        assert_eq!(config.server.listen.to_string(), "127.0.0.1:15742");
        assert_eq!(
            config.update.panel_precheck_command,
            "test -d '/Users/example/Library/Application Support/NexusHub'"
        );
        assert!(!config
            .update
            .panel_precheck_command
            .contains("http://127.0.0.1:15742/healthz"));
    }

    #[test]
    fn default_config_uses_codex_update_transient_wrappers() {
        let config = Config::default();

        assert_eq!(
            config.update.precheck_command,
            "/usr/local/bin/nexushub-codex-precheck"
        );
        assert_eq!(
            config.update.update_command,
            "/usr/local/bin/nexushub-codex-update"
        );
        assert_eq!(
            config.update.prune_command,
            "/usr/local/bin/nexushub-codex-prune"
        );
    }

    #[test]
    fn default_config_has_no_app_server_runtime_dependency() {
        let config = Config::default();

        assert!(!config.codex.bridge_enabled);
        assert!(config.codex.app_server_service.trim().is_empty());
        assert!(config.codex.app_server_socket.is_none());
        assert!(!config.probe.hooks.reload_app_server_after_install);
    }

    #[test]
    fn default_config_includes_builtin_probe_settings() {
        let config = Config::default();

        assert!(config.probe.enabled);
        assert_eq!(config.probe.poll_seconds, 15);
        assert_eq!(config.probe.recent_limit, 50);
        assert!(config.probe.hooks.manage_stop_hook);
        assert!(!config.probe.hooks.reload_app_server_after_install);
        assert!(!config.probe.notifications.enabled);
        assert_eq!(config.probe.notifications.server_url, "https://api.day.app");
        assert!(config.probe.notifications.notify_reply_needed);
        assert!(config.probe.notifications.notify_recoverable);
        assert!(config.probe.logs_db.enabled);
        assert_eq!(config.probe.logs_db.retention_days, 2);
    }

    #[test]
    fn default_config_matches_mac_app_probe_runtime_defaults() {
        let config = Config::default();

        assert_eq!(config.probe.observability.hook_event_max_lines, 500);
        assert_eq!(config.probe.observability.hook_cooldown_max_lines, 1_000);
        assert_eq!(config.probe.observability.log_max_bytes, 5 * 1024 * 1024);
        assert_eq!(config.probe.logs_db.retention_days, 2);
        assert_eq!(config.probe.logs_db.maintenance_interval_hours, 6);
        assert_eq!(config.probe.logs_db.codex_exit_grace_seconds, 5);
        assert_eq!(config.probe.logs_db.codex_exit_max_wait_seconds, 1_800);
        assert_eq!(config.probe.logs_db.delete_chunk_rows, 5_000);
        assert_eq!(config.probe.logs_db.max_delete_rows_per_run, 100_000);
        assert_eq!(config.probe.logs_db.busy_timeout_ms, 500);
        assert_eq!(config.probe.logs_db.compact_interval_hours, 24);
        assert_eq!(config.probe.logs_db.compact_min_freelist_mb, 256);
        assert_eq!(config.probe.logs_db.compact_min_freelist_ratio_percent, 20);
        assert_eq!(config.probe.logs_db.minimum_free_space_mb, 1_024);
    }

    #[test]
    fn probe_notification_server_url_requires_https_except_loopback_http() {
        let mut config = Config::default();
        config.probe.notifications.server_url = "http://example.com".to_string();
        config.normalize();

        assert_eq!(config.probe.notifications.server_url, "https://api.day.app");

        config.probe.notifications.server_url = "http://127.0.0.1:8080".to_string();
        config.normalize();
        assert_eq!(
            config.probe.notifications.server_url,
            "http://127.0.0.1:8080"
        );

        config.probe.notifications.server_url = "https://bark.example.com".to_string();
        config.normalize();
        assert_eq!(
            config.probe.notifications.server_url,
            "https://bark.example.com"
        );
    }

    #[test]
    fn probe_config_patch_preserves_unknown_sections_and_comments() {
        let input = r#"# hand kept
[server]
listen = "127.0.0.1:15742"

[custom]
keep = "yes"

[codex]
home = "/root/.codex"
app_server_service = "codex-app-server-root.service"
app_server_socket = "/root/.codex/app-server.sock"
bridge_enabled = true
bridge_transport = "socket"
bridge_timeout_seconds = 60
host_label = "old-host"

[probe]
enabled = true
poll_seconds = 15

[probe.hooks]
manage_stop_hook = true
reload_app_server_after_install = true

[probe.notifications]
enabled = false
group = "old"
"#;
        let patch = ProbeConfigFilePatch {
            codex: Some(super::CodexProbeConfigPatch {
                home: Some(Some("/srv/codex".into())),
                host_label: Some("43.155.235.227".into()),
                ..Default::default()
            }),
            probe: Some(ProbeSettingsPatch {
                enabled: Some(false),
                poll_seconds: Some(30),
                recent_limit: Some(80),
                notifications: Some(super::ProbeNotificationsConfigPatch {
                    enabled: Some(true),
                    group: Some("NexusHub".into()),
                    notify_completion: Some(true),
                    ..Default::default()
                }),
                logs_db: Some(super::ProbeLogsDbConfigPatch {
                    retention_days: Some(30),
                    minimum_free_space_mb: Some(512),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        };

        let output = super::patch_probe_config_toml(input, &patch).unwrap();

        assert!(output.contains("# hand kept"));
        assert!(output.contains("[custom]\nkeep = \"yes\""));
        assert!(output.contains("home = \"/srv/codex\""));
        assert!(output.contains("host_label = \"43.155.235.227\""));
        assert!(!output.contains("app_server_service"));
        assert!(!output.contains("app_server_socket"));
        assert!(!output.contains("bridge_enabled"));
        assert!(!output.contains("bridge_transport"));
        assert!(!output.contains("bridge_timeout_seconds"));
        assert!(!output.contains("reload_app_server_after_install"));
        assert!(output.contains("[probe]\nenabled = false\npoll_seconds = 30"));
        assert!(output.contains("recent_limit = 80"));
        assert!(output.contains("[probe.notifications]"));
        assert!(output.contains("enabled = true"));
        assert!(output.contains("group = \"NexusHub\""));
        assert!(output.contains("[probe.logs_db]"));
        assert!(output.contains("retention_days = 30"));
        assert!(output.contains("minimum_free_space_mb = 512"));
    }

    #[test]
    fn config_accepts_missing_codex_home_as_auto() {
        let input = r#"
[server]
listen = "127.0.0.1:15742"
trust_forwarded_headers = true

[codex]
workspace = "/home/ubuntu/codex-workspace"
host_label = "43.155.235.227"

[security]
secret_key = "test-secret"
cookie_secure = true
session_ttl_seconds = 31536000
login_rate_limit_per_minute = 8

[paths]
data_dir = "/opt/nexushub"
db_path = "/opt/nexushub/nexushub.sqlite"
webui_dir = "/opt/nexushub/webui"
log_dir = "/opt/nexushub/logs"

[update]
precheck_command = "/usr/local/bin/nexushub-codex-precheck"
update_command = "/usr/local/bin/nexushub-codex-update"
prune_command = "/usr/local/bin/nexushub-codex-prune"
doctor_command = "/home/ubuntu/codex-admin/bin/codex-cloud-doctor"
panel_update_command = "/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest"
panel_precheck_command = "test -x /usr/local/bin/nexushub-update && systemctl is-active nexushub && curl -fsS http://127.0.0.1:15742/healthz"
"#;
        let config: Config = toml::from_str(input).unwrap();

        assert_eq!(config.codex.home.to_string_lossy(), "auto");
        assert!(config.codex.app_server_service.is_empty());
    }

    #[test]
    fn probe_config_patch_removes_codex_home_for_auto_discovery() {
        let input = r#"
[codex]
home = "/root/.codex"
workspace = "/home/ubuntu/codex-workspace"
"#;
        let patch: ProbeConfigFilePatch =
            serde_json::from_str(r#"{"codex":{"home":null}}"#).unwrap();
        let output = super::patch_probe_config_toml(input, &patch).unwrap();

        assert!(!output.contains("home = "));
        assert!(output.contains("workspace = \"/home/ubuntu/codex-workspace\""));
    }

    #[test]
    fn probe_config_patch_cannot_reintroduce_legacy_app_server_bridge_keys() {
        let input = r#"
[codex]
home = "/root/.codex"
workspace = "/home/ubuntu/codex-workspace"
host_label = "old"
"#;
        let patch: ProbeConfigFilePatch = serde_json::from_str(
            r#"{
                "codex": {
                    "home": "/srv/codex",
                    "workspace": "/srv/workspace",
                    "host_label": "cloud",
                    "app_server_service": "codex-app-server-root.service",
                    "app_server_socket": "/root/.codex/app-server.sock",
                    "bridge_enabled": true,
                    "bridge_transport": "socket",
                    "bridge_timeout_seconds": 99
                }
            }"#,
        )
        .unwrap();

        let output = super::patch_probe_config_toml(input, &patch).unwrap();

        assert!(output.contains("home = \"/srv/codex\""));
        assert!(output.contains("workspace = \"/srv/workspace\""));
        assert!(output.contains("host_label = \"cloud\""));
        assert!(!output.contains("app_server_service"));
        assert!(!output.contains("app_server_socket"));
        assert!(!output.contains("bridge_enabled"));
        assert!(!output.contains("bridge_transport"));
        assert!(!output.contains("bridge_timeout_seconds"));
    }

    #[test]
    fn load_reads_secret_key_from_sibling_env_file() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("nexushub-config-{unique}"));
        fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.toml");
        let env_path = dir.join("env");
        let mut config = Config::default();
        config.security.secret_key.clear();
        fs::write(&config_path, toml::to_string_pretty(&config).unwrap()).unwrap();
        fs::write(
            &env_path,
            "NEXUSHUB_SECRET_KEY=7q9DCmCPyxnTrH3FhrV1sUJol1yqPgscQsBnR-mXA2E\n",
        )
        .unwrap();

        let loaded = Config::load(&config_path).unwrap();

        assert_eq!(
            loaded.security.secret_key,
            "7q9DCmCPyxnTrH3FhrV1sUJol1yqPgscQsBnR-mXA2E"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn default_config_uses_365_day_sessions_and_neutral_host() {
        let config = Config::default();

        assert_eq!(config.security.session_ttl_seconds, 31_536_000);
        assert_eq!(config.codex.host_label, "43.155.235.227");
        assert_eq!(
            config.security.turnstile_expected_hostname.as_deref(),
            Some("661313.xyz")
        );
        assert_eq!(
            config.security.turnstile_expected_action.as_deref(),
            Some("login")
        );
        assert!(!toml::to_string(&config)
            .unwrap()
            .contains(&super::legacy_host_label()));
    }

    #[test]
    fn normalize_migrates_legacy_session_ttl_and_host_label() {
        let mut config = Config::default();
        config.security.session_ttl_seconds = 604_800;
        config.codex.host_label = super::legacy_host_label();
        config.security.turnstile_expected_action = None;
        config.server.listen = "127.0.0.1:15732".parse().unwrap();
        config.update.panel_precheck_command =
            "test -x /usr/local/bin/nexushub-update && systemctl is-active nexushub && curl -fsS http://127.0.0.1:15732/healthz".to_string();

        config.normalize();

        assert_eq!(config.security.session_ttl_seconds, 31_536_000);
        assert_eq!(config.codex.host_label, "43.155.235.227");
        assert_eq!(config.server.listen.to_string(), "127.0.0.1:15742");
        match super::current_platform_kind() {
            PlatformKind::Linux | PlatformKind::Windows => assert!(config
                .update
                .panel_precheck_command
                .contains("http://127.0.0.1:15742/healthz")),
            PlatformKind::Macos => {
                assert!(config.update.panel_precheck_command.starts_with("test -d "));
                assert!(!config
                    .update
                    .panel_precheck_command
                    .contains("http://127.0.0.1:15742/healthz"));
            }
        }
        assert_eq!(
            config.security.turnstile_expected_action.as_deref(),
            Some("login")
        );
    }

    #[test]
    fn normalize_migrates_legacy_codex_precheck_to_transient_wrapper() {
        for legacy in [
            super::LEGACY_PRECHECK_COMMAND_SIMPLE,
            super::LEGACY_PRECHECK_COMMAND_WITH_AUDIT,
        ] {
            let mut config = Config::default();
            config.update.precheck_command = legacy.to_string();

            config.normalize();

            assert_eq!(
                config.update.precheck_command,
                "/usr/local/bin/nexushub-codex-precheck"
            );
        }
    }

    #[test]
    fn normalize_keeps_custom_codex_precheck_command() {
        let mut config = Config::default();
        config.update.precheck_command = "/opt/custom/precheck --strict".to_string();

        config.normalize();

        assert_eq!(
            config.update.precheck_command,
            "/opt/custom/precheck --strict"
        );
    }
}
