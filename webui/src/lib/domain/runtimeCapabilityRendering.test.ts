import { describe, expect, test } from "vitest";
import appSource from "../../App.tsx?raw";
import authGateSource from "../../components/auth/WebAuthGate.tsx?raw";
import {
  opsWorkspacePanelTitles,
  opsWorkspaceVisibleCopy
} from "./runtimeViewModel";
import type { RuntimeCapabilityMatrix } from "./capabilities";

const linuxWebCapabilities: RuntimeCapabilityMatrix = {
  runtimeKind: "web",
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
  forkAction: true,
  approvalActions: true
};

const macosTauriCapabilities: RuntimeCapabilityMatrix = {
  runtimeKind: "desktop",
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
  forkAction: false,
  approvalActions: false
};

function extractFunctionSource(name: string): string {
  const source = name === "LoginScreen" ? authGateSource : appSource;
  const start = source.indexOf(`function ${name}`);
  expect(start).toBeGreaterThanOrEqual(0);

  const next = source.indexOf("\nfunction ", start + 1);
  return source.slice(start, next === -1 ? source.length : next);
}

describe("runtime capability rendering", () => {
  test("macOS Tauri renders only shared capabilities plus app updater copy", () => {
    const visibleCopy = [
      ...opsWorkspacePanelTitles(macosTauriCapabilities),
      ...opsWorkspaceVisibleCopy(macosTauriCapabilities)
    ].join("\n");

    expect(macosTauriCapabilities).toMatchObject({ runtimeKind: "desktop", webAuth: false, securitySettings: false });
    expect(visibleCopy).toMatch(/系统状态|NexusHub 更新|Check|Install/);
    expect(visibleCopy).toMatch(/归档线程清理|隐藏线程清理|Job History/);
    expect(visibleCopy).not.toMatch(/登录|CSRF|Turnstile|security settings|管理员密码|systemd|Nginx|公网入口|Public endpoint|Linux update|Linux prune|Prune/i);
  });

  test("Linux WebUI renders auth, security, public endpoint, service, and Linux update operations", () => {
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
    expect(appShellSource).toContain('capabilities.securitySettings && view === "security"');
    expect(appShellSource).toContain("<WebAuthGate");
    expect(appShellSource).toContain("webAuth={capabilities.webAuth}");
    expect(authGateSource).toContain("!session && webAuth");
    expect(visibleCopy).toMatch(/登录|Turnstile|登录设置|修改密码|systemd|Nginx|Public endpoint|Precheck|Update|Prune/);
    expect(securityWorkspaceSource).toMatch(/Turnstile|登录设置|修改密码|Secret Key|Session TTL/);
    expect(loginScreenSource).toMatch(/Turnstile|登录|password|turnstileToken/);
  });
});
