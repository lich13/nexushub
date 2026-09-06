import { QueryClient } from "@tanstack/react-query";
import { describe, expect, test, vi } from "vitest";
import appSource from "./App.tsx?raw";
import authGateSource from "./components/auth/WebAuthGate.tsx?raw";
import chatWorkspaceSource from "./components/chat/ChatWorkspace.tsx?raw";
import conversationSource from "./components/chat/Conversation.tsx?raw";
import jobListSource from "./components/jobs/JobList.tsx?raw";
import messageStreamSource from "./components/chat/MessageStream.tsx?raw";
import opsWorkspaceSource from "./components/ops/OpsWorkspace.tsx?raw";
import probeWorkspaceSource from "./components/probe/ProbeWorkspace.tsx?raw";
import securityWorkspaceSource from "./components/security/SecurityWorkspace.tsx?raw";
import conversationControllerSource from "./hooks/useConversationController.ts?raw";
import codexViewModelSource from "./lib/domain/codexViewModel.ts?raw";
import runtimeViewModelSource from "./lib/domain/runtimeViewModel.ts?raw";
import threadQuerySource from "./lib/query/threads.ts?raw";
import type { RuntimeCapabilityMatrix } from "./lib/api";
import type { MessageBlock, PluginInfo, ProbeEvent, ThreadSummary, UpdateStatus } from "./types";

type AppExports = typeof import("./App") & typeof import("./lib/domain/conversationViewModel") & typeof import("./lib/domain/runtimeViewModel") & typeof import("./lib/domain/codexViewModel");

type ThreadQueryExports = typeof import("./lib/query/threads") & {
  clearArchivedThreadClientState?: (qc: QueryClient, messageStore: { clear: (threadId: string) => void }, threadId: string) => void;
  applyOptimisticThreadTitle?: (qc: QueryClient, threadId: string, title: string) => unknown;
  rollbackOptimisticThreadTitle?: (qc: QueryClient, snapshot: unknown) => void;
  applyOptimisticThreadArchive?: (qc: QueryClient, messageStore: { clear: (threadId: string) => void }, threadId: string) => unknown;
  rollbackOptimisticThreadArchive?: (qc: QueryClient, snapshot: unknown) => void;
  applyOptimisticThreadRestore?: (qc: QueryClient, threadId: string) => unknown;
  rollbackOptimisticThreadRestore?: (qc: QueryClient, snapshot: unknown) => void;
  connectThreadRealtimeSubscription?: (input: {
    threadId: string;
    messageStore: {
      isActive: (threadId: string) => boolean;
      applyRealtimeBlocks: (threadId: string, blocks: MessageBlock[]) => void;
      applySummary: (threadId: string, summary: ThreadSummary) => void;
      setFeedback: (threadId: string, message: string | null) => void;
    };
    threadCache: {
      updateThreadListCaches: (summary: ThreadSummary) => void;
      invalidateThreads: (refetchType?: "active" | "all" | "inactive" | "none") => void;
      invalidateThread: (threadId: string, refetchType?: "active" | "all" | "inactive" | "none") => void;
    };
    applyThreadTitleOverride?: (summary: ThreadSummary) => ThreadSummary;
    onBeforeActiveBlocks?: () => void;
    subscribe?: (threadId: string, handlers: {
      onBlocks?: (blocks: MessageBlock[], threadId: string) => void;
      onSummary?: (summary: ThreadSummary, threadId: string) => void;
      onError?: (message: string, threadId: string) => void;
    }) => () => void;
  }) => () => void;
  useThreadRealtimeSubscription?: unknown;
};

async function loadApp(): Promise<AppExports> {
  return { ...await import("./lib/domain/conversationViewModel"), ...await import("./lib/domain/runtimeViewModel"), ...await import("./lib/domain/codexViewModel"), ...await import("./App") } as AppExports;
}

async function loadThreadQuery(): Promise<ThreadQueryExports> {
  return import("./lib/query/threads") as Promise<ThreadQueryExports>;
}

function extractThreadListSource(): string {
  const source = chatWorkspaceSource;
  const start = source.indexOf("function ThreadList(");

  expect(start).toBeGreaterThanOrEqual(0);
  return source.slice(start);
}


function extractProbeWorkspaceSource(): string {
  const source = probeWorkspaceSource;
  const start = source.indexOf("function ProbeWorkspace(");

  expect(start).toBeGreaterThanOrEqual(0);
  return source.slice(start);
}

