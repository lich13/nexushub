import { describe, expect, test } from "vitest";
import appSource from "../../App.tsx?raw";
import opsWorkspaceSource from "../../components/ops/OpsWorkspace.tsx?raw";
import {
  opsWorkspacePanelTitles,
  opsWorkspaceVisibleCopy
} from "./runtimeViewModel";
import {
  macosForbiddenVisualSurfaces,
  sharedActionLabels,
  sharedCorePanelTitles,
  sharedDisabledStates,
  sharedNavigationLabels,
  visualContractForRuntime
} from "./visualContract";
import type { RuntimeCapabilityMatrix } from "./capabilities";

const remoteCapabilities: RuntimeCapabilityMatrix = {
  runtimeKind: "desktop",
  hostSurface: "linux_server_api",

  codexStatePaths: true,
  updatePrune: true,
  threadCleanup: true,
  threadArchiveActions: true,
  updateServiceLabels: true,
};

const macosTauriCapabilities: RuntimeCapabilityMatrix = {
  runtimeKind: "desktop",
  hostSurface: "desktop_embedded_tauri",

  codexStatePaths: false,
  updatePrune: false,
  threadCleanup: true,
  threadArchiveActions: true,
  updateServiceLabels: false,
};

describe("runtime capability rendering", () => {
  test("shared visual contract keeps remote and local machines on one layout vocabulary", () => {
    const linuxContract = visualContractForRuntime(remoteCapabilities);
    const macContract = visualContractForRuntime(macosTauriCapabilities);

    expect(linuxContract.sharedNavigation).toEqual([...sharedNavigationLabels]);
    expect(macContract.sharedNavigation).toEqual([...sharedNavigationLabels]);
    expect(linuxContract.sharedPanels).toEqual(expect.arrayContaining([...sharedCorePanelTitles]));
    expect(macContract.sharedPanels).toEqual([...sharedCorePanelTitles]);
    expect(linuxContract.sharedActions).toEqual(expect.arrayContaining([
      sharedActionLabels.dryRun,
      sharedActionLabels.archiveConfirm,
      sharedActionLabels.hiddenConfirm
    ]));
    expect(macContract.sharedActions).toEqual(expect.arrayContaining([
      sharedActionLabels.dryRun,
      sharedActionLabels.archiveConfirm,
      sharedActionLabels.hiddenConfirm
    ]));

    expect(linuxContract.cleanupRequiresDryRun).toBe(true);
    expect(macContract.cleanupRequiresDryRun).toBe(true);
    expect(linuxContract.disabledStates).toEqual(expect.arrayContaining([
      sharedDisabledStates.updateInstallWithoutAvailableVersion,
      sharedDisabledStates.cleanupArmBeforeDryRun,
      sharedDisabledStates.cleanupConfirmBeforeArmed,
      sharedDisabledStates.cleanupDuringMutation
    ]));
    expect(macContract.disabledStates).toEqual(expect.arrayContaining([
      sharedDisabledStates.updateInstallWithoutAvailableVersion,
      sharedDisabledStates.cleanupArmBeforeDryRun,
      sharedDisabledStates.cleanupConfirmBeforeArmed,
      sharedDisabledStates.cleanupDuringMutation
    ]));
  });

  test("macOS Tauri renders only shared capabilities plus app updater copy", () => {
    const contract = visualContractForRuntime(macosTauriCapabilities);
    const visibleCopy = [
      ...opsWorkspacePanelTitles(macosTauriCapabilities),
      ...opsWorkspaceVisibleCopy(macosTauriCapabilities)
    ].join("\n");

    expect(macosTauriCapabilities).toMatchObject({ runtimeKind: "desktop", hostSurface: "desktop_embedded_tauri" });
    expect(visibleCopy).toMatch(/NexusHub 更新|Check|Install/);
    expect(visibleCopy).not.toContain("系统状态");
    expect(visibleCopy).not.toMatch(/WebUI 服务|启动 WebUI|停止 WebUI|重置 WebUI 密码/);
    expect(visibleCopy).toMatch(/归档线程清理|隐藏线程清理|Job History/);
    expect(contract.updateActions).toEqual(["Check", "Install"]);
    expect(contract.desktopTauriOnly).toEqual([]);
    expect(contract.forbidden).toEqual([...macosForbiddenVisualSurfaces]);
    expect(opsWorkspaceSource).not.toContain("DesktopWebUiPanel");
    expect(visibleCopy).not.toMatch(/登录|CSRF|Turnstile|security settings|管理员密码|systemd|Nginx|公网入口|Public endpoint|Linux update|Linux prune|Prune/i);
  });

  test("remote API capabilities expose server maintenance in the shared desktop UI", () => {
    const contract = visualContractForRuntime(remoteCapabilities);
    const visibleCopy = opsWorkspaceVisibleCopy(remoteCapabilities).join("\n");
    expect(contract.remoteApiOnly).toEqual(expect.arrayContaining(["服务更新", "Prune"]));
    expect(contract.updateActions).toEqual(["Precheck", "Update", "Prune"]);
    expect(visibleCopy).toMatch(/Precheck|Update|Prune/);
    expect(appSource).not.toMatch(/WebAuthGate|SecurityWorkspace/);
    expect(contract.remoteApiOnly.join(" ")).not.toMatch(/Turnstile|Public endpoint|Nginx|安全/);
  });

  test("task controls and desktop LAN service are absent from the visual contract", () => {
    const contract = visualContractForRuntime(macosTauriCapabilities);
    expect(contract.sharedNavigation).toEqual(["Codex", "Grok Build", "Pi", "Probe", "设置"]);
    expect(contract.sharedActions).not.toEqual(expect.arrayContaining(["发送", "停止", "恢复 Goal"]));
    expect(contract.desktopTauriOnly).toEqual([]);
  });
});
