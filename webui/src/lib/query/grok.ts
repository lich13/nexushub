import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { getGrokSession, listGrokSessions, renameGrokSession, previewGrokSessionDelete, deleteGrokSession } from "../api/grok";
import type { GrokDeleteRequest } from "../../types";

export const grokQueryKeys = { list: ["grok", "sessions"] as const };
export function useGrokSessions(q: string) {
  return useQuery({ queryKey: [...grokQueryKeys.list, q], queryFn: () => listGrokSessions({ q, limit: 100 }), refetchInterval: 5000 });
}
export function useGrokDetail(id?: string) {
  return useQuery({
    queryKey: ["grok", "session", id],
    queryFn: () => getGrokSession(id!),
    enabled: Boolean(id),
    refetchInterval: query => query.state.data?.summary?.status === "running" ? 1000 : 2000
  });
}
export function useGrokActions(csrfToken?: string | null) {
  const client = useQueryClient();
  const refresh = () => { void client.invalidateQueries({ queryKey: ["grok"] }); };
  return {
    rename: useMutation({ mutationFn: ({ id, title }: { id: string; title: string }) => renameGrokSession(id, title, csrfToken), onSuccess: refresh }),
    preview: useMutation({ mutationFn: (id: string) => previewGrokSessionDelete(id, csrfToken) }),
    remove: useMutation({ mutationFn: (request: GrokDeleteRequest) => deleteGrokSession(request, csrfToken), onSuccess: refresh })
  };
}
