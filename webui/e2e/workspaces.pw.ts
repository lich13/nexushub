import { expect, test } from "@playwright/test";
import { assertContrast, assertNoOverflow, mockApi } from "./fixtures";

const viewports = [{ width: 1440, height: 900 }, { width: 1280, height: 820 }, { width: 390, height: 844 }, { width: 320, height: 844 }, ...[700, 701, 767, 768].map(width => ({ width, height: 844 }))];

for (const colorScheme of ["light", "dark"] as const) {
  for (const viewport of viewports) {
    test(`${colorScheme} ${viewport.width}: four workspaces and list transitions`, async ({ page }, info) => {
      await page.setViewportSize(viewport);
      await page.emulateMedia({ colorScheme });
      const errors: string[] = [];
      page.on("pageerror", error => errors.push(error.message));
      page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
      await mockApi(page);
      await page.goto("/");
      const nav = page.locator(viewport.width <= 767 ? ".mobile-tabs" : ".side-nav");
      const first = page.locator(".thread-item").first();
      await first.click();
      await expect(page.locator(".conversation-title")).toBeVisible();
      await assertContrast(page, ".conversation-title, .history-collapse-cell span, .chat-meta span, .tool-title");
      await assertNoOverflow(page);
      if (viewport.width <= 767) {
        await page.getByTitle("返回任务列表").click();
        await expect(first).toBeVisible();
        await page.getByPlaceholder("搜索标题或 ID").fill("not-found-fixture");
        await expect(page.getByText("没有匹配任务")).toBeVisible();
        await page.getByPlaceholder("搜索标题或 ID").fill("");
        await first.click();
      } else {
        await page.getByTitle("折叠导航").click();
        const navBox = await page.locator(".side-nav").boundingBox();
        const listBox = await page.locator(".thread-column").boundingBox();
        expect(navBox?.width).toBe(48);
        expect(listBox!.x).toBeGreaterThanOrEqual(navBox!.width);
        await page.getByTitle("展开导航").click();
      }
      await page.screenshot({ path: info.outputPath("codex.png") });
      await nav.getByRole("button", { name: "Grok Build", exact: true }).click();
      await page.locator(".provider-session").first().click();
      await expect(page.locator(".provider-events")).toBeVisible();
      await assertContrast(page, ".provider-detail .conversation-title, .provider-events .markdown-content");
      await assertNoOverflow(page);
      if (viewport.width <= 767) {
        await page.getByTitle("返回任务列表").click();
        await expect(page.locator(".provider-session")).toBeVisible();
      }
      await nav.getByRole("button", { name: "Probe", exact: true }).click();
      await expect(page.getByRole("button", { name: "最近事件", exact: true })).toBeVisible();
      await assertContrast(page, ".probe-status-banner strong, .probe-status-banner span, .probe-event-card .status-chip");
      await page.locator(".probe-layout .segmented").getByRole("button", { name: "通知配置" }).click();
      await expect(page.getByText("Bark", { exact: true })).toBeVisible();
      await assertContrast(page, ".field-label, .metric span, .metric strong");
      await assertNoOverflow(page);
      await page.screenshot({ path: info.outputPath("probe.png"), fullPage: true });
      await nav.getByRole("button", { name: "设置", exact: true }).click();
      await page.getByRole("tab", { name: "维护", exact: true }).click();
      await expect(page.getByText("Codex 日志库维护", { exact: true })).toBeVisible();
      await assertContrast(page, ".panel header, .metric span, .metric strong, .status-chip");
      await assertNoOverflow(page);
      expect(errors).toEqual([]);
    });
  }
}

test("theme changes at runtime and 200 percent zoom keeps controls available", async ({ page }, info) => {
  await mockApi(page);
  await page.setViewportSize({ width: 1280, height: 820 });
  await page.goto("/");
  await page.emulateMedia({ colorScheme: "light" });
  const color = () => page.locator("body").evaluate(el => getComputedStyle(el).backgroundColor);
  expect(await color()).toBe("rgb(255, 255, 255)");
  await page.emulateMedia({ colorScheme: "dark" });
  expect(await color()).toBe("rgb(32, 32, 32)");
  await page.locator(".thread-item").first().click();
  await page.evaluate(() => document.documentElement.style.zoom = "2");
  await assertNoOverflow(page);
  await page.screenshot({ path: info.outputPath("zoom-200.png") });
});

