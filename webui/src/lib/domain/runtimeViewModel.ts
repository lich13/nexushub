import type {
  ArchiveDeletePlan,
  ArchiveDeleteResult,
  CodexGoal,
  HiddenThreadDeletePlan,
  HiddenThreadDeleteResult,
  JobRecord,
  ProbeEvent,
  ProbeJobAction,
  ProbeLogsDbStatus,
  ProbeSettings,
  ProbeStatus,
  SystemStatus,
  ThreadDetail,
  ThreadSummary,
  UpdateStatus
} from "../../types";
import { runtimeCapabilitiesForRuntime, type RuntimeCapabilityMatrix } from "./capabilities";
import { hostCapabilityPolicy, redactHostCopy } from "./hostCapabilityPolicy";

export type RuntimeCapabilityInput = RuntimeCapabilityMatrix | undefined;

const DEFAULT_RUNTIME_CAPABILITIES = runtimeCapabilitiesForRuntime("web");
const secondsPerDay = 86400;

export function capabilitiesForInput(input?: RuntimeCapabilityInput): RuntimeCapabilityMatrix {
  return input ?? DEFAULT_RUNTIME_CAPABILITIES;
}

export const OPS_PANEL_TITLES = {
  system: "系统状态",
  updates: "NexusHub 更新",
  desktopWebui: "WebUI 服务",
  archivedCleanup: "归档线程清理",
  hiddenCleanup: "隐藏线程清理",
  jobs: "Job History"
} as const;

export function opsWorkspacePanelTitles(input?: RuntimeCapabilityInput): string[] {
  const capabilities = capabilitiesForInput(input);
  return [
    OPS_PANEL_TITLES.system,
    OPS_PANEL_TITLES.updates,
    ...(capabilities.desktopWebuiControl ? [OPS_PANEL_TITLES.desktopWebui] : []),
    ...(capabilities.threadCleanup ? [OPS_PANEL_TITLES.archivedCleanup, OPS_PANEL_TITLES.hiddenCleanup] : []),
    OPS_PANEL_TITLES.jobs
  ];
}

export function opsWorkspaceVisibleCopy(input?: RuntimeCapabilityInput): string[] {
  const capabilities = capabilitiesForInput(input);
  return [
    ...opsWorkspacePanelTitles(input),
    "Hostname",
    ...(capabilities.publicEndpointStatus ? ["Public endpoint"] : []),
    ...(capabilities.codexStatePaths ? ["state DB", "Codex Home", "State DB"] : []),
    "Hidden threads",
    "Sources",
    "Current",
    "Latest",
    "Update",
    ...opsUpdateActionView(null, capabilities).map((action) => action.label),
    ...(capabilities.desktopWebuiControl ? [
      "WebUI 服务",
      "Status",
      "Enabled",
      "Password",
      "Listen",
      "URL",
      "PID",
      "启用 WebUI 服务",
      "Secure cookie",
      "保存 WebUI 服务",
      "重置 WebUI 密码",
      "启动 WebUI",
      "停止 WebUI"
    ] : []),
    ...(capabilities.threadCleanup ? [
      "Dry-run",
      "清理归档",
      "确认清理归档",
      "扫描隐藏线程",
      "清理隐藏线程",
      "确认清理隐藏",
      "active",
      "archived",
      "integrity",
      "session index",
      "rollout 文件",
      "visible",
      "hidden",
      "sources",
      "rollout 删除结果"
    ] : []),
    failureCategoryLabel("systemd_failure", capabilities),
    failureCategoryLabel("nginx_failure", capabilities),
    failureCategoryLabel("permission_denied_sudo", capabilities)
  ];
}

export function desktopRuntimeVisibleCopy(): string[] {
  return [
    "Codex 本地线程",
    "Goal",
    "Plan Mode",
    "线程工具",
    "名称与归档",
    "线程标题",
    "重命名",
    "归档",
    "恢复",
    "复制与路径",
    "线程 ID",
    "会话文件",
    "复制 ID",
    "复制文件路径",
    "复制 codex resume+ID"
  ];
}

