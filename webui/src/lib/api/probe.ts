import type {
  OptionalResult,
  ProbeEventsResponse,
  ProbeJobAction,
  ProbeLogsDbStatus,
  ProbeSettings,
  ProbeStatus,
  SentinelStatus
} from "../../types";
import { runtimeRpc } from "./transport";
import { jobIdFromRuntimeResult, normalizeOptionalResult, normalizeProbeRuntimePayload, USE_DEMO } from "./shared";
import { demoProbeSettings, demoProbeStatus } from "./demo";

export async function getSentinelStatus(): Promise<OptionalResult<SentinelStatus>> {
  return getProbeStatus();
}

export async function getProbeStatus(): Promise<OptionalResult<ProbeStatus>> {
  if (USE_DEMO) {
    return {
      available: true,
      data: demoProbeStatus()
    };
  }
  return normalizeOptionalResult<ProbeStatus>(await runtimeRpc<ProbeStatus | OptionalResult<ProbeStatus>>("getProbeStatus"));
}

export async function getProbeSettings(): Promise<OptionalResult<ProbeSettings>> {
  if (USE_DEMO) {
    return {
      available: true,
      data: demoProbeSettings()
    };
  }
  const payload = await runtimeRpc<ProbeSettings | OptionalResult<ProbeSettings>>("getProbeSettings");
  const result = normalizeOptionalResult<ProbeSettings>(payload);
  return result.available
    ? { ...result, data: normalizeProbeRuntimePayload(result.data) as ProbeSettings }
    : result;
}

function normalizeProbeSettingsSavePayload(settings: Partial<ProbeSettings>): Partial<ProbeSettings> {
  const nestedDeviceKey = settings.probe?.notifications?.device_key;
  if (typeof nestedDeviceKey !== "string" || !nestedDeviceKey.trim()) {
    return settings;
  }
  return {
    ...settings,
    notifications: {
      ...settings.notifications,
      device_key: nestedDeviceKey.trim()
    }
  };
}

export async function saveProbeSettings(settings: Partial<ProbeSettings>, csrfToken?: string | null): Promise<ProbeSettings> {
  if (USE_DEMO) return { ...demoProbeSettings(), ...settings } as ProbeSettings;
  const normalizedSettings = normalizeProbeSettingsSavePayload(settings);
  const payload = await runtimeRpc<ProbeSettings>("saveProbeSettings", {
    settings: normalizedSettings,
    csrfToken
  });
  return normalizeProbeRuntimePayload(payload) as ProbeSettings;
}

export async function getProbeLogsDbStatus(): Promise<OptionalResult<ProbeLogsDbStatus>> {
  if (USE_DEMO) return {
    available: true,
    data: {
      status: "maintenance_ready",
      logs_db_status: "maintenance_ready",
      target: "codex_logs_2",
      path: "/root/.codex/logs_2.sqlite",
      configured_codex_home: "/root/.codex",
      resolved_codex_home: "/root/.codex",
      codex_home_source: "config",
      logs_db_source: "resolved_codex_home",
      discovery_warnings: [],
      total_rows: 128,
      old_rows: 6,
      retained_rows: 122,
      database_size: 524288,
      db_size_bytes: 524288,
      wal_size: 4096,
      wal_size_bytes: 4096,
      shm_size: 32768,
      shm_size_bytes: 32768,
      size_bytes: 524288,
      last_maintain_at: "2026-06-14T18:15:32Z",
      next_run_at: "2026-06-15T00:15:32Z",
      last_result: "dry-run: would_delete_rows=6",
      recent_result: "dry-run: would_delete_rows=6"
    }
  };
  const payload = await runtimeRpc<ProbeLogsDbStatus | OptionalResult<ProbeLogsDbStatus>>("getProbeLogsDbStatus");
  const result = normalizeOptionalResult<ProbeLogsDbStatus>(payload);
  return result.available
    ? { ...result, data: normalizeProbeRuntimePayload(result.data) as ProbeLogsDbStatus }
    : result;
}

