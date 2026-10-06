import { expect, test } from "@playwright/test";
import { demoThreads } from "../src/lib/api/demo";
import type { ClaudeSessionSummary } from "../src/types";
import { assertNoOverflow, mockApi, mockCommand } from "./fixtures";

const claudeSummary: ClaudeSessionSummary = {
  id: "claude-menu-fixture",
  sessionKey: "claude:menu-fixture",
  title: "Claude menu fixture",
  cwd: "/isolated/claude-workspace",
  path: "/isolated/claude-sessions/menu-fixture.jsonl",
  status: "recent",
  formatVersion: "2.1.284",
  messageCount: 2,
  canRename: true,
  canDelete: true,
  updatedAt: "2026-10-01T00:00:00Z"
};

test("Codex context menu preserves selection, supports Shift+F10 and returns focus on Escape", async ({ page }) => {
  await mockApi(page);
  await mockCommand(page, "claude.list", () => [claudeSummary]);
  await page.setViewportSize({ width: 1280, height: 820 });
  await page.goto("/");

  const rows = page.locator(".thread-item");
  const selected = rows.first();
  const target = rows.nth(1);
  await selected.click();
  await expect(selected).toHaveClass(/\bselected\b/);
  const selectedTitle = await selected.locator(".thread-item-title").innerText();
  await expect(page.locator(".conversation-title")).toHaveText(selectedTitle);

  const menu = page.locator(".context-task-menu");
  await target.click({ button: "right" });
  await expect(menu).toBeVisible();
  await expect(selected).toHaveClass(/\bselected\b/);
  await expect(target).not.toHaveClass(/\bselected\b/);
  await expect(page.locator(".conversation-title")).toHaveText(selectedTitle);
  await expect(menu.getByRole("menuitem", { name: "改名", exact: true })).toBeFocused();

  await page.keyboard.press("Escape");
  await expect(menu).toHaveCount(0);
  await expect(target).toBeFocused();

  await target.focus();
  await page.keyboard.press("Shift+F10");
  await expect(menu).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "改名", exact: true })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(target).toBeFocused();

  await target.click({ button: "right" });
  await expect(menu).toBeVisible();
  await page.locator(".thread-title-row strong").click();
  await expect(menu).toHaveCount(0);

  const shortRow = page.locator(".renameable-session").nth(1);
  await shortRow.hover();
  await expect(shortRow.locator(".session-row-menu")).toHaveCSS("opacity", "1");
});

test("Codex row menu stays reachable after renaming a selected thread to a long title", async ({ page }) => {
  await mockApi(page);
  await page.setViewportSize({ width: 1280, height: 820 });
  await page.goto("/");

  const wrapper = page.locator(".renameable-session").nth(1);
  const row = wrapper.locator(".thread-item");
  await row.hover();
  await expect(wrapper.locator(".session-row-menu")).toHaveCSS("opacity", "1");
  await wrapper.locator(".session-row-menu").getByRole("button", { name: "线程操作" }).click();
  await page.locator(".context-task-menu").getByRole("menuitem", { name: "改名", exact: true }).click();

  const title = "长任务标题".repeat(28);
  const input = page.getByRole("textbox", { name: "线程名称", exact: true });
  await input.fill(title);
  await input.press("Enter");
  await expect(row.locator(".thread-item-title")).toHaveText(title);
  await row.click();
  await expect(row).toHaveClass(/\bselected\b/);
  await expect(page.locator(".conversation-title")).toHaveText(title);
  await row.hover();
  await expect(wrapper.locator(".session-row-menu")).toHaveCSS("opacity", "1");
  await assertNoOverflow(page);

  await page.setViewportSize({ width: 390, height: 740 });
  await page.getByRole("button", { name: "返回任务列表" }).click();
  await row.scrollIntoViewIfNeeded();
  await assertNoOverflow(page);
});

test("Codex context menu stays inside the viewport when opened at the lower edge", async ({ page }) => {
  await mockApi(page);
  const sourceRows = demoThreads("all", "").filter(thread => thread.status !== "Archived");
  const rows = Array.from({ length: 24 }, (_, index) => ({
    ...sourceRows[index % sourceRows.length],
    id: `codex-menu-${index}`,
    title: `Codex menu fixture ${index}`
  }));
  await mockCommand(page, "threads.list", () => rows);
  await page.setViewportSize({ width: 390, height: 360 });
  await page.goto("/");

  const target = page.locator(".thread-item").last();
  await expect(target).toBeVisible();
  await target.scrollIntoViewIfNeeded();
  const bounds = await target.boundingBox();
  expect(bounds).not.toBeNull();
  await target.click({
    button: "right",
    position: { x: Math.max(1, bounds!.width - 3), y: Math.max(1, bounds!.height - 3) }
  });

  const menu = page.locator(".context-task-menu");
  await expect(menu).toBeVisible();
  const rect = await menu.evaluate(element => {
    const box = element.getBoundingClientRect();
    return { left: box.left, top: box.top, right: box.right, bottom: box.bottom };
  });
  expect(rect.left).toBeGreaterThanOrEqual(8);
  expect(rect.top).toBeGreaterThanOrEqual(8);
  expect(rect.right).toBeLessThanOrEqual(382);
  expect(rect.bottom).toBeLessThanOrEqual(352);
});

