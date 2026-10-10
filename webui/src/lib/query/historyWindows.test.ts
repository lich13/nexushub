import { InfiniteQueryObserver, QueryClient } from "@tanstack/react-query";
import { describe, expect, test, vi } from "vitest";
import type { ClaudeHistoryEvent, ClaudeSessionDetail, SubagentDetailResponse } from "../../types";
import { retainClaudeHistory, retainSubagentHistory } from "./historyWindows";

function claudeEvent(id: string, text = id): ClaudeHistoryEvent {
  return { id, kind: "message", role: "assistant", text };
}

function claudePage(overrides: Partial<ClaudeSessionDetail> = {}): ClaudeSessionDetail {
  return {
    summary: {
      id: "session-fixture",
      sessionKey: "session-key-fixture",
      title: "示例会话",
      cwd: "/tmp/example",
      path: "/tmp/example/session.jsonl",
      messageCount: 3,
      status: "idle",
      formatVersion: "1",
      canRename: false,
      canDelete: false
    },
    events: [],
    totalEvents: 0,
    hasMore: false,
    beforeCursor: null,
    ...overrides
  };
}

function subagentBlock(id: string, text = id) {
  return { id, role: "assistant", kind: "message", text, questions: [] };
}

function subagentPage(overrides: Partial<SubagentDetailResponse> = {}): SubagentDetailResponse {
  return {
    rootThreadId: "root-fixture",
    parentThreadId: "parent-fixture",
    agent: { agentId: "child-fixture", name: "示例子智能体", status: "running", available: true },
    detail: {
      summary: { id: "child-fixture", title: "示例子智能体", status: "Running", message_count: 2 },
      messages: [],
      blocks: [],
      raw_event_count: 2,
      total_blocks: 2,
      has_more_blocks: false,
      before_cursor: null
    },
    ...overrides
  };
}

