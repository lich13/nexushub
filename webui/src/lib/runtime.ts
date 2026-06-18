type RuntimeKind = "web" | "desktop";

type RpcArgs = Record<string, unknown> | undefined;

export type DesktopApiUpload = {
  name: string;
  mime: string;
  bytes: number[];
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

function desktopApiRoute(
  path: string | ((args?: RpcArgs) => string),
  method?: string,
  body?: (args?: RpcArgs) => unknown,
): DesktopRoute {
  return {
    fallback: (args) => invokeDesktopApi({
      path: typeof path === "function" ? path(args) : path,
      method,
      body: body?.(args)
    })
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

const ROUTES: Record<string, { web: WebRoute; desktop: DesktopRoute }> = {
  getPublicSettings: {
    web: { path: "/api/public/settings" },
    desktop: { fallback: desktopPublicSettings }
  },
  desktopApi: {
    web: { unavailable: true },
    desktop: {
      command: "desktop_api_command",
      args: (args) => args?.request && typeof args.request === "object"
        ? { request: args.request as Record<string, unknown> }
        : { request: { path: "/api/public/settings", method: "GET" } }
    }
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
      command: "desktop_threads_command",
      args: (args) => ({
        request: {
          status: args?.status,
          query: args?.q,
          limit: args?.limit
        }
      })
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
      command: "desktop_thread_detail_command",
      args: (args) => ({ id: argString(args, "id") })
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
      args: (args) => {
        const options = (args?.options && typeof args.options === "object"
          ? args.options
          : {}) as Record<string, unknown>;
        return {
          request: {
            id: argString(args, "id"),
            limit: options.limit,
            before: options.before
          }
        };
      }
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
      unavailable: "安全/Turnstile 配置是 Linux WebUI 专属入口，macOS 桌面端不会调用 Web auth 或 CSRF。"
    }
  },
  saveSecurity: {
    web: { path: "/api/security", method: "PATCH", csrfArg: "csrfToken", body: (args) => args?.settings ?? {} },
    desktop: { unavailable: "Desktop security settings command is not implemented" }
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
    desktop: desktopApiRoute("/api/providers/claude-code/overview", "GET")
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
    desktop: { command: "desktop_probe_status_command" }
  },
  getProbeSettings: {
    web: { path: "/api/probe/settings" },
    desktop: desktopApiRoute("/api/probe/settings", "GET")
  },
  saveProbeSettings: {
    web: { path: "/api/probe/settings", method: "PATCH", csrfArg: "csrfToken", body: (args) => args?.settings ?? {} },
    desktop: desktopApiRoute("/api/probe/settings", "PATCH", (args) => args?.settings ?? {})
  },
  getProbeLogsDbStatus: {
    web: { path: "/api/probe/logs-db/status" },
    desktop: { fromHome: (home) => ({ available: true, data: home.logsDb ?? home.logs_db ?? {} }) }
  },
  getProbeEvents: {
    web: { path: (args) => `/api/probe/events?limit=${encodeURIComponent(String(args?.limit ?? 10))}` },
    desktop: desktopApiRoute((args) => `/api/probe/events?limit=${encodeURIComponent(String(args?.limit ?? 10))}`, "GET")
  },
  dryRunArchiveDelete: {
    web: { path: "/api/archives/delete/dry-run", method: "POST", csrfArg: "csrfToken" },
    desktop: { command: "desktop_archive_plan_command" }
  },
  startArchiveDelete: {
    web: {
      path: "/api/archives/delete/execute",
      method: "POST",
      csrfArg: "csrfToken",
      body: () => ({ confirmed: true })
    },
    desktop: desktopApiRoute("/api/archives/delete/execute", "POST", () => ({ confirmed: true }))
  },
  dryRunHiddenThreadDelete: {
    web: { path: "/api/hidden-threads/delete/dry-run", method: "POST", csrfArg: "csrfToken" },
    desktop: { command: "desktop_hidden_plan_command" }
  },
  startHiddenThreadDelete: {
    web: {
      path: "/api/hidden-threads/delete/execute",
      method: "POST",
      csrfArg: "csrfToken",
      body: () => ({ confirmed: true })
    },
    desktop: desktopApiRoute("/api/hidden-threads/delete/execute", "POST", () => ({ confirmed: true }))
  },
  startUpdateJob: {
    web: { path: (args) => String(args?.path ?? ""), method: "POST", csrfArg: "csrfToken" },
    desktop: { unavailable: "Desktop update jobs command is not implemented" }
  },
  getUpdateStatus: {
    web: { path: "/api/system/update/status" },
    desktop: { command: "desktop_update_status" }
  },
  runUpdateAction: {
    web: { path: (args) => String(args?.path ?? ""), method: "POST", csrfArg: "csrfToken" },
    desktop: {
      fallback: (args) => {
        const action = args?.action;
        return action === "install"
          ? invokeDesktop("install_update_and_restart")
          : invokeDesktop("check_update_status");
      }
    }
  },
  startProbeJob: {
    web: { path: (args) => String(args?.path ?? ""), method: "POST", csrfArg: "csrfToken", body: (args) => args?.body },
    desktop: desktopApiRoute((args) => String(args?.path ?? ""), "POST", (args) => args?.body)
  },
  uploadFiles: {
    web: { path: "/api/uploads", method: "POST", csrfArg: "csrfToken", body: (args) => args?.form, skipContentType: true },
    desktop: { command: "desktop_upload_files_command", args: (args) => ({ files: Array.isArray(args?.files) ? args.files : [] }) }
  },
  deleteUpload: {
    web: { path: (args) => `/api/uploads/${encodeURIComponent(argString(args, "id"))}`, method: "DELETE", csrfArg: "csrfToken" },
    desktop: desktopApiRoute((args) => `/api/uploads/${encodeURIComponent(argString(args, "id"))}`, "DELETE")
  },
  createThread: {
    web: { path: "/api/threads", method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: desktopApiRoute("/api/threads", "POST", (args) => args?.payload ?? {})
  },
  sendMessage: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/messages`, method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: desktopApiRoute((args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/messages`, "POST", (args) => args?.payload ?? {})
  },
  steerThread: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/steer`, method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: desktopApiRoute((args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/steer`, "POST", (args) => args?.payload ?? {})
  },
  listFollowUps: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/follow-ups` },
    desktop: desktopApiRoute((args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/follow-ups`, "GET")
  },
  enqueueFollowUp: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/follow-ups`, method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: desktopApiRoute((args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/follow-ups`, "POST", (args) => args?.payload ?? {})
  },
  cancelFollowUp: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/follow-ups/${encodeURIComponent(argString(args, "followUpId"))}/cancel`, method: "POST", csrfArg: "csrfToken" },
    desktop: desktopApiRoute((args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/follow-ups/${encodeURIComponent(argString(args, "followUpId"))}/cancel`, "POST")
  },
  stopThread: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/stop`, method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: desktopApiRoute((args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/stop`, "POST", (args) => args?.payload ?? {})
  },
  archiveThread: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/archive`, method: "POST", csrfArg: "csrfToken" },
    desktop: desktopApiRoute((args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/archive`, "POST")
  },
  restoreThread: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/restore`, method: "POST", csrfArg: "csrfToken" },
    desktop: desktopApiRoute((args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/restore`, "POST")
  },
  renameThread: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/rename`, method: "POST", csrfArg: "csrfToken", body: (args) => ({ name: args?.name }) },
    desktop: desktopApiRoute((args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/rename`, "POST", (args) => ({ name: args?.name }))
  },
  forkThread: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/fork`, method: "POST", csrfArg: "csrfToken" },
    desktop: { unavailable: "Desktop fork command is not implemented" }
  },
  answerElicitation: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/elicitation`, method: "POST", csrfArg: "csrfToken", body: (args) => ({ answers: args?.answers ?? {} }) },
    desktop: desktopApiRoute((args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/elicitation`, "POST", (args) => ({ answers: args?.answers ?? {} }))
  },
  acceptPlan: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/plan/accept`, method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: desktopApiRoute((args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/plan/accept`, "POST", (args) => args?.payload ?? {})
  },
  revisePlan: {
    web: { path: (args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/plan/revise`, method: "POST", csrfArg: "csrfToken", body: (args) => args?.payload ?? {} },
    desktop: desktopApiRoute((args) => `/api/threads/${encodeURIComponent(argString(args, "threadId"))}/plan/revise`, "POST", (args) => args?.payload ?? {})
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
    desktop: desktopApiRoute("/api/jobs?limit=30", "GET")
  },
  getJob: {
    web: { path: (args) => `/api/jobs/${encodeURIComponent(argString(args, "id"))}` },
    desktop: desktopApiRoute((args) => `/api/jobs/${encodeURIComponent(argString(args, "id"))}`, "GET")
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

function parseJsonBody(body?: unknown): Record<string, unknown> {
  if (!body) return {};
  if (typeof body === "string") {
    try {
      const parsed = JSON.parse(body);
      return parsed && typeof parsed === "object"
        ? parsed as Record<string, unknown>
        : {};
    } catch {
      return {};
    }
  }
  return body && typeof body === "object"
    ? body as Record<string, unknown>
    : {};
}

function stripQuery(path: string): { pathname: string; query: URLSearchParams } {
  const [rawPathname, rawQuery = ""] = path.split("?", 2);
  let pathname = rawPathname;
  try {
    pathname = decodeURIComponent(rawPathname);
  } catch {
    pathname = rawPathname;
  }
  return { pathname, query: new URLSearchParams(rawQuery) };
}

function desktopThreadPage(detail: unknown, threadId: string) {
  const value = detail && typeof detail === "object"
    ? detail as {
      blocks?: unknown[];
      total_blocks?: number;
      has_more_blocks?: boolean;
      before_cursor?: string | null;
    }
    : {};
  return {
    thread_id: threadId,
    blocks: Array.isArray(value.blocks) ? value.blocks : [],
    total_blocks: typeof value.total_blocks === "number"
      ? value.total_blocks
      : Array.isArray(value.blocks)
        ? value.blocks.length
        : 0,
    has_more_blocks: Boolean(value.has_more_blocks),
    before_cursor: value.before_cursor ?? null
  };
}

export async function invokeDesktopApi<T = unknown>(request: {
  path: string;
  method?: string;
  body?: unknown;
}): Promise<T> {
  const method = request.method ?? (request.body ? "POST" : "GET");
  const body = parseJsonBody(request.body);
  return invokeDesktop("desktop_api_command", {
    request: {
      path: request.path,
      method,
      body: Object.keys(body).length ? body : undefined
    }
  }) as Promise<T>;
}

export async function invokeDesktopUpload<T = unknown>(
  uploads: DesktopApiUpload[],
): Promise<T> {
  return invokeDesktop("desktop_upload_files_command", { files: uploads }) as Promise<T>;
}
