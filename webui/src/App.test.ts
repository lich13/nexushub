import { describe, expect, test } from "vitest";
import type { GoalModeState, ThreadSummary } from "./types";

type AppExports = typeof import("./App") & {
  composerFileInputAcceptValue?: () => string | undefined;
  formatGoalStatus?: (goal: Pick<GoalModeState, "enabled" | "status"> | null | undefined) => string;
  segmentInternalReferences?: (text: string) => Array<{ type: "text" | "internal_reference"; text: string; copyText?: string; kind?: string }>;
  slashCommandSuggestions?: (draft: string, cursor: number) => Array<{ command: string; description: string }>;
  applySlashCommandSelection?: (draft: string, cursor: number, command: string) => { value: string; cursor: number };
  nextRenameDraftValue?: (input: {
    previousThreadId: string;
    threadId: string;
    currentDraft: string;
    incomingTitle: string;
    dirty: boolean;
  }) => string;
  mergeSavedThreadTitle?: (threads: ThreadSummary[], threadId: string, title: string) => ThreadSummary[];
};

async function loadApp(): Promise<AppExports> {
  return import("./App") as Promise<AppExports>;
}

describe("conversation helpers", () => {
  test("segments internal paths and Codex ids as copyable references", async () => {
    const app = await loadApp();
    const segments = app.segmentInternalReferences?.(
      "请看 /Users/gosu/.codex/sessions/2026/06/15/rollout-019ec943.jsonl 和 thread 019ec943-0b86-7e22-86e9-4dc0c919b09d 的 turn turn-live。"
    );

    expect(segments).toEqual(expect.arrayContaining([
      expect.objectContaining({
        type: "internal_reference",
        kind: "path",
        text: "/Users/gosu/.codex/sessions/2026/06/15/rollout-019ec943.jsonl",
        copyText: "/Users/gosu/.codex/sessions/2026/06/15/rollout-019ec943.jsonl"
      }),
      expect.objectContaining({
        type: "internal_reference",
        kind: "thread",
        text: "thread 019ec943-0b86-7e22-86e9-4dc0c919b09d",
        copyText: "thread 019ec943-0b86-7e22-86e9-4dc0c919b09d"
      }),
      expect.objectContaining({
        type: "internal_reference",
        kind: "turn",
        text: "turn turn-live",
        copyText: "turn turn-live"
      })
    ]));
  });

  test("slash command helpers include goal resume and insert without submitting", async () => {
    const app = await loadApp();

    expect(app.slashCommandSuggestions?.("/go", 3)).toEqual([
      expect.objectContaining({ command: "/goal resume", description: "恢复当前线程 Goal" })
    ]);
    expect(app.applySlashCommandSelection?.("继续 /go", 6, "/goal resume")).toEqual({
      value: "继续 /goal resume ",
      cursor: "继续 /goal resume ".length
    });
  });

  test("title refresh keeps a dirty local edit for the same thread but syncs on thread switch", async () => {
    const app = await loadApp();

    expect(app.nextRenameDraftValue?.({
      previousThreadId: "thread-a",
      threadId: "thread-a",
      currentDraft: "我正在输入的新标题",
      incomingTitle: "服务端旧标题",
      dirty: true
    })).toBe("我正在输入的新标题");

    expect(app.nextRenameDraftValue?.({
      previousThreadId: "thread-a",
      threadId: "thread-b",
      currentDraft: "我正在输入的新标题",
      incomingTitle: "thread-b 标题",
      dirty: true
    })).toBe("thread-b 标题");
  });

  test("saved title updates thread list cache values without waiting for a refetch", async () => {
    const app = await loadApp();
    const threads: ThreadSummary[] = [
      { id: "thread-a", title: "旧标题", status: "Recent", message_count: 1 },
      { id: "thread-b", title: "其他", status: "Recent", message_count: 1 }
    ];

    expect(app.mergeSavedThreadTitle?.(threads, "thread-a", "新标题")).toEqual([
      { id: "thread-a", title: "新标题", status: "Recent", message_count: 1 },
      { id: "thread-b", title: "其他", status: "Recent", message_count: 1 }
    ]);
  });

  test("composer file picker has no accept whitelist", async () => {
    const app = await loadApp();

    expect(app.composerFileInputAcceptValue?.()).toBeUndefined();
  });

  test("goal status labels normalize known app-server states in Chinese", async () => {
    const app = await loadApp();

    expect(app.formatGoalStatus?.({ enabled: true, status: "running" })).toBe("运行中");
    expect(app.formatGoalStatus?.({ enabled: true, status: "completed" })).toBe("已完成");
    expect(app.formatGoalStatus?.({ enabled: true, status: "blocked" })).toBe("已阻塞");
    expect(app.formatGoalStatus?.({ enabled: false, status: "missing_thread" })).toBe("未选择线程");
    expect(app.formatGoalStatus?.({ enabled: false, status: "cleared" })).toBe("已清除");
  });
});
