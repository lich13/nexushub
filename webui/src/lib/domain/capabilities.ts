import type { SystemCapabilities, SystemStatus } from "../../types";
import { runtimeValue } from "../api/transport";

export type RuntimeCapabilityMatrix = {
  runtimeKind: "web" | "desktop";
  webAuth: boolean;
  logout: boolean;
  securitySettings: boolean;
  publicEndpointStatus: boolean;
  codexStatePaths: boolean;
  backupPrune: boolean;
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
  backupPrune: false,
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
  backupPrune: false,
  updateServiceLabels: false,
  forkAction: false,
  approvalActions: false
};

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
    backupPrune: core.prune_backups,
    updateServiceLabels: core.linux_update_job,
    forkAction: core.web_auth,
    approvalActions: core.web_auth
  };
}

export function runtimeCapabilities(): RuntimeCapabilityMatrix {
  return runtimeValue({
    web: webBootstrapCapabilities,
    desktop: desktopBootstrapCapabilities
  });
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
