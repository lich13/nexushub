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
import type { RuntimeCapabilityMatrix } from "../domain/capabilities";
import { runtimeCapabilities } from "../domain/capabilities";
import { jobIdFromRuntimeResult, USE_DEMO } from "./shared";

export async function getUpdateStatus(): Promise<UpdateStatus> {
  if (USE_DEMO) {
    return runtimeValue({
      desktop: {
        current_version: "0.1.100",
        latest_version: "v0.1.103",
        update_available: true,
        channel: "stable",
        method: "macos_tauri_updater",
        state: "idle",
        failure_category: null,
        recommended_action: "Confirm install in the Tauri updater after signature verification.",
        capabilities: ["check", "confirm_install", "job_history", "signature_verification", "restart_after_install"]
      },
      web: {
        current_version: "0.1.100",
        latest_version: "v0.1.103",
        update_available: true,
        channel: "stable",
        method: "linux_systemd_job",
        state: "idle",
        failure_category: null,
        recommended_action: "/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest",
        capabilities: ["check", "confirm_install", "job_history", "sha256_verification", "systemd_health_check", "rollback", "prune_backups"]
      }
    });
  }
  return runtimeDispatch<UpdateStatus>({
    command: "getUpdateStatus"
  });
}

export type UnifiedUpdateAction = "check" | "install" | "prune";

export type UpdateActionResult = {
  job_id: string;
  status?: UpdateStatus;
};

export async function runUpdateAction(
  action: UnifiedUpdateAction,
  csrfToken?: string | null,
  capabilities: RuntimeCapabilityMatrix = runtimeCapabilities(),
): Promise<UpdateActionResult> {
  if (USE_DEMO) return { job_id: `update-${action}-demo` };
  if (action === "prune" && !capabilities.backupPrune) {
    throw new RuntimeUnavailableError("当前运行时不支持备份清理动作", "Desktop backup prune command is not implemented");
  }
  const result = await runtimeDispatch<{ job_id?: string | null; jobId?: string | null; status?: UpdateStatus }>({
    command: "runUpdateAction",
    args: { action, csrfToken },
  });
  return {
    ...jobIdFromRuntimeResult(result, `update-${action}`),
    ...(result.status ? { status: result.status } : {})
  };
}
