import { expect, test } from "@playwright/test";
import { mockApi, mockCommand } from "./fixtures";
import * as demo from "../src/lib/api/demo";
import type { MessageBlock } from "../src/types";

for (const provider of ["codex", "grok", "pi"] as const) {
  for (const mobile of [false, true]) {
    test(`${provider} historical activity stays between replies and remembers choices (${mobile ? "mobile" : "desktop"})`, async ({ page }) => {
      await page.setViewportSize(mobile ? { width: 390, height: 844 } : { width: 1280, height: 850 });
      await page.emulateMedia({ colorScheme: mobile ? "dark" : "light", reducedMotion: "reduce" });
      await mockApi(page);
      const events = [
        { kind: provider === "grok" ? "agent_message_chunk" : "assistant_message", text: "Before activity" },
        ...Array.from({ length: 90 }, (_, n) => ({ kind: "tool_call", callId: `cmd-${n}`, role: "bash", method: "exec_command", text: "exec_command", detail: JSON.stringify({ cmd: `printf example-${n}` }), status: "completed" })),
        { kind: provider === "grok" ? "agent_message_chunk" : "assistant_message", text: "Between activity" },
        { kind: "tool_call", callId: "read-last", role: "read", method: "read", text: "Read guide.md", detail: "Guide contents", status: "completed" },
        { kind: provider === "grok" ? "agent_message_chunk" : "assistant_message", text: "After activity" }
      ];
      const blocks: MessageBlock[] = events.map((event, index) => ({ id: event.callId ?? `text-${index}`, role: event.callId ? "tool" : "assistant", kind: event.callId ? "function_call_output" : "message", tool_name: event.method, call_id: event.callId, status: event.status, input: event.detail, text: event.callId ? "Output verified" : event.text, questions: [] }));
      const detail = demo.demoThreadDetail("019e95a0-demo");
      detail.blocks = blocks;
      detail.total_blocks = blocks.length;
      await mockCommand(page, `${provider === "codex" ? "threads" : provider}.detail`, args => provider === "codex" ? detail : { summary: {}, events });
      await page.goto("/");
      if (provider === "codex") await page.locator(".thread-item").filter({ hasText: "Plan Mode 修复" }).click();
      else {
        await page.locator(mobile ? ".mobile-tabs" : ".side-nav").getByRole("button", { name: provider === "grok" ? "Grok Build" : "Pi", exact: true }).click();
        if (mobile) await page.locator(".provider-session").click();
      }
      const stream = page.locator(provider === "codex" ? ".message-stream" : ".provider-events");
      const groups = stream.locator("details.execution-group");
      await expect(groups).toHaveCount(2);
      await expect(groups.first().locator(":scope > summary")).toContainText("90 条命令");
      const order = await stream.locator(":scope > article, :scope > details").evaluateAll(nodes => nodes.map(node => node.matches("details") ? "tools" : node.textContent?.includes("Before activity") ? "before" : node.textContent?.includes("Between activity") ? "between" : "after"));
      expect(order).toEqual(["before", "tools", "between", "tools", "after"]);
      await groups.first().locator(":scope > summary").focus();
      await page.keyboard.press("Enter");
      const first = groups.first().locator("details.execution-command").first();
      await expect(groups.first().locator("details.execution-command")).toHaveCount(90);
      await expect(first.locator("summary")).toContainText("printf example-0");
      await first.locator("summary").click();
      await expect(first).toHaveAttribute("open", "");
      if (provider !== "codex") {
        if (mobile) await page.getByTitle("返回任务列表").click();
        await page.getByTitle("刷新任务").click();
        if (mobile) await page.locator(".provider-session").click();
      }
      await page.emulateMedia({ colorScheme: mobile ? "light" : "dark" });
      await expect(first).toHaveAttribute("open", "");
      await groups.first().locator(":scope > summary").click();
      await groups.first().locator(":scope > summary").click();
      await expect(first).toHaveAttribute("open", "");
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollHeight - innerHeight)).toBeLessThanOrEqual(1);
      const rows = groups.first().locator(".execution-rows");
      expect(await rows.evaluate(node => node.scrollHeight > node.clientHeight)).toBe(true);
      await stream.evaluate(node => { node.scrollTop = node.scrollHeight; });
      expect(await page.evaluate(() => window.scrollY)).toBe(0);
    });
  }
}

