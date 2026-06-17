import type {
  ArchiveDeletePlan,
  ArchiveDeleteResult,
  AgentProviderInfo,
  ClaudeOverview,
  CodexConfig,
  CodexModel,
  FollowUpQueueItem,
  FollowUpQueueState,
  HiddenThreadDeletePlan,
  HiddenThreadDeleteResult,
  JobRecord,
  OptionalResult,
  PermissionProfile,
  PlatformOverview,
  PluginInfo,
  ProbeLogsDbStatus,
  ProbeEventsResponse,
  ProbeJobAction,
  ProbeSettings,
  ProbeStatus,
  PublicSettings,
  SecuritySettings,
  SentinelStatus,
  SessionUser,
  SystemStatus,
  SystemVersion,
  BridgeActionResult,
  CodexGoal,
  CodexGoalSaveInput,
  MessageBlock,
  ThreadDetail,
  ThreadBlockPage,
  ThreadSummary,
  UploadOutcome
} from "../types";
import {
  RuntimeUnavailableError,
  buildRuntimeApiPath,
  desktopSessionUser,
  isDesktopRuntime,
  runtimeRpc,
  type DesktopApiUpload
} from "./runtime";

type RequestOptions = RequestInit & {
  csrfToken?: string | null;
  skipContentType?: boolean;
};

function normalizeApiBase(base: string | undefined): string {
  const value = (base ?? "").trim();
  if (!value || value === "/") {
    return "";
  }
  if (/^https?:\/\//i.test(value)) {
    return value.replace(/\/+$/g, "");
  }
  return `/${value.replace(/^\/+|\/+$/g, "")}`;
}

const API_BASE = normalizeApiBase(import.meta.env.VITE_API_BASE);
const USE_DEMO = import.meta.env.DEV && import.meta.env.VITE_USE_REAL_API !== "1";

export class ApiError extends Error {
  constructor(message: string, readonly status: number) {
    super(message);
    this.name = "ApiError";
  }
}

function isMissingEndpoint(error: unknown): boolean {
  return error instanceof RuntimeUnavailableError || error instanceof ApiError && [404, 405, 501].includes(error.status);
}

export function buildApiPath(path: string): string {
  return buildRuntimeApiPath(path);
}

async function parse<T>(response: Response): Promise<T> {
  const contentType = response.headers.get("content-type") ?? "";
  const payload = contentType.includes("application/json") ? await response.json() : await response.text();
  if (!response.ok) {
    const message = typeof payload === "object" && payload && "error" in payload
      ? String((payload as { error: unknown }).error)
      : `请求失败，HTTP ${response.status}`;
    throw new ApiError(message, response.status);
  }
  return payload as T;
}

async function apiFetch<T>(path: string, options: RequestOptions = {}): Promise<T> {
  if (isDesktopRuntime()) {
    const method = options.method ?? (options.body ? "POST" : "GET");
    const body = typeof options.body === "string"
      ? JSON.parse(options.body)
      : undefined;
    return runtimeRpc<T>("desktopApi", { request: { path, method, body } });
  }
  const headers = new Headers(options.headers);
  if (!options.skipContentType && !headers.has("content-type") && options.body) {
    headers.set("content-type", "application/json");
  }
  if (options.csrfToken) {
    headers.set("x-csrf-token", options.csrfToken);
  }
  const response = await fetch(buildApiPath(path), {
    credentials: "include",
    ...options,
    headers
  });
  return parse<T>(response);
}

async function optionalApiFetch<T>(path: string, options: RequestOptions = {}): Promise<OptionalResult<T>> {
  try {
    return normalizeOptionalResult(await apiFetch<T>(path, options));
  } catch (error) {
    if (isMissingEndpoint(error)) {
      return { available: false, error: error instanceof Error ? error.message : String(error) };
    }
    throw error;
  }
}

async function apiFetchFirst<T>(paths: string[], options: RequestOptions = {}, label = "API"): Promise<T> {
  let lastMissing: ApiError | null = null;
  for (const path of paths) {
    try {
      return await apiFetch<T>(path, options);
    } catch (error) {
      if (isMissingEndpoint(error)) {
        lastMissing = error instanceof ApiError ? error : null;
        continue;
      }
      throw error;
    }
  }
  throw new ApiError(`${label} endpoint is not available${lastMissing ? ` (${lastMissing.message})` : ""}`, 404);
}

async function optionalApiFetchFirst<T>(paths: string[], options: RequestOptions = {}): Promise<OptionalResult<T>> {
  try {
    return normalizeOptionalResult(await apiFetchFirst<T>(paths, options));
  } catch (error) {
    if (isMissingEndpoint(error)) {
      return { available: false, error: error instanceof Error ? error.message : String(error) };
    }
    throw error;
  }
}

function normalizeOptionalResult<T>(payload: T): OptionalResult<T> {
  if (payload && typeof payload === "object" && "available" in payload && (payload as { available?: unknown }).available === false) {
    const unavailable = payload as { reason?: unknown; error?: unknown };
    return {
      available: false,
      reason: typeof unavailable.reason === "string" ? unavailable.reason : null,
      error: typeof unavailable.error === "string" ? unavailable.error : undefined
    };
  }
  return { available: true, data: payload };
}

export async function getPublicSettings(): Promise<PublicSettings> {
  if (isDesktopRuntime()) {
    return { site_name: "NexusHub", turnstile_enabled: false, turnstile_required: false, turnstile_site_key: "", turnstile_action: "login", admin_configured: true };
  }
  if (USE_DEMO) {
    return { site_name: "NexusHub", turnstile_enabled: false, turnstile_required: false, turnstile_site_key: "", turnstile_action: "login", admin_configured: true };
  }
  return apiFetch<PublicSettings>("/api/public/settings");
}

export async function login(username: string, password: string, turnstileToken?: string | null): Promise<SessionUser> {
  if (isDesktopRuntime()) {
    return desktopSessionUser();
  }
  if (USE_DEMO) {
    return { id: "dev", username, csrf_token: "dev-csrf" };
  }
  const body: { username: string; password: string; turnstile_token?: string } = { username, password };
  if (turnstileToken?.trim()) {
    body.turnstile_token = turnstileToken.trim();
  }
  return apiFetch<SessionUser>("/api/auth/login", {
    method: "POST",
    body: JSON.stringify(body)
  });
}

