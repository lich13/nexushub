use crate::overview::DesktopState;
use anyhow::{Context, Result};
use nexushub_core::{
    config::Config,
    services::desktop_webui::{
        self as core_desktop_webui, DesktopWebuiPasswordReset, DesktopWebuiSettingsPatch,
        DesktopWebuiSettingsView, DesktopWebuiStatus,
    },
};
use std::{
    fs::{self, File},
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

const PID_FILE: &str = "desktop-webui.pid";
const LOG_FILE: &str = "desktop-webui.log";

pub(crate) fn settings(state: &DesktopState) -> Result<DesktopWebuiSettingsView> {
    let config = state.config();
    Ok(core_desktop_webui::settings_view(
        &config,
        password_configured(state, &config)?,
    ))
}

pub(crate) fn save_settings(
    state: &DesktopState,
    patch: DesktopWebuiSettingsPatch,
) -> Result<DesktopWebuiSettingsView> {
    let mut config = state.config();
    core_desktop_webui::apply_settings_patch(&mut config, patch)?;
    write_config(state, &config)?;
    state.replace_config(config);
    settings(state)
}

pub(crate) fn reset_password(
    state: &DesktopState,
    request: DesktopWebuiPasswordReset,
) -> Result<DesktopWebuiSettingsView> {
    let realm_username = core_desktop_webui::validate_password_reset(&request)?;
    let hash = nexushub_core::security::hash_password(&request.password)?;
    state
        .db
        .upsert_admin(&desktop_admin_id(&realm_username), &realm_username, &hash)?;
    let mut config = state.config();
    config.desktop_webui.username = request.username.trim().to_string();
    config.desktop_webui.normalize();
    write_config(state, &config)?;
    state.replace_config(config);
    settings(state)
}

pub(crate) fn status(state: &DesktopState) -> Result<DesktopWebuiStatus> {
    let config = state.config();
    let pid = read_pid(state).ok();
    let running = health_check(config.desktop_webui.listen);
    Ok(core_desktop_webui::status(
        &config,
        password_configured(state, &config)?,
        running,
        pid,
        status_message(pid, running),
    ))
}

pub(crate) fn start(state: &DesktopState) -> Result<DesktopWebuiStatus> {
    let config = state.config();
    if health_check(config.desktop_webui.listen) {
        return status(state);
    }
    let plan = core_desktop_webui::start_plan(&config, password_configured(state, &config)?)?;
    ensure_port_available(plan.listen)?;
    let helper = state.platform().daemon_binary();
    if !helper.is_file() {
        anyhow::bail!("nexushub-webd helper not found: {}", helper.display());
    }
    fs::create_dir_all(&state.platform().log_dir)?;
    let log = File::create(log_file(state)?)?;
    let log_err = log.try_clone()?;
    let child = Command::new(&helper)
        .arg("--config")
        .arg(&state.platform().config_file)
        .arg("serve")
        .arg("--surface")
        .arg("desktop-lan-webui")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .spawn()
        .with_context(|| format!("start {}", helper.display()))?;
    write_pid(state, child.id())?;
    if wait_for_health(plan.listen) {
        return status(state);
    }
    let _ = stop_pid(child.id());
    let _ = fs::remove_file(pid_file(state));
    anyhow::bail!("desktop WebUI did not become healthy at {}", plan.url)
}

pub(crate) fn stop(state: &DesktopState) -> Result<DesktopWebuiStatus> {
    if let Ok(pid) = read_pid(state) {
        stop_pid(pid)?;
        let _ = fs::remove_file(pid_file(state));
    }
    status(state)
}

fn password_configured(state: &DesktopState, config: &Config) -> Result<bool> {
    Ok(state
        .db
        .admin_by_username(&core_desktop_webui::realm_username(
            &config.desktop_webui.username,
        ))?
        .is_some())
}

fn write_config(state: &DesktopState, config: &Config) -> Result<()> {
    let path = &state.platform().config_file;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, toml::to_string_pretty(config)?)?;
    Ok(())
}

fn desktop_admin_id(realm_username: &str) -> String {
    format!(
        "desktop-webui-{}",
        nexushub_core::security::hash_token(realm_username)
    )
}

fn ensure_port_available(listen: SocketAddr) -> Result<()> {
    TcpListener::bind(listen)
        .map(|_| ())
        .map_err(|err| anyhow::anyhow!("desktop WebUI listen address is unavailable: {err}"))
}

