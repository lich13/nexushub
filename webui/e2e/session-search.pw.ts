import { expect, test, type Page } from "@playwright/test";
import { mockApi, mockCommand } from "./fixtures";
import type {
  ClaudeHistoryEvent,
  ClaudeSessionSummary,
  GrokHistoryEvent,
  GrokSessionSummary,
  MessageBlock,
  ThreadSummary
} from "../src/types";

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

type SyntheticSearchOptions = {
  title: string;
  blocks: MessageBlock[];
  olderBlocks?: MessageBlock[];
  positionKey: string;
  matchKind: string;
  alternateResults?: Record<string, { positionKey: string; matchKind: string }>;
};

async function installSyntheticSearch(page: Page, options: SyntheticSearchOptions) {
  await mockApi(page);
  const threadId = "019e-demo-search";
  const summary: ThreadSummary = {
    id: threadId,
    title: options.title,
    status: "Recent",
    message_count: options.blocks.length + (options.olderBlocks?.length ?? 0)
  };
  const totalBlocks = summary.message_count;
  const hasOlderPage = Boolean(options.olderBlocks?.length);
  const cursor = hasOlderPage ? `b:${options.blocks.length}` : null;
  const historyRequests: Array<string | null> = [];
  const searchRequests: SearchRequest[] = [];

  await mockCommand(page, "threads.list", () => [summary]);
  await mockCommand(page, "threads.detail", () => ({
    summary,
    blocks: options.blocks,
    messages: [],
    raw_event_count: totalBlocks,
    total_blocks: totalBlocks,
    has_more_blocks: hasOlderPage,
    before_cursor: cursor
  }));
  await mockCommand(page, "threads.blocks", args => {
    historyRequests.push(args.options?.before ?? null);
    return {
      threadId,
      blocks: options.olderBlocks ?? [],
      totalBlocks,
      hasMoreBlocks: false,
      beforeCursor: null
    };
  });
  await mockCommand(page, "sessions.search", args => {
    const request = args.request as SearchRequest;
    searchRequests.push(request);
    const match = options.alternateResults?.[request.query] ?? options;
    return {
      provider: request.provider,
      scope: request.scope,
      query: request.query,
      results: [{
        resultId: `synthetic-${request.query}-${match.positionKey}`,
        sessionKey: threadId,
        nativeId: match.positionKey,
        title: options.title,
        matchKind: match.matchKind,
        positionKey: match.positionKey,
        snippet: `Synthetic match ${match.positionKey}`,
        timestamp: null
      }],
      truncated: false
    };
  });
  return { summary, historyRequests, searchRequests };
}

function planBlock(id: string, text: string): MessageBlock {
  return {
    id,
    role: "assistant",
    kind: "plan",
    display_kind: "plan",
    status: "completed",
    plan_status: "completed",
    resolved: true,
    text: `<proposed_plan>${text}</proposed_plan>`,
    questions: []
  };
}

function answeredQuestionBlock(id: string, answer: string): MessageBlock {
  return {
    id,
    role: "assistant",
    kind: "request_user_input_result",
    display_kind: "question_result",
    status: "completed",
    resolved: true,
    questions: [{ id: `${id}-question`, question: "继续吗？", options: [{ label: "继续" }] }],
    answers: [{ question_id: `${id}-question`, answers: [answer] }]
  };
}

async function openSyntheticThread(page: Page, title: string) {
  await page.goto("/");
  await page.locator(".thread-item").filter({ hasText: title }).click();
  await expect(page.locator(".conversation-title")).toHaveText(title);
}

async function searchAndSelect(page: Page, query: string, providerLabel = "Codex") {
  await page.getByTitle("搜索会话（⌘F / Ctrl+F）").click();
  const dialog = page.getByRole("dialog", { name: "搜索会话" });
  await dialog.getByPlaceholder(`搜索 ${providerLabel}`).fill(query);
  await expect(dialog.getByRole("option")).toHaveCount(1);
  await dialog.getByRole("option").click();
  await expect(dialog).toHaveCount(0);
}

