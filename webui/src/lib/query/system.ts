import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { getSystemStatus } from "../api";
import {
  runtimeCapabilities,
  runtimeCapabilitiesForRuntime,
  runtimeCapabilitiesFromSystemStatus,
  type RuntimeCapabilityMatrix
} from "../domain/capabilities";
import type { SystemStatus } from "../../types";
import { preservePreviousQueryData } from "./shared";

export const systemQueryKeys = {
  status: ["system-status"] as const
};

export function bootstrapRuntimeCapabilities(): RuntimeCapabilityMatrix {
  return runtimeCapabilities();
}

export function useBootstrapRuntimeCapabilities(): RuntimeCapabilityMatrix {
  return bootstrapRuntimeCapabilities();
}

export function useRuntimeCapabilities(
  status?: Pick<SystemStatus, "capabilities"> | null,
  fallback: RuntimeCapabilityMatrix = bootstrapRuntimeCapabilities(),
): RuntimeCapabilityMatrix {
  return useMemo(
    () => runtimeCapabilitiesFromSystemStatus(status, fallback),
    [fallback, status]
  );
}

export function useSystemStatusQuery(options: { enabled?: boolean; refetchInterval?: number } = {}) {
  return useQuery({
    queryKey: systemQueryKeys.status,
    queryFn: getSystemStatus,
    enabled: options.enabled,
    refetchInterval: options.refetchInterval,
    staleTime: 5000,
    placeholderData: preservePreviousQueryData
  });
}

export { runtimeCapabilitiesForRuntime };
export type { RuntimeCapabilityMatrix };
