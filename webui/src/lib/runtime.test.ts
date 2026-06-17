import { afterEach, describe, expect, test, vi } from "vitest";

async function loadRuntime(desktop = false) {
  vi.resetModules();
  if (desktop) {
    globalThis.__NEXUSHUB_DESKTOP_RUNTIME__ = true;
  } else {
    delete globalThis.__NEXUSHUB_DESKTOP_RUNTIME__;
  }
  return import("./runtime");
}

describe("NexusHub runtime adapter", () => {
  afterEach(() => {
    delete globalThis.__NEXUSHUB_DESKTOP_RUNTIME__;
    delete globalThis.__NEXUSHUB_TEST_INVOKE__;
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  test("detects web runtime by default and desktop runtime when forced for tests", async () => {
    expect((await loadRuntime()).getRuntimeKind()).toBe("web");
    expect((await loadRuntime(true)).getRuntimeKind()).toBe("desktop");
  });

  test("web rpc posts to the existing HTTP endpoint", async () => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({ ok: true }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);
    const { runtimeRpc } = await loadRuntime();

    await runtimeRpc("getPublicSettings");

    const [path, options] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(path).toBe("/api/public/settings");
    expect(options.method).toBe("GET");
    expect(options.credentials).toBe("include");
  });

  test("desktop rpc invokes Tauri commands and never calls fetch", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, args) => ({
      command,
      args
    }));
    const { runtimeRpc } = await loadRuntime(true);

    const result = await runtimeRpc("listThreads", {
      status: "all",
      q: "plan",
      limit: 20
    });

    expect(result).toEqual({
      command: "desktop_threads_command",
      args: { request: { status: "all", query: "plan", limit: 20 } }
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  test("desktop api bridge invokes the native desktop_api_command", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, args) => ({ command, args }));
    const { runtimeRpc } = await loadRuntime(true);

    const result = await runtimeRpc("desktopApi", {
      request: { path: "/api/threads/thread-a/rename", method: "POST", body: { name: "新标题" } }
    });

    expect(result).toEqual({
      command: "desktop_api_command",
      args: { request: { path: "/api/threads/thread-a/rename", method: "POST", body: { name: "新标题" } } }
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  test("desktop direct API helper delegates all routes to the native bridge", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, args) => ({ command, args }));
    const { invokeDesktopApi } = await loadRuntime(true);

    const result = await invokeDesktopApi({
      path: "/api/probe/settings",
      method: "PATCH",
      body: { probe: { enabled: true } }
    });

    expect(result).toEqual({
      command: "desktop_api_command",
      args: {
        request: {
          path: "/api/probe/settings",
          method: "PATCH",
          body: { probe: { enabled: true } }
        }
      }
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  test("desktop upload helper delegates to native upload command", async () => {
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, args) => ({ command, args }));
    const { invokeDesktopUpload } = await loadRuntime(true);

    const result = await invokeDesktopUpload([{ name: "note.md", mime: "text/markdown", bytes: [35] }]);

    expect(result).toEqual({
      command: "desktop_upload_files_command",
      args: { files: [{ name: "note.md", mime: "text/markdown", bytes: [35] }] }
    });
  });

  test("desktop unsupported capabilities reject instead of reporting success", async () => {
    const { runtimeRpc, RuntimeUnavailableError } = await loadRuntime(true);

    await expect(runtimeRpc("startUpdateJob", {
      path: "/api/system/panel/update/start",
      csrfToken: "ignored"
    })).rejects.toBeInstanceOf(RuntimeUnavailableError);
  });
});
