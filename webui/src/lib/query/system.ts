import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { getSystemCapabilities } from "../api";
import {
  runtimeCapabilities,
  runtimeCapabilitiesForRuntime,
  runtimeCapabilitiesFromResponse,
  type RuntimeCapabilityMatrix
} from "../domain/capabilities";
import { currentRuntimeContext } from "../api/transport";
import type { SystemCapabilitiesResponse } from "../../types";
import { preservePreviousQueryData } from "./shared";

export const systemQueryKeys = {
  capabilities: ["system-capabilities"] as const
};

export function bootstrapRuntimeCapabilities(): RuntimeCapabilityMatrix {
  return runtimeCapabilities(currentRuntimeContext());
}

export function useBootstrapRuntimeCapabilities(): RuntimeCapabilityMatrix {
  return bootstrapRuntimeCapabilities();
}

export function useRuntimeCapabilities(
  status?: Partial<SystemCapabilitiesResponse> | null,
  fallback: RuntimeCapabilityMatrix = bootstrapRuntimeCapabilities(),
): RuntimeCapabilityMatrix {
  return useMemo(
    () => runtimeCapabilitiesFromResponse(status, fallback),
    [fallback, status]
  );
}

export function useSystemCapabilitiesQuery(options: { enabled?: boolean; refetchInterval?: number } = {}) {
  return useQuery({
    queryKey: systemQueryKeys.capabilities,
    queryFn: getSystemCapabilities,
    enabled: options.enabled,
    refetchInterval: options.refetchInterval,
    staleTime: 5000,
    placeholderData: preservePreviousQueryData
  });
}

export { runtimeCapabilitiesForRuntime };
export type { RuntimeCapabilityMatrix };
