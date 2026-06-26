import type { PlatformOverview, SecuritySettings, SystemCapabilities, SystemStatus } from "../../types";

export type DemoFixtureKey = "linux-web" | "macos-tauri" | "desktop-lan-webui";
export type DemoFixture = {
  platform: PlatformOverview;
  system: SystemStatus;
  security: SecuritySettings;
};

const macApplicationSupport = "~/Library/Application Support/NexusHub";
const linuxDemoRoot = "/srv/nexushub-demo";

type CapabilityFieldValues = Record<DemoFixtureKey, boolean>;

const capabilityFields = {
  threads: { "linux-web": true, "macos-tauri": true, "desktop-lan-webui": true },
  jobs: { "linux-web": true, "macos-tauri": true, "desktop-lan-webui": true },
  probe: { "linux-web": true, "macos-tauri": true, "desktop-lan-webui": true },
  status: { "linux-web": true, "macos-tauri": true, "desktop-lan-webui": true },
  settings: { "linux-web": true, "macos-tauri": true, "desktop-lan-webui": true },
  job_history: { "linux-web": true, "macos-tauri": true, "desktop-lan-webui": true },
  app_updater: { "linux-web": true, "macos-tauri": true, "desktop-lan-webui": false },
  web_auth: { "linux-web": true, "macos-tauri": false, "desktop-lan-webui": true },
  csrf: { "linux-web": true, "macos-tauri": false, "desktop-lan-webui": true },
  security_settings: { "linux-web": true, "macos-tauri": false, "desktop-lan-webui": false },
  turnstile: { "linux-web": true, "macos-tauri": false, "desktop-lan-webui": false },
  systemd: { "linux-web": true, "macos-tauri": false, "desktop-lan-webui": false },
  nginx: { "linux-web": true, "macos-tauri": false, "desktop-lan-webui": false },
  public_endpoint: { "linux-web": true, "macos-tauri": false, "desktop-lan-webui": false },
  admin_password: { "linux-web": true, "macos-tauri": false, "desktop-lan-webui": false },
  linux_update_job: { "linux-web": true, "macos-tauri": false, "desktop-lan-webui": false },
  prune_backups: { "linux-web": true, "macos-tauri": false, "desktop-lan-webui": false },
  thread_cleanup: { "linux-web": true, "macos-tauri": true, "desktop-lan-webui": true },
  probe_log_maintenance: { "linux-web": true, "macos-tauri": true, "desktop-lan-webui": true },
  thread_archive_actions: { "linux-web": true, "macos-tauri": true, "desktop-lan-webui": true },
  desktop_webui_control: { "linux-web": false, "macos-tauri": true, "desktop-lan-webui": false }
} satisfies Record<keyof SystemCapabilities, CapabilityFieldValues>;

const macosEnumerableCapabilityKeys: readonly (keyof SystemCapabilities)[] = [
  "threads",
  "jobs",
  "probe",
  "status",
  "settings",
  "job_history",
  "app_updater",
  "thread_cleanup",
  "probe_log_maintenance",
  "thread_archive_actions",
  "desktop_webui_control"
] satisfies Array<keyof SystemCapabilities>;

function buildDemoCapabilities(fixture: DemoFixtureKey): SystemCapabilities {
  if (fixture !== "macos-tauri") {
    return Object.fromEntries(
      Object.entries(capabilityFields).map(([field, values]) => [field, values[fixture]])
    ) as SystemCapabilities;
  }
  const capabilities: Partial<SystemCapabilities> = {};
  for (const field of Object.keys(capabilityFields) as Array<keyof SystemCapabilities>) {
    Object.defineProperty(capabilities, field, {
      value: capabilityFields[field][fixture],
      enumerable: macosEnumerableCapabilityKeys.includes(field),
      configurable: true
    });
  }
  return capabilities as SystemCapabilities;
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
        platform: "macos",
        host_surface: "desktop_embedded_tauri",
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
  if (fixture === "desktop-lan-webui") {
    return {
      platform: {
        kind: "macos",
        data_dir: macApplicationSupport,
        config_file: `${macApplicationSupport}/config.toml`,
        webui_dir: `${macApplicationSupport}/desktop-assets`,
        log_dir: "~/Library/Logs/NexusHub",
        service_name: "NexusHub LAN WebUI",
        service_kind: "desktop-lan-webui"
      },
      system: {
        platform: "macos",
        host_surface: "desktop_lan_webui",
        host_label: "local-desktop-lan",
        hostname: "macos",
        public_endpoint: null,
        capabilities: buildDemoCapabilities("desktop-lan-webui"),
        codex_home: "~/.codex",
        configured_codex_home: "~/.codex",
        resolved_codex_home: "~/.codex",
        codex_home_source: "default",
        panel_db: `${macApplicationSupport}/nexushub.sqlite`,
        state_db_integrity: "ok"
      },
      security: {
        turnstile_enabled: false,
        turnstile_required: false,
        turnstile_site_key: "",
        turnstile_secret_configured: false,
        session_ttl_seconds: 86400,
        turnstile_expected_hostname: null,
        turnstile_expected_action: "login"
      }
    };
  }
  return {
    platform: {
      kind: "linux",
      data_dir: linuxDemoRoot,
      config_file: `${linuxDemoRoot}/config.toml`,
      webui_dir: `${linuxDemoRoot}/webui`,
      log_dir: `${linuxDemoRoot}/logs`,
      service_name: "nexushub",
      service_kind: "systemd"
    },
    system: {
      platform: "linux",
      host_surface: "linux_server_webui",
      host_label: "demo-linux-web",
      hostname: "demo-linux-web",
      public_endpoint: "https://demo.nexushub.local/nexushub/",
      capabilities: buildDemoCapabilities("linux-web"),
      codex_home: "/root/.codex",
      configured_codex_home: "/root/.codex",
      resolved_codex_home: "/root/.codex",
      codex_home_source: "config",
      panel_db: `${linuxDemoRoot}/panel.sqlite`,
      state_db_integrity: "ok"
    },
    security: {
      turnstile_enabled: false,
      turnstile_required: false,
      turnstile_site_key: "",
      turnstile_secret_configured: false,
      session_ttl_seconds: 31536000,
      turnstile_expected_hostname: "demo.nexushub.local",
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
