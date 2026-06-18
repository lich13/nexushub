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

  test("desktop runtime routes shared app capabilities through the native API bridge", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, args) => ({ command, args }));
    const { runtimeRpc } = await loadRuntime(true);

    await runtimeRpc("getProbeSettings");
    await runtimeRpc("saveProbeSettings", { settings: { probe: { enabled: true } } });
    await runtimeRpc("getProbeEvents", { limit: 12 });
    await runtimeRpc("startProbeJob", { path: "/api/probe/bark/test" });
    await runtimeRpc("getClaudeCodeOverview");
    await runtimeRpc("getThreadBlocks", { id: "thread-a", options: { limit: 80, before: "b:200" } });
    await runtimeRpc("createThread", { payload: { message: "hello" } });
    await runtimeRpc("sendMessage", { threadId: "thread-a", payload: { message: "resume" } });
    await runtimeRpc("archiveThread", { threadId: "thread-a" });
    await runtimeRpc("renameThread", { threadId: "thread-a", name: "新标题" });

    expect(fetchMock).not.toHaveBeenCalled();
    expect((globalThis.__NEXUSHUB_TEST_INVOKE__ as ReturnType<typeof vi.fn>).mock.calls).toEqual([
      ["desktop_api_command", { request: { path: "/api/probe/settings", method: "GET" } }],
      ["desktop_api_command", { request: { path: "/api/probe/settings", method: "PATCH", body: { probe: { enabled: true } } } }],
      ["desktop_api_command", { request: { path: "/api/probe/events?limit=12", method: "GET" } }],
      ["desktop_api_command", { request: { path: "/api/probe/bark/test", method: "POST" } }],
      ["desktop_api_command", { request: { path: "/api/providers/claude-code/overview", method: "GET" } }],
      ["desktop_thread_blocks", { request: { id: "thread-a", limit: 80, before: "b:200" } }],
      ["desktop_api_command", { request: { path: "/api/threads", method: "POST", body: { message: "hello" } } }],
      ["desktop_api_command", { request: { path: "/api/threads/thread-a/messages", method: "POST", body: { message: "resume" } } }],
      ["desktop_api_command", { request: { path: "/api/threads/thread-a/archive", method: "POST" } }],
      ["desktop_api_command", { request: { path: "/api/threads/thread-a/rename", method: "POST", body: { name: "新标题" } } }]
    ]);
  });

  test("desktop update action routes to macOS updater command", async () => {
    const { runtimeRpc } = await loadRuntime(true);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, _args) => {
      expect(command).toBe("install_update_and_restart");
      return { job_id: "desktop-native-job", installed: false };
    });

    await expect(runtimeRpc("runUpdateAction", { action: "install" })).resolves.toMatchObject({
      job_id: "desktop-native-job",
      installed: false
    });
  });

  test("desktop update check routes to signed updater feed and job history command", async () => {
    const { runtimeRpc } = await loadRuntime(true);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, _args) => {
      expect(command).toBe("check_update_status");
      return {
        job_id: "desktop-check-job",
        status: {
          current_version: "0.1.101",
          latest_version: "v0.1.102",
          update_available: true,
          channel: "stable",
          method: "macos_tauri_updater",
          state: "ready",
          recommended_action: "Confirm install in the Tauri updater after signature verification.",
          capabilities: ["signature_verification", "job_history"]
        }
      };
    });

    await expect(runtimeRpc("runUpdateAction", { action: "check" })).resolves.toMatchObject({
      job_id: "desktop-check-job",
      status: { state: "ready" }
    });
  });
});
