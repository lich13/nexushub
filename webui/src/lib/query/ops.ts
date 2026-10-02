import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  dryRunArchiveDelete,
  dryRunHiddenThreadDelete,
  getUpdateStatus,
  listJobs,
  startArchiveDelete,
  startHiddenThreadDelete,
  updates,
  type RuntimeCapabilityMatrix,
  type UnifiedUpdateAction
} from "../api";
import type { ArchiveDeletePlan, ArchiveDeleteResult, HiddenThreadDeletePlan, HiddenThreadDeleteResult } from "../../types";
import { backgroundJobKey } from "./jobs";
import { machineScope } from "../runtime";

export const opsQueryKeys = {
  updateStatus: ["update-status"] as const,
  jobs: ["jobs"] as const,
  threads: ["threads"] as const
};

export function useOpsQueries({ historyOpen }: { historyOpen: boolean }) {
  return {
    update: useQuery({
      queryKey: [...opsQueryKeys.updateStatus, machineScope()] as const,
      queryFn: getUpdateStatus,
      refetchInterval: 30000,
      staleTime: 15000
    }),
    jobs: useQuery({
      queryKey: [...opsQueryKeys.jobs, machineScope()] as const,
      queryFn: listJobs,
      enabled: historyOpen,
      refetchInterval: 5000
    })
  };
}

export function useOpsActions(input: {
  capabilities: RuntimeCapabilityMatrix;
  onArchiveDryRun: (plan: ArchiveDeletePlan) => void;
  onArchiveExecute: (result: ArchiveDeleteResult) => void;
  onHiddenDryRun: (plan: HiddenThreadDeletePlan) => void;
  onHiddenExecute: (result: HiddenThreadDeleteResult) => void;
}) {
  const qc = useQueryClient();
  const { capabilities } = input;
  const invalidateJobs = () => qc.invalidateQueries({ queryKey: opsQueryKeys.jobs });
  const invalidateThreads = () => qc.invalidateQueries({ queryKey: opsQueryKeys.threads });
  const requireThreadCleanup = () => {
    if (!capabilities.threadCleanup) {
      throw new Error("当前运行时不支持线程清理动作");
    }
  };

  return {
    updateJob: useMutation({
      mutationKey: backgroundJobKey,
      gcTime: Infinity,
      mutationFn: ({ action }: { action: UnifiedUpdateAction }) => {
        if (action === "check") return updates.check();
        if (action === "install") return updates.install();
        return updates.prune(capabilities);
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
        return dryRunArchiveDelete();
      },
      onSuccess: input.onArchiveDryRun
    }),
    archiveExecute: useMutation({
      mutationFn: ({ expectedCount }: { expectedCount: number }) => {
        requireThreadCleanup();
        return startArchiveDelete({ expectedCount });
      },
      onSuccess: (result) => {
        input.onArchiveExecute(result);
        invalidateJobs();
        invalidateThreads();
      }
    }),
    hiddenDryRun: useMutation({
      mutationFn: () => {
        requireThreadCleanup();
        return dryRunHiddenThreadDelete();
      },
      onSuccess: input.onHiddenDryRun
    }),
    hiddenExecute: useMutation({
      mutationFn: ({ expectedCount }: { expectedCount: number }) => {
        requireThreadCleanup();
        return startHiddenThreadDelete({ expectedCount });
      },
      onSuccess: (result) => {
        input.onHiddenExecute(result);
        invalidateJobs();
        invalidateThreads();
      }
    })
  };
}
