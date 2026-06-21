import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  getProbeEvents,
  getProbeLogsDbStatus,
  getProbeSettings,
  getProbeStatus,
  listJobs,
  saveProbeSettings,
  startProbeBarkTest,
  startProbeHooksInstall,
  startProbeLogsDbDryRun,
  startProbeLogsDbExecute
} from "../api";
import type { ProbeJobAction, ProbeSettings } from "../../types";
import type { RuntimeCapabilityMatrix } from "../api";

export const probeQueryKeys = {
  status: ["probe-status"] as const,
  settings: ["probe-settings"] as const,
  logsDbStatus: ["probe-logs-db-status"] as const,
  events: ["probe-events"] as const,
  jobs: ["jobs"] as const
};

export function useProbeQueries() {
  return {
    status: useQuery({
      queryKey: probeQueryKeys.status,
      queryFn: getProbeStatus,
      refetchInterval: 15000,
      staleTime: 10000,
      placeholderData: keepPreviousData
    }),
    settings: useQuery({
      queryKey: probeQueryKeys.settings,
      queryFn: getProbeSettings,
      refetchInterval: 30000,
      staleTime: 15000,
      placeholderData: keepPreviousData
    }),
    logsDbStatus: useQuery({
      queryKey: probeQueryKeys.logsDbStatus,
      queryFn: getProbeLogsDbStatus,
      refetchInterval: 30000,
      staleTime: 15000,
      placeholderData: keepPreviousData
    }),
    events: useQuery({
      queryKey: probeQueryKeys.events,
      queryFn: () => getProbeEvents(10),
      refetchInterval: 15000,
      staleTime: 10000,
      placeholderData: keepPreviousData
    }),
    jobs: useQuery({
      queryKey: probeQueryKeys.jobs,
      queryFn: listJobs,
      refetchInterval: 5000,
      placeholderData: keepPreviousData
    })
  };
}

export function useProbeActions(input: {
  csrfToken?: string | null;
  capabilities: RuntimeCapabilityMatrix;
  savePayload: (submittedDeviceKey?: string) => Partial<ProbeSettings>;
  onJobSuccess: (action: ProbeJobAction) => void;
  onJobError: (error: Error, action: ProbeJobAction) => void;
  onSaveSuccess: (settings: ProbeSettings, submittedDeviceKey?: string) => void;
  onSaveError: (error: Error) => void;
}) {
  const qc = useQueryClient();
  const invalidateProbe = () => {
    qc.invalidateQueries({ queryKey: probeQueryKeys.status });
    qc.invalidateQueries({ queryKey: probeQueryKeys.logsDbStatus });
    qc.invalidateQueries({ queryKey: probeQueryKeys.events });
  };

  const runProbeCommand = (action: ProbeJobAction) => {
    if (action === "bark-test") return startProbeBarkTest(input.csrfToken);
    if (action === "hooks-install") return startProbeHooksInstall(input.csrfToken);
    if (action === "logs-db-dry-run") return startProbeLogsDbDryRun(input.csrfToken);
    return startProbeLogsDbExecute(input.csrfToken);
  };

  return {
    qc,
    refresh: () => {
      invalidateProbe();
      qc.invalidateQueries({ queryKey: probeQueryKeys.settings });
    },
    job: useMutation({
      mutationFn: (action: ProbeJobAction) => {
        if ((action === "logs-db-dry-run" || action === "logs-db-execute") && !input.capabilities.probeLogMaintenance) {
          throw new Error("当前运行时不支持探针日志库维护");
        }
        return runProbeCommand(action);
      },
      onSuccess: (_result, action) => {
        input.onJobSuccess(action);
        qc.invalidateQueries({ queryKey: probeQueryKeys.jobs });
        invalidateProbe();
      },
      onError: (error: Error, action) => input.onJobError(error, action)
    }),
    save: useMutation({
      mutationFn: (submittedDeviceKey?: string) => saveProbeSettings(input.savePayload(submittedDeviceKey), input.csrfToken),
      onSuccess: (settings, submittedDeviceKey) => {
        input.onSaveSuccess(settings, submittedDeviceKey);
        qc.invalidateQueries({ queryKey: probeQueryKeys.settings });
        invalidateProbe();
      },
      onError: (error: Error) => input.onSaveError(error)
    })
  };
}

function keepPreviousData<T>(previous: T | undefined): T | undefined {
  return previous;
}