export async function logout(csrfToken?: string | null): Promise<void> {
  if (isDesktopRuntime()) return;
  if (USE_DEMO) return;
  await apiFetch("/api/auth/logout", { method: "POST", csrfToken });
}

export async function me(): Promise<SessionUser> {
  if (isDesktopRuntime()) return desktopSessionUser();
  if (USE_DEMO) return { id: "dev", username: "admin", csrf_token: "dev-csrf" };
  return apiFetch<SessionUser>("/api/auth/me");
}

export async function listThreads(status: string, q: string): Promise<ThreadSummary[]> {
  if (USE_DEMO) return demoThreads(status, q);
  const params = new URLSearchParams();
  if (status !== "all") params.set("status", status);
  if (q.trim()) params.set("q", q.trim());
  params.set("limit", "120");
  return apiFetch<ThreadSummary[]>(`/api/threads?${params.toString()}`);
}

export type ThreadDetailOptions = {
  limit?: number;
  before?: string | null;
  full?: boolean;
};

export async function getThread(id: string, options: ThreadDetailOptions = {}): Promise<ThreadDetail> {
  if (USE_DEMO) {
    const summary = demoThreads("all", "").find((thread) => thread.id === id) ?? demoThreads("all", "")[0];
    const longChatBlocks: MessageBlock[] = Array.from({ length: 68 }, (_, index) => ({
      id: `history-${index}`,
      role: index % 2 === 0 ? "user" : "assistant",
      kind: "message",
      text: index % 2 === 0 ? `历史请求 ${index + 1}` : `历史回复 ${index + 1}`,
      questions: []
    }));
    const completedTools: MessageBlock[] = Array.from({ length: 20 }, (_, index) => ({
      id: `tool-history-${index}`,
      role: "tool",
      kind: "function_call_output",
      tool_name: "shell",
      status: "completed",
      summary: `历史工具 ${index + 1} 已完成`,
      text: `stdout line ${index + 1}`,
      questions: []
    }));
    return {
      summary: id === "019e95a0-demo" ? { ...summary, active_turn_id: "turn-plan-demo", pending_elicitation: { turn_id: "turn-plan-demo", item_id: "question-demo", questions: [{ id: "q1", question: "选择执行方式", options: [{ label: "直接实施", description: "使用当前计划继续执行" }, { label: "先修改", description: "补充约束后重新计划" }] }] } } : summary,
      raw_event_count: 96,
      total_blocks: 96,
      has_more_blocks: false,
      before_cursor: null,
      blocks: [
        ...longChatBlocks,
        { id: "u1", role: "user", kind: "userMessage", text: "检查云机 Codex 状态。", questions: [] },
        { id: "plan-demo", role: "assistant", kind: "plan", display_kind: "plan", turn_id: "turn-plan-demo", item_id: "plan-demo", status: "pending", resolved: false, plan_status: "pending", text: "<proposed_plan>1. 核对线程状态\n2. 修复 Plan/Questions 展示\n3. 验证并部署</proposed_plan>", questions: [] },
        { id: "question-answered", role: "assistant", kind: "request_user_input_result", display_kind: "question_result", turn_id: "turn-old-demo", status: "completed", resolved: true, answers: [{ question_id: "q0", answers: ["保留"], note: "历史选择已回答" }], questions: [{ id: "q0", question: "历史选项", options: [{ label: "保留" }, { label: "修改" }] }] },
        { id: "question-demo", role: "assistant", kind: "request_user_input", display_kind: "question", turn_id: "turn-plan-demo", call_id: "question-demo", status: "pending", resolved: false, questions: [{ id: "q1", question: "选择执行方式", options: [{ label: "直接实施", description: "使用当前计划继续执行" }, { label: "先修改", description: "补充约束后重新计划" }] }] },
        { id: "a1", role: "assistant", kind: "agentMessage", text: "状态正常，本地 Codex 状态库可读。归档删除 dry-run 可执行。", questions: [] },
        ...completedTools,
        { id: "t1", role: "tool", kind: "commandExecution", tool_name: "shell", text: "codex-cloud-doctor\nsqlite integrity_check: ok", status: "completed", questions: [] },
        { id: "t-running", role: "tool", kind: "function_call", tool_name: "shell", summary: "正在刷新本地状态", text: "sqlite3 /root/.codex/state_5.sqlite 'pragma integrity_check;'", status: "running", questions: [] }
      ],
      messages: [
        { role: "user", kind: "message", text: "检查云机 Codex 状态。" },
        { role: "assistant", kind: "message", text: "状态正常，本地 Codex 状态库可读。归档删除 dry-run 可执行。" },
        { role: "tool", kind: "function_call", text: "codex-cloud-doctor\nsqlite integrity_check: ok" }
      ]
    };
  }
  const params = new URLSearchParams();
  if (options.limit !== undefined) params.set("limit", String(options.limit));
  if (options.before) params.set("before", options.before);
  if (options.full) params.set("full", "true");
  const query = params.toString();
  return apiFetch<ThreadDetail>(`/api/threads/${id}${query ? `?${query}` : ""}`);
}

export async function getThreadBlocks(id: string, options: Pick<ThreadDetailOptions, "limit" | "before"> = {}): Promise<ThreadBlockPage> {
  if (USE_DEMO) {
    const detail = await getThread(id, options);
    return {
      thread_id: id,
      blocks: detail.blocks,
      total_blocks: detail.total_blocks ?? detail.blocks.length,
      has_more_blocks: Boolean(detail.has_more_blocks),
      before_cursor: detail.before_cursor ?? null
    };
  }
  const params = new URLSearchParams();
  if (options.limit !== undefined) params.set("limit", String(options.limit));
  if (options.before) params.set("before", options.before);
  const query = params.toString();
  return apiFetch<ThreadBlockPage>(`/api/threads/${id}/blocks${query ? `?${query}` : ""}`);
}

