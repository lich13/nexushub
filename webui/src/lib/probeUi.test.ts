import { describe, expect, test } from "vitest";
import type { ProbeEvent, ProbeSettings } from "../types";
import {
  buildProbeSettingsDraft,
  buildProbeSettingsPayload,
  PROBE_NAV_LABEL,
  probeEventCard,
  probeEventDisplay,
  probeNumberInputDraftValue,
  probeSections,
  probeSettingsValidation,
} from "./probeUi";

const settings: ProbeSettings = {
  codex: {
    home: "/root/.codex",
    workspace: "/home/ubuntu/codex-workspace",
    host_label: "43.155.235.227"
  } as ProbeSettings["codex"],
  probe: {
    enabled: true,
    poll_seconds: 15,
    recent_limit: 50,
    hooks: {
      manage_stop_hook: true
    },
    notifications: {
      enabled: true,
      server_url: "https://api.day.app",
      sound: "bell",
      group: "NexusHub",
      url: "https://661313.xyz/nexushub/",
      notify_completion: true,
      notify_reply_needed: true,
      notify_recoverable: true
    },
    observability: {
      hook_event_max_lines: 120,
      hook_cooldown_max_lines: 80,
      log_max_bytes: 262144
    },
    logs_db: {
      enabled: true,
      retention_days: 14,
      maintenance_interval_hours: 24,
      maintain_on_codex_exit: true,
      codex_exit_grace_seconds: 10,
      codex_exit_max_wait_seconds: 120,
      delete_chunk_rows: 2000,
      max_delete_rows_per_run: 50000,
      busy_timeout_ms: 5000,
      auto_compact_when_codex_closed: true,
      compact_interval_hours: 168,
      compact_min_freelist_mb: 64,
      compact_min_freelist_ratio_percent: 20,
      minimum_free_space_mb: 256
    }
  },
  notifications: {
    enabled: true,
    device_key_configured: true,
    server_url: "https://api.day.app",
    sound: "bell",
    group: "NexusHub",
    url: "https://661313.xyz/nexushub/",
    notify_completion: true,
    notify_reply_needed: true,
    notify_recoverable: true
  },
  logs_db: {
    enabled: true,
    retention_days: 14,
    maintenance_interval_hours: 24,
    maintain_on_codex_exit: true,
    codex_exit_grace_seconds: 10,
    codex_exit_max_wait_seconds: 120,
    delete_chunk_rows: 2000,
    max_delete_rows_per_run: 50000,
    busy_timeout_ms: 5000,
    auto_compact_when_codex_closed: true,
    compact_interval_hours: 168,
    compact_min_freelist_mb: 64,
    compact_min_freelist_ratio_percent: 20,
    minimum_free_space_mb: 256
  }
};

