import { QueryClient } from "@tanstack/react-query";
import { describe, expect, test, vi } from "vitest";
import appSource from "./App.tsx?raw";
import authGateSource from "./components/auth/WebAuthGate.tsx?raw";
import chatWorkspaceSource from "./components/chat/ChatWorkspace.tsx?raw";
import composerControlsSource from "./components/composer/ComposerControls.tsx?raw";
import conversationSource from "./components/chat/Conversation.tsx?raw";
import currentActionCardSource from "./components/chat/CurrentActionCard.tsx?raw";
import jobListSource from "./components/jobs/JobList.tsx?raw";
import messageStreamSource from "./components/chat/MessageStream.tsx?raw";
import opsWorkspaceSource from "./components/ops/OpsWorkspace.tsx?raw";
import probeWorkspaceSource from "./components/probe/ProbeWorkspace.tsx?raw";
import runConfigControlsSource from "./components/chat/RunConfigControls.tsx?raw";
import securityWorkspaceSource from "./components/security/SecurityWorkspace.tsx?raw";
import threadGoalPanelSource from "./components/chat/ThreadGoalPanel.tsx?raw";
import threadInspectorPanelsSource from "./components/chat/ThreadInspectorPanels.tsx?raw";
import conversationControllerSource from "./hooks/useConversationController.ts?raw";
import codexViewModelSource from "./lib/domain/codexViewModel.ts?raw";
import runtimeViewModelSource from "./lib/domain/runtimeViewModel.ts?raw";
import threadQuerySource from "./lib/query/threads.ts?raw";
import type { RuntimeCapabilityMatrix } from "./lib/api";
import type { CodexGoal, MessageBlock, PluginInfo, ProbeEvent, ThreadSummary, UpdateStatus } from "./types";