export function canShowForkAction(input?: RuntimeCapabilityInput): boolean {
  return capabilitiesForInput(input).forkAction;
}

export function approvalActionMode(input?: RuntimeCapabilityInput): "interactive" | "unsupported" {
  return capabilitiesForInput(input).approvalActions ? "interactive" : "unsupported";
}

export function threadInspectorActionState(input?: RuntimeCapabilityInput): {
  showFork: boolean;
  showArchive: boolean;
  approvalMode: "interactive" | "unsupported";
} {
  const capabilities = capabilitiesForInput(input);
  return {
    showFork: canShowForkAction(capabilities),
    showArchive: capabilities.threadArchiveActions,
    approvalMode: approvalActionMode(capabilities)
  };
}

export function resolvedSelectedThreadId(selectedId: string | "__new" | null): string | null {
  return selectedId === "__new" ? null : selectedId;
}

export function canStartHiddenThreadDelete(plan: HiddenThreadDeletePlan | null | undefined): boolean {
  return (plan?.hidden_threads ?? 0) > 0;
}

export function canStartUpdateInstall(status: UpdateStatus | null | undefined): boolean {
  return status?.update_available === true;
}

export function opsUpdateActionView(
  status: UpdateStatus | null | undefined,
  input?: RuntimeCapabilityInput
): Array<{ action: "check" | "install" | "prune"; label: string; tone: "secondary" | "primary" | "danger"; disabled: boolean }> {
  const capabilities = capabilitiesForInput(input);
  return [
    {
      action: "check",
      label: capabilities.updateServiceLabels ? "Precheck" : "Check",
      tone: "secondary",
      disabled: false
    },
    {
      action: "install",
      label: capabilities.updateServiceLabels ? "Update" : "Install",
      tone: "primary",
      disabled: !canStartUpdateInstall(status)
    },
    ...(capabilities.updatePrune ? [{
      action: "prune" as const,
      label: "Prune",
      tone: "danger" as const,
      disabled: false
    }] : [])
  ];
}

type Tone = "success" | "warning" | "danger";

type CleanupStageInput = {
  hasPlan: boolean;
  dryRunPending: boolean;
  armed: boolean;
  executePending: boolean;
  executableCount: number;
};

export type CleanupStageView = {
  label: string;
  tone?: Tone;
};

export type HiddenThreadDeleteStatsView = {
  hidden: number;
  visible: number;
  sourceCounts: string;
  integrity: string;
};

export type OpsWorkspaceView = {
  hostname: string;
  publicEndpoint: string | null;
  systemMetrics: RuntimeMetricView[];
  hiddenStats: HiddenThreadDeleteStatsView;
  archivedCleanupStage: CleanupStageView;
  hiddenCleanupStage: CleanupStageView;
  updateActions: ReturnType<typeof opsUpdateActionView>;
};

export type RuntimeMetricView = {
  label: string;
  value: string;
  tone?: Tone;
  wide?: boolean;
};

export type CleanupMutationState = {
  dryRunPending: boolean;
  armed: boolean;
  executePending: boolean;
};

export type OpsWorkspaceViewModel = {
  panelTitles: string[];
  systemMetrics: RuntimeMetricView[];
  updateActions: ReturnType<typeof opsUpdateActionView>;
  archiveCleanup: {
    stage: CleanupStageView;
    canArm: boolean;
    nextPlan: ArchiveDeletePlan | null;
  };
  hiddenCleanup: {
    stage: CleanupStageView;
    canArm: boolean;
    stats: HiddenThreadDeleteStatsView;
    rolloutDeleteResult: string;
  };
};

