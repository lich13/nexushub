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
    host_label: "192.0.2.10"
  } as ProbeSettings["codex"],
  probe: {
    enabled: true,
    poll_seconds: 15,
    recent_limit: 50,
    hooks: {
      manage_stop_hook: true
    },
    error_monitor: {
      enabled: true,
    },
    notifications: {
      enabled: true,
      server_url: "https://api.day.app",
      sound: "bell",
      group: "NexusHub",
      url: "https://panel.example.com/nexushub/",
      notify_completion: true,
      notify_reply_needed: true,
      notify_recoverable: true
    },
    observability: {
        event_retention_days: 2,
      hook_event_max_lines: 120,
      hook_cooldown_max_lines: 80,
      log_max_bytes: 262144
    },
  },
  notifications: {
    enabled: true,
    device_key_configured: true,
    server_url: "https://api.day.app",
    sound: "bell",
    group: "NexusHub",
    url: "https://panel.example.com/nexushub/",
    notify_completion: true,
    notify_reply_needed: true,
    notify_recoverable: true
  },
};

describe("Probe UI helpers", () => {
  test("uses Chinese probe labels and only slim first-screen sections", () => {
    expect(PROBE_NAV_LABEL).toBe("Probe");
    expect(probeSections.map((section) => section.id)).toEqual(["events", "settings"]);
    expect(probeSections.map((section) => section.label)).toEqual(["最近事件", "通知配置"]);
  });

  test("builds readable event display details without exposing secret payload values", () => {
    const event: ProbeEvent = {
      id: "event-a",
      kind: "reply-needed",
      thread_id: "thread-a",
      title: "Codex Reply Needed",
      message: "Need operator input",
      dedupe_key: "reply-needed:thread-a:turn-a",
      source: "nexushub-webd probe passive-scan",
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
    expect(display.source).toBe("nexushub-webd probe passive-scan");
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
      source: "nexushub-webd probe passive-scan",
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
        source: "nexushub-webd probe passive-scan",
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
      source: "nexushub-webd probe passive-scan",
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
      source: "nexushub-webd probe notify-completion",
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

  test("event card summary is compact even when backend keeps a long safe summary", () => {
    const event: ProbeEvent = {
      id: "event-long-summary",
      kind: "completion",
      thread_id: "thread-a",
      source: "nexushub-webd probe notify-completion",
      payload: {
        event_type: "completion",
        body_summary: "长正文片段 ".repeat(80),
        body_length: 4096,
        body_sha256: "abc123"
      },
      created_at: "2026-06-16T00:00:00Z"
    };

    const card = probeEventCard(event);

    expect(card.summary.length).toBeLessThanOrEqual(243);
    expect(card.summary).toContain("...");
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
        host_label: "192.0.2.10"
      },
      probe: {
        enabled: true,
        poll_seconds: 15,
        recent_limit: 50,
        error_monitor: {
          enabled: true,
        },
        hooks: {
          manage_stop_hook: true
        },
        notifications: {
          enabled: true,
          server_url: "https://api.day.app",
          sound: "bell",
          group: "NexusHub",
          url: "https://panel.example.com/nexushub/",
          notify_completion: true,
          notify_reply_needed: true,
          notify_codex: true,
          notify_grok: true,
          notify_grok_completion: true,
          notify_grok_failure: true,
          notify_claude: true,
          notify_claude_completion: true,
          notify_claude_failure: true,
          notify_claude_reply_needed: true,

          notify_recoverable: true
        },
        observability: {
        event_retention_days: 2,
          hook_event_max_lines: 120,
          hook_cooldown_max_lines: 80,
          log_max_bytes: 262144
        },
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
    };

    expect(buildProbeSettingsDraft(minimal)).toMatchObject({
      notifications: { server_url: "https://api.day.app" },
      observability: {
        event_retention_days: 2,
        hook_event_max_lines: 500,
        hook_cooldown_max_lines: 1000,
        log_max_bytes: 5 * 1024 * 1024
      },
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
      host_label: "192.0.2.10"
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

  test("builds a safe draft from partial desktop Probe settings", () => {
    const partialSettings = {
      codex: {
        host_label: "macbook"
      }
    } as ProbeSettings;

    expect(() => buildProbeSettingsDraft(partialSettings)).not.toThrow();
    expect(buildProbeSettingsDraft(partialSettings)).toMatchObject({
      codex: {
        home: "",
        workspace: "",
        host_label: "macbook"
      },
      probe: {
        enabled: false,
        poll_seconds: 15,
        recent_limit: 50,
        error_monitor: {
          enabled: true,
        }
      },
      notifications: {
        enabled: false,
        device_key_configured: false,
        server_url: "https://api.day.app",
        group: "NexusHub"
      },
    });
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
    expect(buildProbeSettingsPayload(draft, unconfiguredSettings).notifications).toMatchObject({
      device_key: "new-device-key"
    });
  });

  test("a configured Device Key does not override an explicit Bark disable", () => {
    const draft = buildProbeSettingsDraft(settings);
    draft.notifications.enabled = false;
    draft.notifications.device_key = "";
    const payload = buildProbeSettingsPayload(draft, settings);
    expect(payload.probe.notifications?.enabled).toBe(false);
    const reloaded = buildProbeSettingsDraft({
      ...settings,
      notifications: { ...settings.notifications, enabled: false, device_key_configured: true }
    });
    expect(reloaded.notifications.enabled).toBe(false);
    expect(reloaded.notifications.device_key_configured).toBe(true);
  });

  test("uses the submitted Bark Device Key when the React draft is stale", () => {
    const unconfiguredSettings = {
      ...settings,
      notifications: { ...settings.notifications, enabled: false, device_key_configured: false }
    };
    const staleDraft = buildProbeSettingsDraft(unconfiguredSettings);
    staleDraft.notifications.device_key = "";

    const payload = buildProbeSettingsPayload(staleDraft, unconfiguredSettings, " submitted-device-key ");

    expect(payload.probe.notifications).toMatchObject({
      enabled: true,
      device_key: "submitted-device-key"
    });
    expect(payload.notifications).toMatchObject({
      device_key: "submitted-device-key"
    });
  });

  test("keeps blank numeric input as a blank draft value until validation blocks save", () => {
    const draft = buildProbeSettingsDraft(settings);
    draft.probe.poll_seconds = probeNumberInputDraftValue("");
    draft.observability.log_max_bytes = probeNumberInputDraftValue("abc");
    draft.observability.event_retention_days = probeNumberInputDraftValue("3");

    expect(draft.probe.poll_seconds).toBe("");
    expect(draft.observability.log_max_bytes).toBe("");
    expect(draft.observability.event_retention_days).toBe(3);
    expect(probeSettingsValidation(draft)).toEqual([
      "轮询间隔必须在 5 到 3600 秒之间",
      "日志读取上限必须在 1024 到 10485760 字节之间"
    ]);
    expect(() => buildProbeSettingsPayload(draft, settings)).toThrow("轮询间隔必须在 5 到 3600 秒之间");
  });

  test("validates numeric settings before save", () => {
    const draft = buildProbeSettingsDraft(settings);
    draft.probe.poll_seconds = 0;
    draft.probe.recent_limit = 501;
    draft.observability.event_retention_days = 0;

    expect(probeSettingsValidation(draft)).toEqual([
      "轮询间隔必须在 5 到 3600 秒之间",
      "最近事件数量必须在 1 到 500 之间",
      "通知事件保留天数必须在 1 到 3650 天之间"
    ]);
  });

});

function gotifySettings(overrides: Partial<NonNullable<ProbeSettings["gotify"]>> = {}): ProbeSettings {
  return {
    codex: { host_label: "fixture-host" },
    probe: {},
    notifications: { enabled: false, server_url: "https://example.invalid/bark/" },
    gotify: {
      enabled: false,
      server_url: "https://example.invalid/gotify/",
      priority: 5,
      token_configured: false,
      ...overrides
    }
  };
}

describe("Gotify settings", () => {
  test("defaults to disabled with priority 5 and never prefills a returned Token", () => {
    const defaults = gotifySettings();
    defaults.gotify = {} as NonNullable<ProbeSettings["gotify"]>;
    expect(buildProbeSettingsDraft(defaults).gotify).toEqual({
      supported: true,
      enabled: false,
      server_url: "",
      priority: 5,
      token: "",
      token_configured: false,
      clear_token: false
    });
    const configured = gotifySettings({ token_configured: true, token: "fixture-returned-token" });
    expect(buildProbeSettingsDraft(configured).gotify).toMatchObject({ token: "", token_configured: true });
  });

  test("an old service without Gotify settings receives no Gotify patch or validation errors", () => {
    const legacy = gotifySettings();
    delete legacy.gotify;
    const draft = buildProbeSettingsDraft(legacy);
    expect(draft.gotify).toMatchObject({ supported: false, enabled: false, priority: 5 });
    draft.gotify.enabled = true;
    draft.gotify.server_url = "http://example.invalid/gotify/";
    draft.gotify.priority = "";
    expect(probeSettingsValidation(draft)).toEqual([]);
    expect(buildProbeSettingsPayload(draft, legacy)).not.toHaveProperty("gotify");
  });

  test.each(["", "   "])("enabling Gotify without a configured or entered Token rejects %j", token => {
    const draft = buildProbeSettingsDraft(gotifySettings({ enabled: true }));
    draft.gotify.token = token;
    expect(probeSettingsValidation(draft)).toEqual(["请先填写 Gotify Application Token"]);
    expect(() => buildProbeSettingsPayload(draft)).toThrow("请先填写 Gotify Application Token");
  });

  test("an entered Token is trimmed without enabling either channel implicitly", () => {
    const draft = buildProbeSettingsDraft(gotifySettings());
    draft.gotify.token = " fixture-application-token ";
    expect(probeSettingsValidation(draft)).toEqual([]);
    const disabled = buildProbeSettingsPayload(draft);
    expect(disabled.gotify).toMatchObject({ enabled: false, token: "fixture-application-token" });
    expect(disabled.probe.notifications?.enabled).toBe(false);
    draft.gotify.enabled = true;
    expect(buildProbeSettingsPayload(draft).gotify?.enabled).toBe(true);
  });

  test.each([
    "",
    "http://example.invalid/gotify/",
    "ftp://example.invalid/gotify/",
    "https://test:fixture-password@example.invalid/gotify/",
    "https://example.invalid/gotify/?token=fixture-token",
    "https://example.invalid/gotify/#fixture",
    "fixture-invalid-url"
  ])("rejects an enabled Gotify address outside the safe URL contract: %j", server_url => {
    const draft = buildProbeSettingsDraft(gotifySettings({ enabled: true, token_configured: true, server_url }));
    expect(probeSettingsValidation(draft)).toEqual(["Gotify 地址必须使用 HTTPS，且不能包含凭据或查询参数"]);
    expect(() => buildProbeSettingsPayload(draft)).toThrow("Gotify 地址必须使用 HTTPS");
  });

  test.each([
    "https://example.invalid/gotify/",
    "https://example.invalid:8443/push/gotify/",
    "http://127.0.0.1:8080/gotify/",
    "http://localhost:8080/gotify/",
    "http://[::1]:8080/gotify/"
  ])("accepts HTTPS and loopback HTTP addresses: %s", server_url => {
    const draft = buildProbeSettingsDraft(gotifySettings({ enabled: true, token_configured: true, server_url }));
    expect(probeSettingsValidation(draft)).toEqual([]);
    expect(buildProbeSettingsPayload(draft).gotify?.server_url).toBe(server_url);
  });

  test("a disabled and unconfigured channel can be saved with no URL or Token", () => {
    const draft = buildProbeSettingsDraft(gotifySettings({ server_url: "" }));
    expect(probeSettingsValidation(draft)).toEqual([]);
    expect(buildProbeSettingsPayload(draft).gotify).toEqual({ enabled: false, server_url: "", priority: 5 });
  });

  test.each([-1, 11, 2.5, Number.NaN, Number.POSITIVE_INFINITY, ""] as const)(
    "rejects priority values outside integer 0 through 10: %j", priority => {
      const draft = buildProbeSettingsDraft(gotifySettings({ enabled: true, token_configured: true }));
      draft.gotify.priority = priority;
      expect(probeSettingsValidation(draft)).toEqual(["Gotify 优先级必须在 0 到 10 之间"]);
      expect(() => buildProbeSettingsPayload(draft)).toThrow("Gotify 优先级必须在 0 到 10 之间");
    }
  );

  test.each([0, 5, 10])("preserves valid priority %i", priority => {
    const draft = buildProbeSettingsDraft(gotifySettings({ priority }));
    expect(probeSettingsValidation(draft)).toEqual([]);
    expect(buildProbeSettingsPayload(draft).gotify?.priority).toBe(priority);
  });

  test("blank Token input keeps a configured Token without resending credential state", () => {
    const current = gotifySettings({ enabled: true, token_configured: true });
    const draft = buildProbeSettingsDraft(current);
    draft.gotify.token = "   ";
    draft.gotify.server_url = " https://example.invalid/gotify/ ";
    expect(probeSettingsValidation(draft)).toEqual([]);
    expect(buildProbeSettingsPayload(draft, current).gotify).toEqual({
      enabled: true, server_url: "https://example.invalid/gotify/", priority: 5
    });
  });

  test("explicit Token removal disables Gotify and takes precedence over an entered replacement", () => {
    const current = gotifySettings({ enabled: true, token_configured: true });
    current.notifications.enabled = true;
    const draft = buildProbeSettingsDraft(current);
    const originalBark = buildProbeSettingsPayload(draft, current).probe.notifications;
    draft.gotify.clear_token = true;
    draft.gotify.token = "fixture-replacement-token";
    const payload = buildProbeSettingsPayload(draft, current);
    expect(payload.gotify).toEqual({
      enabled: false, server_url: "https://example.invalid/gotify/", priority: 5, clear_token: true
    });
    expect(payload.probe.notifications).toEqual(originalBark);
  });

  test.each([
    { bark: false, gotify: false },
    { bark: false, gotify: true },
    { bark: true, gotify: false },
    { bark: true, gotify: true }
  ])("preserves independent channel switches and shared Provider filters: %j", channels => {
    const current = gotifySettings({ token_configured: true });
    const draft = buildProbeSettingsDraft(current);
    const filters = {
      notify_codex: false,
      notify_completion: false,
      notify_reply_needed: true,
      notify_recoverable: false,
      notify_grok: true,
      notify_grok_completion: false,
      notify_grok_failure: true,
      notify_claude: false,
      notify_claude_completion: true,
      notify_claude_failure: false,
      notify_claude_reply_needed: true
    };
    Object.assign(draft.notifications, filters, { enabled: channels.bark });
    draft.gotify.enabled = channels.gotify;
    const payload = buildProbeSettingsPayload(draft, current);
    expect(payload.probe.notifications).toMatchObject({ enabled: channels.bark, ...filters });
    expect(payload.gotify).toEqual({
      enabled: channels.gotify, server_url: "https://example.invalid/gotify/", priority: 5
    });
    for (const key of Object.keys(filters)) expect(payload.gotify).not.toHaveProperty(key);
  });

  test("saving a new Bark key does not enable Gotify or replace its configured Token", () => {
    const draft = buildProbeSettingsDraft(gotifySettings({ token_configured: true }));
    draft.notifications.device_key = "fixture-bark-key";
    const payload = buildProbeSettingsPayload(draft);
    expect(payload.probe.notifications).toMatchObject({ enabled: true, device_key: "fixture-bark-key" });
    expect(payload.gotify).toEqual({ enabled: false, server_url: "https://example.invalid/gotify/", priority: 5 });
  });
});

function notificationEvent(payload: ProbeEvent["payload"]): ProbeEvent {
  return {
    id: "fixture-notification-event",
    kind: "completion",
    source: "fixture-provider-monitor",
    payload,
    created_at: "2026-06-16T00:00:00Z"
  };
}

describe("independent notification event outcomes", () => {
  test("Gotify success stays visible when Bark fails and confirms only server acceptance", () => {
    const card = probeEventCard(notificationEvent({
      bark: { sent: false, reason: "http_error", http_status: 503 },
      gotify: { sent: true, http_status: 200, message_id: 1, token: "fixture-application-token" }
    }));
    expect(card.bark).toEqual({ label: "Bark 未发送: http_error", tone: "warning" });
    expect(card.gotify).toEqual({ label: "Gotify 已送达服务器", tone: "success" });
    expect(JSON.stringify(card)).not.toMatch(/安卓.*(?:已收到|已接收|已送达)|(?:已收到|已接收|已送达).*安卓/);
    expect(JSON.stringify(card)).not.toContain("fixture-application-token");
  });

  test("Bark success stays visible when Gotify fails", () => {
    const card = probeEventCard(notificationEvent({
      bark: { sent: true, http_status: 200 },
      gotify: { sent: false, reason: "http_error", http_status: 503 }
    }));
    expect(card.bark).toEqual({ label: "Bark 已发送 HTTP 200", tone: "success" });
    expect(card.gotify).toEqual({ label: "Gotify 投递失败", tone: "warning" });
  });

  test.each(["delivery_outcome_unknown", "delivery_interrupted_outcome_unknown", "response_invalid"])(
    "does not turn an uncertain Gotify result into success: %s", reason => {
      const card = probeEventCard(notificationEvent({ gotify: { sent: false, reason } }));
      expect(card.gotify).toEqual({ label: "Gotify 投递结果未知", tone: "warning" });
    }
  );

  test("legacy and disabled Gotify events omit the badge while explicit skips stay readable", () => {
    expect(probeEventCard(notificationEvent({ bark: { sent: true } })).gotify).toBeUndefined();
    expect(probeEventCard(notificationEvent({ gotify: { skipped: true, reason: "notifications_disabled" } })).gotify).toBeUndefined();
    expect(probeEventCard(notificationEvent({ gotify: { skipped: true, reason: "dedupe" } })).gotify)
      .toEqual({ label: "Gotify 已跳过", tone: "muted" });
  });

  test("Gotify test events use a channel-specific title", () => {
    const event = notificationEvent({ gotify: { sent: true } });
    event.kind = "gotify-test";
    expect(probeEventCard(event).title).toBe("Gotify 测试");
  });
});
