import type { PlatformOverview, SecuritySettings, SystemCapabilities, SystemStatus } from "../../types";

export type DemoFixtureKey = "linux-web" | "macos-tauri";
export type DemoFixture = {
  platform: PlatformOverview;
  system: SystemStatus;
  security: SecuritySettings;
};

const macApplicationSupport = "~/Library/Application Support/NexusHub";

function buildDemoCapabilities(fixture: DemoFixtureKey): SystemCapabilities {
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
  if (fixture === "macos-tauri") {
    return {
      ...shared,
      web_auth: false,
      csrf: false,
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
    csrf: true,
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

export function buildDemoFixture(fixture: DemoFixtureKey): DemoFixture {
  if (fixture === "macos-tauri") {
    return {
      platform: {
        kind: "macos",
        data_dir: macApplicationSupport,
        config_file: `${macApplicationSupport}/config.toml`,
        webui_dir: "",
        log_dir: "~/Library/Logs/NexusHub",
        service_name: "NexusHub.app",
        service_kind: "tauri"
      },
      system: {
        host_label: "local-macos",
        hostname: "macos",
        public_endpoint: null,
        capabilities: buildDemoCapabilities("macos-tauri"),
        codex_home: "~/.codex",
        configured_codex_home: "~/.codex",
        resolved_codex_home: "~/.codex",
        codex_home_source: "default",
        panel_db: `${macApplicationSupport}/nexushub.sqlite`,
        state_db_integrity: "ok"
      },
      security: {} as SecuritySettings
    };
  }
  return {
    platform: {
      kind: "linux",
      data_dir: "/opt/nexushub",
      config_file: "/opt/nexushub/config.toml",
      webui_dir: "/opt/nexushub/webui",
      log_dir: "/opt/nexushub/logs",
      service_name: "nexushub",
      service_kind: "systemd"
    },
    system: {
      host_label: "43.155.235.227",
      hostname: "codex-cloud-root",
      public_endpoint: "https://661313.xyz/nexushub/",
      capabilities: buildDemoCapabilities("linux-web"),
      codex_home: "/root/.codex",
      configured_codex_home: "/root/.codex",
      resolved_codex_home: "/root/.codex",
      codex_home_source: "config",
      panel_db: "/opt/nexushub/panel.sqlite",
      state_db_integrity: "ok"
    },
    security: {
      turnstile_enabled: false,
      turnstile_required: false,
      turnstile_site_key: "",
      turnstile_secret_configured: false,
      session_ttl_seconds: 31536000,
      turnstile_expected_hostname: "661313.xyz",
      turnstile_expected_action: "login"
    }
  };
}

export function buildDemoPlatformOverview(fixture: DemoFixtureKey): PlatformOverview {
  return buildDemoFixture(fixture).platform;
}

export function buildDemoSystemStatus(fixture: DemoFixtureKey): SystemStatus {
  return buildDemoFixture(fixture).system;
}

export function buildDemoSecurity(fixture: DemoFixtureKey): SecuritySettings {
  return buildDemoFixture(fixture).security;
}

const demoWebFixture = buildDemoFixture("linux-web");
const demoDesktopFixture = buildDemoFixture("macos-tauri");

export const demoWebPlatformOverview: PlatformOverview = demoWebFixture.platform;
export const demoDesktopPlatformOverview: PlatformOverview = demoDesktopFixture.platform;
export const demoWebSystemStatus: SystemStatus = demoWebFixture.system;
export const demoDesktopSystemStatus: SystemStatus = demoDesktopFixture.system;
export const demoWebSecurity: SecuritySettings = demoWebFixture.security;
export const demoDesktopSecurity: SecuritySettings = demoDesktopFixture.security;
