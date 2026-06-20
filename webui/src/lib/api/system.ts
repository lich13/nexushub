import type {
  ArchiveDeletePlan,
  ArchiveDeleteResult,
  AgentProviderInfo,
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
  ProbeEventsResponse,
  ProbeJobAction,
  ProbeLogsDbStatus,
  ProbeSettings,
  ProbeStatus,
  PublicSettings,
  SecuritySettings,
  SentinelStatus,
  SessionUser,
  SystemStatus,
  SystemVersion,
  ThreadBlockPage,
  ThreadDetail,
  ThreadSummary,
  UpdateStatus,
  UploadOutcome
} from "../../types";
import {
  RuntimeUnavailableError,
  createRuntimeThreadEventSource,
  desktopSessionUser,
  runtimeDispatch,
  runtimeRpc,
  runtimeValue,
  uploadRuntimeFiles
} from "./transport";
import {
  isMissingEndpoint,
  jobIdFromRuntimeResult,
  normalizeModels,
  normalizeOptionalResult,
  normalizePermissionProfiles,
  normalizeProbeRuntimePayload,
  USE_DEMO
} from "./shared";
import { demoPlatformOverview, demoSystemStatus } from "./demo";

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
