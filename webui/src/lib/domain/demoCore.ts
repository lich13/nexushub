import type { PlatformOverview, SecuritySettings, SystemStatus } from "../../types";

export const demoWebPlatformOverview: PlatformOverview = {
  kind: "linux",
  data_dir: "/opt/nexushub",
  config_file: "/opt/nexushub/config.toml",
  webui_dir: "/opt/nexushub/webui",
  log_dir: "/opt/nexushub/logs",
  service_name: "nexushub",
  service_kind: "systemd"
};

export const demoDesktopPlatformOverview: PlatformOverview = {
  kind: "macos",
  data_dir: "~/Library/Application Support/NexusHub",
  config_file: "~/Library/Application Support/NexusHub/config.toml",
  webui_dir: "~/Library/Application Support/NexusHub/webui",
  log_dir: "~/Library/Logs/NexusHub",
  service_name: "NexusHub.app",
  service_kind: "tauri"
};

export const demoWebSystemStatus: SystemStatus = {
  host_label: "43.155.235.227",
  hostname: "codex-cloud-root",
  public_endpoint: "https://661313.xyz/nexushub/",
  capabilities: {
    threads: true,
    jobs: true,
    probe: true,
    status: true,
    settings: true,
    job_history: true,
    app_updater: true,
    web_auth: true,
    security_settings: true,
    turnstile: true,
    systemd: true,
    nginx: true,
    public_endpoint: true,
    admin_password: true,
    linux_update_job: true,
    prune_backups: true,
    thread_cleanup: true,
    probe_log_maintenance: true,
    thread_archive_actions: true
  },
  codex_home: "/root/.codex",
  configured_codex_home: "/root/.codex",
  resolved_codex_home: "/root/.codex",
  codex_home_source: "config",
  panel_db: "/opt/nexushub/panel.sqlite",
  state_db_integrity: "ok"
};

export const demoDesktopSystemStatus: SystemStatus = {
  host_label: "local-macos",
  hostname: "macos",
  public_endpoint: null,
  codex_home: "~/.codex",
  configured_codex_home: "~/.codex",
  resolved_codex_home: "~/.codex",
  codex_home_source: "default",
  panel_db: "~/Library/Application Support/NexusHub/panel.sqlite",
  state_db_integrity: "ok"
};

export const demoWebSecurity: SecuritySettings = {
  turnstile_enabled: false,
  turnstile_required: false,
  turnstile_site_key: "",
  turnstile_secret_configured: false,
  session_ttl_seconds: 31536000,
  turnstile_expected_hostname: "661313.xyz",
  turnstile_expected_action: "login"
};

export const demoDesktopSecurity: SecuritySettings = {
  turnstile_enabled: false,
  turnstile_required: false,
  turnstile_site_key: "",
  turnstile_secret_configured: false,
  session_ttl_seconds: 31536000,
  turnstile_expected_hostname: null,
  turnstile_expected_action: null
};
