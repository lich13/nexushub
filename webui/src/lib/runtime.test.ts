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

describe("NexusHub runtime adapter", () => {
  afterEach(() => {
    delete globalThis.__NEXUSHUB_DESKTOP_RUNTIME__;
    delete globalThis.__NEXUSHUB_TEST_INVOKE__;
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
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

  test("keeps API requests at root by default when no API base is configured", async () => {
    vi.stubEnv("BASE_URL", "/nexushub/");
    const { buildRuntimeApiPath } = await loadRuntime();

    expect(buildRuntimeApiPath("/api/auth/login")).toBe("/api/auth/login");
  });

  test("uses an explicit API base override when the WebUI is served from a subpath", async () => {
    vi.stubEnv("BASE_URL", "/nexushub/");
    vi.stubEnv("VITE_API_BASE", "/backend/");
    const { buildRuntimeApiPath } = await loadRuntime();

    expect(buildRuntimeApiPath("/api/auth/login")).toBe("/backend/api/auth/login");
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
      command: "desktop_threads",
      args: { request: { status: "all", query: "plan", limit: 20 } }
    });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  test("desktop api bridge route is not available", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, args) => ({ command, args }));
    const { runtimeRpc } = await loadRuntime(true);

    await expect(runtimeRpc("desktopApi", {
      request: { path: "/api/threads/thread-a/rename", method: "POST", body: { name: "新标题" } }
    })).rejects.toMatchObject({ feature: "desktopApi" });

    expect(fetchMock).not.toHaveBeenCalled();
    expect(globalThis.__NEXUSHUB_TEST_INVOKE__).not.toHaveBeenCalled();
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

  test("web thread event transport opens EventSource through runtime API paths", async () => {
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
    expect(MockEventSource.instances[0].url).toBe("/api/threads/thread-a/events");
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

  test("desktop runtime routes shared app capabilities through typed native commands", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, args) => ({ command, args }));
    const { runtimeRpc } = await loadRuntime(true);

    await runtimeRpc("getProbeSettings");
    await runtimeRpc("saveProbeSettings", { settings: { probe: { enabled: true } } });
    await runtimeRpc("getProbeEvents", { limit: 12 });
    await runtimeRpc("startProbeJob", { action: "bark-test" });
    await runtimeRpc("getClaudeCodeOverview");
    await runtimeRpc("getThreadBlocks", { id: "thread-a", options: { limit: 80, before: "b:200" } });
    await runtimeRpc("createThread", { payload: { message: "hello" } });
    await runtimeRpc("sendMessage", { threadId: "thread-a", payload: { message: "resume" } });
    await runtimeRpc("archiveThread", { threadId: "thread-a" });
    await runtimeRpc("renameThread", { threadId: "thread-a", name: "新标题" });

    expect(fetchMock).not.toHaveBeenCalled();
    const calls = (globalThis.__NEXUSHUB_TEST_INVOKE__ as ReturnType<typeof vi.fn>).mock.calls;
    expect(calls.map(([command]) => command)).toEqual([
      "desktop_probe_settings",
      "desktop_probe_save_settings",
      "desktop_probe_events",
      "desktop_probe_bark_test",
      "desktop_claude_code_overview",
      "desktop_thread_blocks",
      "desktop_send_message",
      "desktop_send_message",
      "desktop_archive_thread",
      "desktop_rename_thread"
    ]);
    expect(calls[1][1]).toMatchObject({ request: { probe: { enabled: true } } });
    expect(calls[2][1]).toEqual({ request: { limit: 12 } });
    expect(calls[5][1]).toEqual({ request: { id: "thread-a", limit: 80, before: "b:200" } });
    expect(calls[6][1]).toMatchObject({ request: { message: "hello" } });
    expect(calls[7][1]).toEqual({ request: { message: "resume", threadId: "thread-a" } });
    expect(calls[8][1]).toEqual({ request: { threadId: "thread-a" } });
    expect(calls[9][1]).toEqual({ request: { threadId: "thread-a", name: "新标题" } });
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
          current_version: "0.1.103",
          latest_version: "v0.1.103",
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

  test("production runtime keeps the retired desktop API bridge out of the route table", async () => {
    expect(runtimeSource).not.toContain("desktopApiRoute");
    expect(runtimeSource).not.toContain("invokeDesktopApi");
    expect(runtimeSource).not.toContain("desktop_api_command");
    expect(runtimeSource).not.toContain("DesktopApiUpload");
    expect(runtimeSource).not.toContain('runtimeRpc("desktopApi"');
  });
});
