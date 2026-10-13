import { usePagedHistory } from "./usePagedHistory";
import { getSubagentDetail } from "../api/threads";
import { machineScope } from "./connection";
import { subagentHistoryAdapter } from "./historyWindows";

export function useSubagentDetail(rootThreadId: string, agentId?: string | null) {
  return usePagedHistory({
    ...subagentHistoryAdapter,
    queryKey: ["subagent-detail", machineScope(), rootThreadId, agentId],
    enabled: !!agentId,
    read: async (pageParam, signal) => {
      const response = await getSubagentDetail({ rootThreadId, agentId: agentId!, before: pageParam ?? null, limit: 120 });
      if (signal.aborted) throw new DOMException("请求已取消", "AbortError");
      if (response.rootThreadId !== rootThreadId || response.agent.agentId !== agentId) {
        throw new Error("子智能体记录已变化，请重新打开");
      }
      return response;
    },
    interval: latest => {
      return latest && (latest.agent.status === "running" || latest.agent.status === "creating"
        || latest.detail.subagents?.counts.running || latest.detail.subagents?.counts.creating) ? 2000 : 5000;
    }
  });
}
