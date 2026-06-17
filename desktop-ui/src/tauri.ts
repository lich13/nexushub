import { invoke } from "@tauri-apps/api/core";

export type NexusPaths = {
  appSupportDir: string;
  configFile: string;
  databaseFile: string;
  logDir: string;
  appLogFile: string;
};

export type DesktopOverview = {
  productName: string;
  version: string;
  identifier: string;
  os: string;
  arch: string;
  paths: NexusPaths;
  appSupportDirReady: boolean;
  logDirReady: boolean;
  configFileExists: boolean;
  databaseFileExists: boolean;
  codexHome: string;
  codexHomeSource: string;
};

export type ThreadSummary = {
  id: string;
  title: string;
  status: "Recent" | "Running" | "ReplyNeeded" | "Recoverable" | "Archived" | string;
  updated_at?: string | null;
  latest_message?: string | null;
  model?: string | null;
};

export type ProbeStatus = {
  enabled: boolean;
  hook_status: string;
  bark_status: string;
  logs_db_status: string;
  lifecycle_status: string;
  host_label: string;
  resolved_codex_home: string;
};

export type DeletePlan = {
  total_threads?: number;
  active_threads?: number;
  archived_threads?: number;
  hidden_threads?: number;
  visible_threads?: number;
  rollout_files?: number;
  integrity?: string;
};

export type DesktopGoal = {
  available: boolean;
  enabled: boolean;
  threadId?: string | null;
  objective?: string | null;
  tokenBudget?: number | null;
  status: string;
  completedAt?: number | null;
  blockedReason?: string | null;
};

export type DesktopHome = {
  overview: DesktopOverview;
  system?: unknown;
  probe?: ProbeStatus | null;
  logsDb?: unknown;
  threads: ThreadSummary[];
  plugins: Array<{ id: string; label: string; status: string }>;
  models: Array<{ id: string; label: string; default?: boolean }>;
  permissionProfiles: Array<{ id: string; label: string; default?: boolean }>;
  codexConfig: { cwd: string; permission_profile?: string; permissionProfile?: string };
  archivePlan?: DeletePlan | null;
  hiddenPlan?: DeletePlan | null;
  goal: DesktopGoal;
  warnings: string[];
};

const fallbackOverview: DesktopOverview = {
  productName: "NexusHub",
  version: "0.1.98",
  identifier: "com.lich13.nexushub",
  os: "macos",
  arch: "aarch64",
  paths: {
    appSupportDir: "~/Library/Application Support/NexusHub",
    configFile: "~/Library/Application Support/NexusHub/config.toml",
    databaseFile: "~/Library/Application Support/NexusHub/nexushub.sqlite",
    logDir: "~/Library/Logs/NexusHub",
    appLogFile: "~/Library/Logs/NexusHub/nexushub.log",
  },
  appSupportDirReady: false,
  logDirReady: false,
  configFileExists: false,
  databaseFileExists: false,
  codexHome: "~/.codex",
  codexHomeSource: "preview",
};

export async function loadDesktopOverview(): Promise<DesktopOverview> {
  if (!("__TAURI_INTERNALS__" in window)) {
    return fallbackOverview;
  }

  return invoke<DesktopOverview>("desktop_overview");
}

export async function loadDesktopHome(): Promise<DesktopHome> {
  if (!("__TAURI_INTERNALS__" in window)) {
    return {
      overview: fallbackOverview,
      probe: {
        enabled: true,
        hook_status: "preview",
        bark_status: "not_configured",
        logs_db_status: "preview",
        lifecycle_status: "native_app",
        host_label: "local",
        resolved_codex_home: "~/.codex",
      },
      threads: [
        {
          id: "preview",
          title: "桌面预览线程",
          status: "Recent",
          latest_message: "真实数据会在 Tauri App 中从本机 Codex 状态读取。",
        },
      ],
      plugins: [
        { id: "codex", label: "Codex", status: "ready" },
        { id: "probe", label: "Probe", status: "ready" },
        { id: "system_ops", label: "System/Ops", status: "ready" },
      ],
      models: [{ id: "gpt-5.5", label: "GPT-5.5", default: true }],
      permissionProfiles: [{ id: "danger-full-access", label: "Danger full access", default: true }],
      codexConfig: { cwd: "~/nexushub-workspace", permission_profile: "danger-full-access" },
      archivePlan: { total_threads: 0, archived_threads: 0, rollout_files: 0, integrity: "preview" },
      hiddenPlan: { total_threads: 0, hidden_threads: 0, rollout_files: 0, integrity: "preview" },
      goal: { available: true, enabled: false, threadId: "preview", status: "idle" },
      warnings: ["当前为浏览器预览，真实命令仅在 Tauri App 中执行。"],
    };
  }

  return invoke<DesktopHome>("desktop_home");
}

export async function saveDesktopGoal(
  threadId: string,
  objective: string,
  tokenBudget?: number | null,
): Promise<DesktopGoal> {
  if (!("__TAURI_INTERNALS__" in window)) {
    return {
      available: true,
      enabled: !!objective.trim(),
      threadId,
      objective: objective.trim() || null,
      tokenBudget: tokenBudget ?? null,
      status: objective.trim() ? "active" : "cleared",
    };
  }

  return invoke<DesktopGoal>("desktop_save_goal_command", {
    request: { threadId, objective, tokenBudget },
  });
}

export async function clearDesktopGoal(threadId: string): Promise<DesktopGoal> {
  if (!("__TAURI_INTERNALS__" in window)) {
    return { available: true, enabled: false, threadId, status: "cleared" };
  }

  return invoke<DesktopGoal>("desktop_clear_goal_command", { threadId });
}

export async function pauseDesktopGoal(threadId: string): Promise<DesktopGoal> {
  if (!("__TAURI_INTERNALS__" in window)) {
    return { available: true, enabled: true, threadId, status: "paused" };
  }

  return invoke<DesktopGoal>("desktop_pause_goal_command", { threadId });
}

export async function resumeDesktopGoal(threadId: string): Promise<DesktopGoal> {
  if (!("__TAURI_INTERNALS__" in window)) {
    return { available: true, enabled: true, threadId, status: "active" };
  }

  return invoke<DesktopGoal>("desktop_resume_goal_command", { threadId });
}

export async function openConfigDir(): Promise<void> {
  if (!("__TAURI_INTERNALS__" in window)) return;
  await invoke("desktop_open_config_dir_command");
}

export async function openLogDir(): Promise<void> {
  if (!("__TAURI_INTERNALS__" in window)) return;
  await invoke("desktop_open_log_dir_command");
}
