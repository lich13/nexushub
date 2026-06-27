import { describe, expect, test } from "vitest";
import demoCoreSource from "./demoCore.ts?raw";
import {
  buildDemoFixture,
  buildDemoPlatformOverview,
  buildDemoSecurity,
  buildDemoSystemStatus,
  type DemoFixtureKey
} from "./demoCore";

const fixtureKeys: DemoFixtureKey[] = ["linux-web", "macos-tauri", "desktop-lan-webui"];

const expectedCapabilityKeys = [
  "admin_password",
  "app_updater",
  "csrf",
  "desktop_webui_control",
  "job_history",
  "jobs",
  "linux_update_job",
  "nginx",
  "probe",
  "probe_log_maintenance",
  "prune_backups",
  "public_endpoint",
  "security_settings",
  "settings",
  "status",
  "systemd",
  "thread_archive_actions",
  "thread_cleanup",
  "threads",
  "turnstile",
  "web_auth"
];

const macosVisibleCapabilityKeys = [
  "app_updater",
  "desktop_webui_control",
  "job_history",
  "jobs",
  "probe",
  "probe_log_maintenance",
  "settings",
  "status",
  "thread_archive_actions",
  "thread_cleanup",
  "threads"
];

describe("demo fixture builder", () => {
  test("demo fixtures use neutral values and never leak production host details", () => {
    const serialized = JSON.stringify(fixtureKeys.map((fixture) => buildDemoFixture(fixture)));

    expect(serialized).not.toMatch(/43\.155\.235\.227|661313\.xyz|\/opt\/nexushub/i);
    expect(buildDemoPlatformOverview("linux-web")).toMatchObject({
      data_dir: "/srv/nexushub-demo",
      config_file: "/srv/nexushub-demo/config.toml",
      webui_dir: "/srv/nexushub-demo/webui",
      log_dir: "/srv/nexushub-demo/logs"
    });
    expect(buildDemoSystemStatus("linux-web")).toMatchObject({
      host_surface: "linux_server_webui",
      host_label: "demo-linux-web",
      public_endpoint: "https://demo.nexushub.local/nexushub/",
      panel_db: "/srv/nexushub-demo/panel.sqlite"
    });
    expect(buildDemoSecurity("linux-web")).toMatchObject({
      turnstile_expected_hostname: "demo.nexushub.local"
    });
  });

  test("builds complete runtime capabilities from one explicit field table", () => {
    expect(demoCoreSource).toContain("const capabilityFields");
    expect(demoCoreSource).toContain("satisfies Record<keyof SystemCapabilities,");
    expect(demoCoreSource).not.toContain("const shared = {");

    const fixtureCapabilities = fixtureKeys.map((fixture) => buildDemoSystemStatus(fixture).capabilities ?? {});
    expect(Object.keys(fixtureCapabilities[0]).sort()).toEqual(expectedCapabilityKeys);
    expect(Object.keys(fixtureCapabilities[1]).sort()).toEqual(macosVisibleCapabilityKeys);

    const [linuxCapabilities, macCapabilities] = fixtureCapabilities;
    for (const key of expectedCapabilityKeys) {
      expect(typeof linuxCapabilities[key as keyof typeof linuxCapabilities], `linux-web ${key}`).toBe("boolean");
      expect(typeof macCapabilities[key as keyof typeof macCapabilities], `macos-tauri ${key}`).toBe("boolean");
    }

    expect(linuxCapabilities).toMatchObject({
      web_auth: true,
      csrf: true,
      security_settings: true,
      turnstile: true,
      systemd: true,
      nginx: true,
      public_endpoint: true,
      admin_password: true,
      linux_update_job: true,
      prune_backups: true,
      app_updater: true
    });
    expect(macCapabilities).toMatchObject({
      web_auth: false,
      csrf: false,
      security_settings: false,
      turnstile: false,
      systemd: false,
      nginx: false,
      public_endpoint: false,
      admin_password: false,
      linux_update_job: false,
      prune_backups: false,
      app_updater: true,
      threads: true,
      jobs: true,
      probe: true,
      status: true,
      settings: true,
      job_history: true,
      thread_cleanup: true,
      probe_log_maintenance: true,
      thread_archive_actions: true,
      desktop_webui_control: true
    });
    expect({ ...macCapabilities }).not.toHaveProperty("systemd");
    expect({ ...macCapabilities }).not.toHaveProperty("turnstile");
    expect({ ...macCapabilities }).not.toHaveProperty("public_endpoint");
  });

  test("macOS Tauri fixture visible payload contains only shared host data and the app updater surface", () => {
    const fixture = buildDemoFixture("macos-tauri");
    const serializedFixture = JSON.stringify(fixture);

    expect(buildDemoPlatformOverview("macos-tauri")).toMatchObject({
      kind: "macos",
      service_kind: "tauri",
      service_name: "NexusHub.app"
    });
    expect(fixture.system.host_surface).toBe("desktop_embedded_tauri");
    expect(fixture.system.capabilities).toMatchObject({
      app_updater: true,
      desktop_webui_control: true,
      web_auth: false,
      systemd: false,
      nginx: false,
      prune_backups: false
    });
    expect(buildDemoSecurity("macos-tauri")).toEqual({});
    expect(serializedFixture).toMatch(/NexusHub\.app|Application Support\/NexusHub|Library\/Logs\/NexusHub/);
    expect(serializedFixture).not.toMatch(
      /systemd|Nginx|Turnstile|管理员密码|公网入口|Linux prune|Linux update|\/opt\/nexushub|43\.155\.235\.227|661313\.xyz/i
    );
  });

  test("Linux Web fixture keeps Linux-only WebUI operations visible", () => {
    const fixture = buildDemoFixture("linux-web");
    const serializedFixture = JSON.stringify(fixture);

    expect(buildDemoPlatformOverview("linux-web")).toMatchObject({
      kind: "linux",
      service_kind: "systemd",
      service_name: "nexushub-webd"
    });
    expect(buildDemoSecurity("linux-web")).toMatchObject({
      turnstile_expected_hostname: "demo.nexushub.local",
      turnstile_expected_action: "login"
    });
    expect(serializedFixture).toMatch(/systemd|demo-linux-web|demo\.nexushub\.local|linux_update_job|prune_backups/);
    expect(serializedFixture).not.toMatch(/\/opt\/nexushub|43\.155\.235\.227|661313\.xyz/i);
    expect(fixture.system.capabilities).toMatchObject({
      web_auth: true,
      csrf: true,
      security_settings: true,
      turnstile: true,
      admin_password: true,
      systemd: true,
      nginx: true,
      public_endpoint: true,
      linux_update_job: true,
      prune_backups: true
    });
  });

  test("desktop LAN WebUI fixture keeps browser auth but hides Linux server administration", () => {
    const fixture = buildDemoFixture("desktop-lan-webui");
    const serializedFixture = JSON.stringify(fixture);

    expect(buildDemoPlatformOverview("desktop-lan-webui")).toMatchObject({
      kind: "macos",
      service_kind: "desktop-lan-webui",
      service_name: "NexusHub LAN WebUI"
    });
    expect(fixture.system).toMatchObject({
      platform: "macos",
      host_surface: "desktop_lan_webui",
      public_endpoint: null
    });
    expect(fixture.system.capabilities).toMatchObject({
      web_auth: true,
      csrf: true,
      security_settings: false,
      turnstile: false,
      systemd: false,
      nginx: false,
      public_endpoint: false,
      admin_password: false,
      linux_update_job: false,
      prune_backups: false,
      app_updater: false,
      desktop_webui_control: false,
      thread_cleanup: true
    });
    expect(buildDemoSecurity("desktop-lan-webui")).toMatchObject({
      turnstile_enabled: false,
      turnstile_required: false,
      session_ttl_seconds: 86400
    });
    expect(serializedFixture).not.toMatch(/Linux update|Linux prune|\/opt\/nexushub|43\.155\.235\.227|661313\.xyz/i);
  });
});
