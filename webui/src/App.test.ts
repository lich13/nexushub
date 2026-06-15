import { describe, expect, test } from "vitest";
import type { GoalModeState, MessageBlock, PluginInfo, ProbeEvent, ThreadSummary } from "./types";

type AppExports = typeof import("./App") & {
  buildPayload?: (message: string, config: Record<string, unknown>, attachments?: Array<{ id: string }>) => Record<string, unknown>;
  composerFileInputAcceptValue?: () => string | undefined;
  composerActionMode?: (running: boolean, draft: string, canStop: boolean, attachmentCount?: number) => string;
  defaultRunConfig?: () => Record<string, unknown>;
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
  mergeSavedThreadTitle?: (threads: ThreadSummary[], threadId: string, title: string) => ThreadSummary[];
  threadSettingsMetricLabels?: () => string[];
  threadResumeCommand?: (threadId?: string | null) => string | null;
  probeStatusThreads?: (status?: { running_threads?: ThreadSummary[]; reply_needed_threads?: ThreadSummary[]; recoverable_threads?: ThreadSummary[] } | null) => ThreadSummary[];
  probeEventSummary?: (event: ProbeEvent) => string;
  probeEventCard?: (event: ProbeEvent) => { headline: string; summary: string; details: Array<{ label: string; value: string }> };
  shouldAutoScrollProbeFeed?: (
    current: { scrollTop: number; clientHeight: number; scrollHeight: number },
    previous?: { scrollTop: number; clientHeight: number; scrollHeight: number } | null
  ) => boolean;
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
    expect(app.exactSlashCommandFromDraft?.(" /goal   resume ")).toBe("/goal resume");
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
    expect(app.slashCommandAction?.("/goal resume")).toEqual({ kind: "resume_goal", command: "/goal resume" });
    expect(app.slashCommandAction?.("/goal resume", false)).toEqual({
      kind: "requires_thread",
      command: "/goal resume",
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

  test("plan mode button is a next-send state and successful sends reset it", async () => {
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
      collaborationMode: "",
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
    expect(app.runConfigAfterSuccessfulSend?.(app.mergeRunConfigFromDefaults?.(current, defaults) ?? current).collaborationMode).toBe("");
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
        beijing_time: "2026-06-16 09:30:00 CST",
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

  test("goal status labels normalize known app-server states in Chinese", async () => {
    const app = await loadApp();

    expect(app.formatGoalStatus?.({ enabled: true, status: "running" })).toBe("运行中");
    expect(app.formatGoalStatus?.({ enabled: true, status: "completed" })).toBe("已完成");
    expect(app.formatGoalStatus?.({ enabled: true, status: "blocked" })).toBe("已阻塞");
    expect(app.formatGoalStatus?.({ enabled: false, status: "missing_thread" })).toBe("未选择线程");
    expect(app.formatGoalStatus?.({ enabled: false, status: "cleared" })).toBe("已清除");
  });
});
