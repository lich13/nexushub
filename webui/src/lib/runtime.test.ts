import { afterEach, describe, expect, test, vi } from "vitest";
import runtimeSource from "./runtime.ts?raw";

async function loadRuntime(desktop = false) {
  vi.resetModules();
  if (desktop) {
    globalThis.__NEXUSHUB_DESKTOP_RUNTIME__ = true;
  } else {
    delete globalThis.__NEXUSHUB_DESKTOP_RUNTIME__;
  }
  return import("./runtime");
}

describe("NexusHub runtime transport", () => {
  afterEach(() => {
    delete globalThis.__NEXUSHUB_DESKTOP_RUNTIME__;
    delete globalThis.__NEXUSHUB_TEST_INVOKE__;
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
    vi.resetModules();
  });

  test("web rpc posts one command envelope to the Linux RPC endpoint", async () => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({ ok: true }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);
    const { runtimeRpc } = await loadRuntime();

    await runtimeRpc("auth.publicSettings", { csrfToken: "csrf-token", q: "needle" });

    const [path, options] = fetchMock.mock.calls[0] as unknown as [string, RequestInit & { headers: Headers; body: string }];
    expect(path).toBe("/api/rpc/auth.publicSettings");
    expect(options.method).toBe("POST");
    expect(options.credentials).toBe("include");
    expect(options.headers.get("content-type")).toBe("application/json");
    expect(options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(JSON.parse(options.body)).toEqual({ q: "needle" });
  });

  test("runtime only exposes transport primitives", async () => {
    const runtime = await loadRuntime();

    expect(Object.keys(runtime).sort()).toEqual([
      "RuntimeUnavailableError",
      "buildRuntimeApiPath",
      "createRuntimeThreadEventSource",
      "runtimeRpc",
      "uploadRuntimeFiles"
    ]);
    expect(runtimeSource).not.toContain("selectRuntimeFallback");
    expect(runtimeSource).not.toContain("runtimeValue");
  });

  test("runtime source exports only the public transport boundary", () => {
    const exportedNames = Array.from(
      runtimeSource.matchAll(/^export\s+(?:async\s+)?(?:class|function|type|interface|const|let|var)\s+(\w+)/gm),
      (match) => match[1],
    ).sort();

    expect(exportedNames).toEqual([
      "RuntimeUnavailableError",
      "buildRuntimeApiPath",
      "createRuntimeThreadEventSource",
      "runtimeRpc",
      "uploadRuntimeFiles",
    ]);
  });

  test("keeps API requests at root by default when no API base is configured", async () => {
    vi.stubEnv("BASE_URL", "/nexushub/");
    const { buildRuntimeApiPath } = await loadRuntime();

    expect(buildRuntimeApiPath("/api/rpc/login")).toBe("/api/rpc/login");
  });

  test("uses an explicit API base override when the WebUI is served from a subpath", async () => {
    vi.stubEnv("BASE_URL", "/nexushub/");
    vi.stubEnv("VITE_API_BASE", "/backend/");
    const { buildRuntimeApiPath } = await loadRuntime();

    expect(buildRuntimeApiPath("/api/rpc/login")).toBe("/backend/api/rpc/login");
  });

  test("desktop dispatch invokes typed Tauri commands and never calls fetch", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, args) => ({
      command,
      args
    }));
    const { runtimeRpc } = await loadRuntime(true);

    const result = await runtimeRpc("threads.list", { status: "all", q: "plan", limit: 20 });

    expect(result).toEqual({
      command: "threads.list",
      args: { status: "all", q: "plan", limit: 20 }
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  test("desktop dispatch strips CSRF-only transport args before native invoke", async () => {
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, args) => ({ command, args }));
    const { runtimeRpc } = await loadRuntime(true);

    await expect(runtimeRpc("updates.check", { csrfToken: "csrf-token" })).resolves.toEqual({
      command: "updates.check",
      args: undefined
    });
  });

  test("desktop upload helper delegates to native upload command", async () => {
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, args) => ({ command, args }));
    const { uploadRuntimeFiles } = await loadRuntime(true);

    const result = await uploadRuntimeFiles([new File(["#"], "note.md", { type: "text/markdown" })]);

    expect(result).toEqual({
      command: "uploadFiles",
      args: { files: [{ name: "note.md", mime: "text/markdown", bytes: [35] }] }
    });
  });

  test("web upload transport posts FormData to the RPC upload endpoint", async () => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({ files: [] }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);
    const { uploadRuntimeFiles } = await loadRuntime();

    await uploadRuntimeFiles([new File(["# Plan"], "plan.md", { type: "text/markdown" })], "csrf-token");

    const [path, options] = fetchMock.mock.calls[0] as unknown as [string, RequestInit & { headers: Headers; body: FormData }];
    expect(path).toBe("/api/rpc/uploadFiles");
    expect(options.method).toBe("POST");
    expect(options.body).toBeInstanceOf(FormData);
    expect(options.headers.get("content-type")).toBeNull();
    expect(options.headers.get("x-csrf-token")).toBe("csrf-token");
  });

  test("web thread event transport opens EventSource through the runtime RPC stream", async () => {
    const close = vi.fn();
    class MockEventSource {
      static instances: MockEventSource[] = [];
      constructor(readonly url: string, readonly init?: EventSourceInit) {
        MockEventSource.instances.push(this);
      }
      addEventListener = vi.fn();
      close = close;
    }
    vi.stubGlobal("EventSource", MockEventSource);
    const { createRuntimeThreadEventSource } = await loadRuntime();

    const source = createRuntimeThreadEventSource("thread-a");
    source.close();

    expect(MockEventSource.instances).toHaveLength(1);
    expect(MockEventSource.instances[0].url).toBe("/api/rpc/threadEvents/thread-a");
    expect(MockEventSource.instances[0].init).toEqual({ withCredentials: true });
    expect(close).toHaveBeenCalledOnce();
  });

  test("desktop thread event transport is unavailable without touching EventSource", async () => {
    const EventSourceMock = vi.fn();
    vi.stubGlobal("EventSource", EventSourceMock);
    const { createRuntimeThreadEventSource } = await loadRuntime(true);

    const source = createRuntimeThreadEventSource("thread-a");
    source.addEventListener("block", vi.fn());
    source.close();

    expect(source.unavailable).toBe(true);
    expect(EventSourceMock).not.toHaveBeenCalled();
  });

  test("production runtime stays a thin transport layer", async () => {
    const runtimeDispatchOptionsBody = runtimeSource.match(/export type RuntimeDispatchOptions[\s\S]*?};/)?.[0] ?? "";

    expect(runtimeSource).not.toContain("const ROUTES");
    expect(runtimeSource).not.toContain("WebRoute");
    expect(runtimeSource).not.toContain("DesktopRoute");
    expect(runtimeSource).not.toContain("fromHome");
    expect(runtimeSource).not.toMatch(/export async function webJsonRpc\b/);
    expect(runtimeSource).not.toMatch(/export async function webFormRpc\b/);
    expect(runtimeSource).not.toMatch(/export async function invokeDesktop\b/);
    expect(runtimeSource).not.toMatch(/export async function invokeDesktopUpload\b/);
    expect(runtimeSource).not.toMatch(/export function getRuntimeKind\b/);
    expect(runtimeSource).not.toContain("desktop_api_command");
    expect(runtimeSource).not.toContain("desktopApiRoute");
    expect(runtimeSource).not.toContain("invokeDesktopApi");
    expect(runtimeDispatchOptionsBody).not.toContain("desktopCommand");
    expect(runtimeDispatchOptionsBody).not.toContain("desktopArgs");
    expect(runtimeDispatchOptionsBody).not.toContain("webCommand");
    expect(runtimeDispatchOptionsBody).not.toContain("webArgs");
    expect(runtimeSource).not.toContain("systemd");
    expect(runtimeSource).not.toContain("Nginx");
  });

  test("runtime kind checks and dispatch are not production-facing exports", () => {
    expect(runtimeSource).not.toMatch(/export function isDesktopRuntime\b/);
    expect(runtimeSource).not.toMatch(/export function isWebRuntime\b/);
    expect(runtimeSource).not.toMatch(/export async function runtimeDispatch\b/);
    expect(runtimeSource).not.toContain("__testRuntimeDispatch");
  });
});
