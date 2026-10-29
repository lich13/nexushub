import { describe, expect, test } from "vitest";
import { grokHandoffPath } from "../../components/grok/GrokWorkspace";
import type {
  ClaudeSessionDetail,
  GrokSessionDetail,
  MessageBlock,
  ProviderHistoryEvent,
  ProviderSessionDetail,
  ThreadDetail,
} from "../../types";
import {
  retainClaudeHistory,
  retainCodexHistory,
  retainGrokHistory,
  retainProviderHistory,
  retainSubagentHistory,
} from "../query/historyWindows";
import { compactConversationBlocks } from "./conversationViewModel";
import { groupProviderEvents, type ExecutionGroup } from "./executionGroups";
import {
  foldProcessSequence,
  processGroupSummaryText,
  thinkingStepFromText,
} from "./processGroups";

function makeExecutionGroup(
  id: string,
  commandCount = 2,
  options: { running?: boolean; failedCount?: number; turnId?: string | null } = {},
): ExecutionGroup {
  return {
    id,
    kind: "command",
    provider: "Antigravity",
    running: options.running ?? false,
    failedCount: options.failedCount ?? 0,
    turnId: options.turnId,
    commands: Array.from({ length: commandCount }, (_, index) => ({
      id: `${id}:cmd:${index}`,
      sourceIds: [`call-${id}-${index}`, `out-${id}-${index}`],
      title: "run_command",
      preview: `printf ${index}`,
      status:
        index < (options.failedCount ?? 0)
          ? "failed"
          : options.running && index === commandCount - 1
            ? "running"
            : "completed",
      sections: [{ label: "结果", text: "ok" }],
    })),
  };
}

