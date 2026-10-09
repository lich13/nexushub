import { expect, test, type Page } from "@playwright/test";
import * as demo from "../src/lib/api/demo";
import type { ProbeSettings } from "../src/types";
import { assertContrast, assertNoOverflow, mockApi, mockCommand } from "./fixtures";

type JobOutcome = "succeeded" | "failed";

function gotifySettings(): ProbeSettings {
  const settings = demo.demoProbeSettings("macos-tauri");
  return {
    ...settings,
    codex: {
      ...settings.codex,
      home: null,
      configured_codex_home: null,
      resolved_codex_home: null,
      codex_home_source: null,
      logs_db_source: null,
      workspace: null,
      host_label: "fixture-host"
    },
    notifications: {
      ...settings.notifications,
      enabled: false,
      device_key_configured: false,
      server_url: "https://example.invalid/bark/"
    },
    gotify: {
      enabled: false,
      server_url: "https://example.invalid/gotify/",
      priority: 5,
      token_configured: true
    }
  };
}

async function openGotifySettings(page: Page, outcome: JobOutcome, mobile: boolean) {
  await mockApi(page);
  const capabilityResponse = demo.demoSystemCapabilities("macos-tauri");
  let currentSettings = gotifySettings();
  const saves: Array<Record<string, any>> = [];
  const testCalls: string[] = [];
  const saveSettings = (args: Record<string, any>) => {
    saves.push(args.settings);
    currentSettings = {
      ...currentSettings,
      gotify: { ...(currentSettings.gotify ?? {}), ...args.settings.gotify }
    };
    return currentSettings;
  };

  await mockCommand(page, "system.capabilities", () => ({
    ...capabilityResponse,
    capabilities: { ...capabilityResponse.capabilities, gotify: true }
  }));
  await mockCommand(page, "probe.settings.get", () => ({ available: true, data: currentSettings }));
  await mockCommand(page, "probe.settings.save", saveSettings);
  await mockCommand(page, "probe.gotifyTest", () => {
    testCalls.push("probe.gotifyTest");
    return { job_id: "fixture-gotify-test" };
  });
  await mockCommand(page, "jobs.detail", args => ({
    ...demo.demoJob(args.id),
    id: args.id,
    kind: "probe_gotify_test",
    status: outcome,
    title: "Gotify fixture test",
    output: outcome === "succeeded" ? "HTTP 200\nserver accepted" : "Fixture delivery rejected",
    error: outcome === "failed" ? "Fixture delivery rejected" : undefined
  }));
  await page.goto("/");
  const navigation = page.locator(mobile ? ".mobile-tabs" : ".side-nav");
  await navigation.getByRole("button", { name: "Probe", exact: true }).click();
  await page.getByRole("button", { name: "通知配置", exact: true }).click();
  const panel = page.locator(".panel").filter({ has: page.getByText("Gotify", { exact: true }) });
  await expect(panel.getByLabel("启用 Gotify", { exact: true })).toBeVisible();
  return { panel, saves, testCalls, saveSettings };
}

for (const colorScheme of ["light", "dark"] as const) {
  test(`Gotify settings stay independent and keyboard-accessible in a narrow ${colorScheme} window`, async ({ page }) => {
    await page.setViewportSize({ width: 360, height: 780 });
    await page.emulateMedia({ colorScheme, reducedMotion: "reduce" });
    const { panel, saves, testCalls } = await openGotifySettings(page, "succeeded", true);

    const enabled = panel.getByLabel("启用 Gotify", { exact: true });
    const address = panel.getByLabel("HTTPS 地址", { exact: true });
    const token = panel.getByLabel("Application Token", { exact: true });
    const priority = panel.getByLabel("优先级", { exact: true });
    await expect(enabled).not.toBeChecked();
    await expect(priority).toHaveValue("5");
    await expect(priority).toHaveAttribute("min", "0");
    await expect(priority).toHaveAttribute("max", "10");
    await expect(token).toHaveValue("");
    await expect(token).toHaveAttribute("placeholder", "已配置，留空保持不变");

    await enabled.focus();
    await page.keyboard.press("Space");
    await expect(enabled).toBeChecked();
    await page.keyboard.press("Tab");
    await expect(address).toBeFocused();
    await page.keyboard.press("Tab");
    await expect(token).toBeFocused();

    const save = panel.getByRole("button", { name: "保存", exact: true });
    await save.focus();
    await page.keyboard.press("Enter");
    await expect.poll(() => saves.length).toBe(1);
    expect(saves[0].gotify).toMatchObject({
      enabled: true,
      server_url: "https://example.invalid/gotify/",
      priority: 5
    });
    expect(saves[0].gotify).not.toHaveProperty("token");
    expect(saves[0].probe.notifications.enabled).toBe(false);
    await expect(page.getByLabel("启用 Bark", { exact: true })).not.toBeChecked();
    await expect(panel.getByRole("status")).toContainText("设置已保存");
    await expect(token).toHaveValue("");
    await expect(token).toHaveAttribute("placeholder", "已配置，留空保持不变");

    await panel.getByRole("button", { name: "测试推送", exact: true }).click();
    const feedback = panel.getByRole("status").filter({ hasText: "服务器已接收测试通知" });
    await expect(feedback).toBeVisible();
    const feedbackText = await feedback.textContent();
    expect(feedbackText).toContain("服务器已接收测试通知");
    expect(feedbackText).not.toMatch(/安卓设备(?:已经|已)(?:收到|接收|送达)/);
    expect(testCalls).toEqual(["probe.gotifyTest"]);
    await assertContrast(page, ".probe-card-stack .field-label");
    await assertNoOverflow(page);
  });
}

test("Gotify save and delivery failures remain visible", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 820 });
  await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" });
  const { panel, saveSettings } = await openGotifySettings(page, "failed", false);
  const enabled = panel.getByLabel("启用 Gotify", { exact: true });
  await enabled.check();

  await mockCommand(page, "probe.settings.save", () => Promise.reject(new Error("Fixture settings rejected")));
  await panel.getByRole("button", { name: "保存", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("Fixture settings rejected");

  await mockCommand(page, "probe.settings.save", saveSettings);
  await panel.getByRole("button", { name: "保存", exact: true }).click();
  await expect(panel.getByRole("status")).toContainText("设置已保存");

  await panel.getByRole("button", { name: "测试推送", exact: true }).click();
  await expect(panel.getByRole("alert")).toContainText("测试推送失败");
  await expect(panel.getByRole("alert")).not.toContainText("已接收");
  await assertContrast(page, ".form-error");
  await assertNoOverflow(page);
});
