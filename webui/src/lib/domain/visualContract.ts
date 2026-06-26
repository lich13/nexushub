import { OPS_PANEL_TITLES, opsUpdateActionView } from "./runtimeViewModel";
import { capabilitiesForInput } from "./runtimeViewModel";
import type { RuntimeCapabilityMatrix } from "./capabilities";

export const sharedVisualTokens = {
  shellClass: "app-shell",
  navClass: "side-nav",
  panelClass: "panel",
  primaryButtonClass: "primary-button",
  secondaryButtonClass: "secondary-button",
  dangerButtonClass: "danger-button",
  disabledAttribute: "disabled"
} as const;

export const sharedNavigationLabels = ["Codex", "Claude Code", "Probe", "运维"] as const;

export const sharedCorePanelTitles = [
  "Codex 本地线程",
  "Goal",
  "系统状态",
  "NexusHub 更新",
  "Job History",
  OPS_PANEL_TITLES.archivedCleanup,
  OPS_PANEL_TITLES.hiddenCleanup
] as const;

export const sharedActionLabels = {
  dryRun: "Dry-run",
  archiveCleanup: "清理归档",
  archiveConfirm: "确认清理归档",
  hiddenDryRun: "扫描隐藏线程",
  hiddenCleanup: "清理隐藏线程",
  hiddenConfirm: "确认清理隐藏",
  send: "发送",
  followup: "跟进",
  stop: "停止"
} as const;

export const sharedDisabledStates = {
  updateInstallWithoutAvailableVersion: "Install/Update disabled until update_available is true",
  cleanupArmBeforeDryRun: "cleanup arm disabled until dry-run returns candidates",
  cleanupConfirmBeforeArmed: "cleanup execute disabled until confirmation is armed",
  cleanupDuringMutation: "cleanup controls disabled while dry-run or execute is pending"
} as const;

export const linuxWebOnlyVisualSurfaces = [
  "登录",
  "Turnstile",
  "登录设置",
  "修改密码",
  "Public endpoint",
  "systemd",
  "Nginx",
  "Precheck",
  "Update",
  "Prune",
  "安全"
] as const;

export const macosForbiddenVisualSurfaces = [
  "登录",
  "Turnstile",
  "登录设置",
  "修改密码",
  "Public endpoint",
  "systemd",
  "Nginx",
  "管理员密码",
  "Linux prune",
  "安全"
] as const;

export type VisualContract = {
  sharedNavigation: string[];
  sharedPanels: string[];
  sharedActions: string[];
  disabledStates: string[];
  linuxWebOnly: string[];
  forbidden: string[];
  cleanupRequiresDryRun: boolean;
  updateActions: string[];
};

export function visualContractForRuntime(input?: RuntimeCapabilityMatrix): VisualContract {
  const capabilities = capabilitiesForInput(input);
  return {
    sharedNavigation: [...sharedNavigationLabels],
    sharedPanels: sharedCorePanelTitles.filter((title) => (
      capabilities.threadCleanup || (title !== OPS_PANEL_TITLES.archivedCleanup && title !== OPS_PANEL_TITLES.hiddenCleanup)
    )),
    sharedActions: [
      sharedActionLabels.send,
      sharedActionLabels.followup,
      sharedActionLabels.stop,
      ...(capabilities.threadCleanup ? [
        sharedActionLabels.dryRun,
        sharedActionLabels.archiveCleanup,
        sharedActionLabels.archiveConfirm,
        sharedActionLabels.hiddenDryRun,
        sharedActionLabels.hiddenCleanup,
        sharedActionLabels.hiddenConfirm
      ] : [])
    ],
    disabledStates: [
      sharedDisabledStates.updateInstallWithoutAvailableVersion,
      ...(capabilities.threadCleanup ? [
        sharedDisabledStates.cleanupArmBeforeDryRun,
        sharedDisabledStates.cleanupConfirmBeforeArmed,
        sharedDisabledStates.cleanupDuringMutation
      ] : [])
    ],
    linuxWebOnly: capabilities.runtimeKind === "web" ? [...linuxWebOnlyVisualSurfaces] : [],
    forbidden: capabilities.runtimeKind === "desktop" ? [...macosForbiddenVisualSurfaces] : [],
    cleanupRequiresDryRun: capabilities.threadCleanup,
    updateActions: opsUpdateActionView(null, capabilities).map((action) => action.label)
  };
}
