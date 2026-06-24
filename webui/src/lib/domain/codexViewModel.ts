import type {
  BridgeActionResult,
  CodexConfig,
  CodexModel,
  ThreadDetail,
  ThreadStatus,
  ThreadSummary,
  UploadRecord
} from "../../types";
import { isNoisyThreadTitle, mergeThreadSummaryTitle } from "../threadMessageStore";
import { capabilitiesForInput, resolvedSelectedThreadId, type RuntimeCapabilityInput } from "./runtimeViewModel";

export type View = "codex" | "claude" | "probe" | "ops" | "security";
export type SelectedThread = string | "__new" | null;
export type PermissionPresetId = "ask" | "auto" | "full" | "custom";

export type RunConfig = {
  model: string;
  serviceTier: string;
  reasoning: string;
  cwd: string;
  permissionPreset: PermissionPresetId;
  permissionProfile: string;
  approvalPolicy: string;
  sandboxMode: string;
  networkAccess: boolean | null;
  collaborationMode: string;
};

export type ThreadSendPayload = {
  message: string;
  attachments?: string[];
  model?: string | null;
  service_tier?: string | null;
  reasoning_effort?: string | null;
  cwd?: string | null;
  permission_profile?: string | null;
  approval_policy?: string | null;
  sandbox_mode?: string | null;
  network_access?: boolean | null;
  collaboration_mode?: string | null;
};

export type ThreadTitleLike = {
  title?: string | null;
  [key: string]: unknown;
};

export type ThreadListItemLike = ThreadTitleLike & {
  status?: ThreadStatus | string | null;
  latest_message?: string | null;
};

export const codexLocalCopy = {
  loginSubtitle: "Codex 本地状态控制台",
  threadListEyebrow: "Codex 本地线程"
};

export const reasoningOptions = ["", "low", "medium", "high", "xhigh"];
export const defaultSessionTtlDays = 365;
export const secondsPerDay = 86400;

const defaultCwd = "";

export function visibleNavigationItems<T extends { id: View }>(items: T[], input?: RuntimeCapabilityInput): T[] {
  return capabilitiesForInput(input).securitySettings
    ? items
    : items.filter((item) => item.id !== "security");
}

export function navigationLabelsForRuntime<T extends { id: View; label: string }>(items: T[], input?: RuntimeCapabilityInput): string[] {
  return visibleNavigationItems(items, input).map((item) => item.label);
}

export function shouldShowLogoutForRuntime(input?: RuntimeCapabilityInput): boolean {
  return capabilitiesForInput(input).logout;
}

export function shouldUseSavedSessionForRuntime(input?: RuntimeCapabilityInput): boolean {
  return capabilitiesForInput(input).webAuth;
}

export function makeRunConfig(config?: CodexConfig, summary?: ThreadSummary): RunConfig {
  return {
    model: summary?.model ?? config?.model ?? "gpt-5.5",
    serviceTier: normalizeServiceTier(config?.service_tier),
    reasoning: config?.reasoning_effort ?? "xhigh",
    cwd: summary?.cwd ?? config?.cwd ?? defaultCwd,
    permissionPreset: permissionPresetFromConfig(config),
    permissionProfile: config?.permission_profile ?? "",
    approvalPolicy: config?.approval_policy ?? "never",
    sandboxMode: config?.sandbox_mode ?? "danger-full-access",
    networkAccess: config?.network_access ?? true,
    collaborationMode: ""
  };
}

function normalizeServiceTier(value?: string | null): string {
  if (value === "fast") return "priority";
  return value?.trim() || "";
}

function permissionPresetFromConfig(config?: CodexConfig): PermissionPresetId {
  if (!config?.approval_policy && !config?.sandbox_mode && !config?.permission_profile) return "custom";
  if (config?.approval_policy === "on-request" && config?.sandbox_mode === "workspace-write") return "ask";
  if (config?.approval_policy === "untrusted" && config?.sandbox_mode === "workspace-write") return "auto";
  if (config?.approval_policy === "never" && config?.sandbox_mode === "danger-full-access") return "full";
  return "custom";
}

