import { afterEach, describe, expect, test, vi } from "vitest";
import type { MessageBlock, ProbeLogsDbStatus, ProbeStatus, SystemStatus, ThreadDetail, ThreadSummary } from "../types";

async function loadRealApi() {
  vi.stubEnv("VITE_USE_REAL_API", "1");
  vi.resetModules();
  return import("./api");
}

describe("archive delete API compatibility", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
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

    const [path, options] = fetchMock.mock.calls[0] as [string, RequestInit & { headers: Headers; body: string }];
    expect(path).toBe("/api/archives/delete/execute");
    expect(options.method).toBe("POST");
    expect(options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(JSON.parse(options.body)).toEqual({ confirmed: true });
  });

  test("uses hidden thread cleanup endpoints with dry-run and boolean confirmation", async () => {
    const { dryRunHiddenThreadDelete, startHiddenThreadDelete } = await loadRealApi();
    const fetchMock = vi.fn(async (path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify(
      String(path).endsWith("/dry-run")
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
      "/api/hidden-threads/delete/dry-run",
      "/api/hidden-threads/delete/execute"
    ]);
    const [, executeOptions] = fetchMock.mock.calls[1] as [string, RequestInit & { headers: Headers; body: string }];
    expect(executeOptions.method).toBe("POST");
    expect(executeOptions.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(JSON.parse(executeOptions.body)).toEqual({ confirmed: true });
  });

  test("upload API posts FormData without JSON content-type and deletes uploads with csrf", async () => {
    const { uploadFiles, deleteUpload } = await loadRealApi();
    const fetchMock = vi.fn(async (path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify(
      String(path).startsWith("/api/uploads/")
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
    const [, uploadOptions] = fetchMock.mock.calls[0] as [string, RequestInit & { headers: Headers; body: FormData }];
    expect(fetchMock.mock.calls[0][0]).toBe("/api/uploads");
    expect(uploadOptions.method).toBe("POST");
    expect(uploadOptions.body).toBeInstanceOf(FormData);
    expect(uploadOptions.headers.get("content-type")).toBeNull();
    expect(uploadOptions.headers.get("x-csrf-token")).toBe("csrf-token");
    const [, deleteOptions] = fetchMock.mock.calls[1] as [string, RequestInit & { headers: Headers }];
    expect(fetchMock.mock.calls[1][0]).toBe("/api/uploads/upload-1");
    expect(deleteOptions.method).toBe("DELETE");
    expect(deleteOptions.headers.get("x-csrf-token")).toBe("csrf-token");
  });

  test("goal resume posts a controlled csrf-protected API request", async () => {
    const { resumeGoalMode } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify({
      enabled: true,
      objective: "ship the fix",
      token_budget: 123,
      status: "active"
    }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    const result = await resumeGoalMode("thread-a", "csrf-token");

    expect(result).toMatchObject({ available: true, data: { status: "active" } });
    const [path, options] = fetchMock.mock.calls[0] as [string, RequestInit & { headers: Headers; body: string }];
    expect(path).toBe("/api/codex/goal/resume");
    expect(options.method).toBe("POST");
    expect(options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(JSON.parse(options.body)).toEqual({ thread_id: "thread-a" });
  });

  test("stop thread posts stop payload with csrf", async () => {
    const { stopThread } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response("{}", {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    await stopThread("thread-a", { turn_id: "turn-live", job_id: "job-live" }, "csrf-token");

    const [path, options] = fetchMock.mock.calls[0] as [string, RequestInit & { headers: Headers; body: string }];
    expect(path).toBe("/api/threads/thread-a/stop");
    expect(options.method).toBe("POST");
    expect(options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(JSON.parse(options.body)).toEqual({ turn_id: "turn-live", job_id: "job-live" });
  });

  test("demo plugin list mirrors composer mention metadata", async () => {
    vi.resetModules();
    const { listPlugins } = await import("./api");

    const plugins = await listPlugins();

    expect(plugins).toEqual(expect.arrayContaining([
      expect.objectContaining({
        id: "codex",
        description: expect.stringContaining("app-server"),
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

    expect(fetchMock.mock.calls[0][0]).toBe("/api/threads/thread-a?limit=120&before=b%3A240&full=true");
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

    expect(fetchMock.mock.calls[0][0]).toBe("/api/probe/events?limit=10");
    expect(result.available).toBe(true);
    expect(result.data?.events[0].kind).toBe("hook-stop");
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

    expect(fetchMock.mock.calls[0][0]).toBe("/api/threads/thread-a/blocks?limit=80&before=b%3A200");
    expect(page.thread_id).toBe("thread-a");
    expect(page.blocks[0].id).toBe("b1");
    expect(page.before_cursor).toBe("b:120");
  });

  test("routes panel updates to panel-specific endpoints", async () => {
    const { startUpdateJob } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify({ job_id: "panel-job" }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    const result = await startUpdateJob("panel", "start", "csrf-token");

    const [path, options] = fetchMock.mock.calls[0] as [string, RequestInit & { headers: Headers }];
    expect(result).toEqual({ job_id: "panel-job" });
    expect(path).toBe("/api/system/panel/update/start");
    expect(options.method).toBe("POST");
    expect(options.headers.get("x-csrf-token")).toBe("csrf-token");
  });

  test("routes codex updates to codex-specific endpoints before legacy fallback", async () => {
    const { startUpdateJob } = await loadRealApi();
    const fetchMock = vi.fn(async (path: RequestInfo | URL, _options?: RequestInit) => new Response(
      String(path).includes("/api/system/codex/update/precheck")
        ? JSON.stringify({ job_id: "codex-job" })
        : JSON.stringify({ error: "wrong route" }),
      {
        status: String(path).includes("/api/system/codex/update/precheck") ? 200 : 404,
        headers: { "content-type": "application/json" }
      }
    ));
    vi.stubGlobal("fetch", fetchMock);

    const result = await startUpdateJob("codex", "precheck", "csrf-token");

    const [path, options] = fetchMock.mock.calls[0] as [string, RequestInit & { headers: Headers }];
    expect(result).toEqual({ job_id: "codex-job" });
    expect(path).toBe("/api/system/codex/update/precheck");
    expect(options.method).toBe("POST");
    expect(options.headers.get("x-csrf-token")).toBe("csrf-token");
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
      "/api/providers": [{ id: "codex", label: "Codex", status: "ready", capabilities: ["threads"] }],
      "/api/providers/claude-code/overview": {
        home: "/Users/gosu/.claude",
        settings_exists: true,
        settings_preview: { apiKey: "[redacted]" },
        projects: [{ id: "-Users-gosu-demo", display_name: "/Users/gosu/demo", session_count: 1, sessions: [] }]
      },
      "/api/platform": {
        kind: "linux",
        data_dir: "/opt/nexushub",
        config_file: "/opt/nexushub/config.toml",
        webui_dir: "/opt/nexushub/webui",
        log_dir: "/opt/nexushub/logs",
        service_name: "nexushub",
        service_kind: "systemd"
      },
      "/api/probe/status": {
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
      "/api/probe/settings": {
        codex: {
          home: "/root/.codex",
          app_server_service: "codex-app-server-root.service",
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
      "/api/providers",
      "/api/providers/claude-code/overview",
      "/api/platform",
      "/api/probe/status",
      "/api/probe/settings"
    ]);
  });

  test("preview provider endpoints return unavailable when the backend has not enabled them", async () => {
    const { getClaudeCodeOverview, getProbeStatus } = await loadRealApi();
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({ error: "not found" }), {
      status: 404,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(getClaudeCodeOverview()).resolves.toMatchObject({ available: false });
    await expect(getProbeStatus()).resolves.toMatchObject({ available: false });
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
    expect(result.data?.recent_sessions?.[0]).toMatchObject({ id: "session-a", project_display_name: "/Users/gosu/demo" });
  });

  test("Probe slim API surface keeps logs DB maintenance read-only", async () => {
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
      if (textPath.endsWith("/settings")) {
        return new Response(JSON.stringify({ saved: true }), {
          status: 200,
          headers: { "content-type": "application/json" }
        });
      }
      if (textPath.endsWith("/logs-db/status")) {
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
    await saveProbeSettings({
      codex: { home: "/root/.codex", app_server_service: "codex-app-server-root.service", host_label: "cloud" },
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
      ["/api/probe/logs-db/status", undefined, null, null],
      ["/api/probe/bark/test", "POST", "csrf-token", null],
      ["/api/probe/settings", "PATCH", "csrf-token", {
        codex: { home: "/root/.codex", app_server_service: "codex-app-server-root.service", host_label: "cloud" },
        probe: {
          poll_seconds: 20,
          notifications: { enabled: true, device_key: "secret" },
          logs_db: { enabled: true, retention_days: 2 }
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
      configured_app_server_socket: "/run/codex/custom.sock",
      resolved_app_server_socket: "/run/codex/custom.sock",
      app_server_socket_source: "configured",
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
      "/api/probe/status": probeStatus,
      "/api/probe/logs-db/status": logsDbStatus
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
        configured_app_server_socket: "/run/codex/custom.sock",
        resolved_app_server_socket: "/run/codex/custom.sock",
        app_server_socket_source: "configured",
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
      panel_db: "/opt/nexushub/nexushub.sqlite",
      app_server_service: { active: true }
    };
    const logsDb: ProbeLogsDbStatus = {
      path: "/home/codex/.codex/logs_2.sqlite",
      logs_db_source: "resolved_codex_home"
    };

    expect(app.codexHomeStatusValue(status)).toBe("/home/codex/.codex · auto");
    expect(app.logsDbPathStatusValue(logsDb)).toBe("/home/codex/.codex/logs_2.sqlite · resolved_codex_home");
    expect(app.codexHomeStatusValue({ codex_home: "" })).toBe("未知");
  });

  test("saveProbeSettings sends only canonical codex and probe keys", async () => {
    const { saveProbeSettings } = await loadRealApi();
    const fetchMock = vi.fn(async (_path: RequestInfo | URL, _options?: RequestInit) => new Response(JSON.stringify({ saved: true }), {
      status: 200,
      headers: { "content-type": "application/json" }
    }));
    vi.stubGlobal("fetch", fetchMock);

    await saveProbeSettings({
      codex: { home: "/root/.codex", app_server_service: "codex-app-server-root.service", host_label: "cloud" },
      probe: {
        enabled: true,
        notifications: { enabled: true, device_key: "secret", server_url: "https://api.day.app" },
        logs_db: { enabled: true, retention_days: 2 }
      }
    }, "csrf-token");

    const [path, options] = fetchMock.mock.calls[0] as [string, RequestInit & { headers: Headers; body: string }];
    const body = JSON.parse(options.body);
    expect(path).toBe("/api/probe/settings");
    expect(options.method).toBe("PATCH");
    expect(options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(Object.keys(body).sort()).toEqual(["codex", "probe"]);
    expect(body.probe.notifications).toEqual({
      enabled: true,
      device_key: "secret",
      server_url: "https://api.day.app"
    });
    expect(body.probe.logs_db).toEqual({ enabled: true, retention_days: 2 });
  });

  test("fixed Claude maintenance jobs use canonical API routes", async () => {
    const { startClaudeCodeJob } = await loadRealApi();
    const fetchMock = vi.fn(async (path: RequestInfo | URL, _options?: RequestInit) => {
      const segments = String(path).split("/");
      return new Response(JSON.stringify({ job_id: `${segments[segments.length - 1]}-job` }), {
      status: 200,
      headers: { "content-type": "application/json" }
      });
    });
    vi.stubGlobal("fetch", fetchMock);

    await expect(startClaudeCodeJob("version-check", "csrf-token")).resolves.toEqual({ job_id: "version-check-job" });
    await expect(startClaudeCodeJob("update-precheck", "csrf-token")).resolves.toEqual({ job_id: "precheck-job" });
    await expect(startClaudeCodeJob("update-start", "csrf-token")).resolves.toEqual({ job_id: "start-job" });
    await expect(startClaudeCodeJob("smoke", "csrf-token")).resolves.toEqual({ job_id: "smoke-job" });
    await expect(startClaudeCodeJob("cache-status", "csrf-token")).resolves.toEqual({ job_id: "cache-status-job" });

    expect(fetchMock.mock.calls.map(([path, options]) => [path, (options as RequestInit).method, ((options as RequestInit).headers as Headers).get("x-csrf-token")])).toEqual([
      ["/api/providers/claude-code/jobs/version-check", "POST", "csrf-token"],
      ["/api/providers/claude-code/jobs/update/precheck", "POST", "csrf-token"],
      ["/api/providers/claude-code/jobs/update/start", "POST", "csrf-token"],
      ["/api/providers/claude-code/jobs/smoke", "POST", "csrf-token"],
      ["/api/providers/claude-code/jobs/cache-status", "POST", "csrf-token"]
    ]);
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

    const [path, options] = fetchMock.mock.calls[0] as [string, RequestInit & { body: string }];
    expect(path).toBe("/api/auth/login");
    expect(JSON.parse(options.body)).toEqual({
      username: "admin",
      password: "password",
      turnstile_token: "turnstile-token"
    });
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
      "/api/threads/thread-1/plan/accept",
      "/api/threads/thread-1/plan/revise",
      "/api/threads/thread-1/approval"
    ]);
    expect(JSON.parse((fetchMock.mock.calls[0][1] as RequestInit).body as string)).toEqual({
      turn_id: "turn-1",
      item_id: "plan-1"
    });
    expect(JSON.parse((fetchMock.mock.calls[1][1] as RequestInit).body as string)).toEqual({
      turn_id: "turn-1",
      item_id: "plan-1",
      instructions: "缩小范围"
    });
    expect(JSON.parse((fetchMock.mock.calls[2][1] as RequestInit).body as string)).toEqual({
      turn_id: "turn-1",
      item_id: "approval-1",
      decision: "approved"
    });
  });

  test("follow-up API posts thread payload with csrf and supports listing and cancel", async () => {
    const { enqueueFollowUp, listFollowUps, cancelFollowUp } = await loadRealApi();
    const fetchMock = vi.fn(async (path: RequestInfo | URL, options?: RequestInit) => {
      if (String(path).endsWith("/cancel")) {
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

    const [postPath, postOptions] = fetchMock.mock.calls[0] as [string, RequestInit & { headers: Headers; body: string }];
    expect(postPath).toBe("/api/threads/thread-a/follow-ups");
    expect(postOptions.method).toBe("POST");
    expect(postOptions.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(JSON.parse(postOptions.body)).toEqual({ message: "继续检查", model: "gpt-5.5" });
    expect(fetchMock.mock.calls[1][0]).toBe("/api/threads/thread-a/follow-ups");
    expect(fetchMock.mock.calls[2][0]).toBe("/api/threads/thread-a/follow-ups/fu-1/cancel");
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
      cwd: "/home/ubuntu/codex-workspace"
    };
    const result = await steerThread("thread-a", payload, "csrf-token");

    const [path, options] = fetchMock.mock.calls[0] as [string, RequestInit & { headers: Headers; body: string }];
    expect(result).toMatchObject({ turn_id: "turn-live", fallback: false });
    expect(path).toBe("/api/threads/thread-a/steer");
    expect(options.method).toBe("POST");
    expect(options.headers.get("x-csrf-token")).toBe("csrf-token");
    expect(JSON.parse(options.body)).toEqual(payload);
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

  test("slash commands expose compact Codex composer entry points", async () => {
    const app = await import("../App");

    expect(app.slashCommandSuggestions("/goal r", 7)).toEqual([
      expect.objectContaining({ command: "/goal resume", description: expect.stringContaining("恢复") })
    ]);
    expect(app.slashCommandSuggestions("/goal r", 7, false)).toEqual([
      expect.objectContaining({ command: "/goal resume", requiresThread: true })
    ]);
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

  test("does not surface stale pending blocks without reply-needed state or active turn", async () => {
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
    expect(app.pendingFromBlocks([oldChoice], "ReplyNeeded", null)).toBeNull();
    expect(app.latestActionBlock([oldPlan], "ReplyNeeded", null, app.isPlanBlock)).toBeNull();
    expect(app.pendingFromBlocks([oldChoice], "ReplyNeeded", "turn-old")?.questions[0].question).toBe("旧选择");
    expect(app.latestActionBlock([oldPlan], "ReplyNeeded", "turn-old", app.isPlanBlock)?.id).toBe("plan-old");
    expect(app.pendingFromBlocks([oldChoice], "Running", "turn-old")?.questions[0].question).toBe("旧选择");
    expect(app.latestActionBlock([oldPlan], "Running", "turn-old", app.isPlanBlock)?.id).toBe("plan-old");
    expect(app.latestActionBlock([oldPlan], "ReplyNeeded", "turn-new", app.isPlanBlock)).toBeNull();
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
      "是，实施此计划",
      "否，请告知 Codex 如何调整"
    ]);
    expect(app.planActionSubmission(0, "")).toEqual({ action: "accept" });
    expect(app.planActionSubmission(1, "")).toBeNull();
    expect(app.planActionSubmission(1, "  增加验收步骤  ")).toEqual({
      action: "revise",
      instructions: "增加验收步骤"
    });
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
      error: "bridge unavailable"
    })).toBe("bridge unavailable");
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

    expect(MockEventSource.instances[0].url).toBe("/api/threads/thread-a/events");
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

  test("codex update state helper exposes available, current, and unknown states", async () => {
    const app = await import("../App");

    expect(app.codexUpdateState({ codex_update_available: true })).toEqual({
      label: "可更新",
      tone: "warning"
    });
    expect(app.codexUpdateState({ codex_update_available: false })).toEqual({
      label: "已是最新",
      tone: "success"
    });
    expect(app.codexUpdateState({ codex_update_available: null })).toEqual({
      label: "未知",
      tone: "warning"
    });
    expect(app.codexUpdateState({})).toEqual({
      label: "未知",
      tone: "warning"
    });
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
