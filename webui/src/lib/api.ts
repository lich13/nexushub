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
  UpdateStatus,
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
  createRuntimeThreadEventSource,
  desktopSessionUser,
  runtimeDispatch,
  runtimeRpc,
  runtimeValue,
  uploadRuntimeFiles
} from "./api/transport";
import {
  runtimeCapabilities,
  runtimeCapabilitiesForRuntime,
  runtimeCapabilitiesFromSystemStatus,
  type RuntimeCapabilityMatrix
} from "./domain/capabilities";
import {
  demoDesktopPlatformOverview,
  demoDesktopSecurity,
  demoDesktopSystemStatus,
  demoWebPlatformOverview,
  demoWebSecurity,
  demoWebSystemStatus
} from "./domain/demoCore";

const USE_DEMO = import.meta.env.DEV && import.meta.env.VITE_USE_REAL_API !== "1";

export class ApiError extends Error {
  constructor(message: string, readonly status: number) {
    super(message);
    this.name = "ApiError";
  }
}

function optionalString(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}

function objectValue(value: unknown): Record<string, unknown> {
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

function normalizeProbeRuntimePayload(value: unknown): Record<string, unknown> {
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

function isMissingEndpoint(error: unknown): boolean {
  return error instanceof RuntimeUnavailableError || error instanceof ApiError && [404, 405, 501].includes(error.status);
}

function normalizeOptionalResult<T>(payload: unknown): OptionalResult<T> {
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

export {
  runtimeCapabilities,
  runtimeCapabilitiesForRuntime,
  runtimeCapabilitiesFromSystemStatus,
  type RuntimeCapabilityMatrix
};

export function desktopRuntimeSessionUser(): SessionUser {
  return desktopSessionUser();
}

export async function getPublicSettings(): Promise<PublicSettings> {
  if (USE_DEMO) {
    return { site_name: "NexusHub", turnstile_enabled: false, turnstile_required: false, turnstile_site_key: "", turnstile_action: "login", admin_configured: true };
  }
  return runtimeDispatch<PublicSettings>({
    command: "getPublicSettings",
    desktopFallback: () => ({ site_name: "NexusHub", turnstile_enabled: false, turnstile_required: false, turnstile_site_key: "", turnstile_action: "login", admin_configured: true })
  });
}

export async function login(username: string, password: string, turnstileToken?: string | null): Promise<SessionUser> {
  if (USE_DEMO) {
    return runtimeValue<SessionUser>({
      web: { id: "dev", username, csrf_token: "dev-csrf" },
      desktop: () => desktopSessionUser()
    });
  }
  return runtimeDispatch<SessionUser>({
    command: "login",
    webArgs: { username, password, turnstile_token: turnstileToken ?? null },
    desktopFallback: () => desktopSessionUser()
  });
}

export async function logout(csrfToken?: string | null): Promise<void> {
  if (USE_DEMO) return;
  await runtimeDispatch<void>({
    command: "logout",
    webArgs: { csrfToken },
    desktopFallback: () => undefined
  });
}

export async function me(): Promise<SessionUser> {
  if (USE_DEMO) {
    return runtimeValue<SessionUser>({
      web: { id: "dev", username: "admin", csrf_token: "dev-csrf" },
      desktop: () => desktopSessionUser()
    });
  }
  return runtimeDispatch<SessionUser>({
    command: "me",
    desktopFallback: () => desktopSessionUser()
  });
}

export async function listThreads(status: string, q: string): Promise<ThreadSummary[]> {
  if (USE_DEMO) return demoThreads(status, q);
  return runtimeDispatch<ThreadSummary[]>({
    command: "listThreads",
    webArgs: { status, q, limit: 120 },
    desktopCommand: "desktop_threads",
    desktopArgs: { request: { status, query: q, limit: 120 } }
  });
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
  return runtimeDispatch<ThreadDetail>({
    command: "getThread",
    webArgs: { id, options },
    desktopCommand: "desktop_thread_detail",
    desktopArgs: desktopThreadRequest(id, options)
  });
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
  return runtimeDispatch<ThreadBlockPage>({
    command: "getThreadBlocks",
    webArgs: { id, options },
    desktopCommand: "desktop_thread_blocks",
    desktopArgs: { request: { id, limit: options.limit, before: options.before } }
  });
}

export async function getSystemStatus(): Promise<SystemStatus> {
  if (USE_DEMO) {
    return demoSystemStatus();
  }
  return runtimeRpc<SystemStatus>("getSystemStatus");
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
  return runtimeRpc<SystemVersion>("getSystemVersion");
}

export async function getUpdateStatus(): Promise<UpdateStatus> {
  if (USE_DEMO) {
    return runtimeValue({
      desktop: {
        current_version: "0.1.100",
        latest_version: "v0.1.103",
        update_available: true,
        channel: "stable",
        method: "macos_tauri_updater",
        state: "idle",
        failure_category: null,
        recommended_action: "Confirm install in the Tauri updater after signature verification.",
        capabilities: ["check", "confirm_install", "job_history", "signature_verification", "restart_after_install"]
      },
      web: {
        current_version: "0.1.100",
        latest_version: "v0.1.103",
        update_available: true,
        channel: "stable",
        method: "linux_systemd_job",
        state: "idle",
        failure_category: null,
        recommended_action: "/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest",
        capabilities: ["check", "confirm_install", "job_history", "sha256_verification", "systemd_health_check", "rollback", "prune_backups"]
      }
    });
  }
  return runtimeDispatch<UpdateStatus>({
    command: "getUpdateStatus",
    desktopCommand: "desktop_update_status"
  });
}

export async function getSecurity(): Promise<SecuritySettings> {
  if (USE_DEMO) {
    return demoSecurity();
  }
  return runtimeRpc<SecuritySettings>("getSecurity");
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
  return runtimeRpc<AgentProviderInfo[]>("listProviders");
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
  try {
    return normalizeOptionalResult(await runtimeRpc<ClaudeOverview>("getClaudeCodeOverview"));
  } catch (error) {
    if (isMissingEndpoint(error)) {
      return { available: false, error: error instanceof Error ? error.message : String(error) };
    }
    throw error;
  }
}

export async function getPlatformOverview(): Promise<PlatformOverview> {
  if (USE_DEMO) {
    return demoPlatformOverview();
  }
  return runtimeRpc<PlatformOverview>("getPlatformOverview");
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
  return runtimeRpc<PluginInfo[]>("listPlugins");
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
  return normalizeOptionalResult<ProbeStatus>(await runtimeRpc<ProbeStatus | OptionalResult<ProbeStatus>>("getProbeStatus"));
}

export async function getProbeSettings(): Promise<OptionalResult<ProbeSettings>> {
  if (USE_DEMO) {
    return {
      available: true,
      data: demoProbeSettings()
    };
  }
  const payload = await runtimeDispatch<ProbeSettings | OptionalResult<ProbeSettings>>({
    command: "getProbeSettings",
    desktopCommand: "desktop_probe_settings"
  });
  const result = normalizeOptionalResult<ProbeSettings>(payload);
  return result.available
    ? { ...result, data: normalizeProbeRuntimePayload(result.data) as ProbeSettings }
    : result;
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

function codexGoalWebArgs(threadId: string, csrfToken?: string | null) {
  return { thread_id: threadId, csrfToken };
}

function codexGoalThreadArg(threadId: string) {
  return { threadId };
}

function codexGoalSaveWebArgs(threadId: string, goal: CodexGoalSaveInput, csrfToken?: string | null) {
  return {
    thread_id: threadId,
    objective: goal.objective,
    token_budget: goal.token_budget ?? null,
    csrfToken
  };
}

function codexGoalSaveDesktopArgs(threadId: string, goal: CodexGoalSaveInput) {
  return {
    request: {
      threadId,
      objective: goal.objective,
      tokenBudget: goal.token_budget ?? null
    }
  };
}

function desktopThreadRequest(id: string, options: ThreadDetailOptions = {}) {
  return {
    request: {
      id,
      limit: options.limit,
      before: options.before,
      full: options.full
    }
  };
}

function desktopThreadIdArg(threadId: string) {
  return { threadId };
}

export async function saveProbeSettings(settings: Partial<ProbeSettings>, csrfToken?: string | null): Promise<ProbeSettings> {
  if (USE_DEMO) return { ...demoProbeSettings(), ...settings } as ProbeSettings;
  const normalizedSettings = normalizeProbeSettingsSavePayload(settings);
  const payload = await runtimeDispatch<ProbeSettings>({
    command: "saveProbeSettings",
    webArgs: {
      settings: normalizedSettings,
      csrfToken
    },
    desktopCommand: "desktop_probe_save_settings",
    desktopArgs: { request: normalizedSettings }
  });
  return normalizeProbeRuntimePayload(payload) as ProbeSettings;
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
  const payload = await runtimeRpc<ProbeLogsDbStatus | OptionalResult<ProbeLogsDbStatus>>("getProbeLogsDbStatus");
  const result = normalizeOptionalResult<ProbeLogsDbStatus>(payload);
  return result.available
    ? { ...result, data: normalizeProbeRuntimePayload(result.data) as ProbeLogsDbStatus }
    : result;
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
  return normalizeOptionalResult<ProbeEventsResponse>(await runtimeRpc<ProbeEventsResponse | OptionalResult<ProbeEventsResponse>>("getProbeEvents", { limit }));
}

export async function saveSecurity(settings: Partial<SecuritySettings> & { turnstile_secret_key?: string }, csrfToken?: string | null) {
  return runtimeRpc<SecuritySettings>("saveSecurity", { settings, csrfToken });
}

export async function dryRunArchiveDelete(csrfToken?: string | null): Promise<ArchiveDeletePlan> {
  if (USE_DEMO) {
    return { total_threads: 42, active_threads: 31, archived_threads: 11, session_index_lines: 44, rollout_files: 39, archived_ids: ["019e-demo-a", "019e-demo-b"], integrity: "ok" };
  }
  return runtimeDispatch<ArchiveDeletePlan>({
    command: "dryRunArchiveDelete",
    webArgs: { csrfToken },
    desktopCommand: "desktop_archive_delete_dry_run",
    desktopArgs: undefined
  });
}

export async function startArchiveDelete(csrfToken?: string | null): Promise<ArchiveDeleteResult> {
  return runtimeDispatch<ArchiveDeleteResult>({
    command: "startArchiveDelete",
    webArgs: { confirmed: true, csrfToken },
    desktopCommand: "desktop_archive_delete_execute",
    desktopArgs: undefined
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
  return runtimeDispatch<HiddenThreadDeletePlan>({
    command: "dryRunHiddenThreadDelete",
    webArgs: { csrfToken },
    desktopCommand: "desktop_hidden_delete_dry_run",
    desktopArgs: undefined
  });
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
  return runtimeDispatch<HiddenThreadDeleteResult>({
    command: "startHiddenThreadDelete",
    webArgs: { confirmed: true, csrfToken },
    desktopCommand: "desktop_hidden_delete_execute",
    desktopArgs: undefined
  });
}

export type UnifiedUpdateAction = "check" | "install" | "prune";

function jobIdFromRuntimeResult(result: { job_id?: string | null; jobId?: string | null }, fallback: string): { job_id: string } {
  return { job_id: result.job_id ?? result.jobId ?? fallback };
}

export type UpdateActionResult = {
  job_id: string;
  status?: UpdateStatus;
};

export async function runUpdateAction(
  action: UnifiedUpdateAction,
  csrfToken?: string | null,
  capabilities: RuntimeCapabilityMatrix = runtimeCapabilities(),
): Promise<UpdateActionResult> {
  if (USE_DEMO) return { job_id: `update-${action}-demo` };
  if (action === "prune" && !capabilities.linuxBackupPrune) {
    throw new RuntimeUnavailableError("当前运行时不支持备份清理动作", "Desktop backup prune command is not implemented");
  }
  const desktopCommand = action === "install"
    ? "install_update_and_restart"
    : action === "check"
      ? "check_update_status"
      : undefined;
  const result = await runtimeDispatch<{ job_id?: string | null; jobId?: string | null; status?: UpdateStatus }>({
    command: "runUpdateAction",
    webArgs: { action, csrfToken },
    desktopCommand,
  });
  return {
    ...jobIdFromRuntimeResult(result, `update-${action}`),
    ...(result.status ? { status: result.status } : {})
  };
}

export async function startProbeJob(action: ProbeJobAction, csrfToken?: string | null): Promise<{ job_id: string }> {
  if (USE_DEMO) return { job_id: `probe-${action}-demo` };
  const result = await runtimeDispatch<{ job_id?: string | null; jobId?: string | null }>({
    command: "startProbeJob",
    webArgs: { action, csrfToken },
    desktopCommand: action === "bark-test"
      ? "desktop_probe_bark_test"
      : action === "hooks-install"
        ? "desktop_probe_hooks_install"
        : action === "logs-db-dry-run" || action === "logs-db-execute"
          ? "desktop_probe_logs_db_maintain"
          : undefined,
    desktopArgs: action === "logs-db-dry-run"
      ? { request: { dryRun: true, compact: false } }
      : action === "logs-db-execute"
        ? { request: { dryRun: false, compact: true } }
        : undefined
  });
  return jobIdFromRuntimeResult(result, `probe-${action}`);
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
  return uploadRuntimeFiles<UploadOutcome>(files, csrfToken);
}

export async function deleteUpload(id: string, csrfToken?: string | null): Promise<{ ok: boolean; deleted: boolean }> {
  if (USE_DEMO) return { ok: true, deleted: true };
  return runtimeDispatch<{ ok: boolean; deleted: boolean }>({
    command: "deleteUpload",
    webArgs: { id, csrfToken },
    desktopCommand: "desktop_delete_upload",
    desktopArgs: { id }
  });
}

export async function createThread(payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return { bridge: false, thread_id: "019e-new-demo", turn_id: "turn-demo", fallback: true, message: "已提交给 Codex" };
  return runtimeDispatch<BridgeActionResult>({
    command: "createThread",
    webArgs: { payload, csrfToken },
    desktopCommand: "desktop_send_message",
    desktopArgs: { request: { ...payload, threadId: null } }
  });
}

export async function sendMessage(threadId: string, payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return { bridge: false, thread_id: threadId, turn_id: "turn-demo", fallback: true, message: "已提交给 Codex" };
  return runtimeDispatch<BridgeActionResult>({
    command: "sendMessage",
    webArgs: { threadId, payload, csrfToken },
    desktopCommand: "desktop_send_message",
    desktopArgs: { request: { ...payload, threadId } }
  });
}

export async function steerThread(threadId: string, payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return { bridge: false, thread_id: threadId, turn_id: "turn-demo", fallback: true, message: "已提交给 Codex" };
  return runtimeDispatch<BridgeActionResult>({
    command: "steerThread",
    webArgs: { threadId, payload, csrfToken },
    desktopCommand: "desktop_continue_thread",
    desktopArgs: { request: { ...payload, threadId } }
  });
}

export async function listFollowUps(threadId: string): Promise<FollowUpQueueState> {
  if (USE_DEMO) return { items: [] };
  const result = await runtimeDispatch<FollowUpQueueState | FollowUpQueueItem[]>({
    command: "listFollowUps",
    webArgs: { threadId },
    desktopCommand: "desktop_list_followups",
    desktopArgs: { request: { threadId, limit: 20 } }
  });
  return Array.isArray(result) ? { items: result } : result;
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
  return runtimeDispatch<FollowUpQueueItem>({
    command: "enqueueFollowUp",
    webArgs: { threadId, payload, csrfToken },
    desktopCommand: "desktop_enqueue_followup",
    desktopArgs: { request: { ...payload, threadId } }
  });
}

export async function cancelFollowUp(threadId: string, followUpId: string, csrfToken?: string | null): Promise<{ ok: boolean }> {
  if (USE_DEMO) return { ok: true };
  return runtimeDispatch<{ ok: boolean }>({
    command: "cancelFollowUp",
    webArgs: { threadId, followUpId, csrfToken },
    desktopCommand: "desktop_cancel_followup",
    desktopArgs: { request: { threadId, followUpId } }
  });
}

export async function stopThread(threadId: string, payload: { turn_id?: string | null; job_id?: string | null }, csrfToken?: string | null) {
  if (USE_DEMO) return { ok: true };
  return runtimeDispatch({
    command: "stopThread",
    webArgs: { threadId, payload, csrfToken },
    desktopCommand: "desktop_stop_thread",
    desktopArgs: { request: { threadId, turn_id: payload.turn_id, job_id: payload.job_id } }
  });
}

export async function archiveThread(threadId: string, csrfToken?: string | null) {
  return runtimeDispatch({
    command: "archiveThread",
    webArgs: { threadId, csrfToken },
    desktopCommand: "desktop_archive_thread",
    desktopArgs: desktopThreadIdArg(threadId)
  });
}

export async function restoreThread(threadId: string, csrfToken?: string | null) {
  return runtimeDispatch({
    command: "restoreThread",
    webArgs: { threadId, csrfToken },
    desktopCommand: "desktop_restore_thread",
    desktopArgs: desktopThreadIdArg(threadId)
  });
}

export async function renameThread(threadId: string, name: string, csrfToken?: string | null) {
  return runtimeDispatch({
    command: "renameThread",
    webArgs: { threadId, name, csrfToken },
    desktopCommand: "desktop_rename_thread",
    desktopArgs: { threadId, name }
  });
}

export async function forkThread(threadId: string, csrfToken?: string | null): Promise<BridgeActionResult> {
  return runtimeDispatch<BridgeActionResult>({
    command: "forkThread",
    webArgs: { threadId, csrfToken },
    desktopUnavailable: "Desktop fork command is not implemented"
  });
}

export async function answerElicitation(threadId: string, answers: Record<string, string[]>, csrfToken?: string | null): Promise<BridgeActionResult> {
  return runtimeDispatch<BridgeActionResult>({
    command: "answerElicitation",
    webArgs: { threadId, answers, csrfToken },
    desktopCommand: "desktop_answer_elicitation",
    desktopArgs: { request: { threadId, answers } }
  });
}

export async function acceptPlan(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  return runtimeDispatch<BridgeActionResult>({
    command: "acceptPlan",
    webArgs: { threadId, payload, csrfToken },
    desktopCommand: "desktop_plan_accept",
    desktopArgs: { request: { threadId, turn_id: payload.turn_id, item_id: payload.item_id } }
  });
}

export async function revisePlan(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null; instructions: string },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  return runtimeDispatch<BridgeActionResult>({
    command: "revisePlan",
    webArgs: { threadId, payload, csrfToken },
    desktopCommand: "desktop_plan_revise",
    desktopArgs: { request: { threadId, turn_id: payload.turn_id, item_id: payload.item_id, instructions: payload.instructions } }
  });
}

export async function answerApproval(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null; request_id?: string | null; decision: string },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  return runtimeDispatch<BridgeActionResult>({
    command: "answerApproval",
    webArgs: { threadId, payload, csrfToken },
    desktopUnavailable: "Desktop approval command is not implemented"
  });
}

export async function changePassword(current_password: string, new_password: string, csrfToken?: string | null) {
  return runtimeRpc("changePassword", { current_password, new_password, csrfToken });
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
  try {
    const result = normalizeOptionalResult(await runtimeRpc<unknown[]>("listModels"));
    return result.available ? { available: true, data: normalizeModels(result.data ?? []) } : result as OptionalResult<CodexModel[]>;
  } catch (error) {
    if (isMissingEndpoint(error)) {
      return { available: false, error: error instanceof Error ? error.message : String(error) };
    }
    throw error;
  }
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
  try {
    const result = normalizeOptionalResult(await runtimeRpc<unknown[]>("listPermissionProfiles"));
    return result.available ? { available: true, data: normalizePermissionProfiles(result.data ?? []) } : result as OptionalResult<PermissionProfile[]>;
  } catch (error) {
    if (isMissingEndpoint(error)) {
      return { available: false, error: error instanceof Error ? error.message : String(error) };
    }
    throw error;
  }
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
  try {
    return normalizeOptionalResult(await runtimeRpc<CodexConfig>("getCodexConfig"));
  } catch (error) {
    if (isMissingEndpoint(error)) {
      return { available: false, error: error instanceof Error ? error.message : String(error) };
    }
    throw error;
  }
}

export async function getCodexGoal(threadId: string): Promise<CodexGoal> {
  if (USE_DEMO) return demoCodexGoal(threadId);
  const result = await runtimeDispatch<CodexGoal | { goal?: CodexGoal | null }>({
    command: "getCodexGoal",
    webArgs: { thread_id: threadId },
    desktopCommand: "desktop_home",
    desktopArgs: undefined
  });
  return result && typeof result === "object" && "goal" in result
    ? result.goal ?? demoCodexGoal(threadId)
    : result as CodexGoal;
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
  return runtimeDispatch<CodexGoal>({
    command: "saveCodexGoal",
    webArgs: codexGoalSaveWebArgs(threadId, goal, csrfToken),
    desktopCommand: "desktop_save_goal_command",
    desktopArgs: codexGoalSaveDesktopArgs(threadId, goal)
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
  return runtimeDispatch<CodexGoal>({
    command: "clearCodexGoal",
    webArgs: codexGoalWebArgs(threadId, csrfToken),
    desktopCommand: "desktop_clear_goal_command",
    desktopArgs: codexGoalThreadArg(threadId)
  });
}

export async function pauseCodexGoal(threadId: string, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return {
      ...demoCodexGoal(threadId),
      enabled: true,
      status: "paused"
    };
  }
  return runtimeDispatch<CodexGoal>({
    command: "pauseCodexGoal",
    webArgs: codexGoalWebArgs(threadId, csrfToken),
    desktopCommand: "desktop_pause_goal_command",
    desktopArgs: codexGoalThreadArg(threadId)
  });
}

export async function resumeCodexGoal(threadId: string, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return {
      ...demoCodexGoal(threadId),
      enabled: true,
      status: "active"
    };
  }
  return runtimeDispatch<CodexGoal>({
    command: "resumeCodexGoal",
    webArgs: codexGoalWebArgs(threadId, csrfToken),
    desktopCommand: "desktop_resume_goal_command",
    desktopArgs: codexGoalThreadArg(threadId)
  });
}

export function subscribeThreadEvents(
  threadId: string,
  handlers: { onBlock?: (block: MessageBlock, threadId: string) => void; onBlocks?: (blocks: MessageBlock[], threadId: string) => void; onSummary?: (summary: ThreadSummary, threadId: string) => void; onError?: (message: string, threadId: string) => void }
): () => void {
  if (USE_DEMO) return () => {};
  const source = createRuntimeThreadEventSource(threadId);
  if (source.unavailable) return () => {};
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
      { id: "job-demo", kind: "nexushub_update_check", status: "succeeded", title: "NexusHub update precheck", started_at: 1780731606, output: "version check\nintegrity_check: ok" },
      { id: "job-failed-demo", kind: "panel_update", status: "failed", title: "Panel update", started_at: 1780731206, finished_at: 1780731252, exit_code: 1, output: "download release asset\nverify checksum", error: "release asset checksum mismatch", analysis: "Downloaded asset digest did not match release metadata.", explanation: "Retry after confirming the release asset has finished publishing." }
    ];
  }
  const payload = await runtimeDispatch<JobRecord[] | OptionalResult<JobRecord[]>>({
    command: "listJobs",
    desktopCommand: "desktop_jobs",
    desktopArgs: { request: { limit: 30 } }
  });
  const result = normalizeOptionalResult<JobRecord[]>(payload);
  return result.available && Array.isArray(result.data) ? result.data : [];
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
  return runtimeDispatch<JobRecord>({
    command: "getJob",
    webArgs: { id },
    desktopCommand: "desktop_job_detail",
    desktopArgs: { request: { id } }
  });
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

function demoPlatformOverview(): PlatformOverview {
  return runtimeValue({
    web: demoWebPlatformOverview,
    desktop: demoDesktopPlatformOverview
  });
}

function demoSystemStatus(): SystemStatus {
  return runtimeValue({
    web: demoWebSystemStatus,
    desktop: demoDesktopSystemStatus
  });
}

function demoSecurity(): SecuritySettings {
  return runtimeValue({
    web: demoWebSecurity,
    desktop: demoDesktopSecurity
  });
}

function demoProbeStatus(): ProbeStatus {
  const platform = demoPlatformOverview();
  const system = demoSystemStatus();
  return {
    label: "Probe",
    enabled: true,
    available: true,
    platform: platform.kind,
    service_kind: platform.service_kind,
    service_name: platform.service_name,
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
    config_path: platform.config_file,
    codex_home: system.codex_home,
    configured_codex_home: system.configured_codex_home,
    resolved_codex_home: system.resolved_codex_home,
    codex_home_source: system.codex_home_source,
    logs_db_source: "resolved_codex_home",
    host_label: system.host_label,
    snapshot_age_seconds: 0,
    is_refreshing: false,
    snapshot_status: "cached"
  };
}

function demoProbeSettings(): ProbeSettings {
  const platform = demoPlatformOverview();
  const system = demoSystemStatus();
  const runtimeProbeSettings = runtimeValue({
    web: {
      logsPath: "/root/.codex/logs_2.sqlite",
      workspace: "/home/ubuntu/codex-workspace"
    },
    desktop: {
      logsPath: "~/Library/Application Support/NexusHub/logs_2.sqlite",
      workspace: "~/Documents"
    }
  });
  return {
    codex: {
      home: system.codex_home,
      configured_codex_home: system.configured_codex_home,
      resolved_codex_home: system.resolved_codex_home,
      codex_home_source: system.codex_home_source,
      logs_db_source: "resolved_codex_home",
      discovery_warnings: [],
      workspace: runtimeProbeSettings.workspace,
      host_label: system.host_label
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
      path: runtimeProbeSettings.logsPath,
      resolved_path: runtimeProbeSettings.logsPath,
      logs_db_source: "resolved_codex_home",
      config_file: platform.config_file,
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
