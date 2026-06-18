type RuntimeKind = "web" | "desktop";

type RpcArgs = Record<string, unknown> | undefined;

export type RuntimeUploadFile = {
  name: string;
  mime: string;
  bytes: number[];
};

export type RuntimeThreadEventSource = {
  unavailable?: boolean;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
  close(): void;
};

type WebRoute = {
  path?: string | ((args?: RpcArgs) => string);
  method?: string;
  body?: (args?: RpcArgs) => unknown;
  csrfArg?: string;
  skipContentType?: boolean;
  unavailable?: boolean;
};

type DesktopRoute = {
  command?: string;
  args?: (args?: RpcArgs) => RpcArgs;
  fromHome?: (home: DesktopHome, args?: RpcArgs) => unknown;
  fallback?: (args?: RpcArgs) => unknown;
  unavailable?: string;
};

export class RuntimeUnavailableError extends Error {
  constructor(message: string, readonly feature: string) {
    super(message);
    this.name = "RuntimeUnavailableError";
  }
}

type TauriInternals = {
  invoke?: (command: string, args?: RpcArgs) => Promise<unknown>;
};

type RuntimeGlobal = typeof globalThis & {
  __TAURI_INTERNALS__?: TauriInternals;
  __NEXUSHUB_DESKTOP_RUNTIME__?: boolean;
  __NEXUSHUB_TEST_INVOKE__?: (
    command: string,
    args?: RpcArgs,
  ) => Promise<unknown> | unknown;
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

type ProbeJobAction = "bark-test" | "hooks-install" | "logs-db-dry-run" | "logs-db-execute";

export function getRuntimeKind(): RuntimeKind {
  const target = globalThis as RuntimeGlobal;
  if (target.__NEXUSHUB_DESKTOP_RUNTIME__) {
    return "desktop";
  }
  if (target.__TAURI_INTERNALS__) {
    return "desktop";
  }
  return "web";
}

export function isDesktopRuntime(): boolean {
  return getRuntimeKind() === "desktop";
}

export function isWebRuntime(): boolean {
  return getRuntimeKind() === "web";
}

export function desktopSessionUser() {
  return {
    id: "desktop",
    username: "desktop",
    csrf_token: null,
    session_id: "desktop"
  };
}

function apiBase(): string {
  const raw = import.meta.env.VITE_API_BASE;
  const value = typeof raw === "string" ? raw.trim() : "";
  if (!value || value === "/") return "";
  if (/^https?:\/\//i.test(value)) return value.replace(/\/+$/g, "");
  return `/${value.replace(/^\/+|\/+$/g, "")}`;
}

export function buildRuntimeApiPath(path: string): string {
  if (/^https?:\/\//i.test(path)) {
    return path;
  }
  const normalizedPath = path.startsWith("/") ? path : `/${path}`;
  const base = apiBase();
  return base ? `${base}${normalizedPath}` : normalizedPath;
}

function unavailableThreadEventSource(): RuntimeThreadEventSource {
  return {
    unavailable: true,
    addEventListener: () => undefined,
    close: () => undefined
  };
}

export function createRuntimeThreadEventSource(threadId: string): RuntimeThreadEventSource {
  if (isDesktopRuntime()) {
    return unavailableThreadEventSource();
  }
  return new EventSource(
    buildRuntimeApiPath(`/api/threads/${encodeURIComponent(threadId)}/events`),
    { withCredentials: true },
  );
}

function createUnavailableApiError(message: string): Error & { status: number } {
  return Object.assign(new RuntimeUnavailableError(message, message), {
    status: 501
  });
}

async function parseResponse(response: Response): Promise<unknown> {
  const contentType = response.headers.get("content-type") ?? "";
  return contentType.includes("application/json")
    ? response.json()
    : response.text();
}

async function webRpc(route: WebRoute, args?: RpcArgs): Promise<unknown> {
  if (route.unavailable) {
    throw new RuntimeUnavailableError("Web endpoint is unavailable", "web");
  }
  const headers = new Headers();
  const body = route.body?.(args);
  if (!route.skipContentType && body !== undefined) {
    headers.set("content-type", "application/json");
  }
  if (route.csrfArg && typeof args?.[route.csrfArg] === "string") {
    headers.set("x-csrf-token", args[route.csrfArg] as string);
  }
  if (!route.path) {
    throw new RuntimeUnavailableError("Web endpoint is unavailable", "web");
  }
  const path = typeof route.path === "function" ? route.path(args) : route.path;
  const response = await fetch(buildRuntimeApiPath(path), {
    method: route.method ?? "GET",
    credentials: "include",
    headers,
    body: body instanceof FormData
      ? body
      : body === undefined
        ? undefined
        : JSON.stringify(body)
  });
  const payload = await parseResponse(response);
  if (!response.ok) {
    const message =
      payload && typeof payload === "object" && "error" in payload
        ? String((payload as { error: unknown }).error)
        : `请求失败，HTTP ${response.status}`;
    throw Object.assign(new Error(message), { status: response.status });
  }
  return payload;
}

async function invokeDesktop(command: string, args?: RpcArgs): Promise<unknown> {
  const target = globalThis as RuntimeGlobal;
  if (target.__NEXUSHUB_TEST_INVOKE__) {
    return target.__NEXUSHUB_TEST_INVOKE__(command, args);
  }
  if (target.__TAURI_INTERNALS__?.invoke) {
    return target.__TAURI_INTERNALS__.invoke(command, args);
  }
  throw new RuntimeUnavailableError(
    "Tauri invoke is not available in this runtime",
    command,
  );
}

async function desktopHome(): Promise<DesktopHome> {
  return (await invokeDesktop("desktop_home")) as DesktopHome;
}

async function desktopRpc(route: DesktopRoute, args?: RpcArgs): Promise<unknown> {
  if (route.unavailable) {
    throw new RuntimeUnavailableError(route.unavailable, route.unavailable);
  }
  if (route.fallback) {
    return route.fallback(args);
  }
  if (route.fromHome) {
    return route.fromHome(await desktopHome(), args);
  }
  if (!route.command) {
    throw new RuntimeUnavailableError("Desktop command is not configured", "desktop");
  }
  return invokeDesktop(route.command, route.args?.(args));
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

function unavailableOptional(reason: string) {
  return { available: false, reason, error: reason };
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

function updateWebPath(action: unknown): string {
  switch (action) {
    case "check":
      return "/api/system/update/precheck";
    case "install":
      return "/api/system/update/install";
    case "prune":
      return "/api/system/update/prune";
    default:
      throw new RuntimeUnavailableError(`Unknown update action: ${String(action)}`, "runUpdateAction");
  }
}

function probeJobWebRoute(action: unknown): { path: string; body?: Record<string, unknown> } {
  switch (action as ProbeJobAction) {
    case "bark-test":
      return { path: "/api/probe/bark/test" };
    case "hooks-install":
      return { path: "/api/probe/hooks/install" };
    case "logs-db-dry-run":
      return { path: "/api/probe/logs-db/maintain", body: { dry_run: true } };
    case "logs-db-execute":
      return { path: "/api/probe/logs-db/maintain", body: { dry_run: false, compact: false } };
    default:
      throw new RuntimeUnavailableError(`Unknown Probe job action: ${String(action)}`, "startProbeJob");
  }
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
    web: { path: "/api/public/settings" },
    desktop: { fallback: desktopPublicSettings }
  },
  desktopUploadFiles: {
    web: { unavailable: true },
    desktop: {
      command: "desktop_upload_files_command",
      args: (args) => ({ files: Array.isArray(args?.files) ? args.files : [] })
    }
  },
  login: {
    web: {
      path: "/api/auth/login",
      method: "POST",
      body: (args) => {
        const body: Record<string, unknown> = {
          username: args?.username,
          password: args?.password
        };
        if (typeof args?.turnstileToken === "string" && args.turnstileToken.trim()) {
          body.turnstile_token = args.turnstileToken.trim();
        }
        return body;
      }
    },
    desktop: { fallback: desktopSessionUser }
  },
  logout: {
    web: { path: "/api/auth/logout", method: "POST", csrfArg: "csrfToken" },
    desktop: { fallback: () => undefined }
  },
  me: {
    web: { path: "/api/auth/me" },
    desktop: { fallback: desktopSessionUser }
  },
  listThreads: {
    web: {
      path: (args) => {
        const params = new URLSearchParams();
        if (args?.status && args.status !== "all") params.set("status", String(args.status));
        if (typeof args?.q === "string" && args.q.trim()) params.set("q", args.q.trim());
        params.set("limit", String(args?.limit ?? 120));
        return `/api/threads?${params.toString()}`;
      }
    },
    desktop: {
      command: "desktop_threads",
      args: (args) => ({ request: threadListRequest(args) })
    }
  },
  getThread: {
    web: {
      path: (args) => {
        const id = encodeURIComponent(argString(args, "id"));
        const options = (args?.options && typeof args.options === "object"
          ? args.options
          : {}) as Record<string, unknown>;
        const params = new URLSearchParams();
        if (options.limit !== undefined) params.set("limit", String(options.limit));
        if (options.before) params.set("before", String(options.before));
        if (options.full) params.set("full", "true");
        const query = params.toString();
        return `/api/threads/${id}${query ? `?${query}` : ""}`;
      }
    },
    desktop: {
      command: "desktop_thread_detail",
      args: (args) => ({ request: threadDetailRequest(args) })
    }
  },
  getThreadBlocks: {
    web: {
      path: (args) => {
        const id = encodeURIComponent(argString(args, "id"));
        const options = (args?.options && typeof args.options === "object"
          ? args.options
          : {}) as Record<string, unknown>;
        const params = new URLSearchParams();
        if (options.limit !== undefined) params.set("limit", String(options.limit));
        if (options.before) params.set("before", String(options.before));
        const query = params.toString();
        return `/api/threads/${id}/blocks${query ? `?${query}` : ""}`;
      }
    },
    desktop: {
      command: "desktop_thread_blocks",
      args: (args) => ({ request: threadBlocksRequest(args) })
    }
  },
  getSystemStatus: {
    web: { path: "/api/system/status" },
    desktop: { fromHome: (home) => home.system ?? {} }
  },
  getSystemVersion: {
    web: { path: "/api/system/version" },
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
    web: { path: "/api/security" },
    desktop: {
      unavailable: "该宿主不支持安全设置"
    }
  },
  saveSecurity: {
    web: { path: "/api/security", method: "PATCH", csrfArg: "csrfToken", body: (args) => args?.settings ?? {} },
    desktop: { unavailable: "该宿主不支持安全设置" }
  },
  changePassword: {
    web: {
      path: "/api/security/password",
      method: "POST",
      csrfArg: "csrfToken",
      body: (args) => ({
        current_password: args?.current_password,
        new_password: args?.new_password
      })
    },
    desktop: { unavailable: "Desktop password command is not implemented" }
  },
  listProviders: {
    web: { path: "/api/providers" },
    desktop: { fromHome: (home) => home.plugins ?? [] }
  },
  getClaudeCodeOverview: {
    web: { path: "/api/providers/claude-code/overview" },
    desktop: { command: "desktop_claude_code_overview" }
  },
  getPlatformOverview: {
    web: { path: "/api/platform" },
    desktop: { fromHome: desktopPlatform }
  },
  listPlugins: {
    web: { path: "/api/plugins" },
    desktop: { fromHome: (home) => home.plugins ?? [] }
  },
  getProbeStatus: {
    web: { path: "/api/probe/status" },
    desktop: { command: "desktop_probe_status" }
  },
  getProbeSettings: {
    web: { path: "/api/probe/settings" },
    desktop: { command: "desktop_probe_settings" }
  },
  saveProbeSettings: {
    web: { path: "/api/probe/settings", method: "PATCH", csrfArg: "csrfToken", body: (args) => args?.settings ?? {} },
    desktop: {
      command: "desktop_probe_save_settings",
      args: (args) => ({ request: camelizeProbeSettings(objectArg(args, "settings")) })
    }
  },
  getProbeLogsDbStatus: {
    web: { path: "/api/probe/logs-db/status" },
    desktop: { fromHome: (home) => ({ available: true, data: home.logsDb ?? home.logs_db ?? {} }) }
  },
  getProbeEvents: {
    web: { path: (args) => `/api/probe/events?limit=${encodeURIComponent(String(args?.limit ?? 10))}` },
    desktop: {
      command: "desktop_probe_events",
      args: (args) => ({ request: { limit: args?.limit ?? 10 } })
    }
  },
  dryRunArchiveDelete: {
    web: { path: "/api/archives/delete/dry-run", method: "POST", csrfArg: "csrfToken" },
    desktop: { command: "desktop_archive_delete_dry_run" }
  },
  startArchiveDelete: {
    web: {
      path: "/api/archives/delete/execute",
      method: "POST",
      csrfArg: "csrfToken",
      body: () => ({ confirmed: true })
    },
    desktop: { command: "desktop_archive_delete_execute" }
  },
  dryRunHiddenThreadDelete: {
    web: { path: "/api/hidden-threads/delete/dry-run", method: "POST", csrfArg: "csrfToken" },
    desktop: { command: "desktop_hidden_delete_dry_run" }
  },
  startHiddenThreadDelete: {
    web: {
      path: "/api/hidden-threads/delete/execute",
      method: "POST",
      csrfArg: "csrfToken",
      body: () => ({ confirmed: true })
    },
    desktop: { command: "desktop_hidden_delete_execute" }
  },
  getUpdateStatus: {
    web: { path: "/api/system/update/status" },
    desktop: { command: "desktop_update_status" }
  },
	  runUpdateAction: {
	    web: { path: (args) => updateWebPath(args?.action), method: "POST", csrfArg: "csrfToken" },
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
    web: {
      path: (args) => probeJobWebRoute(args?.action).path,
      method: "POST",
      csrfArg: "csrfToken",
      body: (args) => probeJobWebRoute(args?.action).body
    },
    desktop: {
      fallback: (args) => desktopRpc(probeJobDesktopRoute(args?.action), args)
    }
  },
  uploadFiles: {
    web: { path: "/api/uploads", method: "POST", csrfArg: "csrfToken", body: (args) => args?.form, skipContentType: true },
    desktop: { command: "desktop_upload_files_command", args: (args) => ({ files: Array.isArray(args?.files) ? args.files : [] }) }
  },
  deleteUpload: {
    web: { path: (args) => `/api/uploads/${encodeURIComponent(argString(args, "id"))}`, method: "DELETE", csrfArg: "csrfToken" },
    desktop: {
      command: "desktop_delete_upload",
      args: (args) => ({ id: argString(args, "id") })
    }
  },
  createThread: {
    web: { path: "/api/threads", method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: {
      command: "desktop_send_message",
      args: (args) => ({ request: threadSendRequest(args) })
    }
  },
  sendMessage: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/messages`, method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: {
      command: "desktop_send_message",
      args: (args) => ({ request: threadSendRequest(args) })
    }
  },
  steerThread: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/steer`, method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: {
      command: "desktop_continue_thread",
      args: (args) => ({ request: threadSendRequest(args) })
    }
  },
  listFollowUps: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/follow-ups` },
    desktop: {
      command: "desktop_list_followups",
      args: (args) => ({ request: { threadId: argString(args, "threadId"), limit: args?.limit ?? 20 } })
    }
  },
  enqueueFollowUp: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/follow-ups`, method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: {
      command: "desktop_enqueue_followup",
      args: (args) => ({ request: threadSendRequest(args) })
    }
  },
  cancelFollowUp: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/follow-ups/${encodeURIComponent(argString(args, "followUpId"))}/cancel`, method: "POST", csrfArg: "csrfToken" },
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
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/stop`, method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: {
      command: "desktop_stop_thread",
      args: (args) => ({ request: { threadId: argString(args, "threadId"), ...objectArg(args, "payload") } })
    }
  },
  archiveThread: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/archive`, method: "POST", csrfArg: "csrfToken" },
    desktop: {
      command: "desktop_archive_thread",
      args: (args) => ({ request: { threadId: argString(args, "threadId") } })
    }
  },
  restoreThread: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/restore`, method: "POST", csrfArg: "csrfToken" },
    desktop: {
      command: "desktop_restore_thread",
      args: (args) => ({ request: { threadId: argString(args, "threadId") } })
    }
  },
  renameThread: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/rename`, method: "POST", csrfArg: "csrfToken", body: (args) => ({ name: args?.name }) },
    desktop: {
      command: "desktop_rename_thread",
      args: (args) => ({ request: { threadId: argString(args, "threadId"), name: args?.name } })
    }
  },
  forkThread: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/fork`, method: "POST", csrfArg: "csrfToken" },
    desktop: { unavailable: "Desktop fork command is not implemented" }
  },
  answerElicitation: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/elicitation`, method: "POST", csrfArg: "csrfToken", body: (args) => ({ answers: args?.answers ?? {} }) },
    desktop: {
      command: "desktop_answer_elicitation",
      args: (args) => ({ request: { threadId: argString(args, "threadId"), answers: args?.answers ?? {} } })
    }
  },
  acceptPlan: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/plan/accept`, method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: {
      command: "desktop_plan_accept",
      args: (args) => ({ request: planRequest(args) })
    }
  },
  revisePlan: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/plan/revise`, method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: {
      command: "desktop_plan_revise",
      args: (args) => ({ request: planRequest(args) })
    }
  },
  answerApproval: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/approval`, method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: { unavailable: "Desktop approval command is not implemented" }
  },
  listModels: {
    web: { path: "/api/codex/models" },
    desktop: { fromHome: (home) => ({ available: true, data: home.models ?? [] }) }
  },
  listPermissionProfiles: {
    web: { path: "/api/codex/permission-profiles" },
    desktop: { fromHome: (home) => ({ available: true, data: home.permissionProfiles ?? home.permission_profiles ?? [] }) }
  },
  getCodexConfig: {
    web: { path: "/api/codex/config" },
    desktop: { fromHome: (home) => ({ available: true, data: home.codexConfig ?? home.codex_config ?? {} }) }
  },
  getCodexGoal: {
    web: { path: (args) => `/api/codex/goal?${new URLSearchParams({ thread_id: argString(args, "threadId") }).toString()}` },
    desktop: { fromHome: desktopGoalFromHome }
  },
  saveCodexGoal: {
    web: {
      path: "/api/codex/goal",
      method: "POST",
      csrfArg: "csrfToken",
      body: (args) => ({
        thread_id: args?.threadId,
        objective: args?.objective,
        token_budget: args?.tokenBudget ?? null
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
    web: { path: "/api/codex/goal/clear", method: "POST", csrfArg: "csrfToken", body: (args) => ({ thread_id: args?.threadId }) },
    desktop: { command: "desktop_clear_goal_command", args: (args) => ({ threadId: argString(args, "threadId") }) }
  },
  pauseCodexGoal: {
    web: { path: "/api/codex/goal/pause", method: "POST", csrfArg: "csrfToken", body: (args) => ({ thread_id: args?.threadId }) },
    desktop: { command: "desktop_pause_goal_command", args: (args) => ({ threadId: argString(args, "threadId") }) }
  },
  resumeCodexGoal: {
    web: { path: "/api/codex/goal/resume", method: "POST", csrfArg: "csrfToken", body: (args) => ({ thread_id: args?.threadId }) },
    desktop: { command: "desktop_resume_goal_command", args: (args) => ({ threadId: argString(args, "threadId") }) }
  },
  listJobs: {
    web: { path: "/api/jobs?limit=30" },
    desktop: {
      command: "desktop_jobs",
      args: () => ({ request: { limit: 30 } })
    }
  },
  getJob: {
    web: { path: (args) => `/api/jobs/${encodeURIComponent(argString(args, "id"))}` },
    desktop: {
      command: "desktop_job_detail",
      args: (args) => ({ request: { id: argString(args, "id") } })
    }
  }
};

export async function runtimeRpc<T = unknown>(
  name: keyof typeof ROUTES | string,
  args?: RpcArgs,
): Promise<T> {
  const route = ROUTES[name];
  if (!route) {
    throw new RuntimeUnavailableError(`Unknown runtime RPC: ${name}`, name);
  }
  const result = isDesktopRuntime()
    ? await desktopRpc(route.desktop, args)
    : await webRpc(route.web, args);
  return result as T;
}

export async function invokeDesktopUpload<T = unknown>(
  uploads: RuntimeUploadFile[],
): Promise<T> {
  return invokeDesktop("desktop_upload_files_command", { files: uploads }) as Promise<T>;
}
