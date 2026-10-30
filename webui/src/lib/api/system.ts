import type { AgentProviderInfo, PlatformOverview, SystemCapabilitiesResponse, SystemVersion } from "../../types";
import { callCommand } from "./transport";
import { currentDemoFixtureKey, USE_DEMO } from "./shared";

export async function getSystemCapabilities(): Promise<SystemCapabilitiesResponse> {
  if (USE_DEMO) {
    return (await import("./demo")).demoSystemCapabilities(currentDemoFixtureKey());
  }
  return callCommand<SystemCapabilitiesResponse>("system.capabilities");
}

export async function getSystemVersion(): Promise<SystemVersion> {
  if (USE_DEMO) {
    return (await import("./demo")).demoSystemVersion();
  }
  return callCommand<SystemVersion>("system.version");
}

export async function listProviders(): Promise<AgentProviderInfo[]> {
  if (USE_DEMO) {
    return (await import("./demo")).demoProviders();
  }
  return callCommand<AgentProviderInfo[]>("system.providers");
}

export async function getPlatformOverview(): Promise<PlatformOverview> {
  if (USE_DEMO) {
    return (await import("./demo")).demoPlatformOverview(currentDemoFixtureKey());
  }
  return callCommand<PlatformOverview>("system.platform");
}