export async function getSystemStatus(): Promise<SystemStatus> {
  if (USE_DEMO) {
    return {
      host_label: "43.155.235.227",
      hostname: "codex-cloud-root",
      public_endpoint: "https://661313.xyz/nexushub/",
      codex_home: "/root/.codex",
      configured_codex_home: "/root/.codex",
      resolved_codex_home: "/root/.codex",
      codex_home_source: "config",
      panel_db: "/opt/nexushub/panel.sqlite",
      state_db_integrity: "ok"
    };
  }
  return apiFetch<SystemStatus>("/api/system/status");
}

export async function getSystemVersion(): Promise<SystemVersion> {
  if (USE_DEMO) {
    return {
      panel_current: "0.1.5",
      panel_latest: "v0.1.5",
      panel_update_available: false,
      codex_current: "0.137.0",
      codex_latest: "0.137.0",
      codex_update_available: false,
      codex_user: "codex-cli 0.137.0",
      codex_root: "codex-cli 0.137.0",
      codex_raw: "codex-cli 0.137.0"
    };
  }
  return apiFetch<SystemVersion>("/api/system/version");
}

export async function getSecurity(): Promise<SecuritySettings> {
  if (USE_DEMO) {
    return {
      turnstile_enabled: false,
      turnstile_required: false,
      turnstile_site_key: "",
      turnstile_secret_configured: false,
      session_ttl_seconds: 31536000,
      turnstile_expected_hostname: "661313.xyz",
      turnstile_expected_action: "login"
    };
  }
  return apiFetch<SecuritySettings>("/api/security");
}

export async function listProviders(): Promise<AgentProviderInfo[]> {
  if (USE_DEMO) {
    return [
      {
        id: "codex",
        label: "Codex",
        status: "ready",
        description: "完整 Codex 控制面，使用官方 state DB、session_index、rollout 与受控 job。",
        capabilities: ["threads", "chat", "plan_questions", "uploads", "updates", "doctor"],
        safety: "保留官方数据结构，不修改 Codex DB schema"
      },
      {
        id: "claude_code",
        label: "Claude Code",
        status: "preview",
        description: "只读发现 ~/.claude 项目、会话和配置摘要。",
        capabilities: ["projects", "sessions", "settings_read"],
        safety: "不写入 ~/.claude，不启动或恢复会话"
      },
      { id: "cursor", label: "Cursor CLI", status: "planned", capabilities: [], safety: "未开放命令执行" },
      { id: "gemini", label: "Gemini CLI", status: "planned", capabilities: [], safety: "未开放命令执行" }
    ];
  }
  return apiFetch<AgentProviderInfo[]>("/api/providers");
}

export async function getClaudeCodeOverview(): Promise<OptionalResult<ClaudeOverview>> {
  if (USE_DEMO) {
    const now = new Date().toISOString();
    const oneHourAgo = new Date(Date.now() - 3600_000).toISOString();
    return {
      available: true,
      data: {
        home: "~/.claude",
        settings_exists: true,
        settings_preview: {
          permissions: { allow: ["Read"], deny: ["Write"] },
          mcpServers: {
            github: { command: "npx", args: ["-y", "@modelcontextprotocol/server-github"], env: { GITHUB_TOKEN: "[redacted]" } }
          },
          apiKey: "[redacted]"
        },
        projects: [{
          id: "-Users-gosu-demo",
          display_name: "/Users/gosu/demo",
          path_hint: "/Users/gosu/demo",
          session_count: 2,
          sessions: [
            { id: "session-a", title: "NexusHub provider shell", updated_at: now, message_count: 18, last_message_preview: "Provider summary ready" },
            { id: "session-b", title: "只读配置审计", updated_at: oneHourAgo, message_count: 7, last_message_preview: "Settings redacted" }
          ]
        }],
        recent_sessions: [
          { project_id: "-Users-gosu-demo", project_display_name: "/Users/gosu/demo", id: "session-a", title: "NexusHub provider shell", updated_at: now, message_count: 18, last_message_preview: "Provider summary ready" },
          { project_id: "-Users-gosu-demo", project_display_name: "/Users/gosu/demo", id: "session-b", title: "只读配置审计", updated_at: oneHourAgo, message_count: 7, last_message_preview: "Settings redacted" }
        ],
        mcp: {
          config_files: ["~/.claude/settings.json"],
          server_count: 1,
          servers: [{ name: "github", command: "npx", transport: null, args_count: 2, env_keys: ["GITHUB_TOKEN"], has_sensitive_env: true }]
        },
        installation: {
          claude_home: "~/.claude",
          settings_file: "~/.claude/settings.json",
          settings_exists: true,
          settings_local_file: "~/.claude/settings.local.json",
          settings_local_exists: false,
          user_config_file: "~/.claude.json",
          user_config_exists: true,
          executable_candidates: ["/usr/local/bin/claude"],
          version_hint: "demo",
          health_hints: []
        },
        cache_status: {
          cache_dir: "~/.claude/cache",
          cache_exists: true,
          cache_file_count: 3,
          cache_total_bytes: 4096,
          log_dir: "~/.claude/logs",
          log_exists: true,
          log_file_count: 2,
          log_total_bytes: 2048
        }
      }
    };
  }
  return optionalApiFetch<ClaudeOverview>("/api/providers/claude-code/overview");
}

export async function getPlatformOverview(): Promise<PlatformOverview> {
  if (USE_DEMO) {
    return {
      kind: "linux",
      data_dir: "/opt/nexushub",
      config_file: "/opt/nexushub/config.toml",
      webui_dir: "/opt/nexushub/webui",
      log_dir: "/opt/nexushub/logs",
      service_name: "nexushub",
      service_kind: "systemd"
    };
  }
  return apiFetch<PlatformOverview>("/api/platform");
}

