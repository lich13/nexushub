import type {
  UpdateStatus
} from "../../types";
import {
  RuntimeUnavailableError,
  runtimeRpc,
  runtimeValue
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
  return runtimeRpc<UpdateStatus>("getUpdateStatus");
}

export type UnifiedUpdateAction = "check" | "install" | "prune";

export type UpdateActionResult = {
  job_id: string;
  status?: UpdateStatus;
};

async function runTypedUpdateCommand(
  command: "updates.check" | "updates.install" | "updates.prune",
  fallback: string,
  csrfToken?: string | null,
): Promise<UpdateActionResult> {
  const result = await runtimeRpc<{ job_id?: string | null; jobId?: string | null; status?: UpdateStatus }>(command, { csrfToken });
  return {
    ...jobIdFromRuntimeResult(result, fallback),
    ...(result.status ? { status: result.status } : {})
  };
}

export const updates = {
  async check(csrfToken?: string | null): Promise<UpdateActionResult> {
    if (USE_DEMO) return { job_id: "update-check-demo" };
    return runTypedUpdateCommand("updates.check", "update-check", csrfToken);
  },
  async install(csrfToken?: string | null): Promise<UpdateActionResult> {
    if (USE_DEMO) return { job_id: "update-install-demo" };
    return runTypedUpdateCommand("updates.install", "update-install", csrfToken);
  },
  async prune(
    csrfToken?: string | null,
    capabilities: RuntimeCapabilityMatrix = runtimeCapabilities(),
  ): Promise<UpdateActionResult> {
    if (USE_DEMO) return { job_id: "update-prune-demo" };
    if (!capabilities.backupPrune) {
      throw new RuntimeUnavailableError("当前运行时不支持备份清理动作", "Desktop backup prune command is not implemented");
    }
    return runTypedUpdateCommand("updates.prune", "update-prune", csrfToken);
  }
};
