import { expect, test } from "@playwright/test";
import { mockApi, mockCommand } from "./fixtures";

test("selected Grok session shows a new message within three seconds", async ({ page }) => {
  await mockApi(page);
  let text = "Initial native message";
  await mockCommand(page, "grok.list", args => [{ id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", path: "/isolated/sessions/grok-fixture", messageCount: 2, status: "running" }]);
  await mockCommand(page, "grok.detail", args => ({
    summary: { id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", status: "running" },
    events: [
      { kind: "tool_call", method: "session/update", text: "Execute `pwd`", callId: "command-1", status: "in_progress" },
      { kind: "tool_call", method: "session/update", text: "Execute `ls`", callId: "command-2", status: "completed" },
      { kind: "agent_message_chunk", text }
    ]
  }));
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Grok Build", exact: true }).click();
  await expect(page.locator(".provider-session .thread-running-indicator").first()).toBeVisible();
  const group = page.locator("details.execution-group");
  await expect(group).toContainText("2 条命令");
  await expect(group.locator("details.execution-command")).toHaveCount(2);
  await expect(group.locator("details.execution-command").first()).toHaveAttribute("open", "");
  await expect(group.locator("details.execution-command").nth(1)).not.toHaveAttribute("open");
  await expect(page.getByText(text, { exact: true })).toBeVisible();
  const appendedAt = Date.now();
  text = "Fresh native message after opening the conversation";
  await expect(page.getByText(text, { exact: true })).toBeVisible({ timeout: 3000 });
  expect(Date.now() - appendedAt).toBeLessThan(3000);
});

test("Grok follows new messages at the bottom and preserves history reading position", async ({ page }) => {
  await mockApi(page);
  const events = Array.from({ length: 30 }, (_, index) => ({ kind: "agent_message_chunk", text: `Historical message ${index}\n\n${"Readable history. ".repeat(15)}` }));
  await mockCommand(page, "grok.detail", args => ({
    summary: { id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", status: "running" }, events
  }));
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Grok Build", exact: true }).click();
  const stream = page.locator(".provider-events");
  await expect(stream.locator("article")).toHaveCount(30);
  const bottomGap = () => stream.evaluate(element => element.scrollHeight - element.clientHeight - element.scrollTop);
  await expect.poll(bottomGap).toBeLessThan(3);
  events.push({ kind: "agent_message_chunk", text: "New message at the bottom" });
  await expect(stream.locator("article")).toHaveCount(31, { timeout: 3000 });
  await expect.poll(bottomGap).toBeLessThan(3);
  await stream.evaluate(element => { element.scrollTop = 100; element.dispatchEvent(new Event("scroll", { bubbles: true })); });
  events.push({ kind: "agent_message_chunk", text: "New message while reviewing history" });
  await expect(stream.locator("article")).toHaveCount(32, { timeout: 3000 });
  await expect.poll(() => stream.evaluate(element => element.scrollTop)).toBe(100);
});

test("long mixed Grok tool runs fold together without creating an outer page scrollbar", async ({ page }) => {
  await mockApi(page);
  const events = Array.from({ length: 24 }, (_, index) => ({
    kind: "tool_call", method: "session/update", callId: `tool-${index}`, status: "completed",
    text: `${["Read", "List", "Search", "Execute"][index % 4]} fixture-${index}`,
    detail: `Result ${index}`
  }));
  await mockCommand(page, "grok.detail", args => ({
    summary: { id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", status: "running" },
    events: [{ kind: "user_message_chunk", text: "Inspect fixtures" }, ...events, { kind: "agent_message_chunk", text: "Summary" }]
  }));
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Grok Build", exact: true }).click();
  const group = page.locator(".provider-events details.execution-group");
  await expect(group).toHaveCount(1);
  await expect(group).toContainText("已使用 Grok 运行工具");
  await expect(group).toContainText("24 项工具");
  await expect(group).not.toHaveAttribute("open", "");
  await group.locator("summary").first().click();
  await expect(group.locator("details.execution-command")).toHaveCount(24);
  await expect(group).toHaveAttribute("open", "");
  await expect(group.locator("details.execution-command").first()).not.toHaveAttribute("open", "");
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollHeight - innerHeight)).toBeLessThanOrEqual(1);
  await page.locator(".provider-events").evaluate(element => { element.scrollTop = element.scrollHeight; });
  expect(await page.evaluate(() => window.scrollY)).toBe(0);
  events.push({ kind: "tool_call", method: "session/update", callId: "new-tool", status: "completed", text: "Read new-fixture", detail: "New result" });
  await expect(group.locator("details.execution-command")).toHaveCount(25, { timeout: 3000 });
  await expect(group).toHaveAttribute("open", "");
});

test("mobile dark Grok history keeps one scroll container with reduced motion", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" });
  await mockApi(page);
  await mockCommand(page, "grok.detail", args => ({
    summary: { id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", status: "recent" },
    events: [...Array.from({ length: 20 }, (_, index) => ({ kind: "tool_call", callId: `read-${index}`, text: `Read fixture-${index}`, status: "completed" })), { kind: "agent_message_chunk", text: "Result" }]
  }));
  await page.goto("/");
  await page.locator(".mobile-tabs").getByRole("button", { name: "Grok Build", exact: true }).click();
  await page.locator(".provider-session").click();
  await expect(page.locator(".execution-group")).toContainText("20 项工具");
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollHeight - innerHeight)).toBeLessThanOrEqual(1);
});

test("narrow Grok list stops hidden detail reads and reopening reads fresh messages", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const calls = await mockApi(page);
  await page.goto("/");
  await page.locator(".mobile-tabs").getByRole("button", { name: "Grok Build", exact: true }).click();
  await expect(page.locator(".provider-session")).toBeVisible();
  await page.waitForTimeout(2200);
  expect(calls.filter(call => call === "grok.detail")).toHaveLength(0);
  await page.locator(".provider-session").click();
  await expect(page.getByText("Check the isolated fixture.", { exact: true })).toBeVisible();
  await page.getByTitle("返回任务列表").click();
  const before = calls.filter(call => call === "grok.detail").length;
  await page.waitForTimeout(2200);
  expect(calls.filter(call => call === "grok.detail")).toHaveLength(before);
  await page.locator(".provider-session").click();
  await expect.poll(() => calls.filter(call => call === "grok.detail").length).toBeGreaterThan(before);
});




test("Grok spinner stops when the selected session ends and unknown activity stays explicit", async ({ page }) => {
  await mockApi(page);
  await page.clock.install();
  await page.emulateMedia({ reducedMotion: "reduce" });
  let status = "running";
  const summary = () => ({ id: "grok-fixture", title: "Grok activity fixture", cwd: "/isolated/workspace", path: "/isolated/sessions/grok-fixture", messageCount: 2, updatedAt: "2026-01-01T00:00:00Z", status });
  await mockCommand(page, "grok.list", () => [summary()]);
  await mockCommand(page, "grok.detail", () => ({ summary: summary(), events: [] }));
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Grok Build", exact: true }).click();
  const row = page.locator(".provider-session");
  await expect(row.getByLabel("运行中")).toBeVisible();
  status = "recent";
  await page.clock.fastForward(16000);
  await expect(row.getByLabel("运行中")).toHaveCount(0);
  await expect(row).not.toContainText("状态未知");
  status = "unknown";
  await page.clock.fastForward(16000);
  await expect(row).toContainText("状态未知");
  await expect(row.getByLabel("运行中")).toHaveCount(0);
});