function extractFunctionSource(name: string): string {
  const source = name === "OpsWorkspace"
      ? opsWorkspaceSource
      : name === "JobList"
        ? jobListSource
        : name === "ProbeWorkspace"
          ? probeWorkspaceSource
          : name === "ChatWorkspace" || name === "ThreadList"
            ? chatWorkspaceSource
            : name === "MessageBlockView"
                    ? messageStreamSource
                    : [
                        "Conversation",
                        "EmptyConversation",
                        "StatusChip"
                      ].includes(name)
                        ? conversationSource
                        : name === "SecurityWorkspace"
                          ? securityWorkspaceSource
                          : name === "WebAuthGate" || name === "LoginScreen"
                            ? authGateSource
                            : appSource;
  const start = source.indexOf(`function ${name}`);

  expect(start).toBeGreaterThanOrEqual(0);

  const next = source.indexOf("\nfunction ", start + 1);
  return source.slice(start, next === -1 ? source.length : next);
}

const forbiddenComponentTokens = [
  '"/api/',
  "'/api/",
  "`/api/",
  "@tauri-apps/api",
  "runtimeRpc(",
  "fetch(",
  "EventSource",
  "useQueryClient",
  "setQueryData",
  "invalidateQueries",
  "isDesktopRuntime(",
  "desktop_api_command",
  "desktopApiRoute",
  "invokeDesktopApi",
  "runtimeCapabilities(",
  "runtimeCapabilitiesForRuntime("
];

