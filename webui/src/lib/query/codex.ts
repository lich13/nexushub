import { useQuery } from "@tanstack/react-query";
import { getCodexConfig, listModels, listPermissionProfiles } from "../api";
import { preservePreviousQueryData } from "./shared";

export const codexQueryKeys = {
  models: ["codex-models"] as const,
  permissionProfiles: ["codex-permission-profiles"] as const,
  config: ["codex-config"] as const
};

export function useCodexModelQuery() {
  return useQuery({
    queryKey: codexQueryKeys.models,
    queryFn: listModels,
    staleTime: 60000,
    placeholderData: preservePreviousQueryData
  });
}

export function useCodexPermissionProfilesQuery() {
  return useQuery({
    queryKey: codexQueryKeys.permissionProfiles,
    queryFn: listPermissionProfiles,
    staleTime: 60000,
    placeholderData: preservePreviousQueryData
  });
}

export function useCodexConfigQuery() {
  return useQuery({
    queryKey: codexQueryKeys.config,
    queryFn: getCodexConfig,
    staleTime: 60000,
    placeholderData: preservePreviousQueryData
  });
}
