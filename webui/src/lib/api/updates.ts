import type {
  UpdateStatus
} from "../../types";
import {
  callCommand,
  RuntimeUnavailableError,
} from "./transport";
import type { RuntimeCapabilityMatrix } from "../domain/capabilities";
import { runtimeCapabilities } from "../domain/capabilities";
import { demoUpdateJobId, demoUpdateStatus } from "./demo";
import { currentDemoFixtureKey, jobIdFromRuntimeResult, USE_DEMO } from "./shared";

export async function getUpdateStatus(): Promise<UpdateStatus> {
  if (USE_DEMO) {
    return demoUpdateStatus(currentDemoFixtureKey());
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
  ): Promise<UpdateActionResult> {
  const result = await callCommand<{ job_id?: string | null; jobId?: string | null; status?: UpdateStatus }>(command, {});
  return {
    ...jobIdFromRuntimeResult(result, fallback),
    ...(result.status ? { status: result.status } : {})
  };
}

export const updates = {
  async check(): Promise<UpdateActionResult> {
    if (USE_DEMO) return demoUpdateJobId("check");
    return runTypedUpdateCommand("updates.check", "update-check");
  },
  async install(): Promise<UpdateActionResult> {
    if (USE_DEMO) return demoUpdateJobId("install");
    return runTypedUpdateCommand("updates.install", "update-install");
  },
  async prune(
    capabilities: RuntimeCapabilityMatrix = runtimeCapabilities(),
  ): Promise<UpdateActionResult> {
    if (USE_DEMO) return demoUpdateJobId("prune");
    if (!capabilities.updatePrune) {
      throw new RuntimeUnavailableError("当前运行时不支持备份清理动作", "Desktop backup prune command is not implemented");
    }
    return runTypedUpdateCommand("updates.prune", "update-prune");
  }
};

// App updates always use the local native updater, including while viewing a remote machine.
export async function getLocalAppUpdateStatus(): Promise<UpdateStatus> {
  const { runtimeRpc } = await import("../runtime");
  return runtimeRpc<UpdateStatus>("updates.status", undefined, true);
}
export async function runLocalAppUpdate(action: "check" | "install"): Promise<UpdateActionResult> {
  const { runtimeRpc } = await import("../runtime");
  return runtimeRpc<UpdateActionResult>(`updates.${action}`, undefined, true);
}