type AppExports = typeof import("./App") & {
  buildPayload?: (message: string, config: Record<string, unknown>, attachments?: Array<{ id: string }>) => Record<string, unknown>;
  composerFileInputAcceptValue?: () => string | undefined;
  composerActionMode?: (running: boolean, draft: string, canStop: boolean, attachmentCount?: number) => string;
  defaultRunConfig?: () => Record<string, unknown>;
  segmentInternalReferences?: (text: string) => Array<{ type: "text" | "internal_reference"; text: string; copyText?: string; kind?: string }>;
  slashCommands?: Array<{ command: string; description: string; usageHint: string; requiresThread?: boolean }>;
  slashCommandSuggestions?: (draft: string, cursor: number, hasThread?: boolean, capabilities?: RuntimeCapabilityMatrix) => Array<{ command: string; description: string; usageHint: string; requiresThread?: boolean }>;
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
  exactSlashCommandFromDraft?: (draft: string, capabilities?: RuntimeCapabilityMatrix) => string | null;
  slashCommandForComposerSubmit?: (draft: string, capabilities?: RuntimeCapabilityMatrix) => string | null;
  composerSubmitDraftValue?: (stateValue: string, domValue?: string | null) => string;
  composerMenuKeyAction?: (input: {
    key: string;
    shiftKey?: boolean;
    composing?: boolean;
    menuSelectionArmed?: boolean;
    selected: number;
    suggestions: Array<{ command?: string; id?: string }>;
  }) => { action: "move"; selected: number } | { action: "insert"; index: number } | { action: "dismiss" } | { action: "none" };
  slashCommandAction?: (command: string, hasThread?: boolean, capabilities?: RuntimeCapabilityMatrix) => { kind: string; message?: string; command?: string };
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
  threadRolloutPath?: (rolloutPath?: string | null) => string | null;
  probeRunningCountValue?: (status?: { running_count?: number; running_threads?: ThreadSummary[] } | null) => string;
  probeSettingsAfterBarkSave?: <T extends { notifications: { device_key_configured?: boolean } }>(saved: T, submittedDeviceKey?: string | null) => T;
  probeStatusThreads?: (status?: { running_threads?: ThreadSummary[]; reply_needed_threads?: ThreadSummary[]; recoverable_threads?: ThreadSummary[] } | null) => ThreadSummary[];
  probeAvailabilityView?: (input: {
    available?: boolean;
    probeEnabled?: boolean;
    loading?: boolean;
    fetching?: boolean;
    hasData?: boolean;
    error?: boolean;
  }) => { headline: string; metric: string; tone: "success" | "warning" | "danger" };
  probeEventSummary?: (event: ProbeEvent) => string;
  probeEventCard?: (event: ProbeEvent) => { headline: string; summary: string; details: Array<{ label: string; value: string }> };
  shouldAutoScrollProbeFeed?: (
    current: { scrollTop: number; clientHeight: number; scrollHeight: number },
    previous?: { scrollTop: number; clientHeight: number; scrollHeight: number } | null
  ) => boolean;
  nextVisibleThreadIdAfterRemoval?: (threads: ThreadSummary[], removedThreadId: string) => string | null;
  shouldHydrateThreadDetail?: (threadId: string | null | undefined, detail?: { summary: ThreadSummary } | null) => boolean;
  resolvedSelectedThreadId?: (selectedId: string | "__new" | null) => string | null;
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
  failureCategoryLabel?: (category: string, capabilities?: RuntimeCapabilityMatrix) => string;
  jobFailureAnalysisView?: (
    analysis: NonNullable<import("./types").JobRecord["failure_analysis"]>,
    capabilities?: RuntimeCapabilityMatrix
  ) => { label: string; explanation: string; suggestions: string[] };
  optionalUnavailableMessage?: (feature: string, result?: { available: boolean; reason?: string | null; error?: string | null } | null) => string;
  renderConversationHeaderHtml?: (summary: ThreadSummary) => string;
  preservePreviousQueryData?: <T>(previous: T | undefined) => T | undefined;
  threadCopyId?: (threadId?: string | null) => string | null;
  opsWorkspacePanelTitles?: (capabilities?: RuntimeCapabilityMatrix) => string[];
  opsWorkspaceVisibleCopy?: (capabilities?: RuntimeCapabilityMatrix) => string[];
  archivePlanAfterExecute?: (
    current: import("./types").ArchiveDeletePlan | null,
    result: Pick<import("./types").ArchiveDeleteResult, "after_total_threads" | "after_active_threads" | "after_archived_threads" | "after_integrity">
  ) => import("./types").ArchiveDeletePlan | null;
  canStartUpdateInstall?: (status?: UpdateStatus | null) => boolean;
  opsUpdateActionView?: (
    status?: UpdateStatus | null,
    capabilities?: RuntimeCapabilityMatrix
  ) => Array<{ action: "check" | "install" | "prune"; label: string; disabled: boolean }>;
  threadInspectorActionState?: (capabilities?: RuntimeCapabilityMatrix) => {
    showFork: boolean;
    showArchive: boolean;
    approvalMode: "interactive" | "unsupported";
  };
  desktopRuntimeVisibleCopy?: () => string[];
  runtimeCapabilitiesForRuntime?: (runtime?: boolean | "web" | "desktop") => RuntimeCapabilityMatrix;
  navigationLabelsForRuntime?: (capabilities?: RuntimeCapabilityMatrix) => string[];
  shouldShowLogoutForRuntime?: (capabilities?: RuntimeCapabilityMatrix) => boolean;
  initialSessionForRuntime?: (capabilities?: RuntimeCapabilityMatrix) => import("./types").SessionUser | null;
};

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
  return import("./App") as Promise<AppExports>;
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

function extractThreadInspectorSource(): string {
  const source = threadInspectorPanelsSource;
  const start = source.indexOf("function ThreadInspectorPanels(");
  const end = source.length;

  expect(start).toBeGreaterThanOrEqual(0);
  expect(end).toBeGreaterThan(start);
  return source.slice(start, end);
}

function extractProbeWorkspaceSource(): string {
  const source = probeWorkspaceSource;
  const start = source.indexOf("function ProbeWorkspace(");

  expect(start).toBeGreaterThanOrEqual(0);
  return source.slice(start);
}