test("Codex earlier-page loading preserves all tool rows and the open group", async ({ page }) => {
  await mockApi(page);
  const tools: MessageBlock[] = Array.from({ length: 90 }, (_, index) => ({ id: `history-${index}`, call_id: `history-${index}`, role: "tool", kind: "function_call_output", tool_name: "exec_command", status: "completed", input: JSON.stringify({ cmd: `printf earlier-${index}` }), text: `output-${index}`, questions: [] }));
  const detail = demo.demoThreadDetail("019e95a0-demo");
  detail.blocks = [...tools.slice(45), { id: "last", role: "assistant", kind: "message", text: "Latest reply", questions: [] }];
  detail.has_more_blocks = true;
  detail.before_cursor = "b:45";
  detail.total_blocks = 91;
  await mockCommand(page, "threads.detail", args => detail);
  await mockCommand(page, "threads.blocks", args => ({ threadId: detail.summary.id, blocks: tools.slice(0, 45), totalBlocks: 91, hasMoreBlocks: false, beforeCursor: null }));
  await page.goto("/");
  await page.locator(".thread-item").filter({ hasText: "Plan Mode 修复" }).click();
  const group = page.locator("details.execution-group");
  await group.locator(":scope > summary").click();
  const retained = page.locator("details.execution-command").filter({ hasText: "printf earlier-45" });
  await retained.locator("summary").click();
  await expect(retained).toHaveAttribute("open", "");
  await page.getByRole("button", { name: "较早消息", exact: true }).click();
  await expect(group).toContainText("90 条命令");
  await expect(group).toHaveAttribute("open", "");
  await expect(retained).toHaveAttribute("open", "");
  await expect(group.locator("details.execution-command")).toHaveCount(90);
  await expect(page.getByRole("button", { name: "较早消息", exact: true })).toHaveCount(0);
  expect(await page.locator(".message-stream").evaluate(element => element.scrollTop >= 0 && element.scrollTop <= element.scrollHeight - element.clientHeight + 1)).toBe(true);
});

test("Pi standalone bash exposes the original command and its output", async ({ page }) => {
  await mockApi(page);
  await mockCommand(page, "pi.detail", args => ({ summary: {}, events: [
    { kind: "assistant_message", text: "Before command" },
    { kind: "tool_result", role: "bashExecution", text: "printf standalone", detail: "standalone result", status: "completed" },
    { kind: "assistant_message", text: "After command" }
  ] }));
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Pi", exact: true }).click();
  await page.locator(".execution-group > summary").click();
  const command = page.locator(".execution-command");
  await expect(command.locator("summary")).toContainText("printf standalone");
  await command.locator("summary").click();
  await expect(command.locator(".execution-section")).toHaveText(["命令printf standalone", "结果standalone result"]);
});

test("native pagination inserts older activity while preserving the visible anchor", async ({ page }) => {
  await mockApi(page);
  const blocks: MessageBlock[] = Array.from({ length: 40 }, (_, index) => [
    { id: `reply-${index}`, role: "assistant", kind: "message", text: `Timeline reply ${index}`, questions: [] },
    { id: `call-${index}`, role: "tool", kind: "function_call_output", tool_name: "exec_command", status: "completed", input: `printf example-${index}`, text: "OK", questions: [] }
  ]).flat();
  const detail = { ...demo.demoThreadDetail("019e95a0-demo"), blocks: blocks.slice(40), total_blocks: 80, has_more_blocks: true, before_cursor: "b:40" };
  await mockCommand(page, "threads.detail", args => detail);
  await mockCommand(page, "threads.blocks", args => ({ threadId: detail.summary.id, blocks: blocks.slice(0, 40), totalBlocks: 80, hasMoreBlocks: false, beforeCursor: null }));
  await page.goto("/");
  await page.locator(".thread-item").filter({ hasText: "Plan Mode 修复" }).click();
  const stream = page.locator(".message-stream");
  await stream.evaluate(node => { node.scrollTop = 0; });
  const anchor = page.getByText("Timeline reply 20", { exact: true });
  const before = await anchor.evaluate(node => node.getBoundingClientRect().top);
  await page.getByRole("button", { name: "较早消息", exact: true }).click();
  await expect(stream.locator(".execution-group")).toHaveCount(40);
  expect(Math.abs(await anchor.evaluate(node => node.getBoundingClientRect().top) - before)).toBeLessThanOrEqual(2);
  await page.getByTitle("刷新任务").click();
  await expect(stream.locator(".execution-group")).toHaveCount(40);
  await expect(page.getByRole("button", { name: "较早消息", exact: true })).toHaveCount(0);
});

