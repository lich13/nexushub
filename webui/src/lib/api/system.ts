import type {
  AgentProviderInfo,
  ClaudeOverview,
  CodexConfig,
  CodexModel,
  OptionalResult,
  PermissionProfile,
  PlatformOverview,
  PluginInfo,
  SystemStatus,
  SystemVersion
} from "../../types";
import { callCommand } from "./transport";
import {
  isMissingEndpoint,
  normalizeModels,
  normalizeOptionalResult,
  normalizePermissionProfiles,
  USE_DEMO
} from "./shared";
import {
  demoClaudeCodeOverview,
  demoCodexConfig,
  demoModels,
  demoPermissionProfiles,
  demoPlatformOverview,
  demoPlugins,
  demoProviders,
  demoSystemStatus,
  demoSystemVersion
} from "./demo";

export async function getSystemStatus(): Promise<SystemStatus> {
  if (USE_DEMO) {
    return demoSystemStatus();
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

export async function getClaudeCodeOverview(): Promise<OptionalResult<ClaudeOverview>> {
  if (USE_DEMO) {
    return demoClaudeCodeOverview();
  }
  try {
    return normalizeOptionalResult(await callCommand<ClaudeOverview>("system.claudeCodeOverview"));
  } catch (error) {
    if (isMissingEndpoint(error)) {
      return { available: false, error: error instanceof Error ? error.message : String(error) };
    }
    throw error;
  }
}

export async function getPlatformOverview(): Promise<PlatformOverview> {
  if (USE_DEMO) {
    return demoPlatformOverview();
  }
  return callCommand<PlatformOverview>("system.platform");
}

export async function listPlugins(): Promise<PluginInfo[]> {
  if (USE_DEMO) {
    return demoPlugins();
  }
  return callCommand<PluginInfo[]>("system.plugins");
}

export async function listModels(): Promise<OptionalResult<CodexModel[]>> {
  if (USE_DEMO) {
    return demoModels();
  }
  try {
    const result = normalizeOptionalResult(await callCommand<unknown[]>("system.models"));
    return result.available ? { available: true, data: normalizeModels(result.data ?? []) } : result as OptionalResult<CodexModel[]>;
  } catch (error) {
    if (isMissingEndpoint(error)) {
      return { available: false, error: error instanceof Error ? error.message : String(error) };
    }
    throw error;
  }
}

export async function listPermissionProfiles(): Promise<OptionalResult<PermissionProfile[]>> {
  if (USE_DEMO) {
    return demoPermissionProfiles();
  }
  try {
    const result = normalizeOptionalResult(await callCommand<unknown[]>("system.permissionProfiles"));
    return result.available ? { available: true, data: normalizePermissionProfiles(result.data ?? []) } : result as OptionalResult<PermissionProfile[]>;
  } catch (error) {
    if (isMissingEndpoint(error)) {
      return { available: false, error: error instanceof Error ? error.message : String(error) };
    }
    throw error;
  }
}

export async function getCodexConfig(): Promise<OptionalResult<CodexConfig>> {
  if (USE_DEMO) {
    return {
      available: true,
      data: demoCodexConfig()
    };
  }
  try {
    return normalizeOptionalResult(await callCommand<CodexConfig>("system.codexConfig"));
  } catch (error) {
    if (isMissingEndpoint(error)) {
      return { available: false, error: error instanceof Error ? error.message : String(error) };
    }
    throw error;
  }
}
