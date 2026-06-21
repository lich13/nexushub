import type { CodexModel, OptionalResult, PermissionProfile } from "../../types";
import { RuntimeUnavailableError } from "./transport";

export const USE_DEMO = import.meta.env.DEV && import.meta.env.VITE_USE_REAL_API !== "1";

type RuntimeGlobal = typeof globalThis & {
  __TAURI_INTERNALS__?: unknown;
  __NEXUSHUB_DESKTOP_RUNTIME__?: boolean;
};

function useDesktopDemoFallback(): boolean {
  const target = globalThis as RuntimeGlobal;
  return target.__NEXUSHUB_DESKTOP_RUNTIME__ === true || Boolean(target.__TAURI_INTERNALS__);
}

export function selectRuntimeFallback<T>(options: { web: T; desktop: T }): T {
  return useDesktopDemoFallback() ? options.desktop : options.web;
}

export class ApiError extends Error {
  constructor(message: string, readonly status: number) {
    super(message);
    this.name = "ApiError";
  }
}

export function objectValue(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function snakeCaseKey(key: string): string {
  return key.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
}

function snakeCaseKeys(value: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [key, item] of Object.entries(value)) {
    out[snakeCaseKey(key)] = item;
  }
  return out;
}

export function normalizeProbeRuntimePayload(value: unknown): Record<string, unknown> {
  const raw = objectValue(value);
  const top = snakeCaseKeys(raw);
  const codex = snakeCaseKeys(objectValue(raw.codex));
  const probe = snakeCaseKeys(objectValue(raw.probe));
  const notifications = snakeCaseKeys(objectValue(raw.notifications));
  const logsDb = snakeCaseKeys(objectValue(raw.logs_db ?? raw.logsDb));
  const nestedLogsDb = snakeCaseKeys(objectValue(probe.logs_db ?? probe.logsDb));
  const nestedNotifications = snakeCaseKeys(objectValue(probe.notifications));
  const nestedObservability = snakeCaseKeys(objectValue(probe.observability));
  const hooks = snakeCaseKeys(objectValue(probe.hooks));
  return {
    ...top,
    codex,
    probe: {
      ...probe,
      hooks,
      notifications: nestedNotifications,
      observability: nestedObservability,
      logs_db: nestedLogsDb
    },
    notifications,
    logs_db: logsDb
  };
}

export function isMissingEndpoint(error: unknown): boolean {
  return error instanceof RuntimeUnavailableError || error instanceof ApiError && [404, 405, 501].includes(error.status);
}

export function normalizeOptionalResult<T>(payload: unknown): OptionalResult<T> {
  if (payload && typeof payload === "object" && "available" in payload && ("data" in payload || "error" in payload || "reason" in payload)) {
    const wrapped = payload as { available?: unknown; data?: T; reason?: unknown; error?: unknown };
    if (wrapped.available === false) {
      return {
        available: false,
        reason: typeof wrapped.reason === "string" ? wrapped.reason : null,
        error: typeof wrapped.error === "string" ? wrapped.error : undefined
      };
    }
    return {
      available: true,
      data: wrapped.data as T
    };
  }
  return { available: true, data: payload as T };
}

export function jobIdFromRuntimeResult(result: { job_id?: string | null; jobId?: string | null }, fallback: string): { job_id: string } {
  return { job_id: result.job_id ?? result.jobId ?? fallback };
}

export function normalizeModels(value: unknown): CodexModel[] {
  const list = Array.isArray(value) ? value : typeof value === "object" && value && "models" in value && Array.isArray((value as { models: unknown }).models) ? (value as { models: unknown[] }).models : [];
  return list.flatMap((item) => {
    if (typeof item === "string") return [{ id: item }];
    if (typeof item !== "object" || !item) return [];
    const raw = item as Record<string, unknown>;
    const id = String(raw.id ?? raw.name ?? raw.model ?? "").trim();
    if (!id) return [];
    return [{
      id,
      label: typeof raw.label === "string" ? raw.label : typeof raw.name === "string" ? raw.name : null,
      description: typeof raw.description === "string" ? raw.description : null,
      default: typeof raw.default === "boolean" ? raw.default : null,
      service_tiers: normalizeServiceTiers(raw.service_tiers ?? raw.serviceTiers),
      default_service_tier: typeof raw.default_service_tier === "string"
        ? raw.default_service_tier
        : typeof raw.defaultServiceTier === "string"
          ? raw.defaultServiceTier
          : null
    }];
  });
}

function normalizeServiceTiers(value: unknown): CodexModel["service_tiers"] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    if (typeof item === "string") return [{ id: item }];
    if (typeof item !== "object" || !item) return [];
    const raw = item as Record<string, unknown>;
    const id = String(raw.id ?? raw.name ?? "").trim();
    if (!id) return [];
    return [{
      id,
      name: typeof raw.name === "string" ? raw.name : null,
      description: typeof raw.description === "string" ? raw.description : null
    }];
  });
}

export function normalizePermissionProfiles(value: unknown): PermissionProfile[] {
  const list = Array.isArray(value) ? value : typeof value === "object" && value && "profiles" in value && Array.isArray((value as { profiles: unknown }).profiles) ? (value as { profiles: unknown[] }).profiles : [];
  return list.flatMap((item) => {
    if (typeof item === "string") return [{ id: item }];
    if (typeof item !== "object" || !item) return [];
    const raw = item as Record<string, unknown>;
    const id = String(raw.id ?? raw.name ?? raw.profile ?? "").trim();
    if (!id) return [];
    return [{
      id,
      label: typeof raw.label === "string" ? raw.label : typeof raw.name === "string" ? raw.name : null,
      description: typeof raw.description === "string" ? raw.description : null,
      approval_policy: typeof raw.approval_policy === "string" ? raw.approval_policy : null,
      sandbox_mode: typeof raw.sandbox_mode === "string" ? raw.sandbox_mode : null,
      network_access: typeof raw.network_access === "boolean" ? raw.network_access : null,
      default: typeof raw.default === "boolean" ? raw.default : null
    }];
  });
}
