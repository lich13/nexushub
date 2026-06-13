use crate::{crypto::SecretBox, platform::PlatformPaths};
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
const LEGACY_PRECHECK_COMMAND_SIMPLE: &str = "codex --version && sudo -n codex --version && /usr/local/bin/codex-raw --version && sqlite3 /root/.codex/state_5.sqlite 'pragma integrity_check;' && /home/ubuntu/codex-admin/bin/codex-cloud-doctor";
const LEGACY_PRECHECK_COMMAND_WITH_AUDIT: &str = "codex --version && sudo -n codex --version && /usr/local/bin/codex-raw --version && readlink -f /usr/local/bin/codex && readlink -f /usr/local/bin/codex-raw && sqlite3 /root/.codex/state_5.sqlite 'pragma integrity_check;' && sqlite3 /root/.codex/state_5.sqlite \"select count(*) total, sum(archived_at is null) active, sum(archived_at is not null) archived from threads;\" && /home/ubuntu/codex-admin/bin/codex-cloud-doctor";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub codex: CodexConfig,
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
    pub home: PathBuf,
    pub workspace: PathBuf,
    pub app_server_service: String,
    #[serde(default = "default_app_server_socket")]
    pub app_server_socket: Option<PathBuf>,
    #[serde(default = "default_bridge_enabled")]
    pub bridge_enabled: bool,
    #[serde(default = "default_bridge_transport")]
    pub bridge_transport: BridgeTransport,
    #[serde(default = "default_bridge_timeout_seconds")]
    pub bridge_timeout_seconds: u64,
    pub host_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BridgeTransport {
    Websocket,
    JsonLine,
    Lsp,
}

fn default_app_server_socket() -> Option<PathBuf> {
    Some(PathBuf::from(
        "/root/.codex/app-server-control/app-server-control.sock",
    ))
}

fn default_bridge_enabled() -> bool {
    true
}

fn default_bridge_transport() -> BridgeTransport {
    BridgeTransport::Websocket
}

fn default_bridge_timeout_seconds() -> u64 {
    20
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
    "test -x /usr/local/bin/nexushub-update && systemctl is-active nexushub && curl -fsS http://127.0.0.1:15732/healthz".to_string()
}

impl Default for Config {
    fn default() -> Self {
        let listen = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 15732);
        let platform = PlatformPaths::for_kind(crate::platform::PlatformKind::Linux);
        Self {
            server: ServerConfig {
                listen,
                public_base_url: None,
                trust_forwarded_headers: true,
            },
            codex: CodexConfig {
                home: PathBuf::from("/root/.codex"),
                workspace: PathBuf::from("/home/ubuntu/codex-workspace"),
                app_server_service: "codex-app-server-root.service".to_string(),
                app_server_socket: default_app_server_socket(),
                bridge_enabled: default_bridge_enabled(),
                bridge_transport: default_bridge_transport(),
                bridge_timeout_seconds: default_bridge_timeout_seconds(),
                host_label: DEFAULT_HOST_LABEL.to_string(),
            },
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
                doctor_command: "/home/ubuntu/codex-admin/bin/codex-cloud-doctor".to_string(),
                panel_update_command: default_panel_update_command(),
                panel_precheck_command: default_panel_precheck_command(),
            },
        }
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
        if self.codex.bridge_timeout_seconds == 0 {
            self.codex.bridge_timeout_seconds = 20;
        }
        if self.codex.host_label.trim().is_empty() || self.codex.host_label == legacy_host_label() {
            self.codex.host_label = DEFAULT_HOST_LABEL.to_string();
        }
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

#[cfg(test)]
mod tests {
    use super::Config;
    use std::{fs, time::SystemTime};

    #[test]
    fn default_config_uses_loopback_panel_port() {
        let config = Config::default();
        assert_eq!(config.server.listen.to_string(), "127.0.0.1:15732");
        assert_eq!(config.codex.home.to_string_lossy(), "/root/.codex");
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

        config.normalize();

        assert_eq!(config.security.session_ttl_seconds, 31_536_000);
        assert_eq!(config.codex.host_label, "43.155.235.227");
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
