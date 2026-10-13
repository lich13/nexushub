import { expect, test } from "@playwright/test";
import { mockApi, mockCommand } from "./fixtures";

type SearchRequest = {
  provider: "codex" | "claude_code" | "grok";
  scope: "thread" | "provider";
  sessionKey?: string | null;
  query: string;
};

test("global search respects thread and provider scopes and locates loaded history", async ({ page }) => {
  await mockApi(page);
  const requests: SearchRequest[] = [];
  await mockCommand(page, "sessions.search", args => {
    const request = args.request as SearchRequest;
    requests.push(request);
    const threadScope = request.scope === "thread";
    const positionKey = threadScope ? "history-45" : "history-12";
    return {
      provider: request.provider,
      scope: request.scope,
      query: request.query,
      results: [{
        resultId: `fixture-${request.scope}`,
        sessionKey: threadScope ? "019e95a0-demo" : "019e8c1f-demo",
        nativeId: positionKey,
        title: threadScope ? "Plan Mode 修复" : "活动库审阅链路",
        matchKind: "assistant_message",
        positionKey,
        snippet: `Search result fixture for ${request.scope}`,
        timestamp: null
      }],
      truncated: false
    };
  });

  await page.goto("/");
  await page.locator(".thread-item").filter({ hasText: "Plan Mode 修复" }).click();
  const openSearch = () => page.getByTitle("搜索会话（⌘F / Ctrl+F）").click();
  const dialog = page.getByRole("dialog", { name: "搜索会话" });
  const input = () => dialog.getByPlaceholder("搜索 Codex");

  await openSearch();
  await input().fill("thread-anchor-fixture");
  await expect.poll(() => requests.at(-1)?.query).toBe("thread-anchor-fixture");
  expect(requests.at(-1)).toMatchObject({
    provider: "codex", scope: "thread", sessionKey: "019e95a0-demo", query: "thread-anchor-fixture"
  });
  await dialog.getByRole("option").click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator(".timeline-target-highlight")).toContainText("历史回复 46");

  await openSearch();
  await dialog.getByRole("tab", { name: "当前 Provider" }).click();
  await input().fill("provider-anchor-fixture");
  await expect.poll(() => requests.at(-1)?.query).toBe("provider-anchor-fixture");
  expect(requests.at(-1)).toMatchObject({
    provider: "codex", scope: "provider", sessionKey: null, query: "provider-anchor-fixture"
  });
  await dialog.getByRole("option").click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator(".conversation-title")).toHaveText("活动库审阅链路");
  await expect(page.locator(".timeline-target-highlight")).toContainText("历史请求 13");
});
