import type { PlatformOverview, SecuritySettings, SystemCapabilities, SystemStatus } from "../../types";

type DemoPlatform = "web" | "desktop";

const macApplicationSupport = "~/Library/Application Support/NexusHub";

function buildDemoCapabilities(platform: DemoPlatform): SystemCapabilities {
  const shared = {
    threads: true,
    jobs: true,
    probe: true,
    status: true,
    settings: true,
    job_history: true,
    app_updater: true,
    thread_cleanup: true,
    probe_log_maintenance: true,
    thread_archive_actions: true
  };
  if (platform === "desktop") {
    return {
      ...shared,
      web_auth: false,
      security_settings: false,
      turnstile: false,
      systemd: false,
      nginx: false,
      public_endpoint: false,
      admin_password: false,
      linux_update_job: false,
      prune_backups: false
    };
  }
  return {
    ...shared,
    web_auth: true,
    security_settings: true,
    turnstile: true,
    systemd: true,
    nginx: true,
    public_endpoint: true,
    admin_password: true,
    linux_update_job: true,
    prune_backups: true
  };
}

export function buildDemoPlatformOverview(platform: DemoPlatform): PlatformOverview {
  if (platform === "desktop") {
    return {
      kind: "macos",
      data_dir: macApplicationSupport,
      config_file: `${macApplicationSupport}/config.toml`,
      webui_dir: "",
      log_dir: "~/Library/Logs/NexusHub",
      service_name: "NexusHub.app",
      service_kind: "tauri"
    };
  }
  return {
    kind: "linux",
    data_dir: "/opt/nexushub",
    config_file: "/opt/nexushub/config.toml",
    webui_dir: "/opt/nexushub/webui",
    log_dir: "/opt/nexushub/logs",
    service_name: "nexushub",
    service_kind: "systemd"
  };
}

export function buildDemoSystemStatus(platform: DemoPlatform): SystemStatus {
  if (platform === "desktop") {
    return {
      host_label: "local-macos",
      hostname: "macos",
      public_endpoint: null,
      capabilities: buildDemoCapabilities("desktop"),
      codex_home: "~/.codex",
      configured_codex_home: "~/.codex",
      resolved_codex_home: "~/.codex",
      codex_home_source: "default",
      panel_db: `${macApplicationSupport}/nexushub.sqlite`,
      state_db_integrity: "ok"
    };
  }
  return {
    host_label: "43.155.235.227",
    hostname: "codex-cloud-root",
    public_endpoint: "https://661313.xyz/nexushub/",
    capabilities: buildDemoCapabilities("web"),
    codex_home: "/root/.codex",
    configured_codex_home: "/root/.codex",
    resolved_codex_home: "/root/.codex",
    codex_home_source: "config",
    panel_db: "/opt/nexushub/panel.sqlite",
    state_db_integrity: "ok"
  };
}

export function buildDemoSecurity(platform: DemoPlatform): SecuritySettings {
  if (platform === "desktop") {
    return {} as SecuritySettings;
  }
  return {
    turnstile_enabled: false,
    turnstile_required: false,
    turnstile_site_key: "",
    turnstile_secret_configured: false,
    session_ttl_seconds: 31536000,
    turnstile_expected_hostname: "661313.xyz",
    turnstile_expected_action: "login"
  };
}

export const demoWebPlatformOverview: PlatformOverview = buildDemoPlatformOverview("web");
export const demoDesktopPlatformOverview: PlatformOverview = buildDemoPlatformOverview("desktop");
export const demoWebSystemStatus: SystemStatus = buildDemoSystemStatus("web");
export const demoDesktopSystemStatus: SystemStatus = buildDemoSystemStatus("desktop");
export const demoWebSecurity: SecuritySettings = buildDemoSecurity("web");
export const demoDesktopSecurity: SecuritySettings = buildDemoSecurity("desktop");