export async function listPlugins(): Promise<PluginInfo[]> {
  if (USE_DEMO) {
    return [
      {
        id: "codex",
        label: "Codex",
        status: "ready",
        kind: "builtin",
        description: "Codex 本地线程、状态和受控操作",
        invocation_template: "@Codex "
      },
      {
        id: "probe",
        label: "Probe",
        status: "ready",
        kind: "builtin",
        description: "云机探针状态、Hook、Bark 和日志库维护",
        invocation_template: "@Probe "
      },
      {
        id: "claude_code",
        label: "Claude Code",
        status: "preview",
        kind: "builtin",
        description: "Claude Code 项目、会话和 MCP 只读预览",
        unavailable_reason: "当前仅支持只读预览，暂不支持从 Web 端调用 Claude Code",
        invocation_template: "@Claude Code "
      },
      {
        id: "system_ops",
        label: "System/Ops",
        status: "ready",
        kind: "builtin",
        description: "固定系统运维动作和发布更新任务",
        invocation_template: "@System/Ops "
      }
    ];
  }
  return apiFetch<PluginInfo[]>("/api/plugins");
}

export async function getSentinelStatus(): Promise<OptionalResult<SentinelStatus>> {
  return getProbeStatus();
}

export async function getProbeStatus(): Promise<OptionalResult<ProbeStatus>> {
  if (USE_DEMO) {
    return {
      available: true,
      data: demoProbeStatus()
    };
  }
  return optionalApiFetch<ProbeStatus>("/api/probe/status");
}

export async function getProbeSettings(): Promise<OptionalResult<ProbeSettings>> {
  if (USE_DEMO) {
    return {
      available: true,
      data: demoProbeSettings()
    };
  }
  return optionalApiFetch<ProbeSettings>("/api/probe/settings");
}

function normalizeProbeSettingsSavePayload(settings: Partial<ProbeSettings>): Partial<ProbeSettings> {
  const nestedDeviceKey = settings.probe?.notifications?.device_key;
  if (typeof nestedDeviceKey !== "string" || !nestedDeviceKey.trim()) {
    return settings;
  }
  return {
    ...settings,
    notifications: {
      ...settings.notifications,
      device_key: nestedDeviceKey.trim()
    }
  };
}

export async function saveProbeSettings(settings: Partial<ProbeSettings>, csrfToken?: string | null): Promise<ProbeSettings> {
  if (USE_DEMO) return { ...demoProbeSettings(), ...settings } as ProbeSettings;
  return apiFetch<ProbeSettings>("/api/probe/settings", {
    method: "PATCH",
    csrfToken,
    body: JSON.stringify(normalizeProbeSettingsSavePayload(settings))
  });
}

export async function getProbeLogsDbStatus(): Promise<OptionalResult<ProbeLogsDbStatus>> {
  if (USE_DEMO) return {
    available: true,
    data: {
      status: "maintenance_ready",
      logs_db_status: "maintenance_ready",
      target: "codex_logs_2",
      path: "/root/.codex/logs_2.sqlite",
      configured_codex_home: "/root/.codex",
      resolved_codex_home: "/root/.codex",
      codex_home_source: "config",
      logs_db_source: "resolved_codex_home",
      discovery_warnings: [],
      total_rows: 128,
      old_rows: 6,
      retained_rows: 122,
      database_size: 524288,
      db_size_bytes: 524288,
      wal_size: 4096,
      wal_size_bytes: 4096,
      shm_size: 32768,
      shm_size_bytes: 32768,
      size_bytes: 524288,
      last_maintain_at: "2026-06-14T18:15:32Z",
      next_run_at: "2026-06-15T00:15:32Z",
      last_result: "dry-run: would_delete_rows=6",
      recent_result: "dry-run: would_delete_rows=6"
    }
  };
  return optionalApiFetch<ProbeLogsDbStatus>("/api/probe/logs-db/status");
}

export async function getProbeEvents(limit = 10): Promise<OptionalResult<ProbeEventsResponse>> {
  if (USE_DEMO) {
    return {
      available: true,
      data: {
        limit,
        events: [
          {
            id: "probe-event-reply-demo",
            kind: "reply-needed",
            thread_id: "019e95a0-demo",
            title: "Raw reply event",
            message: "Probe 事件已记录",
            dedupe_key: "reply-needed:019e95a0-demo:turn-plan-demo",
            source: "nexushubd probe passive-scan",
            payload: {
              event_type: "reply-needed",
              thread_title: "Plan Mode 修复",
              thread_id: "019e95a0-demo",
              turn_id: "turn-plan-demo",
              beijing_time: "2026-06-16 09:30:00 北京时间",
              reason_label: "等待用户确认",
              body_summary: "Plan Mode 等待用户确认",
              body_sha256: "6b5d9f4f5a5a",
              body_length: 324,
              body_source: "proposed_plan",
              body_truncated: false,
              source: "nexushubd probe passive-scan",
              bark: { title: "等待回复：Plan Mode 修复", sent: false, skipped: true, reason: "dedupe", http_status: 200, dedupe_hit: true, chunk_count: 1, request_count: 0 },
              dedupe: { claimed: true, duplicate: false, status: "claimed" }
            },
            created_at: new Date().toISOString(),
            handled_at: null
          },
          {
            id: "probe-event-completion-demo",
            kind: "completion",
            thread_id: "019e5281-demo",
            title: "Completion",
            message: "Thread completed",
            dedupe_key: "completion:019e5281-demo:turn-done",
            source: "nexushubd probe hook-stop",
            payload: {
              event_type: "completion",
              thread_title: "检查仓库状态",
              thread_id: "019e5281-demo",
              turn_id: "turn-done",
              reason_label: "任务完成",
              body_summary: "仓库状态干净",
              body_sha256: "a13f98c0",
              body_length: 128,
              body_source: "task_complete.last_agent_message",
              body_truncated: false,
              source: "nexushubd probe hook-stop",
              bark: { title: "线程正常完成：检查仓库状态", sent: true, skipped: false, http_status: 200, dedupe_hit: false, chunk_count: 1, request_count: 1 },
              dedupe: { claimed: true, duplicate: false, status: "claimed" }
            },
            created_at: new Date(Date.now() - 300000).toISOString(),
            handled_at: null
          },
          {
            id: "probe-event-hook-demo",
            kind: "hook-stop",
            thread_id: "019e95a0-demo",
            title: "Codex Stop Hook",
            message: "Stop Hook event recorded by NexusHub Probe",
            dedupe_key: "hook-stop:019e95a0-demo:turn-demo",
            source: "nexushubd probe hook-stop",
            payload: {
              event_type: "hook-stop",
              thread_title: "Plan Mode 修复",
              thread_id: "019e95a0-demo",
              turn_id: "turn-demo",
              reason_label: "Stop Hook",
              body_summary: "Stop Hook event recorded by NexusHub Probe",
              body_sha256: "d9a8",
              body_length: 212,
              body_source: "default",
              body_truncated: false,
              source: "nexushubd probe hook-stop",
              bark: { title: "探针事件：Plan Mode 修复", skipped: true, reason: "notifications-disabled", dedupe_hit: false, chunk_count: 0, request_count: 0 },
              dedupe: { claimed: false, duplicate: true, status: "duplicate" }
            },
            created_at: new Date(Date.now() - 600000).toISOString(),
            handled_at: null
          }
        ]
      }
    };
  }
  const query = limit ? `?limit=${encodeURIComponent(String(limit))}` : "";
  return optionalApiFetch<ProbeEventsResponse>(`/api/probe/events${query}`);
}

