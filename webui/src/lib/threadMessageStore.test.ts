import { describe, expect, test } from "vitest";
import type { MessageBlock, SubagentActivity, SubagentCollection, ThreadDetail, ThreadSummary } from "../types";
import {
  applyRealtimeBlocksToThreadSlot,
  applyThreadBlockPageToSlot,
  applyThreadDetailToSlot,
  clearThreadSlot,
  createThreadMessageStoreState,
  setActiveThreadSlot,
  setThreadFeedback,
  setThreadLastResult,
  threadDetailFromMessageSlot
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

function subagents(status: SubagentActivity["status"] = "running"): SubagentCollection {
  return {
    agents: [{ agentId: "child-fixture", name: "规范示例名称", status, available: true }],
    counts: { creating: 0, running: 0, completed: 0, failed: 0, interrupted: 0, unknown: 0, [status]: 1 },
    complete: true
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

  test.each([
    { scenario: "fully overlapping cached blocks", includeHistory: false, includeRealtimeTail: false },
    { scenario: "loaded pagination history", includeHistory: true, includeRealtimeTail: false },
    { scenario: "loaded history and a newer SSE tail", includeHistory: true, includeRealtimeTail: true }
  ])("uses refreshed native activity order with $scenario and leaves identical refreshes stable", ({ includeHistory, includeRealtimeTail }) => {
    const store = createThreadMessageStoreState();
    const threadId = "thread-order-fixture";
    const agent: SubagentActivity = {
      agentId: "child-order-fixture", name: "顺序示例", status: "running", available: true,
      delegation: "检查示例顺序。"
    };
    const spawn: MessageBlock = {
      id: "spawn-anchor-fixture", role: "tool", kind: "function_call", tool_name: "spawn_agent",
      questions: [], subagent: agent
    };
    const beforeCursor = includeHistory ? "b:2" : null;
    const totalBlocks = includeHistory ? 5 : 3;
    const slot = applyThreadDetailToSlot(store, threadId, {
      ...detail(threadId, [spawn, block("before-native-fixture"), block("tail-fixture")], beforeCursor),
      total_blocks: totalBlocks
    });
    if (includeHistory) {
      applyThreadBlockPageToSlot(store, threadId, {
        thread_id: threadId,
        blocks: [block("old-before-fixture"), block("old-after-fixture")],
        total_blocks: totalBlocks, has_more_blocks: false, before_cursor: null
      }, "b:2");
    }
    if (includeRealtimeTail) {
      applyRealtimeBlocksToThreadSlot(store, threadId, [block("sse-tail-fixture", "新近实时回复示例")]);
    }
    const refreshedTail = () => ({
      ...detail(threadId, [
        block("before-native-fixture"),
        {
          id: spawn.id, role: "tool", kind: "subagent_activity", questions: [],
          subagent: { ...agent, eventKind: "started", eventId: "native-start-fixture" }
        },
        block("tail-fixture")
      ], beforeCursor),
      total_blocks: totalBlocks
    });

    applyThreadDetailToSlot(store, threadId, refreshedTail());

    expect(slot.blocks.map(item => item.id)).toEqual([
      ...(includeHistory ? ["old-before-fixture", "old-after-fixture"] : []),
      "before-native-fixture", "spawn-anchor-fixture", "tail-fixture",
      ...(includeRealtimeTail ? ["sse-tail-fixture"] : [])
    ]);
    expect(slot.blocks.find(item => item.id === spawn.id)).toMatchObject({
      kind: "subagent_activity",
      subagent: { ...agent, eventKind: "started", eventId: "native-start-fixture" }
    });
    if (includeRealtimeTail) {
      expect(slot.blocks[slot.blocks.length - 1]?.text).toBe("新近实时回复示例");
    }
    const stableBlocks = slot.blocks;
    const stableFollowRevision = slot.bottomFollowRevision;

    applyThreadDetailToSlot(store, threadId, refreshedTail());

    expect(slot.blocks).toBe(stableBlocks);
    expect(slot.bottomFollowRevision).toBe(stableFollowRevision);
  });

  test.each(["completed", "failed", "interrupted", "unknown"] as const)("a tail-only refresh updates an older child to %s without losing its delegation or position", status => {
    const store = createThreadMessageStoreState();
    const agent: SubagentActivity = {
      agentId: "child-fixture", name: "示例子智能体", status: "running", available: true,
      delegation: "仅检查示例文件并报告结果。"
    };
    const spawn: MessageBlock = {
      id: "old-spawn", role: "tool", kind: "function_call", tool_name: "spawn_agent", questions: [], subagent: agent
    };
    const tail = { ...detail("thread-a", [block("tail")], "b:80"), total_blocks: 81 };
    applyThreadDetailToSlot(store, "thread-a", tail);
    applyThreadBlockPageToSlot(store, "thread-a", {
      thread_id: "thread-a", blocks: [block("old-before"), spawn, block("old-after")],
      total_blocks: 81, has_more_blocks: true, before_cursor: "b:77"
    }, "b:80");
    applyThreadDetailToSlot(store, "thread-b", detail("thread-b", [spawn]));

    const update: SubagentActivity = { agentId: agent.agentId, name: agent.name, status, available: true };
    const slot = applyThreadDetailToSlot(store, "thread-a", {
      ...tail, blocks: [block("tail", "更新后的末页回复")], subagent_updates: { "old-spawn": update }
    });

    expect(slot.blocks.map(item => item.id)).toEqual(["old-before", "old-spawn", "old-after", "tail"]);
    expect(slot.blocks[1].subagent).toMatchObject({ ...update, delegation: agent.delegation });
    expect(slot.blocks[3].text).toBe("更新后的末页回复");
    expect(slot.beforeCursor).toBe("b:77");
    expect(store.slots.get("thread-b")?.blocks[0].subagent?.status).toBe("running");
    expect(spawn.subagent?.status).toBe("running");
  });

  test("lightweight child updates cannot insert an unloaded block or turn ordinary text into activity", () => {
    const store = createThreadMessageStoreState();
    const tail = detail("thread-a", [block("ordinary"), block("tail")]);
    applyThreadDetailToSlot(store, "thread-a", tail);
    const update: SubagentActivity = { agentId: "child-fixture", name: "示例子智能体", status: "completed", available: true };
    const slot = applyThreadDetailToSlot(store, "thread-a", {
      ...tail, subagent_updates: { "unloaded-spawn": update, ordinary: update }
    });
    expect(slot.blocks.map(item => item.id)).toEqual(["ordinary", "tail"]);
    expect(slot.blocks[0].subagent).toBeUndefined();
    expect(slot.blocks[0].text).toBe("ordinary");
  });

  test("direct child collections round-trip with the captured thread and remain isolated from another selection", () => {
    const store = createThreadMessageStoreState();
    const collection = subagents("creating");
    const first = applyThreadDetailToSlot(store, "thread-a", { ...detail("thread-a", [block("a1")]), subagents: collection });
    setActiveThreadSlot(store, "thread-b");
    const second = applyThreadDetailToSlot(store, "thread-b", detail("thread-b", [block("b1")]));

    expect(threadDetailFromMessageSlot("thread-a", first).subagents).toEqual(collection);
    expect(threadDetailFromMessageSlot("thread-b", second).subagents).toBeUndefined();
    expect(store.activeThreadId).toBe("thread-b");
  });

  test("a collection-only refresh updates counts without following the message stream to the bottom", () => {
    const store = createThreadMessageStoreState();
    const tail = detail("thread-a", [block("unchanged")]);
    const slot = applyThreadDetailToSlot(store, "thread-a", { ...tail, subagents: subagents() });
    const followRevision = slot.bottomFollowRevision;
    const visibleRevision = slot.visibleUpdateRevision;
    const completed = subagents("completed");

    applyThreadDetailToSlot(store, "thread-a", { ...tail, subagents: completed });

    expect(threadDetailFromMessageSlot("thread-a", slot).subagents).toEqual(completed);
    expect(slot.visibleUpdateRevision).toBeGreaterThan(visibleRevision);
    expect(slot.bottomFollowRevision).toBe(followRevision);
  });

  test("older pages use the latest known child state while keeping native event identity and delegation", () => {
    const store = createThreadMessageStoreState();
    const historical: SubagentActivity = {
      agentId: "child-fixture", name: "旧示例名称", status: "running", available: true,
      eventKind: "started", eventId: "start-fixture", delegation: "检查示例文件。"
    };
    const updated: SubagentActivity = {
      agentId: "child-fixture", name: "规范示例名称", status: "completed", available: true
    };
    const collection = subagents("completed");
    const tail = {
      ...detail("thread-a", [block("latest")], "b:2"), total_blocks: 3,
      subagents: collection, subagent_updates: { "old-start": updated }
    };
    applyThreadDetailToSlot(store, "thread-a", tail);
    const slot = applyThreadBlockPageToSlot(store, "thread-a", {
      thread_id: "thread-a", total_blocks: 3, has_more_blocks: false, before_cursor: null,
      blocks: [block("before-start"), { id: "old-start", role: "tool", kind: "subagent_activity", questions: [], subagent: historical }]
    }, "b:2");

    expect(slot.blocks.map(item => item.id)).toEqual(["before-start", "old-start", "latest"]);
    expect(slot.blocks[1].subagent).toMatchObject({ ...historical, ...updated });
    expect(threadDetailFromMessageSlot("thread-a", slot).subagents).toEqual(collection);
    expect(historical.status).toBe("running");
    expect(historical.name).toBe("旧示例名称");
  });

  test("each historical event retains its own kind and identity when current child state changes", () => {
    const store = createThreadMessageStoreState();
    const kinds = ["started", "interacted", "interrupted", "completed"] as const;
    const events = kinds.map((eventKind, index): MessageBlock => ({
      id: `activity-${index}`, role: "tool", kind: "subagent_activity", questions: [],
      subagent: {
        agentId: "child-fixture", name: "旧示例名称", status: "running", available: true,
        eventKind, eventId: `native-${index}`
      }
    }));
    applyThreadDetailToSlot(store, "thread-a", detail("thread-a", events));
    const update: SubagentActivity = { agentId: "child-fixture", name: "规范示例名称", status: "completed", available: true };
    const slot = applyThreadDetailToSlot(store, "thread-a", {
      ...detail("thread-a", [block("tail")]),
      subagent_updates: Object.fromEntries(events.map(event => [event.id, update]))
    });

    expect(slot.blocks.slice(0, events.length).map(event => event.subagent?.eventKind)).toEqual(kinds);
    expect(slot.blocks.slice(0, events.length).map(event => event.subagent?.eventId)).toEqual(kinds.map((_, index) => `native-${index}`));
    for (const event of slot.blocks.slice(0, events.length)) expect(event.subagent).toMatchObject(update);
  });

  test("an explicit empty collection replaces previous children and partial results retain their warning", () => {
    const store = createThreadMessageStoreState();
    const tail = detail("thread-a", [block("tail")]);
    const slot = applyThreadDetailToSlot(store, "thread-a", { ...tail, subagents: subagents() });
    const partial = { ...subagents("unknown"), complete: false, warning: "示例关联记录暂不可读" };
    applyThreadDetailToSlot(store, "thread-a", { ...tail, subagents: partial });
    expect(threadDetailFromMessageSlot("thread-a", slot).subagents).toEqual(partial);

    const empty: SubagentCollection = {
      agents: [], counts: { creating: 0, running: 0, completed: 0, failed: 0, interrupted: 0, unknown: 0 }, complete: true
    };
    applyThreadDetailToSlot(store, "thread-a", { ...tail, subagents: empty });
    expect(threadDetailFromMessageSlot("thread-a", slot).subagents).toEqual(empty);
  });

  test("an older service without a collection remains distinguishable from a confirmed empty collection", () => {
    const store = createThreadMessageStoreState();
    const slot = applyThreadDetailToSlot(store, "thread-a", detail("thread-a", [block("legacy")]));
    expect(threadDetailFromMessageSlot("thread-a", slot).subagents).toBeUndefined();
    expect(slot.blocks.map(item => item.text)).toEqual(["legacy"]);
  });

  test("a mismatched detail cannot replace the selected thread's collection", () => {
    const store = createThreadMessageStoreState();
    const collection = subagents();
    const slot = applyThreadDetailToSlot(store, "thread-a", { ...detail("thread-a", [block("tail")]), subagents: collection });
    applyThreadDetailToSlot(store, "thread-a", { ...detail("thread-b", []), subagents: subagents("failed") });
    expect(threadDetailFromMessageSlot("thread-a", slot).subagents).toEqual(collection);
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

  test("a refreshed tail does not restore pagination after all blocks are loaded", () => {
    const store = createThreadMessageStoreState();
    const tail = { ...detail("thread-a", [block("new")], "b:1"), total_blocks: 2 };
    applyThreadDetailToSlot(store, "thread-a", tail);
    applyThreadBlockPageToSlot(store, "thread-a", { thread_id: "thread-a", blocks: [block("old")], total_blocks: 2, has_more_blocks: false, before_cursor: null }, "b:1");
    const slot = applyThreadDetailToSlot(store, "thread-a", tail);
    expect(slot.blocks.map(item => item.id)).toEqual(["old", "new"]);
    expect(slot.hasMoreBlocks).toBe(false);
    expect(slot.beforeCursor).toBeNull();
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

  test("stores feedback per captured thread", () => {
    const store = createThreadMessageStoreState();

    setThreadFeedback(store, "thread-a", "submitted");
    setActiveThreadSlot(store, "thread-b");

    expect(store.slots.get("thread-a")?.feedback).toBe("submitted");
    expect(store.slots.get("thread-b")?.feedback).toBeNull();
  });

  test("clears archived active slot without disturbing other thread slots", () => {
    const store = createThreadMessageStoreState();
    applyThreadDetailToSlot(store, "thread-a", detail("thread-a", [block("a1")]));
    applyThreadDetailToSlot(store, "thread-b", detail("thread-b", [block("b1")]));
    setActiveThreadSlot(store, "thread-a");

    clearThreadSlot(store, "thread-a");

    expect(store.activeThreadId).toBeNull();
    expect(store.slots.get("thread-a")).toBeUndefined();
    expect(store.slots.get("thread-b")?.blocks.map((item) => item.id)).toEqual(["b1"]);
  });

  test("tracks bottom-follow revisions only for message appends and explicit follow actions", () => {
    const store = createThreadMessageStoreState();
    const slot = applyThreadDetailToSlot(store, "thread-a", detail("thread-a", [block("a1")]));
    const initialFollowRevision = slot.bottomFollowRevision;

    setThreadFeedback(store, "thread-a", "status only");
    applyThreadBlockPageToSlot(store, "thread-a", {
      thread_id: "thread-a",
      blocks: [block("older")],
      total_blocks: 2,
      has_more_blocks: false,
      before_cursor: null
    });

    expect(store.slots.get("thread-a")?.bottomFollowRevision).toBe(initialFollowRevision);

    applyRealtimeBlocksToThreadSlot(store, "thread-a", [block("a2")]);
    expect(store.slots.get("thread-a")?.bottomFollowRevision).toBe((initialFollowRevision ?? 0) + 1);
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

  test("keeps a real thread title when detail refresh sends placeholder, plan, or long assistant body titles", () => {
    const store = createThreadMessageStoreState();
    applyThreadDetailToSlot(store, "thread-a", {
      ...detail("thread-a", [block("a1")]),
      summary: { ...summary("thread-a"), title: "真实标题" }
    });

    for (const incomingTitle of [
      "读取中",
      "<proposed_plan>1. 检查\n2. 修复</proposed_plan>",
      "1. 检查缓存\n2. 清理归档状态\n3. 运行回归测试",
      "我会先检查现有线程缓存和消息存储行为，然后补上归档后的缓存清理逻辑，最后运行测试确认不会再把 assistant 正文当成标题。"
    ]) {
      applyThreadDetailToSlot(store, "thread-a", {
        ...detail("thread-a", [block(`refresh-${incomingTitle.length}`)]),
        summary: { ...summary("thread-a"), title: incomingTitle }
      });
      expect(store.slots.get("thread-a")?.summary?.title).toBe("真实标题");
    }
  });

  test("short real titles can replace placeholders while assistant prose cannot", () => {
    const store = createThreadMessageStoreState();
    applyThreadDetailToSlot(store, "thread-a", {
      ...detail("thread-a", [block("a1")]),
      summary: { ...summary("thread-a"), title: "Untitled Thread" }
    });

    applyThreadDetailToSlot(store, "thread-a", {
      ...detail("thread-a", [block("a2")]),
      summary: { ...summary("thread-a"), title: "计划确认修复" }
    });

    expect(store.slots.get("thread-a")?.summary?.title).toBe("计划确认修复");

    applyThreadDetailToSlot(store, "thread-a", {
      ...detail("thread-a", [block("a3")]),
      summary: {
        ...summary("thread-a"),
        title: "先补失败测试覆盖 Goal 可用态和归档缓存，然后实现最小修复并运行 webui 测试。"
      }
    });

    expect(store.slots.get("thread-a")?.summary?.title).toBe("计划确认修复");
  });
});
