import type {
  UpdateStatus
} from "../../types";
import {
  callCommand,
  RuntimeUnavailableError,
} from "./transport";
import type { RuntimeCapabilityMatrix } from "../domain/capabilities";
import { runtimeCapabilities } from "../domain/capabilities";
import { demoUpdateStatus } from "./demo";
import { jobIdFromRuntimeResult, USE_DEMO } from "./shared";

export async function getUpdateStatus(): Promise<UpdateStatus> {
  if (USE_DEMO) {
    return demoUpdateStatus();
  }
  return callCommand<UpdateStatus>("updates.status");
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
  const result = await callCommand<{ job_id?: string | null; jobId?: string | null; status?: UpdateStatus }>(command, { csrfToken });
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
    if (!capabilities.updatePrune) {
      throw new RuntimeUnavailableError("当前运行时不支持备份清理动作", "Desktop backup prune command is not implemented");
    }
    return runTypedUpdateCommand("updates.prune", "update-prune", csrfToken);
  }
};