export async function saveSecurity(settings: Partial<SecuritySettings> & { turnstile_secret_key?: string }, csrfToken?: string | null) {
  return apiFetch<SecuritySettings>("/api/security", {
    method: "PATCH",
    csrfToken,
    body: JSON.stringify(settings)
  });
}

export async function dryRunArchiveDelete(csrfToken?: string | null): Promise<ArchiveDeletePlan> {
  if (USE_DEMO) {
    return { total_threads: 42, active_threads: 31, archived_threads: 11, session_index_lines: 44, rollout_files: 39, archived_ids: ["019e-demo-a", "019e-demo-b"], integrity: "ok" };
  }
  return apiFetch<ArchiveDeletePlan>("/api/archives/delete/dry-run", { method: "POST", csrfToken });
}

export async function startArchiveDelete(csrfToken?: string | null): Promise<ArchiveDeleteResult> {
  return apiFetch<ArchiveDeleteResult>("/api/archives/delete/execute", {
    method: "POST",
    csrfToken,
    body: JSON.stringify({ confirmed: true })
  });
}

export async function dryRunHiddenThreadDelete(csrfToken?: string | null): Promise<HiddenThreadDeletePlan> {
  if (USE_DEMO) {
    return {
      total_threads: 42,
      visible_threads: 38,
      hidden_threads: 4,
      archived_threads: 0,
      session_index_lines: 42,
      rollout_files: 42,
      hidden_ids: ["019e-hidden-a", "019e-hidden-b", "019e-hidden-c", "019e-hidden-d"],
      hidden_source_counts: { exec: 1, subagent: 3 },
      integrity: "ok"
    };
  }
  return apiFetch<HiddenThreadDeletePlan>("/api/hidden-threads/delete/dry-run", { method: "POST", csrfToken });
}

export async function startHiddenThreadDelete(csrfToken?: string | null): Promise<HiddenThreadDeleteResult> {
  if (USE_DEMO) {
    return {
      before: {
        total_threads: 42,
        visible_threads: 38,
        hidden_threads: 4,
        archived_threads: 0,
        session_index_lines: 42,
        rollout_files: 42,
        hidden_ids: ["019e-hidden-a", "019e-hidden-b", "019e-hidden-c", "019e-hidden-d"],
        hidden_source_counts: { exec: 1, subagent: 3 },
        integrity: "ok"
      },
      deleted_threads: 4,
      after_total_threads: 38,
      after_visible_threads: 38,
      after_hidden_threads: 0,
      after_archived_threads: 0,
      after_integrity: "ok",
      visible_threads: 38,
      hidden_threads: 0,
      integrity: "ok",
      deleted_rollout_files: 4
    };
  }
  return apiFetch<HiddenThreadDeleteResult>("/api/hidden-threads/delete/execute", {
    method: "POST",
    csrfToken,
    body: JSON.stringify({ confirmed: true })
  });
}

export async function startJob(path: string, csrfToken?: string | null): Promise<{ job_id: string }> {
  if (USE_DEMO) return { job_id: "job-demo" };
  return apiFetch<{ job_id: string }>(path, { method: "POST", csrfToken });
}

export type UpdateTarget = "panel";
export type UpdateAction = "precheck" | "start" | "prune";

const panelUpdateRoutes: Record<UpdateAction, string> = {
  precheck: "/api/system/panel/update/precheck",
  start: "/api/system/panel/update/start",
  prune: "/api/system/panel/update/prune"
};

export async function startUpdateJob(target: UpdateTarget, action: UpdateAction, csrfToken?: string | null): Promise<{ job_id: string }> {
  if (target !== "panel") {
    throw new ApiError(`Unsupported update target: ${String(target)}`, 400);
  }
  if (USE_DEMO) return { job_id: `panel-${action}-demo` };
  if (isDesktopRuntime()) {
    throw new ApiError("macOS App 不提供 Linux 面板更新任务，请使用 DMG 或 Release 资产更新。", 501);
  }
  return apiFetch<{ job_id: string }>(panelUpdateRoutes[action], { method: "POST", csrfToken });
}

const probeJobRoutes: Record<ProbeJobAction, { path: string; body?: Record<string, unknown> }> = {
  "bark-test": { path: "/api/probe/bark/test" },
  "hooks-install": { path: "/api/probe/hooks/install" },
  "logs-db-dry-run": { path: "/api/probe/logs-db/maintain", body: { dry_run: true } },
  "logs-db-execute": { path: "/api/probe/logs-db/maintain", body: { dry_run: false, compact: false } }
};

export async function startProbeJob(action: ProbeJobAction, csrfToken?: string | null): Promise<{ job_id: string }> {
  if (USE_DEMO) return { job_id: `probe-${action}-demo` };
  const route = probeJobRoutes[action];
  return apiFetch<{ job_id: string }>(route.path, {
    method: "POST",
    csrfToken,
    body: route.body ? JSON.stringify(route.body) : undefined
  });
}

export type ThreadSendPayload = {
  message: string;
  attachments?: string[];
  model?: string | null;
  service_tier?: string | null;
  reasoning_effort?: string | null;
  cwd?: string | null;
  permission_profile?: string | null;
  approval_policy?: string | null;
  sandbox_mode?: string | null;
  network_access?: boolean | null;
  collaboration_mode?: string | null;
};

