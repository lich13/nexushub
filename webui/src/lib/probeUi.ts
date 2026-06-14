import type { ProbeActionPlan, ProbeJobAction, ProbeSettings } from "../types";

export const PROBE_NAV_LABEL = "探针";

export const probeSections = [
  { id: "overview", label: "总览" },
  { id: "threads", label: "线程" },
  { id: "events", label: "事件" },
  { id: "diagnostics", label: "诊断" },
  { id: "settings", label: "设置与迁移" }
] as const;

export type ProbeSectionId = typeof probeSections[number]["id"];

export type ProbeSettingsDraft = {
  codex: {
    home: string;
    workspace: string;
    app_server_service: string;
    app_server_socket: string;
    bridge_enabled: boolean;
    bridge_transport: string;
    bridge_timeout_seconds: number;
    host_label: string;
  };
  probe: {
    enabled: boolean;
    poll_seconds: ProbeNumericDraftValue;
    recent_limit: ProbeNumericDraftValue;
  };
  hooks: {
    manage_stop_hook: boolean;
    reload_app_server_after_install: boolean;
  };
  notifications: {
    enabled: boolean;
    device_key: string;
    device_key_configured: boolean;
    server_url: string;
    sound: string;
    group: string;
    url: string;
    notify_completion: boolean;
    notify_reply_needed: boolean;
    notify_recoverable: boolean;
  };
  observability: {
    hook_event_max_lines: ProbeNumericDraftValue;
    hook_cooldown_max_lines: ProbeNumericDraftValue;
    log_max_bytes: ProbeNumericDraftValue;
  };
  logs_db: {
    enabled: boolean;
    retention_days: ProbeNumericDraftValue;
    maintenance_interval_hours: ProbeNumericDraftValue;
    maintain_on_codex_exit: boolean;
    codex_exit_grace_seconds: ProbeNumericDraftValue;
    codex_exit_max_wait_seconds: ProbeNumericDraftValue;
    delete_chunk_rows: ProbeNumericDraftValue;
    max_delete_rows_per_run: ProbeNumericDraftValue;
    busy_timeout_ms: ProbeNumericDraftValue;
    auto_compact_when_codex_closed: boolean;
    compact_interval_hours: ProbeNumericDraftValue;
    compact_min_freelist_mb: ProbeNumericDraftValue;
    compact_min_freelist_ratio_percent: ProbeNumericDraftValue;
    minimum_free_space_mb: ProbeNumericDraftValue;
  };
};

export type ProbeNumericDraftValue = number | "";

export const PROBE_MAC_DEFAULTS = {
  observability: {
    hook_event_max_lines: 500,
    hook_cooldown_max_lines: 1000,
    log_max_bytes: 5 * 1024 * 1024
  },
  logs_db: {
    retention_days: 2,
    maintenance_interval_hours: 6,
    codex_exit_grace_seconds: 5,
    codex_exit_max_wait_seconds: 1800,
    delete_chunk_rows: 5000,
    max_delete_rows_per_run: 100000,
    busy_timeout_ms: 500,
    compact_interval_hours: 24,
    compact_min_freelist_mb: 256,
    compact_min_freelist_ratio_percent: 20,
    minimum_free_space_mb: 1024
  }
} as const;

export type ProbeActionUiState = {
  action: ProbeJobAction;
  phase: "idle" | "planning" | "awaiting-confirmation" | "executing" | "executed" | "error";
  plan: ProbeActionPlan | null;
  error: string | null;
  jobId: string | null;
  canExecute: boolean;
  confirmLabel: string;
};

export type ProbeActionUiEvent =
  | { type: "planning" }
  | { type: "planned"; plan: ProbeActionPlan }
  | { type: "executing" }
  | { type: "executed"; jobId: string }
  | { type: "error"; message: string }
  | { type: "reset" };

