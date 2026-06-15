import { describe, expect, test } from "vitest";
import type { GoalModeState, ThreadSummary } from "./types";

type AppExports = typeof import("./App") & {
  composerFileInputAcceptValue?: () => string | undefined;
  formatGoalStatus?: (goal: Pick<GoalModeState, "enabled" | "status"> | null | undefined) => string;
  segmentInternalReferences?: (text: string) => Array<{ type: "text" | "internal_reference"; text: string; copyText?: string; kind?: string }>;
  slashCommands?: Array<{ command: string; description: string; usageHint: string; requiresThread?: boolean }>;
  slashCommandSuggestions?: (draft: string, cursor: number, hasThread?: boolean) => Array<{ command: string; description: string; usageHint: string; requiresThread?: boolean }>;
  applySlashCommandSelection?: (draft: string, cursor: number, command: string) => { value: string; cursor: number };
  renderSlashCommandMenuHtml?: (draft: string, cursor: number, hasThread?: boolean, selected?: number) => string;
  nextSlashCommandSelection?: (current: number, total: number, key: string) => number;
  slashCommandKeyAction?: (input: {
    key: string;
    shiftKey?: boolean;
    selected: number;
    suggestions: Array<{ command: string }>;
  }) => { action: "move"; selected: number } | { action: "insert"; command: string } | { action: "dismiss" } | { action: "none" };
  nextRenameDraftValue?: (input: {
    previousThreadId: string;
    threadId: string;
    currentDraft: string;
    incomingTitle: string;
    dirty: boolean;
  }) => string;
  mergeSavedThreadTitle?: (threads: ThreadSummary[], threadId: string, title: string) => ThreadSummary[];
  threadSettingsMetricLabels?: () => string[];
  threadResumeCommand?: (threadId?: string | null) => string | null;
  probeStatusThreads?: (status?: { running_threads?: ThreadSummary[]; reply_needed_threads?: ThreadSummary[]; recoverable_threads?: ThreadSummary[] } | null) => ThreadSummary[];
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

  test("slash command catalog covers Codex TUI commands with Chinese descriptions and usage hints", async () => {
    const app = await loadApp();

    const requiredCommands = [
      "/permissions", "/ide", "/keymap", "/vim", "/sandbox-add-read-dir", "/agent", "/apps", "/plugins", "/hooks",
      "/clear", "/archive", "/compact", "/copy", "/diff", "/exit", "/quit", "/experimental", "/approve",
      "/memories", "/skills", "/feedback", "/init", "/logout", "/mcp", "/mention", "/model", "/fast", "/plan",
      "/goal", "/goal pause", "/goal resume", "/goal clear", "/personality", "/ps", "/stop", "/fork", "/side",
      "/btw", "/raw", "/resume", "/new", "/review", "/status", "/debug-config", "/statusline", "/title", "/theme"
    ];
    const commands = app.slashCommands?.map((item) => item.command);

    expect(commands).toEqual(requiredCommands);
    expect(app.slashCommands?.every((item) => item.description.trim().length > 0)).toBe(true);
    expect(app.slashCommands?.every((item) => /[\u4e00-\u9fff]/.test(item.description))).toBe(true);
    expect(app.slashCommands?.every((item) => item.usageHint.trim().length > 0)).toBe(true);
    expect(app.slashCommands?.filter((item) => item.command.startsWith("/goal")).every((item) => item.requiresThread)).toBe(true);
  });

  test("slash command helpers filter full catalog and keep thread commands visible for new threads", async () => {
    const app = await loadApp();

    expect(app.slashCommandSuggestions?.("/", 1).map((item) => item.command)).toEqual(app.slashCommands?.map((item) => item.command));
    expect(app.slashCommandSuggestions?.("/go", 3).map((item) => item.command)).toEqual([
      "/goal", "/goal pause", "/goal resume", "/goal clear"
    ]);
    expect(app.slashCommandSuggestions?.("/goal", 5).map((item) => item.command)).toEqual([
      "/goal", "/goal pause", "/goal resume", "/goal clear"
    ]);
    expect(app.slashCommandSuggestions?.("/goal r", 7)).toEqual([
      expect.objectContaining({
        command: "/goal resume",
        description: expect.stringContaining("恢复"),
        usageHint: expect.stringContaining("/goal resume"),
        requiresThread: true
      })
    ]);
    expect(app.slashCommandSuggestions?.("/goal resume", "/goal resume".length, false)).toEqual([
      expect.objectContaining({ command: "/goal resume", requiresThread: true })
    ]);
    expect(app.slashCommandSuggestions?.("/theme", 6)).toEqual([
      expect.objectContaining({ command: "/theme", description: expect.stringContaining("主题") })
    ]);
  });

  test("slash command menu renders listbox with command, Chinese explanation, usage, and thread marker", async () => {
    const app = await loadApp();

    const html = app.renderSlashCommandMenuHtml?.("/goal r", 7, false);

    expect(html).toContain('role="listbox"');
    expect(html).toContain("/goal resume");
    expect(html).toContain("恢复");
    expect(html).toContain("用法");
    expect(html).toContain("需要已有线程");
  });

  test("slash command selection helpers support keyboard actions and insert without submitting", async () => {
    const app = await loadApp();
    const suggestions = app.slashCommandSuggestions?.("/", 1) ?? [];

    expect(app.nextSlashCommandSelection?.(0, suggestions.length, "ArrowDown")).toBe(1);
    expect(app.nextSlashCommandSelection?.(0, suggestions.length, "ArrowUp")).toBe(suggestions.length - 1);
    expect(app.slashCommandKeyAction?.({ key: "ArrowDown", selected: 0, suggestions })).toEqual({ action: "move", selected: 1 });
    expect(app.slashCommandKeyAction?.({ key: "Escape", selected: 0, suggestions })).toEqual({ action: "dismiss" });
    expect(app.slashCommandKeyAction?.({ key: "Enter", selected: 2, suggestions })).toEqual({ action: "insert", command: suggestions[2].command });
    expect(app.slashCommandKeyAction?.({ key: "Enter", shiftKey: true, selected: 2, suggestions })).toEqual({ action: "none" });
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

  test("thread settings hides internal metrics and copies a resume command from the ID button", async () => {
    const app = await loadApp();

    expect(app.threadSettingsMetricLabels?.()).not.toEqual(expect.arrayContaining([
      "Thread ID",
      "Active turn",
      "Active job",
      "Last event",
      "Rollout path",
      "Blocks"
    ]));
    expect(app.threadResumeCommand?.("019ec943-0b86-7e22-86e9-4dc0c919b09d")).toBe(
      "codex resume 019ec943-0b86-7e22-86e9-4dc0c919b09d"
    );
    expect(app.threadResumeCommand?.("  ")).toBeNull();
    expect(app.threadResumeCommand?.(null)).toBeNull();
  });

  test("probe thread rows use canonical ThreadSummary status values", async () => {
    const app = await loadApp();
    const rows = app.probeStatusThreads?.({
      running_threads: [{ id: "running", title: "run", status: "Running", message_count: 1 }],
      reply_needed_threads: [{ id: "reply", title: "reply", status: "ReplyNeeded", message_count: 1 }],
      recoverable_threads: [{ id: "recoverable", title: "recover", status: "Recoverable", message_count: 1 }]
    }) ?? [];

    expect(rows.map((thread) => thread.status)).toEqual(["Running", "ReplyNeeded", "Recoverable"]);
    expect(rows.map((thread) => app.threadListItemStatusText?.(thread))).toEqual(["运行中", "待回复", "异常"]);
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
