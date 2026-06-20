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

export function desktopRuntimeSessionUser(): SessionUser {
  return desktopSessionUser();
}

export async function getPublicSettings(): Promise<PublicSettings> {
  if (USE_DEMO) {
    return { site_name: "NexusHub", turnstile_enabled: false, turnstile_required: false, turnstile_site_key: "", turnstile_action: "login", admin_configured: true };
  }
  return runtimeDispatch<PublicSettings>({
    command: "getPublicSettings",
    desktopFallback: () => ({ site_name: "NexusHub", turnstile_enabled: false, turnstile_required: false, turnstile_site_key: "", turnstile_action: "login", admin_configured: true })
  });
}

export async function login(username: string, password: string, turnstileToken?: string | null): Promise<SessionUser> {
  if (USE_DEMO) {
    return runtimeValue<SessionUser>({
      web: { id: "dev", username, csrf_token: "dev-csrf" },
      desktop: () => desktopSessionUser()
    });
  }
  return runtimeDispatch<SessionUser>({
    command: "login",
    args: { username, password, turnstile_token: turnstileToken ?? null },
    desktopFallback: () => desktopSessionUser()
  });
}

export async function logout(csrfToken?: string | null): Promise<void> {
  if (USE_DEMO) return;
  await runtimeDispatch<void>({
    command: "logout",
    args: { csrfToken },
    desktopFallback: () => undefined
  });
}

export async function me(): Promise<SessionUser> {
  if (USE_DEMO) {
    return runtimeValue<SessionUser>({
      web: { id: "dev", username: "admin", csrf_token: "dev-csrf" },
      desktop: () => desktopSessionUser()
    });
  }
  return runtimeDispatch<SessionUser>({
    command: "me",
    desktopFallback: () => desktopSessionUser()
  });
}