export function buildProbeSettingsDraft(settings: ProbeSettings): ProbeSettingsDraft {
  return {
    codex: {
      home: settings.codex.home ?? "",
      workspace: settings.codex.workspace ?? "",
      app_server_service: settings.codex.app_server_service ?? "",
      app_server_socket: settings.codex.app_server_socket ?? "",
      bridge_enabled: Boolean(settings.codex.bridge_enabled),
      bridge_transport: settings.codex.bridge_transport ?? "websocket",
      bridge_timeout_seconds: toBoundedInteger(settings.codex.bridge_timeout_seconds, 20)
        ?? 20,
      host_label: settings.codex.host_label ?? ""
    },
    probe: {
      enabled: Boolean(settings.probe.enabled),
      poll_seconds: toBoundedInteger(settings.probe.poll_seconds, 15) ?? 15,
      recent_limit: toBoundedInteger(settings.probe.recent_limit, 50) ?? 50
    },
    hooks: {
      manage_stop_hook: settings.probe.hooks?.manage_stop_hook !== false,
      reload_app_server_after_install: settings.probe.hooks?.reload_app_server_after_install !== false
    },
    notifications: {
      enabled: Boolean(settings.notifications.enabled || settings.notifications.device_key_configured),
      device_key: "",
      device_key_configured: Boolean(settings.notifications.device_key_configured),
      server_url: settings.notifications.server_url ?? "https://api.day.app",
      sound: stringOrEmpty(settings.notifications.sound),
      group: typeof settings.notifications.group === "string" ? settings.notifications.group : "NexusHub",
      url: stringOrEmpty(settings.notifications.url),
      notify_completion: settings.notifications.notify_completion !== false,
      notify_reply_needed: settings.notifications.notify_reply_needed !== false,
      notify_recoverable: settings.notifications.notify_recoverable !== false
    },
    observability: {
      hook_event_max_lines: toBoundedInteger(settings.probe.observability?.hook_event_max_lines, PROBE_MAC_DEFAULTS.observability.hook_event_max_lines) ?? PROBE_MAC_DEFAULTS.observability.hook_event_max_lines,
      hook_cooldown_max_lines: toBoundedInteger(settings.probe.observability?.hook_cooldown_max_lines, PROBE_MAC_DEFAULTS.observability.hook_cooldown_max_lines) ?? PROBE_MAC_DEFAULTS.observability.hook_cooldown_max_lines,
      log_max_bytes: toBoundedInteger(settings.probe.observability?.log_max_bytes, PROBE_MAC_DEFAULTS.observability.log_max_bytes) ?? PROBE_MAC_DEFAULTS.observability.log_max_bytes
    },
    logs_db: {
      enabled: Boolean(settings.logs_db.enabled),
      retention_days: toBoundedInteger(settings.logs_db.retention_days, PROBE_MAC_DEFAULTS.logs_db.retention_days) ?? PROBE_MAC_DEFAULTS.logs_db.retention_days,
      maintenance_interval_hours: toBoundedInteger(settings.logs_db.maintenance_interval_hours, PROBE_MAC_DEFAULTS.logs_db.maintenance_interval_hours) ?? PROBE_MAC_DEFAULTS.logs_db.maintenance_interval_hours,
      maintain_on_codex_exit: settings.logs_db.maintain_on_codex_exit !== false,
      codex_exit_grace_seconds: toBoundedInteger(settings.logs_db.codex_exit_grace_seconds, PROBE_MAC_DEFAULTS.logs_db.codex_exit_grace_seconds) ?? PROBE_MAC_DEFAULTS.logs_db.codex_exit_grace_seconds,
      codex_exit_max_wait_seconds: toBoundedInteger(settings.logs_db.codex_exit_max_wait_seconds, PROBE_MAC_DEFAULTS.logs_db.codex_exit_max_wait_seconds) ?? PROBE_MAC_DEFAULTS.logs_db.codex_exit_max_wait_seconds,
      delete_chunk_rows: toBoundedInteger(settings.logs_db.delete_chunk_rows, PROBE_MAC_DEFAULTS.logs_db.delete_chunk_rows) ?? PROBE_MAC_DEFAULTS.logs_db.delete_chunk_rows,
      max_delete_rows_per_run: toBoundedInteger(settings.logs_db.max_delete_rows_per_run, PROBE_MAC_DEFAULTS.logs_db.max_delete_rows_per_run) ?? PROBE_MAC_DEFAULTS.logs_db.max_delete_rows_per_run,
      busy_timeout_ms: toBoundedInteger(settings.logs_db.busy_timeout_ms, PROBE_MAC_DEFAULTS.logs_db.busy_timeout_ms) ?? PROBE_MAC_DEFAULTS.logs_db.busy_timeout_ms,
      auto_compact_when_codex_closed: settings.logs_db.auto_compact_when_codex_closed !== false,
      compact_interval_hours: toBoundedInteger(settings.logs_db.compact_interval_hours, PROBE_MAC_DEFAULTS.logs_db.compact_interval_hours) ?? PROBE_MAC_DEFAULTS.logs_db.compact_interval_hours,
      compact_min_freelist_mb: toBoundedInteger(settings.logs_db.compact_min_freelist_mb, PROBE_MAC_DEFAULTS.logs_db.compact_min_freelist_mb) ?? PROBE_MAC_DEFAULTS.logs_db.compact_min_freelist_mb,
      compact_min_freelist_ratio_percent: toBoundedInteger(settings.logs_db.compact_min_freelist_ratio_percent, PROBE_MAC_DEFAULTS.logs_db.compact_min_freelist_ratio_percent) ?? PROBE_MAC_DEFAULTS.logs_db.compact_min_freelist_ratio_percent,
      minimum_free_space_mb: toBoundedInteger(settings.logs_db.minimum_free_space_mb, PROBE_MAC_DEFAULTS.logs_db.minimum_free_space_mb) ?? PROBE_MAC_DEFAULTS.logs_db.minimum_free_space_mb
    }
  };
}

