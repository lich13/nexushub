import type { HostSurface, SystemCapabilities, SystemStatus } from "../../types";

export type RuntimeContext = {
  kind: "web" | "desktop";
};

export type RuntimeCapabilityMatrix = {
  runtimeKind: "web" | "desktop";
  hostSurface: HostSurface;
  webAuth: boolean;
  logout: boolean;
  securitySettings: boolean;
  publicEndpointStatus: boolean;
  codexStatePaths: boolean;
  updatePrune: boolean;
  threadCleanup: boolean;
  probeLogMaintenance: boolean;
  threadArchiveActions: boolean;
  updateServiceLabels: boolean;
  desktopWebuiControl: boolean;
  forkAction: boolean;
  approvalActions: boolean;
};

export const webBootstrapCapabilities: RuntimeCapabilityMatrix = {
  runtimeKind: "web",
  hostSurface: "linux_server_webui",
  webAuth: true,
  logout: true,
  securitySettings: false,
  publicEndpointStatus: false,
  codexStatePaths: false,
  updatePrune: false,
  threadCleanup: false,
  probeLogMaintenance: false,
  threadArchiveActions: false,
  updateServiceLabels: false,
  desktopWebuiControl: false,
  forkAction: false,
  approvalActions: false
};

export const desktopBootstrapCapabilities: RuntimeCapabilityMatrix = {
  runtimeKind: "desktop",
  hostSurface: "desktop_embedded_tauri",
  webAuth: false,
  logout: false,
  securitySettings: false,
  publicEndpointStatus: false,
  codexStatePaths: false,
  updatePrune: false,
  threadCleanup: false,
  probeLogMaintenance: false,
  threadArchiveActions: false,
  updateServiceLabels: false,
  desktopWebuiControl: false,
  forkAction: false,
  approvalActions: false
};

function runtimeKindForHostSurface(hostSurface: HostSurface): RuntimeCapabilityMatrix["runtimeKind"] {
  return hostSurface === "desktop_embedded_tauri" ? "desktop" : "web";
}

function runtimeCapabilitiesFromCore(
  core: SystemCapabilities,
  hostSurface: HostSurface,
): RuntimeCapabilityMatrix {
  const runtimeKind = runtimeKindForHostSurface(hostSurface);
  if (runtimeKind === "desktop") {
    return {
      ...desktopBootstrapCapabilities,
      hostSurface,
      threadCleanup: core.thread_cleanup === true,
      probeLogMaintenance: core.probe_log_maintenance === true,
      threadArchiveActions: core.thread_archive_actions === true,
      updateServiceLabels: false,
      updatePrune: false,
      desktopWebuiControl: core.desktop_webui_control === true,
      forkAction: false,
      approvalActions: false
    };
  }

  return {
    runtimeKind,
    hostSurface,
    webAuth: core.web_auth,
    logout: core.web_auth,
    securitySettings: core.security_settings || core.turnstile || core.admin_password,
    publicEndpointStatus: core.public_endpoint,
    codexStatePaths: core.systemd,
    updatePrune: core.prune_backups,
    threadCleanup: core.thread_cleanup === true,
    probeLogMaintenance: core.probe_log_maintenance === true,
    threadArchiveActions: core.thread_archive_actions === true,
    updateServiceLabels: core.linux_update_job,
    desktopWebuiControl: core.desktop_webui_control === true,
    forkAction: core.web_auth,
    approvalActions: core.web_auth
  };
}

export function runtimeCapabilities(context: RuntimeContext = { kind: "web" }): RuntimeCapabilityMatrix {
  return runtimeCapabilitiesForRuntime(context.kind);
}

export function runtimeCapabilitiesForRuntime(
  desktop: boolean | RuntimeCapabilityMatrix["runtimeKind"] = false,
): RuntimeCapabilityMatrix {
  return desktop === true || desktop === "desktop"
    ? desktopBootstrapCapabilities
    : webBootstrapCapabilities;
}

export function runtimeCapabilitiesFromSystemStatus(
  status?: Pick<SystemStatus, "capabilities" | "host_surface"> | null,
  fallback: RuntimeCapabilityMatrix = runtimeCapabilities(),
): RuntimeCapabilityMatrix {
  const core = status?.capabilities;
  if (!core) return fallback;
  return runtimeCapabilitiesFromCore(
    core,
    status?.host_surface ?? fallback.hostSurface,
  );
}
