import type {
  ArchiveDeletePlan,
  ArchiveDeleteResult,
  HiddenThreadDeletePlan,
  HiddenThreadDeleteResult,
  JobRecord,
  OptionalResult
} from "../../types";
import { callCommand, currentRuntimeContext } from "./transport";
import { normalizeOptionalResult, USE_DEMO } from "./shared";
import {
  demoArchiveDeletePlan,
  demoArchiveDeleteResult,
  demoHiddenThreadDeletePlan,
  demoHiddenThreadDeleteResult,
  demoJob,
  demoJobs
} from "./demo";

export async function dryRunArchiveDelete(csrfToken?: string | null): Promise<ArchiveDeletePlan> {
  if (USE_DEMO) {
    return demoArchiveDeletePlan();
  }
  return callCommand<ArchiveDeletePlan>("cleanup.archiveDryRun", { csrfToken });
}

export async function startArchiveDelete(request: {
  csrfToken?: string | null;
  expectedCount: number;
}): Promise<ArchiveDeleteResult> {
  if (USE_DEMO) return demoArchiveDeleteResult();
  const confirmation = {
    confirmed: true,
    expectedCount: request.expectedCount
  };
  return callCommand<ArchiveDeleteResult>(
    "cleanup.archiveExecute",
    currentRuntimeContext().kind === "desktop"
      ? { request: confirmation, csrfToken: request.csrfToken }
      : { ...confirmation, csrfToken: request.csrfToken }
  );
}

export async function dryRunHiddenThreadDelete(csrfToken?: string | null): Promise<HiddenThreadDeletePlan> {
  if (USE_DEMO) {
    return demoHiddenThreadDeletePlan();
  }
  return callCommand<HiddenThreadDeletePlan>("cleanup.hiddenDryRun", { csrfToken });
}

export async function startHiddenThreadDelete(request: {
  csrfToken?: string | null;
  expectedCount: number;
}): Promise<HiddenThreadDeleteResult> {
  if (USE_DEMO) {
    return demoHiddenThreadDeleteResult();
  }
  const confirmation = {
    confirmed: true,
    expectedCount: request.expectedCount
  };
  return callCommand<HiddenThreadDeleteResult>(
    "cleanup.hiddenExecute",
    currentRuntimeContext().kind === "desktop"
      ? { request: confirmation, csrfToken: request.csrfToken }
      : { ...confirmation, csrfToken: request.csrfToken }
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
