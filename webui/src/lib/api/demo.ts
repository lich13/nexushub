import type {
  CodexConfig,
  CodexGoal,
  PlatformOverview,
  ProbeSettings,
  ProbeStatus,
  SecuritySettings,
  SessionUser,
  SystemStatus,
  ThreadSummary,
  UpdateStatus
} from "../../types";
import {
  buildDemoPlatformOverview,
  buildDemoSecurity,
  buildDemoSystemStatus,
  type DemoFixtureKey
} from "../domain/demoCore";

type DemoRuntimeGlobal = typeof globalThis & {
  __NEXUSHUB_DESKTOP_RUNTIME__?: boolean;
  __TAURI_INTERNALS__?: unknown;
};

export function currentDemoFixtureKey(): DemoFixtureKey {
  const target = globalThis as DemoRuntimeGlobal;
  return target.__NEXUSHUB_DESKTOP_RUNTIME__ || target.__TAURI_INTERNALS__
    ? "macos-tauri"
    : "linux-web";
}

export function demoSessionUser(username = "admin"): SessionUser {
  return currentDemoFixtureKey() === "macos-tauri"
    ? {
      id: "desktop",
      username: "desktop",
      csrf_token: null,
      session_id: "desktop"
    }
    : {
      id: "dev",
      username,
      csrf_token: "dev-csrf"
    };
}

export function demoPlatformOverview(fixture: DemoFixtureKey = currentDemoFixtureKey()): PlatformOverview {
  return buildDemoPlatformOverview(fixture);
}

export function demoSystemStatus(fixture: DemoFixtureKey = currentDemoFixtureKey()): SystemStatus {
  return buildDemoSystemStatus(fixture);
}

export function demoSecurity(fixture: DemoFixtureKey = currentDemoFixtureKey()): SecuritySettings {
  return buildDemoSecurity(fixture);
}

export function demoUpdateStatus(fixture: DemoFixtureKey = currentDemoFixtureKey()): UpdateStatus {
  if (fixture === "macos-tauri") {
    return {
      current_version: "0.1.100",
      latest_version: "v0.1.103",
      update_available: true,
      channel: "stable",
      method: "macos_tauri_updater",
      state: "idle",
      failure_category: null,
      recommended_action: "Confirm install in the Tauri updater after signature verification.",
      capabilities: ["check", "confirm_install", "job_history", "signature_verification", "restart_after_install"]
    };
  }
  return {
    current_version: "0.1.100",
    latest_version: "v0.1.103",
    update_available: true,
    channel: "stable",
    method: "linux_systemd_job",
    state: "idle",
    failure_category: null,
    recommended_action: "/usr/local/bin/nexushub-update --repo lich13/nexushub --version latest",
    capabilities: ["check", "confirm_install", "job_history", "sha256_verification", "systemd_health_check", "rollback", "prune_backups"]
  };
}

export function demoCodexConfig(fixture: DemoFixtureKey = currentDemoFixtureKey()): CodexConfig {
  return {
    model: "gpt-5.5",
    service_tier: null,
    reasoning_effort: "xhigh",
    cwd: fixture === "macos-tauri" ? null : "/home/ubuntu/codex-workspace",
    permission_profile: "danger-full-access",
    approval_policy: "never",
    sandbox_mode: "danger-full-access",
    network_access: true,
    collaboration_mode: null
  };
}

export function demoProbeStatus(): ProbeStatus {
  const platform = demoPlatformOverview();
  const system = demoSystemStatus();
  return {
    label: "Probe",
    enabled: true,
    available: true,
    platform: platform.kind,
    service_kind: platform.service_kind,
    service_name: platform.service_name,
    flavor: "builtin",
    hook_status: "managed",
    bark_status: "not_configured",
    logs_db_status: "maintenance_ready",
    recent_event_count: 1,
    running_count: 1,
    reply_needed_count: 1,
    recoverable_count: 0,
    running_threads: [
      { id: "019e8c1f-demo", title: "活动库审阅链路", status: "Running", message_count: 18, latest_message: "正在逐项审计脚本输出。" }
    ],
    reply_needed_threads: [
      { id: "019e95a0-demo", title: "Plan Mode 修复", status: "ReplyNeeded", message_count: 7, latest_message: "等待确认" }
    ],
    recoverable_threads: [],
    lifecycle_status: "ok",
    doctor_status: "ok",
    runtime_version: "demo",
    config_path: platform.config_file,
    codex_home: system.codex_home,
    configured_codex_home: system.configured_codex_home,
    resolved_codex_home: system.resolved_codex_home,
    codex_home_source: system.codex_home_source,
    logs_db_source: "resolved_codex_home",
    host_label: system.host_label,
    snapshot_age_seconds: 0,
    is_refreshing: false,
    snapshot_status: "cached"
  };
}

