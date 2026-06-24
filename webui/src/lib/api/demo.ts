import type {
  AgentProviderInfo,
  ArchiveDeletePlan,
  ArchiveDeleteResult,
  BridgeActionResult,
  ClaudeOverview,
  CodexConfig,
  CodexGoal,
  CodexGoalSaveInput,
  CodexModel,
  FollowUpQueueItem,
  FollowUpQueueState,
  HiddenThreadDeletePlan,
  HiddenThreadDeleteResult,
  JobRecord,
  MessageBlock,
  OptionalResult,
  PermissionProfile,
  PlatformOverview,
  PluginInfo,
  PublicSettings,
  ProbeEventsResponse,
  ProbeLogsDbStatus,
  ProbeSettings,
  ProbeStatus,
  SecuritySettings,
  SessionUser,
  SystemStatus,
  SystemVersion,
  ThreadBlockPage,
  ThreadDetail,
  ThreadSummary,
  UploadOutcome,
  UpdateStatus
} from "../../types";
import {
  buildDemoPlatformOverview,
  buildDemoSecurity,
  buildDemoSystemStatus,
  type DemoFixtureKey
} from "../domain/demoCore";

type DemoRuntimeGlobal = typeof globalThis & {
  __NEXUSHUB_DESKTOP_RUNTIME__?: boolean;
  __TAURI_INTERNALS__?: unknown;
};

export function currentDemoFixtureKey(): DemoFixtureKey {
  const target = globalThis as DemoRuntimeGlobal;
  return target.__NEXUSHUB_DESKTOP_RUNTIME__ || target.__TAURI_INTERNALS__
    ? "macos-tauri"
    : "linux-web";
}

export function demoSessionUser(username = "admin"): SessionUser {
  return currentDemoFixtureKey() === "macos-tauri"
    ? {
      id: "desktop",
      username: "desktop",
      csrf_token: null,
      session_id: "desktop"
    }
    : {
      id: "dev",
      username,
      csrf_token: "dev-csrf"
    };
}

export function demoPublicSettings(): PublicSettings {
  return {
    site_name: "NexusHub",
    turnstile_enabled: false,
    turnstile_required: false,
    turnstile_site_key: "",
    turnstile_action: "login",
    admin_configured: true
  };
}

export function demoPlatformOverview(fixture: DemoFixtureKey = currentDemoFixtureKey()): PlatformOverview {
  return buildDemoPlatformOverview(fixture);
}

export function demoSystemStatus(fixture: DemoFixtureKey = currentDemoFixtureKey()): SystemStatus {
  return buildDemoSystemStatus(fixture);
}

export function demoSecurity(fixture: DemoFixtureKey = currentDemoFixtureKey()): SecuritySettings {
  return buildDemoSecurity(fixture);
}

export function demoUpdateStatus(fixture: DemoFixtureKey = currentDemoFixtureKey()): UpdateStatus {
  if (fixture === "macos-tauri") {
    return {
      current_version: "0.1.100",
      latest_version: "v0.1.103",
      update_available: true,
      channel: "stable",
      method: "macos_tauri_updater",
      state: "idle",
      failure_category: null,
      recommended_action: "Confirm install in the Tauri updater after signature verification.",
      capabilities: ["check", "confirm_install", "job_history", "signature_verification", "restart_after_install"]
    };
  }
  return {
    current_version: "0.1.100",
    latest_version: "v0.1.103",
    update_available: true,
    channel: "stable",
    method: "linux_systemd_job",
    state: "idle",
    failure_category: null,
    recommended_action: "/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest",
    capabilities: ["check", "confirm_install", "job_history", "sha256_verification", "systemd_health_check", "rollback", "prune_backups"]
  };
}

