import { describe, expect, test } from "vitest";
import appSource from "../../App.tsx?raw";
import authGateSource from "../../components/auth/WebAuthGate.tsx?raw";
import opsWorkspaceSource from "../../components/ops/OpsWorkspace.tsx?raw";
import securityWorkspaceSource from "../../components/security/SecurityWorkspace.tsx?raw";
import {
  opsWorkspacePanelTitles,
  opsWorkspaceVisibleCopy
} from "./runtimeViewModel";
import {
  desktopLanForbiddenVisualSurfaces,
  macosForbiddenVisualSurfaces,
  sharedActionLabels,
  sharedCorePanelTitles,
  sharedDisabledStates,
  sharedNavigationLabels,
  visualContractForRuntime
} from "./visualContract";
import type { RuntimeCapabilityMatrix } from "./capabilities";

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

const macosTauriCapabilities: RuntimeCapabilityMatrix = {
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

const desktopLanWebCapabilities: RuntimeCapabilityMatrix = {
  runtimeKind: "web",
  hostSurface: "desktop_lan_webui",
  webAuth: true,
  logout: true,
  securitySettings: false,
  publicEndpointStatus: false,
  codexStatePaths: false,
  updatePrune: false,
  threadCleanup: true,
  probeLogMaintenance: true,
  threadArchiveActions: true,
  updateServiceLabels: false,
  desktopWebuiControl: false,
  forkAction: true,
  approvalActions: true
};

function extractFunctionSource(name: string): string {
  const source = name === "LoginScreen" ? authGateSource : name === "SecurityWorkspace" ? securityWorkspaceSource : appSource;
  const start = source.indexOf(`function ${name}`);
  expect(start).toBeGreaterThanOrEqual(0);

  const next = source.indexOf("\nfunction ", start + 1);
  return source.slice(start, next === -1 ? source.length : next);
}

describe("runtime capability rendering", () => {
  test("shared visual contract keeps Linux WebUI and macOS Tauri on one layout vocabulary", () => {
    const linuxContract = visualContractForRuntime(linuxWebCapabilities);
    const macContract = visualContractForRuntime(macosTauriCapabilities);
    const lanContract = visualContractForRuntime(desktopLanWebCapabilities);

    expect(linuxContract.sharedNavigation).toEqual([...sharedNavigationLabels]);
    expect(macContract.sharedNavigation).toEqual([...sharedNavigationLabels]);
    expect(lanContract.sharedNavigation).toEqual([...sharedNavigationLabels]);
    expect(linuxContract.sharedPanels).toEqual(expect.arrayContaining([...sharedCorePanelTitles]));
    expect(macContract.sharedPanels).toEqual(expect.arrayContaining([
      "Codex 本地线程",
      "Goal",
      "系统状态",
      "NexusHub 更新",
      "Job History",
      "归档线程清理",
      "隐藏线程清理"
    ]));
    expect(linuxContract.sharedActions).toEqual(expect.arrayContaining([
      sharedActionLabels.send,
      sharedActionLabels.followup,
      sharedActionLabels.stop,
      sharedActionLabels.dryRun,
      sharedActionLabels.archiveConfirm,
      sharedActionLabels.hiddenConfirm
    ]));
    expect(macContract.sharedActions).toEqual(expect.arrayContaining([
      sharedActionLabels.send,
      sharedActionLabels.followup,
      sharedActionLabels.stop,
      sharedActionLabels.dryRun,
      sharedActionLabels.archiveConfirm,
      sharedActionLabels.hiddenConfirm
    ]));
    expect(lanContract.sharedActions).toEqual(expect.arrayContaining([
      sharedActionLabels.send,
      sharedActionLabels.followup,
      sharedActionLabels.stop,
      sharedActionLabels.dryRun,
      sharedActionLabels.archiveConfirm,
      sharedActionLabels.hiddenConfirm
    ]));
    expect(linuxContract.cleanupRequiresDryRun).toBe(true);
    expect(macContract.cleanupRequiresDryRun).toBe(true);
    expect(lanContract.cleanupRequiresDryRun).toBe(true);
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

    expect(macosTauriCapabilities).toMatchObject({ runtimeKind: "desktop", webAuth: false, securitySettings: false });
    expect(visibleCopy).toMatch(/系统状态|NexusHub 更新|Check|Install/);
    expect(visibleCopy).toMatch(/WebUI 服务|启动 WebUI|停止 WebUI|重置 WebUI 密码/);
    expect(visibleCopy).toMatch(/归档线程清理|隐藏线程清理|Job History/);
    expect(contract.updateActions).toEqual(["Check", "Install"]);
    expect(contract.desktopTauriOnly).toEqual(["WebUI 服务", "启动 WebUI", "停止 WebUI", "重置 WebUI 密码"]);
    expect(contract.forbidden).toEqual([...macosForbiddenVisualSurfaces]);
    expect(opsWorkspaceSource).toContain("capabilities.desktopWebuiControl && <DesktopWebUiPanel");
    expect(visibleCopy).not.toMatch(/登录|CSRF|Turnstile|security settings|管理员密码|systemd|Nginx|公网入口|Public endpoint|Linux update|Linux prune|Prune/i);
  });

  test("Linux WebUI renders auth, security, public endpoint, service, and Linux update operations", () => {
    const contract = visualContractForRuntime(linuxWebCapabilities);
    const securityWorkspaceSource = extractFunctionSource("SecurityWorkspace");
    const loginScreenSource = extractFunctionSource("LoginScreen");
    const appShellSource = appSource.slice(
      appSource.indexOf("function App()"),
      appSource.indexOf("class WorkspaceErrorBoundary")
    );
    const visibleCopy = [
      ...opsWorkspacePanelTitles(linuxWebCapabilities),
      ...opsWorkspaceVisibleCopy(linuxWebCapabilities),
      securityWorkspaceSource,
      loginScreenSource
    ].join("\n");

    expect(linuxWebCapabilities).toMatchObject({ runtimeKind: "web", webAuth: true, securitySettings: true });
    expect(contract.linuxWebOnly).toEqual(expect.arrayContaining(["Turnstile", "Public endpoint", "systemd", "Nginx", "Prune", "安全"]));
    expect(contract.desktopTauriOnly).toEqual([]);
    expect(contract.updateActions).toEqual(["Precheck", "Update", "Prune"]);
    expect(appShellSource).toContain('capabilities.securitySettings && view === "security"');
    expect(appShellSource).toContain("<WebAuthGate");
    expect(appShellSource).toContain("webAuth={capabilities.webAuth}");
    expect(authGateSource).toContain("!session && webAuth");
    expect(visibleCopy).toMatch(/登录|Turnstile|登录设置|修改密码|systemd|Nginx|Public endpoint|Precheck|Update|Prune/);
    expect(securityWorkspaceSource).toMatch(/Turnstile|登录设置|修改密码|Secret Key|Session TTL/);
    expect(loginScreenSource).toMatch(/Turnstile|登录|password|turnstileToken/);
  });

  test("desktop LAN WebUI keeps browser login but hides server and Tauri control surfaces", () => {
    const contract = visualContractForRuntime(desktopLanWebCapabilities);
    const visibleCopy = [
      ...opsWorkspacePanelTitles(desktopLanWebCapabilities),
      ...opsWorkspaceVisibleCopy(desktopLanWebCapabilities)
    ].join("\n");

    expect(desktopLanWebCapabilities).toMatchObject({
      runtimeKind: "web",
      hostSurface: "desktop_lan_webui",
      webAuth: true,
      securitySettings: false,
      desktopWebuiControl: false
    });
    expect(contract.linuxWebOnly).toEqual([]);
    expect(contract.desktopTauriOnly).toEqual([]);
    expect(contract.forbidden).toEqual([...desktopLanForbiddenVisualSurfaces]);
    expect(contract.updateActions).toEqual(["Check", "Install"]);
    expect(visibleCopy).toMatch(/系统状态|NexusHub 更新|Check|Install|归档线程清理|隐藏线程清理/);
    expect(visibleCopy).not.toMatch(/Turnstile|登录设置|修改密码|systemd|Nginx|Public endpoint|Precheck|Prune|WebUI 服务/i);
  });
});
