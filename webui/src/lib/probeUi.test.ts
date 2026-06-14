import { describe, expect, test } from "vitest";
import type { ProbeSettings } from "../types";
import {
  buildProbeSettingsDraft,
  buildProbeSettingsPayload,
  createProbeActionState,
  PROBE_NAV_LABEL,
  probeSections,
  probeSettingsValidation,
  reduceProbeActionState
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
  test("uses Chinese probe labels and the five required sections", () => {
    expect(PROBE_NAV_LABEL).toBe("探针");
    expect(probeSections.map((section) => section.label)).toEqual([
      "总览",
      "线程",
      "事件",
      "诊断",
      "设置与迁移"
    ]);
  });

  test("builds a form draft and leaves configured Bark device_key unchanged when blank", () => {
    const draft = buildProbeSettingsDraft(settings);
    expect(draft.notifications.device_key).toBe("");

    const payload = buildProbeSettingsPayload(draft, settings);
    expect(payload.notifications).not.toHaveProperty("device_key");
    expect(payload).toMatchObject({
      probe: {
        poll_seconds: 15,
        hooks: { manage_stop_hook: true },
        observability: { hook_event_max_lines: 120 },
        logs_db: { busy_timeout_ms: 5000 }
      },
      notifications: {
        sound: "bell",
        notify_recoverable: true
      },
      logs_db: { retention_days: 14, compact_min_freelist_ratio_percent: 20 }
    });
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

  test("requires plan then confirm execute for logs-db and Hook actions", () => {
    const initial = createProbeActionState("logs-db-maintain");
    const planned = reduceProbeActionState(initial, {
      type: "planned",
      plan: {
        plan_id: "plan-1",
        kind: "logs-db-maintain",
        title: "Logs DB 维护",
        steps: ["dry-run", "compact"],
        requires_confirmation: true
      }
    });

    expect(planned.phase).toBe("awaiting-confirmation");
    expect(planned.canExecute).toBe(true);
    expect(planned.confirmLabel).toBe("确认执行");

    const executed = reduceProbeActionState(planned, { type: "executed", jobId: "job-1" });
    expect(executed.phase).toBe("executed");
    expect(executed.jobId).toBe("job-1");

    const hookState = reduceProbeActionState(createProbeActionState("hooks-install"), {
      type: "planned",
      plan: {
        plan_id: "hook-plan",
        kind: "hooks-install",
        title: "Hook 安装",
        steps: ["检查", "安装"],
        requires_confirmation: true
      }
    });
    expect(hookState.canExecute).toBe(true);
  });
});