test("Grok and Claude context menus expose their provider actions", async ({ page }) => {
  await mockApi(page);
  await mockCommand(page, "claude.list", () => [claudeSummary]);
  await page.setViewportSize({ width: 1280, height: 820 });
  await page.goto("/");

  const menu = page.locator(".context-task-menu");
  await page.locator(".side-nav").getByRole("button", { name: "Grok Build", exact: true }).click();
  await page.locator(".provider-session").click({ button: "right" });
  await expect(menu.getByRole("menuitem", { name: "改名", exact: true })).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "复制线程 ID", exact: true })).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "删除任务文件", exact: true })).toBeVisible();
  await page.keyboard.press("Escape");

  await page.locator(".side-nav").getByRole("button", { name: "Claude Code", exact: true }).click();
  await page.locator(".provider-session").click({ button: "right" });
  await expect(menu.getByRole("menuitem", { name: "改名", exact: true })).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "复制线程 ID", exact: true })).toBeVisible();
  await expect(menu.getByRole("menuitem", { name: "删除任务文件", exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
});

test("Codex ordinary thread deletion shows an explicit preview before any execution", async ({ page }) => {
  await mockApi(page);
  const previews: Array<{ provider: string; operation: string; sessionKeys: string[] }> = [];
  let executions = 0;
  await mockCommand(page, "sessions.bulkPreview", args => {
    const request = args.request;
    previews.push(request);
    return {
      provider: request.provider,
      operation: request.operation,
      items: request.sessionKeys.map((sessionKey: string) => ({
        sessionKey,
        id: sessionKey,
        title: "Codex preview fixture",
        paths: ["/isolated/codex/threads/fixture.jsonl"],
        bytes: 64,
        allowed: true,
        reason: null,
        fingerprint: "fixture-fingerprint"
      }))
    };
  });
  await mockCommand(page, "sessions.bulkExecute", () => { executions++; return { items: [] }; });
  await page.setViewportSize({ width: 1280, height: 820 });
  await page.goto("/");

  const first = page.locator(".thread-item").first();
  await first.click();
  await expect(page.locator(".conversation-title")).toBeVisible();
  await page.getByLabel("任务操作", { exact: true }).click();
  await page.getByRole("button", { name: "永久删除", exact: true }).click();

  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("heading", { name: "删除所选线程", exact: true })).toBeVisible();
  await expect(dialog).toContainText("Codex preview fixture");
  await expect(dialog.locator(".delete-path")).toHaveText("/isolated/codex/threads/fixture.jsonl");
  expect(previews).toEqual([{
    provider: "codex",
    operation: "delete",
    sessionKeys: ["019e8c1f-demo"]
  }]);
  expect(executions).toBe(0);

  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  expect(executions).toBe(0);
});


for (const theme of ["light", "dark"] as const) {
  for (const width of [1280, 390]) {
    test(`Grok cards fill equal widths with short, long and empty titles: ${theme} ${width}`, async ({ page }) => {
      await mockApi(page);
      await mockCommand(page, "grok.list", () => ["Short", "Long fixture title ".repeat(12), ""].map((title, index) => ({
        id: `grok-width-${index}`, title, cwd: "/isolated/workspace", path: `/isolated/sessions/${index}`,
        messageCount: 2, status: index === 1 ? "running" : "recent"
      })));
      await page.setViewportSize({ width, height: 820 });
      await page.emulateMedia({ colorScheme: theme, reducedMotion: "reduce" });
      await page.goto("/");
      await page.locator(width < 768 ? ".mobile-tabs" : ".side-nav").getByRole("button", { name: "Grok Build", exact: true }).click();
      const cards = page.locator(".provider-session");
      await expect(cards).toHaveCount(3);
      const widths = await cards.evaluateAll(elements => elements.map(element => element.getBoundingClientRect().width));
      expect(Math.max(...widths) - Math.min(...widths)).toBeLessThan(1);
      await expect(page.getByLabel("运行中", { exact: true })).toBeVisible();
      await page.getByRole("button", { name: "多选线程", exact: true }).click();
      await expect(page.locator(".session-checkbox")).toHaveCount(3);
      const batchWidths = await cards.evaluateAll(elements => elements.map(element => element.getBoundingClientRect().width));
      expect(Math.max(...batchWidths) - Math.min(...batchWidths)).toBeLessThan(1);
      await cards.first().click({ button: "right" });
      await expect(page.locator(".context-task-menu")).toHaveCount(0);
      await assertNoOverflow(page);
    });
  }
}
