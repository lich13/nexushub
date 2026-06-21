import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  dryRunArchiveDelete,
  dryRunHiddenThreadDelete,
  getSystemStatus,
  getUpdateStatus,
  listJobs,
  startArchiveDelete,
  startHiddenThreadDelete,
  updates,
  type RuntimeCapabilityMatrix,
  type UnifiedUpdateAction
} from "../api";
import type { ArchiveDeletePlan, ArchiveDeleteResult, HiddenThreadDeletePlan, HiddenThreadDeleteResult } from "../../types";
import { systemQueryKeys } from "./system";

export const opsQueryKeys = {
  systemStatus: systemQueryKeys.status,
  updateStatus: ["update-status"] as const,
  jobs: ["jobs"] as const,
  threads: ["threads"] as const
};

export function useOpsQueries() {
  return {
    status: useQuery({
      queryKey: opsQueryKeys.systemStatus,
      queryFn: getSystemStatus,
      refetchInterval: 8000,
      staleTime: 5000,
      placeholderData: keepPreviousData
    }),
    update: useQuery({
      queryKey: opsQueryKeys.updateStatus,
      queryFn: getUpdateStatus,
      refetchInterval: 30000,
      staleTime: 15000,
      placeholderData: keepPreviousData
    }),
    jobs: useQuery({
      queryKey: opsQueryKeys.jobs,
      queryFn: listJobs,
      refetchInterval: 5000,
      placeholderData: keepPreviousData
    })
  };
}

export function useOpsActions(input: {
  csrfToken?: string | null;
  capabilities: RuntimeCapabilityMatrix;
  onArchiveDryRun: (plan: ArchiveDeletePlan) => void;
  onArchiveExecute: (result: ArchiveDeleteResult) => void;
  onHiddenDryRun: (plan: HiddenThreadDeletePlan) => void;
  onHiddenExecute: (result: HiddenThreadDeleteResult) => void;
}) {
  const qc = useQueryClient();
  const { csrfToken, capabilities } = input;
  const invalidateJobs = () => qc.invalidateQueries({ queryKey: opsQueryKeys.jobs });
  const invalidateSystem = () => qc.invalidateQueries({ queryKey: opsQueryKeys.systemStatus });
  const invalidateThreads = () => qc.invalidateQueries({ queryKey: opsQueryKeys.threads });
  const requireThreadCleanup = () => {
    if (!capabilities.threadCleanup) {
      throw new Error("当前运行时不支持线程清理动作");
    }
  };

  return {
    qc,
    updateJob: useMutation({
      mutationFn: ({ action }: { action: UnifiedUpdateAction }) => {
        if (action === "check") return updates.check(csrfToken);
        if (action === "install") return updates.install(csrfToken);
        return updates.prune(csrfToken, capabilities);
      },
      onSuccess: (result) => {
        if (result.status) {
          qc.setQueryData(opsQueryKeys.updateStatus, result.status);
        }
        invalidateJobs();
        qc.invalidateQueries({ queryKey: opsQueryKeys.updateStatus });
      }
    }),
    archiveDryRun: useMutation({
      mutationFn: () => {
        requireThreadCleanup();
        return dryRunArchiveDelete(csrfToken);
      },
      onSuccess: input.onArchiveDryRun
    }),
    archiveExecute: useMutation({
      mutationFn: () => {
        requireThreadCleanup();
        return startArchiveDelete(csrfToken);
      },
      onSuccess: (result) => {
        input.onArchiveExecute(result);
        invalidateJobs();
        invalidateSystem();
        invalidateThreads();
      }
    }),
    hiddenDryRun: useMutation({
      mutationFn: () => {
        requireThreadCleanup();
        return dryRunHiddenThreadDelete(csrfToken);
      },
      onSuccess: input.onHiddenDryRun
    }),
    hiddenExecute: useMutation({
      mutationFn: () => {
        requireThreadCleanup();
        return startHiddenThreadDelete(csrfToken);
      },
      onSuccess: (result) => {
        input.onHiddenExecute(result);
        invalidateJobs();
        invalidateSystem();
        invalidateThreads();
      }
    })
  };
}

function keepPreviousData<T>(previous: T | undefined): T | undefined {
  return previous;
}
