import { afterEach, describe, expect, test, vi } from "vitest";
import appSource from "../App.tsx?raw";
import apiSource from "./api.ts?raw";
import type { MessageBlock, ProbeLogsDbStatus, ProbeStatus, SystemStatus, ThreadDetail, ThreadSummary } from "../types";

async function loadRealApi() {
  vi.stubEnv("VITE_USE_REAL_API", "1");
  vi.resetModules();
  return import("./api");
}

async function loadDesktopApi() {
  vi.stubEnv("VITE_USE_REAL_API", "1");
  vi.resetModules();
  globalThis.__NEXUSHUB_DESKTOP_RUNTIME__ = true;
  return import("./api");
}

function rpcCall(fetchMock: ReturnType<typeof vi.fn>, index = 0) {
  const [path, options] = fetchMock.mock.calls[index] as [string, RequestInit & { headers: Headers; body?: string | FormData }];
  return {
    path,
    command: path.replace(/^.*\/api\/rpc\//, ""),
    options,
    body: typeof options.body === "string" ? JSON.parse(options.body) : options.body
  };
}

describe("archive delete API compatibility", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
    delete globalThis.__NEXUSHUB_DESKTOP_RUNTIME__;
    delete globalThis.__NEXUSHUB_TEST_INVOKE__;
    vi.resetModules();
  });

  test("uses a boolean confirmation payload without requiring typed UI confirmation", async () => {
    const { startArchiveDelete } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response("{}", {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    await startArchiveDelete("csrf-token");

    const call = rpcCall(fetchMock);
    expect(call.path).toBe("/api/rpc/startArchiveDelete");
    expect(call.options.method).toBe("POST");
    expect(call.options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(call.body).toEqual({ confirmed: true });
  });

  test("uses hidden thread cleanup endpoints with dry-run and boolean confirmation", async () => {
    const { dryRunHiddenThreadDelete, startHiddenThreadDelete } = await loadRealApi();
    const fetchMock = vi.fn(async (path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify(
      String(path).endsWith("/dryRunHiddenThreadDelete")
        ? {
          total_threads: 9,
          visible_threads: 7,
          hidden_threads: 2,
          archived_threads: 0,
          session_index_lines: 9,
          rollout_files: 9,
          hidden_ids: ["child-a", "child-b"],
          hidden_source_counts: { subagent: 2 },
          integrity: "ok"
        }
        : {
          before: {
            total_threads: 9,
            visible_threads: 7,
            hidden_threads: 2,
            archived_threads: 0,
            session_index_lines: 9,
            rollout_files: 9,
            hidden_ids: ["child-a", "child-b"],
            hidden_source_counts: { subagent: 2 },
            integrity: "ok"
          },
          deleted_threads: 2,
          after_total_threads: 7,
          after_visible_threads: 7,
          after_hidden_threads: 0,
          after_archived_threads: 0,
          after_integrity: "ok",
          visible_threads: 7,
          hidden_threads: 0,
          integrity: "ok",
          deleted_rollout_files: 2
        }
    ), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    const plan = await dryRunHiddenThreadDelete("csrf-token");
    const result = await startHiddenThreadDelete("csrf-token");

    expect(plan.hidden_threads).toBe(2);
    expect(result.deleted_threads).toBe(2);
    expect(fetchMock.mock.calls.map(([path]) => path)).toEqual([
      "/api/rpc/dryRunHiddenThreadDelete",
      "/api/rpc/startHiddenThreadDelete"
    ]);
    const execute = rpcCall(fetchMock, 1);
    expect(execute.options.method).toBe("POST");
    expect(execute.options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(execute.body).toEqual({ confirmed: true });
  });

  test("upload API posts FormData without JSON content-type and deletes uploads with csrf", async () => {
    const { uploadFiles, deleteUpload } = await loadRealApi();
    const fetchMock = vi.fn(async (path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify(
      String(path).endsWith("/deleteUpload")
        ? { ok: true, deleted: true }
        : {
          files: [{
            id: "upload-1",
            name: "plan.md",
            mime: "text/markdown",
            size: 12,
            sha256: "sha",
            kind: "markdown",
            status: "ready"
          }]
        }
    ), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    const outcome = await uploadFiles([new File(["# Plan"], "plan.md", { type: "text/markdown" })], "csrf-token");
    const deleted = await deleteUpload("upload-1", "csrf-token");

    expect(outcome.files[0].id).toBe("upload-1");
    expect(deleted.deleted).toBe(true);
    const upload = rpcCall(fetchMock, 0);
    expect(upload.path).toBe("/api/rpc/uploadFiles");
    expect(upload.options.method).toBe("POST");
    expect(upload.body).toBeInstanceOf(FormData);
    expect(upload.options.headers.get("content-type")).toBeNull();
    expect(upload.options.headers.get("x-csrf-token")).toBe("csrf-token");
    const deletedCall = rpcCall(fetchMock, 1);
    expect(deletedCall.path).toBe("/api/rpc/deleteUpload");
    expect(deletedCall.options.method).toBe("POST");
    expect(deletedCall.options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(deletedCall.body).toEqual({ id: "upload-1" });
  });

  test("stop thread posts stop payload with csrf", async () => {
    const { stopThread } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response("{}", {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    await stopThread("thread-a", { turn_id: "turn-live", job_id: "job-live" }, "csrf-token");

    const call = rpcCall(fetchMock);
    expect(call.path).toBe("/api/rpc/stopThread");
    expect(call.options.method).toBe("POST");
    expect(call.options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(call.body).toEqual({
      threadId: "thread-a",
      payload: { turn_id: "turn-live", job_id: "job-live" }
    });
  });

  test("codex goal helpers cover get save clear pause and resume endpoints", async () => {
    const { getCodexGoal, saveCodexGoal, clearCodexGoal, pauseCodexGoal, resumeCodexGoal } = await loadRealApi();
    const responses = [
      { available: true, enabled: false, objective: null, token_budget: null, status: "idle" },
      { available: true, enabled: true, objective: "ship local goal", token_budget: 12345, status: "active" },
      { available: true, enabled: false, objective: null, token_budget: null, status: "cleared" },
      { available: true, enabled: true, objective: "ship local goal", token_budget: 12345, status: "paused" },
      { available: true, enabled: true, objective: "ship local goal", token_budget: 12345, status: "active" }
    ];
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify(responses.shift()), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    const initial = await getCodexGoal("thread-a");
    const saved = await saveCodexGoal("thread-a", { objective: "ship local goal", token_budget: 12345 }, "csrf-token");
    const cleared = await clearCodexGoal("thread-a", "csrf-token");
    const paused = await pauseCodexGoal("thread-a", "csrf-token");
    const resumed = await resumeCodexGoal("thread-a", "csrf-token");

    expect(initial.status).toBe("idle");
    expect(saved.status).toBe("active");
    expect(cleared.status).toBe("cleared");
    expect(paused.status).toBe("paused");
    expect(resumed.status).toBe("active");
    expect(fetchMock.mock.calls.map(([path]) => path)).toEqual([
      "/api/rpc/getCodexGoal",
      "/api/rpc/saveCodexGoal",
      "/api/rpc/clearCodexGoal",
      "/api/rpc/pauseCodexGoal",
      "/api/rpc/resumeCodexGoal"
    ]);
    const save = rpcCall(fetchMock, 1);
    expect(save.options.method).toBe("POST");
    expect(save.options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(save.body).toEqual({
      thread_id: "thread-a",
      objective: "ship local goal",
      token_budget: 12345
    });
    for (const index of [2, 3, 4]) {
      const call = rpcCall(fetchMock, index);
      expect(call.options.method).toBe("POST");
      expect(call.options.headers.get("x-csrf-token")).toBe("csrf-token");
      expect(call.body).toEqual({ thread_id: "thread-a" });
    }
  });

  test("demo plugin list mirrors composer mention metadata", async () => {
    vi.resetModules();
    const { listPlugins } = await import("./api");

    const plugins = await listPlugins();

    expect(plugins).toEqual(expect.arrayContaining([
      expect.objectContaining({
        id: "codex",
        description: expect.stringContaining("本地线程"),
        invocation_template: "@Codex "
      }),
      expect.objectContaining({
        id: "claude_code",
        unavailable_reason: expect.stringContaining("只读"),
        invocation_template: "@Claude Code "
      })
    ]));
  });

  test("thread detail request supports pagination query parameters", async () => {
    const { getThread } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify({
      summary: { id: "thread-a", title: "wanka", status: "Recent", message_count: 1 },
      messages: [],
      blocks: [],
      raw_event_count: 1,
      total_blocks: 240,
      has_more_blocks: true,
      before_cursor: "b:120"
    }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    const detail = await getThread("thread-a", { limit: 120, before: "b:240", full: true });

    expect(rpcCall(fetchMock).path).toBe("/api/rpc/getThread");
    expect(rpcCall(fetchMock).body).toEqual({
      id: "thread-a",
      options: { limit: 120, before: "b:240", full: true }
    });
    expect(detail.total_blocks).toBe(240);
    expect(detail.before_cursor).toBe("b:120");
  });

  test("probe events request uses the dedicated endpoint and limit parameter", async () => {
    const { getProbeEvents } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify({
      events: [{ id: "event-1", kind: "hook-stop", source: "test", payload: {}, created_at: "2026-06-15T00:00:00Z" }],
      limit: 10
    }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    const result = await getProbeEvents(10);

    expect(rpcCall(fetchMock).path).toBe("/api/rpc/getProbeEvents");
    expect(rpcCall(fetchMock).body).toEqual({ limit: 10 });
    expect(result.available).toBe(true);
    expect(result.data?.events[0].kind).toBe("hook-stop");
  });

  test("demo probe events expose structured payload fields for the UI cards", async () => {
    vi.resetModules();
    const { getProbeEvents } = await import("./api");

    const result = await getProbeEvents(10);

    expect(result.available).toBe(true);
    expect(result.data?.events).toHaveLength(3);
    expect(result.data?.events[0]).toMatchObject({
      payload: expect.objectContaining({
        event_type: "reply-needed",
        thread_title: "Plan Mode 修复",
        reason_label: "等待用户确认",
        bark: expect.objectContaining({ sent: false, skipped: true, reason: "dedupe", http_status: 200, dedupe_hit: true }),
        dedupe: expect.objectContaining({ claimed: true, duplicate: false, status: "claimed" })
      })
    });
    expect(JSON.stringify(result.data)).not.toContain("secret");
  });

  test("demo probe and thread data do not expose raw proposed plan tags", async () => {
    vi.resetModules();
    const { getProbeStatus, listThreads } = await import("./api");

    const status = await getProbeStatus();
    const threads = await listThreads("reply-needed", "");

    expect(status.available).toBe(true);
    expect(JSON.stringify(status.data)).not.toContain("<proposed_plan>");
    expect(JSON.stringify(threads)).not.toContain("<proposed_plan>");
    expect(status.data?.reply_needed_threads?.[0]?.latest_message).toBe("等待确认");
    expect(threads[0]?.latest_message).toBe("等待确认");
  });

  test("demo jobs include probe job history entries", async () => {
    vi.resetModules();
    const { listJobs } = await import("./api");

    const jobs = await listJobs();
    const serialized = JSON.stringify(jobs);
    const retiredCodexJobTitle = ["Codex", "update", "precheck"].join(" ");
    const retiredClaudeJobTitle = ["Claude Code", "update"].join(" ");

    expect(jobs.some((job) => job.kind.startsWith("probe_"))).toBe(true);
    expect(jobs.some((job) => job.title.includes("Bark"))).toBe(true);
    expect(serialized).not.toContain(retiredCodexJobTitle);
    expect(serialized).not.toContain(retiredClaudeJobTitle);
  });

  test("thread block page request uses lightweight blocks endpoint", async () => {
    const { getThreadBlocks } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify({
      thread_id: "thread-a",
      blocks: [{ id: "b1", role: "assistant", kind: "message", text: "old", questions: [] }],
      total_blocks: 240,
      has_more_blocks: true,
      before_cursor: "b:120"
    }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    const page = await getThreadBlocks("thread-a", { limit: 80, before: "b:200" });

    expect(rpcCall(fetchMock).path).toBe("/api/rpc/getThreadBlocks");
    expect(rpcCall(fetchMock).body).toEqual({
      id: "thread-a",
      options: { limit: 80, before: "b:200" }
    });
    expect(page.thread_id).toBe("thread-a");
    expect(page.blocks[0].id).toBe("b1");
    expect(page.before_cursor).toBe("b:120");
  });

  test("desktop thread block page request keeps limit and cursor in the typed native command", async () => {
    const { getThreadBlocks } = await loadDesktopApi();
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, args) => {
      expect(command).toBe("desktop_thread_blocks");
      expect(args).toEqual({
        request: {
          id: "thread-a",
          limit: 80,
          before: "b:200"
        }
      });
      return {
        thread_id: "thread-a",
        blocks: [{ id: "b1", role: "assistant", kind: "message", text: "old", questions: [] }],
        total_blocks: 240,
        has_more_blocks: true,
        before_cursor: "b:120"
      };
    });

    const page = await getThreadBlocks("thread-a", { limit: 80, before: "b:200" });

    expect(fetchMock).not.toHaveBeenCalled();
    expect(page.before_cursor).toBe("b:120");
  });

  test("routes NexusHub updates to unified endpoints", async () => {
    const { getUpdateStatus, runUpdateAction } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify({ job_id: "panel-job" }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    fetchMock.mockResolvedValueOnce(new Response(JSON.stringify({
      current_version: "0.1.100",
      latest_version: "v0.1.103",
      update_available: true,
      channel: "stable",
      method: "linux_systemd_job",
      state: "idle",
      recommended_action: "/usr/local/bin/nexushub-update",
      capabilities: ["job_history"]
    }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    const status = await getUpdateStatus();
    const result = await runUpdateAction("install", "csrf-token");

    const statusCall = rpcCall(fetchMock, 0);
    const actionCall = rpcCall(fetchMock, 1);
    expect(status.method).toBe("linux_systemd_job");
    expect(result).toEqual({ job_id: "panel-job" });
    expect(statusCall.path).toBe("/api/rpc/getUpdateStatus");
    expect(actionCall.path).toBe("/api/rpc/runUpdateAction");
    expect(actionCall.options.method).toBe("POST");
    expect(actionCall.options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(actionCall.body).toEqual({ action: "install" });
  });

  test("does not export legacy update job helper or codex update targets", async () => {
    const api = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify({ job_id: "unexpected" }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    expect("startUpdateJob" in api).toBe(false);
    expect(apiSource).not.toContain("codex/update");
    expect(apiSource).not.toContain("/api/system/panel/update");

    expect(fetchMock).not.toHaveBeenCalled();
  });

  test("provider framework endpoints use the NexusHub read-only API surface", async () => {
    const {
      getClaudeCodeOverview,
      getPlatformOverview,
      getProbeSettings,
      getProbeStatus,
      listProviders
    } = await loadRealApi();
    const responses: Record<string, unknown> = {
      "/api/rpc/listProviders": [{ id: "codex", label: "Codex", status: "ready", capabilities: ["threads"] }],
      "/api/rpc/getClaudeCodeOverview": {
        home: "/Users/gosu/.claude",
        settings_exists: true,
        settings_preview: { apiKey: "[redacted]" },
        projects: [{ id: "-Users-gosu-demo", display_name: "/Users/gosu/demo", session_count: 1, sessions: [] }]
      },
      "/api/rpc/getPlatformOverview": {
        kind: "linux",
        data_dir: "/opt/nexushub",
        config_file: "/opt/nexushub/config.toml",
        webui_dir: "/opt/nexushub/webui",
        log_dir: "/opt/nexushub/logs",
        service_name: "nexushub",
        service_kind: "systemd"
      },
      "/api/rpc/getProbeStatus": {
        label: "Probe",
        enabled: true,
        platform: "linux",
        service_kind: "systemd",
        service_name: "nexushub",
        flavor: "builtin",
        available: true,
        hook_status: "managed",
        bark_status: "not_configured",
        logs_db_status: "maintenance_ready",
        recent_event_count: 0,
        running_count: 0,
        reply_needed_count: 0,
        recoverable_count: 0,
        running_threads: [],
        reply_needed_threads: [],
        recoverable_threads: [],
        config_path: "/opt/nexushub/config.toml"
      },
      "/api/rpc/getProbeSettings": {
        codex: {
          home: "/root/.codex",
          workspace: "/home/ubuntu/codex-workspace",
          host_label: "43.155.235.227"
        },
        probe: { enabled: true, poll_seconds: 15, recent_limit: 50 },
        notifications: { enabled: false, device_key_configured: false, server_url: "https://api.day.app" },
        logs_db: { enabled: true, retention_days: 14 }
      }
    };
    const fetchMock = vi.fn(async (path: RequestInfo | URL) => new Response(JSON.stringify(responses[String(path)]), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(listProviders()).resolves.toMatchObject([{ id: "codex", status: "ready" }]);
    await expect(getClaudeCodeOverview()).resolves.toMatchObject({ available: true, data: { settings_exists: true } });
    await expect(getPlatformOverview()).resolves.toMatchObject({ kind: "linux", data_dir: "/opt/nexushub" });
    await expect(getProbeStatus()).resolves.toMatchObject({ available: true, data: { hook_status: "managed", flavor: "builtin", service_name: "nexushub" } });
    await expect(getProbeSettings()).resolves.toMatchObject({ available: true, data: { probe: { poll_seconds: 15 } } });
    expect(fetchMock.mock.calls.map(([path]) => path)).toEqual([
      "/api/rpc/listProviders",
      "/api/rpc/getClaudeCodeOverview",
      "/api/rpc/getPlatformOverview",
      "/api/rpc/getProbeStatus",
      "/api/rpc/getProbeSettings"
    ]);
  });

  test("preview provider endpoints return unavailable when the backend has not enabled them", async () => {
    const { getClaudeCodeOverview, getProbeStatus } = await loadRealApi();
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({ error: "not found" }), {
      status: 404,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(getClaudeCodeOverview()).rejects.toMatchObject({ status: 404 });
    await expect(getProbeStatus()).rejects.toMatchObject({ status: 404 });
  });

  test("Probe demo data labels the builtin NexusHub service consistently", async () => {
    vi.unstubAllEnvs();
    vi.resetModules();
    const { getProbeStatus } = await import("./api");

    await expect(getProbeStatus()).resolves.toMatchObject({
      available: true,
      data: { flavor: "builtin", service_kind: "systemd", service_name: "nexushub" }
    });
  });

  test("desktop demo platform data reflects macOS Tauri instead of Linux WebUI", async () => {
    vi.unstubAllEnvs();
    vi.resetModules();
    globalThis.__NEXUSHUB_DESKTOP_RUNTIME__ = true;
    const { getPlatformOverview, getProbeStatus } = await import("./api");

    await expect(getPlatformOverview()).resolves.toMatchObject({
      kind: "macos",
      service_kind: "tauri",
      service_name: "NexusHub.app"
    });
    await expect(getProbeStatus()).resolves.toMatchObject({
      available: true,
      data: {
        platform: "macos",
        service_kind: "tauri",
        service_name: "NexusHub.app"
      }
    });
  });

  test("Claude Code demo overview includes read-only MCP install and cache summaries", async () => {
    vi.unstubAllEnvs();
    vi.resetModules();
    const { getClaudeCodeOverview } = await import("./api");

    const result = await getClaudeCodeOverview();

    expect(result).toMatchObject({
      available: true,
      data: {
        mcp: { server_count: 1 },
        installation: { settings_exists: true, version_hint: "demo" },
        cache_status: { cache_exists: true, log_exists: true }
      }
    });
    expect(result.data).not.toHaveProperty(["maintenance", "commands"].join("_"));
    expect(result.data?.recent_sessions?.[0]).toMatchObject({ id: "session-a", project_display_name: "/Users/gosu/demo" });
  });

  test("Probe fixed jobs use canonical API routes", async () => {
    const api = await loadRealApi() as Record<string, unknown>;
    const { getProbeLogsDbStatus, saveProbeSettings, startProbeJob } = api as typeof import("./api");
    expect(api.getProbeRunning).toBeUndefined();
    expect(api.getProbeReplyNeeded).toBeUndefined();
    expect(api.getProbeRecoverable).toBeUndefined();
    expect(api.getProbeDashboard).toBeUndefined();
    expect(api.planProbeAction).toBeUndefined();
    expect(api.executeProbePlan).toBeUndefined();

    const fetchMock = vi.fn(async (path: RequestInfo | URL, options?: RequestInit) => {
      const textPath = String(path);
      if (textPath.endsWith("/saveProbeSettings")) {
        return new Response(JSON.stringify({ saved: true }), {
          status: 200,
          headers: { "content-type": "application/json" }
        });
      }
      if (textPath.endsWith("/getProbeLogsDbStatus")) {
        return new Response(JSON.stringify({
          status: "maintenance_ready",
          path: "/root/.codex/logs_2.sqlite",
          old_rows: 12,
          retained_rows: 34,
          db_size_bytes: 4096,
          wal_size_bytes: 128,
          shm_size_bytes: 256,
          last_run_at: "2026-06-15T01:00:00Z",
          next_run_at: "2026-06-15T07:00:00Z",
          recent_result: "ok"
        }), {
          status: 200,
          headers: { "content-type": "application/json" }
        });
      }
      return new Response(JSON.stringify({ job_id: "bark-job-1" }), {
        status: 200,
        headers: { "content-type": "application/json" }
      });
    });
    vi.stubGlobal("fetch", fetchMock);

    await expect(getProbeLogsDbStatus()).resolves.toMatchObject({ available: true, data: { path: "/root/.codex/logs_2.sqlite", retained_rows: 34 } });
    await expect(startProbeJob("bark-test", "csrf-token")).resolves.toEqual({ job_id: "bark-job-1" });
    await expect(startProbeJob("hooks-install", "csrf-token")).resolves.toEqual({ job_id: "bark-job-1" });
    await expect(startProbeJob("logs-db-dry-run", "csrf-token")).resolves.toEqual({ job_id: "bark-job-1" });
    await expect(startProbeJob("logs-db-execute", "csrf-token")).resolves.toEqual({ job_id: "bark-job-1" });
    await saveProbeSettings({
      codex: { home: "/root/.codex", workspace: "/home/ubuntu/codex-workspace", host_label: "cloud" },
      probe: {
        poll_seconds: 20,
        notifications: { enabled: true, device_key: "secret" },
        logs_db: { enabled: true, retention_days: 2 }
      }
    }, "csrf-token");

    const calls = fetchMock.mock.calls.map(([path, options]) => [
      path,
      (options as RequestInit).method,
      ((options as RequestInit).headers as Headers).get("x-csrf-token"),
      (options as RequestInit & { body?: string }).body ? JSON.parse(String((options as RequestInit & { body?: string }).body)) : null
    ]);
    expect(calls).toEqual([
      ["/api/rpc/getProbeLogsDbStatus", "POST", null, {}],
      ["/api/rpc/startProbeJob", "POST", "csrf-token", { action: "bark-test" }],
      ["/api/rpc/startProbeJob", "POST", "csrf-token", { action: "hooks-install" }],
      ["/api/rpc/startProbeJob", "POST", "csrf-token", { action: "logs-db-dry-run" }],
      ["/api/rpc/startProbeJob", "POST", "csrf-token", { action: "logs-db-execute" }],
      ["/api/rpc/saveProbeSettings", "POST", "csrf-token", {
        settings: {
        codex: { home: "/root/.codex", workspace: "/home/ubuntu/codex-workspace", host_label: "cloud" },
        notifications: { device_key: "secret" },
        probe: {
          poll_seconds: 20,
          notifications: { enabled: true, device_key: "secret" },
          logs_db: { enabled: true, retention_days: 2 }
        }
        }
      }]
    ]);
  });

  test("Probe API accepts resolved Codex path discovery fields", async () => {
    const { getProbeLogsDbStatus, getProbeStatus } = await loadRealApi();
    const probeStatus: ProbeStatus = {
      label: "Probe",
      enabled: true,
      available: true,
      platform: "linux",
      service_kind: "systemd",
      service_name: "nexushub",
      flavor: "builtin",
      hook_status: "managed",
      bark_status: "configured",
      logs_db_status: "maintenance_ready",
      recent_event_count: 0,
      running_count: 0,
      reply_needed_count: 0,
      recoverable_count: 0,
      running_threads: [],
      reply_needed_threads: [],
      recoverable_threads: [],
      config_path: "/opt/nexushub/config.toml",
      codex_home: "/root/.codex",
      configured_codex_home: null,
      resolved_codex_home: "/home/codex/.codex",
      codex_home_source: "auto",
      logs_db_source: "resolved_codex_home",
      discovery_warnings: ["configured Codex home missing"]
    };
    const logsDbStatus: ProbeLogsDbStatus = {
      status: "maintenance_ready",
      logs_db_status: "maintenance_ready",
      target: "codex_logs_2",
      path: "/home/codex/.codex/logs_2.sqlite",
      configured_codex_home: null,
      resolved_codex_home: "/home/codex/.codex",
      codex_home_source: "auto",
      logs_db_source: "resolved_codex_home",
      discovery_warnings: ["configured Codex home missing"],
      retained_rows: 34
    };
    const responses: Record<string, unknown> = {
      "/api/rpc/getProbeStatus": probeStatus,
      "/api/rpc/getProbeLogsDbStatus": logsDbStatus
    };
    vi.stubGlobal("fetch", vi.fn(async (path: RequestInfo | URL) => new Response(JSON.stringify(responses[String(path)]), {
      status: 200,
      headers: { "content-type": "application/json" }
    })));

    await expect(getProbeStatus()).resolves.toMatchObject({
      available: true,
      data: {
        configured_codex_home: null,
        resolved_codex_home: "/home/codex/.codex",
        codex_home_source: "auto",
        logs_db_source: "resolved_codex_home",
        discovery_warnings: ["configured Codex home missing"]
      }
    });
    await expect(getProbeLogsDbStatus()).resolves.toMatchObject({
      available: true,
      data: {
        configured_codex_home: null,
        resolved_codex_home: "/home/codex/.codex",
        codex_home_source: "auto",
        path: "/home/codex/.codex/logs_2.sqlite",
        logs_db_source: "resolved_codex_home",
        discovery_warnings: ["configured Codex home missing"]
      }
    });
  });

  test("status path display helpers prefer resolved backend paths and source labels", async () => {
    const app = await import("../App");
    const status: SystemStatus = {
      host_label: "cloud",
      codex_home: "/root/.codex",
      configured_codex_home: null,
      resolved_codex_home: "/home/codex/.codex",
      codex_home_source: "auto",
      panel_db: "/opt/nexushub/nexushub.sqlite"
    };
    const logsDb: ProbeLogsDbStatus = {
      path: "/home/codex/.codex/logs_2.sqlite",
      logs_db_source: "resolved_codex_home"
    };

    expect(app.codexHomeStatusValue(status)).toBe("/home/codex/.codex · auto");
    expect(app.logsDbPathStatusValue(logsDb)).toBe("/home/codex/.codex/logs_2.sqlite · resolved_codex_home");
    expect(app.codexHomeStatusValue({ codex_home: "" })).toBe("未知");
  });

  test("runtime UI capabilities derive from core system capabilities", async () => {
    const { runtimeCapabilitiesFromSystemStatus, runtimeCapabilitiesForRuntime } = await loadRealApi();
    const linuxCore: SystemStatus["capabilities"] = {
      threads: true,
      jobs: true,
      probe: true,
      status: true,
      settings: true,
      job_history: true,
      app_updater: true,
      web_auth: true,
      security_settings: true,
      turnstile: true,
      systemd: true,
      nginx: true,
      public_endpoint: true,
      admin_password: true,
      linux_update_job: true,
      prune_backups: true
    };
    const macCore: SystemStatus["capabilities"] = {
      ...linuxCore,
      web_auth: false,
      security_settings: false,
      turnstile: false,
      systemd: false,
      nginx: false,
      public_endpoint: false,
      admin_password: false,
      linux_update_job: false,
      prune_backups: false
    };

    expect(runtimeCapabilitiesFromSystemStatus({ capabilities: linuxCore }, runtimeCapabilitiesForRuntime("web"))).toMatchObject({
      runtimeKind: "web",
      webAuth: true,
      securitySettings: true,
      publicEndpointStatus: true,
      linuxBackupPrune: true,
      linuxUpdateLabels: true,
      forkAction: true,
      approvalActions: true
    });
    expect(runtimeCapabilitiesFromSystemStatus({ capabilities: macCore }, runtimeCapabilitiesForRuntime("desktop"))).toMatchObject({
      runtimeKind: "desktop",
      webAuth: false,
      securitySettings: false,
      publicEndpointStatus: false,
      linuxBackupPrune: false,
      linuxUpdateLabels: false,
      forkAction: false,
      approvalActions: false
    });
  });

  test("saveProbeSettings sends the canonical probe payload plus Bark compatibility key", async () => {
    const { saveProbeSettings } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify({ saved: true }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    await saveProbeSettings({
      codex: { home: "/root/.codex", workspace: "/home/ubuntu/codex-workspace", host_label: "cloud" },
      probe: {
        enabled: true,
        notifications: { enabled: true, device_key: "secret", server_url: "https://api.day.app" },
        logs_db: { enabled: true, retention_days: 2 }
      }
    }, "csrf-token");

    const call = rpcCall(fetchMock);
    const body = call.body as Record<string, any>;
    expect(call.path).toBe("/api/rpc/saveProbeSettings");
    expect(call.options.method).toBe("POST");
    expect(call.options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(Object.keys(body).sort()).toEqual(["settings"]);
    expect(Object.keys(body.settings).sort()).toEqual(["codex", "notifications", "probe"]);
    expect(body.settings.probe.notifications).toEqual({
      enabled: true,
      device_key: "secret",
      server_url: "https://api.day.app"
    });
    expect(body.settings.notifications).toEqual({ device_key: "secret" });
    expect(body.settings.probe.logs_db).toEqual({ enabled: true, retention_days: 2 });
  });

  test("retired provider job helper is no longer exported", async () => {
    const api = await loadRealApi() as Record<string, unknown>;
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify({ job_id: "unexpected" }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    expect(api[["startClaude", "CodeJob"].join("")]).toBeUndefined();
    expect(api[["claudeCode", "JobRoutes"].join("")]).toBeUndefined();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  test("Job History labels read-only filesystem failures in Chinese", async () => {
    const app = await import("../App") as typeof import("../App") & {
      failureCategoryLabel?: (category: string) => string;
    };

    expect(app.failureCategoryLabel?.("read_only_file_system")).toBe("文件系统只读/安装目录不可写");
  });

  test("includes an optional Turnstile token in login payload", async () => {
    const { login } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify({
      id: "admin",
      username: "admin",
      csrf_token: "csrf"
    }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    await login("admin", "password", "turnstile-token");

    const call = rpcCall(fetchMock);
    expect(call.path).toBe("/api/rpc/login");
    expect(call.body).toEqual({
      username: "admin",
      password: "password",
      turnstile_token: "turnstile-token"
    });
  });

  test("login uses the scoped API base configured for the Linux /nexushub/ package", async () => {
    vi.stubEnv("BASE_URL", "/nexushub/");
    vi.stubEnv("VITE_API_BASE", "/nexushub");
    const { login } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify({
      id: "admin",
      username: "admin",
      csrf_token: "csrf"
    }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    await login("admin", "password");

    const [path] = fetchMock.mock.calls[0] as [string, RequestInit];
    expect(path).toBe("/nexushub/api/rpc/login");
  });

  test("desktop auth helpers never call Web auth endpoints", async () => {
    const { getPublicSettings, login, logout, me } = await loadDesktopApi();
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async () => null);

    expect(await getPublicSettings()).toMatchObject({
      site_name: "NexusHub",
      admin_configured: true,
      turnstile_enabled: false
    });
    expect(await me()).toMatchObject({
      username: "desktop",
      csrf_token: null
    });
    await login("admin", "password", "ignored-token");
    await logout("ignored-csrf");

    expect(fetchMock).not.toHaveBeenCalled();
    expect(globalThis.__NEXUSHUB_TEST_INVOKE__).not.toHaveBeenCalled();
  });

  test("desktop update helpers use macOS updater command instead of Linux panel routes", async () => {
    const { getUpdateStatus, runUpdateAction } = await loadDesktopApi();
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, _args) => {
      if (command === "install_update_and_restart") {
        return { job_id: "desktop-native-job", installed: false };
      }
      if (command === "check_update_status") {
        return {
          job_id: "desktop-check-job",
          status: {
            current_version: "0.1.100",
            latest_version: "v0.1.103",
            update_available: true,
            channel: "stable",
            method: "macos_tauri_updater",
            state: "ready",
            recommended_action: "Confirm install in the Tauri updater after signature verification.",
            capabilities: ["signature_verification", "job_history"]
          }
        };
      }
      expect(command).toBe("desktop_update_status");
      return {
        current_version: "0.1.100",
        latest_version: "v0.1.103",
        update_available: true,
        channel: "stable",
        method: "macos_tauri_updater",
        state: "idle",
        recommended_action: "Confirm install in the Tauri updater after signature verification.",
        capabilities: ["signature_verification", "job_history"]
      };
    });

    expect((await getUpdateStatus()).method).toBe("macos_tauri_updater");
    await expect(runUpdateAction("check", "ignored-csrf")).resolves.toEqual({
      job_id: "desktop-check-job",
      status: expect.objectContaining({
        latest_version: "v0.1.103",
        state: "ready"
      })
    });
    await expect(runUpdateAction("install", "ignored-csrf")).resolves.toEqual({ job_id: "desktop-native-job" });

    expect(fetchMock).not.toHaveBeenCalled();
    expect(globalThis.__NEXUSHUB_TEST_INVOKE__).toHaveBeenCalledTimes(3);
  });

  test("desktop archive cleanup uses a typed native command instead of a route bridge", async () => {
    const { startArchiveDelete } = await loadDesktopApi();
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, args) => {
      expect(command).toBe("desktop_archive_delete_execute");
      expect(args).toBeUndefined();
      return {
        before: {},
        deleted_threads: 0,
        after_total_threads: 1,
        after_archived_threads: 0,
        after_integrity: "ok"
      };
    });

    await expect(startArchiveDelete("ignored-csrf")).resolves.toMatchObject({ after_integrity: "ok" });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  test("desktop thread, probe, upload, job and goal helpers route through typed Tauri invoke commands", async () => {
    const {
      listThreads,
      getThread,
      dryRunArchiveDelete,
      startArchiveDelete,
      dryRunHiddenThreadDelete,
      startHiddenThreadDelete,
      getProbeSettings,
      saveProbeSettings,
      startProbeJob,
      uploadFiles,
      deleteUpload,
      listJobs,
      getJob,
      getCodexGoal,
      saveCodexGoal,
      clearCodexGoal,
      pauseCodexGoal,
      resumeCodexGoal
    } = await loadDesktopApi();
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, args) => {
      switch (command) {
        case "desktop_threads":
          expect(args).toEqual({ request: { status: "all", query: "needle", limit: 120 } });
          return [];
        case "desktop_thread_detail":
          expect(args).toEqual({ request: { id: "thread-a", limit: undefined, before: undefined, full: undefined } });
          return { summary: { id: "thread-a", title: "A", status: "Recent", message_count: 1 }, messages: [], blocks: [], raw_event_count: 0 };
        case "desktop_archive_delete_dry_run":
          expect(args).toBeUndefined();
          return { total_threads: 1, active_threads: 1, archived_threads: 0, session_index_lines: 1, rollout_files: 1, archived_ids: [], integrity: "ok" };
        case "desktop_archive_delete_execute":
          expect(args).toBeUndefined();
          return { deleted_threads: 0, before: {}, after_total_threads: 1, after_archived_threads: 0, after_integrity: "ok" };
        case "desktop_hidden_delete_dry_run":
          expect(args).toBeUndefined();
          return { total_threads: 1, visible_threads: 1, hidden_threads: 0, archived_threads: 0, session_index_lines: 1, rollout_files: 1, hidden_ids: [], hidden_source_counts: {}, integrity: "ok" };
        case "desktop_hidden_delete_execute":
          expect(args).toBeUndefined();
          return { deleted_threads: 0, before: {}, after_total_threads: 1, after_visible_threads: 1, after_hidden_threads: 0, after_integrity: "ok" };
        case "desktop_probe_settings":
          expect(args).toBeUndefined();
          return { probe: { enabled: true }, notifications: {}, logs_db: {} };
        case "desktop_probe_save_settings":
          expect(args).toMatchObject({ request: { probe: { enabled: false } } });
          return { probe: { enabled: false }, notifications: {}, logs_db: {} };
        case "desktop_probe_bark_test":
          expect(args).toBeUndefined();
          return { job_id: "probe-bark-job" };
        case "desktop_probe_logs_db_maintain":
          expect(args).toEqual({ request: { dryRun: true, compact: false } });
          return { job_id: "probe-logs-job" };
        case "desktop_delete_upload":
          expect(args).toEqual({ id: "upload-a" });
          return { ok: true, deleted: true };
        case "desktop_jobs":
          expect(args).toEqual({ request: { limit: 30 } });
          return [{ id: "job-a", kind: "probe", status: "succeeded", title: "Job A", started_at: 1 }];
        case "desktop_job_detail":
          expect(args).toEqual({ request: { id: "job-a" } });
          return { id: "job-a", kind: "probe", status: "succeeded", title: "Job A", started_at: 1 };
        case "desktop_home":
          expect(args).toBeUndefined();
          return { goal: { available: true, enabled: false, objective: null, token_budget: null, status: "idle" } };
        case "desktop_save_goal_command":
          expect(args).toEqual({ request: { threadId: "thread-a", objective: "ship", tokenBudget: 5000 } });
          return { available: true, enabled: true, thread_id: "thread-a", objective: "ship", token_budget: 5000, status: "active" };
        case "desktop_clear_goal_command":
          expect(args).toEqual({ threadId: "thread-a" });
          return { available: true, enabled: false, thread_id: "thread-a", objective: null, token_budget: null, status: "cleared" };
        case "desktop_pause_goal_command":
          expect(args).toEqual({ threadId: "thread-a" });
          return { available: true, enabled: true, thread_id: "thread-a", objective: "ship", token_budget: 5000, status: "paused" };
        case "desktop_resume_goal_command":
          expect(args).toEqual({ threadId: "thread-a" });
          return { available: true, enabled: true, thread_id: "thread-a", objective: "ship", token_budget: 5000, status: "active" };
        default:
          throw new Error(`unexpected command ${command}`);
      }
    });

    await listThreads("all", "needle");
    await getThread("thread-a");
    await dryRunArchiveDelete("ignored-csrf");
    await startArchiveDelete("ignored-csrf");
    await dryRunHiddenThreadDelete("ignored-csrf");
    await startHiddenThreadDelete("ignored-csrf");
    await getProbeSettings();
    await saveProbeSettings({ probe: { enabled: false } }, "ignored-csrf");
    await startProbeJob("bark-test", "ignored-csrf");
    await startProbeJob("logs-db-dry-run", "ignored-csrf");
    await deleteUpload("upload-a", "ignored-csrf");
    await listJobs();
    await getJob("job-a");
    await getCodexGoal("thread-a");
    await saveCodexGoal("thread-a", { objective: "ship", token_budget: 5000 }, "ignored-csrf");
    await clearCodexGoal("thread-a", "ignored-csrf");
    await pauseCodexGoal("thread-a", "ignored-csrf");
    await resumeCodexGoal("thread-a", "ignored-csrf");

    expect(fetchMock).not.toHaveBeenCalled();
    expect((globalThis.__NEXUSHUB_TEST_INVOKE__ as ReturnType<typeof vi.fn>).mock.calls).toEqual([
      ["desktop_threads", { request: { status: "all", query: "needle", limit: 120 } }],
      ["desktop_thread_detail", { request: { id: "thread-a", limit: undefined, before: undefined, full: undefined } }],
      ["desktop_archive_delete_dry_run", undefined],
      ["desktop_archive_delete_execute", undefined],
      ["desktop_hidden_delete_dry_run", undefined],
      ["desktop_hidden_delete_execute", undefined],
      ["desktop_probe_settings", undefined],
      ["desktop_probe_save_settings", expect.objectContaining({ request: expect.objectContaining({ probe: expect.objectContaining({ enabled: false }) }) })],
      ["desktop_probe_bark_test", undefined],
      ["desktop_probe_logs_db_maintain", { request: { dryRun: true, compact: false } }],
      ["desktop_delete_upload", { id: "upload-a" }],
      ["desktop_jobs", { request: { limit: 30 } }],
      ["desktop_job_detail", { request: { id: "job-a" } }],
      ["desktop_home", undefined],
      ["desktop_save_goal_command", { request: { threadId: "thread-a", objective: "ship", tokenBudget: 5000 } }],
      ["desktop_clear_goal_command", { threadId: "thread-a" }],
      ["desktop_pause_goal_command", { threadId: "thread-a" }],
      ["desktop_resume_goal_command", { threadId: "thread-a" }]
    ]);
    const file = new File(["# hello"], "note.md", { type: "text/markdown" });
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async (command, args) => {
      expect(command).toBe("desktop_upload_files_command");
      expect(args).toMatchObject({ files: [{ name: "note.md", mime: "text/markdown" }] });
      return { files: [{ id: "upload-a", name: "note.md", mime: "text/markdown", size: 7, sha256: "x", kind: "markdown", status: "ready" }] };
    });
    await uploadFiles([file], "ignored-csrf");
  });

  test("frontend production code does not reintroduce route bridges or component API passthroughs", () => {
    expect(apiSource).not.toContain("desktopApiRoute");
    expect(apiSource).not.toContain('runtimeRpc("desktopApi"');
    expect(apiSource).not.toContain("invokeDesktopApi");
    expect(apiSource).not.toContain("desktop_api_command");
    expect(apiSource).not.toContain("DesktopApiUpload");
    expect(apiSource).not.toContain('"/api/');
    expect(apiSource).not.toContain("'/api/");
    expect(apiSource).not.toContain("`/api/");
    expect(apiSource).not.toContain("new EventSource");
    expect(apiSource).not.toContain("/api/system/panel/update");
    expect(appSource).not.toContain("desktopApiRoute");
    expect(appSource).not.toContain('runtimeRpc("desktopApi"');
    expect(appSource).not.toContain("invoke(");
    expect(appSource).not.toContain('"/api/');
    expect(appSource).not.toContain("'/api/");
  });

  test("uses protocol endpoints for Plan and approval actions", async () => {
    const { acceptPlan, revisePlan, answerApproval } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify({
      bridge: true,
      thread_id: "thread-1",
      turn_id: "turn-2",
      fallback: false
    }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    await acceptPlan("thread-1", { turn_id: "turn-1", item_id: "plan-1" }, "csrf-token");
    await revisePlan("thread-1", { turn_id: "turn-1", item_id: "plan-1", instructions: "缩小范围" }, "csrf-token");
    await answerApproval("thread-1", { turn_id: "turn-1", item_id: "approval-1", decision: "approved" }, "csrf-token");

    expect(fetchMock.mock.calls.map(([path]) => path)).toEqual([
      "/api/rpc/acceptPlan",
      "/api/rpc/revisePlan",
      "/api/rpc/answerApproval"
    ]);
    expect(rpcCall(fetchMock, 0).body).toEqual({
      threadId: "thread-1",
      payload: {
        turn_id: "turn-1",
        item_id: "plan-1"
      }
    });
    expect(rpcCall(fetchMock, 1).body).toEqual({
      threadId: "thread-1",
      payload: {
        turn_id: "turn-1",
        item_id: "plan-1",
        instructions: "缩小范围"
      }
    });
    expect(rpcCall(fetchMock, 2).body).toEqual({
      threadId: "thread-1",
      payload: {
        turn_id: "turn-1",
        item_id: "approval-1",
        decision: "approved"
      }
    });
  });

  test("desktop unsupported Fork and Approval actions short-circuit before native bridge", async () => {
    const { forkThread, answerApproval } = await loadDesktopApi();
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    globalThis.__NEXUSHUB_TEST_INVOKE__ = vi.fn(async () => {
      throw new Error("desktop bridge should not be called");
    });

    await expect(forkThread("thread-a", "ignored-csrf")).rejects.toMatchObject({
      feature: "Desktop fork command is not implemented"
    });
    await expect(answerApproval("thread-a", { decision: "approved" }, "ignored-csrf")).rejects.toMatchObject({
      feature: "Desktop approval command is not implemented"
    });
    expect(fetchMock).not.toHaveBeenCalled();
    expect(globalThis.__NEXUSHUB_TEST_INVOKE__).not.toHaveBeenCalled();
  });

  test("follow-up API posts thread payload with csrf and supports listing and cancel", async () => {
    const { enqueueFollowUp, listFollowUps, cancelFollowUp } = await loadRealApi();
    const fetchMock = vi.fn(async (path: RequestInfo | URL, options?: RequestInit) => {
      if (String(path).endsWith("/cancelFollowUp")) {
        return new Response(JSON.stringify({ ok: true }), {
          status: 200,
          headers: { "content-type": "application/json" }
        });
      }
      if ((options?.method ?? "GET") === "POST") {
        return new Response(JSON.stringify({ id: "fu-1", thread_id: "thread-a", status: "pending", message: "继续检查" }), {
          status: 200,
          headers: { "content-type": "application/json" }
        });
      }
      return new Response(JSON.stringify({ items: [{ id: "fu-1", thread_id: "thread-a", status: "pending", message: "继续检查" }] }), {
        status: 200,
        headers: { "content-type": "application/json" }
      });
    });
    vi.stubGlobal("fetch", fetchMock);

    await enqueueFollowUp("thread-a", { message: "继续检查", model: "gpt-5.5" }, "csrf-token");
    await listFollowUps("thread-a");
    await cancelFollowUp("thread-a", "fu-1", "csrf-token");

    const post = rpcCall(fetchMock, 0);
    expect(post.path).toBe("/api/rpc/enqueueFollowUp");
    expect(post.options.method).toBe("POST");
    expect(post.options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(post.body).toEqual({
      threadId: "thread-a",
      payload: { message: "继续检查", model: "gpt-5.5" }
    });
    expect(rpcCall(fetchMock, 1).path).toBe("/api/rpc/listFollowUps");
    expect(rpcCall(fetchMock, 2).path).toBe("/api/rpc/cancelFollowUp");
    expect(rpcCall(fetchMock, 2).body).toEqual({ threadId: "thread-a", followUpId: "fu-1" });
  });

  test("steer API posts official follow-up endpoint with csrf and full run payload", async () => {
    const { steerThread } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify({
      bridge: true,
      thread_id: "thread-a",
      turn_id: "turn-live",
      fallback: false
    }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    const payload = {
      message: "继续检查",
      model: "gpt-5.5",
      service_tier: "priority",
      reasoning_effort: "xhigh",
      cwd: "/tmp/nexushub-workspace"
    };
    const result = await steerThread("thread-a", payload, "csrf-token");

    const call = rpcCall(fetchMock);
    expect(result).toMatchObject({ turn_id: "turn-live", fallback: false });
    expect(call.path).toBe("/api/rpc/steerThread");
    expect(call.options.method).toBe("POST");
    expect(call.options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(call.body).toEqual({ threadId: "thread-a", payload });
  });

  test("permission preset payloads match Codex-style compact choices", async () => {
    const app = await import("../App");

    expect(app.buildPayload("go", app.applyPermissionPreset(app.defaultRunConfig(), "ask"))).toMatchObject({
      approval_policy: "on-request",
      sandbox_mode: "workspace-write",
      network_access: true
    });
    expect(app.buildPayload("go", app.applyPermissionPreset(app.defaultRunConfig(), "auto"))).toMatchObject({
      approval_policy: "untrusted",
      sandbox_mode: "workspace-write",
      network_access: true
    });
    expect(app.buildPayload("go", app.applyPermissionPreset(app.defaultRunConfig(), "full"))).toMatchObject({
      approval_policy: "never",
      sandbox_mode: "danger-full-access",
      network_access: true
    });
    expect(app.buildPayload("go", app.applyPermissionPreset(app.defaultRunConfig(), "custom"))).toEqual({
      message: "go",
      model: "gpt-5.5",
      service_tier: null,
      reasoning_effort: "xhigh",
      cwd: "/home/ubuntu/codex-workspace",
      permission_profile: null,
      approval_policy: null,
      sandbox_mode: null,
      network_access: null,
      collaboration_mode: null
    });
    expect(app.buildPayload("", app.defaultRunConfig(), [{ id: "upload-1" }, { id: "upload-2" }])).toMatchObject({
      message: "",
      attachments: ["upload-1", "upload-2"]
    });
    expect(app.buildPayload("go", { ...app.defaultRunConfig(), collaborationMode: "plan" })).toMatchObject({
      message: "go",
      collaboration_mode: "plan"
    });
    expect(app.buildPayload("go", app.defaultRunConfig()).attachments).toBeUndefined();
  });

  test("fast mode payload and model service tier helpers follow official serviceTiers", async () => {
    const app = await import("../App");
    const models = [
      { id: "gpt-5.5", service_tiers: [{ id: "priority", name: "Fast" }], default_service_tier: "default" },
      { id: "gpt-5.4-mini", service_tiers: [] }
    ];

    expect(app.modelSupportsServiceTier(models, "gpt-5.5", "priority")).toBe(true);
    expect(app.modelSupportsServiceTier(models, "gpt-5.4-mini", "priority")).toBe(false);
    expect(app.buildPayload("go", { ...app.defaultRunConfig(), serviceTier: "priority" })).toMatchObject({
      service_tier: "priority"
    });
    expect(app.buildPayload("go", { ...app.defaultRunConfig(), serviceTier: "" })).toMatchObject({
      service_tier: null
    });
    expect(app.runConfigWithSupportedServiceTier(
      { ...app.defaultRunConfig(), model: "gpt-5.5", serviceTier: "priority" },
      models
    ).serviceTier).toBe("priority");
    expect(app.runConfigWithSupportedServiceTier(
      { ...app.defaultRunConfig(), model: "gpt-5.4-mini", serviceTier: "priority" },
      models
    ).serviceTier).toBe("");
    expect(app.runConfigWithSupportedServiceTier(
      { ...app.defaultRunConfig(), model: "gpt-5.5", serviceTier: "priority" },
      []
    ).serviceTier).toBe("");
  });

  test("slash commands omit removed Goal controls", async () => {
    const app = await import("../App");

    expect(app.slashCommands.map((item: { command: string }) => item.command)).not.toContain("/goal");
    expect(app.slashCommands.map((item: { command: string }) => item.command)).not.toContain("/goal clear");
    expect(app.slashCommandSuggestions("/goal r", 7)).toEqual([]);
    expect(app.slashCommands.some((item: { command: string }) => item.command.startsWith("/goal"))).toBe(false);
  });

  test("Plan Mode persists after successful send until explicit user change", async () => {
    const app = await import("../App");

    expect(app.runConfigAfterSuccessfulSend({ collaborationMode: "plan", model: "gpt-5.5" })).toEqual({
      collaborationMode: "plan",
      model: "gpt-5.5"
    });
    expect(app.runConfigAfterSuccessfulSend({ collaborationMode: "", model: "gpt-5.5" })).toEqual({
      collaborationMode: "",
      model: "gpt-5.5"
    });
  });

  test("bridge action copy hides fallback implementation wording", async () => {
    const app = await import("../App");
    const implementationCopy = ["started", "codex", "exec", "fallback", "job"].join(" ");

    expect(app.actionMessage({ bridge: false, fallback: true, job_id: "job-12345678", message: implementationCopy })).toBe("已提交给 Codex");
    expect(app.actionMessage({ bridge: false, fallback: true, message: "已提交给 Codex" })).toBe("已提交给 Codex");
    expect(app.actionMessage({ bridge: false, fallback: false, turn_id: "turn-12345678" })).toBe("已提交给 Codex");
    expect([
      app.actionMessage({ bridge: false, fallback: true, job_id: "job-12345678", message: implementationCopy }),
      app.actionMessage({ bridge: false, fallback: false, turn_id: "turn-12345678" })
    ].join(" ")).not.toContain(implementationCopy);
  });

  test("demo send helpers hide fallback transport wording", async () => {
    vi.resetModules();
    const { createThread, sendMessage, steerThread } = await import("./api");

    const results = await Promise.all([
      createThread({ message: "new" }),
      sendMessage("thread-a", { message: "next" }),
      steerThread("thread-a", { message: "follow up" })
    ]);
    const serialized = JSON.stringify(results);

    expect(serialized).not.toContain("controlled Codex job queued");
    expect(serialized).not.toContain("follow-up queued for the active Codex turn");
    expect(results.map((result) => result.message)).toEqual(["已提交给 Codex", "已提交给 Codex", "已提交给 Codex"]);
  });

  test("plugin mention helpers expose @ as the web plugin trigger instead of dollar", async () => {
    const app = await import("../App");
    const plugins = [
      { id: "probe", label: "Probe", status: "ready", kind: "builtin", description: "探针状态" },
      { id: "system-ops", label: "System/Ops", status: "ready", kind: "builtin", description: "固定运维动作" }
    ];

    expect(app.pluginMentionSuggestions("@", 1, plugins).map((item) => item.id)).toEqual(["probe", "system-ops"]);
    expect(app.pluginMentionSuggestions("$", 1, plugins)).toEqual([]);
    expect(app.activeComposerMenuKind("$p", 2, plugins)).toBeNull();
  });

  test("does not surface pending blocks without reply-needed state or active turn", async () => {
    const app = await import("../App");
    const oldChoice = {
      id: "choice-old",
      role: "assistant",
      kind: "request_user_input",
      turn_id: "turn-old",
      questions: [{ id: "q1", question: "旧选择", options: [{ label: "1" }] }]
    };
    const oldPlan = {
      id: "plan-old",
      role: "assistant",
      kind: "plan",
      turn_id: "turn-old",
      text: "<proposed_plan>旧计划</proposed_plan>",
      questions: []
    };

    expect(app.pendingFromBlocks([oldChoice], "Recent", null)).toBeNull();
    expect(app.latestActionBlock([oldPlan], "Recent", null, app.isPlanBlock)).toBeNull();
    expect(app.pendingFromBlocks([oldChoice], "ReplyNeeded", "turn-old")?.questions[0].question).toBe("旧选择");
    expect(app.latestActionBlock([oldPlan], "ReplyNeeded", "turn-old", app.isPlanBlock)?.id).toBe("plan-old");
    expect(app.pendingFromBlocks([oldChoice], "Running", "turn-old")?.questions[0].question).toBe("旧选择");
    expect(app.latestActionBlock([oldPlan], "Running", "turn-old", app.isPlanBlock)?.id).toBe("plan-old");
    expect(app.latestActionBlock([oldPlan], "ReplyNeeded", "turn-new", app.isPlanBlock)).toBeNull();
  });

  test("reply-needed fallback surfaces the latest unresolved plan when active turn is missing", async () => {
    const app = await import("../App");
    const plan = {
      id: "plan-live",
      role: "assistant",
      kind: "plan",
      item_id: "plan-item",
      status: "pending",
      resolved: false,
      text: "<proposed_plan>当前计划</proposed_plan>",
      questions: []
    } satisfies MessageBlock;

    const current = app.latestActionBlock([plan], "ReplyNeeded", null, app.isPlanBlock);

    expect(current?.id).toBe("plan-live");
    expect(app.isActionablePlanBlock(plan, current)).toBe(true);
    expect(app.currentActionKey(current, null)).toBe("plan:turn:plan-item");
    expect(app.renderCurrentActionCardSnapshot({ kind: "plan" }).buttons).toEqual([
      "接受计划",
      "修改计划",
      "保持计划模式"
    ]);
  });

  test("reply-needed fallback does not revive resolved plans or plans followed by execution progress", async () => {
    const app = await import("../App");
    const plan = {
      id: "plan-live",
      role: "assistant",
      kind: "plan",
      item_id: "plan-item",
      status: "pending",
      resolved: false,
      text: "<proposed_plan>当前计划</proposed_plan>",
      questions: []
    } satisfies MessageBlock;
    const resolvedPlan = {
      ...plan,
      id: "plan-resolved",
      status: "completed",
      resolved: true
    } satisfies MessageBlock;
    const assistantProgress = {
      id: "assistant-progress",
      role: "assistant",
      kind: "message",
      text: "开始执行计划",
      questions: []
    } satisfies MessageBlock;

    expect(app.latestActionBlock([resolvedPlan], "ReplyNeeded", null, app.isPlanBlock)).toBeNull();
    expect(app.latestActionBlock([plan, assistantProgress], "ReplyNeeded", null, app.isPlanBlock)).toBeNull();
  });

  test("summary pending elicitation must belong to the active turn", async () => {
    const app = await import("../App");
    const pending = {
      item_id: "choice-1",
      questions: [{ id: "q1", question: "选择方案", options: [{ label: "A" }] }]
    };

    expect(app.currentPendingElicitation(pending, "turn-live")).toBeNull();
    expect(app.currentPendingElicitation({ ...pending, turn_id: "turn-old" }, "turn-live")).toBeNull();
    expect(app.currentPendingElicitation({ ...pending, turn_id: "turn-live" }, "turn-live")).toMatchObject({
      turn_id: "turn-live",
      item_id: "choice-1"
    });
  });

  test("thread status tabs do not expose archived list entries", async () => {
    const app = await import("../App");

    expect(app.statusTabs.map((tab: { id: string }) => tab.id)).toEqual([
      "all",
      "running",
      "reply-needed",
      "recoverable"
    ]);
  });

  test("NexusHub navigation exposes the slim provider workspaces", async () => {
    const app = await import("../App");

    expect(app.navigationItems.map((item: { id: string }) => item.id)).toEqual([
      "codex",
      "claude",
      "probe",
      "ops",
      "security"
    ]);
    expect(app.navigationItems.map((item: { label: string }) => item.label)).toEqual([
      "Codex",
      "Claude Code",
      "探针",
      "运维",
      "安全"
    ]);
  });

  test("thread list item text only exposes the title", async () => {
    const app = await import("../App");

    expect(app.threadListItemText({
      id: "thread-a",
      title: "wanka",
      status: "Running",
      latest_message: "接手这个线程的工作 019e86d2...",
      model: "custom",
      last_event_kind: "app-server.thread/list",
      message_count: 8359
    })).toBe("wanka");
  });

  test("thread list metadata shows status and preview without contaminating title", async () => {
    const app = await import("../App");
    const thread = {
      id: "thread-a",
      title: "wanka",
      status: "Running",
      latest_message: "正在执行长任务输出",
      active_turn_id: "turn-1",
      message_count: 24
    } satisfies Partial<ThreadSummary>;

    expect(app.threadListItemStatusText(thread)).toBe("运行中");
    expect(app.threadListItemPreviewText(thread)).toBe("正在执行长任务输出");
    expect(app.threadListItemText(thread)).toBe("wanka");
    expect(app.threadListItemPreviewText({
      ...thread,
      latest_message: "<proposed_plan>\n1. 检查\n2. 修复\n</proposed_plan>"
    })).toBe("1. 检查 2. 修复");
    expect(app.threadListItemPreviewText({ ...thread, latest_message: "" })).toBe("");
    expect(app.isThreadListItemRunning(thread)).toBe(true);
    expect(app.isThreadListItemRunning({ ...thread, status: "Recent", active_turn_id: undefined })).toBe(false);
  });

  test("demo thread list does not expose cwd or runtime workspace paths", async () => {
    vi.resetModules();
    const { listThreads } = await import("./api");

    const threads = await listThreads("all", "");
    const serialized = JSON.stringify(threads);

    expect(threads.every((thread) => !("cwd" in thread))).toBe(true);
    expect(serialized).not.toContain("/srv/hermes");
    expect(serialized).not.toContain("/root/.codex");
    expect(serialized).not.toContain("/home/ubuntu/codex-workspace");
  });

  test("thread list cache helper inserts and removes rows for running filter", async () => {
    const app = await import("../App");
    const existing = {
      id: "thread-a",
      title: "wanka",
      status: "Recent",
      latest_message: "old",
      message_count: 1
    } satisfies ThreadSummary;
    const running = {
      ...existing,
      title: "未命名线程",
      status: "Running",
      active_turn_id: "turn-live",
      latest_message: "working"
    } satisfies ThreadSummary;
    const completed = {
      ...existing,
      status: "Recent",
      active_turn_id: null,
      latest_message: "done"
    } satisfies ThreadSummary;

    expect(app.mergeThreadSummaryIntoListCache([], running, "running", "")).toEqual([
      expect.objectContaining({ id: "thread-a", status: "Running", active_turn_id: "turn-live" })
    ]);
    expect(app.mergeThreadSummaryIntoListCache([existing], running, "all", "")).toEqual([
      expect.objectContaining({ id: "thread-a", title: "wanka", latest_message: "working" })
    ]);
    expect(app.mergeThreadSummaryIntoListCache([running], completed, "running", "")).toEqual([]);
    expect(app.mergeThreadSummaryIntoListCache([existing], running, "running", "xianbao")).toEqual([]);
  });

  test("thread list visibility filters archived and every known subagent marker", async () => {
    const app = await import("../App");
    const rows = [
      { id: "main", title: "wanka", status: "Running", message_count: 1 },
      { id: "archived", title: "old", status: "Archived", message_count: 1 },
      { id: "parent", title: "child", status: "Recent", message_count: 1, parentThreadId: "main" },
      { id: "source-kind", title: "child", status: "Recent", message_count: 1, sourceKind: "subAgentRun" },
      { id: "thread-source", title: "child", status: "Recent", message_count: 1, thread_source: "subagent" },
      { id: "source-json", title: "child", status: "Recent", message_count: 1, source: { subagent: { thread_spawn: { parentThreadId: "main" } } } },
      { id: "agent-path", title: "child", status: "Recent", message_count: 1, agentPath: "/tmp/subagent" },
      { id: "agent-nickname", title: "child", status: "Recent", message_count: 1, agentNickname: "reviewer" },
      { id: "agent-role", title: "child", status: "Recent", message_count: 1, agentRole: "explorer" },
      {
        id: "internal-exec",
        title: "只读验证任务。不要修改文件。",
        status: "Recent",
        message_count: 1,
        source: "exec",
        thread_source: "user",
        has_user_event: 0,
        first_user_message: "只读验证任务。不要修改文件。使用 tool_search 查询 spawn_agent。"
      },
      {
        id: "internal-subagent-prompt",
        title: "你是子代理 A，必须使用 gpt-5.5 和 xhigh。",
        status: "Recent",
        message_count: 1,
        source: "exec",
        thread_source: "user",
        has_user_event: 0
      }
    ] satisfies Array<Partial<ThreadSummary>>;

    expect(app.filterVisibleThreadSummaries(rows).map((thread) => thread.id)).toEqual(["main"]);
  });

  test("conversation title text ignores latest message previews", async () => {
    const app = await import("../App");

    expect(app.conversationTitleText({
      title: " wanka ",
      latest_message: "接手这个线程的工作 019e86d2...",
      model: "custom",
      last_event_kind: "app-server.thread/read"
    })).toBe("wanka");
    expect(app.conversationTitleText({
      title: "   ",
      latest_message: "接手这个线程的工作 019e86d2..."
    })).toBe("未命名线程");
  });

  test("incoming realtime summary keeps an existing title when the update has only a placeholder", async () => {
    const app = await import("../App");
    const current = {
      id: "thread-a",
      title: "wanka",
      status: "Recent",
      message_count: 8,
      latest_message: "旧摘要",
      last_event_kind: "task_complete"
    } satisfies Partial<ThreadSummary>;
    const incoming = {
      id: "thread-a",
      title: "未命名线程",
      status: "Running",
      message_count: 9,
      latest_message: "新摘要",
      active_turn_id: "turn-live",
      last_event_kind: "app-server.thread/list"
    } satisfies Partial<ThreadSummary>;

    expect(app.mergeIncomingThreadSummary(current, incoming)).toMatchObject({
      title: "wanka",
      status: "Running",
      latest_message: "新摘要",
      active_turn_id: "turn-live",
      last_event_kind: "task_complete"
    });
    expect(app.lastEventKindText({ last_event_kind: "app-server.thread/read" })).toBe("未知");
    expect(app.lastEventKindText({ last_event_kind: "panel.job.running" })).toBe("未知");
    expect(app.lastEventKindText({ last_event_kind: "task_complete" })).toBe("task_complete");
  });

  test("thread detail summary merges fresh list status without losing title", async () => {
    const app = await import("../App");
    const detail = {
      summary: {
        id: "thread-a",
        title: "wanka",
        status: "Recent",
        message_count: 8,
        latest_message: "旧摘要",
        active_turn_id: null
      },
      messages: [],
      blocks: [],
      raw_event_count: 20
    } satisfies ThreadDetail;
    const merged = app.mergeThreadDetailSummaryFromList(detail, {
      id: "thread-a",
      title: "未命名线程",
      status: "Running",
      message_count: 9,
      latest_message: "正在执行",
      active_turn_id: "turn-live",
      last_event_kind: "app-server.thread/list"
    });

    expect(merged).not.toBe(detail);
    expect(merged.summary).toMatchObject({
      title: "wanka",
      status: "Running",
      latest_message: "正在执行",
      active_turn_id: "turn-live"
    });
  });

  test("thread detail refetch keeps polling idle selections and speeds up running threads", async () => {
    const app = await import("../App");
    const idleDetail = {
      summary: {
        id: "thread-a",
        title: "wanka",
        status: "Recent",
        message_count: 8
      },
      messages: [],
      blocks: [],
      raw_event_count: 20
    } satisfies ThreadDetail;

    expect(app.threadDetailRefetchInterval(idleDetail, null)).toBe(5000);
    expect(app.threadDetailRefetchInterval(idleDetail, { status: "Running" })).toBe(5000);
    expect(app.threadDetailRefetchInterval(undefined, { status: "Running" })).toBe(2000);
    expect(app.threadDetailRefetchInterval({
      ...idleDetail,
      summary: { ...idleDetail.summary, status: "Running" }
    }, null)).toBe(2000);
  });

  test("tool block helpers expose compact title, summary, and detail text", async () => {
    const app = await import("../App");
    const block = {
      id: "tool-1",
      role: "tool",
      kind: "function_call_output",
      status: "completed",
      tool_name: "exec_command",
      call_id: "call-1",
      summary: "Output: ok",
      input: "{\n  \"cmd\": \"pwd\"\n}",
      text: "Output:\n/home/ubuntu",
      truncated: true,
      questions: []
    };

    expect(app.isToolBlock(block)).toBe(true);
    expect(app.toolBlockTitle(block)).toBe("exec_command");
    expect(app.toolBlockSummary(block)).toBe("Output: ok");
    expect(app.toolBlockDetailText(block)).toContain("\"cmd\": \"pwd\"");
    expect(app.toolBlockDetailText(block)).toContain("Output:\n/home/ubuntu");
    expect(app.toolBlockDetailText(block)).toContain("[output truncated]");
    expect(app.toolBlockTitle({
      id: "chat-history-collapsed",
      role: "tool",
      kind: "chat_history_collapsed",
      status: "completed",
      tool_name: "chat_history",
      summary: "4078 条历史对话已折叠",
      questions: []
    })).toBe("4078 条历史对话已折叠");
  });

  test("upsert message block appends, replaces changed blocks, and keeps identical references", async () => {
    const app = await import("../App");
    const current = [{
      id: "tool-1",
      role: "tool",
      kind: "function_call",
      status: "running",
      tool_name: "exec_command",
      questions: []
    }] satisfies MessageBlock[];
    const completed = {
      ...current[0],
      kind: "function_call_output",
      status: "completed",
      text: "Output:\n/tmp"
    } satisfies MessageBlock;

    const unchanged = app.upsertMessageBlock(current, current[0]);
    expect(unchanged).toBe(current);

    const replaced = app.upsertMessageBlock(current, completed);
    expect(replaced).not.toBe(current);
    expect(replaced).toHaveLength(1);
    expect(replaced[0]).toEqual(completed);

    const appended = app.upsertMessageBlock(replaced, {
      id: "assistant-1",
      role: "assistant",
      kind: "message",
      text: "done",
      questions: []
    });
    expect(appended).toHaveLength(2);
    expect(appended[1].id).toBe("assistant-1");
  });

  test("merge message blocks appends, prepends, updates, and preserves unchanged references", async () => {
    const app = await import("../App");
    const current = [
      { id: "b2", role: "assistant", kind: "message", text: "middle", questions: [] },
      { id: "b3", role: "assistant", kind: "message", text: "old", questions: [] }
    ] satisfies MessageBlock[];

    expect(app.mergeMessageBlocks(current, [])).toBe(current);
    expect(app.mergeMessageBlocks(current, [{ ...current[0] }])).toBe(current);

    const appended = app.mergeMessageBlocks(current, [
      { id: "b3", role: "assistant", kind: "message", text: "updated", questions: [] },
      { id: "b4", role: "assistant", kind: "message", text: "new", questions: [] }
    ]);
    expect(appended).not.toBe(current);
    expect(appended.map((block: MessageBlock) => `${block.id}:${block.text}`)).toEqual([
      "b2:middle",
      "b3:updated",
      "b4:new"
    ]);

    const prepended = app.mergeMessageBlocks(appended, [
      { id: "b0", role: "user", kind: "message", text: "older", questions: [] },
      { id: "b1", role: "assistant", kind: "message", text: "old answer", questions: [] },
      { id: "b2", role: "assistant", kind: "message", text: "middle", questions: [] }
    ], "prepend");
    expect(prepended.map((block: MessageBlock) => block.id)).toEqual(["b0", "b1", "b2", "b3", "b4"]);
  });

  test("running and composer action helpers switch between send stop and follow-up", async () => {
    const app = await import("../App");
    const runningTool = {
      id: "tool-1",
      role: "tool",
      kind: "function_call",
      status: "running",
      questions: []
    } satisfies MessageBlock;
    const completedTool = {
      ...runningTool,
      status: "completed"
    } satisfies MessageBlock;

    expect(app.isThreadRunning({ status: "Running" }, [], null)).toBe(true);
    expect(app.isThreadRunning({ status: "Recent", active_job_id: "job-1" }, [], null)).toBe(true);
    expect(app.isThreadRunning({ status: "ReplyNeeded", active_turn_id: "turn-1" }, [], null)).toBe(false);
    expect(app.isThreadListItemRunning({ status: "Recoverable", active_turn_id: "turn-1" })).toBe(false);
    expect(app.isThreadRunning({ status: "Recent" }, [runningTool], null)).toBe(false);
    expect(app.isThreadRunning({ status: "Running", active_turn_id: "turn-1" }, [runningTool], null)).toBe(true);
    expect(app.isThreadRunning({ status: "Recent" }, [completedTool], null)).toBe(false);
    expect(app.isThreadRunning({ status: "Recent" }, [], { bridge: false, fallback: true, job_id: "job-old" })).toBe(false);
    expect(app.isThreadRunning({ status: "Running" }, [], { bridge: false, fallback: true, job_id: "job-live" })).toBe(true);
    expect(app.composerActionMode(false, "hello", false)).toBe("send");
    expect(app.composerActionMode(true, "", true)).toBe("stop");
    expect(app.composerActionMode(true, "continue", true)).toBe("followup");
    expect(app.composerActionMode(false, "", false, 1)).toBe("send");
    expect(app.composerActionMode(true, "", true, 1)).toBe("followup");
    expect(app.composerActionMode(false, "", false)).toBe("disabled");
    expect(app.readyComposerUploads([
      { id: "ready", name: "a.md", mime: "text/markdown", size: 1, sha256: "sha", kind: "markdown", status: "ready" },
      { id: "error", name: "b.zip", mime: "application/zip", size: 1, sha256: "", kind: "text", status: "error", local_status: "error" }
    ])).toHaveLength(1);
    expect(app.composerUploadIds([
      { id: "ready", name: "a.md", mime: "text/markdown", size: 1, sha256: "sha", kind: "markdown", status: "ready" }
    ])).toEqual(["ready"]);
    expect(app.uploadKindLabel("spreadsheet")).toBe("表格");
    expect(app.uploadStatusText({ status: "error", local_status: "error", error_preview: "bad", local_error: null })).toBe("bad");
    expect(app.formatFileSize(1536)).toBe("1.5 KiB");
    expect(app.composerActionLabel("followup")).toBe("跟进");
    expect(app.composerActionTitle("followup")).toContain("跟进队列");
  });

  test("conversation block compaction collapses old completed tools but keeps running tools", async () => {
    const app = await import("../App");
    const completed = Array.from({ length: 5 }, (_, index) => ({
      id: `tool-${index}`,
      role: "tool",
      kind: "function_call_output",
      status: "completed",
      text: `done-${index}`,
      questions: []
    })) satisfies MessageBlock[];
    const running = {
      id: "tool-live",
      role: "tool",
      kind: "function_call",
      status: "running",
      text: "running",
      questions: []
    } satisfies MessageBlock;

    const compacted = app.compactConversationBlocks([...completed, running], 2);

    expect(compacted.map((block) => block.id)).toEqual([
      "completed-tool-history-collapsed",
      "tool-3",
      "tool-4",
      "tool-live"
    ]);
    expect(compacted[0].summary).toBe("3 个历史工具调用已折叠");
    expect(compacted[compacted.length - 1]).toBe(running);
  });

  test("conversation block compaction defaults to compact completed tool history", async () => {
    const app = await import("../App");
    const completed = Array.from({ length: 18 }, (_, index) => ({
      id: `tool-${index}`,
      role: "tool",
      kind: "function_call_output",
      status: "completed",
      text: `done-${index}`,
      questions: []
    })) satisfies MessageBlock[];

    const compacted = app.compactConversationBlocks(completed);

    expect(compacted).toHaveLength(5);
    expect(compacted[0].id).toBe("completed-tool-history-collapsed");
    expect(compacted[0].summary).toBe("14 个历史工具调用已折叠");
    expect(compacted[compacted.length - 1]?.id).toBe("tool-17");
  });

  test("conversation message presentation uses light chat rows", async () => {
    const app = await import("../App");

    expect(app.conversationMessagePresentation({ role: "user" })).toEqual({
      kind: "user",
      rowClassName: "chat-row user",
      bodyClassName: "chat-bubble"
    });
    expect(app.conversationMessagePresentation({ role: "assistant" })).toEqual({
      kind: "assistant",
      rowClassName: "chat-row assistant",
      bodyClassName: "assistant-message-body"
    });
  });

  test("plan and question blocks render in the conversation stream but not the action stack", async () => {
    const app = await import("../App");
    const plan = {
      id: "plan-1",
      role: "assistant",
      kind: "plan",
      display_kind: "plan",
      turn_id: "turn-live",
      item_id: "plan-item",
      text: "<proposed_plan>先检查，再实现。</proposed_plan>",
      status: "pending",
      resolved: false,
      questions: []
    } satisfies MessageBlock;
    const question = {
      id: "question-1",
      role: "assistant",
      kind: "request_user_input",
      display_kind: "question",
      turn_id: "turn-live",
      call_id: "call-1",
      status: "pending",
      resolved: false,
      questions: [{ id: "q1", question: "选择方案", options: [{ label: "A" }, { label: "B" }] }]
    } satisfies MessageBlock;
    const approval = {
      id: "approval-1",
      role: "assistant",
      kind: "approval",
      display_kind: "approval",
      turn_id: "turn-live",
      text: "Allow command?",
      questions: []
    } satisfies MessageBlock;

    expect(app.shouldRenderConversationBlock(plan)).toBe(true);
    expect(app.shouldRenderConversationBlock(question)).toBe(true);
    expect(app.shouldRenderConversationBlock(approval)).toBe(false);
    expect(app.shouldRenderActionStackBlock(plan)).toBe(false);
    expect(app.shouldRenderActionStackBlock(question)).toBe(false);
    expect(app.shouldRenderActionStackBlock(approval)).toBe(true);
    expect(app.compactConversationBlocks([plan, question])).toEqual([plan, question]);
  });

  test("only unresolved active-turn plans and questions expose stream actions", async () => {
    const app = await import("../App");
    const currentPlan = {
      id: "plan-live",
      role: "assistant",
      kind: "plan",
      turn_id: "turn-live",
      item_id: "plan-item",
      status: "pending",
      resolved: false,
      text: "<proposed_plan>当前计划</proposed_plan>",
      questions: []
    } satisfies MessageBlock;
    const historyPlan = {
      ...currentPlan,
      id: "plan-old",
      turn_id: "turn-old",
      status: "completed",
      resolved: true,
      text: "<proposed_plan>旧计划</proposed_plan>"
    } satisfies MessageBlock;
    const answeredQuestion = {
      id: "question-old",
      role: "assistant",
      kind: "request_user_input",
      turn_id: "turn-old",
      call_id: "call-old",
      status: "completed",
      resolved: true,
      questions: [{ id: "q1", question: "选择方案", options: [{ label: "A" }, { label: "B" }] }],
      answers: [{ question_id: "q1", answers: ["B"], note: null }]
    } satisfies MessageBlock;
    const currentQuestion = {
      ...answeredQuestion,
      id: "question-live",
      turn_id: "turn-live",
      call_id: "call-live",
      status: "pending",
      resolved: false,
      answers: []
    } satisfies MessageBlock;

    expect(app.isActionablePlanBlock(currentPlan, currentPlan)).toBe(true);
    expect(app.isActionablePlanBlock(historyPlan, currentPlan)).toBe(false);
    expect(app.isActionableQuestionBlock(currentQuestion, currentQuestion)).toBe(true);
    expect(app.isActionableQuestionBlock(answeredQuestion, currentQuestion)).toBe(false);
    expect(app.questionAnswerLabels(answeredQuestion, "q1")).toEqual(["B"]);
    expect(app.isResolvedActionBlock(answeredQuestion)).toBe(true);
    expect(app.pendingFromBlocks([answeredQuestion], "ReplyNeeded", "turn-old")).toBeNull();

    expect(app.prioritizeCurrentActionBlocks(
      [currentPlan, answeredQuestion, currentQuestion],
      currentPlan,
      currentQuestion
    ).map((block) => block.id)).toEqual([
      "question-old",
      "plan-live",
      "question-live"
    ]);
  });

  test("current action card helper keys hide locally until the server action changes", async () => {
    const app = await import("../App");
    const plan = {
      id: "plan-live",
      role: "assistant",
      kind: "plan",
      turn_id: "turn-live",
      item_id: "plan-item",
      status: "pending",
      resolved: false,
      text: "<proposed_plan>当前计划</proposed_plan>",
      questions: []
    } satisfies MessageBlock;
    const pending = {
      turn_id: "turn-live",
      item_id: "question-item",
      questions: [{ id: "q1", question: "选择方案", options: [{ label: "A" }, { label: "B" }] }]
    };

    expect(app.currentActionKey(plan, null)).toBe("plan:turn-live:plan-item");
    expect(app.currentActionKey(null, pending)).toBe("question:turn-live:question-item");
    expect(app.shouldShowCurrentActionCard("plan:turn-live:plan-item", null)).toBe(true);
    expect(app.shouldShowCurrentActionCard("plan:turn-live:plan-item", "plan:turn-live:plan-item")).toBe(false);
    expect(app.shouldShowCurrentActionCard("question:turn-live:question-item", "plan:turn-live:plan-item")).toBe(true);
  });

  test("current action keyboard helpers select digits and move within bounds", async () => {
    const app = await import("../App");

    expect(app.selectionFromDigitKey("1", 2)).toBe(0);
    expect(app.selectionFromDigitKey("9", 2)).toBeNull();
    expect(app.moveActionSelection(0, 2, 1)).toBe(1);
    expect(app.moveActionSelection(1, 2, 1)).toBe(0);
    expect(app.moveActionSelection(0, 2, -1)).toBe(1);
  });

  test("current plan action helpers map accept and revise choices", async () => {
    const app = await import("../App");

    expect(app.currentPlanActionOptions().map((option) => option.label)).toEqual([
      "接受计划",
      "修改计划",
      "保持计划模式"
    ]);
    expect(app.renderCurrentActionCardSnapshot({ kind: "plan" })).toMatchObject({
      buttons: ["接受计划", "修改计划", "保持计划模式"],
      supplementalInput: false
    });
    expect(app.planActionSubmission(0, "")).toEqual({ action: "accept" });
    expect(app.planActionSubmission(1, "")).toBeNull();
    expect(app.planActionSubmission(1, "  增加验收步骤  ")).toEqual({
      action: "revise",
      instructions: "增加验收步骤"
    });
    expect(app.planActionSubmission(2, "")).toEqual({ action: "keep_plan" });
  });

  test("question and hidden cleanup helpers expose readiness and disabled state", async () => {
    const app = await import("../App");
    const questions = [
      { id: "q1", question: "选择方案", options: [{ label: "A" }, { label: "B" }] },
      { id: "q2", question: "选择范围", options: [{ label: "小" }, { label: "大" }] }
    ];
    const hiddenPlan = {
      total_threads: 9,
      visible_threads: 7,
      hidden_threads: 2,
      archived_threads: 0,
      session_index_lines: 9,
      rollout_files: 9,
      hidden_ids: ["child-a", "child-b"],
      hidden_source_counts: { exec: 1, subagent: 1 },
      integrity: "ok"
    };

    expect(app.questionAnswersReady(questions, { q1: "A" })).toBe(false);
    expect(app.questionAnswersReady(questions, { q1: "A", q2: "小" })).toBe(true);
    expect(app.questionAnswerPayload(questions, { q1: "A", q2: ["大"] })).toEqual({
      q1: ["A"],
      q2: ["大"]
    });
    expect(app.renderCurrentActionCardSnapshot({ kind: "question", questions })).toMatchObject({
      buttons: ["A", "B", "小", "大"],
      supplementalInput: true
    });
    expect(app.hiddenThreadDeleteStats(hiddenPlan)).toEqual({
      hidden: 2,
      visible: 7,
      sourceCounts: "exec:1 subagent:1",
      integrity: "ok"
    });
    expect(app.canStartHiddenThreadDelete(hiddenPlan)).toBe(true);
    expect(app.canStartHiddenThreadDelete({ ...hiddenPlan, hidden_threads: 0 })).toBe(false);
    expect(app.canStartHiddenThreadDelete(null)).toBe(false);
  });

  test("conversation block compaction collapses old chat messages but keeps recent chat and running tools", async () => {
    const app = await import("../App");
    const chat = Array.from({ length: 6 }, (_, index) => ({
      id: `chat-${index}`,
      role: index % 2 === 0 ? "user" : "assistant",
      kind: "message",
      text: `message-${index}`,
      questions: []
    })) satisfies MessageBlock[];
    const running = {
      id: "tool-live",
      role: "tool",
      kind: "function_call",
      status: "running",
      text: "running",
      questions: []
    } satisfies MessageBlock;

    const compacted = app.compactConversationBlocks([...chat, running], 80, 2);

    expect(compacted.map((block) => block.id)).toEqual([
      "chat-history-collapsed",
      "chat-4",
      "chat-5",
      "tool-live"
    ]);
    expect(compacted[0].summary).toBe("4 条历史对话已折叠");
    expect(compacted[compacted.length - 1]).toBe(running);
  });

  test("conversation block compaction collapses historical plans while preserving the current plan", async () => {
    const app = await import("../App");
    const historicalPlans = Array.from({ length: 8 }, (_, index) => ({
      id: `plan-${index}`,
      role: "assistant",
      kind: "plan",
      display_kind: "plan",
      turn_id: `turn-${index}`,
      item_id: `plan-item-${index}`,
      status: "pending",
      resolved: false,
      text: `<proposed_plan>历史计划 ${index}</proposed_plan>`,
      questions: []
    })) satisfies MessageBlock[];
    const currentPlan = {
      id: "plan-current",
      role: "assistant",
      kind: "plan",
      display_kind: "plan",
      turn_id: "turn-current",
      item_id: "plan-item-current",
      status: "pending",
      resolved: false,
      text: "<proposed_plan>当前计划</proposed_plan>",
      questions: []
    } satisfies MessageBlock;

    const compacted = app.visibleConversationBlocksForHistory(
      [...historicalPlans, currentPlan],
      false,
      currentPlan,
      null
    );
    const prioritized = app.prioritizeCurrentActionBlocks(compacted, currentPlan, null);

    expect(prioritized.map((block) => block.id)).toEqual([
      "action-history-collapsed",
      "plan-5",
      "plan-6",
      "plan-7",
      "plan-current"
    ]);
    expect(prioritized[0].summary).toBe("5 条历史计划/问题已折叠");
    expect(app.visibleConversationBlocksForHistory([...historicalPlans, currentPlan], true)).toHaveLength(9);
  });

  test("visible conversation history can expand from compacted to full renderable blocks", async () => {
    const app = await import("../App");
    const chat = Array.from({ length: 70 }, (_, index) => ({
      id: `chat-${index}`,
      role: index % 2 === 0 ? "user" : "assistant",
      kind: "message",
      text: `message-${index}`,
      questions: []
    })) satisfies MessageBlock[];

    const compacted = app.visibleConversationBlocksForHistory(chat, false);
    const expanded = app.visibleConversationBlocksForHistory(chat, true);

    expect(compacted.map((block) => block.id)).toEqual([
      "chat-history-collapsed",
      ...Array.from({ length: 60 }, (_, index) => `chat-${index + 10}`)
    ]);
    expect(app.visibleConversationBlocksForHistory(chat, false).some((block) => block.kind === "chat_history_collapsed")).toBe(true);
    expect(expanded).toEqual(chat);
  });

  test("conversation block compaction does not duplicate server history collapse cards", async () => {
    const app = await import("../App");
    const serverCollapsed = {
      id: "chat-history-collapsed",
      role: "tool",
      kind: "chat_history_collapsed",
      status: "completed",
      summary: "4078 条历史对话已折叠",
      questions: []
    } satisfies MessageBlock;
    const chat = Array.from({ length: 6 }, (_, index) => ({
      id: `chat-${index}`,
      role: "assistant",
      kind: "message",
      text: `message-${index}`,
      questions: []
    })) satisfies MessageBlock[];

    const compacted = app.compactConversationBlocks([serverCollapsed, ...chat], 80, 2);

    expect(compacted.map((block) => block.id)).toEqual([
      "chat-history-collapsed",
      "chat-0",
      "chat-1",
      "chat-2",
      "chat-3",
      "chat-4",
      "chat-5"
    ]);
  });

  test("follow-up queue helpers show status and compact previews", async () => {
    const app = await import("../App");

    expect(app.followUpStatusLabel("pending")).toBe("待跟进");
    expect(app.followUpStatusLabel("submitted")).toBe("已提交");
    expect(app.followUpMessagePreview({
      status: "pending",
      message: "继续\n检查   运行状态",
      error: null
    })).toBe("继续 检查 运行状态");
    expect(app.followUpMessagePreview({
      status: "error",
      message: "ignored",
      error: "local state unavailable"
    })).toBe("local state unavailable");
  });

  test("subscribe thread events uses credentials and dispatches block summary and errors", async () => {
    const { subscribeThreadEvents } = await loadRealApi();
    const listeners = new Map<string, (event: MessageEvent) => void>();
    const close = vi.fn();
    class MockEventSource {
      static instances: MockEventSource[] = [];
      constructor(readonly url: string, readonly init?: EventSourceInit) {
        MockEventSource.instances.push(this);
      }
      addEventListener(type: string, listener: EventListenerOrEventListenerObject) {
        listeners.set(type, listener as (event: MessageEvent) => void);
      }
      close = close;
    }
    vi.stubGlobal("EventSource", MockEventSource);
    const onBlock = vi.fn();
    const onSummary = vi.fn();
    const onError = vi.fn();

    const unsubscribe = subscribeThreadEvents("thread-a", { onBlock, onSummary, onError });
    listeners.get("block")?.(new MessageEvent("block", { data: JSON.stringify({ id: "b1", role: "assistant", kind: "message", questions: [] }) }));
    listeners.get("summary")?.(new MessageEvent("summary", { data: JSON.stringify({ id: "thread-a", title: "wanka", status: "Running", message_count: 1 }) }));
    listeners.get("error")?.(new MessageEvent("error", { data: "stream failed" }));
    listeners.get("error")?.(new Event("error") as MessageEvent);
    unsubscribe();

    expect(MockEventSource.instances[0].url).toBe("/api/rpc/threadEvents/thread-a");
    expect(MockEventSource.instances[0].init).toEqual({ withCredentials: true });
    expect(onBlock).toHaveBeenCalledWith(expect.objectContaining({ id: "b1" }), "thread-a");
    expect(onSummary).toHaveBeenCalledWith(expect.objectContaining({ status: "Running" }), "thread-a");
    expect(onError).toHaveBeenCalledWith("stream failed", "thread-a");
    expect(onError).toHaveBeenCalledWith("stream disconnected", "thread-a");
    expect(close).toHaveBeenCalledOnce();
  });

  test("subscribe thread events batches block updates and flushes on unsubscribe", async () => {
    vi.useFakeTimers();
    const { subscribeThreadEvents } = await loadRealApi();
    const listeners = new Map<string, (event: MessageEvent) => void>();
    const close = vi.fn();
    class MockEventSource {
      static instances: MockEventSource[] = [];
      constructor(readonly url: string, readonly init?: EventSourceInit) {
        MockEventSource.instances.push(this);
      }
      addEventListener(type: string, listener: EventListenerOrEventListenerObject) {
        listeners.set(type, listener as (event: MessageEvent) => void);
      }
      close = close;
    }
    vi.stubGlobal("EventSource", MockEventSource);
    const onBlock = vi.fn();
    const onBlocks = vi.fn();

    const unsubscribe = subscribeThreadEvents("thread-a", { onBlock, onBlocks });
    listeners.get("block")?.(new MessageEvent("block", { data: JSON.stringify({ id: "b1", role: "assistant", kind: "message", text: "one", questions: [] }) }));
    listeners.get("block")?.(new MessageEvent("block", { data: JSON.stringify({ id: "b2", role: "assistant", kind: "message", text: "two", questions: [] }) }));

    expect(onBlock).toHaveBeenCalledTimes(2);
    expect(onBlocks).not.toHaveBeenCalled();
    vi.advanceTimersByTime(99);
    expect(onBlocks).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(onBlocks).toHaveBeenCalledTimes(1);
    expect(onBlocks).toHaveBeenLastCalledWith([
      expect.objectContaining({ id: "b1" }),
      expect.objectContaining({ id: "b2" })
    ], "thread-a");

    listeners.get("block")?.(new MessageEvent("block", { data: JSON.stringify({ id: "b3", role: "assistant", kind: "message", text: "three", questions: [] }) }));
    unsubscribe();

    expect(onBlocks).toHaveBeenCalledTimes(2);
    expect(onBlocks).toHaveBeenLastCalledWith([expect.objectContaining({ id: "b3" })], "thread-a");
    expect(close).toHaveBeenCalledOnce();
  });

  test("conversation message helper hides internal rollout roles", async () => {
    const app = await import("../App");

    expect(app.shouldRenderConversationMessage({
      id: "developer-1",
      role: "developer",
      kind: "message",
      text: "internal instructions",
      questions: []
    })).toBe(false);
    expect(app.shouldRenderConversationMessage({
      id: "reasoning-1",
      role: "assistant",
      kind: "reasoning",
      text: "hidden reasoning",
      questions: []
    })).toBe(false);
    expect(app.shouldRenderConversationMessage({
      id: "assistant-1",
      role: "assistant",
      kind: "message",
      text: "visible answer",
      questions: []
    })).toBe(true);
  });

  test("conversation message helper hides plan mode action and subagent context blocks", async () => {
    const app = await import("../App");

    expect(app.shouldRenderConversationMessage({
      id: "choice-1",
      role: "assistant",
      kind: "request_user_input",
      text: "选择方案",
      questions: [{ id: "q1", question: "选择方案", options: [{ label: "A" }] }]
    })).toBe(false);
    expect(app.shouldRenderConversationMessage({
      id: "subagent-1",
      role: "user",
      kind: "message",
      text: "<subagent_notification>{\"agent_path\":\"/tmp/child\"}</subagent_notification>",
      questions: []
    })).toBe(false);
    expect(app.shouldRenderConversationMessage({
      id: "subagent-2",
      role: "user",
      kind: "message",
      text: "<subagent_context>\n- /tmp/child: worker\n</subagent_context>",
      questions: []
    })).toBe(false);
  });

  test("plan helper exposes proposed plan body without transcript tags", async () => {
    const app = await import("../App");

    expect(app.extractPlanText("<proposed_plan>\n# Summary\n- Fix it\n</proposed_plan>")).toBe("# Summary\n- Fix it");
    expect(app.extractPlanText("")).toBe("Plan 内容等待 Codex 写入。");
  });

  test("message stream only follows when already near the bottom", async () => {
    const app = await import("../App");

    expect(app.shouldAutoFollowMessageStream({
      scrollTop: 900,
      clientHeight: 600,
      scrollHeight: 1540
    })).toBe(true);
    expect(app.shouldAutoFollowMessageStream({
      scrollTop: 320,
      clientHeight: 600,
      scrollHeight: 1540
    })).toBe(false);
  });
});
