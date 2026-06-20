import type {
  ArchiveDeletePlan,
  ArchiveDeleteResult,
  AgentProviderInfo,
  BridgeActionResult,
  ClaudeOverview,
  CodexConfig,
  CodexGoal,
  CodexGoalSaveInput,
  CodexModel,
  FollowUpQueueItem,
  FollowUpQueueState,
  HiddenThreadDeletePlan,
  HiddenThreadDeleteResult,
  JobRecord,
  MessageBlock,
  OptionalResult,
  PermissionProfile,
  PlatformOverview,
  PluginInfo,
  ProbeEventsResponse,
  ProbeJobAction,
  ProbeLogsDbStatus,
  ProbeSettings,
  ProbeStatus,
  PublicSettings,
  SecuritySettings,
  SentinelStatus,
  SessionUser,
  SystemStatus,
  SystemVersion,
  ThreadBlockPage,
  ThreadDetail,
  ThreadSummary,
  UpdateStatus,
  UploadOutcome
} from "../../types";
import {
  RuntimeUnavailableError,
  createRuntimeThreadEventSource,
  desktopSessionUser,
  runtimeDispatch,
  runtimeRpc,
  runtimeValue,
  uploadRuntimeFiles
} from "./transport";
import { USE_DEMO } from "./shared";
import { demoSecurity } from "./demo";

export async function getSecurity(): Promise<SecuritySettings> {
  if (USE_DEMO) {
    return demoSecurity();
  }
  return runtimeRpc<SecuritySettings>("getSecurity");
}

export async function saveSecurity(settings: Partial<SecuritySettings> & { turnstile_secret_key?: string }, csrfToken?: string | null) {
  return runtimeRpc<SecuritySettings>("saveSecurity", { settings, csrfToken });
}

export async function changePassword(current_password: string, new_password: string, csrfToken?: string | null) {
  return runtimeRpc("changePassword", { current_password, new_password, csrfToken });
}