export function opsWorkspaceView(input: {
  status?: SystemStatus | null;
  update?: UpdateStatus | null;
  hiddenPlan?: HiddenThreadDeletePlan | null;
  archivePlan?: ArchiveDeletePlan | null;
  archiveDryRunPending?: boolean;
  archiveDeleteArmed?: boolean;
  archiveExecutePending?: boolean;
  hiddenDryRunPending?: boolean;
  hiddenDeleteArmed?: boolean;
  hiddenExecutePending?: boolean;
  capabilities?: RuntimeCapabilityInput;
}): OpsWorkspaceView {
  const hiddenStats = hiddenThreadDeleteStats(input.hiddenPlan ?? null, input.status);
  return {
    hostname: cleanHostValue(input.status?.hostname) ?? "读取中",
    publicEndpoint: cleanHostValue(input.status?.public_endpoint),
    systemMetrics: opsSystemMetrics(input.status, input.capabilities),
    hiddenStats,
    archivedCleanupStage: cleanupStageLabel({
      hasPlan: Boolean(input.archivePlan),
      dryRunPending: Boolean(input.archiveDryRunPending),
      armed: Boolean(input.archiveDeleteArmed),
      executePending: Boolean(input.archiveExecutePending),
      executableCount: input.archivePlan?.archived_threads ?? 0
    }),
    hiddenCleanupStage: cleanupStageLabel({
      hasPlan: Boolean(input.hiddenPlan),
      dryRunPending: Boolean(input.hiddenDryRunPending),
      armed: Boolean(input.hiddenDeleteArmed),
      executePending: Boolean(input.hiddenExecutePending),
      executableCount: hiddenStats.hidden
    }),
    updateActions: opsUpdateActionView(input.update, input.capabilities)
  };
}

export function opsWorkspaceViewModel(input: {
  capabilities?: RuntimeCapabilityInput;
  status?: Partial<SystemStatus> | null;
  updateStatus?: UpdateStatus | null;
  archivePlan?: ArchiveDeletePlan | null;
  archiveExecuteResult?: Pick<ArchiveDeleteResult, "after_total_threads" | "after_active_threads" | "after_archived_threads" | "after_integrity"> | null;
  hiddenPlan?: HiddenThreadDeletePlan | null;
  hiddenDeleteResult?: Pick<HiddenThreadDeleteResult, "deleted_rollout_files"> | null;
  archiveCleanup: CleanupMutationState;
  hiddenCleanup: CleanupMutationState;
}): OpsWorkspaceViewModel {
  const capabilities = capabilitiesForInput(input.capabilities);
  const hiddenStats = hiddenThreadDeleteStats(input.hiddenPlan ?? null, input.status);
  const archivePlan = input.archiveExecuteResult
    ? archivePlanAfterExecute(input.archivePlan ?? null, input.archiveExecuteResult)
    : input.archivePlan ?? null;
  return {
    panelTitles: opsWorkspacePanelTitles(capabilities),
    systemMetrics: opsSystemMetrics(input.status, capabilities),
    updateActions: opsUpdateActionView(input.updateStatus, capabilities),
    archiveCleanup: {
      stage: cleanupStageLabel({
        ...input.archiveCleanup,
        hasPlan: Boolean(archivePlan),
        executableCount: archivePlan?.archived_threads ?? 0
      }),
      canArm: (archivePlan?.archived_threads ?? 0) > 0 && !input.archiveCleanup.dryRunPending && !input.archiveCleanup.executePending,
      nextPlan: archivePlan
    },
    hiddenCleanup: {
      stage: cleanupStageLabel({
        ...input.hiddenCleanup,
        hasPlan: Boolean(input.hiddenPlan),
        executableCount: hiddenStats.hidden
      }),
      canArm: canStartHiddenThreadDelete(input.hiddenPlan) && !input.hiddenCleanup.dryRunPending && !input.hiddenCleanup.executePending,
      stats: hiddenStats,
      rolloutDeleteResult: hiddenRolloutDeleteResultText(input.hiddenDeleteResult)
    }
  };
}

