use anyhow::{anyhow, Result};
use nexushub_core::{
    codex::{resolve_codex_paths, CodexGoalClient, CodexPaths},
    config::Config,
    crypto::SecretBox,
    db::PanelDb,
    jobs::JobRunner,
    local::{
        default_codex_models, default_permission_profiles, local_codex_config,
        local_plugin_catalog, CodexModelInfo, CodexPermissionProfile, LocalCodexConfig,
        LocalPluginInfo,
    },
    platform::{PlatformKind, PlatformPaths},
    services::system::HostSurface,
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusPaths {
    pub app_support_dir: PathBuf,
    pub config_file: PathBuf,
    pub database_file: PathBuf,
    pub log_dir: PathBuf,
    pub app_log_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopOverview {
    pub product_name: String,
    pub version: String,
    pub identifier: String,
    pub os: String,
    pub arch: String,
    pub paths: NexusPaths,
    pub app_support_dir_ready: bool,
    pub log_dir_ready: bool,
    pub config_file_exists: bool,
    pub database_file_exists: bool,
    pub codex_home: PathBuf,
    pub codex_home_source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopHome {
    pub overview: DesktopOverview,
    pub plugins: Vec<LocalPluginInfo>,
    pub models: Vec<CodexModelInfo>,
    pub permission_profiles: Vec<CodexPermissionProfile>,
    pub codex_config: LocalCodexConfig,
    pub warnings: Vec<String>,
}

#[derive(Clone)]
pub struct DesktopState {
    config: Arc<RwLock<Config>>,
    pub db: PanelDb,
    pub jobs: JobRunner,
    pub(crate) goal_client: CodexGoalClient,
    platform: PlatformPaths,
    host_surface: HostSurface,
}

impl DesktopState {
    pub fn current() -> Result<Self> {
        let platform = PlatformPaths::desktop_current();
        let mut config = load_desktop_config(&platform);
        std::fs::create_dir_all(&config.paths.data_dir)?;
        std::fs::create_dir_all(&config.paths.log_dir)?;
        if nexushub_core::codex::is_macos_network_volume_path(&config.codex.workspace) {
            config.codex.workspace = default_local_desktop_workspace()
                .ok_or_else(|| anyhow!("cannot resolve local desktop workspace"))?;
        }
        std::fs::create_dir_all(&config.codex.workspace)?;
        let db = open_panel_db(&config)?;
        Ok(Self::new(config, db, platform))
    }

    pub fn new(config: Config, db: PanelDb, platform: PlatformPaths) -> Self {
        Self::new_inner(config, db, platform, CodexGoalClient::new())
    }

    #[cfg(test)]
    pub fn new_with_goal_client(
        config: Config,
        db: PanelDb,
        platform: PlatformPaths,
        goal_client: CodexGoalClient,
    ) -> Self {
        Self::new_inner(config, db, platform, goal_client)
    }

    fn new_inner(
        config: Config,
        db: PanelDb,
        platform: PlatformPaths,
        goal_client: CodexGoalClient,
    ) -> Self {
        let jobs = JobRunner::new(db.clone());
        Self {
            config: Arc::new(RwLock::new(config)),
            db,
            jobs,
            goal_client,
            platform,
            host_surface: HostSurface::DesktopEmbeddedTauri,
        }
    }

    pub fn config(&self) -> Config {
        self.config.read().expect("desktop config rwlock").clone()
    }

    pub fn replace_config(&self, config: Config) {
        *self.config.write().expect("desktop config rwlock") = config;
    }

    pub fn resolved_codex_paths(&self) -> nexushub_core::codex::ResolvedCodexPaths {
        let config = self.config();
        resolve_codex_paths(&config.codex.home)
    }

    pub fn codex_paths(&self) -> CodexPaths {
        self.resolved_codex_paths().codex_paths()
    }

    pub fn platform(&self) -> &PlatformPaths {
        &self.platform
    }

    pub fn host_surface(&self) -> HostSurface {
        self.host_surface
    }
}

fn default_local_desktop_workspace() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join("nexushub-workspace"))
}

pub fn nexus_paths_for_home(home: impl Into<PathBuf>) -> NexusPaths {
    nexus_paths_for_platform_home(PlatformKind::Macos, home)
}

pub fn nexus_paths_for_platform_home(kind: PlatformKind, home: impl Into<PathBuf>) -> NexusPaths {
    let platform = PlatformPaths::for_desktop_kind_with_home(kind, home);
    nexus_paths_for_platform(&platform)
}

fn nexus_paths_for_platform(platform: &PlatformPaths) -> NexusPaths {
    NexusPaths {
        config_file: platform.config_file.clone(),
        database_file: platform.data_dir.join("nexushub.sqlite"),
        app_log_file: platform.log_dir.join("nexushub.log"),
        app_support_dir: platform.data_dir.clone(),
        log_dir: platform.log_dir.clone(),
    }
}

pub fn build_desktop_overview() -> Result<DesktopOverview> {
    let platform = PlatformPaths::desktop_current();
    let paths = nexus_paths_for_platform(&platform);
    let config = load_desktop_config(&platform);
    build_desktop_overview_for_config(paths, &config)
}

fn build_desktop_overview_for_config(
    paths: NexusPaths,
    config: &Config,
) -> Result<DesktopOverview> {
    let resolved = resolve_codex_paths(&config.codex.home);

    Ok(DesktopOverview {
        product_name: "NexusHub".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        identifier: "com.lich13.nexushub".to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        app_support_dir_ready: paths.app_support_dir.is_dir(),
        log_dir_ready: paths.log_dir.is_dir(),
        config_file_exists: paths.config_file.is_file(),
        database_file_exists: paths.database_file.is_file(),
        codex_home: resolved.home,
        codex_home_source: resolved.codex_home_source,
        paths,
    })
}

pub async fn build_desktop_home() -> Result<DesktopHome> {
    let state = DesktopState::current()?;
    build_desktop_home_with_state(&state).await
}

pub async fn build_desktop_home_with_state(state: &DesktopState) -> Result<DesktopHome> {
    let config = state.config();
    let overview =
        build_desktop_overview_for_config(nexus_paths_for_platform(state.platform()), &config)?;
    let mut warnings = overview_warning(&overview);
    let resolved = resolve_codex_paths(&config.codex.home);
    warnings.extend(resolved.discovery_warnings.clone());

    Ok(DesktopHome {
        overview,
        plugins: local_plugin_catalog(),
        models: default_codex_models(),
        permission_profiles: default_permission_profiles(),
        codex_config: local_codex_config(&config, None),
        warnings,
    })
}

fn load_desktop_config(platform: &PlatformPaths) -> Config {
    if platform.config_file.is_file() {
        Config::load(&platform.config_file)
            .unwrap_or_else(|_| Config::for_desktop_platform_kind(platform.kind))
    } else {
        Config::for_desktop_platform_kind(platform.kind)
    }
}

pub(crate) fn open_panel_db(config: &Config) -> Result<PanelDb> {
    let secret_box = config
        .secret_box()
        .unwrap_or_else(|_| SecretBox::deterministic_dev());
    PanelDb::open_with_secret_box(&config.paths.db_path, secret_box)
}

fn overview_warning(overview: &DesktopOverview) -> Vec<String> {
    let mut warnings = Vec::new();
    if !overview.app_support_dir_ready {
        warnings.push("配置目录尚未创建".to_string());
    }
    if !overview.log_dir_ready {
        warnings.push("日志目录尚未创建".to_string());
    }
    if !overview.config_file_exists {
        warnings.push("未找到 config.toml，将使用内置默认配置".to_string());
    }
    warnings
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexushub_core::services::{jobs as job_service, settings as settings_service};

    fn test_desktop_state() -> (tempfile::TempDir, DesktopState) {
        let temp = tempfile::tempdir().unwrap();
        let mut config = Config::for_platform_kind_with_home(PlatformKind::Macos, temp.path());
        config.paths.data_dir = temp.path().join("data");
        config.paths.db_path = temp.path().join("panel.sqlite");
        config.paths.log_dir = temp.path().join("logs");
        config.codex.home = temp.path().join("codex-home");
        config.codex.workspace = temp.path().join("workspace");
        std::fs::create_dir_all(&config.paths.data_dir).unwrap();
        std::fs::create_dir_all(&config.paths.log_dir).unwrap();
        std::fs::create_dir_all(&config.codex.home).unwrap();
        std::fs::create_dir_all(config.codex.home.join("sessions")).unwrap();
        std::fs::create_dir_all(&config.codex.workspace).unwrap();
        let db =
            PanelDb::open_with_secret_box(&config.paths.db_path, SecretBox::deterministic_dev())
                .unwrap();
        let state = DesktopState::new(
            config,
            db,
            PlatformPaths::for_kind_with_home(PlatformKind::Macos, temp.path()),
        );
        (temp, state)
    }

    #[test]
    fn mac_style_paths_use_nexushub_application_support_and_logs() {
        let paths = nexus_paths_for_home("/Users/example");

        assert_eq!(
            paths.app_support_dir,
            PathBuf::from("/Users/example/Library/Application Support/NexusHub")
        );
        assert_eq!(
            paths.log_dir,
            PathBuf::from("/Users/example/Library/Logs/NexusHub")
        );
        assert_eq!(
            paths.config_file,
            PathBuf::from("/Users/example/Library/Application Support/NexusHub/config.toml")
        );
        assert_eq!(
            paths.app_log_file,
            PathBuf::from("/Users/example/Library/Logs/NexusHub/nexushub.log")
        );
    }

    #[test]
    fn desktop_overview_exposes_native_paths_without_external_entry_fields() {
        let overview = build_desktop_overview().unwrap();

        assert_eq!(overview.product_name, "NexusHub");
        #[cfg(target_os = "macos")]
        {
            assert!(overview
                .paths
                .app_support_dir
                .ends_with("Library/Application Support/NexusHub"));
            assert!(overview.paths.log_dir.ends_with("Library/Logs/NexusHub"));
        }
        #[cfg(target_os = "linux")]
        {
            assert!(overview
                .paths
                .app_support_dir
                .ends_with(".local/share/NexusHub"));
            assert!(overview
                .paths
                .log_dir
                .ends_with(".local/state/NexusHub/logs"));
            assert!(!overview.paths.app_support_dir.starts_with("/opt/nexushub"));
        }
    }

    #[test]
    fn desktop_probe_save_settings_uses_shared_bark_device_key_constant() {
        let (_temp, state) = test_desktop_state();
        std::fs::create_dir_all(state.platform().config_file.parent().unwrap()).unwrap();
        let config = state.config();
        std::fs::write(
            &state.platform().config_file,
            toml::to_string(&config).unwrap(),
        )
        .unwrap();

        crate::services::settings::test_probe_save_settings_with_state(
            &state,
            settings_service::ProbeSettingsSaveRequest {
                notifications: Some(settings_service::ProbeNotificationsSavePatch {
                    device_key: Some("  shared-device-key  ".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(
            state
                .db
                .get_secret_setting_bytes(settings_service::PROBE_BARK_DEVICE_KEY_SETTING)
                .unwrap()
                .as_deref(),
            Some(b"shared-device-key".as_slice())
        );
        let hardcoded_setter = concat!("set_secret_setting_bytes(\"", "probe_bark_device_key\"");
        assert!(!include_str!("commands/settings.rs").contains(hardcoded_setter));
    }

    #[test]
    fn desktop_typed_uploads_store_under_local_codex_home_retired() {
        let source = [
            include_str!("commands/threads.rs"),
            include_str!("services/threads.rs"),
            include_str!("commands/settings.rs"),
            include_str!("services/settings.rs"),
        ]
        .join("\n");
        for forbidden in ["store_uploads_with_state", "DesktopUploadFile"] {
            assert!(
                !source.contains(forbidden),
                "retired write chain remains: {forbidden}"
            );
        }
    }

    #[test]
    fn desktop_typed_uploads_use_shared_batch_validation_retired() {
        let source = [
            include_str!("commands/threads.rs"),
            include_str!("services/threads.rs"),
            include_str!("commands/settings.rs"),
            include_str!("services/settings.rs"),
        ]
        .join("\n");
        for forbidden in ["store_to_root", "DesktopDeleteUploadRequest"] {
            assert!(
                !source.contains(forbidden),
                "retired write chain remains: {forbidden}"
            );
        }
    }

    #[test]
    fn desktop_send_message_uses_shared_job_service_and_attachment_context_retired() {
        let source = [
            include_str!("commands/threads.rs"),
            include_str!("services/threads.rs"),
            include_str!("commands/settings.rs"),
            include_str!("services/settings.rs"),
        ]
        .join("\n");
        for forbidden in [
            "codex_job_spec_for_request",
            "start_codex_job",
            "send_message_with_state",
        ] {
            assert!(
                !source.contains(forbidden),
                "retired write chain remains: {forbidden}"
            );
        }
    }

    #[test]
    fn desktop_unsupported_action_is_explicitly_unavailable() {
        let response =
            job_service::action_unavailable("forkThread", "fork uses Codex app-server state");

        assert!(!response.available);
        assert!(!response.ok);
        assert_eq!(response.command, "forkThread");
        assert!(response
            .message
            .contains("fork uses Codex app-server state"));
    }

    #[test]
    fn desktop_home_does_not_carry_cleanup_or_goal_business_fields() {
        let source = include_str!("overview.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("overview source must include production section");

        for forbidden in ["archive_plan", "hidden_plan", "goal:"] {
            assert!(
                !source.contains(forbidden),
                "DesktopHome must stay a thin desktop summary and not carry {forbidden}"
            );
        }
    }

    #[test]
    fn fixed_probe_command_uses_platform_daemon_path() {
        let platform = PlatformPaths::for_kind_with_home(PlatformKind::Macos, "/Users/example");
        let config = Config::for_platform_kind_with_home(PlatformKind::Macos, "/Users/example");
        let plan = nexushub_core::services::probe::plan_probe_action(
            &config,
            &platform,
            nexushub_core::services::probe::ProbeAction::BarkTest,
        )
        .unwrap();
        let command = plan.job.unwrap().command;

        assert!(
            command.contains(
                "'/Users/example/Library/Application Support/NexusHub/bin/nexushub-webd'"
            ),
            "{command}"
        );
        assert!(!command.contains("/opt/nexushub/bin/nexushub-webd"));
    }

    #[test]
    fn macos_default_config_does_not_use_linux_panel_update_precheck() {
        let config = Config::for_platform_kind_with_home(PlatformKind::Macos, "/Users/example");

        assert!(config
            .update
            .panel_update_command
            .contains("nexushub-webd-update"));
        assert!(!config.update.panel_precheck_command.contains("systemctl"));
        assert!(!config
            .update
            .panel_precheck_command
            .contains("127.0.0.1:15742/healthz"));
        assert!(!config
            .update
            .panel_precheck_command
            .contains("/opt/nexushub"));
        assert!(config
            .update
            .panel_precheck_command
            .contains("Library/Application Support/NexusHub"));
    }
}
