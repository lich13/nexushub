import { QueryClient } from "@tanstack/react-query";
import { describe, expect, test } from "vitest";
import type { CodexGoal, MessageBlock, PluginInfo, ProbeEvent, ThreadSummary } from "./types";

type AppExports = typeof import("./App") & {
  buildPayload?: (message: string, config: Record<string, unknown>, attachments?: Array<{ id: string }>) => Record<string, unknown>;
  composerFileInputAcceptValue?: () => string | undefined;
  composerActionMode?: (running: boolean, draft: string, canStop: boolean, attachmentCount?: number) => string;
  defaultRunConfig?: () => Record<string, unknown>;
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
  pluginMentionSuggestions?: (draft: string, cursor: number, plugins?: PluginInfo[] | null, unavailable?: boolean) => Array<{ id: string; label: string; description: string; unavailableReason?: string | null }>;
  applyPluginMentionSelection?: (draft: string, cursor: number, plugin: Pick<PluginInfo, "id" | "label" | "invocation_template">) => { value: string; cursor: number };
  renderPluginMentionMenuHtml?: (draft: string, cursor: number, plugins?: PluginInfo[] | null, unavailable?: boolean, selected?: number) => string;
  activeComposerMenuKind?: (draft: string, cursor: number, plugins?: PluginInfo[] | null) => "slash" | "plugin" | null;
  exactSlashCommandFromDraft?: (draft: string) => string | null;
  slashCommandForComposerSubmit?: (draft: string) => string | null;
  composerSubmitDraftValue?: (stateValue: string, domValue?: string | null) => string;
  composerMenuKeyAction?: (input: {
    key: string;
    shiftKey?: boolean;
    composing?: boolean;
    menuSelectionArmed?: boolean;
    selected: number;
    suggestions: Array<{ command?: string; id?: string }>;
  }) => { action: "move"; selected: number } | { action: "insert"; index: number } | { action: "dismiss" } | { action: "none" };
  slashCommandAction?: (command: string, hasThread?: boolean) => { kind: string; message?: string; command?: string };
  planModeButtonState?: (nextMessagePlan: boolean, threadStatus?: string, hasPendingPlan?: boolean, hasPendingQuestion?: boolean) => { pressed: boolean; label: string; statusText: string };
  mergeRunConfigFromDefaults?: <T extends { collaborationMode: string }>(current: T, defaults: T) => T;
  runConfigAfterSuccessfulSend?: <T extends { collaborationMode: string }>(config: T) => T;
  latestAssistantCopyText?: (blocks: MessageBlock[]) => string | null;
  nextRenameDraftValue?: (input: {
    previousThreadId: string;
    threadId: string;
    currentDraft: string;
    incomingTitle: string;
    dirty: boolean;
  }) => string;
  mergeIncomingThreadSummary?: <T extends Partial<ThreadSummary>>(current: T, incoming: Partial<ThreadSummary>) => T & Partial<ThreadSummary>;
  mergeSavedThreadTitle?: (threads: ThreadSummary[], threadId: string, title: string) => ThreadSummary[];
  threadSettingsMetricLabels?: () => string[];
  threadResumeCommand?: (threadId?: string | null) => string | null;
  probeStatusThreads?: (status?: { running_threads?: ThreadSummary[]; reply_needed_threads?: ThreadSummary[]; recoverable_threads?: ThreadSummary[] } | null) => ThreadSummary[];
  probeAvailabilityView?: (input: {
    available?: boolean;
    probeEnabled?: boolean;
    loading?: boolean;
    fetching?: boolean;
    hasData?: boolean;
  }) => { headline: string; metric: string; tone: "success" | "warning" | "danger" };
  probeEventSummary?: (event: ProbeEvent) => string;
  probeEventCard?: (event: ProbeEvent) => { headline: string; summary: string; details: Array<{ label: string; value: string }> };
  shouldAutoScrollProbeFeed?: (
    current: { scrollTop: number; clientHeight: number; scrollHeight: number },
    previous?: { scrollTop: number; clientHeight: number; scrollHeight: number } | null
  ) => boolean;
  nextVisibleThreadIdAfterRemoval?: (threads: ThreadSummary[], removedThreadId: string) => string | null;
  shouldHydrateThreadDetail?: (threadId: string | null | undefined, detail?: { summary: ThreadSummary } | null) => boolean;
  clearArchivedThreadClientState?: (qc: QueryClient, messageStore: { clear: (threadId: string) => void }, threadId: string) => void;
  applyOptimisticThreadTitle?: (qc: QueryClient, threadId: string, title: string) => unknown;
  rollbackOptimisticThreadTitle?: (qc: QueryClient, snapshot: unknown) => void;
  applyOptimisticThreadArchive?: (qc: QueryClient, messageStore: { clear: (threadId: string) => void }, threadId: string) => unknown;
  rollbackOptimisticThreadArchive?: (qc: QueryClient, snapshot: unknown) => void;
  applyOptimisticThreadRestore?: (qc: QueryClient, threadId: string) => unknown;
  rollbackOptimisticThreadRestore?: (qc: QueryClient, snapshot: unknown) => void;
  threadInspectorPanelTitles?: () => string[];
  setLocalThreadTitleOverride?: (threadId: string, title: string, now?: number) => void;
  clearLocalThreadTitleOverride?: (threadId: string) => void;
  applyThreadTitleOverride?: <T extends Partial<ThreadSummary>>(summary: T, now?: number) => T;
  goalStatusLabel?: (goal: CodexGoal | undefined, loading: boolean) => string;
  goalStatusTone?: (goal: CodexGoal | undefined) => "success" | "warning" | "danger" | undefined;
  goalControlState?: (goal: CodexGoal | undefined, options?: { busy?: boolean; objective?: string; tokenBudget?: string }) => {
    saveDisabled: boolean;
    clearDisabled: boolean;
    pauseDisabled: boolean;
    resumeDisabled: boolean;
  };
  formatGoalTimestamp?: (value: number | string | null | undefined) => string;
  codexVisibleCopy?: () => Record<string, string>;
  failureCategoryLabel?: (category: string) => string;
  optionalUnavailableMessage?: (feature: string, result?: { available: boolean; reason?: string | null; error?: string | null } | null) => string;
  renderConversationHeaderHtml?: (summary: ThreadSummary) => string;
  preservePreviousQueryData?: <T>(previous: T | undefined) => T | undefined;
  threadCopyId?: (threadId?: string | null) => string | null;
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
    expect(app.segmentInternalReferences?.("goal abc123")).toEqual([{ type: "text", text: "goal abc123" }]);
  });

  test("slash command catalog covers Codex TUI commands with Chinese descriptions and usage hints", async () => {
    const app = await loadApp();

    const requiredCommands = [
      "/permissions", "/ide", "/keymap", "/vim", "/sandbox-add-read-dir", "/agent", "/apps", "/plugins", "/hooks",
      "/clear", "/archive", "/compact", "/copy", "/diff", "/exit", "/quit", "/experimental", "/approve",
      "/memories", "/skills", "/feedback", "/init", "/logout", "/mcp", "/mention", "/model", "/fast", "/plan",
      "/personality", "/ps", "/stop", "/fork", "/side", "/btw", "/raw", "/resume", "/new", "/review", "/status",
      "/debug-config", "/statusline", "/title", "/theme"
    ];
    const commands = app.slashCommands?.map((item) => item.command);

    expect(commands).toEqual(requiredCommands);
    expect(app.slashCommands?.every((item) => item.description.trim().length > 0)).toBe(true);
    expect(app.slashCommands?.every((item) => /[\u4e00-\u9fff]/.test(item.description))).toBe(true);
    expect(app.slashCommands?.every((item) => item.usageHint.trim().length > 0)).toBe(true);
    expect(app.slashCommands?.some((item) => item.command.startsWith("/goal"))).toBe(false);
  });

  test("slash command helpers filter full catalog and keep thread commands visible for new threads", async () => {
    const app = await loadApp();
    const removedGoalResume = ["/goal", "resume"].join(" ");

    expect(app.slashCommandSuggestions?.("/", 1).map((item) => item.command)).toEqual(app.slashCommands?.map((item) => item.command));
    expect(app.slashCommandSuggestions?.("/go", 3).map((item) => item.command)).toEqual([]);
    expect(app.slashCommandSuggestions?.("/goal", 5).map((item) => item.command)).toEqual([]);
    expect(app.slashCommandSuggestions?.("/goal r", 7)).toEqual([]);
    expect(app.slashCommandSuggestions?.(removedGoalResume, removedGoalResume.length, false)).toEqual([]);
    expect(app.renderSlashCommandMenuHtml?.("/", 1, false)).not.toContain(removedGoalResume);
    expect(app.slashCommandSuggestions?.("/theme", 6)).toEqual([
      expect.objectContaining({ command: "/theme", description: expect.stringContaining("主题") })
    ]);
  });

  test("slash command menu renders listbox with command, Chinese explanation, usage, and thread marker", async () => {
    const app = await loadApp();

    const html = app.renderSlashCommandMenuHtml?.("/archive", 8, false);

    expect(html).toContain('role="listbox"');
    expect(html).toContain("/archive");
    expect(html).toContain("归档");
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
    expect(app.applySlashCommandSelection?.("继续 /pl", 6, "/plan")).toEqual({
      value: "继续 /plan ",
      cursor: "继续 /plan ".length
    });
  });

  test("@ plugin mention helpers filter, insert, render fallback, and ignore non-trigger contexts", async () => {
    const app = await loadApp();
    const plugins: PluginInfo[] = [
      { id: "probe", label: "Probe", status: "ready", kind: "builtin", description: "探针状态和维护" },
      { id: "plugins", label: "Plugins", status: "preview", kind: "builtin", description: "插件列表" },
      { id: "claude-code", label: "Claude Code", status: "planned", kind: "builtin", unavailable_reason: "当前仅支持只读预览" }
    ];

    expect(app.pluginMentionSuggestions?.("@", 1, plugins).map((item) => item.id)).toEqual(["probe", "plugins", "claude-code"]);
    expect(app.pluginMentionSuggestions?.("@p", 2, plugins).map((item) => item.id)).toEqual(["probe", "plugins"]);
    expect(app.pluginMentionSuggestions?.("mail a@b.com", "mail a@b.com".length, plugins)).toEqual([]);
    expect(app.pluginMentionSuggestions?.("const x='@p'", "const x='@p'".length, plugins)).toEqual([]);
    expect(app.pluginMentionSuggestions?.("@p", 2, null, true)).toEqual([
      expect.objectContaining({ id: "__plugins_unavailable__", description: expect.stringContaining("当前无法读取插件列表") })
    ]);

    expect(app.applyPluginMentionSelection?.("请调用 @p", "请调用 @p".length, plugins[0])).toEqual({
      value: "请调用 @Probe ",
      cursor: "请调用 @Probe ".length
    });
    expect(app.renderPluginMentionMenuHtml?.("@claude", 7, plugins)).toContain("当前仅支持只读预览");
  });

  test("composer menu kind gives the nearest valid trigger priority and shares TUI key semantics", async () => {
    const app = await loadApp();
    const plugins: PluginInfo[] = [{ id: "probe", label: "Probe", status: "ready", kind: "builtin" }];

    expect(app.activeComposerMenuKind?.("/", 1, plugins)).toBe("slash");
    expect(app.activeComposerMenuKind?.("/goal @p", "/goal @p".length, plugins)).toBe("plugin");
    expect(app.activeComposerMenuKind?.("@probe /go", "@probe /go".length, plugins)).toBe("slash");
    expect(app.composerMenuKeyAction?.({ key: "Enter", composing: true, selected: 0, suggestions: [{ id: "probe" }] })).toEqual({ action: "none" });
    expect(app.composerMenuKeyAction?.({ key: "Enter", selected: 0, suggestions: [{ id: "probe" }] })).toEqual({ action: "none" });
    expect(app.composerMenuKeyAction?.({ key: "Enter", menuSelectionArmed: true, selected: 0, suggestions: [{ id: "probe" }] })).toEqual({ action: "insert", index: 0 });
    expect(app.composerMenuKeyAction?.({ key: "Tab", selected: 0, suggestions: [{ id: "probe" }] })).toEqual({ action: "insert", index: 0 });
    expect(app.composerMenuKeyAction?.({ key: "Enter", shiftKey: true, selected: 0, suggestions: [{ id: "probe" }] })).toEqual({ action: "none" });
  });

  test("composer submit sends partial slash text literally instead of accepting visible suggestions", async () => {
    const app = await loadApp();
    const config = app.defaultRunConfig?.() ?? {};
    const cases = ["/go", "/plugins 文本", "/plan 文本"];

    for (const draft of cases) {
      expect(app.activeComposerMenuKind?.(draft, draft.length, [])).toBe("slash");
      expect(app.composerMenuKeyAction?.({
        key: "Enter",
        selected: 0,
        suggestions: app.slashCommandSuggestions?.(draft, draft.length) ?? []
      })).toEqual({ action: "none" });
      expect(app.slashCommandForComposerSubmit?.(draft)).toBeNull();
      expect(app.buildPayload?.(draft, config).message).toBe(draft);
    }
  });

  test("composer submit uses the textarea DOM value when React state lags behind", async () => {
    const app = await loadApp();

    const currentDraft = app.composerSubmitDraftValue?.("/plugins", "/plugins 文本");

    expect(currentDraft).toBe("/plugins 文本");
    expect(app.slashCommandForComposerSubmit?.(currentDraft ?? "")).toBeNull();
    expect(app.buildPayload?.(currentDraft ?? "", app.defaultRunConfig?.() ?? {}).message).toBe("/plugins 文本");
  });

  test("exact /plugins stays an explicit control command while /plugins text remains a message", async () => {
    const app = await loadApp();

    expect(app.slashCommandForComposerSubmit?.("/plugins")).toBe("/plugins");
    expect(app.slashCommandAction?.("/plugins")).toEqual({ kind: "open_plugins", command: "/plugins" });
    expect(app.slashCommandForComposerSubmit?.("/plugins 文本")).toBeNull();
    expect(app.slashCommandAction?.("/plugins 文本")).toEqual({
      kind: "unknown",
      command: "/plugins 文本",
      message: expect.stringContaining("未知")
    });
  });

  test("IME composition never submits or inserts slash/plugin menu candidates", async () => {
    const app = await loadApp();

    expect(app.composerMenuKeyAction?.({
      key: "Enter",
      composing: true,
      selected: 0,
      suggestions: [{ command: "/goal" }]
    })).toEqual({ action: "none" });
    expect(app.composerMenuKeyAction?.({
      key: "Enter",
      composing: true,
      menuSelectionArmed: true,
      selected: 0,
      suggestions: [{ id: "probe" }]
    })).toEqual({ action: "none" });
  });

  test("exact slash command detection separates execution from partial candidate insertion", async () => {
    const app = await loadApp();

    expect(app.exactSlashCommandFromDraft?.("/plan")).toBe("/plan");
    expect(app.exactSlashCommandFromDraft?.(" /fork ")).toBe("/fork");
    expect(app.exactSlashCommandFromDraft?.("/go")).toBeNull();
    expect(app.exactSlashCommandFromDraft?.("/goal r")).toBeNull();
  });

  test("composer submit only executes complete controlled slash commands", async () => {
    const app = await loadApp();

    expect(app.slashCommandForComposerSubmit?.("/plugins")).toBe("/plugins");
    expect(app.slashCommandForComposerSubmit?.(" /apps ")).toBe("/apps");
    expect(app.slashCommandForComposerSubmit?.("/plan")).toBe("/plan");
    expect(app.slashCommandForComposerSubmit?.("/p")).toBeNull();
    expect(app.slashCommandForComposerSubmit?.("/plugins 请说明")).toBeNull();
    expect(app.slashCommandForComposerSubmit?.("/unknown")).toBeNull();
    expect(app.slashCommandForComposerSubmit?.("/Users/gosu/Documents")).toBeNull();
    expect(app.slashCommandForComposerSubmit?.("/go")).toBeNull();
    expect(app.slashCommandForComposerSubmit?.("/plan 请先分析")).toBeNull();
  });

  test("slash command action classifier only exposes controlled web actions and Chinese unavailable reasons", async () => {
    const app = await loadApp();

    expect(app.slashCommandAction?.("/plan")).toEqual({ kind: "toggle_plan_mode", command: "/plan" });
    expect(app.slashCommandAction?.("/new")).toEqual({ kind: "open_new_thread", command: "/new" });
    expect(app.slashCommandAction?.("/archive")).toEqual({ kind: "archive_thread", command: "/archive" });
    expect(app.slashCommandAction?.("/archive", false)).toEqual({
      kind: "requires_thread",
      command: "/archive",
      message: expect.stringContaining("需要已有线程")
    });
    expect(app.slashCommandAction?.("/theme")).toEqual({
      kind: "unavailable",
      command: "/theme",
      message: expect.stringContaining("Web 端暂不支持")
    });
    expect(app.slashCommandAction?.("/unknown")).toEqual({
      kind: "unknown",
      command: "/unknown",
      message: expect.stringContaining("未知")
    });
  });

  test("plan mode button is a persistent thread send state", async () => {
    const app = await loadApp();
    const config = {
      ...(app.defaultRunConfig?.() ?? {}),
      collaborationMode: "plan"
    };

    expect(app.planModeButtonState?.(true, "Recent", false, false)).toEqual({
      pressed: true,
      label: "Plan Mode",
      statusText: "下一条消息将使用 Plan Mode"
    });
    expect(app.planModeButtonState?.(false, "ReplyNeeded", true, false)).toEqual({
      pressed: false,
      label: "Plan Mode",
      statusText: "当前线程正在等待计划确认"
    });
    expect(app.planModeButtonState?.(false, "ReplyNeeded", false, true)?.statusText).toBe("当前线程正在等待问题回复");
    expect(app.buildPayload?.("请先制定计划", config).collaboration_mode).toBe("plan");
    expect(app.runConfigAfterSuccessfulSend?.({ collaborationMode: "plan", other: "kept" })).toEqual({
      collaborationMode: "plan",
      other: "kept"
    });
    expect(app.runConfigAfterSuccessfulSend?.({ collaborationMode: "", other: "kept" })).toEqual({
      collaborationMode: "",
      other: "kept"
    });
    expect(app.composerActionMode?.(false, "失败后保留的输入", false)).toBe("send");
  });

  test("config refresh merges persistent defaults without clearing next-send Plan Mode", async () => {
    const app = await loadApp();
    const current = {
      model: "gpt-5.5",
      serviceTier: "priority",
      reasoning: "xhigh",
      cwd: "/old",
      permissionPreset: "full",
      permissionProfile: "",
      approvalPolicy: "never",
      sandboxMode: "danger-full-access",
      networkAccess: true,
      collaborationMode: "plan"
    };
    const defaults = {
      ...current,
      model: "gpt-5.4",
      serviceTier: "",
      reasoning: "high",
      cwd: "/new",
      approvalPolicy: "on-request",
      sandboxMode: "workspace-write",
      collaborationMode: ""
    };

    expect(app.mergeRunConfigFromDefaults?.(current, defaults)).toEqual({
      ...defaults,
      collaborationMode: "plan"
    });
    expect(app.runConfigAfterSuccessfulSend?.(app.mergeRunConfigFromDefaults?.(current, defaults) ?? current).collaborationMode).toBe("plan");
  });

  test("latest assistant copy text skips tools, plans, and internal context", async () => {
    const app = await loadApp();
    const blocks: MessageBlock[] = [
      { id: "u1", role: "user", kind: "message", text: "hello", questions: [] },
      { id: "a1", role: "assistant", kind: "message", text: "first reply", questions: [] },
      { id: "t1", role: "tool", kind: "function_call_output", text: "tool output", questions: [] },
      { id: "p1", role: "assistant", kind: "plan", text: "<proposed_plan>ship</proposed_plan>", questions: [] },
      { id: "ctx", role: "assistant", kind: "message", text: "<environment_context>hidden</environment_context>", questions: [] },
      { id: "a2", role: "assistant", kind: "agentMessage", text: "final reply", questions: [] }
    ];

    expect(app.latestAssistantCopyText?.(blocks)).toBe("final reply");
    expect(app.latestAssistantCopyText?.(blocks.slice(0, -1))).toBe("first reply");
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
    const qc = new QueryClient();
    const threads: ThreadSummary[] = [
      { id: "thread-a", title: "旧标题", status: "Recent", message_count: 1 },
      { id: "thread-b", title: "其他", status: "Recent", message_count: 1 }
    ];
    const detail = {
      summary: threads[0],
      messages: [],
      blocks: [],
      raw_event_count: 0
    };

    expect(app.mergeSavedThreadTitle?.(threads, "thread-a", "新标题")).toEqual([
      { id: "thread-a", title: "新标题", status: "Recent", message_count: 1 },
      { id: "thread-b", title: "其他", status: "Recent", message_count: 1 }
    ]);

    qc.setQueryData(["threads", "all", ""], threads);
    qc.setQueryData(["threads", "running", ""], [threads[1]]);
    qc.setQueryData(["thread", "thread-a"], detail);

    const snapshot = app.applyOptimisticThreadTitle?.(qc, "thread-a", "即时标题");

    expect(qc.getQueryData<ThreadSummary[]>(["threads", "all", ""])?.[0].title).toBe("即时标题");
    expect(qc.getQueryData<ThreadSummary[]>(["threads", "running", ""])?.[0].title).toBe("其他");
    expect(qc.getQueryData<{ summary: ThreadSummary }>(["thread", "thread-a"])?.summary.title).toBe("即时标题");

    app.rollbackOptimisticThreadTitle?.(qc, snapshot);

    expect(qc.getQueryData<ThreadSummary[]>(["threads", "all", ""])?.[0].title).toBe("旧标题");
    expect(qc.getQueryData<{ summary: ThreadSummary }>(["thread", "thread-a"])?.summary.title).toBe("旧标题");
  });

  test("thread title merges keep real titles instead of assistant plan or body text", async () => {
    const app = await loadApp();
    const current: ThreadSummary = { id: "thread-a", title: "真实标题", status: "Recent", message_count: 1 };

    for (const incomingTitle of [
      "读取中",
      "<proposed_plan>1. 检查缓存\n2. 修复归档选择</proposed_plan>",
      "1. 检查缓存\n2. 清理归档状态\n3. 运行回归测试",
      "我会先检查现有线程缓存和消息存储行为，然后补上归档后的缓存清理逻辑，最后运行测试确认不会再把 assistant 正文当成标题。"
    ]) {
      expect(app.mergeIncomingThreadSummary?.(current, { ...current, title: incomingTitle }).title).toBe("真实标题");
    }

    expect(app.mergeIncomingThreadSummary?.({ ...current, title: "读取中" }, { ...current, title: "真实新标题" }).title).toBe("真实新标题");
  });

  test("local title override prevents stale refetch from flashing back after rename", async () => {
    const app = await loadApp();
    const current: ThreadSummary = { id: "thread-a", title: "旧标题", status: "Recent", message_count: 1 };

    app.setLocalThreadTitleOverride?.("thread-a", "即时标题");

    expect(app.applyThreadTitleOverride?.({ ...current, title: "旧标题" }).title).toBe("即时标题");
    expect(app.mergeIncomingThreadSummary?.(current, { ...current, title: "旧标题" }).title).toBe("即时标题");

    app.setLocalThreadTitleOverride?.("thread-a", "短期标题", 1000);
    expect(app.applyThreadTitleOverride?.({ ...current, title: "旧标题" }, 200_000).title).toBe("旧标题");

    app.setLocalThreadTitleOverride?.("thread-a", "再次即时");
    app.clearLocalThreadTitleOverride?.("thread-a");

    expect(app.applyThreadTitleOverride?.({ ...current, title: "旧标题" }).title).toBe("旧标题");
  });

  test("archiving selects the next visible thread and clears every cached copy of the archived thread", async () => {
    const app = await loadApp();
    const qc = new QueryClient();
    const threads: ThreadSummary[] = [
      { id: "thread-a", title: "A", status: "Recent", message_count: 1 },
      { id: "thread-b", title: "B", status: "Recent", message_count: 1 },
      { id: "thread-c", title: "C", status: "Recent", message_count: 1 },
      { id: "thread-archived", title: "Archived", status: "Archived", message_count: 1 }
    ];
    const detail = (thread: ThreadSummary) => ({
      summary: thread,
      messages: [],
      blocks: [],
      raw_event_count: 0
    });
    const cleared: string[] = [];

    qc.setQueryData(["threads", "all", ""], threads);
    qc.setQueryData(["threads", "running", ""], [threads[1], threads[2]]);
    qc.setQueryData(["threads", { status: "all", q: "" }], [threads[0], threads[1]]);
    qc.setQueryData(["threads-extra"], [threads[1]]);
    qc.setQueryData(["thread", "thread-b"], detail(threads[1]));
    qc.setQueryData(["thread", "thread-c"], detail(threads[2]));

    expect(app.nextVisibleThreadIdAfterRemoval?.(threads, "thread-b")).toBe("thread-c");
    expect(app.nextVisibleThreadIdAfterRemoval?.(threads, "thread-c")).toBe("thread-b");

    app.clearArchivedThreadClientState?.(qc, { clear: (threadId) => cleared.push(threadId) }, "thread-b");

    expect(qc.getQueryData<ThreadSummary[]>(["threads", "all", ""])?.map((thread) => thread.id)).toEqual(["thread-a", "thread-c", "thread-archived"]);
    expect(qc.getQueryData<ThreadSummary[]>(["threads", "running", ""])?.map((thread) => thread.id)).toEqual(["thread-c"]);
    expect(qc.getQueryData<ThreadSummary[]>(["threads", { status: "all", q: "" }])?.map((thread) => thread.id)).toEqual(["thread-a"]);
    expect(qc.getQueryData<ThreadSummary[]>(["threads-extra"])?.map((thread) => thread.id)).toEqual(["thread-b"]);
    expect(qc.getQueryData(["thread", "thread-b"])).toBeUndefined();
    expect(qc.getQueryData(["thread", "thread-c"])).toBeDefined();
    expect(cleared).toEqual(["thread-b"]);
  });

  test("archive and restore optimistic cache changes can roll back without empty flashes", async () => {
    const app = await loadApp();
    const qc = new QueryClient();
    const threads: ThreadSummary[] = [
      { id: "thread-a", title: "A", status: "Recent", message_count: 1 },
      { id: "thread-b", title: "B", status: "Recent", message_count: 1 }
    ];
    const detail = {
      summary: threads[0],
      messages: [],
      blocks: [],
      raw_event_count: 0
    };
    const cleared: string[] = [];

    qc.setQueryData(["threads", "all", ""], threads);
    qc.setQueryData(["thread", "thread-a"], detail);

    const archiveSnapshot = app.applyOptimisticThreadArchive?.(qc, { clear: (threadId) => cleared.push(threadId) }, "thread-a");

    expect(qc.getQueryData<ThreadSummary[]>(["threads", "all", ""])?.map((thread) => thread.id)).toEqual(["thread-b"]);
    expect(qc.getQueryData(["thread", "thread-a"])).toBeUndefined();
    expect(cleared).toEqual(["thread-a"]);

    app.rollbackOptimisticThreadArchive?.(qc, archiveSnapshot);

    expect(qc.getQueryData<ThreadSummary[]>(["threads", "all", ""])?.map((thread) => thread.id)).toEqual(["thread-a", "thread-b"]);
    expect(qc.getQueryData<{ summary: ThreadSummary }>(["thread", "thread-a"])?.summary.status).toBe("Recent");

    const archivedThread = { ...threads[0], status: "Archived" as const, archived_at: "2026-06-17T00:00:00Z" };
    qc.setQueryData(["threads", "all", ""], [archivedThread, threads[1]]);
    qc.setQueryData(["thread", "thread-a"], { ...detail, summary: archivedThread });

    const restoreSnapshot = app.applyOptimisticThreadRestore?.(qc, "thread-a");

    expect(qc.getQueryData<ThreadSummary[]>(["threads", "all", ""])?.[0].status).toBe("Recent");
    expect(qc.getQueryData<{ summary: ThreadSummary }>(["thread", "thread-a"])?.summary.status).toBe("Recent");

    app.rollbackOptimisticThreadRestore?.(qc, restoreSnapshot);

    expect(qc.getQueryData<ThreadSummary[]>(["threads", "all", ""])?.[0].status).toBe("Archived");
    expect(qc.getQueryData<{ summary: ThreadSummary }>(["thread", "thread-a"])?.summary.status).toBe("Archived");
  });

  test("archived detail responses are not hydrated back into the active thread view", async () => {
    const app = await loadApp();
    const recent: ThreadSummary = { id: "thread-a", title: "A", status: "Recent", message_count: 1 };
    const archived: ThreadSummary = { id: "thread-a", title: "A", status: "Archived", message_count: 1 };

    expect(app.shouldHydrateThreadDetail?.("thread-a", { summary: recent })).toBe(true);
    expect(app.shouldHydrateThreadDetail?.("thread-a", { summary: archived })).toBe(false);
    expect(app.shouldHydrateThreadDetail?.("thread-a", { summary: { ...recent, id: "thread-b" } })).toBe(false);
  });

  test("visible Codex copy avoids legacy transport labels", async () => {
    const app = await loadApp();
    const visibleText = [
      app.failureCategoryLabel?.("codex_local_state_unavailable"),
      app.failureCategoryLabel?.("app_server_unavailable")
    ].join(" ");

    expect(app.failureCategoryLabel?.("codex_local_state_unavailable")).toBe("Codex 本地状态不可用");
    expect(app.failureCategoryLabel?.("app_server_unavailable")).toBe("Codex 本地状态不可用");
    expect(visibleText).toBe("Codex 本地状态不可用 Codex 本地状态不可用");
  });

  test("composer file picker has no accept whitelist", async () => {
    const app = await loadApp();

    expect(app.composerFileInputAcceptValue?.()).toBeUndefined();
  });

  test("thread settings hides internal metrics and copies only the thread id", async () => {
    const app = await loadApp();

    expect(app.threadInspectorPanelTitles?.()).toEqual(["名称与归档", "Goal", "状态摘要"]);
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
    expect(app.threadCopyId?.("019ec943-0b86-7e22-86e9-4dc0c919b09d")).toBe(
      "019ec943-0b86-7e22-86e9-4dc0c919b09d"
    );
    expect(app.threadCopyId?.("  ")).toBeNull();
    expect(app.threadCopyId?.(null)).toBeNull();
    expect(app.threadResumeCommand?.("  ")).toBeNull();
    expect(app.threadResumeCommand?.(null)).toBeNull();
  });

  test("goal panel helpers cover TUI states and button rules", async () => {
    const app = await loadApp();
    const active: CodexGoal = {
      available: true,
      enabled: true,
      objective: "补齐右栏",
      token_budget: 12000,
      status: "active"
    };
    const paused: CodexGoal = { ...active, status: "paused" };
    const cleared: CodexGoal = {
      available: true,
      enabled: false,
      objective: null,
      token_budget: null,
      status: "cleared"
    };
    const blocked: CodexGoal = { ...active, status: "blocked", blocked_reason: "等待确认" };

    expect(app.goalStatusLabel?.(undefined, true)).toBe("读取中");
    expect(app.goalStatusLabel?.({ ...active, status: "idle", enabled: false }, false)).toBe("未设置");
    expect(app.goalStatusLabel?.(active, false)).toBe("进行中");
    expect(app.goalStatusLabel?.(paused, false)).toBe("已暂停");
    expect(app.goalStatusLabel?.(cleared, false)).toBe("已清除");
    expect(app.goalStatusLabel?.(blocked, false)).toBe("阻塞");
    expect(app.goalStatusLabel?.({ ...active, status: "complete" }, false)).toBe("完成");
    expect(app.goalStatusTone?.(blocked)).toBe("danger");

    expect(app.goalControlState?.(undefined, { objective: "", tokenBudget: "" })).toEqual({
      saveDisabled: true,
      clearDisabled: true,
      pauseDisabled: true,
      resumeDisabled: true
    });
    expect(app.goalControlState?.(active, { objective: "补齐右栏", tokenBudget: "12000" })).toEqual({
      saveDisabled: false,
      clearDisabled: false,
      pauseDisabled: false,
      resumeDisabled: true
    });
    expect(app.goalControlState?.(paused, { objective: "补齐右栏", tokenBudget: "12000" })).toEqual({
      saveDisabled: false,
      clearDisabled: false,
      pauseDisabled: true,
      resumeDisabled: false
    });
    expect(app.goalControlState?.(cleared, { objective: "", tokenBudget: "" })?.pauseDisabled).toBe(true);
    expect(app.goalControlState?.(active, { objective: "补齐右栏", tokenBudget: "0" })?.saveDisabled).toBe(true);
    expect(app.goalControlState?.(active, { busy: true, objective: "补齐右栏", tokenBudget: "12000" })).toEqual({
      saveDisabled: true,
      clearDisabled: true,
      pauseDisabled: true,
      resumeDisabled: true
    });
    expect(app.formatGoalTimestamp?.(0)).toContain("1970");
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

  test("probe availability copy treats initial snapshot fetch as loading instead of unavailable", async () => {
    const app = await loadApp();

    expect(app.probeAvailabilityView?.({ loading: true, fetching: true, hasData: false })).toEqual({
      headline: "正在读取 Probe 快照",
      metric: "读取中",
      tone: "warning"
    });
    expect(app.probeAvailabilityView?.({ available: false, loading: false, fetching: false, hasData: false })).toEqual({
      headline: "Probe 端点不可用",
      metric: "不可用",
      tone: "danger"
    });
    expect(app.probeAvailabilityView?.({ available: true, probeEnabled: true, hasData: true })).toEqual({
      headline: "Probe 正在接管云机观测",
      metric: "运行中",
      tone: "success"
    });
  });

  test("probe event summary shows context without leaking payload secrets", async () => {
    const app = await loadApp();
    const summary = app.probeEventSummary?.({
      id: "event-1",
      kind: "hook-stop",
      thread_id: "thread-a",
      title: "Codex Stop Hook",
      message: "done",
      source: "nexushubd probe hook-stop",
      payload: {
        session_id: "session-a",
        transcript_path: "/tmp/transcript.jsonl",
        last_assistant_message: "assistant",
        device_key: "[redacted]"
      },
      created_at: "2026-06-15T00:00:00Z"
    });

    expect(summary).toContain("线程 thread-a");
    expect(summary).toContain("session");
    expect(summary).toContain("transcript");
    expect(summary).toContain("assistant");
    expect(summary).not.toContain("device");
  });

  test("probe event card renderer prefers structured payload fields and hides secrets", async () => {
    const app = await loadApp();
    const card = app.probeEventCard?.({
      id: "event-structured",
      kind: "legacy-kind",
      thread_id: "fallback-thread",
      title: "Raw event title",
      message: "Raw event message",
      dedupe_key: "reply-needed:thread-a:turn-a",
      source: "raw event source",
      payload: {
        event_type: "reply-needed",
        thread_title: "Plan Mode 修复",
        thread_id: "thread-a",
        turn_id: "turn-a",
        beijing_time: "2026-06-16 09:30:00 北京时间",
        reason_label: "等待用户确认",
        body_summary: "Plan Mode 等待用户确认",
        body_sha256: "abc123",
        body_length: 324,
        source: "nexushubd probe passive-scan",
        bark: { sent: true, skipped: false, http_status: 200, dedupe_hit: false },
        dedupe: { claimed: true, duplicate: false, status: "claimed" },
        device_key: "secret-device-key"
      },
      created_at: "2026-06-16T01:30:00Z"
    });

    expect(card?.headline).toBe("Plan Mode 修复");
    expect(card?.summary).toBe("Plan Mode 等待用户确认");
    expect(card?.details).toEqual(expect.arrayContaining([
      expect.objectContaining({ label: "线程", value: "thread-a" }),
      expect.objectContaining({ label: "Turn", value: "turn-a" })
    ]));
    expect(JSON.stringify(card)).not.toContain("secret-device-key");
  });

  test("probe event cards fall back to safe raw fields when structured payload is absent", async () => {
    const app = await loadApp();
    const card = app.probeEventCard?.({
      id: "event-fallback",
      kind: "hook-stop",
      source: "nexushubd probe hook-stop",
      payload: { session_id: "session-a" },
      created_at: "2026-06-16T00:00:00Z"
    });

    expect(card?.headline).toBe("hook-stop");
    expect(card?.summary).toContain("Probe 事件已记录");
    expect(card?.details).toEqual(expect.arrayContaining([
      expect.objectContaining({ label: "来源", value: "nexushubd probe hook-stop" })
    ]));
  });

  test("probe polling helper preserves the current scroll position unless the user is near the bottom", async () => {
    const app = await loadApp();

    expect(app.shouldAutoScrollProbeFeed?.({ scrollTop: 710, clientHeight: 300, scrollHeight: 1010 }, { scrollTop: 0, clientHeight: 300, scrollHeight: 1010 })).toBe(true);
    expect(app.shouldAutoScrollProbeFeed?.({ scrollTop: 620, clientHeight: 300, scrollHeight: 1010 }, { scrollTop: 0, clientHeight: 300, scrollHeight: 1010 })).toBe(false);
  });

  test("conversation header renders title without cwd or runtime paths", async () => {
    const app = await loadApp();
    const html = app.renderConversationHeaderHtml?.({
      id: "thread-a",
      title: "部署检查",
      status: "Recent",
      message_count: 1,
      cwd: "/root/.codex"
    });

    expect(html).toContain("部署检查");
    expect(html).not.toContain("/root/.codex");
    expect(html).not.toContain("cwd");
    expect(html).not.toContain("工作目录");
  });

  test("query placeholder helper keeps prior successful data during view switches", async () => {
    const app = await loadApp();
    const previousThreads: ThreadSummary[] = [
      { id: "thread-a", title: "A", status: "Recent", message_count: 1 }
    ];
    const previousProbe = { available: true, data: { label: "Probe" } };

    expect(app.preservePreviousQueryData?.(previousThreads)).toBe(previousThreads);
    expect(app.preservePreviousQueryData?.(previousProbe)).toBe(previousProbe);
    expect(app.preservePreviousQueryData?.(undefined)).toBeUndefined();
  });

});