export function defaultRunConfig(): RunConfig {
  return makeRunConfig();
}

export function applyPermissionPreset(config: RunConfig, preset: PermissionPresetId): RunConfig {
  if (preset === "custom") {
    return {
      ...config,
      permissionPreset: preset,
      permissionProfile: "",
      approvalPolicy: "",
      sandboxMode: "",
      networkAccess: null
    };
  }
  if (preset === "full") {
    return {
      ...config,
      permissionPreset: preset,
      permissionProfile: "",
      approvalPolicy: "never",
      sandboxMode: "danger-full-access",
      networkAccess: true
    };
  }
  return {
    ...config,
    permissionPreset: preset,
    permissionProfile: "",
    approvalPolicy: preset === "auto" ? "untrusted" : "on-request",
    sandboxMode: "workspace-write",
    networkAccess: true
  };
}

export function buildPayload(message: string, config: RunConfig, attachments: Pick<UploadRecord, "id">[] = []): ThreadSendPayload {
  const attachmentIds = attachments.map((attachment) => attachment.id).filter(Boolean);
  const payload: ThreadSendPayload = {
    message,
    model: config.model.trim() || null,
    service_tier: config.serviceTier.trim() || null,
    reasoning_effort: config.reasoning.trim() || null,
    cwd: config.cwd.trim() || null,
    permission_profile: config.permissionProfile.trim() || null,
    approval_policy: config.approvalPolicy.trim() || null,
    sandbox_mode: config.sandboxMode.trim() || null,
    network_access: config.networkAccess,
    collaboration_mode: config.collaborationMode.trim() || null
  };
  if (attachmentIds.length > 0) {
    payload.attachments = attachmentIds;
  }
  return payload;
}

export function mergeRunConfigFromDefaults<T extends { collaborationMode: string }>(current: T, defaults: T): T {
  return {
    ...defaults,
    collaborationMode: current.collaborationMode
  };
}

export function runConfigAfterSuccessfulSend<T extends { collaborationMode: string }>(config: T): T {
  return config;
}

export function modelSupportsServiceTier(models: CodexModel[], modelId: string, tierId: string): boolean {
  const model = models.find((item) => item.id === modelId);
  return Boolean(model?.service_tiers?.some((tier) => tier.id === tierId));
}

export function runConfigWithSupportedServiceTier(config: RunConfig, models: CodexModel[]): RunConfig {
  if (!config.serviceTier.trim()) return config;
  if (modelSupportsServiceTier(models, config.model, config.serviceTier.trim())) return config;
  return { ...config, serviceTier: "" };
}

export function threadListItemText(thread: ThreadTitleLike): string {
  return thread.title?.trim() || "未命名线程";
}

export function filterVisibleThreadSummaries<T extends Partial<ThreadSummary>>(threads: T[]): T[] {
  return threads.filter(isVisibleMainThread);
}

export function isVisibleMainThread(thread: Partial<ThreadSummary>): boolean {
  if (thread.status === "Archived" || thread.archived_at) return false;
  if (nonEmptyString(thread.parentThreadId ?? thread.parent_thread_id)) return false;
  if (nonEmptyString(thread.agentPath ?? thread.agent_path)) return false;
  if (nonEmptyString(thread.agentNickname ?? thread.agent_nickname)) return false;
  if (nonEmptyString(thread.agentRole ?? thread.agent_role)) return false;
  if (fieldContainsSubagent(thread.threadSource ?? thread.thread_source)) return false;
  if (fieldContainsSubagent(thread.sourceKind ?? thread.source_kind)) return false;
  if (sourceValueContainsSubagent(thread.source)) return false;
  return !isInternalExecThread(thread);
}

export function threadListItemStatusText(thread: ThreadListItemLike): string {
  return threadStatusLabel(thread.status);
}