export async function getProbeEvents(limit = 10): Promise<OptionalResult<ProbeEventsResponse>> {
  if (USE_DEMO) {
    return {
      available: true,
      data: {
        limit,
        events: [
          {
            id: "probe-event-reply-demo",
            kind: "reply-needed",
            thread_id: "019e95a0-demo",
            title: "Raw reply event",
            message: "Probe 事件已记录",
            dedupe_key: "reply-needed:019e95a0-demo:turn-plan-demo",
            source: "nexushubd probe passive-scan",
            payload: {
              event_type: "reply-needed",
              thread_title: "Plan Mode 修复",
              thread_id: "019e95a0-demo",
              turn_id: "turn-plan-demo",
              beijing_time: "2026-06-16 09:30:00 北京时间",
              reason_label: "等待用户确认",
              body_summary: "Plan Mode 等待用户确认",
              body_sha256: "6b5d9f4f5a5a",
              body_length: 324,
              body_source: "proposed_plan",
              body_truncated: false,
              source: "nexushubd probe passive-scan",
              bark: { title: "等待回复：Plan Mode 修复", sent: false, skipped: true, reason: "dedupe", http_status: 200, dedupe_hit: true, chunk_count: 1, request_count: 0 },
              dedupe: { claimed: true, duplicate: false, status: "claimed" }
            },
            created_at: new Date().toISOString(),
            handled_at: null
          },
          {
            id: "probe-event-completion-demo",
            kind: "completion",
            thread_id: "019e5281-demo",
            title: "Completion",
            message: "Thread completed",
            dedupe_key: "completion:019e5281-demo:turn-done",
            source: "nexushubd probe hook-stop",
            payload: {
              event_type: "completion",
              thread_title: "检查仓库状态",
              thread_id: "019e5281-demo",
              turn_id: "turn-done",
              reason_label: "任务完成",
              body_summary: "仓库状态干净",
              body_sha256: "a13f98c0",
              body_length: 128,
              body_source: "task_complete.last_agent_message",
              body_truncated: false,
              source: "nexushubd probe hook-stop",
              bark: { title: "线程正常完成：检查仓库状态", sent: true, skipped: false, http_status: 200, dedupe_hit: false, chunk_count: 1, request_count: 1 },
              dedupe: { claimed: true, duplicate: false, status: "claimed" }
            },
            created_at: new Date(Date.now() - 300000).toISOString(),
            handled_at: null
          },
          {
            id: "probe-event-hook-demo",
            kind: "hook-stop",
            thread_id: "019e95a0-demo",
            title: "Codex Stop Hook",
            message: "Stop Hook event recorded by NexusHub Probe",
            dedupe_key: "hook-stop:019e95a0-demo:turn-demo",
            source: "nexushubd probe hook-stop",
            payload: {
              event_type: "hook-stop",
              thread_title: "Plan Mode 修复",
              thread_id: "019e95a0-demo",
              turn_id: "turn-demo",
              reason_label: "Stop Hook",
              body_summary: "Stop Hook event recorded by NexusHub Probe",
              body_sha256: "d9a8",
              body_length: 212,
              body_source: "default",
              body_truncated: false,
              source: "nexushubd probe hook-stop",
              bark: { title: "探针事件：Plan Mode 修复", skipped: true, reason: "notifications-disabled", dedupe_hit: false, chunk_count: 0, request_count: 0 },
              dedupe: { claimed: false, duplicate: true, status: "duplicate" }
            },
            created_at: new Date(Date.now() - 600000).toISOString(),
            handled_at: null
          }
        ]
      }
    };
  }
  return normalizeOptionalResult<ProbeEventsResponse>(await runtimeRpc<ProbeEventsResponse | OptionalResult<ProbeEventsResponse>>("getProbeEvents", { limit }));
}

export async function startProbeBarkTest(csrfToken?: string | null): Promise<{ job_id: string }> {
  return startProbeCommand("probe.barkTest", "probe-bark-test", csrfToken);
}

export async function startProbeHooksInstall(csrfToken?: string | null): Promise<{ job_id: string }> {
  return startProbeCommand("probe.installHooks", "probe-hooks-install", csrfToken);
}

export async function startProbeLogsDbDryRun(csrfToken?: string | null): Promise<{ job_id: string }> {
  return startProbeCommand("probe.logsDbDryRun", "probe-logs-db-dry-run", csrfToken);
}

export async function startProbeLogsDbExecute(csrfToken?: string | null): Promise<{ job_id: string }> {
  return startProbeCommand("probe.logsDbExecute", "probe-logs-db-execute", csrfToken);
}

async function startProbeCommand(
  command: "probe.barkTest" | "probe.installHooks" | "probe.logsDbDryRun" | "probe.logsDbExecute",
  fallback: string,
  csrfToken?: string | null,
): Promise<{ job_id: string }> {
  if (USE_DEMO) return { job_id: `${fallback}-demo` };
  const result = await runtimeRpc<{ job_id?: string | null; jobId?: string | null }>(command, { csrfToken });
  return jobIdFromRuntimeResult(result, fallback);
}
