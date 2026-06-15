import { describe, expect, test } from "vitest";
import type { ProbeSettings } from "../types";
import {
  buildProbeSettingsDraft,
  buildProbeSettingsPayload,
  PROBE_NAV_LABEL,
  probeNumberInputDraftValue,
  probeSections,
  probeSettingsValidation,
} from "./probeUi";

const settings: ProbeSettings = {
  codex: {
    home: "/root/.codex",
    workspace: "/home/ubuntu/codex-workspace",
    app_server_service: "codex-app-server-root.service",
    app_server_socket: "/root/.codex/app-server-control/app-server-control.sock",
    bridge_enabled: true,
    bridge_transport: "websocket",
    bridge_timeout_seconds: 20,
    host_label: "43.155.235.227"
  },
  probe: {
    enabled: true,
    poll_seconds: 15,
    recent_limit: 50,
    hooks: {
      manage_stop_hook: true,
      reload_app_server_after_install: true
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
    expect(probeSections.map((section) => section.label)).toEqual([
      "总览",
      "Bark",
      "运行设置",
      "Codex 日志库维护"
    ]);
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
        app_server_service: "codex-app-server-root.service",
        app_server_socket: "/root/.codex/app-server-control/app-server-control.sock",
        bridge_enabled: true,
        bridge_transport: "websocket",
        bridge_timeout_seconds: 20,
        host_label: "43.155.235.227"
      },
      probe: {
        enabled: true,
        poll_seconds: 15,
        recent_limit: 50,
        hooks: {
          manage_stop_hook: true,
          reload_app_server_after_install: true
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
        app_server_service: "codex-app-server-root.service",
        host_label: "cloud"
      },
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

  test("uses configured app-server socket for the draft instead of the resolved socket", () => {
    const autoResolvedSettings: ProbeSettings = {
      ...settings,
      codex: {
        ...settings.codex,
        app_server_socket: "/root/.codex/app-server-control/app-server-control.sock",
        configured_app_server_socket: null,
        resolved_app_server_socket: "/root/.codex/app-server-control/app-server-control.sock",
        app_server_socket_source: "resolved_codex_home"
      }
    };
    expect(buildProbeSettingsDraft(autoResolvedSettings).codex.app_server_socket).toBe("");

    const configuredSettings: ProbeSettings = {
      ...settings,
      codex: {
        ...settings.codex,
        app_server_socket: "/root/.codex/app-server-control/app-server-control.sock",
        configured_app_server_socket: "/run/codex/custom.sock",
        resolved_app_server_socket: "/root/.codex/app-server-control/app-server-control.sock",
        app_server_socket_source: "configured"
      }
    };
    expect(buildProbeSettingsDraft(configuredSettings).codex.app_server_socket).toBe("/run/codex/custom.sock");
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