export function threadListItemPreviewText(thread: ThreadListItemLike): string {
  return cleanThreadPreviewText(thread.latest_message);
}

export function cleanThreadPreviewText(value?: string | null): string {
  const source = value?.trim();
  if (!source) return "";
  return extractPlanText(source).replace(/\s+/g, " ").trim();
}

export function conversationTitleText(thread: ThreadTitleLike): string {
  return thread.title?.trim() || "未命名线程";
}

export function renderConversationHeaderHtml(summary: ThreadSummary): string {
  const title = conversationTitleText(summary);
  return `<div class="conversation-title-copy"><h2 class="conversation-title" title="${escapeHtml(title)}">${escapeHtml(title)}</h2></div>`;
}

const threadTitleOverrides = new Map<string, { title: string; expiresAt: number }>();
const threadTitleOverrideTtlMs = 120_000;

export function setLocalThreadTitleOverride(threadId: string, title: string, now = Date.now()): void {
  const cleanThreadId = threadId.trim();
  const cleanTitle = title.trim();
  if (!cleanThreadId || !cleanTitle) return;
  threadTitleOverrides.set(cleanThreadId, {
    title: cleanTitle,
    expiresAt: now + threadTitleOverrideTtlMs
  });
}

export function clearLocalThreadTitleOverride(threadId: string): void {
  threadTitleOverrides.delete(threadId);
}

export function applyThreadTitleOverride<T extends Partial<ThreadSummary>>(summary: T, now = Date.now()): T {
  const threadId = summary.id?.trim();
  if (!threadId) return summary;
  const override = threadTitleOverrides.get(threadId);
  if (!override) return summary;
  if (override.expiresAt <= now) {
    threadTitleOverrides.delete(threadId);
    return summary;
  }
  return {
    ...summary,
    title: override.title
  };
}

export function applyThreadTitleOverrides<T extends Partial<ThreadSummary>>(threads: T[]): T[] {
  return threads.map((thread) => applyThreadTitleOverride(thread));
}

export function applyThreadTitleOverrideToDetail(detail: ThreadDetail): ThreadDetail {
  const summary = applyThreadTitleOverride(detail.summary);
  return summary === detail.summary ? detail : { ...detail, summary: summary as ThreadSummary };
}

export function mergeIncomingThreadSummary<T extends Partial<ThreadSummary>>(current: T, incoming: Partial<ThreadSummary>): T & Partial<ThreadSummary> {
  const effectiveIncoming = applyThreadTitleOverride(incoming);
  const next = { ...current, ...effectiveIncoming };
  next.title = mergeThreadSummaryTitle(current.title, effectiveIncoming.title);
  if (!isUserVisibleLastEventKind(effectiveIncoming.last_event_kind) && isUserVisibleLastEventKind(current.last_event_kind)) {
    next.last_event_kind = current.last_event_kind;
  }
  return next;
}

export function lastEventKindText(summary: Pick<ThreadSummary, "last_event_kind">): string {
  const value = summary.last_event_kind?.trim();
  if (!isUserVisibleLastEventKind(value)) return "未知";
  return value || "未知";
}

export function mergeThreadDetailSummaryFromList(detail: ThreadDetail, incoming: Partial<ThreadSummary>): ThreadDetail {
  return {
    ...detail,
    summary: mergeIncomingThreadSummary(detail.summary, incoming) as ThreadSummary
  };
}

export function threadMatchesListFilter(thread: Partial<ThreadSummary>, status = "all", q = ""): boolean {
  if (!isVisibleMainThread(thread)) return false;
  if (status !== "all") {
    if (status === "running" && !isThreadListItemRunning(thread)) return false;
    if (status === "reply-needed" && thread.status !== "ReplyNeeded") return false;
    if (status === "recoverable" && thread.status !== "Recoverable") return false;
    if (!["running", "reply-needed", "recoverable"].includes(status) && thread.status !== status) return false;
  }
  const needle = q.trim().toLowerCase();
  if (!needle) return true;
  return [
    thread.id,
    thread.title,
    thread.latest_message
  ].some((value) => String(value ?? "").toLowerCase().includes(needle));
}

