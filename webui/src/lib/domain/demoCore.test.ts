import { describe, expect, test } from "vitest";
import demoCoreSource from "./demoCore.ts?raw";
import {
  buildDemoFixture,
  buildDemoPlatformOverview,
  buildDemoSecurity,
  buildDemoSystemStatus,
  type DemoFixtureKey
} from "./demoCore";

const fixtureKeys: DemoFixtureKey[] = ["linux-web", "macos-tauri"];

const expectedCapabilityKeys = [
  "admin_password",
  "app_updater",
  "csrf",
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
      thread_archive_actions: true
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
    expect(fixture.system.capabilities).toMatchObject({
      app_updater: true,
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
      service_name: "nexushub"
    });
    expect(buildDemoSecurity("linux-web")).toMatchObject({
      turnstile_expected_hostname: "661313.xyz",
      turnstile_expected_action: "login"
    });
    expect(serializedFixture).toMatch(/systemd|\/opt\/nexushub|43\.155\.235\.227|661313\.xyz|linux_update_job|prune_backups/);
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
});