function opsSystemMetrics(
  status: Partial<SystemStatus> | null | undefined,
  input?: RuntimeCapabilityInput
): RuntimeMetricView[] {
  const capabilities = capabilitiesForInput(input);
  const hiddenThreadCount = status?.hidden_thread_count ?? 0;
  return [
    {
      label: "Hostname",
      value: cleanHostValue(status?.hostname) ?? "读取中"
    },
    ...(capabilities.publicEndpointStatus ? [{
      label: "Public endpoint",
      value: cleanHostValue(status?.public_endpoint) ?? "未配置",
      tone: cleanHostValue(status?.public_endpoint) ? "success" as const : "warning" as const
    }] : []),
    ...(capabilities.codexStatePaths ? [
      {
        label: "state DB",
        value: status?.state_db_integrity ?? "unknown",
        tone: status?.state_db_integrity === "ok" ? "success" as const : "warning" as const
      },
      {
        label: "Codex Home",
        value: codexHomeStatusValue(status),
        wide: true
      },
      {
        label: "State DB",
        value: status?.state_db ?? "unknown",
        wide: true
      }
    ] : []),
    {
      label: "Hidden threads",
      value: String(hiddenThreadCount),
      tone: hiddenThreadCount > 0 ? "warning" as const : undefined
    },
    {
      label: "Sources",
      value: sourceCountsText(status?.thread_source_counts)
    }
  ];
}

export function hiddenThreadDeleteStats(
  plan: HiddenThreadDeletePlan | null,
  status?: Pick<SystemStatus, "hidden_thread_count" | "state_db_integrity"> | null
): HiddenThreadDeleteStatsView {
  const hidden = plan?.hidden_threads ?? status?.hidden_thread_count ?? 0;
  return {
    hidden,
    visible: plan?.visible_threads ?? 0,
    sourceCounts: sourceCountsText(plan?.hidden_source_counts),
    integrity: plan?.integrity ?? status?.state_db_integrity ?? "未知"
  };
}

export function archivePlanAfterExecute(
  current: ArchiveDeletePlan | null,
  result: Pick<ArchiveDeleteResult, "after_total_threads" | "after_active_threads" | "after_archived_threads" | "after_integrity">
): ArchiveDeletePlan | null {
  if (!current) return current;
  return {
    ...current,
    total_threads: result.after_total_threads,
    active_threads: result.after_active_threads,
    archived_threads: result.after_archived_threads,
    archived_ids: [],
    integrity: result.after_integrity
  };
}

function cleanupStageLabel(input: CleanupStageInput): CleanupStageView {
  if (input.executePending) return { label: "执行中", tone: "warning" };
  if (input.armed) return { label: "等待确认", tone: "danger" };
  if (input.dryRunPending) return { label: "扫描中", tone: "warning" };
  if (!input.hasPlan) return { label: "待 dry-run" };
  if (input.executableCount > 0) return { label: "可清理", tone: "warning" };
  return { label: "无可清理", tone: "success" };
}

export function hiddenRolloutDeleteResultText(result?: Pick<HiddenThreadDeleteResult, "deleted_rollout_files"> | null): string {
  if (!result) return "等待执行";
  return String(result.deleted_rollout_files ?? 0);
}

export function sourceCountsText(counts?: Record<string, number> | null): string {
  if (!counts || Object.keys(counts).length === 0) return "暂无";
  return Object.entries(counts)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `${key}:${value}`)
    .join(" ");
}

export type ThreadMessageControllerView = {
  hydration: {
    threadId: string | null;
    selectedThreadSummary?: ThreadSummary | null;
    selectedDetail?: ThreadDetail | null;
  };
  realtime: {
    threadId: string;
    applyThreadTitleOverride: <T extends Partial<ThreadSummary>>(summary: T) => T;
  };
};

export function threadMessageControllerView(input: {
  threadId: string | null;
  selectedThreadSummary?: ThreadSummary | null;
  selectedDetail?: ThreadDetail | null;
  applyThreadTitleOverride?: <T extends Partial<ThreadSummary>>(summary: T) => T;
}): ThreadMessageControllerView {
  return {
    hydration: {
      threadId: input.threadId,
      selectedThreadSummary: input.selectedThreadSummary,
      selectedDetail: input.selectedDetail
    },
    realtime: {
      threadId: input.threadId ?? "",
      applyThreadTitleOverride: input.applyThreadTitleOverride ?? ((summary) => summary)
    }
  };
}

export type ProbeAvailabilityView = {
  headline: string;
  metric: string;
  tone: Tone;
};

export type ProbeThreadsByStatusView = {
  running: ThreadSummary[];
  replyNeeded: ThreadSummary[];
  recoverable: ThreadSummary[];
};

