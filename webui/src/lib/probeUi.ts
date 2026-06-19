import type { ProbeEvent, ProbeSettings } from "../types";

export const PROBE_NAV_LABEL = "探针";

export const probeSections = [
  { id: "overview", label: "总览" },
  { id: "reply-needed", label: "需回复" },
  { id: "recoverable", label: "异常/可恢复" },
  { id: "running", label: "运行中" },
  { id: "hook", label: "Hook" },
  { id: "bark", label: "Bark" },
  { id: "logs-db", label: "日志库" },
  { id: "events", label: "最近事件" },
  { id: "settings", label: "设置" }
] as const;

export type ProbeSectionId = typeof probeSections[number]["id"];

export type ProbeSettingsDraft = {
  codex: {
    home: string;
    workspace: string;
    host_label: string;
  };
  probe: {
    enabled: boolean;
    poll_seconds: ProbeNumericDraftValue;
    recent_limit: ProbeNumericDraftValue;
  };
  hooks: {
    manage_stop_hook: boolean;
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

export function buildProbeSettingsDraft(settings: ProbeSettings): ProbeSettingsDraft {
  const codex = settings.codex ?? {};
  const probe = settings.probe ?? {};
  const notifications = settings.notifications ?? {};
  const logsDb = settings.logs_db ?? {};
  return {
    codex: {
      home: configuredCodexHomeDraftValue(settings),
      workspace: stringOrEmpty(codex.workspace),
      host_label: stringOrEmpty(codex.host_label)
    },
    probe: {
      enabled: Boolean(probe.enabled),
      poll_seconds: toBoundedInteger(probe.poll_seconds, 15) ?? 15,
      recent_limit: toBoundedInteger(probe.recent_limit, 50) ?? 50
    },
    hooks: {
      manage_stop_hook: probe.hooks?.manage_stop_hook !== false
    },
    notifications: {
      enabled: Boolean(notifications.enabled || notifications.device_key_configured),
      device_key: "",
      device_key_configured: Boolean(notifications.device_key_configured),
      server_url: notifications.server_url ?? "https://api.day.app",
      sound: stringOrEmpty(notifications.sound),
      group: typeof notifications.group === "string" ? notifications.group : "NexusHub",
      url: stringOrEmpty(notifications.url),
      notify_completion: notifications.notify_completion !== false,
      notify_reply_needed: notifications.notify_reply_needed !== false,
      notify_recoverable: notifications.notify_recoverable !== false
    },
    observability: {
      hook_event_max_lines: toBoundedInteger(probe.observability?.hook_event_max_lines, PROBE_MAC_DEFAULTS.observability.hook_event_max_lines) ?? PROBE_MAC_DEFAULTS.observability.hook_event_max_lines,
      hook_cooldown_max_lines: toBoundedInteger(probe.observability?.hook_cooldown_max_lines, PROBE_MAC_DEFAULTS.observability.hook_cooldown_max_lines) ?? PROBE_MAC_DEFAULTS.observability.hook_cooldown_max_lines,
      log_max_bytes: toBoundedInteger(probe.observability?.log_max_bytes, PROBE_MAC_DEFAULTS.observability.log_max_bytes) ?? PROBE_MAC_DEFAULTS.observability.log_max_bytes
    },
    logs_db: {
      enabled: Boolean(logsDb.enabled),
      retention_days: toBoundedInteger(logsDb.retention_days, PROBE_MAC_DEFAULTS.logs_db.retention_days) ?? PROBE_MAC_DEFAULTS.logs_db.retention_days,
      maintenance_interval_hours: toBoundedInteger(logsDb.maintenance_interval_hours, PROBE_MAC_DEFAULTS.logs_db.maintenance_interval_hours) ?? PROBE_MAC_DEFAULTS.logs_db.maintenance_interval_hours,
      maintain_on_codex_exit: logsDb.maintain_on_codex_exit !== false,
      codex_exit_grace_seconds: toBoundedInteger(logsDb.codex_exit_grace_seconds, PROBE_MAC_DEFAULTS.logs_db.codex_exit_grace_seconds) ?? PROBE_MAC_DEFAULTS.logs_db.codex_exit_grace_seconds,
      codex_exit_max_wait_seconds: toBoundedInteger(logsDb.codex_exit_max_wait_seconds, PROBE_MAC_DEFAULTS.logs_db.codex_exit_max_wait_seconds) ?? PROBE_MAC_DEFAULTS.logs_db.codex_exit_max_wait_seconds,
      delete_chunk_rows: toBoundedInteger(logsDb.delete_chunk_rows, PROBE_MAC_DEFAULTS.logs_db.delete_chunk_rows) ?? PROBE_MAC_DEFAULTS.logs_db.delete_chunk_rows,
      max_delete_rows_per_run: toBoundedInteger(logsDb.max_delete_rows_per_run, PROBE_MAC_DEFAULTS.logs_db.max_delete_rows_per_run) ?? PROBE_MAC_DEFAULTS.logs_db.max_delete_rows_per_run,
      busy_timeout_ms: toBoundedInteger(logsDb.busy_timeout_ms, PROBE_MAC_DEFAULTS.logs_db.busy_timeout_ms) ?? PROBE_MAC_DEFAULTS.logs_db.busy_timeout_ms,
      auto_compact_when_codex_closed: logsDb.auto_compact_when_codex_closed !== false,
      compact_interval_hours: toBoundedInteger(logsDb.compact_interval_hours, PROBE_MAC_DEFAULTS.logs_db.compact_interval_hours) ?? PROBE_MAC_DEFAULTS.logs_db.compact_interval_hours,
      compact_min_freelist_mb: toBoundedInteger(logsDb.compact_min_freelist_mb, PROBE_MAC_DEFAULTS.logs_db.compact_min_freelist_mb) ?? PROBE_MAC_DEFAULTS.logs_db.compact_min_freelist_mb,
      compact_min_freelist_ratio_percent: toBoundedInteger(logsDb.compact_min_freelist_ratio_percent, PROBE_MAC_DEFAULTS.logs_db.compact_min_freelist_ratio_percent) ?? PROBE_MAC_DEFAULTS.logs_db.compact_min_freelist_ratio_percent,
      minimum_free_space_mb: toBoundedInteger(logsDb.minimum_free_space_mb, PROBE_MAC_DEFAULTS.logs_db.minimum_free_space_mb) ?? PROBE_MAC_DEFAULTS.logs_db.minimum_free_space_mb
    }
  };
}

export type ProbeSettingsPayload = {
  codex: ProbeSettings["codex"];
  probe: Pick<ProbeSettings["probe"], "enabled" | "poll_seconds" | "recent_limit" | "hooks" | "notifications" | "observability" | "logs_db">;
  notifications?: Pick<ProbeSettings["notifications"], "device_key">;
};

export function buildProbeSettingsPayload(
  draft: ProbeSettingsDraft,
  _current?: ProbeSettings | null,
  submittedDeviceKey?: string | null
): ProbeSettingsPayload {
  const errors = probeSettingsValidation(draft);
  if (errors.length) {
    throw new Error(errors[0]);
  }
  const deviceKey = (submittedDeviceKey ?? draft.notifications.device_key).trim();
  const notifications: NonNullable<ProbeSettingsPayload["probe"]["notifications"]> & { device_key?: string } = {
    enabled: draft.notifications.enabled || Boolean(deviceKey) || draft.notifications.device_key_configured,
    server_url: draft.notifications.server_url.trim(),
    sound: optionalString(draft.notifications.sound),
    group: draft.notifications.group.trim(),
    url: optionalString(draft.notifications.url),
    notify_completion: draft.notifications.notify_completion,
    notify_reply_needed: draft.notifications.notify_reply_needed,
    notify_recoverable: draft.notifications.notify_recoverable
  };
  if (deviceKey) {
    notifications.device_key = deviceKey;
  }

  const payload: ProbeSettingsPayload = {
    codex: {
      home: codexHomePatchValue(draft.codex.home),
      workspace: draft.codex.workspace.trim() || null,
      host_label: draft.codex.host_label.trim()
    },
    probe: {
      enabled: draft.probe.enabled,
      poll_seconds: requiredDraftNumber(draft.probe.poll_seconds),
      recent_limit: requiredDraftNumber(draft.probe.recent_limit),
      hooks: {
        manage_stop_hook: draft.hooks.manage_stop_hook
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
  if (deviceKey) {
    payload.notifications = { device_key: deviceKey };
  }
  return payload;
}

export function probeSettingsValidation(draft: ProbeSettingsDraft): string[] {
  const errors: string[] = [];
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

export type ProbeEventDisplay = {
  title: string;
  summary: string;
  bark: string;
  dedupe: string;
  source: string;
  time: string;
};

export type ProbeEventCardTone = "success" | "warning" | "danger" | "muted";

export type ProbeEventCard = {
  title: string;
  headline: string;
  summary: string;
  reason: string;
  source: string;
  time: string;
  bark: { label: string; tone: ProbeEventCardTone };
  dedupe: { label: string; tone: ProbeEventCardTone };
  details: Array<{ label: string; value: string }>;
};

export function probeEventDisplay(event: ProbeEvent): ProbeEventDisplay {
  const card = probeEventCard(event);
  return {
    title: card.title,
    summary: card.summary,
    bark: card.bark.label,
    dedupe: card.dedupe.label,
    source: card.source,
    time: card.time
  };
}

export function probeEventCard(event: ProbeEvent): ProbeEventCard {
  const payload = probeEventPayload(event);
  const eventType = cleanProbeEventText(stringFromRecord(payload, "event_type"));
  const source = cleanProbeEventText(stringFromRecord(payload, "source")) || cleanProbeEventText(event.source) || "未知来源";
  const threadId = cleanProbeEventText(stringFromRecord(payload, "thread_id")) || cleanProbeEventText(event.thread_id);
  const turnId = cleanProbeEventText(stringFromRecord(payload, "turn_id"));
  const bodyLength = numberFromRecord(payload, "body_length");
  const bodySha = cleanProbeEventText(stringFromRecord(payload, "body_sha256"));
  const bodySource = cleanProbeEventText(stringFromRecord(payload, "body_source"));
  const bodyTruncated = payload.body_truncated === true;
  const bark = recordFromRecord(payload, "bark");
  const reason = cleanProbeEventText(stringFromRecord(payload, "reason_label")) || cleanProbeEventText(stringFromRecord(payload, "reason"));
  const summary = structuredProbeEventSummary(event, payload);
  const details: Array<{ label: string; value: string }> = [];

  if (threadId) details.push({ label: "线程", value: threadId });
  if (turnId) details.push({ label: "Turn", value: turnId });
  if (bodyLength !== null || bodySha) {
    details.push({
      label: "Body",
      value: [
        bodyLength !== null ? `${bodyLength} bytes` : "",
        bodySha ? `sha256 ${bodySha}` : "",
        bodyTruncated ? "已截断" : "",
        bodySource
      ].filter(Boolean).join(" · ")
    });
  }
  const barkDetail = probeEventBarkDetail(bark);
  if (barkDetail) details.push({ label: "Bark", value: barkDetail });
  details.push({ label: "来源", value: source });

  return {
    title: probeEventKindLabel(eventType || event.kind),
    headline: cleanProbeEventText(stringFromRecord(payload, "thread_title"))
      || cleanProbeEventText(event.title)
      || eventType
      || cleanProbeEventText(event.kind)
      || "Probe 事件",
    summary,
    reason,
    source,
    time: cleanProbeEventText(stringFromRecord(payload, "beijing_time")) || probeEventTimeLabel(event.created_at),
    bark: probeEventBarkBadge(event),
    dedupe: probeEventDedupeBadge(event),
    details
  };
}

function probeEventBarkDetail(bark: Record<string, unknown> | null): string {
  if (!bark) return "";
  const title = cleanProbeEventText(stringFromRecord(bark, "title"));
  const chunkCount = numberFromRecord(bark, "chunk_count");
  const requestCount = numberFromRecord(bark, "request_count");
  return [
    title,
    chunkCount !== null && chunkCount > 1 ? `${chunkCount} 段` : "",
    requestCount !== null && requestCount > 1 ? `${requestCount} 请求` : ""
  ].filter(Boolean).join(" · ");
}

export function probeEventReadableSummary(event: ProbeEvent): string {
  const payload = probeEventPayload(event);
  const structured = structuredProbeEventSummary(event, payload);
  if (structured) return structured;
  const message = cleanProbeEventText(event.message);
  if (message) return message;
  const candidates = [
    stringFromRecord(payload, "summary"),
    stringFromRecord(payload, "status"),
    stringFromRecord(payload, "reason"),
    stringFromRecord(payload, "kind"),
    event.title
  ];
  const kind = cleanProbeEventText(event.kind).toLowerCase();
  const kindLabel = probeEventKindLabel(event.kind).toLowerCase();
  const summary = candidates
    .map(cleanProbeEventText)
    .find((candidate) => {
      if (!candidate) return false;
      const normalized = candidate.toLowerCase();
      return normalized !== kind && normalized !== kindLabel;
    });
  if (summary) return summary;
  if (event.thread_id) return `线程 ${event.thread_id}`;
  return "Probe 事件已记录";
}

export function probeEventBarkStatus(event: ProbeEvent): string {
  return probeEventBarkBadge(event).label;
}

export function probeEventBarkBadge(event: ProbeEvent): { label: string; tone: ProbeEventCardTone } {
  const bark = recordFromRecord(probeEventPayload(event), "bark");
  if (!bark) return { label: "Bark 未记录", tone: "muted" };
  if (bark.sent === true) {
    const status = typeof bark.http_status === "number" ? ` HTTP ${bark.http_status}` : "";
    return { label: `Bark 已发送${status}`, tone: "success" };
  }
  const reason = cleanProbeEventText(stringFromRecord(bark, "reason"));
  const dedupeHit = bark.dedupe_hit === true ? " · 去重命中" : "";
  if (bark.skipped === true) return { label: `Bark 跳过${reason ? `: ${reason}` : ""}${dedupeHit}`, tone: "warning" };
  return { label: `Bark 未发送${reason ? `: ${reason}` : ""}${dedupeHit}`, tone: reason ? "warning" : "muted" };
}

export function probeEventDedupeStatus(event: ProbeEvent): string {
  return probeEventDedupeBadge(event).label;
}

export function probeEventDedupeBadge(event: ProbeEvent): { label: string; tone: ProbeEventCardTone } {
  const payload = probeEventPayload(event);
  const dedupe = recordFromRecord(payload, "dedupe");
  if (dedupe?.duplicate === true) return { label: "重复事件", tone: "warning" };
  if (dedupe?.claimed === true) return { label: "已认领", tone: "success" };
  const dedupeStatus = cleanProbeEventText(stringFromRecord(dedupe ?? {}, "status"));
  if (dedupeStatus) return { label: dedupeStatus, tone: dedupeStatus.toLowerCase().includes("duplicate") ? "warning" : "muted" };
  if (payload.duplicate === true) return { label: "重复事件", tone: "warning" };
  const outcome = recordFromRecord(payload, "probe_event") ?? recordFromRecord(payload, "outcome");
  if (outcome?.duplicate === true) return { label: "重复事件", tone: "warning" };
  if (outcome?.recorded === true) return { label: "已记录", tone: "success" };
  return event.dedupe_key?.trim() ? { label: "去重键已记录", tone: "muted" } : { label: "无去重键", tone: "muted" };
}

export function probeEventKindLabel(kind?: string | null): string {
  const normalized = kind?.trim();
  const labels: Record<string, string> = {
    "hook-stop": "Stop Hook",
    hook_stop: "Stop Hook",
    completion: "完成",
    "reply-needed": "需回复",
    reply_needed: "需回复",
    recoverable: "异常/可恢复",
    running: "运行中",
    "bark-test": "Bark 测试"
  };
  return normalized ? labels[normalized] ?? normalized : "Probe 事件";
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

function configuredCodexHomeDraftValue(settings: ProbeSettings): string {
  const codex = (settings.codex ?? {}) as ProbeSettings["codex"] & Record<string, unknown>;
  if (Object.prototype.hasOwnProperty.call(codex, "configured_codex_home")) {
    return stringOrEmpty(codex.configured_codex_home);
  }
  return stringOrEmpty(codex.home);
}

function codexHomePatchValue(value: string): string | null {
  const trimmed = value.trim();
  return !trimmed || trimmed.toLowerCase() === "auto" ? null : trimmed;
}

function optionalString(value: string): string | null {
  const trimmed = value.trim();
  return trimmed ? trimmed : null;
}

function probeEventPayload(event: ProbeEvent): Record<string, unknown> {
  return event.payload && typeof event.payload === "object" ? event.payload : {};
}

function recordFromRecord(record: Record<string, unknown>, key: string): Record<string, unknown> | null {
  const value = record[key];
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;
}

function stringFromRecord(record: Record<string, unknown>, key: string): string | null {
  const value = record[key];
  return typeof value === "string" ? value : null;
}

function numberFromRecord(record: Record<string, unknown>, key: string): number | null {
  const value = record[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function cleanProbeEventText(value?: string | null): string {
  return value?.replace(/\s+/g, " ").trim() ?? "";
}

function compactProbeEventSummary(value: string): string {
  return value.length > 240 ? `${value.slice(0, 240).trimEnd()}...` : value;
}

function structuredProbeEventSummary(event: ProbeEvent, payload: Record<string, unknown>): string {
  const candidates = [
    stringFromRecord(payload, "body_summary"),
    stringFromRecord(payload, "reason_label"),
    stringFromRecord(payload, "summary"),
    stringFromRecord(payload, "status"),
    stringFromRecord(payload, "reason"),
    event.message,
    event.title
  ];
  const kind = cleanProbeEventText(stringFromRecord(payload, "event_type") || event.kind).toLowerCase();
  const kindLabel = probeEventKindLabel(kind).toLowerCase();
  const summary = candidates
    .map(cleanProbeEventText)
    .find((candidate) => {
      if (!candidate) return false;
      const normalized = candidate.toLowerCase();
      return normalized !== kind && normalized !== kindLabel;
    });
  if (summary) return compactProbeEventSummary(summary);
  return event.thread_id ? `线程 ${event.thread_id}` : "Probe 事件已记录";
}

function probeEventTimeLabel(value: string | number): string {
  if (typeof value === "number") {
    const millis = value > 10_000_000_000 ? value : value * 1000;
    const date = new Date(millis);
    return Number.isNaN(date.getTime()) ? String(value) : date.toISOString();
  }
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toISOString();
}
