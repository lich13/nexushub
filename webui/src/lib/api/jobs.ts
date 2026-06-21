import type {
  ArchiveDeletePlan,
  ArchiveDeleteResult,
  HiddenThreadDeletePlan,
  HiddenThreadDeleteResult,
  JobRecord,
  OptionalResult
} from "../../types";
import { runtimeRpc } from "./transport";
import { normalizeOptionalResult, USE_DEMO } from "./shared";

export async function dryRunArchiveDelete(csrfToken?: string | null): Promise<ArchiveDeletePlan> {
  if (USE_DEMO) {
    return { total_threads: 42, active_threads: 31, archived_threads: 11, session_index_lines: 44, rollout_files: 39, archived_ids: ["019e-demo-a", "019e-demo-b"], integrity: "ok" };
  }
  return runtimeRpc<ArchiveDeletePlan>("dryRunArchiveDelete", { csrfToken });
}

export async function startArchiveDelete(csrfToken?: string | null): Promise<ArchiveDeleteResult> {
  return runtimeRpc<ArchiveDeleteResult>("startArchiveDelete", { confirmed: true, csrfToken });
}

export async function dryRunHiddenThreadDelete(csrfToken?: string | null): Promise<HiddenThreadDeletePlan> {
  if (USE_DEMO) {
    return {
      total_threads: 42,
      visible_threads: 38,
      hidden_threads: 4,
      archived_threads: 0,
      session_index_lines: 42,
      rollout_files: 42,
      hidden_ids: ["019e-hidden-a", "019e-hidden-b", "019e-hidden-c", "019e-hidden-d"],
      hidden_source_counts: { exec: 1, subagent: 3 },
      integrity: "ok"
    };
  }
  return runtimeRpc<HiddenThreadDeletePlan>("dryRunHiddenThreadDelete", { csrfToken });
}

export async function startHiddenThreadDelete(csrfToken?: string | null): Promise<HiddenThreadDeleteResult> {
  if (USE_DEMO) {
    return {
      before: {
        total_threads: 42,
        visible_threads: 38,
        hidden_threads: 4,
        archived_threads: 0,
        session_index_lines: 42,
        rollout_files: 42,
        hidden_ids: ["019e-hidden-a", "019e-hidden-b", "019e-hidden-c", "019e-hidden-d"],
        hidden_source_counts: { exec: 1, subagent: 3 },
        integrity: "ok"
      },
      deleted_threads: 4,
      after_total_threads: 38,
      after_visible_threads: 38,
      after_hidden_threads: 0,
      after_archived_threads: 0,
      after_integrity: "ok",
      visible_threads: 38,
      hidden_threads: 0,
      integrity: "ok",
      deleted_rollout_files: 4
    };
  }
  return runtimeRpc<HiddenThreadDeleteResult>("startHiddenThreadDelete", { confirmed: true, csrfToken });
}

export async function listJobs(): Promise<JobRecord[]> {
  if (USE_DEMO) {
    return [
      { id: "probe-bark-demo", kind: "probe_bark_test", status: "succeeded", title: "Probe Bark 测试", started_at: 1780731706, finished_at: 1780731710, exit_code: 0, output: "POST https://api.day.app\nHTTP 200\nBark push accepted" },
      { id: "probe-logs-demo", kind: "probe_logs_db_maintain", status: "succeeded", title: "Probe logs-db dry-run", started_at: 1780731666, finished_at: 1780731672, exit_code: 0, output: "dry_run=true\nwould_delete_probe_events=42\ncompact=false" },
      { id: "job-demo", kind: "nexushub_update_check", status: "succeeded", title: "NexusHub update precheck", started_at: 1780731606, output: "version check\nintegrity_check: ok" },
      { id: "job-failed-demo", kind: "panel_update", status: "failed", title: "Panel update", started_at: 1780731206, finished_at: 1780731252, exit_code: 1, output: "download release asset\nverify checksum", error: "release asset checksum mismatch", analysis: "Downloaded asset digest did not match release metadata.", explanation: "Retry after confirming the release asset has finished publishing." }
    ];
  }
  const payload = await runtimeRpc<JobRecord[] | OptionalResult<JobRecord[]>>("listJobs", { limit: 30 });
  const result = normalizeOptionalResult<JobRecord[]>(payload);
  return result.available && Array.isArray(result.data) ? result.data : [];
}

export async function getJob(id: string): Promise<JobRecord> {
  if (USE_DEMO) {
    return (await listJobs()).find((job) => job.id === id) ?? {
      id,
      kind: "unknown",
      status: "failed",
      title: id,
      started_at: Date.now() / 1000,
      output: "",
      error: "demo job not found"
    };
  }
  return runtimeRpc<JobRecord>("getJob", { id });
}