export type ProbeSettingsPayload = {
  codex: ProbeSettings["codex"];
  probe: Pick<ProbeSettings["probe"], "enabled" | "poll_seconds" | "recent_limit" | "hooks" | "notifications" | "observability" | "logs_db">;
};

export function buildProbeSettingsPayload(draft: ProbeSettingsDraft, _current?: ProbeSettings | null): ProbeSettingsPayload {
  const errors = probeSettingsValidation(draft);
  if (errors.length) {
    throw new Error(errors[0]);
  }
  const notifications: NonNullable<ProbeSettingsPayload["probe"]["notifications"]> & { device_key?: string } = {
    enabled: draft.notifications.enabled || Boolean(draft.notifications.device_key.trim()) || draft.notifications.device_key_configured,
    server_url: draft.notifications.server_url.trim(),
    sound: optionalString(draft.notifications.sound),
    group: draft.notifications.group.trim(),
    url: optionalString(draft.notifications.url),
    notify_completion: draft.notifications.notify_completion,
    notify_reply_needed: draft.notifications.notify_reply_needed,
    notify_recoverable: draft.notifications.notify_recoverable
  };
  const deviceKey = draft.notifications.device_key.trim();
  if (deviceKey) {
    notifications.device_key = deviceKey;
  }

  return {
    codex: {
      home: draft.codex.home.trim(),
      workspace: draft.codex.workspace.trim() || null,
      app_server_service: draft.codex.app_server_service.trim(),
      app_server_socket: draft.codex.app_server_socket.trim() || null,
      bridge_enabled: draft.codex.bridge_enabled,
      bridge_transport: draft.codex.bridge_transport.trim() || "websocket",
      bridge_timeout_seconds: draft.codex.bridge_timeout_seconds,
      host_label: draft.codex.host_label.trim()
    },
    probe: {
      enabled: draft.probe.enabled,
      poll_seconds: requiredDraftNumber(draft.probe.poll_seconds),
      recent_limit: requiredDraftNumber(draft.probe.recent_limit),
      hooks: {
        manage_stop_hook: draft.hooks.manage_stop_hook,
        reload_app_server_after_install: draft.hooks.reload_app_server_after_install
      },
      notifications,
      observability: {
        hook_event_max_lines: requiredDraftNumber(draft.observability.hook_event_max_lines),
        hook_cooldown_max_lines: requiredDraftNumber(draft.observability.hook_cooldown_max_lines),
        log_max_bytes: requiredDraftNumber(draft.observability.log_max_bytes)
      },
      logs_db: {
        enabled: draft.logs_db.enabled,
        retention_days: requiredDraftNumber(draft.logs_db.retention_days),
        maintenance_interval_hours: requiredDraftNumber(draft.logs_db.maintenance_interval_hours),
        maintain_on_codex_exit: draft.logs_db.maintain_on_codex_exit,
        codex_exit_grace_seconds: requiredDraftNumber(draft.logs_db.codex_exit_grace_seconds),
        codex_exit_max_wait_seconds: requiredDraftNumber(draft.logs_db.codex_exit_max_wait_seconds),
        delete_chunk_rows: requiredDraftNumber(draft.logs_db.delete_chunk_rows),
        max_delete_rows_per_run: requiredDraftNumber(draft.logs_db.max_delete_rows_per_run),
        busy_timeout_ms: requiredDraftNumber(draft.logs_db.busy_timeout_ms),
        auto_compact_when_codex_closed: draft.logs_db.auto_compact_when_codex_closed,
        compact_interval_hours: requiredDraftNumber(draft.logs_db.compact_interval_hours),
        compact_min_freelist_mb: requiredDraftNumber(draft.logs_db.compact_min_freelist_mb),
        compact_min_freelist_ratio_percent: requiredDraftNumber(draft.logs_db.compact_min_freelist_ratio_percent),
        minimum_free_space_mb: requiredDraftNumber(draft.logs_db.minimum_free_space_mb)
      }
    }
  };
}