export async function uploadFiles(files: File[], csrfToken?: string | null): Promise<UploadOutcome> {
  if (USE_DEMO) {
    return {
      files: files.map((file, index) => ({
        id: `upload-demo-${Date.now()}-${index}`,
        name: file.name,
        mime: file.type || "application/octet-stream",
        size: file.size,
        sha256: "demo",
        kind: file.type.startsWith("image/") ? "image" : file.name.endsWith(".md") ? "markdown" : "text",
        status: "ready"
      }))
    };
  }
  if (isDesktopRuntime()) {
    const uploads: DesktopApiUpload[] = await Promise.all(files.map(async (file) => ({
      name: file.name,
      mime: file.type || "application/octet-stream",
      bytes: Array.from(new Uint8Array(await file.arrayBuffer()))
    })));
    return runtimeRpc<UploadOutcome>("desktopUploadFiles", { files: uploads });
  }
  const form = new FormData();
  for (const file of files) {
    form.append("files", file, file.name);
  }
  return apiFetch<UploadOutcome>("/api/uploads", {
    method: "POST",
    csrfToken,
    body: form,
    skipContentType: true
  });
}

export async function deleteUpload(id: string, csrfToken?: string | null): Promise<{ ok: boolean; deleted: boolean }> {
  if (USE_DEMO) return { ok: true, deleted: true };
  return apiFetch<{ ok: boolean; deleted: boolean }>(`/api/uploads/${id}`, {
    method: "DELETE",
    csrfToken
  });
}

export async function createThread(payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return { bridge: false, thread_id: "019e-new-demo", turn_id: "turn-demo", fallback: true, message: "已提交给 Codex" };
  return apiFetch<BridgeActionResult>("/api/threads", {
    method: "POST",
    csrfToken,
    body: JSON.stringify(payload)
  });
}

export async function sendMessage(threadId: string, payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return { bridge: false, thread_id: threadId, turn_id: "turn-demo", fallback: true, message: "已提交给 Codex" };
  return apiFetch<BridgeActionResult>(`/api/threads/${threadId}/messages`, {
    method: "POST",
    csrfToken,
    body: JSON.stringify(payload)
  });
}

export async function steerThread(threadId: string, payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return { bridge: false, thread_id: threadId, turn_id: "turn-demo", fallback: true, message: "已提交给 Codex" };
  return apiFetch<BridgeActionResult>(`/api/threads/${threadId}/steer`, {
    method: "POST",
    csrfToken,
    body: JSON.stringify(payload)
  });
}

export async function listFollowUps(threadId: string): Promise<FollowUpQueueState> {
  if (USE_DEMO) return { items: [] };
  return apiFetch<FollowUpQueueState>(`/api/threads/${threadId}/follow-ups`);
}

export async function enqueueFollowUp(threadId: string, payload: ThreadSendPayload, csrfToken?: string | null): Promise<FollowUpQueueItem> {
  if (USE_DEMO) {
    return {
      id: `follow-up-${Date.now()}`,
      thread_id: threadId,
      status: "pending",
      message: payload.message,
      options: payload,
      created_at: Math.floor(Date.now() / 1000)
    };
  }
  return apiFetch<FollowUpQueueItem>(`/api/threads/${threadId}/follow-ups`, {
    method: "POST",
    csrfToken,
    body: JSON.stringify(payload)
  });
}

export async function cancelFollowUp(threadId: string, followUpId: string, csrfToken?: string | null): Promise<{ ok: boolean }> {
  if (USE_DEMO) return { ok: true };
  return apiFetch<{ ok: boolean }>(`/api/threads/${threadId}/follow-ups/${followUpId}/cancel`, {
    method: "POST",
    csrfToken
  });
}

export async function stopThread(threadId: string, payload: { turn_id?: string | null; job_id?: string | null }, csrfToken?: string | null) {
  if (USE_DEMO) return { ok: true };
  return apiFetch(`/api/threads/${threadId}/stop`, {
    method: "POST",
    csrfToken,
    body: JSON.stringify(payload)
  });
}

export async function archiveThread(threadId: string, csrfToken?: string | null) {
  return apiFetch(`/api/threads/${threadId}/archive`, { method: "POST", csrfToken });
}

export async function restoreThread(threadId: string, csrfToken?: string | null) {
  return apiFetch(`/api/threads/${threadId}/restore`, { method: "POST", csrfToken });
}

export async function renameThread(threadId: string, name: string, csrfToken?: string | null) {
  return apiFetch(`/api/threads/${threadId}/rename`, {
    method: "POST",
    csrfToken,
    body: JSON.stringify({ name })
  });
}

export async function forkThread(threadId: string, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (isDesktopRuntime()) {
    void csrfToken;
    throw new ApiError("macOS App 当前不支持 Fork 操作", 501);
  }
  return apiFetch<BridgeActionResult>(`/api/threads/${threadId}/fork`, { method: "POST", csrfToken });
}

export async function answerElicitation(threadId: string, answers: Record<string, string[]>, csrfToken?: string | null): Promise<BridgeActionResult> {
  return apiFetch<BridgeActionResult>(`/api/threads/${threadId}/elicitation`, {
    method: "POST",
    csrfToken,
    body: JSON.stringify({ answers })
  });
}

export async function acceptPlan(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  return apiFetch<BridgeActionResult>(`/api/threads/${threadId}/plan/accept`, {
    method: "POST",
    csrfToken,
    body: JSON.stringify(payload)
  });
}

export async function revisePlan(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null; instructions: string },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  return apiFetch<BridgeActionResult>(`/api/threads/${threadId}/plan/revise`, {
    method: "POST",
    csrfToken,
    body: JSON.stringify(payload)
  });
}

export async function answerApproval(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null; request_id?: string | null; decision: string },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  if (isDesktopRuntime()) {
    void payload;
    void csrfToken;
    throw new ApiError("macOS App 当前不支持 Approval 操作", 501);
  }
  return apiFetch<BridgeActionResult>(`/api/threads/${threadId}/approval`, {
    method: "POST",
    csrfToken,
    body: JSON.stringify(payload)
  });
}

