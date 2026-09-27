import type { AgentProviderInfo, PlatformOverview, SystemCapabilitiesResponse, SystemVersion } from "../../types";
import { callCommand } from "./transport";
import { currentDemoFixtureKey, USE_DEMO } from "./shared";
import { demoProviders, demoPlatformOverview, demoSystemCapabilities, demoSystemVersion } from "./demo";

export async function getSystemCapabilities(): Promise<SystemCapabilitiesResponse> {
  if (USE_DEMO) {
    return demoSystemCapabilities(currentDemoFixtureKey());
  }
  return callCommand<SystemCapabilitiesResponse>("system.capabilities");
}

export async function getSystemVersion(): Promise<SystemVersion> {
  if (USE_DEMO) {
    return demoSystemVersion();
  }
  return callCommand<SystemVersion>("system.version");
}

export async function listProviders(): Promise<AgentProviderInfo[]> {
  if (USE_DEMO) {
    return demoProviders();
  }
  return callCommand<AgentProviderInfo[]>("system.providers");
}

export async function getPlatformOverview(): Promise<PlatformOverview> {
  if (USE_DEMO) {
    return demoPlatformOverview(currentDemoFixtureKey());
  }
  return callCommand<PlatformOverview>("system.platform");
}
