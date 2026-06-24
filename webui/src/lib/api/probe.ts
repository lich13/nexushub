import type {
  OptionalResult,
  ProbeEventsResponse,
  ProbeJobAction,
  ProbeLogsDbStatus,
  ProbeSettings,
  ProbeStatus
} from "../../types";
import { callCommand } from "./transport";
import { jobIdFromRuntimeResult, normalizeOptionalResult, normalizeProbeRuntimePayload, USE_DEMO } from "./shared";
import {
  demoJobId,
  demoProbeEvents,
  demoProbeLogsDbStatus,
  demoProbeSettings,
  demoProbeStatus,
  demoSavedProbeSettings
} from "./demo";

export async function getProbeStatus(): Promise<OptionalResult<ProbeStatus>> {
  if (USE_DEMO) {
    return {
      available: true,
      data: demoProbeStatus()
    };
  }
  return normalizeOptionalResult<ProbeStatus>(await callCommand<ProbeStatus | OptionalResult<ProbeStatus>>("probe.status"));
}

export async function getProbeSettings(): Promise<OptionalResult<ProbeSettings>> {
  if (USE_DEMO) {
    return {
      available: true,
      data: demoProbeSettings()
    };
  }
  const payload = await callCommand<ProbeSettings | OptionalResult<ProbeSettings>>("probe.settings.get");
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
  if (USE_DEMO) return demoSavedProbeSettings(settings);
  const normalizedSettings = normalizeProbeSettingsSavePayload(settings);
  const payload = await callCommand<ProbeSettings>("probe.settings.save", {
    settings: normalizedSettings,
    csrfToken
  });
  return normalizeProbeRuntimePayload(payload) as ProbeSettings;
}

export async function getProbeLogsDbStatus(): Promise<OptionalResult<ProbeLogsDbStatus>> {
  if (USE_DEMO) return demoProbeLogsDbStatus();
  const payload = await callCommand<ProbeLogsDbStatus | OptionalResult<ProbeLogsDbStatus>>("probe.logsDb.status");
  const result = normalizeOptionalResult<ProbeLogsDbStatus>(payload);
  return result.available
    ? { ...result, data: normalizeProbeRuntimePayload(result.data) as ProbeLogsDbStatus }
    : result;
}

export async function getProbeEvents(limit = 10): Promise<OptionalResult<ProbeEventsResponse>> {
  if (USE_DEMO) {
    return demoProbeEvents(limit);
  }
  return normalizeOptionalResult<ProbeEventsResponse>(await callCommand<ProbeEventsResponse | OptionalResult<ProbeEventsResponse>>("probe.events", { limit }));
}

export async function runProbeBarkTest(csrfToken?: string | null): Promise<{ job_id: string }> {
  return startProbeCommand("probe.barkTest", "probe-bark-test", csrfToken);
}

export async function runProbeHooksInstall(csrfToken?: string | null): Promise<{ job_id: string }> {
  return startProbeCommand("probe.installHooks", "probe-hooks-install", csrfToken);
}

export async function runProbeLogsDbDryRun(csrfToken?: string | null): Promise<{ job_id: string }> {
  return startProbeCommand("probe.logsDbDryRun", "probe-logs-db-dry-run", csrfToken);
}

export async function runProbeLogsDbExecute(csrfToken?: string | null): Promise<{ job_id: string }> {
  return startProbeCommand("probe.logsDbExecute", "probe-logs-db-execute", csrfToken);
}

async function startProbeCommand(
  command: "probe.barkTest" | "probe.installHooks" | "probe.logsDbDryRun" | "probe.logsDbExecute",
  fallback: string,
  csrfToken?: string | null,
): Promise<{ job_id: string }> {
  if (USE_DEMO) return demoJobId(fallback);
  const result = await callCommand<{ job_id?: string | null; jobId?: string | null }>(command, { csrfToken });
  return jobIdFromRuntimeResult(result, fallback);
}
