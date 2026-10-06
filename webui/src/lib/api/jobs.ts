import type {
  ArchiveDeletePlan,
  ArchiveDeleteResult,
  HiddenThreadDeletePlan,
  HiddenThreadDeleteResult,
  JobRecord,
  OptionalResult
} from "../../types";
import { callCommand } from "./transport";
import { normalizeOptionalResult, USE_DEMO } from "./shared";
import {
  demoArchiveDeletePlan,
  demoArchiveDeleteResult,
  demoHiddenThreadDeletePlan,
  demoHiddenThreadDeleteResult,
  demoJob,
  demoJobs
} from "./demo";

export async function dryRunArchiveDelete(): Promise<ArchiveDeletePlan> {
  if (USE_DEMO) {
    return demoArchiveDeletePlan();
  }
  return callCommand<ArchiveDeletePlan>("cleanup.archiveDryRun", {});
}

export async function startArchiveDelete(request: {
  expectedCount: number;
}): Promise<ArchiveDeleteResult> {
  if (USE_DEMO) return demoArchiveDeleteResult();
  const confirmation = {
    confirmed: true,
    expectedCount: request.expectedCount
  };
  return callCommand<ArchiveDeleteResult>(
    "cleanup.archiveExecute",
    { request: confirmation }
  );
}

export async function dryRunHiddenThreadDelete(): Promise<HiddenThreadDeletePlan> {
  if (USE_DEMO) {
    return demoHiddenThreadDeletePlan();
  }
  return callCommand<HiddenThreadDeletePlan>("cleanup.hiddenDryRun", {});
}

export async function startHiddenThreadDelete(request: {
  expectedCount: number;
  candidates: import("../../types").HiddenThreadSelection[];
}): Promise<HiddenThreadDeleteResult> {
  if (USE_DEMO) {
    return demoHiddenThreadDeleteResult();
  }
  const confirmation = {
    confirmed: true,
    expectedCount: request.expectedCount,
    candidates: request.candidates
  };
  return callCommand<HiddenThreadDeleteResult>(
    "cleanup.hiddenExecute",
    { request: confirmation }
  );
}

export async function listJobs(): Promise<JobRecord[]> {
  if (USE_DEMO) {
    return demoJobs();
  }
  const payload = await callCommand<JobRecord[] | OptionalResult<JobRecord[]>>("jobs.list", { limit: 30 });
  const result = normalizeOptionalResult<JobRecord[]>(payload);
  return result.available && Array.isArray(result.data) ? result.data : [];
}

export async function getJob(id: string): Promise<JobRecord> {
  if (USE_DEMO) {
    return demoJob(id);
  }
  return callCommand<JobRecord>("jobs.detail", { id });
}
