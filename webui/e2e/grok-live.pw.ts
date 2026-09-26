import { expect, test } from "@playwright/test";
import { mockApi } from "./fixtures";

test("selected Grok session shows a new message within three seconds", async ({ page }) => {
  await mockApi(page);
  let text = "Initial native message";
  await page.route("**/grok.list", route => route.fulfill({ json: [{ id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", path: "/isolated/sessions/grok-fixture", messageCount: 2, status: "running" }] }));
  await page.route("**/grok.detail", route => route.fulfill({ json: {
    summary: { id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", status: "running" },
    events: [
      { kind: "tool_call", method: "session/update", text: "Execute `pwd`", callId: "command-1", status: "in_progress" },
      { kind: "tool_call", method: "session/update", text: "Execute `ls`", callId: "command-2", status: "completed" },
      { kind: "agent_message_chunk", text }
    ]
  } }));
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
  await page.route("**/grok.detail", route => route.fulfill({ json: {
    summary: { id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", status: "running" }, events
  } }));
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
  await page.route("**/grok.detail", route => route.fulfill({ json: {
    summary: { id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", status: "running" },
    events: [{ kind: "user_message_chunk", text: "Inspect fixtures" }, ...events, { kind: "agent_message_chunk", text: "Summary" }]
  } }));
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Grok Build", exact: true }).click();
  const group = page.locator(".provider-events details.execution-group");
  await expect(group).toHaveCount(1);
  await expect(group).toContainText("工具活动组");
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
  await page.route("**/grok.detail", route => route.fulfill({ json: {
    summary: { id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", status: "recent" },
    events: [...Array.from({ length: 20 }, (_, index) => ({ kind: "tool_call", callId: `read-${index}`, text: `Read fixture-${index}`, status: "completed" })), { kind: "agent_message_chunk", text: "Result" }]
  } }));
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

test("selected running Pi session refreshes within three seconds", async ({ page }) => {
  await mockApi(page);
  let text = "Initial Pi message";
  await page.route("**/pi.list", route => route.fulfill({ json: [{ id: "pi-native-fixture", sessionKey: "project/session-fixture.jsonl", title: "Pi fixture", cwd: "/isolated/pi-workspace", path: "/isolated/pi-sessions/project/session-fixture.jsonl", updatedAt: "2026-09-22T08:00:00Z", messageCount: 3, lastMessage: "Current Pi branch", status: "running", formatVersion: 3, canRename: true, renameBlockReason: null, canDelete: true, deleteBlockReason: null, readError: null }] }));
  await page.route("**/pi.detail", route => route.fulfill({ json: {
    summary: { id: "pi-native-fixture", sessionKey: "project/session-fixture.jsonl", title: "Pi fixture", cwd: "/isolated/pi-workspace", status: "running" },
    events: [
      { kind: "tool_call", role: "bashExecution", text: "pwd", callId: "command-1", status: "in_progress" },
      { kind: "tool_result", role: "bashExecution", detail: "/isolated/pi-workspace", callId: "command-1", status: "completed" },
      { kind: "assistant_message", role: "assistant", text }
    ]
  } }));
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Pi", exact: true }).click();
  await expect(page.locator(".provider-session .thread-running-indicator").first()).toBeVisible();
  await expect(page.locator("details.execution-group")).toContainText("1 条命令");
  await expect(page.locator("details.execution-command")).toHaveCount(1);
  await expect(page.getByText(text, { exact: true })).toBeVisible();
  const appendedAt = Date.now();
  text = "Fresh Pi message after opening the conversation";
  await expect(page.getByText(text, { exact: true })).toBeVisible({ timeout: 3000 });
  expect(Date.now() - appendedAt).toBeLessThan(3000);
});

test("narrow Pi list stops hidden detail reads and reopens with a fresh read", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const calls = await mockApi(page);
  await page.goto("/");
  await page.locator(".mobile-tabs").getByRole("button", { name: "Pi", exact: true }).click();
  await expect(page.locator(".provider-session")).toBeVisible();
  await page.waitForTimeout(2200);
  expect(calls.filter(call => call === "pi.detail")).toHaveLength(0);
  await page.locator(".provider-session").click();
  await expect(page.getByText("Pi fixture result", { exact: true })).toBeVisible();
  await page.getByTitle("返回任务列表").click();
  const before = calls.filter(call => call === "pi.detail").length;
  await page.waitForTimeout(2200);
  expect(calls.filter(call => call === "pi.detail")).toHaveLength(before);
  await page.locator(".provider-session").click();
  await expect.poll(() => calls.filter(call => call === "pi.detail").length).toBeGreaterThan(before);
});
