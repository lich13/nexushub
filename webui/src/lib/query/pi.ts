import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { deletePiSession, getPiSession, listPiSessions, previewPiSessionDelete, renamePiSession } from "../api/pi";
import type { PiDeleteRequest } from "../../types";

export const piQueryKeys = { list: ["pi", "sessions"] as const };
export function usePiSessions(q: string) {
  return useQuery({ queryKey: [...piQueryKeys.list, q], queryFn: () => listPiSessions({ q, limit: 100 }), refetchInterval: 5000 });
}
export function usePiDetail(sessionKey?: string) {
  return useQuery({
    queryKey: ["pi", "session", sessionKey],
    queryFn: () => getPiSession(sessionKey!),
    enabled: Boolean(sessionKey),
    refetchInterval: query => query.state.data?.summary?.status === "running" ? 1000 : 2000
  });
}
export function usePiActions(csrfToken?: string | null) {
  const client = useQueryClient();
  const refresh = () => { void client.invalidateQueries({ queryKey: ["pi"] }); };
  return {
    rename: useMutation({ mutationFn: ({ sessionKey, title }: { sessionKey: string; title: string }) => renamePiSession(sessionKey, title, csrfToken), onSuccess: refresh }),
    preview: useMutation({ mutationFn: (sessionKey: string) => previewPiSessionDelete(sessionKey, csrfToken) }),
    remove: useMutation({ mutationFn: (request: PiDeleteRequest) => deletePiSession(request, csrfToken), onSuccess: refresh })
  };
}
