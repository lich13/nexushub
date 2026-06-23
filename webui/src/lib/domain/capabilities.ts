import type { SystemCapabilities, SystemStatus } from "../../types";

export type RuntimeCapabilityMatrix = {
  runtimeKind: "web" | "desktop";
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
  forkAction: boolean;
  approvalActions: boolean;
};

const webBootstrapCapabilities: RuntimeCapabilityMatrix = {
  runtimeKind: "web",
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
  forkAction: false,
  approvalActions: false
};

const desktopBootstrapCapabilities: RuntimeCapabilityMatrix = {
  runtimeKind: "desktop",
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
  forkAction: false,
  approvalActions: false
};

type RuntimeCapabilityGlobal = typeof globalThis & {
  __NEXUSHUB_DESKTOP_RUNTIME__?: boolean;
  __TAURI_INTERNALS__?: unknown;
};

function bootstrapRuntimeKind(): RuntimeCapabilityMatrix["runtimeKind"] {
  const target = globalThis as RuntimeCapabilityGlobal;
  return target.__NEXUSHUB_DESKTOP_RUNTIME__ || target.__TAURI_INTERNALS__
    ? "desktop"
    : "web";
}

function runtimeCapabilitiesFromCore(
  core: SystemCapabilities,
  runtimeKind: RuntimeCapabilityMatrix["runtimeKind"],
): RuntimeCapabilityMatrix {
  return {
    runtimeKind,
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
    forkAction: core.web_auth,
    approvalActions: core.web_auth
  };
}

export function runtimeCapabilities(): RuntimeCapabilityMatrix {
  return runtimeCapabilitiesForRuntime(bootstrapRuntimeKind());
}

export function runtimeCapabilitiesForRuntime(
  desktop: boolean | RuntimeCapabilityMatrix["runtimeKind"] = false,
): RuntimeCapabilityMatrix {
  return desktop === true || desktop === "desktop"
    ? desktopBootstrapCapabilities
    : webBootstrapCapabilities;
}

export function runtimeCapabilitiesFromSystemStatus(
  status?: Pick<SystemStatus, "capabilities"> | null,
  fallback: RuntimeCapabilityMatrix = runtimeCapabilities(),
): RuntimeCapabilityMatrix {
  const core = status?.capabilities;
  if (!core) return fallback;
  return runtimeCapabilitiesFromCore(core, fallback.runtimeKind);
}
