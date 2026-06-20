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

export const systemQueryKeys = {
  status: ["system-status"] as const
};

const BOOTSTRAP_RUNTIME_CAPABILITIES = runtimeCapabilities();

export function bootstrapRuntimeCapabilities(): RuntimeCapabilityMatrix {
  return BOOTSTRAP_RUNTIME_CAPABILITIES;
}

export function useBootstrapRuntimeCapabilities(): RuntimeCapabilityMatrix {
  return bootstrapRuntimeCapabilities();
}

export function useRuntimeCapabilities(
  status?: Pick<SystemStatus, "capabilities"> | null,
  fallback: RuntimeCapabilityMatrix = BOOTSTRAP_RUNTIME_CAPABILITIES,
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
    placeholderData: keepPreviousData
  });
}

function keepPreviousData<T>(previous: T | undefined): T | undefined {
  return previous;
}

export { runtimeCapabilitiesForRuntime };
export type { RuntimeCapabilityMatrix };