fn wait_for_health(listen: SocketAddr) -> bool {
    for _ in 0..30 {
        if health_check(listen) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

fn health_check(listen: SocketAddr) -> bool {
    let addr = local_health_addr(listen);
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(250)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    if stream
        .write_all(b"GET /healthz HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut buf = [0_u8; 128];
    stream
        .read(&mut buf)
        .map(|len| std::str::from_utf8(&buf[..len]).is_ok_and(|text| text.contains("200 OK")))
        .unwrap_or(false)
}

fn local_health_addr(listen: SocketAddr) -> SocketAddr {
    if listen.ip().is_unspecified() {
        SocketAddr::new("127.0.0.1".parse().expect("loopback"), listen.port())
    } else {
        listen
    }
}

fn pid_file(state: &DesktopState) -> PathBuf {
    state.platform().data_dir.join(PID_FILE)
}

fn log_file(state: &DesktopState) -> Result<PathBuf> {
    Ok(state.platform().log_dir.join(LOG_FILE))
}

fn read_pid(state: &DesktopState) -> Result<u32> {
    let text = fs::read_to_string(pid_file(state))?;
    text.trim()
        .parse::<u32>()
        .map_err(|err| anyhow::anyhow!("invalid desktop WebUI pid file: {err}"))
}

fn write_pid(state: &DesktopState, pid: u32) -> Result<()> {
    fs::create_dir_all(&state.platform().data_dir)?;
    fs::write(pid_file(state), pid.to_string())?;
    Ok(())
}

fn status_message(pid: Option<u32>, running: bool) -> Option<String> {
    match (pid, running) {
        (Some(_), true) => Some("running".to_string()),
        (Some(_), false) => Some("pid file exists but health check failed".to_string()),
        (None, false) => Some("stopped".to_string()),
        (None, true) => Some("running without managed pid file".to_string()),
    }
}

#[cfg(unix)]
fn stop_pid(pid: u32) -> Result<()> {
    Command::new("kill")
        .arg(pid.to_string())
        .status()
        .with_context(|| format!("stop desktop WebUI pid {pid}"))?;
    Ok(())
}

#[cfg(windows)]
fn stop_pid(pid: u32) -> Result<()> {
    Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status()
        .with_context(|| format!("stop desktop WebUI pid {pid}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexushub_core::{
        crypto::SecretBox,
        db::PanelDb,
        platform::{PlatformKind, PlatformPaths},
        services::desktop_webui::{DesktopWebuiPasswordReset, DesktopWebuiSettingsPatch},
    };

    fn test_state() -> (tempfile::TempDir, DesktopState) {
        let temp = tempfile::tempdir().unwrap();
        let mut config = Config::for_platform_kind_with_home(PlatformKind::Macos, temp.path());
        config.paths.data_dir = temp.path().join("data");
        config.paths.db_path = temp.path().join("data").join("nexushub.sqlite");
        config.paths.log_dir = temp.path().join("logs");
        config.codex.home = temp.path().join("codex-home");
        config.codex.workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&config.paths.data_dir).unwrap();
        std::fs::create_dir_all(&config.paths.log_dir).unwrap();
        std::fs::create_dir_all(&config.codex.home).unwrap();
        std::fs::create_dir_all(&config.codex.workspace).unwrap();
        let db =
            PanelDb::open_with_secret_box(&config.paths.db_path, SecretBox::deterministic_dev())
                .unwrap();
        let platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, temp.path());
        (temp, DesktopState::new(config, db, platform))
    }

    #[test]
    fn password_reset_uses_desktop_webui_realm_and_never_writes_plaintext_password() {
        let (_temp, state) = test_state();
        let password = "desktop-webui-secret";

        let view = reset_password(
            &state,
            DesktopWebuiPasswordReset {
                username: "lan-admin".to_string(),
                password: password.to_string(),
            },
        )
        .unwrap();

        assert!(view.password_configured);
        assert_eq!(view.username, "lan-admin");
        let realm_username = core_desktop_webui::realm_username("lan-admin");
        let admin = state
            .db
            .admin_by_username(&realm_username)
            .unwrap()
            .unwrap();
        assert_eq!(admin.username, realm_username);
        assert!(nexushub_core::security::verify_password(
            password,
            &admin.password_hash
        ));
        let persisted_config = std::fs::read_to_string(&state.platform().config_file).unwrap();
        assert!(persisted_config.contains("[desktop_webui]"));
        assert!(persisted_config.contains(r#"username = "lan-admin""#));
        assert!(!persisted_config.contains(password));
        assert!(!persisted_config.contains("desktop-webui-secret"));
    }

    #[test]
    fn save_settings_normalizes_turnstile_off_and_keeps_password_state() {
        let (_temp, state) = test_state();
        reset_password(
            &state,
            DesktopWebuiPasswordReset {
                username: "admin".to_string(),
                password: "desktop-webui-secret".to_string(),
            },
        )
        .unwrap();

        let view = save_settings(
            &state,
            DesktopWebuiSettingsPatch {
                enabled: true,
                listen: "127.0.0.1:15753".to_string(),
                username: " admin ".to_string(),
                session_ttl_seconds: 3600,
                cookie_secure: true,
                public_base_url: Some("http://127.0.0.1:15753/nexushub/".to_string()),
            },
        )
        .unwrap();

        assert!(view.enabled);
        assert!(view.password_configured);
        assert_eq!(view.listen, "127.0.0.1:15753");
        assert_eq!(view.username, "admin");
        assert!(!view.turnstile_enabled);
        let start = core_desktop_webui::start_plan(&state.config(), true).unwrap();
        assert_eq!(start.url, "http://127.0.0.1:15753/nexushub/");
    }

    #[test]
    fn service_manager_uses_fixed_helper_args_and_no_shell_wrapper() {
        let source = include_str!("desktop_webui.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("desktop_webui service source must include production section");

        for required in [
            "let helper = state.platform().daemon_binary();",
            "Command::new(&helper)",
            ".arg(\"--config\")",
            ".arg(&state.platform().config_file)",
            ".arg(\"serve\")",
            ".arg(\"--surface\")",
            ".arg(\"desktop-lan-webui\")",
        ] {
            assert!(
                source.contains(required),
                "desktop WebUI service must start the bundled helper with fixed args: {required}"
            );
        }
        for forbidden in [
            "Command::new(\"sh\")",
            "Command::new(\"bash\")",
            "Command::new(\"zsh\")",
            "Command::new(\"cmd\")",
            ".arg(\"-c\")",
            "shell",
        ] {
            assert!(
                !source.contains(forbidden),
                "desktop WebUI service must not use a shell wrapper: {forbidden}"
            );
        }
    }
}
