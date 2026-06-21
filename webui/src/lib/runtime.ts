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

function getRuntimeKind(): RuntimeKind {
  const target = globalThis as RuntimeGlobal;
  if (target.__NEXUSHUB_DESKTOP_RUNTIME__) {
    return "desktop";
  }
  if (target.__TAURI_INTERNALS__) {
    return "desktop";
  }
  return "web";
}

export function selectRuntimeFallback<T>(options: { web: T; desktop: T }): T {
  return getRuntimeKind() === "desktop" ? options.desktop : options.web;
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
  if (getRuntimeKind() === "desktop") {
    return unavailableThreadEventSource();
  }
  return new EventSource(
    buildRuntimeApiPath(`/api/rpc/threadEvents/${encodeURIComponent(threadId)}`),
    { withCredentials: true },
  );
}

async function parseResponse(response: Response): Promise<unknown> {
  const contentType = response.headers.get("content-type") ?? "";
  return contentType.includes("application/json")
    ? response.json()
    : response.text();
}

function csrfTokenFromArgs(args?: RpcArgs): string | null {
  const value = args?.csrfToken ?? args?.csrf_token;
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function rpcBodyArgs(args?: RpcArgs): RpcArgs {
  if (!args) return {};
  const { csrfToken: _csrfToken, csrf_token: _csrf_token, ...body } = args;
  return body;
}

function desktopRpcArgs(args?: RpcArgs): RpcArgs {
  const body = rpcBodyArgs(args) ?? {};
  return Object.keys(body).length ? body : undefined;
}

async function checkedResponse(response: Response): Promise<unknown> {
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

async function webJsonRpc<T = unknown>(
  command: string,
  args?: RpcArgs,
): Promise<T> {
  const headers = new Headers();
  headers.set("content-type", "application/json");
  const csrfToken = csrfTokenFromArgs(args);
  if (csrfToken) {
    headers.set("x-csrf-token", csrfToken);
  }
  const response = await fetch(
    buildRuntimeApiPath(`/api/rpc/${encodeURIComponent(command)}`),
    {
      method: "POST",
      credentials: "include",
      headers,
      body: JSON.stringify(rpcBodyArgs(args))
    },
  );
  return checkedResponse(response) as Promise<T>;
}

async function webFormRpc<T = unknown>(
  command: string,
  form: FormData,
  csrfToken?: string | null,
): Promise<T> {
  const headers = new Headers();
  if (csrfToken?.trim()) {
    headers.set("x-csrf-token", csrfToken.trim());
  }
  const response = await fetch(
    buildRuntimeApiPath(`/api/rpc/${encodeURIComponent(command)}`),
    {
      method: "POST",
      credentials: "include",
      headers,
      body: form
    },
  );
  return checkedResponse(response) as Promise<T>;
}

async function invokeDesktop<T = unknown>(
  command: string,
  args?: RpcArgs,
): Promise<T> {
  const target = globalThis as RuntimeGlobal;
  if (target.__NEXUSHUB_TEST_INVOKE__) {
    return target.__NEXUSHUB_TEST_INVOKE__(command, args) as Promise<T>;
  }
  if (target.__TAURI_INTERNALS__?.invoke) {
    return target.__TAURI_INTERNALS__.invoke(command, args) as Promise<T>;
  }
  throw new RuntimeUnavailableError(
    "Tauri invoke is not available in this runtime",
    command,
  );
}

export async function runtimeRpc<T = unknown>(
  command: string,
  args?: RpcArgs,
): Promise<T> {
  if (getRuntimeKind() === "desktop") {
    return invokeDesktop<T>(command, desktopRpcArgs(args));
  }
  return webJsonRpc<T>(command, args);
}

export async function uploadRuntimeFiles<T = unknown>(
  files: File[],
  csrfToken?: string | null,
): Promise<T> {
  if (getRuntimeKind() === "desktop") {
    const uploads: RuntimeUploadFile[] = await Promise.all(files.map(async (file) => ({
      name: file.name,
      mime: file.type || "application/octet-stream",
      bytes: Array.from(new Uint8Array(await file.arrayBuffer()))
    })));
    return invokeDesktop<T>("uploadFiles", { files: uploads });
  }

  const form = new FormData();
  for (const file of files) {
    form.append("files", file, file.name);
  }
  return webFormRpc<T>("uploadFiles", form, csrfToken);
}
