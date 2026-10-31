import { compareSessionMessageTime } from "../domain/sessionSelection";
import { usePagedHistory } from "./usePagedHistory";
import { grokHistoryAdapter } from "./historyWindows";
import { visiblePollInterval } from "./shared";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { getGrokSession, listGrokSessions, renameGrokSession, previewGrokSessionDelete, deleteGrokSession } from "../api/grok";
import type { GrokDeleteRequest } from "../../types";

export const grokQueryKeys = { list: ["grok", "sessions"] as const };
export function useGrokSessions(q: string) {
  return useQuery({ queryKey: [...grokQueryKeys.list, q], queryFn: async () => (await listGrokSessions({ q, limit: 100 })).slice().sort(compareSessionMessageTime), refetchInterval: visiblePollInterval(5000) });
}
export function useGrokDetail(id?: string) {
  return usePagedHistory({
    ...grokHistoryAdapter,
    queryKey: ["grok", "session", id], enabled: Boolean(id),
    read: before => getGrokSession(id!, { limit: 160, before }),
    interval: page => page?.summary.status === "running" ? 1000 : 2000
  });
}
export function useGrokActions() {
  const client = useQueryClient();
  const refresh = () => { void client.invalidateQueries({ queryKey: ["grok"] }); };
  return {
    rename: useMutation({ mutationFn: ({ id, title }: { id: string; title: string }) => renameGrokSession(id, title), onSuccess: refresh }),
    preview: useMutation({ mutationFn: (id: string) => previewGrokSessionDelete(id) }),
    remove: useMutation({ mutationFn: (request: GrokDeleteRequest) => deleteGrokSession(request), onSuccess: refresh })
  };
}
