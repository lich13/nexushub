import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  getProbeEvents,
  getProbeSettings,
  getProbeStatus,
  listJobs,
  saveProbeSettings,
  runProbeBarkTest,
  runProbeHooksInstall,
} from "../api";
import type { ProbeJobAction, ProbeSettings } from "../../types";
import type { RuntimeCapabilityMatrix } from "../api";
import { preservePreviousQueryData } from "./shared";
import { backgroundJobKey } from "./jobs";

export const probeQueryKeys = {
  status: ["probe-status"] as const,
  settings: ["probe-settings"] as const,
  events: ["probe-events"] as const,
  jobs: ["jobs"] as const
};

export function useProbeQueries({ section, historyOpen }: { section: "events" | "settings"; historyOpen: boolean }) {
  return {
    status: useQuery({
      queryKey: probeQueryKeys.status,
      queryFn: getProbeStatus,
      refetchInterval: 15000,
      staleTime: 10000,
      placeholderData: preservePreviousQueryData
    }),
    settings: useQuery({
      queryKey: probeQueryKeys.settings,
      queryFn: getProbeSettings,
      enabled: section === "settings",
      refetchInterval: 30000,
      staleTime: 15000,
      placeholderData: preservePreviousQueryData
    }),
    events: useQuery({
      queryKey: probeQueryKeys.events,
      queryFn: () => getProbeEvents(10),
      enabled: section === "events",
      refetchInterval: 15000,
      staleTime: 10000,
      placeholderData: preservePreviousQueryData
    }),
    jobs: useQuery({
      queryKey: probeQueryKeys.jobs,
      queryFn: listJobs,
      enabled: section === "settings" && historyOpen,
      refetchInterval: 5000,
      placeholderData: preservePreviousQueryData
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
    qc.invalidateQueries({ queryKey: probeQueryKeys.events });
  };

  const runProbeCommand = (action: ProbeJobAction) => {
    if (action === "bark-test") return runProbeBarkTest(input.csrfToken);
    return runProbeHooksInstall(input.csrfToken);
  };

  return {
    refresh: () => {
      invalidateProbe();
      qc.invalidateQueries({ queryKey: probeQueryKeys.settings });
    },
    job: useMutation({
      mutationKey: backgroundJobKey,
      gcTime: Infinity,
      mutationFn: (action: ProbeJobAction) => {
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
