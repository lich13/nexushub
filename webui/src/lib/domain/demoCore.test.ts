import { describe, expect, test } from "vitest";
import { buildDemoFixture, buildDemoPlatformOverview, buildDemoSystemCapabilities, type DemoFixtureKey } from "./demoCore";

const fixtureKeys: DemoFixtureKey[] = ["linux-api", "macos-tauri"];
const sharedKeys = ["app_updater", "job_history", "jobs", "probe", "settings", "thread_archive_actions", "thread_cleanup", "threads"];
const retiredKeys = ["admin_password", "csrf", "nginx", "public_endpoint", "security_settings", "turnstile", "web_auth"];

describe("machine demo fixtures", () => {
  test("both hosts use API version 1 and omit retired browser authentication fields", () => {
    for (const fixture of fixtureKeys) {
      const view = buildDemoFixture(fixture);
      expect(view.system.api_version).toBe(2);
      expect(view).not.toHaveProperty("security");
      expect(view.platform).not.toHaveProperty("webui_dir");
      for (const field of retiredKeys) expect(view.system.capabilities).not.toHaveProperty(field);
      expect(JSON.stringify(view)).not.toMatch(/192\.0\.2\.10|panel\.example\.com|\/opt\/nexushub/i);
    }
  });

  test("local desktop exposes shared capabilities and its native updater", () => {
    const view = buildDemoFixture("macos-tauri");
    expect(view.system.host_surface).toBe("desktop_embedded_tauri");
    expect(Object.keys(view.system.capabilities).sort()).toEqual(sharedKeys);
    expect(view.system.capabilities).toMatchObject({ app_updater: true, thread_cleanup: true, thread_archive_actions: true, systemd: false, linux_update_job: false, prune_backups: false });
    expect(view.platform).toMatchObject({ kind: "macos", service_kind: "tauri", service_name: "NexusHub.app" });
    expect(JSON.stringify(view)).not.toMatch(/systemd|Nginx|Turnstile|管理员密码|公网入口|Linux prune|Linux update/i);
  });

  test("remote API fixture retains Linux service, update and backup capabilities", () => {
    const view = buildDemoSystemCapabilities("linux-api");
    expect(view.host_surface).toBe("linux_server_api");
    expect(Object.keys(view.capabilities).sort()).toEqual([...sharedKeys, "systemd", "linux_update_job", "prune_backups"].sort());
    expect(view.capabilities).toMatchObject({ systemd: true, linux_update_job: true, prune_backups: true });
    expect(buildDemoPlatformOverview("linux-api")).toMatchObject({ kind: "linux", data_dir: "/srv/nexushub-demo", config_file: "/srv/nexushub-demo/config.toml", log_dir: "/srv/nexushub-demo/logs", service_kind: "systemd", service_name: "nexushub-webd" });
  });

  test("retired standalone and LAN browser fixtures are unavailable", () => {
    for (const key of ["linux-web", "desktop-lan-webui"]) expect(() => buildDemoFixture(key as DemoFixtureKey)).toThrow("Unsupported host surface");
  });
});
