import { expect, test } from "@playwright/test";
import { assertContrast, assertNoOverflow, mockApi } from "./fixtures";

test("Probe saves Bark and restricted recovery independently and retains drafts on failure", async ({ page }) => {
  await mockApi(page);
  const saved: Record<string, any>[] = [];
  page.on("request", request => { if (request.url().endsWith("/probe.settings.save")) saved.push(request.postDataJSON().settings); });
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Probe", exact: true }).click();
  await page.getByRole("button", { name: "通知配置", exact: true }).click();
  await page.getByLabel("启用 Bark", { exact: true }).uncheck();
  await page.getByLabel("受限 Goal 自动恢复", { exact: true }).check();
  await page.getByRole("button", { name: "保存设置", exact: true }).click();
  await expect.poll(() => saved.length).toBe(1);
  expect(saved[0].probe.notifications.enabled).toBe(false);
  expect(saved[0].probe.error_monitor.auto_resume_goals).toBe(true);
  await expect(page.getByRole("button", { name: "保存设置", exact: true })).toBeEnabled();
  await page.getByLabel("启用 Bark", { exact: true }).check();
  await page.getByLabel("受限 Goal 自动恢复", { exact: true }).uncheck();
  await page.getByRole("button", { name: "保存设置", exact: true }).click();
  await expect.poll(() => saved.length).toBe(2);
  expect(saved[1].probe.notifications.enabled).toBe(true);
  expect(saved[1].probe.error_monitor.auto_resume_goals).toBe(false);
  await expect(page.getByRole("button", { name: "保存设置", exact: true })).toBeEnabled();
  await page.route("**/probe.settings.save", route => route.fulfill({ status: 500, json: { error: "Fixture save failed" } }));
  await page.getByLabel("主机标签", { exact: true }).fill("retained-draft");
  await page.getByRole("button", { name: "保存设置", exact: true }).click();
  await expect(page.locator(".form-error").first()).toContainText("Fixture save failed");
  await expect(page.getByLabel("主机标签", { exact: true })).toHaveValue("retained-draft");
  await assertContrast(page, ".form-error");
});

test("archive and hidden cleanup require previews, explicit confirmation, and visible failure", async ({ page }) => {
  const calls = await mockApi(page);
  const executed: Record<string, number>[] = [];
  await page.route("**/cleanup.archiveExecute", route => {
    executed.push(route.request().postDataJSON());
    return route.fulfill({ status: 409, json: { error: "Fixture count changed; run a new preview" } });
  });
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "设置", exact: true }).click();
  await page.getByRole("tab", { name: "维护", exact: true }).click();
  const archive = page.locator(".panel").filter({ has: page.getByText("归档线程清理", { exact: true }) });
  await expect(archive.getByRole("button", { name: "清理归档", exact: true })).toBeDisabled();
  await archive.getByRole("button", { name: "Dry-run", exact: true }).click();
  await archive.getByRole("button", { name: "清理归档", exact: true }).click();
  expect(executed).toHaveLength(0);
  await archive.getByRole("button", { name: "取消", exact: true }).click();
  expect(executed).toHaveLength(0);
  await archive.getByRole("button", { name: "清理归档", exact: true }).click();
  await archive.getByRole("button", { name: "确认清理归档", exact: true }).click();
  await expect(archive.locator(".form-error")).toContainText("Fixture count changed");
  expect(executed).toHaveLength(1);
  expect(executed[0].expectedCount).toBeGreaterThan(0);
  const hidden = page.locator(".panel").filter({ has: page.getByText("隐藏线程清理", { exact: true }) });
  await expect(hidden.getByRole("button", { name: "清理隐藏线程", exact: true })).toBeDisabled();
  await hidden.getByRole("button", { name: "扫描隐藏线程", exact: true }).click();
  await hidden.getByRole("button", { name: "清理隐藏线程", exact: true }).click();
  expect(calls).not.toContain("cleanup.hiddenExecute");
  await hidden.getByRole("button", { name: "取消", exact: true }).click();
  expect(calls).not.toContain("cleanup.hiddenExecute");
  await assertContrast(page, ".form-error, .status-chip, .danger-button");
});

test("log maintenance waits for a successful dry-run and blocks duplicate running jobs", async ({ page }) => {
  const calls = await mockApi(page);
  await page.clock.install();
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "设置", exact: true }).click();
  await page.getByRole("tab", { name: "维护", exact: true }).click();
  const logs = page.locator(".panel").filter({ has: page.getByText("Codex 日志库维护", { exact: true }) });
  await expect(logs.getByRole("button", { name: "准备执行", exact: true })).toBeDisabled();
  await logs.getByRole("button", { name: "Dry-run", exact: true }).click();
  await expect(logs.getByRole("button", { name: "准备执行", exact: true })).toBeDisabled();
  await expect(logs.getByRole("button", { name: "Dry-run", exact: true })).toBeDisabled();
  await expect.poll(() => calls.filter(name => name === "jobs.detail").length).toBe(1);
  await page.clock.fastForward(2100);
  await expect.poll(() => calls.filter(name => name === "jobs.detail").length).toBe(2);
  await page.clock.fastForward(2100);
  await expect(logs.getByRole("button", { name: "准备执行", exact: true })).toBeEnabled();
  await logs.getByRole("button", { name: "准备执行", exact: true }).click();
  expect(calls).not.toContain("probe.logsDbExecute");
  await logs.getByRole("button", { name: "取消", exact: true }).click();
  expect(calls).not.toContain("probe.logsDbExecute");
});

test("copy commands use the selected task and long load errors stay readable", async ({ page }) => {
  await mockApi(page);
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", { value: { writeText: async (text: string) => { document.documentElement.dataset.copied = text; } } });
  });
  await page.goto("/");
  await page.locator(".thread-item").first().click();
  const menu = page.getByLabel("任务操作", { exact: true });
  await menu.click();
  await page.getByRole("button", { name: "复制 ID", exact: true }).click();
  const id = await page.locator("html").getAttribute("data-copied");
  expect(id).toBeTruthy();
  await expect(page.getByText("已复制", { exact: true })).toBeVisible();
  await menu.click();
  await page.getByRole("button", { name: "复制恢复命令", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-copied", `codex resume ${id}`);
  await page.setViewportSize({ width: 320, height: 844 });
  await page.route("**/grok.list", route => route.fulfill({ status: 500, json: { error: "Fixture load failure: " + "long-path/".repeat(40) } }));
  await page.locator(".mobile-tabs").getByRole("button", { name: "Grok Build", exact: true }).click();
  await expect(page.locator(".form-error").first()).toContainText("Fixture load failure");
  await assertContrast(page, ".form-error");
  await assertNoOverflow(page);
});