describe("foldProcessSequence & ChatGPT-App intermediate process collapsing", () => {
  test("preserves a standalone single ExecutionGroup unwrapped", () => {
    const group = makeExecutionGroup("group-single", 2);
    const items = [
      { type: "user" as const, id: "u-1" },
      { type: "group" as const, group },
      { type: "assistant" as const, id: "a-1" },
    ];

    const folded = foldProcessSequence("antigravity", items, item =>
      item.type === "group" ? { kind: "execution", group: item.group } : null,
    );

    expect(folded.map(item => item.kind)).toEqual(["passthrough", "execution", "passthrough"]);
  });

  test("collapses a 20-row alternating thinking + command wall into a single UnifiedProcessGroup", () => {
    const events: ProviderHistoryEvent[] = [{ id: "u-1", kind: "user_message", text: "Inspect repo" }];
    for (let i = 0; i < 10; i += 1) {
      events.push({
        id: `think-${i}`,
        kind: "thinking",
        text: `Reasoning step ${i}`,
        turnId: "turn-1",
      });
      events.push({
        id: `call-${i}-a`,
        kind: "tool_call",
        role: "run_command",
        callId: `c-${i}-a`,
        detail: `ls step-${i}-a`,
        status: "completed",
        turnId: "turn-1",
      });
      events.push({
        id: `out-${i}-a`,
        kind: "tool_result",
        role: "run_command",
        callId: `c-${i}-a`,
        result: "ok",
        status: "completed",
        turnId: "turn-1",
      });
      events.push({
        id: `call-${i}-b`,
        kind: "tool_call",
        role: "run_command",
        callId: `c-${i}-b`,
        detail: `ls step-${i}-b`,
        status: i === 9 ? "failed" : "completed",
        turnId: "turn-1",
      });
      events.push({
        id: `out-${i}-b`,
        kind: "tool_result",
        role: "run_command",
        callId: `c-${i}-b`,
        result: i === 9 ? "exit 1" : "ok",
        status: i === 9 ? "failed" : "completed",
        turnId: "turn-1",
      });
    }
    events.push({ id: "a-1", kind: "assistant_message", text: "Final summary." });

    const grouped = groupProviderEvents(events, "Antigravity");
    // Before folding: 1 user + 10 thinking + 10 ExecutionGroups + 1 assistant = 22 rows
    expect(grouped).toHaveLength(22);

    const folded = foldProcessSequence("antigravity", grouped, entry => {
      if (entry.kind === "group") {
        return { kind: "execution", group: entry.group, turnId: entry.group.turnId };
      }
      if (entry.item.kind === "thinking") {
        return thinkingStepFromText(entry.key, entry.sourceIds, entry.item.text, false, entry.item.turnId);
      }
      return null;
    });

    expect(folded).toHaveLength(3);
    expect(folded[0]?.kind).toBe("passthrough");
    expect(folded[1]?.kind).toBe("process");
    expect(folded[2]?.kind).toBe("passthrough");

    if (folded[1]?.kind !== "process") throw new Error("expected process group");
    const process = folded[1].process;
    expect(process.thoughtCount).toBe(10);
    expect(process.toolCount).toBe(20);
    expect(process.failedCount).toBe(1);
    expect(process.running).toBe(false);
    expect(process.timelineAliases).toContain("think-0");
    expect(process.timelineAliases).toContain("call-9-b");
    expect(process.timelineAliases).toContain("out-9-b");
    expect(processGroupSummaryText(process)).toEqual({
      primary: "已思考",
      meta: "20 项操作",
    });
  });

  test("splits process groups cleanly across distinct turn boundaries and skips blank thoughts", () => {
    const items = [
      { kind: "thought" as const, id: "blank", text: "   \n  ", turnId: "t-1" },
      { kind: "exec" as const, group: makeExecutionGroup("g-1", 1, { turnId: "t-1" }) },
      { kind: "exec" as const, group: makeExecutionGroup("g-2", 1, { running: true, turnId: "t-2" }) },
    ];

    const folded = foldProcessSequence("codex", items, item =>
      item.kind === "thought"
        ? thinkingStepFromText(item.id, [item.id], item.text, false, item.turnId)
        : { kind: "execution", group: item.group, turnId: item.group.turnId },
    );

    expect(folded.map(item => item.kind)).toEqual(["execution", "execution"]);
  });

  test("formats running and standalone thought summaries in ChatGPT-App style", () => {
    const folded = foldProcessSequence(
      "dsh",
      [{ id: "t-live", text: "Checking invariants" }],
      item => thinkingStepFromText(item.id, [item.id], item.text, true),
    );
    expect(folded).toHaveLength(1);
    if (folded[0]?.kind !== "process") throw new Error("expected process");
    expect(processGroupSummaryText(folded[0].process)).toEqual({
      primary: "正在思考…",
      meta: null,
    });
  });
});

