import { useSyncExternalStore } from "react";

export type MachineTarget = "local" | "remote";
export type RemoteConnectionView = { target: MachineTarget; revision: number; baseUrl: string | null; configured: boolean };
export type RemoteConnectionCredentials = { revision: number; baseUrl: string; apiKey: string };
export type RemoteRevisionRequest = { revision: number };
export type RemoteSelectionRequest = RemoteRevisionRequest & { target: MachineTarget };
export type RemoteInvokeRequest = RemoteRevisionRequest & { command: string; args: Record<string, unknown> };
export type RemoteInvokeResponse = unknown;
export type RuntimeContext = { kind: "desktop" };
type RpcArgs = Record<string, unknown> | undefined;
type RuntimeGlobal = typeof globalThis & {
  __TAURI_INTERNALS__?: { invoke?: (command: string, args?: RpcArgs) => Promise<unknown> };
  __NEXUSHUB_DESKTOP_RUNTIME__?: boolean;
  __NEXUSHUB_TEST_INVOKE__?: (command: string, args?: RpcArgs) => Promise<unknown> | unknown;
};
export class RuntimeUnavailableError extends Error {
  constructor(message: string, readonly feature: string) { super(message); this.name = "RuntimeUnavailableError"; }
}
export function runtimeContext(): RuntimeContext { return { kind: "desktop" }; }
export function hasNativeRuntime() {
  const runtime = globalThis as RuntimeGlobal;
  return Boolean(runtime.__TAURI_INTERNALS__?.invoke || runtime.__NEXUSHUB_TEST_INVOKE__);
}

export async function invokeNative<T = unknown>(command: string, args?: RpcArgs): Promise<T> {
  const runtime = globalThis as RuntimeGlobal;
  try {
    const invoke = runtime.__NEXUSHUB_TEST_INVOKE__ ?? runtime.__TAURI_INTERNALS__?.invoke;
    if (invoke) return await invoke(command, args) as T;
  } catch (reason) {
    throw reason instanceof Error ? reason : new Error(typeof reason === "string" ? reason : "桌面操作失败");
  }
  throw new RuntimeUnavailableError("请使用 NexusHub App", command);
}

const listeners = new Set<() => void>();
let connection = { target: "local" as MachineTarget, revision: 0, baseUrl: null as string | null, configured: false, ready: false, writes: 0, changing: false, error: "", notice: "" };
function publish(patch: Partial<typeof connection>) { connection = { ...connection, ...patch }; listeners.forEach(listener => listener()); }
export function connectionSnapshot() { return connection; }
export function machineScope() { return `${connection.target}:${connection.target === "remote" ? connection.baseUrl : ""}`; }
export function useConnection() { return useSyncExternalStore(listener => { listeners.add(listener); return () => listeners.delete(listener); }, connectionSnapshot); }
let initialization: Promise<void> | undefined;
export function initializeConnection() {
  return initialization ??= (async () => {
    try { publish({ ...await remoteGet(), ready: true, error: "" }); }
    catch (error) { publish({ ready: true, error: (error as Error).message }); }
  })();
}
export function remoteGet() { return invokeNative<RemoteConnectionView>("remote.get"); }
async function changeConnection<T>(command: string, request: unknown): Promise<T> {
  if (connection.writes || connection.changing) throw new Error("操作进行中，请稍后切换");
  publish({ changing: true });
  try {
    const result = await invokeNative<T>(command, { request });
    if (command !== "remote.verify") publish({ ...result as RemoteConnectionView, error: "", notice: command === "remote.save" ? "已保存" : "" });
    return result;
  }
  finally { publish({ changing: false }); }
}
export async function remoteVerify(request: RemoteConnectionCredentials) {
  return changeConnection<import("../types").SystemCapabilitiesResponse>("remote.verify", request);
}
export async function remoteSave(request: RemoteConnectionCredentials) {
  const view = await changeConnection<RemoteConnectionView>("remote.save", request);
  return view;
}
export async function remoteRemove() {
  const view = await changeConnection<RemoteConnectionView>("remote.remove", { revision: connection.revision });
  return view;
}
export async function remoteSelect(target: MachineTarget) {
  if (target === connection.target) return connection;
  const view = await changeConnection<RemoteConnectionView>("remote.select", { revision: connection.revision, target });
  return view;
}
export function remoteInvoke<T = unknown>(request: RemoteInvokeRequest) { return invokeNative<T>("remote.invoke", { request }); }

const reads = new Set(["system.capabilities", "system.version", "system.platform", "system.providers", "threads.list", "threads.detail", "threads.subagentDetail", "threads.blocks", "jobs.list", "jobs.detail", "probe.status", "probe.settings.get", "probe.events", "updates.status", "grok.list", "grok.detail", "grok.deletePreview", "claude.list", "claude.detail", "claude.deletePreview", "sessions.attachmentRead", "sessions.search", "sessions.bulkPreview"]);
const pendingJobs = new Set<string>();
export async function runtimeRpc<T = unknown>(command: string, args?: RpcArgs, localOnly = false): Promise<T> {
  if (connection.changing) throw new Error("连接切换中，请稍后重试");
  const captured = connection;
  const write = !reads.has(command);
  if (write) publish({ writes: connection.writes + 1 });
  try {
    const value = captured.target === "remote" && !localOnly
      ? await remoteInvoke<T>({ revision: captured.revision, command, args: args ?? {} })
      : await invokeNative<T>(command, args);
    if (captured.revision !== connection.revision) throw new Error("连接已变化，已丢弃旧响应");
    if (value && typeof value === "object") {
      const result = value as Record<string, unknown>;
      const jobId = result.job_id ?? result.jobId;
      if (write && typeof jobId === "string" && jobId && !pendingJobs.has(jobId)) {
        // A server write continues after its RPC returns; the existing job poller
        // releases this machine lock only after observing a terminal result.
        pendingJobs.add(jobId);
        publish({ writes: connection.writes + 1 });
      }
      if (command === "jobs.detail" && typeof result.id === "string"
        && ["succeeded", "failed", "cancelled"].includes(String(result.status))
        && pendingJobs.delete(result.id)) {
        publish({ writes: connection.writes - 1 });
      }
    }
    return value;
  } finally { if (write) publish({ writes: connection.writes - 1 }); }
}
