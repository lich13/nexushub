import { expect, test } from "@playwright/test";
import { mockApi, mockCommand } from "./fixtures";
import * as demo from "../src/lib/api/demo";
import type { ClaudeSessionSummary } from "../src/types";

const visibleReply = "## 示例答复\n\n保留 **正文** 和 `example`。";
const sourceReply = `${visibleReply}<oai-mem-citation>fixture internal metadata</oai-mem-citation>`;

for (const provider of ["codex", "claude", "grok"] as const) {
  test(`${provider} assistant copy keeps cleaned Markdown after duplicate labels are removed`, async ({ page }) => {
    await page.addInitScript(() => {
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: { writeText: async (value: string) => { document.documentElement.dataset.copiedReply = value; } }
      });
    });
    await mockApi(page);
    if (provider === "codex") {
      await mockCommand(page, "threads.detail", () => ({
        ...demo.demoThreadDetail("019e95a0-demo"), has_more_blocks: false,
        blocks: [{ id: "assistant-fixture", role: "assistant", kind: "message", text: sourceReply, questions: [] }]
      }));
    } else if (provider === "claude") {
      const summary: ClaudeSessionSummary = {
        id: "claude-fixture", sessionKey: "claude:fixture", title: "Claude 示例", cwd: "/fixture", path: "/fixture/example.jsonl",
        status: "recent", formatVersion: "2.1.284", messageCount: 1, canRename: true, canDelete: true
      };
      await mockCommand(page, "claude.list", () => [summary]);
      await mockCommand(page, "claude.detail", () => ({ summary, events: [{ id: "assistant-fixture", kind: "assistant_message", text: sourceReply }], totalEvents: 1, hasMore: false }));
    } else {
      await mockCommand(page, "grok.detail", () => ({ summary: {}, events: [{ kind: "agent_message_chunk", text: sourceReply }] }));
    }
    await page.goto("/");
    if (provider === "codex") await page.locator(".thread-item").filter({ hasText: "Plan Mode 修复" }).click();
    else await page.locator(".side-nav").getByRole("button", { name: provider === "claude" ? "Claude Code" : "Grok Build", exact: true }).click();
    const reply = page.locator(provider === "codex" ? ".message-stream article" : ".provider-events article");
    await expect(reply).toHaveCount(1);
    await expect(reply.locator("h2")).toHaveText("示例答复");
    await expect(reply.locator("strong")).toHaveText("正文");
    await expect(reply.locator(".chat-meta")).toHaveCount(0);
    await expect(reply).not.toContainText("fixture internal metadata");
    await reply.getByRole("button", { name: "复制回复", exact: true }).click();
    await expect(page.locator("html")).toHaveAttribute("data-copied-reply", visibleReply);
    await expect(reply.getByRole("button", { name: "已复制回复", exact: true })).toBeVisible();
  });
}