export function demoSystemVersion(): SystemVersion {
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

export function demoCodexConfig(fixture: DemoFixtureKey = currentDemoFixtureKey()): CodexConfig {
  return {
    model: "gpt-5.5",
    service_tier: null,
    reasoning_effort: "xhigh",
    cwd: fixture === "macos-tauri" ? null : "/home/ubuntu/codex-workspace",
    permission_profile: "danger-full-access",
    approval_policy: "never",
    sandbox_mode: "danger-full-access",
    network_access: true,
    collaboration_mode: null
  };
}

export function demoProviders(): AgentProviderInfo[] {
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

export function demoClaudeCodeOverview(): OptionalResult<ClaudeOverview> {
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

export function demoPlugins(): PluginInfo[] {
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

export function demoModels(): OptionalResult<CodexModel[]> {
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

export function demoPermissionProfiles(): OptionalResult<PermissionProfile[]> {
  return {
    available: true,
    data: [
      { id: "danger-full-access", label: "Danger full access", sandbox_mode: "danger-full-access", approval_policy: "never", network_access: true, default: true },
      { id: "workspace-write", label: "Workspace write", sandbox_mode: "workspace-write", approval_policy: "on-request", network_access: true },
      { id: "read-only", label: "Read only", sandbox_mode: "read-only", approval_policy: "on-request", network_access: false }
    ]
  };
}

export function demoProbeStatus(): ProbeStatus {
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

export function demoProbeSettings(): ProbeSettings {
  const fixture = currentDemoFixtureKey();
  const platform = demoPlatformOverview(fixture);
  const system = demoSystemStatus(fixture);
  const runtimeProbeSettings = fixture === "macos-tauri"
    ? {
      logsPath: "~/Library/Application Support/NexusHub/logs_2.sqlite",
      workspace: "~/Documents"
    }
    : {
      logsPath: "/root/.codex/logs_2.sqlite",
      workspace: "/home/ubuntu/codex-workspace"
    };
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

export function demoSavedProbeSettings(settings: Partial<ProbeSettings>): ProbeSettings {
  return { ...demoProbeSettings(), ...settings } as ProbeSettings;
}

export function demoProbeLogsDbStatus(): OptionalResult<ProbeLogsDbStatus> {
  return {
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
}

export function demoProbeEvents(limit = 10): OptionalResult<ProbeEventsResponse> {
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

export function demoJobId(fallback: string): { job_id: string } {
  return { job_id: `${fallback}-demo` };
}

export function demoUpdateJobId(action: "check" | "install" | "prune"): { job_id: string } {
  return { job_id: `update-${action}-demo` };
}

export function demoCodexGoal(threadId: string): CodexGoal {
  return {
    available: true,
    enabled: threadId === "019e95a0-demo",
    objective: threadId === "019e95a0-demo" ? "修复 Plan Mode 右栏交互" : null,
    token_budget: threadId === "019e95a0-demo" ? 18000 : null,
    status: threadId === "019e95a0-demo" ? "active" : "idle",
    raw: { source: "demo", thread_id: threadId }
  };
}

export function demoSavedCodexGoal(threadId: string, goal: CodexGoalSaveInput): CodexGoal {
  return {
    ...demoCodexGoal(threadId),
    enabled: true,
    objective: goal.objective.trim(),
    token_budget: goal.token_budget ?? null,
    status: "active"
  };
}

export function demoClearedCodexGoal(threadId: string): CodexGoal {
  return {
    ...demoCodexGoal(threadId),
    enabled: false,
    objective: null,
    token_budget: null,
    status: "cleared"
  };
}

export function demoPausedCodexGoal(threadId: string): CodexGoal {
  return {
    ...demoCodexGoal(threadId),
    enabled: true,
    status: "paused"
  };
}

export function demoResumedCodexGoal(threadId: string): CodexGoal {
  return {
    ...demoCodexGoal(threadId),
    enabled: true,
    status: "active"
  };
}

export function demoThreads(status: string, q: string): ThreadSummary[] {
  const threads: ThreadSummary[] = [
    { id: "019e8c1f-demo", title: "活动库审阅链路", status: "Running", message_count: 18, latest_message: "正在逐项审计脚本输出。", updated_at: new Date().toISOString() },
    { id: "019e95a0-demo", title: "Plan Mode 修复", status: "ReplyNeeded", message_count: 7, latest_message: "等待确认", updated_at: new Date().toISOString() },
    { id: "019e5281-demo", title: "检查仓库状态", status: "Recent", message_count: 3, latest_message: "仓库状态干净。", updated_at: new Date().toISOString() },
    { id: "019e42aa-demo", title: "旧归档线程", status: "Archived", message_count: 2, latest_message: "已归档。", updated_at: new Date(Date.now() - 86400000).toISOString() }
  ];
  return threads.filter((thread) => (status === "all" || status === threadStatusParam(thread.status)) && (!q || `${thread.title} ${thread.id}`.toLowerCase().includes(q.toLowerCase())));
}

export function demoThreadDetail(id: string): ThreadDetail {
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

export function demoThreadBlockPage(id: string): ThreadBlockPage {
  const detail = demoThreadDetail(id);
  return {
    thread_id: id,
    blocks: detail.blocks,
    total_blocks: detail.total_blocks ?? detail.blocks.length,
    has_more_blocks: Boolean(detail.has_more_blocks),
    before_cursor: detail.before_cursor ?? null
  };
}

export function demoUploadOutcome(files: File[]): UploadOutcome {
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

export function demoDeletedUpload(): { ok: boolean; deleted: boolean } {
  return { ok: true, deleted: true };
}

export function demoOk(): { ok: boolean } {
  return { ok: true };
}

export function demoBridgeActionResult(threadId: string): BridgeActionResult {
  return { bridge: false, thread_id: threadId, turn_id: "turn-demo", fallback: true, message: "已提交给 Codex" };
}

export function demoCreatedThreadResult(): BridgeActionResult {
  return demoBridgeActionResult("019e-new-demo");
}

export function demoFollowUps(): FollowUpQueueState {
  return { items: [] };
}

export function demoEnqueuedFollowUp(threadId: string, payload: { message: string; [key: string]: unknown }): FollowUpQueueItem {
  return {
    id: `follow-up-${Date.now()}`,
    thread_id: threadId,
    status: "pending",
    message: payload.message,
    options: payload,
    created_at: Math.floor(Date.now() / 1000)
  };
}

export function demoArchiveDeletePlan(): ArchiveDeletePlan {
  return { total_threads: 42, active_threads: 31, archived_threads: 11, session_index_lines: 44, rollout_files: 39, archived_ids: ["019e-demo-a", "019e-demo-b"], integrity: "ok" };
}

export function demoArchiveDeleteResult(): ArchiveDeleteResult {
  return {
    before: demoArchiveDeletePlan(),
    after_total_threads: 31,
    after_active_threads: 31,
    after_archived_threads: 0,
    after_integrity: "ok",
    deleted_rollout_files: 11
  };
}

export function demoHiddenThreadDeletePlan(): HiddenThreadDeletePlan {
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

export function demoHiddenThreadDeleteResult(): HiddenThreadDeleteResult {
  return {
    before: demoHiddenThreadDeletePlan(),
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

export function demoJobs(): JobRecord[] {
  return [
    { id: "probe-bark-demo", kind: "probe_bark_test", status: "succeeded", title: "Probe Bark 测试", started_at: 1780731706, finished_at: 1780731710, exit_code: 0, output: "POST https://api.day.app\nHTTP 200\nBark push accepted" },
    { id: "probe-logs-demo", kind: "probe_logs_db_maintain", status: "succeeded", title: "Probe logs-db dry-run", started_at: 1780731666, finished_at: 1780731672, exit_code: 0, output: "dry_run=true\nwould_delete_probe_events=42\ncompact=false" },
    { id: "job-demo", kind: "nexushub_update_check", status: "succeeded", title: "NexusHub update precheck", started_at: 1780731606, output: "version check\nintegrity_check: ok" },
    { id: "job-failed-demo", kind: "panel_update", status: "failed", title: "Panel update", started_at: 1780731206, finished_at: 1780731252, exit_code: 1, output: "download release asset\nverify checksum", error: "release asset checksum mismatch", analysis: "Downloaded asset digest did not match release metadata.", explanation: "Retry after confirming the release asset has finished publishing." }
  ];
}

export function demoJob(id: string): JobRecord {
  return demoJobs().find((job) => job.id === id) ?? {
    id,
    kind: "unknown",
    status: "failed",
    title: id,
    started_at: Date.now() / 1000,
    output: "",
    error: "demo job not found"
  };
}

export function threadStatusParam(status: ThreadSummary["status"]): string {
  if (status === "ReplyNeeded") return "reply-needed";
  return status.toLowerCase();
}