for (const provider of ["codex", "claude", "grok", "pi"] as const) {
  test(`${provider} rail marks user instructions and keeps non-user search anchors`, async ({ page }) => {
    await mockApi(page);
    await page.emulateMedia({ reducedMotion: "reduce", colorScheme: provider === "grok" ? "light" : "dark" });
    const kind = provider === "grok" ? "user_message_chunk" : "user_message";
    const assistant = provider === "grok" ? "agent_message_chunk" : "assistant_message";
    const events = [
      { id: "first", kind, text: "First fixture instruction" },
      { id: "answer", kind: assistant, text: "Long fixture reply\n\n".repeat(90) },
      { id: "call", kind: "tool_call", callId: "fixture-call", method: "exec_command", role: "bash", text: "exec_command", detail: "printf fixture", status: "completed" },
      { id: "second", kind, text: "Second fixture instruction" },
      { id: "last", kind: assistant, text: "Final fixture reply" }
    ];
    const summary = { id: "fixture-thread", sessionKey: "fixture-key", title: "Fixture thread", cwd: "/fixture", path: "/fixture/native.jsonl", status: "recent", formatVersion: "2.1.284", messageCount: 4, canRename: true, canDelete: true };
    if (provider === "codex") {
      const detail = demo.demoThreadDetail("019e95a0-demo");
      detail.blocks = events.map(event => ({ id: event.id, role: event.kind === kind ? "user" : event.kind === "tool_call" ? "tool" : "assistant", kind: event.kind === "tool_call" ? "function_call_output" : "message", text: event.text, input: event.detail, call_id: event.callId, tool_name: event.method, status: event.status, questions: [] }));
      detail.has_more_blocks = false;
      await mockCommand(page, "threads.detail", () => detail);
    } else {
      await mockCommand(page, `${provider}.list`, () => [summary]);
      await mockCommand(page, `${provider}.detail`, () => ({ summary, events, totalEvents: events.length, hasMore: false }));
    }
    await page.goto("/");
    if (provider === "codex") await page.locator(".thread-item").filter({ hasText: "Plan Mode 修复" }).click();
    else await page.locator(".side-nav").getByRole("button", { name: { claude: "Claude Code", grok: "Grok Build", pi: "Pi" }[provider], exact: true }).click();
    const markers = page.locator(".timeline-rail-item");
    await expect(markers).toHaveCount(2);
    await expect(markers.nth(0)).toHaveAccessibleName("时间线：First fixture instruction");
    await markers.nth(0).focus(); await page.keyboard.press("Enter");
    await expect(markers.nth(0)).toHaveClass(/active/);
    const stream = page.locator(provider === "codex" ? ".message-stream" : ".provider-events");
    await stream.evaluate(node => { node.scrollTop += 500; });
    await expect(markers.nth(0)).toHaveClass(/active/);
    await stream.evaluate(node => { node.scrollTop = node.scrollHeight; });
    await expect(markers.nth(1)).toHaveClass(/active/);
    await stream.evaluate(node => { node.scrollTop = 0; });
    await expect(markers.nth(0)).toHaveClass(/active/);
    await expect(stream.locator("[data-timeline-id]")).toHaveCount(5);
    await expect(stream.locator(".execution-group")).toHaveCount(1);
    await page.getByTitle("刷新任务").click();
    await expect(markers).toHaveCount(2);
    await expect.poll(() => page.evaluate(() => document.documentElement.scrollHeight - innerHeight)).toBeLessThanOrEqual(1);
  });
}
