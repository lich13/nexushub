import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";
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
import { beforeMachineChange, connectionSnapshot, machineScope } from "../runtime";
import { HiddenScanRunner } from "./hiddenScan";

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
  const scan = useRef<HiddenScanRunner | null>(null);
  const generation = useRef(0);
  useEffect(() => {
    const abandon = async () => { generation.current += 1; await scan.current?.stop(); };
    const unsubscribe = beforeMachineChange(abandon);
    return () => { unsubscribe(); void abandon(); };
  }, []);
  const invalidateJobs = () => qc.invalidateQueries({ queryKey: opsQueryKeys.jobs });
  const invalidateThreads = () => qc.invalidateQueries({ queryKey: opsQueryKeys.threads });
  const requireThreadCleanup = () => {
    if (!capabilities.threadCleanup) {
      throw new Error("当前运行时不支持线程清理动作");
    }
  };

  return {
    stopHiddenScan: () => scan.current?.stop(),
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
      mutationFn: ({ candidates }: { candidates: import("../../types").HiddenThreadSelection[] }) => {
        requireThreadCleanup();
        return startArchiveDelete({ candidates });
      },
      onSuccess: (result) => {
        input.onArchiveExecute(result);
        invalidateJobs();
        invalidateThreads();
      }
    }),
    hiddenDryRun: useMutation({
      mutationFn: async () => {
        requireThreadCleanup();
        const current = ++generation.current;
        const revision = connectionSnapshot().revision;
        const valid = () => generation.current === current && connectionSnapshot().revision === revision;
        await scan.current?.stop();
        if (!valid()) throw new Error("连接已变化，已停止旧扫描");
        const runner = new HiddenScanRunner(dryRunHiddenThreadDelete, plan => {
          if (valid()) input.onHiddenDryRun(plan);
        });
        scan.current = runner;
        try {
          const result = await runner.run(capabilities.hiddenCleanupScan === true);
          if (valid()) input.onHiddenDryRun(result);
          return result;
        } catch (error) {
          void runner.stop();
          throw error;
        }
      }
    }),
    hiddenExecute: useMutation({
      mutationFn: ({ expectedCount, candidates }: { expectedCount: number; candidates: import("../../types").HiddenThreadSelection[] }) => {
        requireThreadCleanup();
        return startHiddenThreadDelete({ expectedCount, candidates });
      },
      onSuccess: (result) => {
        input.onHiddenExecute(result);
        invalidateJobs();
        invalidateThreads();
      }
    })
  };
}