function expectSourceToAvoidTokens(source: string, label: string, tokens: string[]) {
  for (const token of tokens) {
    expect(source, `${label} must not contain ${token}`).not.toContain(token);
  }
  expect(source, `${label} must not call invoke directly`).not.toMatch(/\binvoke\s*\(/);
}

const linuxWebCapabilities: RuntimeCapabilityMatrix = {
  runtimeKind: "web",
  hostSurface: "linux_server_webui",
  webAuth: true,
  logout: true,
  securitySettings: true,
  publicEndpointStatus: true,
  codexStatePaths: true,
  updatePrune: true,
  threadCleanup: true,
  probeLogMaintenance: true,
  threadArchiveActions: true,
  updateServiceLabels: true,
  desktopWebuiControl: false,
  forkAction: true,
  approvalActions: true
};

const macosDesktopCapabilities: RuntimeCapabilityMatrix = {
  runtimeKind: "desktop",
  hostSurface: "desktop_embedded_tauri",
  webAuth: false,
  logout: false,
  securitySettings: false,
  publicEndpointStatus: false,
  codexStatePaths: false,
  updatePrune: false,
  threadCleanup: true,
  probeLogMaintenance: true,
  threadArchiveActions: true,
  updateServiceLabels: false,
  desktopWebuiControl: true,
  forkAction: false,
  approvalActions: false
};

describe("conversation helpers", () => {

  test("desktop runtime maps Linux-only job failure categories to generic copy", async () => {
    const app = await loadApp();

    const webCapabilities = linuxWebCapabilities;
    const desktopCapabilities = macosDesktopCapabilities;

    expect(app.failureCategoryLabel?.("systemd_failure", webCapabilities)).toBe("systemd 失败");
    expect(app.failureCategoryLabel?.("nginx_failure", webCapabilities)).toBe("Nginx 失败");
    expect(app.failureCategoryLabel?.("systemd_failure", desktopCapabilities)).toBe("服务失败");
    expect(app.failureCategoryLabel?.("nginx_failure", desktopCapabilities)).toBe("更新失败");
    expect(app.failureCategoryLabel?.("permission_denied_sudo", desktopCapabilities)).toBe("权限失败");

    const view = app.jobFailureAnalysisView?.({
      category: "nginx_failure",
      explanation: "Nginx reload failed after systemd restart; 输入管理员密码后执行 Linux prune，Turnstile 公网入口 https://661313.xyz/nexushub/ 43.155.235.227",
      suggestions: ["检查 Nginx", "systemd restart", "输入管理员密码", "Linux prune", "Turnstile 661313.xyz"]
    }, desktopCapabilities);
    const rendered = [view?.label, view?.explanation, ...(view?.suggestions ?? [])].join("\n");

    expect(rendered).not.toMatch(/systemd|Nginx|管理员密码|Linux prune|Turnstile|公网入口|661313\.xyz|43\.155\.235\.227/i);
    expect(rendered).toContain("更新失败");
    expect(app.jobOutputView?.(
      "nginx reload failed after systemd restart; 输入管理员密码后执行 Linux prune with sudo; Turnstile 公网入口 https://661313.xyz/nexushub/ 43.155.235.227",
      desktopCapabilities
    )).not.toMatch(/systemd|nginx|管理员密码|Linux prune|sudo|Turnstile|公网入口|661313\.xyz|43\.155\.235\.227/i);
    expect(app.jobOutputView?.(
      "nginx reload failed after systemd restart; 输入管理员密码后执行 Linux prune with sudo",
      webCapabilities
    )).toMatch(/systemd|nginx|管理员密码|Linux prune|sudo/i);
  });

  test("App delegates realtime lifecycle, query placeholders, failure copy, and action gating to lib helpers", async () => {
    const threadQuery = await loadThreadQuery();

    expect(typeof threadQuery.connectThreadRealtimeSubscription).toBe("function");
    expect(typeof threadQuery.useThreadRealtimeSubscription).toBe("function");
    expect(appSource).not.toContain("useConversationController");
    expect(chatWorkspaceSource).toContain("useConversationController");
    expect(conversationControllerSource).toContain("useThreadRealtimeSubscription");
    expect(appSource).not.toContain("subscribeThreadEvents");
    expect(appSource).not.toMatch(/export function (opsWorkspacePanelTitles|opsWorkspaceVisibleCopy|desktopRuntimeVisibleCopy|canShowForkAction|approvalActionMode|preservePreviousQueryData|slashCommandsForRuntime|failureCategoryLabel|jobFailureAnalysisView|jobOutputView)\b/);
    expect(appSource).not.toMatch(/const (linuxFailureLabels|genericFailureLabels|desktopUnsupportedSlashCommands|controlledSlashActions|unavailableSlashCommands)\b/);
  });

  test("App delegates Codex view-model helpers to domain modules", () => {
    expect(appSource).not.toMatch(/function (makeRunConfig|permissionPresetFromConfig|threadStatusLabel|sourceCountsText|normalizeServiceTier)\b/);
    expect(appSource).not.toMatch(/export function (defaultRunConfig|applyPermissionPreset|threadListItemText|filterVisibleThreadSummaries|buildPayload|runConfigWithSupportedServiceTier|conversationTitleText|lastEventKindText)\b/);
    expect(appSource).not.toMatch(/const (codexLocalCopy|defaultCwd)\b/);
    expect(appSource).toContain('from "./lib/domain/codexViewModel"');
    expect(codexViewModelSource).not.toContain("../query/");
    expect(codexViewModelSource).not.toContain("../session");
    expect(codexViewModelSource).not.toContain("../runtime");
  });

  test("App delegates Probe and Ops display derivations to domain modules", () => {
    expect(appSource).not.toMatch(/export function (probeStatusThreads|probeThreadsByStatus|probeRunningCountValue|probeSettingsAfterBarkSave|probeEventSummary|shouldAutoScrollProbeFeed|codexHomeStatusValue|logsDbPathStatusValue|probeDiscoveryWarningsText|probeSnapshotStatusText|probeAvailabilityView|hiddenThreadDeleteStats|archivePlanAfterExecute)\b/);
    expect(appSource).not.toMatch(/function (isProbeJob|probeJobActionLabel|probeStateLabel|probeLogsDbTone|isProbeSettings|logBytesDraftToMb|mbDraftToLogBytes|probeLogDbNumber|probeLogDbString|probeLogDbSize|probeLogDbValue|cleanHostValue|hostnameFromPublicEndpoint|secondsToDays|normalizeTurnstileAction|cleanupStageLabel|hiddenRolloutDeleteResultText)\b/);

    const probeSource = extractProbeWorkspaceSource();
    const opsSource = extractFunctionSource("OpsWorkspace");
    expect(probeSource).toContain("const probeView = probeWorkspaceView(");
    expect(probeSource).not.toContain("probeAvailabilityView({");
    expect(probeSource).not.toContain("const probeThreads =");
    expect(probeSource).not.toContain("const serviceText =");
    expect(opsSource).toContain("const opsView = opsWorkspaceView(");
    expect(opsSource).not.toContain("const publicEndpoint =");
    expect(opsSource).not.toContain("const hiddenStats = hiddenThreadDeleteStats(");
    expect(opsSource).not.toContain("cleanupStageLabel({");
  });

  test("OpsWorkspace surfaces cleanup mutation errors instead of leaving confirm buttons silent", () => {
    const opsSource = extractFunctionSource("OpsWorkspace");

    expect(opsSource).toContain("archiveCleanupError");
    expect(opsSource).toContain("hiddenCleanupError");
    expect(opsSource).toContain("executeDelete.error");
    expect(opsSource).toContain("executeHiddenDelete.error");
    expect(opsSource).toContain("cleanup-error");
  });

  test("thread query realtime helper owns subscription side effects", async () => {
    const threadQuery = await loadThreadQuery();
    expect(typeof threadQuery.connectThreadRealtimeSubscription).toBe("function");
    if (!threadQuery.connectThreadRealtimeSubscription) return;

    let handlers: {
      onBlocks?: (blocks: MessageBlock[], threadId: string) => void;
      onSummary?: (summary: ThreadSummary, threadId: string) => void;
      onError?: (message: string, threadId: string) => void;
    } = {};
    const unsubscribe = vi.fn();
    const messageStore = {
      isActive: vi.fn((threadId: string) => threadId === "thread-a"),
      applyRealtimeBlocks: vi.fn(),
      applySummary: vi.fn(),
      setFeedback: vi.fn()
    };
    const threadCache = {
      updateThreadListCaches: vi.fn(),
      invalidateThreads: vi.fn(),
      invalidateThread: vi.fn()
    };

    const cleanup = threadQuery.connectThreadRealtimeSubscription({
      threadId: "thread-a",
      messageStore,
      threadCache,
      applyThreadTitleOverride: (summary) => ({ ...summary, title: `${summary.title} local` }),
      onBeforeActiveBlocks: vi.fn(),
      subscribe: (_threadId, nextHandlers) => {
        handlers = nextHandlers as typeof handlers;
        return unsubscribe;
      }
    });

    const block: MessageBlock = { id: "block-a", role: "assistant", kind: "message", text: "hello", questions: [] };
    const summary: ThreadSummary = { id: "thread-a", title: "Remote", status: "Recent", message_count: 2 };

    handlers.onBlocks?.([block], "thread-a");
    handlers.onSummary?.(summary, "thread-a");
    handlers.onError?.("stream disconnected", "thread-a");
    cleanup();

    expect(messageStore.applyRealtimeBlocks).toHaveBeenCalledWith("thread-a", [block]);
    expect(messageStore.applySummary).toHaveBeenCalledWith("thread-a", expect.objectContaining({ title: "Remote local" }));
    expect(threadCache.updateThreadListCaches).toHaveBeenCalledWith(expect.objectContaining({ title: "Remote local" }));
    expect(threadCache.invalidateThreads).toHaveBeenCalledWith();
    expect(messageStore.setFeedback).toHaveBeenCalledWith("thread-a", "stream disconnected");
    expect(threadCache.invalidateThread).toHaveBeenCalledWith("thread-a", "all");
    expect(threadCache.invalidateThreads).toHaveBeenCalledWith("all");
    expect(unsubscribe).toHaveBeenCalledTimes(1);
  });

  test("App.tsx keeps domain-only pure helpers out of the component file", () => {
    expect(appSource).not.toContain("export function canStartHiddenThreadDelete(");
    expect(appSource).not.toContain("export function canStartUpdateInstall(");
    expect(appSource).not.toContain("export function goalStatusLabel(");
    expect(appSource).not.toContain("export function goalStatusTone(");
    expect(appSource).not.toContain("export function goalControlState(");
    expect(appSource).not.toContain("export function formatGoalTimestamp(");
    expect(appSource).not.toContain("export function resolvedSelectedThreadId(");
  });

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
    const threadQuery = await loadThreadQuery();
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

    const snapshot = threadQuery.applyOptimisticThreadTitle?.(qc, "thread-a", "即时标题");

    expect(qc.getQueryData<ThreadSummary[]>(["threads", "all", ""])?.[0].title).toBe("即时标题");
    expect(qc.getQueryData<ThreadSummary[]>(["threads", "running", ""])?.[0].title).toBe("其他");
    expect(qc.getQueryData<{ summary: ThreadSummary }>(["thread", "thread-a"])?.summary.title).toBe("即时标题");

    threadQuery.rollbackOptimisticThreadTitle?.(qc, snapshot);

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
    const threadQuery = await loadThreadQuery();
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

    threadQuery.clearArchivedThreadClientState?.(qc, { clear: (threadId) => cleared.push(threadId) }, "thread-b");

    expect(qc.getQueryData<ThreadSummary[]>(["threads", "all", ""])?.map((thread) => thread.id)).toEqual(["thread-a", "thread-c", "thread-archived"]);
    expect(qc.getQueryData<ThreadSummary[]>(["threads", "running", ""])?.map((thread) => thread.id)).toEqual(["thread-c"]);
    expect(qc.getQueryData<ThreadSummary[]>(["threads", { status: "all", q: "" }])?.map((thread) => thread.id)).toEqual(["thread-a"]);
    expect(qc.getQueryData<ThreadSummary[]>(["threads-extra"])?.map((thread) => thread.id)).toEqual(["thread-b"]);
    expect(qc.getQueryData(["thread", "thread-b"])).toBeUndefined();
    expect(qc.getQueryData(["thread", "thread-c"])).toBeDefined();
    expect(cleared).toEqual(["thread-b"]);
  });

  test("archive and restore optimistic cache changes can roll back without empty flashes", async () => {
    const threadQuery = await loadThreadQuery();
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

    const archiveSnapshot = threadQuery.applyOptimisticThreadArchive?.(qc, { clear: (threadId) => cleared.push(threadId) }, "thread-a");

    expect(qc.getQueryData<ThreadSummary[]>(["threads", "all", ""])?.map((thread) => thread.id)).toEqual(["thread-b"]);
    expect(qc.getQueryData(["thread", "thread-a"])).toBeUndefined();
    expect(cleared).toEqual(["thread-a"]);

    threadQuery.rollbackOptimisticThreadArchive?.(qc, archiveSnapshot);

    expect(qc.getQueryData<ThreadSummary[]>(["threads", "all", ""])?.map((thread) => thread.id)).toEqual(["thread-a", "thread-b"]);
    expect(qc.getQueryData<{ summary: ThreadSummary }>(["thread", "thread-a"])?.summary.status).toBe("Recent");

    const archivedThread = { ...threads[0], status: "Archived" as const, archived_at: "2026-06-17T00:00:00Z" };
    qc.setQueryData(["threads", "all", ""], [archivedThread, threads[1]]);
    qc.setQueryData(["thread", "thread-a"], { ...detail, summary: archivedThread });

    const restoreSnapshot = threadQuery.applyOptimisticThreadRestore?.(qc, "thread-a");

    expect(qc.getQueryData<ThreadSummary[]>(["threads", "all", ""])?.[0].status).toBe("Recent");
    expect(qc.getQueryData<{ summary: ThreadSummary }>(["thread", "thread-a"])?.summary.status).toBe("Recent");

    threadQuery.rollbackOptimisticThreadRestore?.(qc, restoreSnapshot);

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

  test("first paint does not auto-select the first thread and trigger detail loading", async () => {
    const app = await loadApp();

    expect(app.resolvedSelectedThreadId?.(null)).toBeNull();
    expect(chatWorkspaceSource).not.toContain('"__new"');
    expect(app.resolvedSelectedThreadId?.("thread-a")).toBe("thread-a");
    expect(appSource).not.toContain("visibleThreads[0]?.id");
  });

  test("thread list keeps title, search, and filters outside the independent scroll container", () => {
    const source = extractThreadListSource();
    const titleIndex = source.indexOf('className="section-title thread-title-row"');
    const searchIndex = source.indexOf('className="search-box"');
    const filtersIndex = source.indexOf('className="segmented"');
    const scrollIndex = source.indexOf('className="thread-scroll"');
    const scrollEndIndex = source.indexOf("\n      </div>\n    </div>\n  );", scrollIndex);
    const scrollBody = source.slice(scrollIndex, scrollEndIndex);

    expect(source.match(/className="thread-scroll"/g)).toHaveLength(1);
    expect(titleIndex).toBeGreaterThanOrEqual(0);
    expect(searchIndex).toBeGreaterThan(titleIndex);
    expect(filtersIndex).toBeGreaterThan(searchIndex);
    expect(scrollIndex).toBeGreaterThan(filtersIndex);
    expect(scrollEndIndex).toBeGreaterThan(scrollIndex);

    expect(scrollBody).toContain("{threads.map((thread) => {");
    expect(scrollBody).toContain("className={`thread-item ");
    expect(scrollBody).toContain("没有匹配线程");
    expect(scrollBody).not.toContain('className="section-title thread-title-row"');
    expect(scrollBody).not.toContain('className="search-box"');
    expect(scrollBody).not.toContain('className="segmented"');
    expect(scrollBody).not.toContain("statusTabs.map");
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
    expect(app.probeAvailabilityView?.({ loading: false, fetching: true, hasData: false, error: true })).toEqual({
      headline: "Probe 快照读取失败",
      metric: "读取失败",
      tone: "danger"
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

  test("bark settings save marks a submitted key configured so test buttons unlock", async () => {
    const app = await loadApp();
    const saved = {
      codex: { host_label: "mac" },
      probe: { enabled: true },
      notifications: { enabled: true, device_key_configured: false },
      logs_db: { enabled: true }
    };

    expect(app.probeSettingsAfterBarkSave?.(saved, " bark-device-key ")).toMatchObject({
      notifications: { device_key_configured: true }
    });
    expect(app.probeSettingsAfterBarkSave?.(saved, "   ")).toMatchObject({
      notifications: { device_key_configured: false }
    });
    expect(extractProbeWorkspaceSource()).toContain("probeSettingsAfterBarkSave(");
  });

  test("probe event summary shows context without leaking payload secrets", async () => {
    const app = await loadApp();
    const summary = app.probeEventSummary?.({
      id: "event-1",
      kind: "hook-stop",
      thread_id: "thread-a",
      title: "Codex Stop Hook",
      message: "done",
      source: "nexushub-webd probe hook-stop",
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
        source: "nexushub-webd probe passive-scan",
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
      source: "nexushub-webd probe hook-stop",
      payload: { session_id: "session-a" },
      created_at: "2026-06-16T00:00:00Z"
    });

    expect(card?.headline).toBe("hook-stop");
    expect(card?.summary).toContain("Probe 事件已记录");
    expect(card?.details).toEqual(expect.arrayContaining([
      expect.objectContaining({ label: "来源", value: "nexushub-webd probe hook-stop" })
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

  test("ops workspace exposes only the current operations panels", async () => {
    const app = await loadApp();
    const retiredCodexPanel = ["Codex", "更新"].join(" ");
    const retiredClaudePanel = ["Claude Code", "维护"].join(" ");
    const retiredReadyCopy = ["CSRF", "已就绪"].join(" ");
    const retiredMissingCopy = ["CSRF", "未恢复"].join(" ");

    expect(app.opsWorkspacePanelTitles?.()).toEqual([
      "系统状态",
      "NexusHub 更新",
      "Job History"
    ]);
    expect(app.opsWorkspacePanelTitles?.(linuxWebCapabilities)).toEqual([
      "系统状态",
      "NexusHub 更新",
      "归档线程清理",
      "隐藏线程清理",
      "Job History"
    ]);
    expect(app.opsWorkspacePanelTitles?.(linuxWebCapabilities)).toEqual(expect.arrayContaining([
      "归档线程清理",
      "隐藏线程清理"
    ]));
    expect(app.opsWorkspacePanelTitles?.()).not.toEqual(expect.arrayContaining([
      retiredCodexPanel,
      retiredClaudePanel,
      "归档清理"
    ]));
    const visibleCopy = app.opsWorkspaceVisibleCopy?.(linuxWebCapabilities).join("\n") ?? "";
    expect(visibleCopy).toContain("归档线程清理");
    expect(visibleCopy).toContain("隐藏线程清理");
    expect(visibleCopy).not.toContain(retiredCodexPanel);
    expect(visibleCopy).not.toContain(retiredClaudePanel);
    expect(visibleCopy).not.toContain(retiredReadyCopy);
    expect(visibleCopy).not.toContain(retiredMissingCopy);
  });

  test("desktop runtime copy hides login and security setup while preserving core Codex controls", async () => {
    const app = await loadApp();
    const copy = app.desktopRuntimeVisibleCopy?.().join("\n") ?? "";

    expect(copy).toContain("Codex 本地线程");
    expect(copy).toContain("Goal");
    expect(copy).toContain("Plan Mode");
    expect(copy).toContain("名称与归档");
    expect(copy).toContain("线程标题");
    expect(copy).toContain("重命名");
    expect(copy).toContain("归档");
    expect(copy).toContain("复制与路径");
    expect(copy).toContain("复制 ID");
    expect(copy).toContain("复制文件路径");
    expect(copy).toContain("复制 codex resume+ID");
    expect(copy).not.toContain("管理员");
    expect(copy).not.toContain("登录");
    expect(copy).not.toContain("Turnstile");
    expect(copy).not.toContain("CSRF");
    expect(copy).not.toContain("Codex Home");
    expect(copy).not.toContain("State DB");
  });

  test("probe workspace labels the paired lifecycle hooks as Codex Hook", () => {
    const probeSource = extractProbeWorkspaceSource();

    expect(probeSource).toContain('<Metric label="Codex Hook"');
    expect(probeSource).toContain("<span>管理 Codex Hook</span>");
    expect(probeSource).not.toContain('<Metric label="Stop Hook"');
  });

  test("probe workspace exposes terminal error monitor controls and runtime status", () => {
    const probeSource = extractProbeWorkspaceSource();

    expect(probeSource).toContain('<Metric label="错误监控"');
    expect(probeSource).toContain('<Metric label="错误事件"');
    expect(probeSource).toContain("<span>终止错误监控</span>");
    expect(probeSource).toContain("<span>受限 Goal 自动恢复</span>");
    expect(probeSource).toContain("status?.error_monitor_status");
    expect(probeSource).toContain("status?.error_monitor_incident_count");
  });

  test("probe workspace tolerates partial desktop settings DTOs without white-screen assumptions", () => {
    const probeSource = extractProbeWorkspaceSource();

    expect(probeSource).toContain("probeWorkspaceView(");
    expect(runtimeViewModelSource).toContain("input.currentSettings?.notifications?.device_key_configured");
    expect(runtimeViewModelSource).toContain("input.currentSettings?.probe?.enabled");
    expect(probeSource).toContain("currentSettings?.codex?.host_label");
    expect(probeSource).toContain("settings?.codex?.discovery_warnings");
  });

  test("archive cleanup execute clears stale dry-run counts without touching hidden cleanup state", async () => {
    const app = await loadApp();
    const current = {
      total_threads: 12,
      active_threads: 8,
      archived_threads: 4,
      session_index_lines: 15,
      rollout_files: 6,
      archived_ids: ["archived-a", "archived-b"],
      integrity: "ok"
    };

    expect(app.archivePlanAfterExecute?.(current, {
      after_total_threads: 8,
      after_active_threads: 8,
      after_archived_threads: 0,
      after_integrity: "ok"
    })).toEqual({
      ...current,
      total_threads: 8,
      active_threads: 8,
      archived_threads: 0,
      archived_ids: [],
      integrity: "ok"
    });
    expect(app.canStartHiddenThreadDelete?.({
      total_threads: 10,
      visible_threads: 7,
      hidden_threads: 3,
      archived_threads: 0,
      session_index_lines: 10,
      rollout_files: 2,
      hidden_ids: ["hidden-a"],
      hidden_source_counts: { subagent: 3 },
      integrity: "ok"
    })).toBe(true);
  });

  test("update install action requires an explicit available update", async () => {
    const app = await loadApp();
    const baseStatus: UpdateStatus = {
      current_version: "0.1.116",
      latest_version: "0.1.116",
      update_available: false,
      channel: "stable",
      method: "macos_tauri_updater",
      state: "idle",
      failure_category: null,
      recommended_action: "No update available.",
      capabilities: ["check", "confirm_install", "job_history", "signature_verification"]
    };

    expect(app.canStartUpdateInstall?.(undefined)).toBe(false);
    expect(app.canStartUpdateInstall?.(null)).toBe(false);
    expect(app.canStartUpdateInstall?.({ ...baseStatus, update_available: null })).toBe(false);
    expect(app.canStartUpdateInstall?.(baseStatus)).toBe(false);
    expect(app.canStartUpdateInstall?.({ ...baseStatus, latest_version: "0.1.118", update_available: true })).toBe(true);
  });

});

test("retired task controls are absent from the user entrypoints", () => {
  for (const source of [appSource, chatWorkspaceSource, conversationSource, messageStreamSource]) {
    expect(source).not.toMatch(/SlashCommandTextarea|useComposerAttachments|ThreadGoalPanel|ThreadInspectorPanels|onDecision|onSubmitQuestion|sendMessage|createThread|enqueueFollowUp|steerThread/);
  }
  expect(conversationSource).toContain("useReadOnlyThreadActions");
  expect(conversationSource).toContain("复制恢复命令");
  expect(conversationSource).not.toContain("textarea");
});