export async function changePassword(current_password: string, new_password: string, csrfToken?: string | null) {
  return apiFetch("/api/security/password", {
    method: "POST",
    csrfToken,
    body: JSON.stringify({ current_password, new_password })
  });
}

export async function listModels(): Promise<OptionalResult<CodexModel[]>> {
  if (USE_DEMO) {
    return {
      available: true,
      data: [
        { id: "gpt-5.5", label: "GPT-5.5", default: true },
        { id: "gpt-5.5-codex", label: "GPT-5.5 Codex", service_tiers: [{ id: "priority", name: "Fast", description: "1.5x speed" }], default_service_tier: "default" },
        { id: "gpt-5.4-mini", label: "GPT-5.4 mini" },
        { id: "gpt-5.3-codex-spark", label: "GPT-5.3 Codex Spark" },
        { id: "o3", label: "o3" }
      ]
    };
  }
  const result = await optionalApiFetchFirst<unknown[]>([
    "/api/codex/models",
    "/api/config/models",
    "/api/models"
  ]);
  return result.available ? { available: true, data: normalizeModels(result.data ?? []) } : result as OptionalResult<CodexModel[]>;
}

export async function listPermissionProfiles(): Promise<OptionalResult<PermissionProfile[]>> {
  if (USE_DEMO) {
    return {
      available: true,
      data: [
        { id: "danger-full-access", label: "Danger full access", sandbox_mode: "danger-full-access", approval_policy: "never", network_access: true, default: true },
        { id: "workspace-write", label: "Workspace write", sandbox_mode: "workspace-write", approval_policy: "on-request", network_access: true },
        { id: "read-only", label: "Read only", sandbox_mode: "read-only", approval_policy: "on-request", network_access: false }
      ]
    };
  }
  const result = await optionalApiFetchFirst<unknown[]>([
    "/api/codex/permission-profiles",
    "/api/codex/permissionProfiles",
    "/api/config/permission-profiles"
  ]);
  return result.available ? { available: true, data: normalizePermissionProfiles(result.data ?? []) } : result as OptionalResult<PermissionProfile[]>;
}

export async function getCodexConfig(): Promise<OptionalResult<CodexConfig>> {
  if (USE_DEMO) {
    return {
      available: true,
      data: {
        model: "gpt-5.5",
        service_tier: null,
        reasoning_effort: "xhigh",
        cwd: "/home/ubuntu/codex-workspace",
        permission_profile: "danger-full-access",
        approval_policy: "never",
        sandbox_mode: "danger-full-access",
        network_access: true,
        collaboration_mode: null
      }
    };
  }
  return optionalApiFetch<CodexConfig>("/api/codex/config");
}

export async function getCodexGoal(threadId: string): Promise<CodexGoal> {
  if (USE_DEMO) return demoCodexGoal(threadId);
  const params = new URLSearchParams({ thread_id: threadId });
  return apiFetch<CodexGoal>(`/api/codex/goal?${params.toString()}`);
}

export async function saveCodexGoal(threadId: string, goal: CodexGoalSaveInput, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return {
      ...demoCodexGoal(threadId),
      enabled: true,
      objective: goal.objective.trim(),
      token_budget: goal.token_budget ?? null,
      status: "active"
    };
  }
  return apiFetch<CodexGoal>("/api/codex/goal", {
    method: "POST",
    csrfToken,
    body: JSON.stringify({
      thread_id: threadId,
      objective: goal.objective,
      token_budget: goal.token_budget ?? null
    })
  });
}

export async function clearCodexGoal(threadId: string, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return {
      ...demoCodexGoal(threadId),
      enabled: false,
      objective: null,
      token_budget: null,
      status: "cleared"
    };
  }
  return updateCodexGoalStatus("/api/codex/goal/clear", threadId, csrfToken);
}

export async function pauseCodexGoal(threadId: string, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return {
      ...demoCodexGoal(threadId),
      enabled: true,
      status: "paused"
    };
  }
  return updateCodexGoalStatus("/api/codex/goal/pause", threadId, csrfToken);
}

export async function resumeCodexGoal(threadId: string, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return {
      ...demoCodexGoal(threadId),
      enabled: true,
      status: "active"
    };
  }
  return updateCodexGoalStatus("/api/codex/goal/resume", threadId, csrfToken);
}

function updateCodexGoalStatus(path: string, threadId: string, csrfToken?: string | null): Promise<CodexGoal> {
  return apiFetch<CodexGoal>(path, {
    method: "POST",
    csrfToken,
    body: JSON.stringify({ thread_id: threadId })
  });
}

export function subscribeThreadEvents(
  threadId: string,
  handlers: { onBlock?: (block: MessageBlock, threadId: string) => void; onBlocks?: (blocks: MessageBlock[], threadId: string) => void; onSummary?: (summary: ThreadSummary, threadId: string) => void; onError?: (message: string, threadId: string) => void }
): () => void {
  if (isDesktopRuntime()) return () => {};
  if (USE_DEMO) return () => {};
  const source = new EventSource(buildApiPath(`/api/threads/${threadId}/events`), { withCredentials: true });
  let pendingBlocks: MessageBlock[] = [];
  let flushTimer: ReturnType<typeof setTimeout> | null = null;
  const flushBlocks = () => {
    flushTimer = null;
    if (!pendingBlocks.length) return;
    const blocks = pendingBlocks;
    pendingBlocks = [];
    handlers.onBlocks?.(blocks, threadId);
  };
  source.addEventListener("block", (event) => {
    const block = JSON.parse((event as MessageEvent).data) as MessageBlock;
    handlers.onBlock?.(block, threadId);
    if (handlers.onBlocks) {
      pendingBlocks.push(block);
      if (!flushTimer) flushTimer = setTimeout(flushBlocks, 100);
    }
  });
  source.addEventListener("summary", (event) => handlers.onSummary?.(JSON.parse((event as MessageEvent).data), threadId));
  source.addEventListener("error", (event) => {
    const data = (event as MessageEvent).data;
    handlers.onError?.(data ? String(data) : "stream disconnected", threadId);
  });
  return () => {
    if (flushTimer) {
      clearTimeout(flushTimer);
      flushBlocks();
    }
    source.close();
  };
}