test("search reveals a compacted request_user_input_result from a paged Codex history", async ({ page }) => {
  const answerId = "older-answer-fixture";
  const title = "Paged question result fixture";
  const fixture = await installSyntheticSearch(page, {
    title,
    blocks: [
      planBlock("follow-up-plan-one", "First follow-up plan"),
      {
        id: "follow-up-question-one", role: "assistant", kind: "request_user_input", display_kind: "question",
        status: "completed", resolved: true, questions: [{ id: "follow-up-question-one-choice", question: "Second follow-up question?", options: [{ label: "Continue" }] }]
      },
      planBlock("follow-up-plan-two", "Third follow-up plan"),
      answeredQuestionBlock("follow-up-question-two", "Fourth follow-up answer")
    ],
    olderBlocks: [answeredQuestionBlock(answerId, "Old answer fixture")],
    positionKey: answerId,
    matchKind: "question_result"
  });

  await openSyntheticThread(page, title);
  const stream = page.locator(".conversation-main .message-stream");
  await expect(stream.locator(".history-collapse-cell")).toContainText("历史计划/问题已折叠");
  await searchAndSelect(page, "older-answer-fixture");

  const target = stream.locator(`.question-result-cell[data-timeline-id="${answerId}"]`);
  await expect.poll(() => fixture.historyRequests.length, { timeout: 5000 }).toBe(1);
  await expect.soft(target).toBeVisible({ timeout: 4000 });
  await expect.soft(target).toContainText("Old answer fixture", { timeout: 4000 });
  await expect.soft(target).toHaveClass(/timeline-target-highlight/, { timeout: 4000 });
  await expect.soft(page.getByRole("status").filter({ hasText: "结果所在历史未加载，请刷新后重试" })).toHaveCount(0);
  expect(fixture.historyRequests).toEqual(["b:4"]);
});

test("search opens the closed Codex tool result and preserves a manual disclosure choice", async ({ page }) => {
  const title = "Closed tool result fixture";
  const resultId = "matched-tool-result-fixture";
  const fixture = await installSyntheticSearch(page, {
    title,
    blocks: [
      {
        id: "matched-tool-call-fixture", role: "tool", kind: "function_call", tool_name: "exec_command",
        call_id: "matched-tool-call-id", turn_id: "matched-tool-turn", input: "printf matched-output-fixture",
        status: "running", questions: []
      },
      {
        id: resultId, role: "tool", kind: "function_call_output", tool_name: "exec_command",
        call_id: "matched-tool-call-id", turn_id: "matched-tool-turn", text: "Matched tool output fixture",
        status: "completed", questions: []
      },
      {
        id: "manual-collapse-call-fixture", role: "tool", kind: "function_call", tool_name: "exec_command",
        call_id: "manual-collapse-call-id", turn_id: "manual-collapse-turn", input: "printf manual-collapse-fixture",
        status: "running", questions: []
      }
    ],
    positionKey: resultId,
    matchKind: "tool_result",
    alternateResults: {
      "missing-result-fixture": { positionKey: "missing-result-fixture", matchKind: "assistant_message" }
    }
  });

  await openSyntheticThread(page, title);
  const stream = page.locator(".conversation-main .message-stream");
  const targetGroup = stream.locator('.execution-group[data-timeline-id*="matched-tool-call-id"]');
  const manuallyCollapsedGroup = stream.locator('.execution-group[data-timeline-id*="manual-collapse-call-id"]');
  await expect(targetGroup).not.toHaveAttribute("open", "");
  await expect(manuallyCollapsedGroup).toHaveAttribute("open", "");
  await manuallyCollapsedGroup.locator(":scope > summary").click();
  await expect(manuallyCollapsedGroup).not.toHaveAttribute("open", "");

  await searchAndSelect(page, "missing-result-fixture");
  const missFeedback = page.getByRole("status").filter({ hasText: "结果所在历史未加载，请刷新后重试" });
  await expect(missFeedback).toBeVisible();

  await searchAndSelect(page, "matched-tool-result-fixture");

  const targetCommand = stream.locator(`.execution-command[data-timeline-aliases~="${resultId}"]`);
  await expect.soft(targetGroup).toHaveAttribute("open", "", { timeout: 4000 });
  await expect.soft(targetCommand).toHaveAttribute("open", "", { timeout: 4000 });
  await expect.soft(targetCommand.locator(".execution-section").last()).toContainText("Matched tool output fixture", { timeout: 4000 });
  await expect.soft(targetCommand).toHaveClass(/timeline-target-highlight/, { timeout: 4000 });
  await expect.soft(manuallyCollapsedGroup).not.toHaveAttribute("open", "");
  await expect.soft(missFeedback).toHaveCount(0);

  await targetCommand.locator(":scope > summary").click();
  await expect(targetCommand).not.toHaveAttribute("open", "");
  await targetGroup.locator(":scope > summary").click();
  await expect(targetGroup).not.toHaveAttribute("open", "");
  await searchAndSelect(page, "matched-tool-result-fixture");
  const repeatedTargetCommand = stream.locator(`.execution-command[data-timeline-aliases~="${resultId}"]`);
  await expect.soft(targetGroup).toHaveAttribute("open", "", { timeout: 4000 });
  await expect.soft(repeatedTargetCommand).toHaveAttribute("open", "", { timeout: 4000 });
  await expect.soft(repeatedTargetCommand).toHaveClass(/timeline-target-highlight/, { timeout: 4000 });
  await expect.soft(manuallyCollapsedGroup).not.toHaveAttribute("open", "");
  await expect.soft(missFeedback).toHaveCount(0);
  expect(fixture.historyRequests).toEqual([]);
});

