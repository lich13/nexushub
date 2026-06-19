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
  SystemCapabilities,
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
  getRuntimeKind,
  invokeDesktop,
  runtimeDispatch,
  uploadRuntimeFiles
} from "./runtime";

const USE_DEMO = import.meta.env.DEV && import.meta.env.VITE_USE_REAL_API !== "1";

export class ApiError extends Error {
  constructor(message: string, readonly status: number) {
    super(message);
    this.name = "ApiError";
  }
}

type RpcArgs = Record<string, unknown> | undefined;

type WebRoute = {
  command: string;
  args?: (args?: RpcArgs) => RpcArgs;
  unavailable?: string;
};

type DesktopRoute = {
  command?: string;
  args?: (args?: RpcArgs) => RpcArgs;
  fromHome?: (home: DesktopHome, args?: RpcArgs) => unknown;
  fallback?: (args?: RpcArgs) => unknown;
  unavailable?: string;
};

type DesktopHome = {
  overview?: unknown;
  system?: unknown;
  probe?: unknown;
  logsDb?: unknown;
  logs_db?: unknown;
  threads?: unknown[];
  plugins?: unknown[];
  models?: unknown[];
  permissionProfiles?: unknown[];
  permission_profiles?: unknown[];
  codexConfig?: unknown;
  codex_config?: unknown;
  archivePlan?: unknown;
  archive_plan?: unknown;
  hiddenPlan?: unknown;
  hidden_plan?: unknown;
  goal?: unknown;
  warnings?: string[];
};

async function desktopHome(): Promise<DesktopHome> {
  return invokeDesktop<DesktopHome>("desktop_home");
}

function argString(args: RpcArgs, key: string): string {
  const value = args?.[key];
  return typeof value === "string" ? value : "";
}

function optionalString(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}

function desktopGoalFromHome(home: DesktopHome, args?: RpcArgs) {
  return normalizeDesktopGoal(home.goal, argString(args, "threadId"));
}

function normalizeDesktopGoal(value: unknown, threadId?: string) {
  const goal = value && typeof value === "object"
    ? value as Record<string, unknown>
    : {};
  return {
    available: goal.available !== false,
    enabled: Boolean(goal.enabled),
    objective: optionalString(goal.objective),
    token_budget: typeof goal.tokenBudget === "number"
      ? goal.tokenBudget
      : typeof goal.token_budget === "number"
        ? goal.token_budget
        : null,
    status: typeof goal.status === "string" ? goal.status : "idle",
    completed_at: typeof goal.completedAt === "number"
      ? goal.completedAt
      : typeof goal.completed_at === "number"
        ? goal.completed_at
        : null,
    blocked_reason: optionalString(goal.blockedReason ?? goal.blocked_reason),
    raw: { ...goal, thread_id: threadId }
  };
}

function desktopPublicSettings() {
  return {
    site_name: "NexusHub",
    turnstile_enabled: false,
    turnstile_required: false,
    turnstile_site_key: "",
    turnstile_action: "login",
    admin_configured: true
  };
}

function desktopPlatform(home: DesktopHome) {
  const overview = home.overview && typeof home.overview === "object"
    ? home.overview as Record<string, unknown>
    : {};
  const paths = overview.paths && typeof overview.paths === "object"
    ? overview.paths as Record<string, unknown>
    : {};
  return {
    kind: "macos",
    data_dir: String(paths.appSupportDir ?? paths.app_support_dir ?? ""),
    config_file: String(paths.configFile ?? paths.config_file ?? ""),
    webui_dir: String(paths.appSupportDir ?? paths.app_support_dir ?? ""),
    log_dir: String(paths.logDir ?? paths.log_dir ?? ""),
    service_name: "NexusHub.app",
    service_kind: "tauri"
  };
}

