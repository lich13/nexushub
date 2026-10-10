import { useInfiniteQuery } from "@tanstack/react-query";
import { getSubagentDetail } from "../api/threads";
import { machineScope } from "./connection";
import { retainSubagentHistory } from "./historyWindows";

export function useSubagentDetail(rootThreadId: string, agentId?: string | null) {
  return useInfiniteQuery({
    queryKey: ["subagent-detail", machineScope(), rootThreadId, agentId],
    enabled: !!agentId,
    initialPageParam: null as string | null,
    queryFn: async ({ pageParam, signal }) => {
      const response = await getSubagentDetail({ rootThreadId, agentId: agentId!, before: pageParam, limit: 120 });
      if (signal.aborted) throw new DOMException("请求已取消", "AbortError");
      if (response.rootThreadId !== rootThreadId || response.agent.agentId !== agentId) {
        throw new Error("子智能体记录已变化，请重新打开");
      }
      return response;
    },
    getNextPageParam: page => page.detail.has_more_blocks ? page.detail.before_cursor : undefined,
    structuralSharing: retainSubagentHistory,
    refetchInterval: query => {
      if (document.visibilityState !== "visible") return false;
      const latest = query.state.data?.pages[0];
      return latest && (latest.agent.status === "running" || latest.agent.status === "creating"
        || latest.detail.subagents?.counts.running || latest.detail.subagents?.counts.creating) ? 2000 : 5000;
    },
    retry: false
  });
}