export type ProbeWorkspaceView = {
  available: boolean;
  data?: ProbeStatus;
  currentSettings?: ProbeSettings;
  logsDb?: ProbeLogsDbStatus;
  logsDbStatusText?: string | null;
  logsDbTone: Tone;
  barkConfigured: boolean;
  probeThreads: ProbeThreadsByStatusView;
  probeEnabled: boolean;
  serviceText: string;
  availability: ProbeAvailabilityView;
  statusTone: Tone;
  snapshotText: string;
  snapshotTone: Tone;
  probeJobs: JobRecord[];
};

export function probeWorkspaceView(input: {
  data?: ProbeStatus;
  available?: boolean;
  currentSettings?: ProbeSettings;
  logsDb?: ProbeLogsDbStatus;
  recentEventCount?: number;
  jobs?: JobRecord[];
  loading?: boolean;
  fetching?: boolean;
  error?: boolean;
  draftDeviceKeyConfigured?: boolean;
}): ProbeWorkspaceView {
  const data = input.data;
  const logsDbStatusText = input.logsDb?.logs_db_status ?? input.logsDb?.status ?? data?.logs_db_status;
  const barkConfigured = Boolean(input.currentSettings?.notifications?.device_key_configured || input.draftDeviceKeyConfigured);
  const probeEnabled = data?.enabled ?? input.currentSettings?.probe?.enabled ?? false;
  const availability = probeAvailabilityView({
    available: input.available,
    probeEnabled,
    loading: input.loading,
    fetching: input.fetching,
    hasData: Boolean(data),
    error: input.error
  });
  return {
    available: input.available ?? false,
    data,
    currentSettings: input.currentSettings,
    logsDb: input.logsDb,
    logsDbStatusText,
    logsDbTone: probeLogsDbTone(logsDbStatusText),
    barkConfigured,
    probeThreads: probeThreadsByStatus(data),
    probeEnabled,
    serviceText: data ? `${data.service_kind}:${data.service_name}` : "未知",
    availability,
    statusTone: availability.tone,
    snapshotText: probeSnapshotStatusText(data, input.fetching),
    snapshotTone: data?.is_refreshing || input.fetching ? "warning" : "success",
    probeJobs: (input.jobs ?? []).filter(isProbeJob).slice(0, 6)
  };
}

export function probeStatusThreads(status?: Pick<ProbeStatus, "running_threads" | "reply_needed_threads" | "recoverable_threads"> | null): ThreadSummary[] {
  return [
    ...(status?.running_threads ?? []),
    ...(status?.reply_needed_threads ?? []),
    ...(status?.recoverable_threads ?? [])
  ];
}

export function probeThreadsByStatus(status?: Pick<ProbeStatus, "running_threads" | "reply_needed_threads" | "recoverable_threads"> | null): ProbeThreadsByStatusView {
  return {
    running: status?.running_threads ?? [],
    replyNeeded: status?.reply_needed_threads ?? [],
    recoverable: status?.recoverable_threads ?? []
  };
}

export function probeRunningCountValue(status?: Pick<ProbeStatus, "running_count" | "running_threads"> | null): string {
  const backendCount = typeof status?.running_count === "number" ? Math.max(0, status.running_count) : 0;
  const threadCount = status?.running_threads?.length ?? 0;
  return String(backendCount > 0 ? backendCount : threadCount);
}

export function probeSettingsAfterBarkSave<T extends { notifications: { device_key_configured?: boolean } }>(
  saved: T,
  submittedDeviceKey?: string | null,
): T {
  if (!submittedDeviceKey?.trim()) return saved;
  return {
    ...saved,
    notifications: {
      ...saved.notifications,
      device_key_configured: true
    }
  };
}

export function isProbeJob(job: JobRecord): boolean {
  return job.kind.startsWith("probe_")
    || job.kind.startsWith("probe-")
    || job.title.includes("探针")
    || job.title.includes("Probe");
}