export function demoProbeSettings(): ProbeSettings {
  const fixture = currentDemoFixtureKey();
  const platform = demoPlatformOverview(fixture);
  const system = demoSystemStatus(fixture);
  const runtimeProbeSettings = fixture === "macos-tauri"
    ? {
      logsPath: "~/Library/Application Support/NexusHub/logs_2.sqlite",
      workspace: "~/Documents"
    }
    : {
      logsPath: "/root/.codex/logs_2.sqlite",
      workspace: "/home/ubuntu/codex-workspace"
    };
  return {
    codex: {
      home: system.codex_home,
      configured_codex_home: system.configured_codex_home,
      resolved_codex_home: system.resolved_codex_home,
      codex_home_source: system.codex_home_source,
      logs_db_source: "resolved_codex_home",
      discovery_warnings: [],
      workspace: runtimeProbeSettings.workspace,
      host_label: system.host_label
    },
    probe: {
      enabled: true,
      poll_seconds: 15,
      recent_limit: 50
    },
    notifications: {
      enabled: false,
      device_key_configured: false,
      server_url: "https://api.day.app",
      group: "NexusHub"
    },
    logs_db: {
      path: runtimeProbeSettings.logsPath,
      resolved_path: runtimeProbeSettings.logsPath,
      logs_db_source: "resolved_codex_home",
      config_file: platform.config_file,
      enabled: true,
      retention_days: 2,
      maintenance_interval_hours: 6,
      maintain_on_codex_exit: true,
      codex_exit_grace_seconds: 5,
      codex_exit_max_wait_seconds: 1800,
      delete_chunk_rows: 5000,
      max_delete_rows_per_run: 100000,
      busy_timeout_ms: 500,
      auto_compact_when_codex_closed: true,
      compact_interval_hours: 24,
      compact_min_freelist_mb: 256,
      compact_min_freelist_ratio_percent: 20,
      minimum_free_space_mb: 1024
    }
  };
}

export function demoCodexGoal(threadId: string): CodexGoal {
  return {
    available: true,
    enabled: threadId === "019e95a0-demo",
    objective: threadId === "019e95a0-demo" ? "修复 Plan Mode 右栏交互" : null,
    token_budget: threadId === "019e95a0-demo" ? 18000 : null,
    status: threadId === "019e95a0-demo" ? "active" : "idle",
    raw: { source: "demo", thread_id: threadId }
  };
}

export function demoThreads(status: string, q: string): ThreadSummary[] {
  const threads: ThreadSummary[] = [
    { id: "019e8c1f-demo", title: "活动库审阅链路", status: "Running", message_count: 18, latest_message: "正在逐项审计脚本输出。", updated_at: new Date().toISOString() },
    { id: "019e95a0-demo", title: "Plan Mode 修复", status: "ReplyNeeded", message_count: 7, latest_message: "等待确认", updated_at: new Date().toISOString() },
    { id: "019e5281-demo", title: "检查仓库状态", status: "Recent", message_count: 3, latest_message: "仓库状态干净。", updated_at: new Date().toISOString() },
    { id: "019e42aa-demo", title: "旧归档线程", status: "Archived", message_count: 2, latest_message: "已归档。", updated_at: new Date(Date.now() - 86400000).toISOString() }
  ];
  return threads.filter((thread) => (status === "all" || status === threadStatusParam(thread.status)) && (!q || `${thread.title} ${thread.id}`.toLowerCase().includes(q.toLowerCase())));
}

export function threadStatusParam(status: ThreadSummary["status"]): string {
  if (status === "ReplyNeeded") return "reply-needed";
  return status.toLowerCase();
}
