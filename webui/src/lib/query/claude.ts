import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { deleteClaudeSession, getClaudeSession, listClaudeSessions, previewClaudeSessionDelete, renameClaudeSession } from "../api/claude";
import type { ClaudeDeleteRequest } from "../../types";

export function useClaudeSessions(q: string) {
  return useQuery({ queryKey: ["claude_code", "sessions", q], queryFn: () => listClaudeSessions({ q, limit: 100 }), refetchInterval: 5000 });
}
export function useClaudeDetail(sessionKey?: string) {
  return useInfiniteQuery({
    queryKey: ["claude_code", "session", sessionKey],
    initialPageParam: undefined as string | undefined,
    queryFn: ({ pageParam }) => getClaudeSession(sessionKey!, pageParam),
    getNextPageParam: page => page.hasMore ? page.beforeCursor ?? undefined : undefined,
    enabled: Boolean(sessionKey),
    refetchInterval: query => query.state.data?.pages[0]?.summary.status === "running" ? 1000 : 2000
  });
}
export function useClaudeActions() {
  const client = useQueryClient();
  const refresh = () => { void client.invalidateQueries({ queryKey: ["claude_code"] }); };
  return {
    rename: useMutation({ mutationFn: ({ sessionKey, title }: { sessionKey: string; title: string }) => renameClaudeSession(sessionKey, title), onSuccess: refresh }),
    preview: useMutation({ mutationFn: (sessionKey: string) => previewClaudeSessionDelete(sessionKey) }),
    remove: useMutation({ mutationFn: (request: ClaudeDeleteRequest) => deleteClaudeSession(request), onSuccess: refresh })
  };
}