export async function listJobs(): Promise<JobRecord[]> {
  if (USE_DEMO) {
    return [
      { id: "probe-bark-demo", kind: "probe_bark_test", status: "succeeded", title: "Probe Bark 测试", started_at: 1780731706, finished_at: 1780731710, exit_code: 0, output: "POST https://api.day.app\nHTTP 200\nBark push accepted" },
      { id: "probe-logs-demo", kind: "probe_logs_db_maintain", status: "succeeded", title: "Probe logs-db dry-run", started_at: 1780731666, finished_at: 1780731672, exit_code: 0, output: "dry_run=true\nwould_delete_probe_events=42\ncompact=false" },
      { id: "job-demo", kind: "panel_update_precheck", status: "succeeded", title: "Panel update precheck", started_at: 1780731606, output: "panel version check\nintegrity_check: ok" },
      { id: "job-failed-demo", kind: "panel_update", status: "failed", title: "Panel update", started_at: 1780731206, finished_at: 1780731252, exit_code: 1, output: "download release asset\nverify checksum", error: "release asset checksum mismatch", analysis: "Downloaded asset digest did not match release metadata.", explanation: "Retry after confirming the release asset has finished publishing." }
    ];
  }
  return apiFetch<JobRecord[]>("/api/jobs?limit=30");
}

export async function getJob(id: string): Promise<JobRecord> {
  if (USE_DEMO) {
    return (await listJobs()).find((job) => job.id === id) ?? {
      id,
      kind: "unknown",
      status: "failed",
      title: id,
      started_at: Date.now() / 1000,
      output: "",
      error: "demo job not found"
    };
  }
  return apiFetch<JobRecord>(`/api/jobs/${id}`);
}

function normalizeModels(value: unknown): CodexModel[] {
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

function normalizePermissionProfiles(value: unknown): PermissionProfile[] {
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

function demoProbeStatus(): ProbeStatus {
  return {
    label: "Probe",
    enabled: true,
    available: true,
    platform: "linux",
    service_kind: "systemd",
    service_name: "nexushub",
    flavor: "builtin",
    hook_status: "managed",
    bark_status: "not_configured",
    logs_db_status: "maintenance_ready",
    recent_event_count: 1,
    running_count: 1,
    reply_needed_count: 1,
    recoverable_count: 0,
    running_threads: [
      { id: "019e8c1f-demo", title: "活动库审阅链路", status: "Running", message_count: 18, latest_message: "正在逐项审计脚本输出。" }
    ],
    reply_needed_threads: [
      { id: "019e95a0-demo", title: "Plan Mode 修复", status: "ReplyNeeded", message_count: 7, latest_message: "等待确认" }
    ],
    recoverable_threads: [],
    lifecycle_status: "ok",
    doctor_status: "ok",
    runtime_version: "demo",
    config_path: "/opt/nexushub/config.toml",
    codex_home: "/root/.codex",
    configured_codex_home: "/root/.codex",
    resolved_codex_home: "/root/.codex",
    codex_home_source: "config",
    logs_db_source: "resolved_codex_home",
    host_label: "43.155.235.227",
    snapshot_age_seconds: 0,
    is_refreshing: false,
    snapshot_status: "cached"
  };
}

function demoProbeSettings(): ProbeSettings {
  return {
    codex: {
      home: "/root/.codex",
      configured_codex_home: "/root/.codex",
      resolved_codex_home: "/root/.codex",
      codex_home_source: "config",
      logs_db_source: "resolved_codex_home",
      discovery_warnings: [],
      workspace: "/home/ubuntu/codex-workspace",
      host_label: "43.155.235.227"
    },
    probe: {
      enabled: true,
      poll_seconds: 15,
      recent_limit: 50
    },
    notifications: {
      enabled: false,
      device_key_configured: false,
      server_url: "https://api.day.app",
      group: "NexusHub"
    },
    logs_db: {
      enabled: true,
      retention_days: 2,
      maintenance_interval_hours: 6,
      maintain_on_codex_exit: true,
      codex_exit_grace_seconds: 5,
      codex_exit_max_wait_seconds: 1800,
      delete_chunk_rows: 5000,
      max_delete_rows_per_run: 100000,
      busy_timeout_ms: 500,
      auto_compact_when_codex_closed: true,
      compact_interval_hours: 24,
      compact_min_freelist_mb: 256,
      compact_min_freelist_ratio_percent: 20,
      minimum_free_space_mb: 1024
    }
  };
}

function demoCodexGoal(threadId: string): CodexGoal {
  return {
    available: true,
    enabled: threadId === "019e95a0-demo",
    objective: threadId === "019e95a0-demo" ? "修复 Plan Mode 右栏交互" : null,
    token_budget: threadId === "019e95a0-demo" ? 18000 : null,
    status: threadId === "019e95a0-demo" ? "active" : "idle",
    raw: { source: "demo", thread_id: threadId }
  };
}

function demoThreads(status: string, q: string): ThreadSummary[] {
  const threads: ThreadSummary[] = [
    { id: "019e8c1f-demo", title: "活动库审阅链路", status: "Running", message_count: 18, latest_message: "正在逐项审计脚本输出。", updated_at: new Date().toISOString() },
    { id: "019e95a0-demo", title: "Plan Mode 修复", status: "ReplyNeeded", message_count: 7, latest_message: "等待确认", updated_at: new Date().toISOString() },
    { id: "019e5281-demo", title: "检查仓库状态", status: "Recent", message_count: 3, latest_message: "仓库状态干净。", updated_at: new Date().toISOString() },
    { id: "019e42aa-demo", title: "旧归档线程", status: "Archived", message_count: 2, latest_message: "已归档。", updated_at: new Date(Date.now() - 86400000).toISOString() }
  ];
  return threads.filter((thread) => (status === "all" || status === threadStatusParam(thread.status)) && (!q || `${thread.title} ${thread.id}`.toLowerCase().includes(q.toLowerCase())));
}

function threadStatusParam(status: ThreadSummary["status"]): string {
  if (status === "ReplyNeeded") return "reply-needed";
  return status.toLowerCase();
}