test("menus support keyboard, rename, archive and restore from their visible entrypoints", async ({ page }) => {
  const calls = await mockApi(page);
  await page.goto("/");
  await page.locator(".thread-item").first().click();
  const menu = page.getByLabel("任务操作", { exact: true });
  await menu.press("ArrowDown");
  await expect(page.getByRole("button", { name: "改名", exact: true })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(menu).toBeFocused();
  await menu.press("ArrowUp");
  await expect(page.locator(".task-menu-items button").last()).toBeFocused();
  await page.keyboard.press("Escape");
  await menu.click();
  await page.getByRole("button", { name: "改名", exact: true }).click();
  const title = "长任务名称".repeat(24);
  await page.getByLabel("任务名称", { exact: true }).fill(title);
  await page.getByTitle("保存名称").click();
  await expect(page.locator(".conversation-title")).toHaveText(title);
  await assertNoOverflow(page);
  await menu.click();
  await page.getByRole("button", { name: "归档", exact: true }).last().click();
  await page.locator(".thread-list .segmented").getByRole("button", { name: "归档", exact: true }).click();
  await page.locator(".thread-item").first().click();
  await menu.click();
  await page.getByRole("button", { name: "取消归档", exact: true }).click();
  await expect.poll(() => calls.filter(name => name === "threads.restore").length).toBe(1);
  expect(calls.filter(name => name === "threads.archive")).toHaveLength(1);
  expect(calls.filter(name => name === "threads.rename")).toHaveLength(1);
});

test("Grok rename and guarded deletion use a scoped preview with Escape and focus return", async ({ page }) => {
  const calls = await mockApi(page);
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Grok Build" }).click();
  await page.locator(".provider-session").click();
  const menu = page.getByLabel("Grok 任务操作");
  await menu.click();
  await page.getByRole("button", { name: "改名", exact: true }).click();
  await page.getByLabel("Grok 任务名称").fill("Renamed fixture");
  await page.getByTitle("保存名称").click();
  await expect(page.locator(".conversation-title")).toHaveText("Renamed fixture");
  await menu.click();
  await page.getByRole("button", { name: "删除任务文件", exact: true }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  expect(calls).not.toContain("grok.deleteExecute");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(menu).toBeFocused();
  await menu.click();
  await page.getByRole("button", { name: "删除任务文件", exact: true }).click();
  await page.getByRole("button", { name: "确认删除任务文件", exact: true }).click();
  await expect(page.getByText("暂无 Grok 任务")).toBeVisible();
  expect(calls.filter(name => name === "grok.deleteExecute")).toHaveLength(1);
});

test("hidden histories stop polling while started jobs continue to terminal", async ({ page }, info) => {
  const calls = await mockApi(page);
  await page.clock.install();
  await page.goto("/");
  const nav = page.locator(".side-nav");
  await nav.getByRole("button", { name: "设置", exact: true }).click();
  await expect(page.getByText("系统状态", { exact: true })).toBeVisible();
  expect(calls).not.toContain("jobs.list");
  await page.locator(".execution-history > summary").click();
  await expect(page.locator(".job-item").first()).toBeVisible();
  await page.locator(".execution-history > summary").click();
  const before = calls.filter(name => name === "jobs.list").length;
  await page.clock.fastForward(16000);
  expect(calls.filter(name => name === "jobs.list")).toHaveLength(before);
  await page.getByRole("tab", { name: "维护", exact: true }).click();
  await expect(page.getByText("Codex 日志库维护", { exact: true })).toBeVisible();
  const maintenance = calls.length;
  await page.clock.fastForward(31000);
  expect(calls.slice(maintenance).filter(name => ["probe.events", "probe.settings.get", "updates.status", "jobs.list", "system.status"].includes(name))).toEqual([]);
  await nav.getByRole("button", { name: "Probe", exact: true }).click();
  await page.locator(".probe-layout .segmented").getByRole("button", { name: "通知配置" }).click();
  await page.getByLabel("Device Key", { exact: true }).fill("isolated-fixture-key");
  await page.getByRole("button", { name: "保存", exact: true }).click();
  await expect(page.getByRole("button", { name: "测试推送", exact: true })).toBeEnabled();
  await page.getByRole("button", { name: "测试推送", exact: true }).click();
  await expect.poll(() => calls.filter(name => name === "jobs.detail").length).toBe(1);
  await nav.getByRole("button", { name: "Codex", exact: true }).click();
  for (let i = 2; i <= 3; i++) {
    await page.clock.fastForward(2100);
    await expect.poll(() => calls.filter(name => name === "jobs.detail").length).toBe(i);
  }
  await page.clock.fastForward(10000);
  expect(calls.filter(name => name === "jobs.detail")).toHaveLength(3);
  await info.attach("request-counts.json", { body: JSON.stringify(calls), contentType: "application/json" });
});
