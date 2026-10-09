import { expect, test, type Page } from "@playwright/test";
import * as demo from "../src/lib/api/demo";
import type { ProbeSettings } from "../src/types";
import { assertContrast, assertNoOverflow, mockApi, mockCommand } from "./fixtures";

async function openBarkSettings(page: Page, mobile: boolean) {
  const calls = await mockApi(page);
  const defaults = demo.demoProbeSettings("macos-tauri");
  let settings: ProbeSettings = {
    ...defaults,
    codex: {
      ...defaults.codex,
      home: null,
      configured_codex_home: null,
      resolved_codex_home: null,
      codex_home_source: null,
      logs_db_source: null,
      workspace: null,
      host_label: "fixture-host"
    },
    notifications: {
      ...defaults.notifications,
      enabled: true,
      device_key_configured: true,
      server_url: "https://bark.example.invalid"
    }
  };
  const saved: Array<Record<string, any>> = [];
  // A stale server response must not restore a retired control or save field.
  await mockCommand(page, "probe.settings.get", () => ({
    available: true,
    data: { ...settings, gotify: { enabled: true, server_url: "https://notify.example.invalid", priority: 7, token_configured: true } }
  }));
  await mockCommand(page, "probe.settings.save", args => {
    saved.push(args.settings);
    settings = {
      ...settings,
      codex: { ...settings.codex, ...args.settings.codex },
      probe: { ...settings.probe, ...args.settings.probe },
      notifications: { ...settings.notifications, ...args.settings.probe.notifications }
    };
    return settings;
  });
  await mockCommand(page, "probe.barkTest", () => ({ job_id: "fixture-bark-test" }));
  await mockCommand(page, "jobs.detail", args => ({
    ...demo.demoJob(args.id),
    id: args.id,
    kind: "probe_bark_test",
    title: "Bark fixture test",
    status: "succeeded",
    output: "HTTP 200",
    error: undefined
  }));
  await page.goto("/");
  await page.locator(mobile ? ".mobile-tabs" : ".side-nav").getByRole("button", { name: "Probe", exact: true }).click();
  await page.getByRole("button", { name: "通知配置", exact: true }).click();
  const panel = page.locator(".panel").filter({ has: page.getByLabel("Device Key", { exact: true }) });
  await expect(panel).toBeVisible();
  return { panel, saved, calls };
}

for (const colorScheme of ["light", "dark"] as const) {
  test(`retired Gotify controls stay absent while Bark saves and tests in narrow ${colorScheme} windows`, async ({ page }) => {
    await page.setViewportSize({ width: 360, height: 780 });
    await page.emulateMedia({ colorScheme, reducedMotion: "reduce" });
    const { panel, saved, calls } = await openBarkSettings(page, true);
    await expect(page.getByText(/Gotify|Application Token|优先级/i)).toHaveCount(0);
    await expect(page.getByLabel("启用 Bark", { exact: true })).toBeChecked();
    const key = panel.getByLabel("Device Key", { exact: true });
    await expect(key).toHaveValue("");
    await expect(key).toHaveAttribute("placeholder", "已配置，留空保持不变");
    await expect(page.getByLabel("Codex 通知", { exact: true })).toBeVisible();
    await expect(page.getByLabel("Grok 通知", { exact: true })).toBeVisible();
    await expect(page.getByLabel("Claude Code 通知", { exact: true })).toBeVisible();

    await page.getByLabel("启用 Bark", { exact: true }).uncheck();
    await panel.getByRole("button", { name: "保存", exact: true }).focus();
    await page.keyboard.press("Enter");
    await expect.poll(() => saved.length).toBe(1);
    expect(saved[0].probe.notifications.enabled).toBe(false);
    expect(JSON.stringify(saved[0])).not.toMatch(/gotify|token_configured/i);
    await expect(panel.locator(".form-success")).toHaveText("设置已保存");
    await expect(key).toHaveValue("");

    await page.getByLabel("启用 Bark", { exact: true }).check();
    await panel.getByRole("button", { name: "保存", exact: true }).click();
    await expect.poll(() => saved.length).toBe(2);
    expect(saved[1].probe.notifications.enabled).toBe(true);
    await panel.getByRole("button", { name: "测试推送", exact: true }).click();
    await expect.poll(() => calls.filter(command => command === "probe.barkTest").length).toBe(1);
    await expect(page.getByText("Bark fixture test", { exact: true }).first()).toBeVisible();
    expect(calls.filter(command => /gotify/i.test(command))).toEqual([]);
    await assertContrast(page, ".probe-card-stack .field-label");
    await assertNoOverflow(page);
  });
}

test("Bark save failure retains the draft without restoring retired settings", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 820 });
  await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" });
  const { panel, calls } = await openBarkSettings(page, false);
  await mockCommand(page, "probe.settings.save", () => Promise.reject(new Error("Fixture Bark settings rejected")));
  await panel.getByLabel("Device Key", { exact: true }).fill("fixture-replacement-bark-key");
  await panel.getByRole("button", { name: "保存", exact: true }).click();
  await expect(panel.locator(".form-error")).toHaveText("Fixture Bark settings rejected");
  await expect(panel.getByLabel("Device Key", { exact: true })).toHaveValue("fixture-replacement-bark-key");
  await expect(page.getByText(/Gotify|Application Token|优先级/i)).toHaveCount(0);
  expect(calls.filter(command => /gotify/i.test(command))).toEqual([]);
  await assertContrast(page, ".form-error");
  await assertNoOverflow(page);
});
