import { expect, test } from "@playwright/test";
import { mockApi } from "./fixtures";
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
      await page.route(`**/${provider === "codex" ? "threads" : provider}.detail`, route => route.fulfill({ json: provider === "codex" ? detail : { summary: {}, events } }));
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
  await page.route("**/threads.detail", route => route.fulfill({ json: detail }));
  await page.route("**/threads.blocks", route => route.fulfill({ json: { thread_id: detail.summary.id, blocks: tools.slice(0, 45), total_blocks: 91, has_more_blocks: false, before_cursor: null } }));
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