export function probeJobActionLabel(action: ProbeJobAction | undefined): string {
  switch (action) {
    case "bark-test":
      return "Bark 测试";
    case "hooks-install":
      return "Hook 安装";
    case "logs-db-dry-run":
      return "日志库 dry-run";
    case "logs-db-execute":
      return "日志库维护";
    default:
      return "Probe job";
  }
}

export function probeEventSummary(event: ProbeEvent): string {
  const thread = event.thread_id ? `线程 ${event.thread_id}` : "无线程";
  const fields = [
    event.payload?.session_id ? "session" : "",
    event.payload?.transcript_path ? "transcript" : "",
    event.payload?.last_assistant_message ? "assistant" : ""
  ].filter(Boolean);
  return [thread, fields.length ? fields.join(" · ") : "payload 已脱敏"].join(" · ");
}

export function shouldAutoScrollProbeFeed(
  current: { scrollTop: number; clientHeight: number; scrollHeight: number },
  _previous?: { scrollTop: number; clientHeight: number; scrollHeight: number } | null
): boolean {
  return current.scrollHeight - current.scrollTop - current.clientHeight <= 32;
}

type CodexHomePathFields = {
  home?: string | null;
  codex_home?: string | null;
  configured_codex_home?: string | null;
  resolved_codex_home?: string | null;
  codex_home_source?: string | null;
};

export function codexHomeStatusValue(status?: CodexHomePathFields | null): string {
  return pathWithSource(
    firstStringValue(status, ["resolved_codex_home", "codex_home", "home", "configured_codex_home"]),
    firstStringValue(status, ["codex_home_source"])
  );
}

export function logsDbPathStatusValue(logsDb?: ProbeLogsDbStatus | ProbeSettings["logs_db"] | null): string {
  return pathWithSource(
    firstStringValue(logsDb, ["resolved_logs_db_path", "resolved_path", "path", "logs_db_path"]),
    firstStringValue(logsDb, ["logs_db_source", "source"])
  );
}

export function probeDiscoveryWarningsText(warnings?: string[] | null): string {
  return warnings?.length ? warnings.join(", ") : "无";
}

export function pathText(value?: string | null): string {
  return value && value.trim() ? value : "未知";
}

function pathWithSource(value?: string | null, source?: string | null): string {
  const path = pathText(value);
  const cleanedSource = source?.trim();
  return path !== "未知" && cleanedSource ? `${path} · ${cleanedSource}` : path;
}

function firstStringValue(source: unknown, keys: string[]): string | null {
  if (!source || typeof source !== "object") return null;
  const record = source as Record<string, unknown>;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value.trim()) return value.trim();
  }
  return null;
}

export function probeStateLabel(value?: string | null): string {
  if (!value) return "未知";
  const labels: Record<string, string> = {
    managed: "已管理",
    stale: "需修复",
    missing: "未安装",
    disabled: "已停用",
    configured: "已配置",
    not_configured: "未配置",
    maintenance_ready: "可维护",
    ready: "就绪",
    ok: "正常",
    builtin: "内置"
  };
  return labels[value] ?? value;
}

export function probeLogsDbTone(value?: string | null): Tone {
  if (value === "ok" || value === "maintenance_ready") return "success";
  if (value === "disabled") return "warning";
  return value ? "danger" : "warning";
}

export function isProbeSettings(value: unknown): value is ProbeSettings {
  return Boolean(value && typeof value === "object" && "codex" in value && "probe" in value && "notifications" in value && "logs_db" in value);
}

export function logBytesDraftToMb(value: number | ""): number | "" {
  if (value === "") return "";
  return Math.max(1, Math.round(value / (1024 * 1024)));
}

export function mbDraftToLogBytes(value: string): number | "" {
  const parsed = numberInputDraftValue(value);
  return parsed === "" ? "" : parsed * 1024 * 1024;
}

export function probeLogDbNumber(logsDb: ProbeLogsDbStatus | undefined, keys: string[]): string {
  const value = probeLogDbValue(logsDb, keys);
  return typeof value === "number" && Number.isFinite(value) ? String(value) : "未知";
}

export function probeLogDbString(logsDb: ProbeLogsDbStatus | undefined, keys: string[]): string {
  const value = probeLogDbValue(logsDb, keys);
  if (typeof value === "string" && value.trim()) return value;
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  if (typeof value === "boolean") return value ? "true" : "false";
  return "未知";
}

