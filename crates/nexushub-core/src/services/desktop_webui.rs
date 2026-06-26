use crate::config::{Config, DesktopWebuiConfig};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};

pub const DESKTOP_WEBUI_ADMIN_PREFIX: &str = "desktop-webui:";
pub const MIN_DESKTOP_WEBUI_PASSWORD_LEN: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DesktopWebuiSettingsView {
    pub enabled: bool,
    pub listen: String,
    pub username: String,
    pub session_ttl_seconds: u64,
    pub cookie_secure: bool,
    pub public_base_url: Option<String>,
    pub turnstile_enabled: bool,
    pub password_configured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DesktopWebuiSettingsPatch {
    pub enabled: bool,
    pub listen: String,
    pub username: String,
    pub session_ttl_seconds: u64,
    pub cookie_secure: bool,
    pub public_base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DesktopWebuiPasswordReset {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DesktopWebuiStatus {
    pub configured: bool,
    pub enabled: bool,
    pub running: bool,
    pub pid: Option<u32>,
    pub listen: String,
    pub url: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopWebuiStartPlan {
    pub listen: SocketAddr,
    pub url: String,
}

pub fn realm_username(username: &str) -> String {
    format!("{DESKTOP_WEBUI_ADMIN_PREFIX}{}", username.trim())
}

pub fn public_username(username: &str) -> &str {
    username
        .strip_prefix(DESKTOP_WEBUI_ADMIN_PREFIX)
        .unwrap_or(username)
}

pub fn is_desktop_webui_admin(username: &str) -> bool {
    username.starts_with(DESKTOP_WEBUI_ADMIN_PREFIX)
}

pub fn settings_view(config: &Config, password_configured: bool) -> DesktopWebuiSettingsView {
    DesktopWebuiSettingsView {
        enabled: config.desktop_webui.enabled,
        listen: config.desktop_webui.listen.to_string(),
        username: config.desktop_webui.username.clone(),
        session_ttl_seconds: config.desktop_webui.session_ttl_seconds,
        cookie_secure: config.desktop_webui.cookie_secure,
        public_base_url: config.desktop_webui.public_base_url.clone(),
        turnstile_enabled: config.desktop_webui.turnstile_enabled,
        password_configured,
    }
}

pub fn apply_settings_patch(config: &mut Config, patch: DesktopWebuiSettingsPatch) -> Result<()> {
    let mut next = DesktopWebuiConfig {
        enabled: patch.enabled,
        listen: parse_listen(&patch.listen)?,
        username: patch.username,
        session_ttl_seconds: patch.session_ttl_seconds,
        cookie_secure: patch.cookie_secure,
        public_base_url: patch.public_base_url,
        turnstile_enabled: false,
    };
    next.normalize();
    validate_listen(next.listen)?;
    config.desktop_webui = next;
    Ok(())
}

pub fn validate_password_reset(request: &DesktopWebuiPasswordReset) -> Result<String> {
    let username = request.username.trim();
    if username.is_empty() {
        bail!("username is required");
    }
    if request.password.len() < MIN_DESKTOP_WEBUI_PASSWORD_LEN {
        bail!("password must be at least {MIN_DESKTOP_WEBUI_PASSWORD_LEN} characters");
    }
    Ok(realm_username(username))
}

pub fn start_plan(config: &Config, password_configured: bool) -> Result<DesktopWebuiStartPlan> {
    if !config.desktop_webui.enabled {
        bail!("desktop WebUI is disabled");
    }
    if !password_configured {
        bail!("desktop WebUI password is not configured");
    }
    validate_listen(config.desktop_webui.listen)?;
    Ok(DesktopWebuiStartPlan {
        listen: config.desktop_webui.listen,
        url: local_url(config.desktop_webui.listen),
    })
}

pub fn status(
    config: &Config,
    password_configured: bool,
    running: bool,
    pid: Option<u32>,
    message: Option<String>,
) -> DesktopWebuiStatus {
    DesktopWebuiStatus {
        configured: password_configured,
        enabled: config.desktop_webui.enabled,
        running,
        pid,
        listen: config.desktop_webui.listen.to_string(),
        url: local_url(config.desktop_webui.listen),
        message,
    }
}

fn parse_listen(value: &str) -> Result<SocketAddr> {
    value
        .trim()
        .parse::<SocketAddr>()
        .map_err(|err| anyhow::anyhow!("invalid listen address: {err}"))
}

fn validate_listen(listen: SocketAddr) -> Result<()> {
    if listen.port() == 0 {
        bail!("listen port must not be 0");
    }
    Ok(())
}

fn local_url(listen: SocketAddr) -> String {
    let host = match listen.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => "127.0.0.1".to_string(),
        IpAddr::V6(ip) if ip.is_unspecified() => "[::1]".to_string(),
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) => format!("[{ip}]"),
    };
    format!("http://{host}:{}/nexushub/", listen.port())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, platform::PlatformKind};

    #[test]
    fn default_settings_are_disabled_and_use_independent_realm() {
        let config = Config::for_platform_kind(PlatformKind::Macos);
        let view = settings_view(&config, false);

        assert!(!view.enabled);
        assert_eq!(view.listen, "0.0.0.0:15743");
        assert_eq!(view.username, "admin");
        assert!(!view.turnstile_enabled);
        assert_eq!(realm_username("admin"), "desktop-webui:admin");
        assert_eq!(public_username("desktop-webui:admin"), "admin");
    }

    #[test]
    fn start_plan_requires_enabled_password_and_fixed_port() {
        let mut config = Config::for_platform_kind(PlatformKind::Macos);
        assert!(start_plan(&config, true)
            .unwrap_err()
            .to_string()
            .contains("disabled"));

        config.desktop_webui.enabled = true;
        assert!(start_plan(&config, false)
            .unwrap_err()
            .to_string()
            .contains("password"));

        config.desktop_webui.listen = "127.0.0.1:0".parse().unwrap();
        assert!(start_plan(&config, true)
            .unwrap_err()
            .to_string()
            .contains("port"));
    }
}
