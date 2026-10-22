import { useMutation, useQuery, useQueryClient, type InfiniteData } from "@tanstack/react-query";
import { deleteClaudeSession, getClaudeOwnershipDiagnostics, getClaudeSession, listClaudeSessions, previewClaudeSessionDelete, renameClaudeSession } from "../api/claude";
import type { ClaudeDeleteRequest, ClaudeSessionDetail, ClaudeSessionSummary } from "../../types";
import { claudeHistoryAdapter } from "./historyWindows";
import { usePagedHistory } from "./usePagedHistory";

export function useClaudeSessions(q: string) {
  return useQuery({ queryKey: ["claude_code", "sessions", q], queryFn: () => listClaudeSessions({ q, limit: 100 }), refetchInterval: 5000 });
}
export function useClaudeDetail(sessionKey?: string) {
  return usePagedHistory({
    ...claudeHistoryAdapter,
    queryKey: ["claude_code", "session", sessionKey], enabled: Boolean(sessionKey),
    read: before => getClaudeSession(sessionKey!, before),
    interval: page => page?.summary.status === "running" ? 1000 : 2000
  });
}
export function useClaudeActions() {
  const client = useQueryClient();
  const refresh = () => { void client.invalidateQueries({ queryKey: ["claude_code"] }); };
  return {
    rename: useMutation({ mutationFn: ({ sessionKey, title }: { sessionKey: string; title: string }) => renameClaudeSession(sessionKey, title), onSuccess: summary => {
      client.setQueryData<InfiniteData<ClaudeSessionDetail>>(["claude_code", "session", summary.sessionKey], previous => previous && ({ ...previous, pages: previous.pages.map(page => ({ ...page, summary })) }));
      client.setQueriesData<ClaudeSessionSummary[]>({ queryKey: ["claude_code", "sessions"] }, previous => previous?.map(item => item.sessionKey === summary.sessionKey ? summary : item));
      refresh();
    } }),
    diagnostics: useMutation({ mutationFn: getClaudeOwnershipDiagnostics }),
    preview: useMutation({ mutationFn: (sessionKey: string) => previewClaudeSessionDelete(sessionKey) }),
    remove: useMutation({ mutationFn: (request: ClaudeDeleteRequest) => deleteClaudeSession(request), onSuccess: refresh })
  };
}