export function probeLogDbSize(logsDb: ProbeLogsDbStatus | undefined, keys: string[]): string {
  const value = probeLogDbValue(logsDb, keys);
  return typeof value === "number" && Number.isFinite(value) ? formatFileSize(value) : "未知";
}

function probeLogDbValue(logsDb: ProbeLogsDbStatus | undefined, keys: string[]): unknown {
  if (!logsDb) return undefined;
  for (const key of keys) {
    const value = logsDb[key];
    if (value !== undefined && value !== null && value !== "") return value;
  }
  return undefined;
}

export function probeSnapshotStatusText(status?: Pick<ProbeStatus, "snapshot_age_seconds" | "is_refreshing" | "snapshot_status"> | null, fetching = false): string {
  const age = typeof status?.snapshot_age_seconds === "number" ? Math.max(0, Math.round(status.snapshot_age_seconds)) : null;
  const prefix = status?.is_refreshing || fetching ? "后台刷新" : "已同步";
  if (age === null) return prefix;
  if (age < 60) return `${prefix} ${age}s`;
  const minutes = Math.floor(age / 60);
  return `${prefix} ${minutes}m`;
}

export function probeAvailabilityView(input: {
  available?: boolean;
  probeEnabled?: boolean;
  loading?: boolean;
  fetching?: boolean;
  hasData?: boolean;
  error?: boolean;
}): ProbeAvailabilityView {
  if (!input.hasData && input.error) {
    return {
      headline: "Probe 快照读取失败",
      metric: "读取失败",
      tone: "danger"
    };
  }
  if (!input.hasData && (input.loading || input.fetching)) {
    return {
      headline: "正在读取 Probe 快照",
      metric: "读取中",
      tone: "warning"
    };
  }
  if (input.available) {
    return input.probeEnabled
      ? { headline: "Probe 正在接管云机观测", metric: "运行中", tone: "success" }
      : { headline: "Probe 已停用", metric: "停用", tone: "warning" };
  }
  return {
    headline: "Probe 端点不可用",
    metric: "不可用",
    tone: "danger"
  };
}

export function cleanHostValue(value?: string | null): string | null {
  const cleaned = value?.trim();
  const legacyAlias = ["tencent", "wanka"].join("-");
  if (!cleaned || cleaned === legacyAlias) return null;
  return cleaned;
}

export function hostnameFromPublicEndpoint(value?: string | null): string | null {
  const endpoint = cleanHostValue(value);
  if (!endpoint) return null;
  try {
    return new URL(endpoint).hostname || null;
  } catch {
    return endpoint.replace(/^\/+/, "").split("/")[0]?.split(":")[0] || null;
  }
}

export function secondsToDays(seconds: number): number {
  return Math.max(1, Math.round(seconds / secondsPerDay));
}

export function normalizeTurnstileAction(value?: string | null): string {
  const action = value?.trim();
  return action || "login";
}

function numberInputDraftValue(value: string): number | "" {
  const trimmed = value.trim();
  if (!trimmed) return "";
  const numeric = Number(trimmed);
  return Number.isFinite(numeric) ? Math.trunc(numeric) : "";
}

function formatFileSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "unknown";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KiB", "MiB", "GiB"];
  let value = bytes / 1024;
  for (const unit of units) {
    if (value < 1024 || unit === units[units.length - 1]) {
      return `${value.toFixed(value >= 10 ? 0 : 1)} ${unit}`;
    }
    value /= 1024;
  }
  return `${bytes} B`;
}

export function failureCategoryLabel(category: string, input?: RuntimeCapabilityInput): string {
  const capabilities = capabilitiesForInput(input);
  const policy = hostCapabilityPolicy(capabilities);
  if (policy.failureLabels[category]) return policy.failureLabels[category];
  const labels: Record<string, string> = {
    release_missing: "Release 缺失",
    download_sha256_mismatch: "下载或校验失败",
    read_only_file_system: "文件系统只读/安装目录不可写",
    codex_auth_failure: "Codex 认证失败",
    sqlite_integrity_failure: "SQLite 完整性失败",
    network_tls_eof: "网络或 TLS 中断",
    codex_local_state_unavailable: "Codex 本地状态不可用",
    app_server_unavailable: "Codex 本地状态不可用",
    unknown: "未知失败"
  };
  return labels[category] ?? category;
}