export function nextVisibleThreadIdAfterRemoval(threads: ThreadSummary[], removedThreadId: string): string | null {
  const visible = filterVisibleThreadSummaries(threads);
  const removedIndex = visible.findIndex((thread) => thread.id === removedThreadId);
  const remaining = visible.filter((thread) => thread.id !== removedThreadId);
  if (!remaining.length) return null;
  if (removedIndex < 0) return remaining[0].id;
  return (remaining[removedIndex] ?? remaining[removedIndex - 1] ?? remaining[0]).id;
}

export type ThreadSelectionView = {
  visibleThreads: ThreadSummary[];
  resolvedSelected: string | null;
  selectedThreadSummary: ThreadSummary | null;
  nextThreadAfterRemoval: string | null;
};

export function threadSelectionView(input: {
  threads: ThreadSummary[];
  selectedId: SelectedThread;
}): ThreadSelectionView {
  const visibleThreads = filterVisibleThreadSummaries(input.threads);
  const resolvedSelected = resolvedSelectedThreadId(input.selectedId);
  const selectedThreadSummary = visibleThreads.find((thread) => thread.id === resolvedSelected) ?? null;
  return {
    visibleThreads,
    resolvedSelected,
    selectedThreadSummary,
    nextThreadAfterRemoval: resolvedSelected ? nextVisibleThreadIdAfterRemoval(visibleThreads, resolvedSelected) : null
  };
}

export function selectedThreadDetailView(input: {
  threadId: string | null | undefined;
  detail?: ThreadDetail | null;
}): { rawSelectedDetail: ThreadDetail | null; selectedDetail: ThreadDetail | null } {
  const rawSelectedDetail = input.detail && input.detail.summary.id === input.threadId ? input.detail : null;
  return {
    rawSelectedDetail,
    selectedDetail: shouldHydrateThreadDetail(input.threadId, rawSelectedDetail) ? rawSelectedDetail : null
  };
}

export type ArchivedSelectedThreadCleanupView = {
  shouldClearClientState: boolean;
  nextSelectedId: SelectedThread;
};

export function archivedSelectedThreadCleanupView(input: {
  threadId: string | null | undefined;
  selectedId: SelectedThread;
  detail?: Pick<ThreadDetail, "summary"> | null;
  visibleThreads: ThreadSummary[];
}): ArchivedSelectedThreadCleanupView {
  const shouldClearClientState = Boolean(input.threadId && input.detail?.summary.status === "Archived");
  return {
    shouldClearClientState,
    nextSelectedId: shouldClearClientState && input.selectedId === input.threadId
      ? nextVisibleThreadIdAfterRemoval(input.visibleThreads, input.threadId!)
      : input.selectedId
  };
}

export function shouldHydrateThreadDetail(threadId: string | null | undefined, detail?: Pick<ThreadDetail, "summary"> | null): detail is ThreadDetail {
  return Boolean(threadId && detail?.summary.id === threadId && detail.summary.status !== "Archived");
}

export function threadDetailRefetchInterval(detail?: ThreadDetail, selectedSummary?: Partial<ThreadSummary> | null): number {
  if (detail) return isThreadRunning(detail.summary, detail.blocks, null) ? 2000 : 5000;
  return selectedSummary && isThreadRunning(selectedSummary, [], null) ? 2000 : 5000;
}

export function isThreadRunning(summary: Partial<ThreadSummary>, blocks: unknown[] = [], lastResult?: Partial<BridgeActionResult> | null): boolean {
  void blocks;
  void lastResult;
  if (summary.status === "Running") return true;
  if (summary.status === "Recent" && Boolean(summary.active_job_id)) return true;
  return false;
}

