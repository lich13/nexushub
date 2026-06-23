import type { JobRecord } from "../../types";
import { runtimeCapabilitiesForRuntime, type RuntimeCapabilityMatrix } from "./capabilities";

export type RuntimeCapabilityInput = RuntimeCapabilityMatrix | undefined;

const DEFAULT_RUNTIME_CAPABILITIES = runtimeCapabilitiesForRuntime("web");

export function capabilitiesForInput(input?: RuntimeCapabilityInput): RuntimeCapabilityMatrix {
  return input ?? DEFAULT_RUNTIME_CAPABILITIES;
}

export const OPS_PANEL_TITLES = {
  system: "系统状态",
  updates: "NexusHub 更新",
  archivedCleanup: "归档线程清理",
  hiddenCleanup: "隐藏线程清理",
  jobs: "Job History"
} as const;

export function opsWorkspacePanelTitles(input?: RuntimeCapabilityInput): string[] {
  const capabilities = capabilitiesForInput(input);
  return [
    OPS_PANEL_TITLES.system,
    OPS_PANEL_TITLES.updates,
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
    capabilities.updateServiceLabels ? "Precheck" : "Check",
    capabilities.updateServiceLabels ? "Update" : "Install",
    ...(capabilities.updatePrune ? ["Prune"] : []),
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

const linuxFailureLabels: Record<string, string> = {
  systemd_failure: "systemd 失败",
  nginx_failure: "Nginx 失败",
  permission_denied_sudo: "权限或 sudo 失败"
};

const genericFailureLabels: Record<string, string> = {
  systemd_failure: "服务失败",
  nginx_failure: "更新失败",
  permission_denied_sudo: "权限失败"
};

export function failureCategoryLabel(category: string, input?: RuntimeCapabilityInput): string {
  const capabilities = capabilitiesForInput(input);
  if (!capabilities.updateServiceLabels && genericFailureLabels[category]) {
    return genericFailureLabels[category];
  }
  const labels: Record<string, string> = {
    release_missing: "Release 缺失",
    download_sha256_mismatch: "下载或校验失败",
    ...linuxFailureLabels,
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
  if (capabilities.updateServiceLabels) {
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
  const sanitized = value
    .replace(/\bsystemd\b/gi, "服务")
    .replace(/\bnginx\b/gi, "服务")
    .replace(/管理员密码/g, "权限")
    .replace(/Linux prune/gi, "清理")
    .replace(/Linux update/gi, "更新")
    .replace(/\bLinux\b/g, "当前宿主")
    .replace(/\bsudo\b/gi, "权限")
    .trim();
  return sanitized || fallback;
}

export function jobOutputView(value: string, input?: RuntimeCapabilityInput): string {
  const output = value.trim() || "no output";
  return capabilitiesForInput(input).updateServiceLabels
    ? output
    : sanitizeDesktopFailureCopy(output, "任务输出不可用");
}