describe("timeline retention & history collapse aliases across all 5 agents", () => {
  const sampleTimeline = [
    { id: "turn-1", title: "First prompt", preview: "Detail" },
    { id: "turn-2", title: "Second prompt" },
  ];

  test("retainProviderHistory, retainClaudeHistory, retainGrokHistory, retainCodexHistory, and retainSubagentHistory preserve pages[0].timeline on cursor poll merges", () => {
    const providerBase: ProviderSessionDetail = {
      summary: {
        id: "ag-1",
        sessionKey: "ag:ag-1",
        title: "Antigravity",
        cwd: "/isolated",
        path: "/isolated/transcript_full.jsonl",
        archived: false,
        updatedAt: "2026-10-01T00:00:00Z",
        messageCount: 2,
        status: "recent",
        formatVersion: "v1",
        canRename: true,
        canArchive: true,
        canDelete: true,
      },
      events: [{ id: "e-2", kind: "user_message", text: "Second prompt" }],
      totalEvents: 2,
      hasMore: true,
      beforeCursor: "1",
      timeline: sampleTimeline,
    };
    const providerPollUpdate: ProviderSessionDetail = {
      ...providerBase,
      timeline: undefined,
    };
    const mergedProvider = retainProviderHistory(
      { pages: [providerBase], pageParams: [undefined] },
      { pages: [providerPollUpdate], pageParams: [undefined] },
    );
    expect(mergedProvider.pages[0]?.timeline).toEqual(sampleTimeline);

    const claudeBase: ClaudeSessionDetail = {
      summary: {
        id: "c-1",
        sessionKey: "claude:c-1",
        title: "Claude",
        cwd: "/isolated",
        path: "/isolated/c-1.jsonl",
        status: "recent",
        formatVersion: "2.1",
        messageCount: 2,
        canRename: true,
        canDelete: true,
      },
      events: [{ id: "ce-2", kind: "user_message", text: "Second prompt" }],
      totalEvents: 2,
      hasMore: true,
      beforeCursor: "1",
      timeline: sampleTimeline,
    };
    expect(
      retainClaudeHistory(
        { pages: [claudeBase], pageParams: [undefined] },
        { pages: [{ ...claudeBase, timeline: undefined }], pageParams: [undefined] },
      ).pages[0]?.timeline,
    ).toEqual(sampleTimeline);

    const grokBase: GrokSessionDetail = {
      summary: {
        id: "g-1",
        title: "Grok",
        cwd: "/isolated",
        path: "/isolated/g-1",
        handoffPath: "/isolated/g-1/chat_history.jsonl",
        messageCount: 2,
        status: "recent",
      },
      events: [{ id: "ge-2", kind: "user_message_chunk", text: "Second prompt" }],
      totalEvents: 2,
      hasMore: true,
      beforeCursor: "1",
      timeline: sampleTimeline,
    };
    expect(
      retainGrokHistory(
        { pages: [grokBase], pageParams: [undefined] },
        { pages: [{ ...grokBase, timeline: undefined }], pageParams: [undefined] },
      ).pages[0]?.timeline,
    ).toEqual(sampleTimeline);

    const codexBase: ThreadDetail = {
      summary: { id: "t-1", title: "Codex", status: "Recent", message_count: 2 },
      blocks: [{ id: "b-2", role: "user", kind: "message", text: "Second prompt", questions: [] }],
      messages: [],
      raw_event_count: 2,
      total_blocks: 2,
      has_more_blocks: true,
      before_cursor: "1",
      timeline: sampleTimeline,
    };
    expect(
      retainCodexHistory(
        { pages: [codexBase], pageParams: [undefined] },
        { pages: [{ ...codexBase, timeline: undefined }], pageParams: [undefined] },
      ).pages[0]?.timeline,
    ).toEqual(sampleTimeline);

    const subagentBase = {
      rootThreadId: "t-root",
      parentThreadId: "t-1",
      agent: {
        agentId: "sub-1",
        status: "completed" as const,
      },
      detail: codexBase,
    };
    expect(
      retainSubagentHistory(
        { pages: [subagentBase], pageParams: [undefined] },
        {
          pages: [{ ...subagentBase, detail: { ...codexBase, timeline: undefined } }],
          pageParams: [undefined],
        },
      ).pages[0]?.detail.timeline,
    ).toEqual(sampleTimeline);
  });

  test("compactConversationBlocks attaches collapsedIds to history-collapse placeholder", () => {
    const blocks: MessageBlock[] = Array.from({ length: 40 }, (_, index) => ({
      id: `block-${index}`,
      role: index % 2 === 0 ? "user" : "assistant",
      kind: "message",
      text: `Turn message ${index}`,
      questions: [],
    }));

    const compacted = compactConversationBlocks(blocks, 10, 4);
    const collapseBlock = compacted.find(block => block.id === "chat-history-collapsed");
    expect(collapseBlock).toBeDefined();
    const payload = collapseBlock?.payload as { collapsedIds?: string[] } | undefined;
    expect(payload?.collapsedIds).toContain("block-0");
    expect(payload?.collapsedIds).toContain("block-20");
    expect(payload?.collapsedIds).not.toContain("block-39");
  });

  test("grokHandoffPath prefers optimal handoffPath over directory path", () => {
    expect(
      grokHandoffPath({
        path: "/isolated/sessions/g-1",
        handoffPath: "/isolated/sessions/g-1/chat_history.jsonl",
      }),
    ).toBe("/isolated/sessions/g-1/chat_history.jsonl");
    expect(grokHandoffPath({ path: "/isolated/sessions/g-1", handoffPath: null })).toBe(
      "/isolated/sessions/g-1",
    );
  });
});