export function isThreadListItemRunning(thread: Partial<ThreadSummary>): boolean {
  return isThreadRunning(thread, [], null);
}

export function threadStatusLabel(status?: ThreadStatus | string | null): string {
  if (status === "ReplyNeeded") return "待回复";
  if (status === "Recoverable") return "异常";
  if (status === "Running") return "运行中";
  if (status === "Archived") return "归档";
  return "最近";
}

export function optionalUnavailableMessage(
  feature: string,
  result?: { available: boolean; reason?: string | null; error?: string | null } | null
): string {
  const label = feature.trim() || "功能";
  if (!result || result.available) return `${label} 已就绪`;
  const detail = result.reason?.trim() || result.error?.trim();
  return detail ? `${label} 不可用：${detail}` : `${label} 暂不可用`;
}

export function sourceCountsText(counts?: Record<string, number> | null): string {
  if (!counts || Object.keys(counts).length === 0) return "暂无";
  return Object.entries(counts)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `${key}:${value}`)
    .join(" ");
}

export function actionMessage(result: BridgeActionResult): string {
  void result;
  return "已提交给 Codex";
}

export function threadSettingsMetricLabels(): string[] {
  return [];
}

export function extractPlanText(value: string): string {
  return value
    .replace(/<\/?proposed_plan>/g, "")
    .trim() || value || "Plan 内容等待 Codex 写入。";
}

function isUserVisibleLastEventKind(value?: string | null): boolean {
  const event = value?.trim();
  return Boolean(event && !event.startsWith("app-server.") && !event.startsWith("panel."));
}

function nonEmptyString(value: unknown): boolean {
  return typeof value === "string" && value.trim().length > 0;
}

function fieldContainsSubagent(value: unknown): boolean {
  return typeof value === "string" && value.toLowerCase().includes("subagent");
}

function sourceValueContainsSubagent(value: unknown): boolean {
  if (typeof value === "string") return value.toLowerCase().includes("subagent");
  if (Array.isArray(value)) return value.some(sourceValueContainsSubagent);
  if (typeof value === "object" && value) {
    return Object.entries(value).some(([key, item]) => key.toLowerCase().includes("subagent") || sourceValueContainsSubagent(item));
  }
  return false;
}

function isInternalExecThread(thread: Partial<ThreadSummary>): boolean {
  if (normalizeString(thread.source) !== "exec") return false;
  if (!explicitlyNoUserEvent(thread.hasUserEvent ?? thread.has_user_event)) return false;

  const threadSource = normalizeString(thread.threadSource ?? thread.thread_source);
  if (threadSource && threadSource !== "user") return false;

  return [
    thread.title,
    thread.firstUserMessage ?? thread.first_user_message,
    thread.preview,
    thread.latest_message
  ].some((value) => typeof value === "string" && isInternalThreadPromptText(value));
}

function explicitlyNoUserEvent(value: unknown): boolean {
  return value === false || value === 0 || value === "0";
}

function normalizeString(value: unknown): string {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

function isInternalThreadPromptText(value: string): boolean {
  const text = value.trim().toLowerCase();
  if (!text) return false;

  const readonlyProbe = text.includes("只读验证")
    || text.includes("只读核查")
    || text.includes("不要修改文件")
    || text.includes("不改文件")
    || text.includes("read-only")
    || text.includes("readonly");
  const agentProbe = text.includes("spawn_agent")
    || text.includes("子代理")
    || text.includes("subagent")
    || text.includes("model_reasoning_effort=xhigh");
  if (readonlyProbe && agentProbe) return true;

  const strongSubagentInstruction = text.includes("你是子代理")
    || text.includes("你是并行子代理")
    || text.includes("you are a subagent");
  const fixedAgentConfig = text.includes("gpt-5.5")
    || text.includes("xhigh")
    || text.includes("model_reasoning_effort=xhigh");
  return strongSubagentInstruction && fixedAgentConfig;
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export { resolvedSelectedThreadId };