describe("Probe UI helpers", () => {
  test("uses Chinese probe labels and only slim first-screen sections", () => {
    expect(PROBE_NAV_LABEL).toBe("探针");
    expect(probeSections.map((section) => section.id)).toEqual([
      "overview",
      "reply-needed",
      "recoverable",
      "running",
      "hook",
      "bark",
      "logs-db",
      "events",
      "settings"
    ]);
    expect(probeSections.map((section) => section.label)).toEqual([
      "总览",
      "需回复",
      "异常/可恢复",
      "运行中",
      "Hook",
      "Bark",
      "日志库",
      "最近事件",
      "设置"
    ]);
  });

  test("builds readable event display details without exposing secret payload values", () => {
    const event: ProbeEvent = {
      id: "event-a",
      kind: "reply-needed",
      thread_id: "thread-a",
      title: "Codex Reply Needed",
      message: "Need operator input",
      dedupe_key: "reply-needed:thread-a:turn-a",
      source: "nexushubd probe passive-scan",
      payload: {
        bark: {
          sent: false,
          skipped: true,
          reason: "dedupe",
          device_key_configured: true,
          dedupe_key: "reply-needed:thread-a:turn-a"
        },
        duplicate: true,
        session_id: "session-a",
        device_key: "secret-device-key"
      },
      created_at: "2026-06-15T00:00:00Z"
    };

    const display = probeEventDisplay(event);

    expect(display.title).toBe("需回复");
    expect(display.summary).toContain("Need operator input");
    expect(display.bark).toBe("Bark 跳过: dedupe");
    expect(display.dedupe).toBe("重复事件");
    expect(display.source).toBe("nexushubd probe passive-scan");
    expect(display.time).toContain("2026-06-15");
    expect(JSON.stringify(display)).not.toContain("secret-device-key");
  });

  test("event summaries do not repeat raw kind or duplicate title when payload has context", () => {
    const event: ProbeEvent = {
      id: "event-summary",
      kind: "reply-needed",
      thread_id: "thread-a",
      title: "reply-needed",
      message: "",
      dedupe_key: "reply-needed:thread-a:turn-a",
      source: "nexushubd probe passive-scan",
      payload: {
        summary: "Plan Mode 等待用户确认",
        status: "reply-needed"
      },
      created_at: "2026-06-15T00:00:00Z"
    };

    expect(probeEventDisplay(event).summary).toBe("Plan Mode 等待用户确认");
  });

  test("builds rich structured event cards from payload fields before raw message or title", () => {
    const event: ProbeEvent = {
      id: "event-structured",
      kind: "legacy-kind",
      thread_id: "fallback-thread",
      title: "Raw event title",
      message: "Raw event message",
      dedupe_key: "reply-needed:thread-a:turn-a",
      source: "raw event source",
      payload: {
        event_type: "reply-needed",
        thread_title: "Plan Mode 修复",
        thread_id: "thread-a",
        turn_id: "turn-a",
        beijing_time: "2026-06-16 09:30:00 北京时间",
        reason_label: "等待用户确认",
        body_summary: "Plan Mode 等待用户确认",
        body_sha256: "abc123",
        body_length: 324,
        source: "nexushubd probe passive-scan",
        bark: {
          title: "等待回复：Plan Mode 修复",
          sent: true,
          skipped: false,
          http_status: 200,
          dedupe_hit: false,
          chunk_count: 4,
          request_count: 4
        },
        dedupe: {
          claimed: true,
          duplicate: false,
          status: "claimed"
        },
        device_key: "secret-device-key",
        bark_device_key: "secret-bark-key",
        token: "secret-token"
      },
      created_at: "2026-06-16T01:30:00Z"
    };

    const card = probeEventCard(event);

    expect(card).toMatchObject({
      title: "需回复",
      headline: "Plan Mode 修复",
      summary: "Plan Mode 等待用户确认",
      reason: "等待用户确认",
      source: "nexushubd probe passive-scan",
      time: "2026-06-16 09:30:00 北京时间",
      bark: { label: "Bark 已发送 HTTP 200", tone: "success" },
      dedupe: { label: "已认领", tone: "success" }
    });
    expect(card.details).toEqual(expect.arrayContaining([
      { label: "线程", value: "thread-a" },
      { label: "Turn", value: "turn-a" },
      { label: "Body", value: "324 bytes · sha256 abc123" },
      { label: "Bark", value: "等待回复：Plan Mode 修复 · 4 段 · 4 请求" }
    ]));
    expect(card.summary).not.toBe("Raw event message");
    expect(JSON.stringify(card)).not.toContain("secret-device-key");
    expect(JSON.stringify(card)).not.toContain("secret-bark-key");
    expect(JSON.stringify(card)).not.toContain("secret-token");
  });

  test("event cards show safe Bark chunk metadata without rendering stored body fields", () => {
    const event: ProbeEvent = {
      id: "event-bark-body",
      kind: "completion",
      thread_id: "thread-safe",
      source: "nexushubd probe notify-completion",
      payload: {
        event_type: "completion",
        thread_title: "完整反馈测试",
        body_summary: "安全摘要",
        body_sha256: "hash",
        body_length: 12000,
        body_source: "task_complete.last_agent_message",
        body_truncated: true,
        bark: {
          title: "线程正常完成：完整反馈测试",
          body: "完整正文不应出现在卡片",
          sent: true,
          http_status: 200,
          chunk_count: 6,
          request_count: 6
        }
      },
      created_at: "2026-06-16T00:00:00Z"
    };

    const card = probeEventCard(event);

    expect(card.summary).toBe("安全摘要");
    expect(card.details).toEqual(expect.arrayContaining([
      { label: "Body", value: "12000 bytes · sha256 hash · 已截断 · task_complete.last_agent_message" },
      { label: "Bark", value: "线程正常完成：完整反馈测试 · 6 段 · 6 请求" }
    ]));
    expect(JSON.stringify(card)).not.toContain("完整正文不应出现在卡片");
  });

  test("marks skipped Bark and duplicate dedupe outcomes with warning labels", () => {
    const event: ProbeEvent = {
      id: "event-duplicate",
      kind: "hook-stop",
      source: "raw source",
      payload: {
        event_type: "completion",
        body_summary: "任务已完成",
        bark: { skipped: true, reason: "dedupe", dedupe_hit: true },
        dedupe: { claimed: false, duplicate: true, status: "duplicate" }
      },
      created_at: "2026-06-16T00:00:00Z"
    };

    const card = probeEventCard(event);

    expect(card.title).toBe("完成");
    expect(card.bark).toEqual({ label: "Bark 跳过: dedupe · 去重命中", tone: "warning" });
    expect(card.dedupe).toEqual({ label: "重复事件", tone: "warning" });
  });

  test("builds a form draft and leaves configured Bark device_key unchanged when blank", () => {
    const draft = buildProbeSettingsDraft(settings);
    expect(draft.notifications.device_key).toBe("");

    const payload = buildProbeSettingsPayload(draft, settings);
    expect(payload).not.toHaveProperty("notifications");
    expect(payload).not.toHaveProperty("logs_db");
    expect(payload.probe?.notifications).not.toHaveProperty("device_key");
    expect(payload).toEqual({
      codex: {
        home: "/root/.codex",
        workspace: "/home/ubuntu/codex-workspace",
        host_label: "43.155.235.227"
      },
      probe: {
        enabled: true,
        poll_seconds: 15,
        recent_limit: 50,
        hooks: {
          manage_stop_hook: true
        },
        notifications: {
          enabled: true,
          server_url: "https://api.day.app",
          sound: "bell",
          group: "NexusHub",
          url: "https://661313.xyz/nexushub/",
          notify_completion: true,
          notify_reply_needed: true,
          notify_recoverable: true
        },
        observability: {
          hook_event_max_lines: 120,
          hook_cooldown_max_lines: 80,
          log_max_bytes: 262144
        },
        logs_db: {
          enabled: true,
          retention_days: 14,
          maintenance_interval_hours: 24,
          maintain_on_codex_exit: true,
          codex_exit_grace_seconds: 10,
          codex_exit_max_wait_seconds: 120,
          delete_chunk_rows: 2000,
          max_delete_rows_per_run: 50000,
          busy_timeout_ms: 5000,
          auto_compact_when_codex_closed: true,
          compact_interval_hours: 168,
          compact_min_freelist_mb: 64,
          compact_min_freelist_ratio_percent: 20,
          minimum_free_space_mb: 256
        }
      }
    });
  });

  test("uses Mac app-compatible defaults when Probe settings omit optional numbers", () => {
    const minimal: ProbeSettings = {
      codex: {
        home: "/root/.codex",
        host_label: "cloud"
      } as ProbeSettings["codex"],
      probe: {},
      notifications: {},
      logs_db: {}
    };

    expect(buildProbeSettingsDraft(minimal)).toMatchObject({
      notifications: { server_url: "https://api.day.app" },
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
    });
  });

  test("uses configured Codex Home for the draft instead of the resolved default", () => {
    const autoResolvedSettings: ProbeSettings = {
      ...settings,
      codex: {
        ...settings.codex,
        home: "/root/.codex",
        configured_codex_home: null,
        resolved_codex_home: "/root/.codex",
        codex_home_source: "auto"
      }
    };
    expect(buildProbeSettingsDraft(autoResolvedSettings).codex.home).toBe("");

    const configuredSettings: ProbeSettings = {
      ...settings,
      codex: {
        ...settings.codex,
        home: "/root/.codex",
        configured_codex_home: "/srv/codex-home",
        resolved_codex_home: "/root/.codex",
        codex_home_source: "config"
      }
    };
    expect(buildProbeSettingsDraft(configuredSettings).codex.home).toBe("/srv/codex-home");
  });

  test("draft codex fields only expose local state paths and host metadata", () => {
    const draft = buildProbeSettingsDraft(settings);

    expect(draft.codex).toEqual({
      home: "/root/.codex",
      workspace: "/home/ubuntu/codex-workspace",
      host_label: "43.155.235.227"
    });
    expect(draft.codex).not.toHaveProperty("app_server_socket");
    expect(draft.codex).not.toHaveProperty("app_server_service");
    expect(draft.codex).not.toHaveProperty("bridge_enabled");
  });

  test("allows blank or auto Codex Home and serializes it as automatic discovery", () => {
    const blankDraft = buildProbeSettingsDraft(settings);
    blankDraft.codex.home = "   ";
    expect(probeSettingsValidation(blankDraft)).not.toContain("Codex Home 不能为空");
    expect(buildProbeSettingsPayload(blankDraft, settings).codex.home).toBeNull();

    const autoDraft = buildProbeSettingsDraft(settings);
    autoDraft.codex.home = "auto";
    expect(probeSettingsValidation(autoDraft)).not.toContain("Codex Home 不能为空");
    expect(buildProbeSettingsPayload(autoDraft, settings).codex.home).toBeNull();
  });

  test("omits a blank Bark device_key whether or not a key is already configured", () => {
    const configuredDraft = buildProbeSettingsDraft(settings);
    configuredDraft.notifications.device_key = "   ";
    expect(buildProbeSettingsPayload(configuredDraft, settings).probe?.notifications).not.toHaveProperty("device_key");

    const unconfiguredSettings: ProbeSettings = {
      ...settings,
      notifications: { ...settings.notifications, device_key_configured: false }
    };
    const unconfiguredDraft = buildProbeSettingsDraft(unconfiguredSettings);
    unconfiguredDraft.notifications.device_key = "";
    expect(buildProbeSettingsPayload(unconfiguredDraft, unconfiguredSettings).probe?.notifications).not.toHaveProperty("device_key");
  });

  test("enables Bark when saving a new Device Key from the slim card", () => {
    const unconfiguredSettings = {
      ...settings,
      notifications: { ...settings.notifications, enabled: false, device_key_configured: false }
    };
    const draft = buildProbeSettingsDraft(unconfiguredSettings);
    draft.notifications.device_key = "new-device-key";

    expect(buildProbeSettingsPayload(draft, unconfiguredSettings).probe.notifications).toMatchObject({
      enabled: true,
      device_key: "new-device-key"
    });
  });

  test("keeps blank numeric input as a blank draft value until validation blocks save", () => {
    const draft = buildProbeSettingsDraft(settings);
    draft.probe.poll_seconds = probeNumberInputDraftValue("");
    draft.logs_db.busy_timeout_ms = probeNumberInputDraftValue("abc");
    draft.logs_db.retention_days = probeNumberInputDraftValue("3");

    expect(draft.probe.poll_seconds).toBe("");
    expect(draft.logs_db.busy_timeout_ms).toBe("");
    expect(draft.logs_db.retention_days).toBe(3);
    expect(probeSettingsValidation(draft)).toEqual([
      "轮询间隔必须在 5 到 3600 秒之间",
      "SQLite busy timeout 必须在 100 到 60000 毫秒之间"
    ]);
    expect(() => buildProbeSettingsPayload(draft, settings)).toThrow("轮询间隔必须在 5 到 3600 秒之间");
  });

  test("validates numeric settings before save", () => {
    const draft = buildProbeSettingsDraft(settings);
    draft.probe.poll_seconds = 0;
    draft.probe.recent_limit = 501;
    draft.logs_db.retention_days = 0;

    expect(probeSettingsValidation(draft)).toEqual([
      "轮询间隔必须在 5 到 3600 秒之间",
      "最近事件数量必须在 1 到 500 之间",
      "Logs DB 保留天数必须在 1 到 3650 天之间"
    ]);
  });

});