function extractFunctionSource(name: string): string {
  const source = name === "SlashCommandTextarea"
    ? composerControlsSource
    : name === "OpsWorkspace"
      ? opsWorkspaceSource
      : name === "JobList"
        ? jobListSource
        : name === "ProbeWorkspace"
          ? probeWorkspaceSource
          : name === "ChatWorkspace" || name === "ThreadList"
            ? chatWorkspaceSource
            : name === "ThreadGoalPanel"
              ? threadGoalPanelSource
              : name === "ThreadInspectorPanels"
                ? threadInspectorPanelsSource
                : name === "RunConfigControls"
                  ? runConfigControlsSource
                  : name === "MessageBlockView"
                    ? messageStreamSource
                    : name === "CurrentActionCard"
                      ? currentActionCardSource
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
  test("desktop runtime hides Web-only auth and security navigation", async () => {
    const app = await loadApp();
    const webCapabilities = linuxWebCapabilities;
    const desktopCapabilities = macosDesktopCapabilities;

    expect(webCapabilities).toBeDefined();
    expect(desktopCapabilities).toBeDefined();
    expect(app.navigationLabelsForRuntime?.(webCapabilities)).toContain("安全");
    expect(app.navigationLabelsForRuntime?.(desktopCapabilities)).toEqual(["Codex", "Claude Code", "探针", "运维"]);
    expect(app.shouldShowLogoutForRuntime?.(webCapabilities)).toBe(true);
    expect(app.shouldShowLogoutForRuntime?.(desktopCapabilities)).toBe(false);
    expect(app.initialSessionForRuntime?.(desktopCapabilities)).toMatchObject({
      username: "desktop",
      csrf_token: null
    });
  }, 15000);

  test("security workspace is gated by web security capability and derives endpoint copy", () => {
    const securityWorkspaceSource = extractFunctionSource("SecurityWorkspace");
    const shellSource = appSource.slice(
      appSource.indexOf("function App()"),
      appSource.indexOf("class WorkspaceErrorBoundary")
    );

    expect(shellSource).toContain('capabilities.securitySettings && view === "security"');
    expect(securityWorkspaceSource).toContain("useSystemStatusQuery");
    expect(securityWorkspaceSource).toContain('value={expectedHostname ?? "未配置"}');
    expect(securityWorkspaceSource).toContain('placeholder={defaultExpectedHostname ?? "未配置"}');
    expect(securityWorkspaceSource).not.toContain('|| "661313.xyz"');
    expect(securityWorkspaceSource).not.toContain("661313.xyz");
  });

  test("desktop runtime keeps shared update entry but removes Linux-only update actions", async () => {
    const app = await loadApp();
    const webCapabilities = linuxWebCapabilities;
    const desktopCapabilities = macosDesktopCapabilities;

    expect(app.navigationLabelsForRuntime?.(desktopCapabilities)).toEqual(expect.arrayContaining(["Codex", "探针", "运维"]));
    expect(app.opsWorkspacePanelTitles?.(webCapabilities)).toContain("NexusHub 更新");
    expect(app.opsWorkspacePanelTitles?.(desktopCapabilities)).toContain("NexusHub 更新");
    expect(app.opsWorkspaceVisibleCopy?.(desktopCapabilities)).not.toEqual(expect.arrayContaining(["Precheck", "Prune", "Public endpoint"]));
    expect(app.opsWorkspaceVisibleCopy?.(desktopCapabilities)).not.toEqual(expect.arrayContaining(["state DB", "Codex Home", "State DB"]));
    expect(app.opsWorkspaceVisibleCopy?.(desktopCapabilities).join("\n")).not.toMatch(/systemd|Nginx|管理员密码|Turnstile|Linux prune/i);
    expect(app.opsWorkspaceVisibleCopy?.(desktopCapabilities)).toEqual(expect.arrayContaining(["系统状态", "NexusHub 更新", "Check", "Install", "归档线程清理", "隐藏线程清理", "Job History"]));
    expect(app.opsWorkspaceVisibleCopy?.(webCapabilities)).toEqual(expect.arrayContaining(["Public endpoint", "state DB", "Codex Home", "State DB", "Precheck", "Update", "Prune", "systemd 失败", "Nginx 失败"]));
    expect(app.opsUpdateActionView?.(null, desktopCapabilities).map((action) => action.label)).toEqual(["Check", "Install"]);
    expect(app.opsUpdateActionView?.({ update_available: true } as UpdateStatus, webCapabilities).map((action) => action.label)).toEqual(["Precheck", "Update", "Prune"]);
    expect(app.opsUpdateActionView?.({ update_available: false } as UpdateStatus, desktopCapabilities).find((action) => action.action === "install")?.disabled).toBe(true);
  });

  test("desktop runtime hides unsupported fork and approval actions", async () => {
    const app = await loadApp();
    const webCapabilities = linuxWebCapabilities;
    const desktopCapabilities = macosDesktopCapabilities;

    expect(webCapabilities).toBeDefined();
    expect(desktopCapabilities).toBeDefined();
    expect(app.canShowForkAction?.(webCapabilities)).toBe(true);
    expect(app.canShowForkAction?.(desktopCapabilities)).toBe(false);
    expect(app.slashCommandsForRuntime?.(webCapabilities).map((item) => item.command)).toContain("/fork");
    expect(app.slashCommandsForRuntime?.(desktopCapabilities).map((item) => item.command)).not.toContain("/fork");
    expect(app.slashCommandsForRuntime?.(webCapabilities).map((item) => item.command)).toContain("/logout");
    expect(app.slashCommandsForRuntime?.(desktopCapabilities).map((item) => item.command)).not.toContain("/logout");
    expect(app.slashCommandSuggestions?.("/fo", 3, true, webCapabilities).map((item) => item.command)).toContain("/fork");
    expect(app.slashCommandSuggestions?.("/fo", 3, true, desktopCapabilities).map((item) => item.command)).not.toContain("/fork");
    expect(app.slashCommandSuggestions?.("/lo", 3, true, webCapabilities).map((item) => item.command)).toContain("/logout");
    expect(app.slashCommandSuggestions?.("/lo", 3, true, desktopCapabilities).map((item) => item.command)).not.toContain("/logout");
    expect(app.exactSlashCommandFromDraft?.(" /fork ", desktopCapabilities)).toBeNull();
    expect(app.exactSlashCommandFromDraft?.(" /logout ", desktopCapabilities)).toBeNull();
    expect(app.slashCommandForComposerSubmit?.(" /fork ", desktopCapabilities)).toBeNull();
    expect(app.slashCommandForComposerSubmit?.(" /logout ", desktopCapabilities)).toBeNull();
    expect(app.slashCommandAction?.("/fork", true, desktopCapabilities)).toEqual({
      kind: "unknown",
      command: "/fork",
      message: expect.stringContaining("未知")
    });
    expect(app.approvalActionMode?.(webCapabilities)).toBe("interactive");
    expect(app.approvalActionMode?.(desktopCapabilities)).toBe("unsupported");
  });

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

  test("component sources use capability props instead of runtime or transport access", () => {
    const guardedComponents = [
      "App",
      "SideNav",
      "MobileTopBar",
      "ChatWorkspace",
      "Conversation",
      "EmptyConversation",
      "SlashCommandTextarea",
      "ProbeWorkspace",
      "OpsWorkspace",
      "JobList"
    ];

    for (const component of guardedComponents) {
      const source = extractFunctionSource(component);
      expectSourceToAvoidTokens(source, component, forbiddenComponentTokens);
    }
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

  test("Conversation passes semantic callbacks while thread query facade owns mutation cache lifecycle", async () => {
    const threadQuery = await loadThreadQuery();
    const conversationSource = extractFunctionSource("Conversation");

    expect(typeof threadQuery.useThreadConversationActions).toBe("function");
    expect(conversationSource).toContain("useThreadConversationActions");
    expect(conversationSource).not.toContain("onSendSuccess");
    expect(conversationSource).not.toContain("onSteerSuccess");
    expect(conversationSource).not.toContain("onArchiveMutate");
    expect(conversationSource).not.toContain("onArchiveError");
    expect(conversationSource).not.toContain("onArchiveSettled");
    expect(conversationSource).not.toContain("onRenameMutate");
    expect(conversationSource).not.toContain("onRenameSuccess");
    expect(conversationSource).not.toContain("onRenameError");
    expect(conversationSource).not.toContain("onRenameSettled");
    expect(conversationSource).not.toContain("invalidateJobs");
    expect(conversationSource).not.toContain("invalidateFollowUps");
    expect(conversationSource).not.toContain("applyOptimisticThread");
    expect(threadQuerySource).toContain("onSendSuccess");
    expect(threadQuerySource).toContain("onArchiveMutate");
    expect(threadQuerySource).toContain("onRenameError");
  });

  test("components consume query/state layer instead of direct domain API functions", () => {
    expect(appSource, "App.tsx must not import the domain API barrel").not.toContain("from \"./lib/api\"");
    expect(appSource, "App.tsx must not import React Query client primitives").not.toContain("@tanstack/react-query");
    expect(appSource, "App.tsx must not create raw mutations; use query/state action hooks").not.toContain("useMutation(");
    expect(appSource, "App.tsx must not hold the query client").not.toContain("useQueryClient");
    expect(appSource, "App.tsx must not write query cache directly").not.toContain("setQueryData");
    expect(appSource, "App.tsx must not invalidate query cache directly").not.toContain("invalidateQueries");
    expect(appSource, "App.tsx must not cancel query cache directly").not.toContain("cancelQueries");
    expect(appSource, "App.tsx must not remove query cache directly").not.toContain("removeQueries");
    expect(appSource, "App.tsx must not type against QueryClient").not.toMatch(/\bQueryClient\b/);
    expect(appSource, "App.tsx must not type against QueryKey").not.toMatch(/\bQueryKey\b/);

    const appImportBlock = appSource.slice(0, appSource.indexOf("import { clearSession"));
    const queryProxiedApiFunctions = [
      "acceptPlan",
      "answerApproval",
      "answerElicitation",
      "archiveThread",
      "cancelFollowUp",
      "changePassword",
      "clearCodexGoal",
      "createThread",
      "deleteUpload",
      "dryRunArchiveDelete",
      "dryRunHiddenThreadDelete",
      "forkThread",
      "getClaudeCodeOverview",
      "getCodexConfig",
      "getCodexGoal",
      "getPlatformOverview",
      "getProbeEvents",
      "getProbeLogsDbStatus",
      "getProbeSettings",
      "getProbeStatus",
      "getPublicSettings",
      "getSecurity",
      "getSystemStatus",
      "getThread",
      "getThreadBlocks",
      "getUpdateStatus",
      "listFollowUps",
      "listJobs",
      "listModels",
      "listPermissionProfiles",
      "listPlugins",
      "listProviders",
      "listThreads",
      "login",
      "logout",
      "pauseCodexGoal",
      "renameThread",
      "restoreThread",
      "resumeCodexGoal",
      "revisePlan",
      "saveCodexGoal",
      "saveProbeSettings",
      "saveSecurity",
      "sendMessage",
      "startArchiveDelete",
      "startHiddenThreadDelete",
      "runProbeBarkTest",
      "runProbeHooksInstall",
      "runProbeLogsDbDryRun",
      "runProbeLogsDbExecute",
      "steerThread",
      "stopThread",
      "uploadFiles"
    ];

    for (const name of queryProxiedApiFunctions) {
      expect(appImportBlock, `App.tsx should consume ${name} via query/state hooks`).not.toMatch(new RegExp(`\\b${name}\\b`));
    }

    const threadQueryPublicExports = threadQuerySource.slice(threadQuerySource.lastIndexOf("export {"));
    for (const name of queryProxiedApiFunctions) {
      expect(threadQueryPublicExports, `query/threads.ts must not re-export raw ${name}`).not.toMatch(new RegExp(`\\b${name}\\b`));
    }

    const opsSource = extractFunctionSource("OpsWorkspace");
    const probeSource = extractProbeWorkspaceSource();
    const chatWorkspaceSource = extractFunctionSource("ChatWorkspace");
    const conversationSource = extractFunctionSource("Conversation");
    const emptyConversationSource = extractFunctionSource("EmptyConversation");
    const goalPanelSource = extractFunctionSource("ThreadGoalPanel");
    for (const source of [opsSource, probeSource, chatWorkspaceSource, conversationSource, emptyConversationSource, goalPanelSource]) {
      for (const name of queryProxiedApiFunctions) {
        expect(source, `workspace should not directly call ${name}`).not.toMatch(new RegExp(`\\b${name}\\b`));
      }
      expect(source).not.toContain("useQueryClient");
      expect(source).not.toContain("setQueryData");
      expect(source).not.toContain("invalidateQueries");
      expect(source).not.toContain("cancelQueries");
      expect(source).not.toContain("removeQueries");
    }
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

    expect(app.slashCommandSuggestions?.("/", 1, true, linuxWebCapabilities).map((item) => item.command)).toEqual(app.slashCommands?.map((item) => item.command));
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

    const html = app.renderSlashCommandMenuHtml?.("/archive", 8, false, 0, linuxWebCapabilities);

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
    expect(app.exactSlashCommandFromDraft?.(" /fork ", linuxWebCapabilities)).toBe("/fork");
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
    expect(app.slashCommandAction?.("/archive")).toEqual({
      kind: "unknown",
      command: "/archive",
      message: expect.stringContaining("未知")
    });
    expect(app.slashCommandAction?.("/archive", true, linuxWebCapabilities)).toEqual({ kind: "archive_thread", command: "/archive" });
    expect(app.slashCommandAction?.("/archive", false, linuxWebCapabilities)).toEqual({
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

  test("slash command execution planning is delegated out of Conversation switch logic", async () => {
    const app = await loadApp();
    const conversationSource = extractFunctionSource("Conversation");

    expect(typeof app.slashCommandExecutionPlan).toBe("function");
    expect(app.slashCommandExecutionPlan?.({
      command: "/fork",
      hasThread: true,
      capabilities: macosDesktopCapabilities,
      inspectorActions: { showFork: false, showArchive: true, approvalMode: "unsupported" },
      supportsFast: false,
      serviceTier: "",
      latestAssistantCopy: "ready"
    })).toEqual({
      kind: "feedback",
      draft: "",
      message: "macOS App 当前不支持 Fork 操作"
    });
    expect(app.slashCommandExecutionPlan?.({
      command: "/fast",
      hasThread: true,
      capabilities: macosDesktopCapabilities,
      inspectorActions: { showFork: false, showArchive: true, approvalMode: "unsupported" },
      supportsFast: false,
      serviceTier: "",
      latestAssistantCopy: "ready"
    })).toEqual({
      kind: "feedback",
      draft: "",
      message: "当前模型不支持 Fast service tier"
    });
    expect(app.slashCommandExecutionPlan?.({
      command: "/copy",
      hasThread: true,
      capabilities: linuxWebCapabilities,
      inspectorActions: { showFork: true, showArchive: true, approvalMode: "interactive" },
      supportsFast: true,
      serviceTier: "",
      latestAssistantCopy: null
    })).toEqual({
      kind: "feedback",
      draft: "",
      message: "没有可复制的最新回复"
    });
    expect(conversationSource).toContain("slashCommandExecutionPlan");
    expect(conversationSource).not.toContain("slashCommandAction(");
    expect(conversationSource).not.toMatch(/switch\s*\(\s*action\.kind\s*\)/);
    expect(conversationSource).not.toContain("当前模型不支持 Fast service tier");
    expect(conversationSource).not.toContain("macOS App 当前不支持 Fork 操作");
    expect(conversationSource).not.toContain("没有可复制的最新回复");
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
    expect(app.resolvedSelectedThreadId?.("__new")).toBeNull();
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

  test("composer file picker has no accept whitelist", async () => {
    const app = await loadApp();

    expect(app.composerFileInputAcceptValue?.()).toBeUndefined();
  });

  test("thread copy panel restores id, rollout path, and resume command without internal metrics", async () => {
    const app = await loadApp();
    const inspectorSource = extractThreadInspectorSource();

    expect(app.threadInspectorPanelTitles?.()).toEqual(["名称与归档", "Goal", "复制与路径"]);
    expect(app.threadSettingsMetricLabels?.()).not.toEqual(expect.arrayContaining([
      "Thread ID",
      "Active turn",
      "Active job",
      "Last event",
      "Rollout path",
      "Blocks"
    ]));
    expect(app.threadInspectorPanelTitles?.()).not.toContain("状态摘要");
    expect(app.threadResumeCommand?.("019ec943-0b86-7e22-86e9-4dc0c919b09d")).toBe(
      "codex resume 019ec943-0b86-7e22-86e9-4dc0c919b09d"
    );
    expect(app.threadCopyId?.("019ec943-0b86-7e22-86e9-4dc0c919b09d")).toBe(
      "019ec943-0b86-7e22-86e9-4dc0c919b09d"
    );
    expect(app.threadRolloutPath?.(" /Users/gosu/.codex/sessions/thread.jsonl ")).toBe(
      "/Users/gosu/.codex/sessions/thread.jsonl"
    );
    expect(app.threadCopyId?.("  ")).toBeNull();
    expect(app.threadCopyId?.(null)).toBeNull();
    expect(app.threadRolloutPath?.("  ")).toBeNull();
    expect(app.threadRolloutPath?.(null)).toBeNull();
    expect(app.threadResumeCommand?.("  ")).toBeNull();
    expect(app.threadResumeCommand?.(null)).toBeNull();
    expect(inspectorSource).toContain("复制与路径");
    expect(inspectorSource).toContain("线程 ID");
    expect(inspectorSource).toContain("复制 ID");
    expect(inspectorSource).toContain("复制文件路径");
    expect(inspectorSource).toContain("复制 codex resume+ID");
    expect(inspectorSource).toContain("会话文件");
    expect(inspectorSource).not.toContain("状态摘要");
    expect(inspectorSource).not.toContain("Codex Home");
    expect(inspectorSource).not.toContain("State DB");
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
    expect(app.goalStatusLabel?.({ ...active, available: false }, false)).toBe("未接入");
    expect(app.goalControlState?.({ ...active, available: false }, { objective: "补齐右栏", tokenBudget: "12000" })).toEqual({
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

  test("probe running summary uses running_threads when backend count is stale", async () => {
    const app = await loadApp();
    const probeSource = extractProbeWorkspaceSource();
    const runningThreads: ThreadSummary[] = [
      { id: "running-a", title: "运行 A", status: "Running", message_count: 1 },
      { id: "running-b", title: "运行 B", status: "Running", message_count: 2 }
    ];

    expect(app.probeThreadsByStatus?.({ running_threads: runningThreads }).running).toEqual(runningThreads);
    expect(app.probeRunningCountValue?.({ running_count: 0, running_threads: runningThreads })).toBe("2");
    expect(app.probeRunningCountValue?.({ running_count: 3, running_threads: runningThreads.slice(0, 1) })).toBe("3");
    expect(probeSource).toContain('<Metric label="运行中" value={probeRunningCountValue(data)}');
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

  test("probe path metrics keep Linux Codex Home while hiding it from desktop runtime", () => {
    const probeSource = extractProbeWorkspaceSource();

    expect(probeSource).toContain('{capabilities.codexStatePaths && <Metric label="Codex Home" value={codexHomeStatusValue(data ?? currentSettings?.codex)} wide />}');
    expect(probeSource).toContain('{capabilities.codexStatePaths && <label className="field-label">Codex Home<input value={draft.codex.home} placeholder="auto" onChange={(event) => setCodex({ home: event.target.value })} /></label>}');
    expect(probeSource).toContain('<Metric label="Logs DB Path" value={logsDbPathStatusValue(logsDb ?? settings?.logs_db)} wide />');
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

  test("thread inspector gating and desktop ops copy stay capability-driven", async () => {
    const app = await loadApp();
    const desktopVisibleCopy = app.opsWorkspaceVisibleCopy?.(macosDesktopCapabilities).join("\n") ?? "";
    const linuxVisibleCopy = app.opsWorkspaceVisibleCopy?.(linuxWebCapabilities).join("\n") ?? "";

    expect(app.threadInspectorActionState?.(macosDesktopCapabilities)).toEqual({
      showFork: false,
      showArchive: true,
      approvalMode: "unsupported"
    });
    expect(app.threadInspectorActionState?.(linuxWebCapabilities)).toEqual({
      showFork: true,
      showArchive: true,
      approvalMode: "interactive"
    });

    expect(desktopVisibleCopy).not.toContain("Web 登录");
    expect(desktopVisibleCopy).not.toContain("Turnstile");
    expect(desktopVisibleCopy).not.toContain("systemd");
    expect(desktopVisibleCopy).not.toContain("Nginx");
    expect(desktopVisibleCopy).not.toContain("管理员密码");
    expect(desktopVisibleCopy).not.toContain("公网入口");
    expect(desktopVisibleCopy).not.toContain("Linux update");
    expect(desktopVisibleCopy).not.toContain("Linux prune");

    expect(linuxVisibleCopy).toContain("Public endpoint");
    expect(linuxVisibleCopy).toContain("Prune");
    expect(linuxVisibleCopy).toContain("systemd 失败");
    expect(linuxVisibleCopy).toContain("Nginx 失败");
  });

});
