import { OPS_PANEL_TITLES, opsUpdateActionView } from "./runtimeViewModel";
import { capabilitiesForInput } from "./runtimeViewModel";
import type { RuntimeCapabilityMatrix } from "./capabilities";
import { contractVisual } from "./contractRegistry";

export const sharedVisualTokens = {
  shellClass: "app-shell",
  navClass: "side-nav",
  panelClass: "panel",
  primaryButtonClass: "primary-button",
  secondaryButtonClass: "secondary-button",
  dangerButtonClass: "danger-button",
  disabledAttribute: "disabled"
} as const;

export const sharedNavigationLabels = contractVisual.navigation;

export const sharedCorePanelTitles = contractVisual.corePanelTitles;

export const sharedActionLabels = contractVisual.actionLabels as {
  dryRun: string;
  archiveCleanup: string;
  archiveConfirm: string;
  hiddenDryRun: string;
  hiddenCleanup: string;
  hiddenConfirm: string;
  send: string;
  followup: string;
  stop: string;
};

export const sharedDisabledStates = contractVisual.disabledStates as {
  updateInstallWithoutAvailableVersion: string;
  cleanupArmBeforeDryRun: string;
  cleanupConfirmBeforeArmed: string;
  cleanupDuringMutation: string;
};

export const linuxWebOnlyVisualSurfaces = contractVisual.linuxWebOnly;

export const macosForbiddenVisualSurfaces = contractVisual.forbidden.desktopEmbeddedTauri;

export const desktopLanForbiddenVisualSurfaces = contractVisual.forbidden.desktopLanWebui;

export const desktopTauriOnlyVisualSurfaces = contractVisual.desktopTauriOnly;

export type VisualContract = {
  sharedNavigation: string[];
  sharedPanels: string[];
  sharedActions: string[];
  disabledStates: string[];
  linuxWebOnly: string[];
  desktopTauriOnly: string[];
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
    linuxWebOnly: capabilities.hostSurface === "linux_server_webui" ? [...linuxWebOnlyVisualSurfaces] : [],
    desktopTauriOnly: capabilities.desktopWebuiControl ? [...desktopTauriOnlyVisualSurfaces] : [],
    forbidden: capabilities.hostSurface === "desktop_embedded_tauri"
      ? [...macosForbiddenVisualSurfaces]
      : capabilities.hostSurface === "desktop_lan_webui"
        ? [...desktopLanForbiddenVisualSurfaces]
        : [],
    cleanupRequiresDryRun: capabilities.threadCleanup,
    updateActions: opsUpdateActionView(null, capabilities).map((action) => action.label)
  };
}
