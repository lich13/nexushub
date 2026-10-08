import { useInfiniteQuery } from "@tanstack/react-query";
import { getSubagentDetail } from "../api/threads";
import { machineScope } from "./connection";

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
    refetchInterval: () => document.visibilityState === "visible" ? 5000 : false,
    retry: false
  });
}
