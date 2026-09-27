import type { PlatformOverview, SecuritySettings, SystemCapabilities, SystemCapabilitiesResponse } from "../../types";

export type DemoFixtureKey = "linux-web" | "macos-tauri";
export type DemoFixture = {
  platform: PlatformOverview;
  system: SystemCapabilitiesResponse;
  security: SecuritySettings;
};

const macApplicationSupport = "~/Library/Application Support/NexusHub";
const linuxDemoRoot = "/srv/nexushub-demo";

type CapabilityFieldValues = Record<DemoFixtureKey, boolean>;

const capabilityFields = {
  threads: { "linux-web": true, "macos-tauri": true },
  jobs: { "linux-web": true, "macos-tauri": true },
  probe: { "linux-web": true, "macos-tauri": true },
  settings: { "linux-web": true, "macos-tauri": true },
  job_history: { "linux-web": true, "macos-tauri": true },
  app_updater: { "linux-web": true, "macos-tauri": true },
  web_auth: { "linux-web": true, "macos-tauri": false },
  csrf: { "linux-web": true, "macos-tauri": false },
  security_settings: { "linux-web": true, "macos-tauri": false },
  turnstile: { "linux-web": true, "macos-tauri": false },
  systemd: { "linux-web": true, "macos-tauri": false },
  nginx: { "linux-web": true, "macos-tauri": false },
  public_endpoint: { "linux-web": true, "macos-tauri": false },
  admin_password: { "linux-web": true, "macos-tauri": false },
  linux_update_job: { "linux-web": true, "macos-tauri": false },
  prune_backups: { "linux-web": true, "macos-tauri": false },
  thread_cleanup: { "linux-web": true, "macos-tauri": true },
  thread_archive_actions: { "linux-web": true, "macos-tauri": true },
} satisfies Record<keyof SystemCapabilities, CapabilityFieldValues>;

const macosEnumerableCapabilityKeys: readonly (keyof SystemCapabilities)[] = [
  "threads",
  "jobs",
  "probe",
  "settings",
  "job_history",
  "app_updater",
  "thread_cleanup",
  "thread_archive_actions",
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
  if (fixture !== "linux-web" && fixture !== "macos-tauri") throw new Error("Unsupported host surface");
  const desktop = fixture === "macos-tauri";
  const root = desktop ? macApplicationSupport : linuxDemoRoot;
  return {
    platform: {
      kind: desktop ? "macos" : "linux",
      data_dir: root,
      config_file: `${root}/config.toml`,
      webui_dir: desktop ? "" : `${root}/webui`,
      log_dir: desktop ? "~/Library/Logs/NexusHub" : `${root}/logs`,
      service_name: desktop ? "NexusHub.app" : "nexushub-webd",
      service_kind: desktop ? "tauri" : "systemd"
    },
    system: {
      host_surface: desktop ? "desktop_embedded_tauri" : "linux_server_webui",
      capabilities: buildDemoCapabilities(fixture)
    },
    security: desktop ? {} as SecuritySettings : {
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

export function buildDemoSystemCapabilities(fixture: DemoFixtureKey): SystemCapabilitiesResponse {
  return buildDemoFixture(fixture).system;
}

export function buildDemoSecurity(fixture: DemoFixtureKey): SecuritySettings {
  return buildDemoFixture(fixture).security;
}

const demoWebFixture = buildDemoFixture("linux-web");
const demoDesktopFixture = buildDemoFixture("macos-tauri");

export const demoWebPlatformOverview: PlatformOverview = demoWebFixture.platform;
export const demoDesktopPlatformOverview: PlatformOverview = demoDesktopFixture.platform;
export const demoWebSystemCapabilities: SystemCapabilitiesResponse = demoWebFixture.system;
export const demoDesktopSystemCapabilities: SystemCapabilitiesResponse = demoDesktopFixture.system;
export const demoWebSecurity: SecuritySettings = demoWebFixture.security;
export const demoDesktopSecurity: SecuritySettings = demoDesktopFixture.security;
