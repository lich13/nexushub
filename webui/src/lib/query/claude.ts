import { useQuery } from "@tanstack/react-query";
import { getClaudeCodeOverview, getPlatformOverview, listProviders } from "../api";

export const claudeQueryKeys = {
  providers: ["providers"] as const,
  overview: ["claude-code-overview"] as const,
  platform: ["platform-overview"] as const
};

export function useClaudeQueries() {
  return {
    providers: useQuery({
      queryKey: claudeQueryKeys.providers,
      queryFn: listProviders,
      refetchInterval: 30000,
      placeholderData: keepPreviousData
    }),
    overview: useQuery({
      queryKey: claudeQueryKeys.overview,
      queryFn: getClaudeCodeOverview,
      refetchInterval: 30000,
      placeholderData: keepPreviousData
    }),
    platform: useQuery({
      queryKey: claudeQueryKeys.platform,
      queryFn: getPlatformOverview,
      refetchInterval: 30000,
      placeholderData: keepPreviousData
    })
  };
}

function keepPreviousData<T>(previous: T | undefined): T | undefined {
  return previous;
}