export function jobFailureAnalysisView(
  analysis: NonNullable<JobRecord["failure_analysis"]>,
  input?: RuntimeCapabilityInput
): { label: string; explanation: string; suggestions: string[] } {
  const capabilities = capabilitiesForInput(input);
  const label = failureCategoryLabel(analysis.category, capabilities);
  const policy = hostCapabilityPolicy(capabilities);
  if (!policy.copyRedactionEnabled) {
    return {
      label,
      explanation: analysis.explanation,
      suggestions: analysis.suggestions
    };
  }
  const sanitize = (value: string) => sanitizeDesktopFailureCopy(value, label);
  return {
    label,
    explanation: sanitize(analysis.explanation),
    suggestions: analysis.suggestions.map(sanitize)
  };
}

function sanitizeDesktopFailureCopy(value: string, fallback: string): string {
  return redactHostCopy(value, fallback);
}

export function jobOutputView(value: string, input?: RuntimeCapabilityInput): string {
  const output = value.trim() || "no output";
  return !hostCapabilityPolicy(capabilitiesForInput(input)).copyRedactionEnabled
    ? output
    : sanitizeDesktopFailureCopy(output, "任务输出不可用");
}

function goalTokenBudgetValue(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) && parsed > 0 ? Math.floor(parsed) : null;
}

function validGoalTokenBudget(value: string): boolean {
  return !value.trim() || goalTokenBudgetValue(value) !== null;
}

export function goalStatusLabel(goal: CodexGoal | undefined, loading: boolean): string {
  if (!goal) return loading ? "读取中" : "未设置";
  if (goal.available === false) return "未接入";
  switch (goal.status) {
    case "active":
      return goal.enabled ? "进行中" : "已设置";
    case "paused":
      return "已暂停";
    case "cleared":
      return "已清除";
    case "blocked":
      return "阻塞";
    case "complete":
    case "completed":
      return "完成";
    case "idle":
      return "未设置";
    case "missing_thread":
      return "缺少线程";
    default:
      return goal.status || "未设置";
  }
}

export function goalStatusTone(goal: CodexGoal | undefined): "success" | "warning" | "danger" | undefined {
  if (!goal) return undefined;
  if (goal.available === false || goal.status === "blocked" || goal.status === "missing_thread") return "danger";
  if (goal.status === "paused" || goal.status === "cleared" || goal.status === "idle") return "warning";
  return "success";
}

export function goalControlState(
  goal: CodexGoal | undefined,
  options: { busy?: boolean; objective?: string; tokenBudget?: string } = {}
): { saveDisabled: boolean; clearDisabled: boolean; pauseDisabled: boolean; resumeDisabled: boolean } {
  const unavailable = goal?.available === false;
  const busy = Boolean(options.busy) || unavailable;
  const objective = options.objective ?? goal?.objective ?? "";
  const hasSavedObjective = Boolean(goal?.objective?.trim());
  const status = goal?.status;
  return {
    saveDisabled: busy || !objective.trim() || !validGoalTokenBudget(options.tokenBudget ?? ""),
    clearDisabled: busy || !hasSavedObjective,
    pauseDisabled: busy || !hasSavedObjective || status === "paused" || status === "cleared" || status === "idle",
    resumeDisabled: busy || !hasSavedObjective || status === "active"
  };
}

export function formatGoalTimestamp(value: number | string | null | undefined): string {
  if (value === null || value === undefined) return "无";
  const numeric = typeof value === "number" ? value : Number(value);
  const millis = Number.isFinite(numeric) ? (numeric > 10_000_000_000 ? numeric : numeric * 1000) : Date.parse(String(value));
  if (!Number.isFinite(millis)) return String(value);
  return new Date(millis).toLocaleString("zh-CN", { hour12: false });
}
