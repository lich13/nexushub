import { useQuery } from "@tanstack/react-query";
import { searchSessions } from "../api/search";
import type { SessionSearchRequest } from "../../types";
import { machineScope } from "../runtime";

export const searchQueryKeys = {
  results: (request: SessionSearchRequest) => ["sessions-search", machineScope(), request.provider, request.scope, request.sessionKey ?? "", request.query, request.cursor ?? ""] as const
};

export function useSessionSearch(request: SessionSearchRequest | null) {
  return useQuery({
    queryKey: request ? searchQueryKeys.results(request) : ["sessions-search", "idle"],
    queryFn: () => searchSessions(request!),
    enabled: Boolean(request?.query.trim()),
    staleTime: 10_000,
    retry: false
  });
}