describe("history window structural sharing", () => {
  test("keeps the earliest loaded Claude page and its cursor while a polling window slides", () => {
    const previous = {
      pages: [
        claudePage({
          events: [claudeEvent("event-2"), claudeEvent("event-3")],
          totalEvents: 4,
          hasMore: true,
          beforeCursor: "cursor-old"
        }),
        claudePage({ events: [claudeEvent("event-0"), claudeEvent("event-1")], totalEvents: 4, hasMore: false })
      ],
      pageParams: [undefined, "cursor-old"]
    };
    const incoming = {
      pages: [claudePage({
        events: [claudeEvent("event-3"), claudeEvent("event-4")],
        totalEvents: 5,
        hasMore: true,
        beforeCursor: "cursor-new"
      })],
      pageParams: [undefined]
    };

    const retained = retainClaudeHistory(previous, incoming);

    expect(retained.pages).toHaveLength(2);
    expect(retained.pages[0].events.map(event => event.id)).toEqual(["event-2", "event-3", "event-4"]);
    expect(retained.pages[0].beforeCursor).toBe("cursor-old");
    expect(retained.pages[0].hasMore).toBe(true);
    expect(retained.pages[1].events.map(event => event.id)).toEqual(["event-0", "event-1"]);
    expect(retained.pageParams).toEqual([undefined, "cursor-old"]);
  });

  test("updates duplicate IDs with fresh content without moving their position", () => {
    const previous = {
      pages: [claudePage({
        events: [claudeEvent("event-a", "A"), claudeEvent("event-b", "旧结果"), claudeEvent("event-c", "C")],
        totalEvents: 3
      })],
      pageParams: [undefined]
    };
    const incoming = {
      pages: [claudePage({
        events: [claudeEvent("event-b", "新结果"), claudeEvent("event-c", "C")],
        totalEvents: 3
      })],
      pageParams: [undefined]
    };

    const retained = retainClaudeHistory(previous, incoming);

    expect(retained.pages[0].events.map(event => event.id)).toEqual(["event-a", "event-b", "event-c"]);
    expect(retained.pages[0].events[1].text).toBe("新结果");
  });

  test("does not turn an exhausted Claude history back into a paginatable one", () => {
    const previous = {
      pages: [claudePage({ hasMore: false, beforeCursor: null, totalEvents: 4 })],
      pageParams: [undefined]
    };
    const incoming = {
      pages: [claudePage({ hasMore: true, beforeCursor: "should-not-revive", totalEvents: 4 })],
      pageParams: [undefined]
    };

    const retained = retainClaudeHistory(previous, incoming);

    expect(retained.pages[0].hasMore).toBe(false);
    expect(retained.pages[0].beforeCursor).toBeNull();
  });

  test.each([
    ["session key", (page: ClaudeSessionDetail) => ({ ...page, summary: { ...page.summary, sessionKey: "other-session" } })],
    ["native ID", (page: ClaudeSessionDetail) => ({ ...page, summary: { ...page.summary, id: "other-id" } })]
  ])("resets the loaded Claude window when %s changes", (_label, changeIdentity) => {
    const previous = {
      pages: [claudePage({ events: [claudeEvent("old-event")], totalEvents: 10 })],
      pageParams: [undefined]
    };
    const changed = changeIdentity(claudePage({ events: [claudeEvent("new-event")], totalEvents: 10 }));
    const incoming = { pages: [changed], pageParams: [undefined] };

    const retained = retainClaudeHistory(previous, incoming);

    expect(retained).toBe(incoming);
    expect(retained.pages[0].events.map(event => event.id)).toEqual(["new-event"]);
  });

  test("resets a Claude window when the native event total shrinks", () => {
    const previous = {
      pages: [claudePage({ events: [claudeEvent("old-event")], totalEvents: 10 })],
      pageParams: [undefined]
    };
    const incoming = {
      pages: [claudePage({ events: [claudeEvent("new-event")], totalEvents: 3 })],
      pageParams: [undefined]
    };

    expect(retainClaudeHistory(previous, incoming)).toBe(incoming);
  });

  test("keeps an old service subagent response readable without optional pagination fields", () => {
    const previousPage = subagentPage({
      detail: {
        ...subagentPage().detail,
        blocks: [subagentBlock("block-old")]
      }
    });
    delete previousPage.detail.has_more_blocks;
    delete previousPage.detail.before_cursor;
    const incomingPage = subagentPage({
      detail: {
        ...subagentPage().detail,
        blocks: [subagentBlock("block-old", "更新结果")]
      }
    });
    delete incomingPage.detail.has_more_blocks;
    delete incomingPage.detail.before_cursor;

    const retained = retainSubagentHistory(
      { pages: [previousPage], pageParams: [null] },
      { pages: [incomingPage], pageParams: [null] }
    );

    expect(retained.pages[0].detail.blocks).toHaveLength(1);
    expect(retained.pages[0].detail.blocks[0].text).toBe("更新结果");
    expect(retained.pages[0].detail.has_more_blocks).toBeUndefined();
    expect(retained.pages[0].detail.before_cursor).toBeUndefined();
  });

  test("keeps a subagent's earliest block and cursor while polling refreshes its latest page", () => {
    const previous = {
      pages: [subagentPage({
        agent: { ...subagentPage().agent, status: "running" },
        detail: {
          ...subagentPage().detail,
          blocks: [subagentBlock("block-2"), subagentBlock("block-3")],
          total_blocks: 4,
          has_more_blocks: true,
          before_cursor: "block-cursor-old"
        }
      }), subagentPage({
        detail: {
          ...subagentPage().detail,
          blocks: [subagentBlock("block-0"), subagentBlock("block-1")],
          total_blocks: 4,
          has_more_blocks: false,
          before_cursor: null
        }
      })],
      pageParams: [null, "block-cursor-old"]
    };
    const incoming = {
      pages: [subagentPage({
        agent: { ...subagentPage().agent, status: "completed" },
        detail: {
          ...subagentPage().detail,
          blocks: [subagentBlock("block-3"), subagentBlock("block-4")],
          total_blocks: 5,
          has_more_blocks: true,
          before_cursor: "block-cursor-new"
        }
      })],
      pageParams: [null]
    };

    const retained = retainSubagentHistory(previous, incoming);

    expect(retained.pages).toHaveLength(2);
    expect(retained.pages[0].agent.status).toBe("completed");
    expect(retained.pages[0].detail.blocks.map(block => block.id)).toEqual(["block-2", "block-3", "block-4"]);
    expect(retained.pages[0].detail.before_cursor).toBe("block-cursor-old");
    expect(retained.pages[1].detail.blocks.map(block => block.id)).toEqual(["block-0", "block-1"]);
    expect(retained.pageParams).toEqual([null, "block-cursor-old"]);
  });

  test.each([
    ["root thread", (page: SubagentDetailResponse) => ({ ...page, rootThreadId: "other-root" })],
    ["parent thread", (page: SubagentDetailResponse) => ({ ...page, parentThreadId: "other-parent" })],
    ["agent", (page: SubagentDetailResponse) => ({ ...page, agent: { ...page.agent, agentId: "other-agent" } })],
    ["detail", (page: SubagentDetailResponse) => ({ ...page, detail: { ...page.detail, summary: { ...page.detail.summary, id: "other-detail" } } })]
  ])("resets the subagent window when %s identity changes", (_label, changeIdentity) => {
    const previous = {
      pages: [subagentPage({ detail: { ...subagentPage().detail, blocks: [subagentBlock("old-block")] } })],
      pageParams: [null]
    };
    const incomingPage = changeIdentity(subagentPage({ detail: { ...subagentPage().detail, blocks: [subagentBlock("new-block")] } }));
    const incoming = { pages: [incomingPage], pageParams: [null] };

    const retained = retainSubagentHistory(previous, incoming);

    expect(retained).toBe(incoming);
    expect(retained.pages[0].detail.blocks.map(block => block.id)).toEqual(["new-block"]);
  });

  test("uses the preserved earliest cursor when a real infinite query polls then fetches another page", async () => {
    const first = claudePage({
      events: [claudeEvent("event-2"), claudeEvent("event-3")],
      totalEvents: 4,
      hasMore: true,
      beforeCursor: "cursor-old"
    });
    const polled = claudePage({
      events: [claudeEvent("event-3"), claudeEvent("event-4")],
      totalEvents: 5,
      hasMore: true,
      beforeCursor: "cursor-new"
    });
    const older = claudePage({
      events: [claudeEvent("event-0"), claudeEvent("event-1")],
      totalEvents: 5,
      hasMore: false,
      beforeCursor: null
    });
    const cursors: (string | undefined)[] = [];
    const queryFn = vi.fn(({ pageParam }: { pageParam: unknown }) => {
      const cursor = typeof pageParam === "string" ? pageParam : undefined;
      cursors.push(cursor);
      if (cursors.length === 1) return Promise.resolve(first);
      if (cursors.length === 2) return Promise.resolve(polled);
      if (cursor === "cursor-old") return Promise.resolve(older);
      throw new Error(`unexpected cursor ${String(cursor)}`);
    });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } });
    const observer = new InfiniteQueryObserver(client, {
      queryKey: ["claude-history-fixture"],
      queryFn,
      initialPageParam: undefined as string | undefined,
      getNextPageParam: page => page.hasMore ? page.beforeCursor ?? undefined : undefined,
      structuralSharing: retainClaudeHistory
    });

    await observer.refetch();
    await observer.refetch();
    expect(cursors).toEqual([undefined, undefined]);
    expect(observer.getCurrentResult().data?.pages[0].beforeCursor).toBe("cursor-old");
    expect(observer.getCurrentResult().data?.pages[0].events.map(event => event.id)).toEqual(["event-2", "event-3", "event-4"]);

    await observer.fetchNextPage();

    expect(cursors).toEqual([undefined, undefined, "cursor-old"]);
    expect(observer.getCurrentResult().data?.pages).toHaveLength(2);
    expect(observer.getCurrentResult().data?.pages[1].events.map(event => event.id)).toEqual(["event-0", "event-1"]);
    client.clear();
  });
});
