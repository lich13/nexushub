import type { AgentProviderInfo, PlatformOverview, SystemStatus, SystemVersion } from "../../types";
import { callCommand } from "./transport";
import { currentDemoFixtureKey, USE_DEMO } from "./shared";
import { demoProviders, demoPlatformOverview, demoSystemStatus, demoSystemVersion } from "./demo";

export async function getSystemStatus(): Promise<SystemStatus> {
  if (USE_DEMO) {
    return demoSystemStatus(currentDemoFixtureKey());
  }
  return callCommand<SystemStatus>("system.status");
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