function objectArg(args: RpcArgs, key: string): Record<string, unknown> {
  const value = args?.[key];
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function camelizeProbeSettings(value: Record<string, unknown>): Record<string, unknown> {
  const probe = objectValue(value.probe);
  const notifications = objectValue(value.notifications);
  const codex = objectValue(value.codex);
  const logsDb = objectValue(value.logs_db ?? value.logsDb);
  return {
    codex: Object.keys(codex).length ? {
      home: optionalString(codex.home),
      workspace: optionalString(codex.workspace),
      hostLabel: optionalString(codex.host_label ?? codex.hostLabel)
    } : undefined,
    probe: Object.keys(probe).length ? {
      enabled: booleanOrUndefined(probe.enabled),
      pollSeconds: numberOrUndefined(probe.poll_seconds ?? probe.pollSeconds),
      recentLimit: numberOrUndefined(probe.recent_limit ?? probe.recentLimit),
      hooks: camelizeKeys(objectValue(probe.hooks)),
      notifications: normalizeProbeNotifications(objectValue(probe.notifications)),
      observability: camelizeKeys(objectValue(probe.observability)),
      logsDb: camelizeKeys(objectValue(probe.logs_db ?? probe.logsDb))
    } : undefined,
    notifications: Object.keys(notifications).length
      ? normalizeProbeNotifications(notifications)
      : undefined,
    logsDb: Object.keys(logsDb).length ? camelizeKeys(logsDb) : undefined
  };
}

function normalizeProbeNotifications(value: Record<string, unknown>): Record<string, unknown> {
  return withoutUndefined({
    deviceKey: optionalString(value.device_key ?? value.deviceKey),
    enabled: booleanOrUndefined(value.enabled),
    serverUrl: optionalString(value.server_url ?? value.serverUrl),
    sound: value.sound,
    group: optionalString(value.group),
    url: value.url,
    notifyCompletion: booleanOrUndefined(value.notify_completion ?? value.notifyCompletion),
    notifyReplyNeeded: booleanOrUndefined(value.notify_reply_needed ?? value.notifyReplyNeeded),
    notifyRecoverable: booleanOrUndefined(value.notify_recoverable ?? value.notifyRecoverable)
  });
}

function objectValue(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function booleanOrUndefined(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined;
}

function numberOrUndefined(value: unknown): number | undefined {
  return typeof value === "number" ? value : undefined;
}

function camelizeKeys(value: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [key, item] of Object.entries(value)) {
    out[key.replace(/_([a-z])/g, (_, letter: string) => letter.toUpperCase())] = item;
  }
  return out;
}

function withoutUndefined(value: Record<string, unknown>): Record<string, unknown> {
  return Object.fromEntries(Object.entries(value).filter(([, item]) => item !== undefined));
}

function threadListRequest(args?: RpcArgs) {
  return {
    status: args?.status,
    query: args?.q,
    limit: args?.limit
  };
}

function threadDetailRequest(args?: RpcArgs) {
  const options = objectArg(args, "options");
  return {
    id: argString(args, "id"),
    limit: options.limit,
    before: options.before,
    full: options.full
  };
}

function threadBlocksRequest(args?: RpcArgs) {
  const options = objectArg(args, "options");
  return {
    id: argString(args, "id"),
    limit: options.limit,
    before: options.before
  };
}

function threadSendRequest(args?: RpcArgs) {
  return {
    ...objectArg(args, "payload"),
    threadId: argString(args, "threadId") || optionalString(objectArg(args, "payload").thread_id) || undefined
  };
}

function planRequest(args?: RpcArgs) {
  const payload = objectArg(args, "payload");
  return {
    threadId: argString(args, "threadId"),
    turnId: payload.turn_id ?? payload.turnId,
    itemId: payload.item_id ?? payload.itemId,
    instructions: payload.instructions
  };
}

function updateWebArgs(args?: RpcArgs): RpcArgs {
  return {
    action: args?.action,
    csrfToken: args?.csrfToken
  };
}

function probeJobWebArgs(args?: RpcArgs): RpcArgs {
  return {
    action: args?.action,
    csrfToken: args?.csrfToken
  };
}

function probeJobDesktopRoute(action: unknown): DesktopRoute {
  switch (action as ProbeJobAction) {
    case "bark-test":
      return { command: "desktop_probe_bark_test" };
    case "hooks-install":
      return { command: "desktop_probe_hooks_install" };
    case "logs-db-dry-run":
      return {
        command: "desktop_probe_logs_db_maintain",
        args: () => ({ request: { dryRun: true, compact: false } })
      };
    case "logs-db-execute":
      return {
        command: "desktop_probe_logs_db_maintain",
        args: () => ({ request: { dryRun: false, compact: false } })
      };
    default:
      return { unavailable: `Unknown Probe job action: ${String(action)}` };
  }
}

const ROUTES: Record<string, { web: WebRoute; desktop: DesktopRoute }> = {
  getPublicSettings: {
    web: { command: "getPublicSettings" },
    desktop: { fallback: desktopPublicSettings }
  },
  login: {
    web: {
      command: "login",
      args: (args) => ({
        username: args?.username,
        password: args?.password,
        turnstile_token: typeof args?.turnstileToken === "string" && args.turnstileToken.trim()
          ? args.turnstileToken.trim()
          : undefined
      })
    },
    desktop: { fallback: desktopSessionUser }
  },
  logout: {
    web: { command: "logout" },
    desktop: { fallback: () => undefined }
  },
  me: {
    web: { command: "me" },
    desktop: { fallback: desktopSessionUser }
  },
  listThreads: {
    web: {
      command: "listThreads",
      args: (args) => ({
        status: args?.status,
        q: args?.q,
        limit: args?.limit
      })
    },
    desktop: {
      command: "desktop_threads",
      args: (args) => ({ request: threadListRequest(args) })
    }
  },
  getThread: {
    web: { command: "getThread" },
    desktop: {
      command: "desktop_thread_detail",
      args: (args) => ({ request: threadDetailRequest(args) })
    }
  },
  getThreadBlocks: {
    web: { command: "getThreadBlocks" },
    desktop: {
      command: "desktop_thread_blocks",
      args: (args) => ({ request: threadBlocksRequest(args) })
    }
  },
  getSystemStatus: {
    web: { command: "getSystemStatus" },
    desktop: { fromHome: (home) => home.system ?? {} }
  },
  getSystemVersion: {
    web: { command: "getSystemVersion" },
    desktop: {
      fromHome: (home) => {
        const overview = home.overview && typeof home.overview === "object"
          ? home.overview as Record<string, unknown>
          : {};
        return {
          panel_current: String(overview.version ?? ""),
          panel_latest: null,
          panel_update_available: false,
          codex_current: null,
          codex_latest: null,
          codex_update_available: null
        };
      }
    }
  },
  getSecurity: {
    web: { command: "getSecurity" },
    desktop: { unavailable: "该宿主不支持安全设置" }
  },
  saveSecurity: {
    web: { command: "saveSecurity" },
    desktop: { unavailable: "该宿主不支持安全设置" }
  },
  changePassword: {
    web: { command: "changePassword" },
    desktop: { unavailable: "Desktop password command is not implemented" }
  },
  listProviders: {
    web: { command: "listProviders" },
    desktop: { fromHome: (home) => home.plugins ?? [] }
  },
  getClaudeCodeOverview: {
    web: { command: "getClaudeCodeOverview" },
    desktop: { command: "desktop_claude_code_overview" }
  },
  getPlatformOverview: {
    web: { command: "getPlatformOverview" },
    desktop: { fromHome: desktopPlatform }
  },
  listPlugins: {
    web: { command: "listPlugins" },
    desktop: { fromHome: (home) => home.plugins ?? [] }
  },
  getProbeStatus: {
    web: { command: "getProbeStatus" },
    desktop: { command: "desktop_probe_status" }
  },
  getProbeSettings: {
    web: { command: "getProbeSettings" },
    desktop: { command: "desktop_probe_settings" }
  },
  saveProbeSettings: {
    web: { command: "saveProbeSettings" },
    desktop: {
      command: "desktop_probe_save_settings",
      args: (args) => ({ request: camelizeProbeSettings(objectArg(args, "settings")) })
    }
  },
  getProbeLogsDbStatus: {
    web: { command: "getProbeLogsDbStatus" },
    desktop: { fromHome: (home) => ({ available: true, data: home.logsDb ?? home.logs_db ?? {} }) }
  },
  getProbeEvents: {
    web: { command: "getProbeEvents" },
    desktop: {
      command: "desktop_probe_events",
      args: (args) => ({ request: { limit: args?.limit ?? 10 } })
    }
  },
  dryRunArchiveDelete: {
    web: { command: "dryRunArchiveDelete" },
    desktop: { command: "desktop_archive_delete_dry_run" }
  },
  startArchiveDelete: {
    web: { command: "startArchiveDelete", args: (args) => ({ ...args, confirmed: true }) },
    desktop: { command: "desktop_archive_delete_execute" }
  },
  dryRunHiddenThreadDelete: {
    web: { command: "dryRunHiddenThreadDelete" },
    desktop: { command: "desktop_hidden_delete_dry_run" }
  },
  startHiddenThreadDelete: {
    web: { command: "startHiddenThreadDelete", args: (args) => ({ ...args, confirmed: true }) },
    desktop: { command: "desktop_hidden_delete_execute" }
  },
  getUpdateStatus: {
    web: { command: "getUpdateStatus" },
    desktop: { command: "desktop_update_status" }
  },
  runUpdateAction: {
    web: { command: "runUpdateAction", args: updateWebArgs },
    desktop: {
      fallback: (args) => {
        const action = args?.action;
        if (action === "install") {
          return invokeDesktop("install_update_and_restart");
        }
        if (action === "check") {
          return invokeDesktop("check_update_status");
        }
        throw new RuntimeUnavailableError("macOS App 没有 Linux 备份清理动作。", "runUpdateAction");
      }
    }
  },
  startProbeJob: {
    web: { command: "startProbeJob", args: probeJobWebArgs },
    desktop: {
      fallback: (args) => runtimeRpcViaDesktopRoute(probeJobDesktopRoute(args?.action), args)
    }
  },
  deleteUpload: {
    web: { command: "deleteUpload" },
    desktop: {
      command: "desktop_delete_upload",
      args: (args) => ({ id: argString(args, "id") })
    }
  },
  createThread: {
    web: { command: "createThread", args: (args) => ({ ...objectArg(args, "payload"), csrfToken: args?.csrfToken }) },
    desktop: {
      command: "desktop_send_message",
      args: (args) => ({ request: threadSendRequest(args) })
    }
  },
  sendMessage: {
    web: { command: "sendMessage" },
    desktop: {
      command: "desktop_send_message",
      args: (args) => ({ request: threadSendRequest(args) })
    }
  },
  steerThread: {
    web: { command: "steerThread" },
    desktop: {
      command: "desktop_continue_thread",
      args: (args) => ({ request: threadSendRequest(args) })
    }
  },
  listFollowUps: {
    web: { command: "listFollowUps" },
    desktop: {
      command: "desktop_list_followups",
      args: (args) => ({ request: { threadId: argString(args, "threadId"), limit: args?.limit ?? 20 } })
    }
  },
  enqueueFollowUp: {
    web: { command: "enqueueFollowUp" },
    desktop: {
      command: "desktop_enqueue_followup",
      args: (args) => ({ request: threadSendRequest(args) })
    }
  },
  cancelFollowUp: {
    web: { command: "cancelFollowUp" },
    desktop: {
      command: "desktop_cancel_followup",
      args: (args) => ({
        request: {
          threadId: argString(args, "threadId"),
          followupId: argString(args, "followUpId")
        }
      })
    }
  },
  stopThread: {
    web: { command: "stopThread" },
    desktop: {
      command: "desktop_stop_thread",
      args: (args) => ({ request: { threadId: argString(args, "threadId"), ...objectArg(args, "payload") } })
    }
  },
  archiveThread: {
    web: { command: "archiveThread" },
    desktop: {
      command: "desktop_archive_thread",
      args: (args) => ({ request: { threadId: argString(args, "threadId") } })
    }
  },
  restoreThread: {
    web: { command: "restoreThread" },
    desktop: {
      command: "desktop_restore_thread",
      args: (args) => ({ request: { threadId: argString(args, "threadId") } })
    }
  },
  renameThread: {
    web: { command: "renameThread" },
    desktop: {
      command: "desktop_rename_thread",
      args: (args) => ({ request: { threadId: argString(args, "threadId"), name: args?.name } })
    }
  },
  forkThread: {
    web: { command: "forkThread" },
    desktop: { unavailable: "Desktop fork command is not implemented" }
  },
  answerElicitation: {
    web: { command: "answerElicitation" },
    desktop: {
      command: "desktop_answer_elicitation",
      args: (args) => ({ request: { threadId: argString(args, "threadId"), answers: args?.answers ?? {} } })
    }
  },
  acceptPlan: {
    web: { command: "acceptPlan" },
    desktop: {
      command: "desktop_plan_accept",
      args: (args) => ({ request: planRequest(args) })
    }
  },
  revisePlan: {
    web: { command: "revisePlan" },
    desktop: {
      command: "desktop_plan_revise",
      args: (args) => ({ request: planRequest(args) })
    }
  },
  answerApproval: {
    web: { command: "answerApproval" },
    desktop: { unavailable: "Desktop approval command is not implemented" }
  },
  listModels: {
    web: { command: "listModels" },
    desktop: { fromHome: (home) => ({ available: true, data: home.models ?? [] }) }
  },
  listPermissionProfiles: {
    web: { command: "listPermissionProfiles" },
    desktop: { fromHome: (home) => ({ available: true, data: home.permissionProfiles ?? home.permission_profiles ?? [] }) }
  },
  getCodexConfig: {
    web: { command: "getCodexConfig" },
    desktop: { fromHome: (home) => ({ available: true, data: home.codexConfig ?? home.codex_config ?? {} }) }
  },
  getCodexGoal: {
    web: { command: "getCodexGoal" },
    desktop: { fromHome: desktopGoalFromHome }
  },
  saveCodexGoal: {
    web: {
      command: "saveCodexGoal",
      args: (args) => ({
        thread_id: args?.threadId,
        objective: args?.objective,
        token_budget: args?.tokenBudget ?? null,
        csrfToken: args?.csrfToken
      })
    },
    desktop: {
      command: "desktop_save_goal_command",
      args: (args) => ({
        request: {
          threadId: argString(args, "threadId"),
          objective: args?.objective,
          tokenBudget: args?.tokenBudget ?? null
        }
      })
    }
  },
  clearCodexGoal: {
    web: { command: "clearCodexGoal", args: (args) => ({ thread_id: args?.threadId, csrfToken: args?.csrfToken }) },
    desktop: { command: "desktop_clear_goal_command", args: (args) => ({ threadId: argString(args, "threadId") }) }
  },
  pauseCodexGoal: {
    web: { command: "pauseCodexGoal", args: (args) => ({ thread_id: args?.threadId, csrfToken: args?.csrfToken }) },
    desktop: { command: "desktop_pause_goal_command", args: (args) => ({ threadId: argString(args, "threadId") }) }
  },
  resumeCodexGoal: {
    web: { command: "resumeCodexGoal", args: (args) => ({ thread_id: args?.threadId, csrfToken: args?.csrfToken }) },
    desktop: { command: "desktop_resume_goal_command", args: (args) => ({ threadId: argString(args, "threadId") }) }
  },
  listJobs: {
    web: { command: "listJobs", args: () => ({ limit: 30 }) },
    desktop: {
      command: "desktop_jobs",
      args: () => ({ request: { limit: 30 } })
    }
  },
  getJob: {
    web: { command: "getJob" },
    desktop: {
      command: "desktop_job_detail",
      args: (args) => ({ request: { id: argString(args, "id") } })
    }
  }
};

async function runtimeRpcViaDesktopRoute<T = unknown>(
  route: DesktopRoute,
  args?: RpcArgs,
): Promise<T> {
  if (route.unavailable) {
    throw new RuntimeUnavailableError(route.unavailable, route.unavailable);
  }
  if (route.fallback) {
    return route.fallback(args) as T;
  }
  if (route.fromHome) {
    return route.fromHome(await desktopHome(), args) as T;
  }
  if (!route.command) {
    throw new RuntimeUnavailableError("Desktop command is not configured", "desktop");
  }
  return invokeDesktop<T>(route.command, route.args?.(args));
}

async function runtimeRpc<T = unknown>(
  name: keyof typeof ROUTES | string,
  args?: RpcArgs,
): Promise<T> {
  const route = ROUTES[name];
  if (!route) {
    throw new RuntimeUnavailableError(`Unknown runtime RPC: ${name}`, name);
  }
  return runtimeDispatch<T>({
    webCommand: route.web.command,
    webArgs: route.web.args?.(args) ?? args,
    webUnavailable: route.web.unavailable,
    desktopCommand: route.desktop.command,
    desktopArgs: route.desktop.args?.(args),
    desktopFallback: route.desktop.fallback
      ? () => route.desktop.fallback?.(args) as T | Promise<T>
      : route.desktop.fromHome
        ? async () => route.desktop.fromHome?.(await desktopHome(), args) as T
        : undefined,
    desktopUnavailable: route.desktop.unavailable
  });
}

function isMissingEndpoint(error: unknown): boolean {
  return error instanceof RuntimeUnavailableError || error instanceof ApiError && [404, 405, 501].includes(error.status);
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

export type RuntimeCapabilityMatrix = {
  runtimeKind: "web" | "desktop";
  webAuth: boolean;
  logout: boolean;
  securitySettings: boolean;
  publicEndpointStatus: boolean;
  codexStatePaths: boolean;
  linuxBackupPrune: boolean;
  linuxUpdateLabels: boolean;
  forkAction: boolean;
  approvalActions: boolean;
};

const DESKTOP_RUNTIME_CAPABILITIES: RuntimeCapabilityMatrix = {
  runtimeKind: "desktop",
  webAuth: false,
  logout: false,
  securitySettings: false,
  publicEndpointStatus: false,
  codexStatePaths: false,
  linuxBackupPrune: false,
  linuxUpdateLabels: false,
  forkAction: false,
  approvalActions: false
};

const WEB_RUNTIME_CAPABILITIES: RuntimeCapabilityMatrix = {
  runtimeKind: "web",
  webAuth: true,
  logout: true,
  securitySettings: true,
  publicEndpointStatus: true,
  codexStatePaths: true,
  linuxBackupPrune: true,
  linuxUpdateLabels: true,
  forkAction: true,
  approvalActions: true
};

export function runtimeCapabilities(): RuntimeCapabilityMatrix {
  return getRuntimeKind() === "desktop"
    ? DESKTOP_RUNTIME_CAPABILITIES
    : WEB_RUNTIME_CAPABILITIES;
}

export function runtimeCapabilitiesForRuntime(
  desktop: boolean | RuntimeCapabilityMatrix["runtimeKind"] = false,
): RuntimeCapabilityMatrix {
  if (desktop === "desktop") return DESKTOP_RUNTIME_CAPABILITIES;
  if (desktop === "web") return WEB_RUNTIME_CAPABILITIES;
  return desktop ? DESKTOP_RUNTIME_CAPABILITIES : WEB_RUNTIME_CAPABILITIES;
}

export function runtimeCapabilitiesFromSystemStatus(
  status?: Pick<SystemStatus, "capabilities"> | null,
  fallback: RuntimeCapabilityMatrix = runtimeCapabilities(),
): RuntimeCapabilityMatrix {
  const core = status?.capabilities;
  if (!core) return fallback;
  return {
    runtimeKind: fallback.runtimeKind,
    webAuth: core.web_auth,
    logout: core.web_auth,
    securitySettings: core.security_settings || core.turnstile || core.admin_password,
    publicEndpointStatus: core.public_endpoint,
    codexStatePaths: core.systemd,
    linuxBackupPrune: core.prune_backups,
    linuxUpdateLabels: core.linux_update_job,
    forkAction: core.web_auth,
    approvalActions: core.web_auth
  };
}

function currentRuntimeCapabilities(): RuntimeCapabilityMatrix {
  return runtimeCapabilities();
}

export function desktopRuntimeSessionUser(): SessionUser {
  return desktopSessionUser();
}

export async function getPublicSettings(): Promise<PublicSettings> {
  if (USE_DEMO) {
    return { site_name: "NexusHub", turnstile_enabled: false, turnstile_required: false, turnstile_site_key: "", turnstile_action: "login", admin_configured: true };
  }
  return runtimeRpc<PublicSettings>("getPublicSettings");
}

export async function login(username: string, password: string, turnstileToken?: string | null): Promise<SessionUser> {
  if (USE_DEMO) {
    return currentRuntimeCapabilities().runtimeKind === "desktop"
      ? desktopSessionUser()
      : { id: "dev", username, csrf_token: "dev-csrf" };
  }
  return runtimeRpc<SessionUser>("login", { username, password, turnstileToken });
}

export async function logout(csrfToken?: string | null): Promise<void> {
  if (USE_DEMO) return;
  await runtimeRpc("logout", { csrfToken });
}

export async function me(): Promise<SessionUser> {
  if (USE_DEMO) {
    return currentRuntimeCapabilities().runtimeKind === "desktop"
      ? desktopSessionUser()
      : { id: "dev", username: "admin", csrf_token: "dev-csrf" };
  }
  return runtimeRpc<SessionUser>("me");
}

export async function listThreads(status: string, q: string): Promise<ThreadSummary[]> {
  if (USE_DEMO) return demoThreads(status, q);
  return runtimeRpc<ThreadSummary[]>("listThreads", { status, q, limit: 120 });
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
  return runtimeRpc<ThreadDetail>("getThread", { id, options });
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
  return runtimeRpc<ThreadBlockPage>("getThreadBlocks", { id, options });
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
    const capabilities = currentRuntimeCapabilities();
    const desktop = capabilities.runtimeKind === "desktop";
    return {
      current_version: "0.1.100",
      latest_version: "v0.1.103",
      update_available: true,
      channel: "stable",
      method: desktop ? "macos_tauri_updater" : "linux_systemd_job",
      state: "idle",
      failure_category: null,
      recommended_action: desktop
        ? "Confirm install in the Tauri updater after signature verification."
        : "/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest",
      capabilities: desktop
        ? ["check", "confirm_install", "job_history", "signature_verification", "restart_after_install"]
        : ["check", "confirm_install", "job_history", "sha256_verification", "systemd_health_check", "rollback", "prune_backups"]
    };
  }
  return runtimeRpc<UpdateStatus>("getUpdateStatus");
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
  return normalizeOptionalResult(await runtimeRpc<ProbeStatus>("getProbeStatus"));
}

export async function getProbeSettings(): Promise<OptionalResult<ProbeSettings>> {
  if (USE_DEMO) {
    return {
      available: true,
      data: demoProbeSettings()
    };
  }
  return normalizeOptionalResult(await runtimeRpc<ProbeSettings>("getProbeSettings"));
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
  return runtimeRpc<ProbeSettings>("saveProbeSettings", {
    settings: normalizeProbeSettingsSavePayload(settings),
    csrfToken
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
  return normalizeOptionalResult(await runtimeRpc<ProbeLogsDbStatus>("getProbeLogsDbStatus"));
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
  return normalizeOptionalResult(await runtimeRpc<ProbeEventsResponse>("getProbeEvents", { limit }));
}

export async function saveSecurity(settings: Partial<SecuritySettings> & { turnstile_secret_key?: string }, csrfToken?: string | null) {
  return runtimeRpc<SecuritySettings>("saveSecurity", { settings, csrfToken });
}

export async function dryRunArchiveDelete(csrfToken?: string | null): Promise<ArchiveDeletePlan> {
  if (USE_DEMO) {
    return { total_threads: 42, active_threads: 31, archived_threads: 11, session_index_lines: 44, rollout_files: 39, archived_ids: ["019e-demo-a", "019e-demo-b"], integrity: "ok" };
  }
  return runtimeRpc<ArchiveDeletePlan>("dryRunArchiveDelete", { csrfToken });
}

export async function startArchiveDelete(csrfToken?: string | null): Promise<ArchiveDeleteResult> {
  return runtimeRpc<ArchiveDeleteResult>("startArchiveDelete", { csrfToken });
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
  return runtimeRpc<HiddenThreadDeletePlan>("dryRunHiddenThreadDelete", { csrfToken });
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
  return runtimeRpc<HiddenThreadDeleteResult>("startHiddenThreadDelete", { csrfToken });
}

export type UnifiedUpdateAction = "check" | "install" | "prune";

function jobIdFromRuntimeResult(result: { job_id?: string | null; jobId?: string | null }, fallback: string): { job_id: string } {
  return { job_id: result.job_id ?? result.jobId ?? fallback };
}

export type UpdateActionResult = {
  job_id: string;
  status?: UpdateStatus;
};

export async function runUpdateAction(action: UnifiedUpdateAction, csrfToken?: string | null): Promise<UpdateActionResult> {
  if (USE_DEMO) return { job_id: `update-${action}-demo` };
  const result = await runtimeRpc<{ job_id?: string | null; jobId?: string | null; status?: UpdateStatus }>("runUpdateAction", { action, csrfToken });
  return {
    ...jobIdFromRuntimeResult(result, `update-${action}`),
    ...(result.status ? { status: result.status } : {})
  };
}

export async function startProbeJob(action: ProbeJobAction, csrfToken?: string | null): Promise<{ job_id: string }> {
  if (USE_DEMO) return { job_id: `probe-${action}-demo` };
  const result = await runtimeRpc<{ job_id?: string | null; jobId?: string | null }>("startProbeJob", { action, csrfToken });
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
  return runtimeRpc<{ ok: boolean; deleted: boolean }>("deleteUpload", { id, csrfToken });
}

export async function createThread(payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return { bridge: false, thread_id: "019e-new-demo", turn_id: "turn-demo", fallback: true, message: "已提交给 Codex" };
  return runtimeRpc<BridgeActionResult>("createThread", { payload, csrfToken });
}

export async function sendMessage(threadId: string, payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return { bridge: false, thread_id: threadId, turn_id: "turn-demo", fallback: true, message: "已提交给 Codex" };
  return runtimeRpc<BridgeActionResult>("sendMessage", { threadId, payload, csrfToken });
}

export async function steerThread(threadId: string, payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return { bridge: false, thread_id: threadId, turn_id: "turn-demo", fallback: true, message: "已提交给 Codex" };
  return runtimeRpc<BridgeActionResult>("steerThread", { threadId, payload, csrfToken });
}

export async function listFollowUps(threadId: string): Promise<FollowUpQueueState> {
  if (USE_DEMO) return { items: [] };
  const result = await runtimeRpc<FollowUpQueueState | FollowUpQueueItem[]>("listFollowUps", { threadId });
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
  return runtimeRpc<FollowUpQueueItem>("enqueueFollowUp", { threadId, payload, csrfToken });
}

export async function cancelFollowUp(threadId: string, followUpId: string, csrfToken?: string | null): Promise<{ ok: boolean }> {
  if (USE_DEMO) return { ok: true };
  return runtimeRpc<{ ok: boolean }>("cancelFollowUp", { threadId, followUpId, csrfToken });
}

export async function stopThread(threadId: string, payload: { turn_id?: string | null; job_id?: string | null }, csrfToken?: string | null) {
  if (USE_DEMO) return { ok: true };
  return runtimeRpc("stopThread", { threadId, payload, csrfToken });
}

export async function archiveThread(threadId: string, csrfToken?: string | null) {
  return runtimeRpc("archiveThread", { threadId, csrfToken });
}

export async function restoreThread(threadId: string, csrfToken?: string | null) {
  return runtimeRpc("restoreThread", { threadId, csrfToken });
}

export async function renameThread(threadId: string, name: string, csrfToken?: string | null) {
  return runtimeRpc("renameThread", { threadId, name, csrfToken });
}

export async function forkThread(threadId: string, csrfToken?: string | null): Promise<BridgeActionResult> {
  return runtimeRpc<BridgeActionResult>("forkThread", { threadId, csrfToken });
}

export async function answerElicitation(threadId: string, answers: Record<string, string[]>, csrfToken?: string | null): Promise<BridgeActionResult> {
  return runtimeRpc<BridgeActionResult>("answerElicitation", { threadId, answers, csrfToken });
}

export async function acceptPlan(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  return runtimeRpc<BridgeActionResult>("acceptPlan", { threadId, payload, csrfToken });
}

export async function revisePlan(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null; instructions: string },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  return runtimeRpc<BridgeActionResult>("revisePlan", { threadId, payload, csrfToken });
}

export async function answerApproval(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null; request_id?: string | null; decision: string },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  return runtimeRpc<BridgeActionResult>("answerApproval", { threadId, payload, csrfToken });
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
  return runtimeRpc<CodexGoal>("getCodexGoal", { threadId });
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
  return runtimeRpc<CodexGoal>("saveCodexGoal", {
    threadId,
    objective: goal.objective,
    tokenBudget: goal.token_budget ?? null,
    csrfToken
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
  return runtimeRpc<CodexGoal>("clearCodexGoal", { threadId, csrfToken });
}

export async function pauseCodexGoal(threadId: string, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return {
      ...demoCodexGoal(threadId),
      enabled: true,
      status: "paused"
    };
  }
  return runtimeRpc<CodexGoal>("pauseCodexGoal", { threadId, csrfToken });
}

export async function resumeCodexGoal(threadId: string, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return {
      ...demoCodexGoal(threadId),
      enabled: true,
      status: "active"
    };
  }
  return runtimeRpc<CodexGoal>("resumeCodexGoal", { threadId, csrfToken });
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
  return runtimeRpc<JobRecord[]>("listJobs");
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
  return runtimeRpc<JobRecord>("getJob", { id });
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
  if (currentRuntimeCapabilities().runtimeKind === "desktop") {
    return {
      kind: "macos",
      data_dir: "~/Library/Application Support/NexusHub",
      config_file: "~/Library/Application Support/NexusHub/config.toml",
      webui_dir: "~/Library/Application Support/NexusHub/webui",
      log_dir: "~/Library/Logs/NexusHub",
      service_name: "NexusHub.app",
      service_kind: "tauri"
    };
  }
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

function demoSystemStatus(): SystemStatus {
  const capabilities = currentRuntimeCapabilities();
  const systemCapabilities: SystemCapabilities = capabilities.runtimeKind === "desktop"
    ? {
      threads: true,
      jobs: true,
      probe: true,
      status: true,
      settings: true,
      job_history: true,
      app_updater: true,
      web_auth: false,
      security_settings: false,
      turnstile: false,
      systemd: false,
      nginx: false,
      public_endpoint: false,
      admin_password: false,
      linux_update_job: false,
      prune_backups: false
    }
    : {
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
  if (currentRuntimeCapabilities().runtimeKind === "desktop") {
    return {
      host_label: "local-macos",
      hostname: "macos",
      public_endpoint: null,
      capabilities: systemCapabilities,
      codex_home: "~/.codex",
      configured_codex_home: "~/.codex",
      resolved_codex_home: "~/.codex",
      codex_home_source: "default",
      panel_db: "~/Library/Application Support/NexusHub/panel.sqlite",
      state_db_integrity: "ok"
    };
  }
  return {
    host_label: "43.155.235.227",
    hostname: "codex-cloud-root",
    public_endpoint: "https://661313.xyz/nexushub/",
    capabilities: systemCapabilities,
    codex_home: "/root/.codex",
    configured_codex_home: "/root/.codex",
    resolved_codex_home: "/root/.codex",
    codex_home_source: "config",
    panel_db: "/opt/nexushub/panel.sqlite",
    state_db_integrity: "ok"
  };
}

function demoSecurity(): SecuritySettings {
  if (currentRuntimeCapabilities().runtimeKind === "desktop") {
    return {
      turnstile_enabled: false,
      turnstile_required: false,
      turnstile_site_key: "",
      turnstile_secret_configured: false,
      session_ttl_seconds: 31536000,
      turnstile_expected_hostname: null,
      turnstile_expected_action: null
    };
  }
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
  const desktop = currentRuntimeCapabilities().runtimeKind === "desktop";
  const logsPath = desktop
    ? "~/Library/Application Support/NexusHub/logs_2.sqlite"
    : "/root/.codex/logs_2.sqlite";
  return {
    codex: {
      home: system.codex_home,
      configured_codex_home: system.configured_codex_home,
      resolved_codex_home: system.resolved_codex_home,
      codex_home_source: system.codex_home_source,
      logs_db_source: "resolved_codex_home",
      discovery_warnings: [],
      workspace: desktop ? "~/Documents" : "/home/ubuntu/codex-workspace",
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
      path: logsPath,
      resolved_path: logsPath,
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
