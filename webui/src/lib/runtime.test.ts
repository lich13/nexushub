import { afterEach, describe, expect, test, vi } from "vitest";
import type { RemoteConnectionView } from "./runtime";

const local: RemoteConnectionView = { target: "local", revision: 0, baseUrl: null, configured: false };
const remote: RemoteConnectionView = { target: "remote", revision: 1, baseUrl: "https://api.example.com/nexushub/", configured: true };
const deferred = <T>() => {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
};

async function loadRuntime(handler: (command: string, args?: Record<string, unknown>) => unknown = () => local) {
  vi.resetModules();
  const invoke = vi.fn(handler);
  vi.stubGlobal("__NEXUSHUB_TEST_INVOKE__", invoke);
  const runtime = await import("./runtime");
  return { ...runtime, invoke };
}

afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); vi.resetModules(); });

describe("native machine connection", () => {
  test("initializes once from the native view without exposing credentials", async () => {
    const runtime = await loadRuntime(() => remote);
    await Promise.all([runtime.initializeConnection(), runtime.initializeConnection()]);
    expect(runtime.invoke).toHaveBeenCalledExactlyOnceWith("remote.get", undefined);
    expect(runtime.connectionSnapshot()).toMatchObject({ ...remote, ready: true, writes: 0 });
    expect(runtime.connectionSnapshot()).not.toHaveProperty("apiKey");
    expect(runtime.machineScope()).toBe("remote:https://api.example.com/nexushub/");
  });

  test("local business calls retain the typed native command and never use browser fetch", async () => {
    const fetch = vi.fn(); vi.stubGlobal("fetch", fetch);
    const runtime = await loadRuntime((command, args) => ({ command, args }));
    await expect(runtime.runtimeRpc("threads.list", { status: "all", q: "plan", limit: 20 })).resolves.toEqual({
      command: "threads.list", args: { status: "all", q: "plan", limit: 20 }
    });
    expect(fetch).not.toHaveBeenCalled();
  });

  test("remote selection routes the same business identity through its revision", async () => {
    const runtime = await loadRuntime((command, args) => command === "remote.select" ? remote : { command, args });
    await runtime.remoteSelect("remote");
    await expect(runtime.runtimeRpc("threads.detail", { id: "same-thread" })).resolves.toEqual({
      command: "remote.invoke", args: { request: { revision: 1, command: "threads.detail", args: { id: "same-thread" } } }
    });
    expect(runtime.invoke).toHaveBeenCalledWith("remote.select", { request: { target: "remote", revision: 0 } });
  });

  test("a read finishing after a connection switch is discarded", async () => {
    const oldRead = deferred<unknown>();
    const runtime = await loadRuntime(command => command === "remote.select" ? remote : oldRead.promise);
    const pending = runtime.runtimeRpc("threads.detail", { id: "same-thread" });
    const rejected = expect(pending).rejects.toThrow("已丢弃旧响应");
    await runtime.remoteSelect("remote");
    oldRead.resolve({ title: "local stale title" });
    await rejected;
    expect(runtime.connectionSnapshot()).toMatchObject(remote);
  });

  test("writes block select, save, verify and remove until the native mutation settles", async () => {
    const mutation = deferred<unknown>();
    const runtime = await loadRuntime(command => command === "threads.rename" ? mutation.promise : remote);
    const pending = runtime.runtimeRpc("threads.rename", { threadId: "same-thread", name: "New title" });
    expect(runtime.connectionSnapshot().writes).toBe(1);
    for (const operation of [
      () => runtime.remoteSelect("remote"),
      () => runtime.remoteSave({ revision: 0, baseUrl: remote.baseUrl!, apiKey: "fixture-key" }),
      () => runtime.remoteVerify({ revision: 0, baseUrl: remote.baseUrl!, apiKey: "fixture-key" }),
      () => runtime.remoteRemove()
    ]) await expect(operation()).rejects.toThrow("操作进行中，请稍后切换");
    expect(runtime.invoke).toHaveBeenCalledTimes(1);
    mutation.resolve({ ok: true }); await pending;
    await runtime.remoteSelect("remote");
    expect(runtime.connectionSnapshot()).toMatchObject({ ...remote, writes: 0 });
  });

  test("a rejected mutation releases the write lock", async () => {
    const runtime = await loadRuntime(command => {
      if (command === "threads.rename") throw "Native rename failed";
      return remote;
    });
    await expect(runtime.runtimeRpc("threads.rename")).rejects.toThrow("Native rename failed");
    expect(runtime.connectionSnapshot().writes).toBe(0);
    await runtime.remoteSelect("remote");
  });

  test.each(["succeeded", "failed", "cancelled"])("background writes block switching until the job is %s", async status => {
    let phase = "running";
    const runtime = await loadRuntime(command => {
      if (command === "updates.check") return { job_id: "update-1" };
      if (command === "jobs.detail") return { id: "update-1", status: phase };
      return remote;
    });
    await runtime.runtimeRpc("updates.check");
    expect(runtime.connectionSnapshot().writes).toBe(1);
    await expect(runtime.remoteSelect("remote")).rejects.toThrow("操作进行中");
    await runtime.runtimeRpc("jobs.detail", { id: "update-1" });
    expect(runtime.connectionSnapshot().writes).toBe(1);
    phase = status;
    await runtime.runtimeRpc("jobs.detail", { id: "update-1" });
    await runtime.runtimeRpc("jobs.detail", { id: "update-1" });
    expect(runtime.connectionSnapshot().writes).toBe(0);
    await runtime.remoteSelect("remote");
    expect(runtime.connectionSnapshot().target).toBe("remote");
  });

  test("an offline remote read stays remote and never falls back to local data", async () => {
    const runtime = await loadRuntime(command => {
      if (command === "remote.select") return remote;
      throw new Error("远程连接失败");
    });
    await runtime.remoteSelect("remote");
    await expect(runtime.runtimeRpc("threads.list")).rejects.toThrow("远程连接失败");
    expect(runtime.connectionSnapshot()).toMatchObject(remote);
    expect(runtime.invoke.mock.calls.map(([command]) => command)).toEqual(["remote.select", "remote.invoke"]);
  });

  test("credentials are sent only to save and are absent from subsequent views and business calls", async () => {
    const runtime = await loadRuntime(command => command === "remote.save" ? remote : []);
    const credentials = { revision: 0, baseUrl: remote.baseUrl!, apiKey: "fixture-secret-key" };
    await runtime.remoteSave(credentials);
    expect(runtime.invoke).toHaveBeenCalledWith("remote.save", { request: credentials });
    expect(JSON.stringify(runtime.connectionSnapshot())).not.toContain(credentials.apiKey);
    await runtime.runtimeRpc("threads.list");
    expect(runtime.invoke.mock.calls[1]).toEqual(["remote.invoke", { request: { revision: 1, command: "threads.list", args: {} } }]);
  });

  test("native Plan saving stays local while update reads use the selected machine", async () => {
    const runtime = await loadRuntime((command, args) => command === "remote.select" ? remote : { command, args });
    await runtime.remoteSelect("remote");
    await expect(runtime.runtimeRpc("plans.save", {}, true)).resolves.toEqual({ command: "plans.save", args: {} });
    await runtime.runtimeRpc("updates.status");
    expect(runtime.invoke.mock.calls.map(([command]) => command)).toEqual(["remote.select", "plans.save", "remote.invoke"]);
    expect(runtime.connectionSnapshot()).toMatchObject({ ...remote, writes: 0 });
  });

  test("an uninitialized browser refuses business calls without a native bridge", async () => {
    vi.resetModules(); vi.stubGlobal("__NEXUSHUB_TEST_INVOKE__", undefined);
    const fetch = vi.fn(); vi.stubGlobal("fetch", fetch);
    const { runtimeRpc } = await import("./runtime");
    await expect(runtimeRpc("threads.list")).rejects.toThrow("请使用 NexusHub App");
    expect(fetch).not.toHaveBeenCalled();
  });
});