type ProviderSearchFixture = {
  title: string;
  providerLabel: "Claude Code" | "Grok Build";
  sessionKey: string;
  toolCallId: string;
  toolResultId: string;
  toolResultText: string;
  instructionCallId: string;
  instructionResultId: string;
  instructionText: string;
};

async function installProviderSearch(page: Page, provider: "claude_code" | "grok"): Promise<ProviderSearchFixture> {
  await mockApi(page);
  const isClaude = provider === "claude_code";
  const fixture: ProviderSearchFixture = {
    title: isClaude ? "Claude closed activity fixture" : "Grok closed activity fixture",
    providerLabel: isClaude ? "Claude Code" : "Grok Build",
    sessionKey: isClaude ? "claude:search-fixture" : "grok-search-fixture",
    toolCallId: isClaude ? "claude-tool-call-fixture" : "grok-tool-call-fixture",
    toolResultId: isClaude ? "claude-tool-result-fixture" : "grok-tool-result-fixture",
    toolResultText: isClaude ? "Claude tool output fixture" : "Grok tool output fixture",
    instructionCallId: isClaude ? "claude-instruction-call-fixture" : "grok-instruction-call-fixture",
    instructionResultId: isClaude ? "claude-instruction-result-fixture" : "grok-instruction-result-fixture",
    instructionText: isClaude ? "Claude AGENTS.md instruction fixture" : "Grok AGENTS.md instruction fixture"
  };
  const matches = {
    "tool-result-search-fixture": { positionKey: fixture.toolResultId, matchKind: "tool_result" },
    "instruction-search-fixture": { positionKey: fixture.instructionResultId, matchKind: "tool_output" }
  };

  await mockCommand(page, "sessions.search", args => {
    const request = args.request as SearchRequest;
    const match = matches[request.query as keyof typeof matches];
    if (!match) throw new Error(`Unexpected synthetic query: ${request.query}`);
    return {
      provider: request.provider,
      scope: request.scope,
      query: request.query,
      results: [{
        resultId: `${provider}-${request.query}`,
        sessionKey: fixture.sessionKey,
        nativeId: match.positionKey,
        title: fixture.title,
        matchKind: match.matchKind,
        positionKey: match.positionKey,
        snippet: `Synthetic ${match.matchKind} ${match.positionKey}`,
        timestamp: null
      }],
      truncated: false
    };
  });

  if (isClaude) {
    const summary: ClaudeSessionSummary = {
      id: "claude-search-session-fixture",
      sessionKey: fixture.sessionKey,
      title: fixture.title,
      cwd: "/fixture/workspace",
      path: "/fixture/claude-session.jsonl",
      status: "recent",
      formatVersion: "2.1.284",
      messageCount: 4,
      canRename: true,
      canDelete: true
    };
    const events: ClaudeHistoryEvent[] = [
      { id: fixture.toolCallId, kind: "tool_call", role: "Bash", callId: "claude-tool-call-id", detail: "printf claude-output-fixture", status: "completed" },
      { id: fixture.toolResultId, kind: "tool_result", role: "Bash", callId: "claude-tool-call-id", result: fixture.toolResultText, status: "completed" },
      { id: fixture.instructionCallId, kind: "tool_call", role: "Read", callId: "claude-instruction-call-id", detail: "/fixture/workspace/AGENTS.md", status: "completed" },
      { id: fixture.instructionResultId, kind: "tool_result", role: "Read", callId: "claude-instruction-call-id", result: fixture.instructionText, status: "completed" }
    ];
    await mockCommand(page, "claude.list", () => [summary]);
    await mockCommand(page, "claude.detail", () => ({ summary, events, totalEvents: events.length, hasMore: false, beforeCursor: null }));
  } else {
    const summary: GrokSessionSummary = {
      id: fixture.sessionKey,
      title: fixture.title,
      cwd: "/fixture/workspace",
      path: "/fixture/grok-session.jsonl",
      messageCount: 4,
      status: "recent"
    };
    const events: GrokHistoryEvent[] = [
      { id: fixture.toolCallId, kind: "tool_call", method: "exec_command", callId: "grok-tool-call-id", detail: "printf grok-output-fixture", status: "completed" },
      { id: fixture.toolResultId, kind: "tool_result", method: "exec_command", callId: "grok-tool-call-id", detail: fixture.toolResultText, status: "completed" },
      { id: fixture.instructionCallId, kind: "tool_call", method: "read_file", callId: "grok-instruction-call-id", detail: "/fixture/workspace/AGENTS.md", status: "completed" },
      { id: fixture.instructionResultId, kind: "tool_result", method: "read_file", callId: "grok-instruction-call-id", detail: fixture.instructionText, status: "completed" }
    ];
    await mockCommand(page, "grok.list", () => [summary]);
    await mockCommand(page, "grok.detail", () => ({ summary, events, totalEvents: events.length, hasMore: false, beforeCursor: null }));
  }
  return fixture;
}

