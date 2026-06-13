import { describe, expect, test } from "vitest";
import type { BridgeActionResult, MessageBlock, ThreadDetail, ThreadSummary } from "../types";
import {
  applyRealtimeBlocksToThreadSlot,
  applyThreadBlockPageToSlot,
  applyThreadDetailToSlot,
  createThreadMessageStoreState,
  setActiveThreadSlot,
  setThreadFeedback,
  setThreadLastResult
} from "./threadMessageStore";

function summary(id: string, title = id): ThreadSummary {
  return {
    id,
    title,
    status: "Recent",
    message_count: 0
  };
}

function block(id: string, text = id): MessageBlock {
  return {
    id,
    role: "assistant",
    kind: "message",
    text,
    questions: []
  };
}

function detail(id: string, blocks: MessageBlock[], beforeCursor: string | null = null): ThreadDetail {
  return {
    summary: summary(id),
    messages: [],
    blocks,
    raw_event_count: blocks.length,
    total_blocks: blocks.length + (beforeCursor ? 10 : 0),
    has_more_blocks: Boolean(beforeCursor),
    before_cursor: beforeCursor
  };
}

describe("thread message store", () => {
  test("keeps message slots isolated across active thread switches", () => {
    const store = createThreadMessageStoreState();

    setActiveThreadSlot(store, "thread-a");
    applyThreadDetailToSlot(store, "thread-a", detail("thread-a", [block("a1")]));
    setActiveThreadSlot(store, "thread-b");
    applyThreadDetailToSlot(store, "thread-b", detail("thread-b", [block("b1")]));

    expect(store.activeThreadId).toBe("thread-b");
    expect(store.slots.get("thread-a")?.blocks.map((item) => item.id)).toEqual(["a1"]);
    expect(store.slots.get("thread-b")?.blocks.map((item) => item.id)).toEqual(["b1"]);
  });

  test("writes stale detail responses to the captured thread slot", () => {
    const store = createThreadMessageStoreState();
    setActiveThreadSlot(store, "thread-b");
    applyThreadDetailToSlot(store, "thread-b", detail("thread-b", [block("b1")]));

    applyThreadDetailToSlot(store, "thread-a", detail("thread-a", [block("a1")]));

    expect(store.activeThreadId).toBe("thread-b");
    expect(store.slots.get("thread-b")?.blocks.map((item) => item.id)).toEqual(["b1"]);
    expect(store.slots.get("thread-a")?.blocks.map((item) => item.id)).toEqual(["a1"]);
  });

  test("rejects detail data whose summary id does not match the target slot", () => {
    const store = createThreadMessageStoreState();
    applyThreadDetailToSlot(store, "thread-b", detail("thread-b", [block("b1")]));

    applyThreadDetailToSlot(store, "thread-b", detail("thread-a", [block("a-stale")]));

    expect(store.slots.get("thread-b")?.blocks.map((item) => item.id)).toEqual(["b1"]);
    expect(store.slots.get("thread-a")).toBeUndefined();
  });

  test("preserves prepended history when a fresh detail page arrives", () => {
    const store = createThreadMessageStoreState();
    applyThreadDetailToSlot(store, "thread-a", detail("thread-a", [block("new-1"), block("new-2")], "b:100"));
    applyThreadBlockPageToSlot(store, "thread-a", {
      thread_id: "thread-a",
      blocks: [block("old-1"), block("old-2")],
      total_blocks: 120,
      has_more_blocks: true,
      before_cursor: "b:80"
    }, "b:100");

    applyThreadDetailToSlot(store, "thread-a", detail("thread-a", [block("new-1", "new updated"), block("new-2")], "b:100"));

    const slot = store.slots.get("thread-a");
    expect(slot?.blocks.map((item) => item.id)).toEqual(["old-1", "old-2", "new-1", "new-2"]);
    expect(slot?.blocks.find((item) => item.id === "new-1")?.text).toBe("new updated");
    expect(slot?.beforeCursor).toBe("b:80");
  });

  test("prepends load-more pages only to the captured slot", () => {
    const store = createThreadMessageStoreState();
    applyThreadDetailToSlot(store, "thread-a", detail("thread-a", [block("a-new")], "b:100"));
    applyThreadDetailToSlot(store, "thread-b", detail("thread-b", [block("b-new")], "b:50"));
    setActiveThreadSlot(store, "thread-b");

    applyThreadBlockPageToSlot(store, "thread-a", {
      thread_id: "thread-a",
      blocks: [block("a-old")],
      total_blocks: 101,
      has_more_blocks: false,
      before_cursor: null
    }, "b:100");

    expect(store.slots.get("thread-a")?.blocks.map((item) => item.id)).toEqual(["a-old", "a-new"]);
    expect(store.slots.get("thread-b")?.blocks.map((item) => item.id)).toEqual(["b-new"]);
  });

  test("rejects load-more pages whose thread id does not match the target slot", () => {
    const store = createThreadMessageStoreState();
    applyThreadDetailToSlot(store, "thread-b", detail("thread-b", [block("b-new")], "b:50"));

    applyThreadBlockPageToSlot(store, "thread-b", {
      thread_id: "thread-a",
      blocks: [block("a-old")],
      total_blocks: 100,
      has_more_blocks: false,
      before_cursor: null
    }, "b:50");

    expect(store.slots.get("thread-b")?.blocks.map((item) => item.id)).toEqual(["b-new"]);
    expect(store.slots.get("thread-b")?.beforeCursor).toBe("b:50");
  });

  test("ignores stale load-more pages with an outdated cursor", () => {
    const store = createThreadMessageStoreState();
    applyThreadDetailToSlot(store, "thread-a", detail("thread-a", [block("new")], "b:100"));
    applyThreadBlockPageToSlot(store, "thread-a", {
      thread_id: "thread-a",
      blocks: [block("old-current")],
      total_blocks: 120,
      has_more_blocks: true,
      before_cursor: "b:80"
    }, "b:100");

    applyThreadBlockPageToSlot(store, "thread-a", {
      thread_id: "thread-a",
      blocks: [block("old-stale")],
      total_blocks: 120,
      has_more_blocks: false,
      before_cursor: null
    }, "b:100");

    expect(store.slots.get("thread-a")?.blocks.map((item) => item.id)).toEqual(["old-current", "new"]);
    expect(store.slots.get("thread-a")?.beforeCursor).toBe("b:80");
  });

  test("appends SSE batches to the subscribed thread slot after active switch", () => {
    const store = createThreadMessageStoreState();
    applyThreadDetailToSlot(store, "thread-a", detail("thread-a", [block("a1")]));
    applyThreadDetailToSlot(store, "thread-b", detail("thread-b", [block("b1")]));
    setActiveThreadSlot(store, "thread-b");

    applyRealtimeBlocksToThreadSlot(store, "thread-a", [block("a2")]);

    expect(store.slots.get("thread-a")?.blocks.map((item) => item.id)).toEqual(["a1", "a2"]);
    expect(store.slots.get("thread-b")?.blocks.map((item) => item.id)).toEqual(["b1"]);
  });

  test("stores mutation result and feedback per captured thread", () => {
    const store = createThreadMessageStoreState();
    const result: BridgeActionResult = {
      bridge: true,
      thread_id: "thread-a",
      turn_id: "turn-a",
      fallback: false
    };

    setThreadLastResult(store, "thread-a", result);
    setThreadFeedback(store, "thread-a", "submitted");
    setActiveThreadSlot(store, "thread-b");

    expect(store.slots.get("thread-a")?.lastResult?.turn_id).toBe("turn-a");
    expect(store.slots.get("thread-a")?.feedback).toBe("submitted");
    expect(store.slots.get("thread-b")?.lastResult).toBeNull();
    expect(store.slots.get("thread-b")?.feedback).toBeNull();
  });

  test("keeps visible last event when detail refresh only has internal or empty event", () => {
    const store = createThreadMessageStoreState();
    applyThreadDetailToSlot(store, "thread-a", {
      ...detail("thread-a", [block("a1")]),
      summary: { ...summary("thread-a"), last_event_kind: "task_complete" }
    });

    applyThreadDetailToSlot(store, "thread-a", {
      ...detail("thread-a", [block("a1")]),
      summary: { ...summary("thread-a"), last_event_kind: "app-server.thread/read" }
    });
    expect(store.slots.get("thread-a")?.summary?.last_event_kind).toBe("task_complete");

    applyThreadDetailToSlot(store, "thread-a", {
      ...detail("thread-a", [block("a1")]),
      summary: { ...summary("thread-a"), last_event_kind: null }
    });
    expect(store.slots.get("thread-a")?.summary?.last_event_kind).toBe("task_complete");
  });
});