export function probeSettingsValidation(draft: ProbeSettingsDraft): string[] {
  const errors: string[] = [];
  if (!draft.codex.home.trim()) errors.push("Codex Home 不能为空");
  if (!draft.codex.app_server_service.trim()) errors.push("app-server 服务不能为空");
  if (!draft.codex.host_label.trim()) errors.push("主机标签不能为空");
  if (!isIntegerInRange(draft.probe.poll_seconds, 5, 3600)) {
    errors.push("轮询间隔必须在 5 到 3600 秒之间");
  }
  if (!isIntegerInRange(draft.probe.recent_limit, 1, 500)) {
    errors.push("最近事件数量必须在 1 到 500 之间");
  }
  if (!isIntegerInRange(draft.observability.hook_event_max_lines, 1, 5000)) {
    errors.push("Hook 事件行数必须在 1 到 5000 之间");
  }
  if (!isIntegerInRange(draft.observability.hook_cooldown_max_lines, 1, 5000)) {
    errors.push("Hook 静默期行数必须在 1 到 5000 之间");
  }
  if (!isIntegerInRange(draft.observability.log_max_bytes, 1024, 10485760)) {
    errors.push("日志读取上限必须在 1024 到 10485760 字节之间");
  }
  if (!isIntegerInRange(draft.logs_db.retention_days, 1, 3650)) {
    errors.push("Logs DB 保留天数必须在 1 到 3650 天之间");
  }
  if (!isIntegerInRange(draft.logs_db.maintenance_interval_hours, 1, 8760)) {
    errors.push("Logs DB 维护间隔必须在 1 到 8760 小时之间");
  }
  if (!isIntegerInRange(draft.logs_db.codex_exit_grace_seconds, 0, 3600)) {
    errors.push("Codex 退出宽限秒数必须在 0 到 3600 之间");
  }
  if (!isIntegerInRange(draft.logs_db.codex_exit_max_wait_seconds, 1, 7200)) {
    errors.push("Codex 退出最长等待必须在 1 到 7200 秒之间");
  }
  if (!isIntegerInRange(draft.logs_db.delete_chunk_rows, 1, 100000)) {
    errors.push("删除分块行数必须在 1 到 100000 之间");
  }
  if (!isIntegerInRange(draft.logs_db.max_delete_rows_per_run, 1, 1000000)) {
    errors.push("单次最大删除行数必须在 1 到 1000000 之间");
  }
  if (!isIntegerInRange(draft.logs_db.busy_timeout_ms, 100, 60000)) {
    errors.push("SQLite busy timeout 必须在 100 到 60000 毫秒之间");
  }
  if (!isIntegerInRange(draft.logs_db.compact_interval_hours, 1, 8760)) {
    errors.push("Compact 间隔必须在 1 到 8760 小时之间");
  }
  if (!isIntegerInRange(draft.logs_db.compact_min_freelist_ratio_percent, 0, 100)) {
    errors.push("Freelist 比例必须在 0 到 100 之间");
  }
  if (!draft.notifications.server_url.trim()) errors.push("Bark 服务 URL 不能为空");
  return errors;
}

export function createProbeActionState(action: ProbeJobAction): ProbeActionUiState {
  return {
    action,
    phase: "idle",
    plan: null,
    error: null,
    jobId: null,
    canExecute: false,
    confirmLabel: "确认执行"
  };
}

export function reduceProbeActionState(state: ProbeActionUiState, event: ProbeActionUiEvent): ProbeActionUiState {
  if (event.type === "reset") return createProbeActionState(state.action);
  if (event.type === "planning") {
    return { ...state, phase: "planning", error: null, plan: null, jobId: null, canExecute: false };
  }
  if (event.type === "planned") {
    return {
      ...state,
      phase: "awaiting-confirmation",
      plan: event.plan,
      error: null,
      jobId: null,
      canExecute: Boolean(event.plan.plan_id),
      confirmLabel: "确认执行"
    };
  }
  if (event.type === "executing") {
    return { ...state, phase: "executing", error: null, canExecute: false };
  }
  if (event.type === "executed") {
    return { ...state, phase: "executed", error: null, jobId: event.jobId, canExecute: false };
  }
  return { ...state, phase: "error", error: event.message, canExecute: Boolean(state.plan?.plan_id) };
}

function toBoundedInteger(value: unknown, fallback: number): number | null {
  const numeric = Number(value ?? fallback);
  return Number.isFinite(numeric) ? Math.trunc(numeric) : null;
}

export function probeNumberInputDraftValue(value: string): ProbeNumericDraftValue {
  const trimmed = value.trim();
  if (!trimmed) return "";
  const numeric = Number(trimmed);
  return Number.isFinite(numeric) ? Math.trunc(numeric) : "";
}

function isIntegerInRange(value: ProbeNumericDraftValue, min: number, max: number): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= min && value <= max;
}

function requiredDraftNumber(value: ProbeNumericDraftValue): number {
  if (typeof value !== "number") {
    throw new Error("探针数字设置无效");
  }
  return value;
}

function stringOrEmpty(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function optionalString(value: string): string | null {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}