for (const provider of ["claude_code", "grok"] as const) {
  test(`${provider} search reveals closed tool output and AGENTS.md instruction activities by event identity`, async ({ page }) => {
    const fixture = await installProviderSearch(page, provider);
    await page.goto("/");
    await page.locator(".side-nav").getByRole("button", { name: fixture.providerLabel, exact: true }).click();
    await page.locator(".provider-session").filter({ hasText: fixture.title }).click();
    await expect(page.locator(".conversation-title")).toHaveText(fixture.title);

    const stream = page.locator(".provider-events");
    const toolGroup = stream.locator(`.execution-group[data-timeline-id*="${fixture.toolCallId}"]`);
    const toolCommand = stream.locator(`.execution-command[data-timeline-aliases~="${fixture.toolResultId}"]`);
    await searchAndSelect(page, "tool-result-search-fixture", fixture.providerLabel);
    await expect.soft(toolGroup).toHaveAttribute("open", "", { timeout: 4000 });
    await expect.soft(toolCommand).toHaveAttribute("open", "", { timeout: 4000 });
    await expect.soft(toolCommand.locator(".execution-section").last()).toContainText(fixture.toolResultText, { timeout: 4000 });
    await expect.soft(toolCommand).toHaveClass(/timeline-target-highlight/, { timeout: 4000 });

    const instructionCommand = stream.locator(`.execution-command[data-timeline-aliases~="${fixture.instructionResultId}"]`);
    await searchAndSelect(page, "instruction-search-fixture", fixture.providerLabel);
    await expect.soft(instructionCommand).toHaveClass(/timeline-target-highlight/, { timeout: 1500 });
    const instructionGroup = instructionCommand.locator("xpath=ancestor::details[contains(concat(' ', normalize-space(@class), ' '), ' execution-group ')][1]");
    await expect.soft(instructionGroup).toHaveAttribute("data-timeline-id", expect.stringContaining(fixture.toolCallId));
    await expect.soft(instructionGroup).toHaveAttribute("open", "");
    await expect.soft(instructionCommand).toHaveAttribute("open", "", { timeout: 4000 });
    await expect.soft(instructionCommand.locator(".execution-section").last()).toContainText(fixture.instructionText, { timeout: 4000 });
    await expect.soft(page.getByRole("status").filter({ hasText: "结果所在历史未加载，请刷新后重试" })).toHaveCount(0);
  });
}

test("search reveals folded AGENTS.md instructions in user and assistant message blocks", async ({ page }) => {
  const title = "Folded message instructions fixture";
  const userBlockId = "user-message-instructions-block-fixture";
  const assistantBlockId = "assistant-message-instructions-block-fixture";
  const userText = "# AGENTS.md instructions for /fixture/workspace\n\n<INSTRUCTIONS>\nUser message instruction fixture.\n</INSTRUCTIONS>\n\nUser request fixture.";
  const assistantText = "# AGENTS.md instructions for /fixture/workspace\n\n<INSTRUCTIONS>\nAssistant message instruction fixture.\n</INSTRUCTIONS>\n\nAssistant reply fixture.";
  const userQuery = "user-message-instructions-search-fixture";
  const assistantQuery = "assistant-message-instructions-search-fixture";
  await installSyntheticSearch(page, {
    title,
    blocks: [
      { id: userBlockId, role: "user", kind: "message", text: userText, questions: [] },
      { id: assistantBlockId, role: "assistant", kind: "message", text: assistantText, questions: [] }
    ],
    positionKey: userBlockId,
    matchKind: "user_message",
    alternateResults: {
      [assistantQuery]: { positionKey: assistantBlockId, matchKind: "assistant_message" }
    }
  });

  await openSyntheticThread(page, title);
  const stream = page.locator(".conversation-main .message-stream");
  const userBlock = stream.locator(`.user-message[data-timeline-id="${userBlockId}"]`);
  const userInstructions = userBlock.locator("details.user-instructions");
  const assistantBlock = stream.locator(`.chat-row.assistant[data-timeline-id="${assistantBlockId}"]`);
  const assistantInstructions = assistantBlock.locator("details.instruction-file");
  await expect(userInstructions).not.toHaveAttribute("open", "");
  await expect(assistantInstructions).not.toHaveAttribute("open", "");

  await searchAndSelect(page, userQuery);
  await expect(userBlock).toHaveClass(/timeline-target-highlight/, { timeout: 1500 });
  await expect(userInstructions).toHaveAttribute("open", "");
  await expect(userInstructions.locator(".user-instructions-body")).toContainText("User message instruction fixture.");

  await searchAndSelect(page, assistantQuery);
  await expect(assistantBlock).toHaveClass(/timeline-target-highlight/, { timeout: 1500 });
  await expect(assistantInstructions).toHaveAttribute("open", "");
  await expect(assistantInstructions.locator(".instruction-file-body")).toContainText("Assistant message instruction fixture.");
  await expect(page.getByRole("status").filter({ hasText: "结果所在历史未加载，请刷新后重试" })).toHaveCount(0);
});
