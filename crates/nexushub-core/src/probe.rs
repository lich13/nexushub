use crate::{
    codex::{resolve_codex_paths, ResolvedCodexPaths, ThreadSummary},
    config::{Config, ProbeLogsDbConfig},
    platform::{PlatformKind, PlatformPaths},
    security::redact_output,
};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use rusqlite::{params, Connection, ErrorCode, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command as StdCommand,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

pub const PROBE_EVENT_DEDUPE_NAMESPACE: &str = "probe_event";
pub const PROBE_EVENT_TTL_SECONDS: i64 = 300;
pub const DEFAULT_LOGS_DB_COMPACT_QUICK_CHECK_TIMEOUT_SECONDS: u64 = 600;
const PROBE_EVENT_ASSISTANT_MESSAGE_MAX_BYTES: usize = 4096;

#[derive(Debug, Clone)]
pub struct ProbeRuntime {
    config: Config,
    paths: PlatformPaths,
}

impl ProbeRuntime {
    pub fn new(config: Config, paths: PlatformPaths) -> Self {
        Self { config, paths }
    }

    pub async fn status(&self) -> Result<ProbeStatus> {
        let resolved = self.resolved_codex_paths();
        let logs_db_status = self.logs_db_status();
        Ok(ProbeStatus {
            label: "Probe".to_string(),
            enabled: self.config.probe.enabled,
            available: true,
            flavor: "builtin".to_string(),
            platform: self.paths.kind,
            service_kind: self.paths.service_kind.clone(),
            service_name: self.paths.service_name.clone(),
            binary_path: None,
            hook_status: if self.config.probe.hooks.manage_stop_hook {
                "managed"
            } else {
                "disabled"
            }
            .to_string(),
            bark_status: if self.config.probe.notifications.enabled {
                "configured"
            } else {
                "not_configured"
            }
            .to_string(),
            hooks_enabled: self.config.probe.hooks.manage_stop_hook,
            hook_stop_enabled: self.config.probe.hooks.manage_stop_hook,
            bark_enabled: self.config.probe.notifications.enabled,
            bark_server_url: self.config.probe.notifications.server_url.clone(),
            bark_notify_completion: self.config.probe.notifications.notify_completion,
            bark_notify_reply_needed: self.config.probe.notifications.notify_reply_needed,
            bark_notify_recoverable: self.config.probe.notifications.notify_recoverable,
            logs_db_status: logs_db_status.status,
            lifecycle_status: self.lifecycle_status_text(),
            doctor_status: self.doctor_status_text(),
            runtime_version: env!("CARGO_PKG_VERSION").to_string(),
            poll_seconds: self.config.probe.poll_seconds,
            recent_event_count: 0,
            running_count: 0,
            reply_needed_count: 0,
            recoverable_count: 0,
            running_threads: Vec::new(),
            reply_needed_threads: Vec::new(),
            recoverable_threads: Vec::new(),
            config_path: self.paths.config_file.clone(),
            codex_home: resolved.home.clone(),
            configured_codex_home: resolved.configured_codex_home.clone(),
            resolved_codex_home: resolved.home.clone(),
            codex_home_source: resolved.codex_home_source.clone(),
            logs_db_source: resolved.logs_db_source.clone(),
            configured_app_server_socket: resolved.configured_app_server_socket.clone(),
            resolved_app_server_socket: resolved.app_server_socket.clone(),
            app_server_socket_source: resolved.app_server_socket_source.clone(),
            discovery_warnings: resolved.discovery_warnings.clone(),
            host_label: self.config.codex.host_label.clone(),
        })
    }

    pub fn diagnostics(&self) -> ProbeDiagnostics {
        let resolved = self.resolved_codex_paths();
        ProbeDiagnostics {
            doctor_status: self.doctor_status_text(),
            lifecycle_status: self.lifecycle_status_text(),
            app_server_service: self.config.codex.app_server_service.clone(),
            app_server_socket: resolved.app_server_socket.clone(),
            configured_app_server_socket: resolved.configured_app_server_socket,
            resolved_app_server_socket: resolved.app_server_socket,
            app_server_socket_source: resolved.app_server_socket_source,
            configured_codex_home: resolved.configured_codex_home,
            resolved_codex_home: resolved.home,
            codex_home_source: resolved.codex_home_source,
            discovery_warnings: resolved.discovery_warnings,
            host_label: self.config.codex.host_label.clone(),
            runtime_version: env!("CARGO_PKG_VERSION").to_string(),
            managed_boundaries: vec![
                "不暴露 root app-server".to_string(),
                "不开放任意 shell".to_string(),
                "不执行自动回复或隐藏桌面控制".to_string(),
            ],
            effective_constants: json!({
                "thread_probe_uses_app_server_read": true,
                "delete_uses_existing_dry_run_confirm_flow": true,
                "legacy_sentinel_cli_runtime": false,
                "hidden_desktop_control": false,
                "auto_reply": false,
                "bark_payload_contains_device_key": false
            }),
            repair_suggestions: self.repair_suggestions(),
        }
    }

    pub fn lifecycle_status(&self) -> ProbeLifecycleStatus {
        ProbeLifecycleStatus {
            status: self.lifecycle_status_text(),
            lifecycle_status: self.lifecycle_status_text(),
            platform: self.paths.kind,
            service_kind: self.paths.service_kind.clone(),
            service_name: self.paths.service_name.clone(),
            enabled: self.config.probe.enabled,
            hooks_enabled: self.config.probe.hooks.manage_stop_hook,
            notifications_enabled: self.config.probe.notifications.enabled,
            logs_db_enabled: self.config.probe.logs_db.enabled,
            poll_seconds: self.config.probe.poll_seconds,
            recent_limit: self.config.probe.recent_limit,
            next_actions: self.lifecycle_next_actions(),
        }
    }

    pub fn hook_status(&self) -> ProbeHookStatus {
        let hook_command = self.hook_command();
        ProbeHookStatus {
            status: if self.config.probe.hooks.manage_stop_hook {
                "managed".to_string()
            } else {
                "disabled".to_string()
            },
            hook_status: if self.config.probe.hooks.manage_stop_hook {
                "managed".to_string()
            } else {
                "disabled".to_string()
            },
            installed: self.config.probe.hooks.manage_stop_hook,
            managed: self.config.probe.hooks.manage_stop_hook,
            hook_command,
            reload_app_server_after_install: self
                .config
                .probe
                .hooks
                .reload_app_server_after_install,
            supported_events: vec!["hook-stop".to_string(), "notify-completion".to_string()],
            dedupe_namespace: PROBE_EVENT_DEDUPE_NAMESPACE.to_string(),
            dedupe_ttl_seconds: PROBE_EVENT_TTL_SECONDS,
        }
    }

    pub fn logs_db_status(&self) -> ProbeLogsDbStatus {
        let resolved = self.resolved_codex_paths();
        ProbeLogsDbStatus::from_resolved_paths(&resolved, &self.config.probe.logs_db, now_ts())
    }

    pub fn maintain_logs_db(&self, dry_run: bool) -> Result<ProbeLogsDbMaintenanceResult> {
        self.maintain_logs_db_with_compaction(dry_run, false)
    }

    pub fn maintain_logs_db_with_compaction(
        &self,
        dry_run: bool,
        compact: bool,
    ) -> Result<ProbeLogsDbMaintenanceResult> {
        self.maintain_logs_db_with_compaction_timeout(
            dry_run,
            compact,
            Duration::from_secs(DEFAULT_LOGS_DB_COMPACT_QUICK_CHECK_TIMEOUT_SECONDS),
        )
    }

    pub fn maintain_logs_db_with_compaction_timeout(
        &self,
        dry_run: bool,
        compact: bool,
        quick_check_timeout: Duration,
    ) -> Result<ProbeLogsDbMaintenanceResult> {
        let resolved = self.resolved_codex_paths();
        maintain_codex_logs_db_with_quick_check_timeout(
            &resolved,
            &self.config.probe.logs_db,
            dry_run,
            compact,
            now_ts(),
            quick_check_timeout,
        )
    }

    pub fn plan_action(&self, kind: ProbeActionPlanKind) -> Result<ProbeActionPlan> {
        let suffix = Uuid::new_v4()
            .simple()
            .to_string()
            .chars()
            .take(12)
            .collect::<String>();
        match kind {
            ProbeActionPlanKind::InstallHooks => Ok(ProbeActionPlan {
                plan_id: format!("probe-hooks-{suffix}"),
                kind: "hooks-install".to_string(),
                title: "安装探针 Hook".to_string(),
                summary: "将 Codex Stop Hook 指向 NexusHub 内建 Probe 子命令".to_string(),
                steps: vec![
                    format!(
                        "备份 {} 后写入 Stop Hook",
                        self.resolved_codex_paths()
                            .home
                            .join("hooks.json")
                            .display()
                    ),
                    format!("Stop Hook 命令包含 `{}`", self.hook_command()),
                    if self.config.probe.hooks.reload_app_server_after_install {
                        format!("重载 {}", self.config.codex.app_server_service)
                    } else {
                        "不重载 app-server".to_string()
                    },
                ],
                payload: json!({
                    "codex_home": self.resolved_codex_paths().home,
                    "configured_codex_home": self.resolved_codex_paths().configured_codex_home,
                    "codex_home_source": self.resolved_codex_paths().codex_home_source,
                    "app_server_service": self.config.codex.app_server_service,
                    "hook_command": self.hook_command(),
                    "reload_app_server_after_install": self.config.probe.hooks.reload_app_server_after_install,
                }),
                requires_confirmation: true,
                command: "nexushubd probe hooks-install".to_string(),
            }),
            ProbeActionPlanKind::LogsDbMaintain => Ok(ProbeActionPlan {
                plan_id: format!("probe-logs-db-{suffix}"),
                kind: "logs-db-maintain".to_string(),
                title: "探针日志库维护".to_string(),
                summary: "按保留期预览日志库维护，并在确认后执行固定维护动作".to_string(),
                steps: vec![
                    format!("保留最近 {} 天", self.config.probe.logs_db.retention_days),
                    format!(
                        "单次最多删除 {} 行",
                        self.config.probe.logs_db.max_delete_rows_per_run
                    ),
                    if self.config.probe.logs_db.enabled {
                        "按 deletion 计划分批清理过期 Probe 事件".to_string()
                    } else {
                        "logs-db 已禁用，跳过删除和 vacuum".to_string()
                    },
                    "仅执行 NexusHub 内建维护逻辑，不调用旧 Sentinel CLI".to_string(),
                ],
                payload: json!({
                    "enabled": self.config.probe.logs_db.enabled,
                    "retention_days": self.config.probe.logs_db.retention_days,
                    "delete_chunk_rows": self.config.probe.logs_db.delete_chunk_rows,
                    "max_delete_rows_per_run": self.config.probe.logs_db.max_delete_rows_per_run,
                    "deletion": self.logs_db_deletion_plan(),
                    "vacuum": self.logs_db_vacuum_plan(),
                    "vacuum_candidate": self.config.probe.logs_db.enabled
                        && self.config.probe.logs_db.auto_compact_when_codex_closed,
                    "skip_reason": self.logs_db_skip_reason(),
                    "would_call_legacy_sentinel_cli": false,
                }),
                requires_confirmation: true,
                command: "nexushubd probe logs-db-maintain".to_string(),
            }),
        }
    }

    pub fn bark_test_plan(&self, device_key_configured: bool) -> ProbeActionPlan {
        let suffix = Uuid::new_v4()
            .simple()
            .to_string()
            .chars()
            .take(12)
            .collect::<String>();
        ProbeActionPlan {
            plan_id: format!("probe-bark-test-{suffix}"),
            kind: "bark-test".to_string(),
            title: "探针 Bark 测试".to_string(),
            summary: "发送一条固定的 Probe 测试通知，计划和审计记录只保留脱敏配置状态".to_string(),
            steps: vec![
                if self.config.probe.notifications.enabled {
                    "检查 Bark 通知开关已启用".to_string()
                } else {
                    "Bark 通知未启用，测试将跳过外发".to_string()
                },
                if device_key_configured {
                    "使用已配置的 Bark device key，计划中不展示密钥".to_string()
                } else {
                    "未配置 Bark device key，测试不会外发".to_string()
                },
            ],
            payload: json!({
                "enabled": self.config.probe.notifications.enabled,
                "configured": device_key_configured,
                "server_url": self.config.probe.notifications.server_url,
                "device_key": if device_key_configured { "[configured]" } else { "[missing]" },
                "bark_payload": {
                    "title": "NexusHub Probe test",
                    "body": "Probe notification route is configured."
                },
                "redacted_fields": ["device_key", "sound", "group", "url"],
                "would_call_legacy_sentinel_cli": false
            }),
            requires_confirmation: false,
            command: "nexushubd probe bark-test".to_string(),
        }
    }

    pub fn build_event(&self, input: ProbeEventInput) -> ProbeBuiltEvent {
        let event_kind = input.event_kind();
        let event_thread_id = input.thread_id.clone().or_else(|| input.session_id.clone());
        let thread_component = event_thread_id.as_deref().unwrap_or("unknown-thread");
        let turn_component = input.turn_id.as_deref().unwrap_or("unknown-turn");
        let dedupe_key = format!(
            "{event_kind}:{}:{}",
            dedupe_component(thread_component),
            dedupe_component(turn_component)
        );
        let notify_completion = matches!(input.kind, ProbeEventInputKind::NotifyCompletion);
        let (title, message) = probe_event_text(event_kind, notify_completion);
        let last_assistant_message = input
            .last_assistant_message
            .as_deref()
            .map(sanitize_probe_event_assistant_message);
        ProbeBuiltEvent {
            kind: event_kind.to_string(),
            thread_id: event_thread_id,
            turn_id: input.turn_id.clone(),
            title,
            message,
            dedupe_namespace: PROBE_EVENT_DEDUPE_NAMESPACE.to_string(),
            dedupe_key,
            ttl_seconds: PROBE_EVENT_TTL_SECONDS,
            source: if notify_completion {
                "nexushubd probe notify-completion".to_string()
            } else {
                "nexushubd probe hook-stop".to_string()
            },
            payload: json!({
                "turn_id": input.turn_id,
                "session_id": input.session_id,
                "transcript_path": input.transcript_path,
                "last_assistant_message": last_assistant_message,
                "host_label": self.config.codex.host_label,
                "platform": self.paths.kind,
                "notify_completion": notify_completion,
                "auto_reply": false,
                "hidden_desktop_control": false,
                "legacy_sentinel_cli_runtime": false
            }),
        }
    }

    fn hook_command(&self) -> String {
        format!(
            "/opt/nexushub/bin/nexushubd --config {} probe hook-stop",
            self.paths.config_file.display()
        )
    }

    fn resolved_codex_paths(&self) -> ResolvedCodexPaths {
        resolve_codex_paths(
            &self.config.codex.home,
            self.config.codex.app_server_socket.as_deref(),
        )
    }

    fn doctor_status_text(&self) -> String {
        if self.config.probe.enabled {
            "ready"
        } else {
            "disabled"
        }
        .to_string()
    }

    fn lifecycle_status_text(&self) -> String {
        if self.config.probe.enabled {
            "managed"
        } else {
            "disabled"
        }
        .to_string()
    }

    fn lifecycle_next_actions(&self) -> Vec<String> {
        let mut actions = Vec::new();
        if self.config.probe.hooks.manage_stop_hook {
            actions.push("probe-hook-ready".to_string());
        } else {
            actions.push("install-probe-hook".to_string());
        }
        if self.config.probe.logs_db.enabled {
            actions.push("logs-db-maintenance-ready".to_string());
        } else {
            actions.push("enable-logs-db-maintenance".to_string());
        }
        actions
    }

    fn repair_suggestions(&self) -> Vec<String> {
        let mut suggestions = Vec::new();
        if !self.config.probe.enabled {
            suggestions.push("enable Probe runtime in config".to_string());
        }
        if !self.config.probe.hooks.manage_stop_hook {
            suggestions.push("enable Stop hook management before installing hooks".to_string());
        }
        if !self.config.probe.logs_db.enabled {
            suggestions.push("enable logs-db maintenance to keep Probe state bounded".to_string());
        }
        suggestions
    }

    fn logs_db_skip_reason(&self) -> Value {
        if self.config.probe.logs_db.enabled {
            Value::Null
        } else {
            Value::String("logs_db_disabled".to_string())
        }
    }

    fn logs_db_deletion_plan(&self) -> ProbeLogsDbDeletionPlan {
        ProbeLogsDbDeletionPlan {
            enabled: self.config.probe.logs_db.enabled,
            retention_days: self.config.probe.logs_db.retention_days,
            chunk_rows: self.config.probe.logs_db.delete_chunk_rows,
            max_rows_per_run: self.config.probe.logs_db.max_delete_rows_per_run,
            busy_timeout_ms: self.config.probe.logs_db.busy_timeout_ms,
            skip_reason: if self.config.probe.logs_db.enabled {
                None
            } else {
                Some("logs_db_disabled".to_string())
            },
        }
    }

    fn logs_db_vacuum_plan(&self) -> ProbeLogsDbVacuumPlan {
        let enabled = self.config.probe.logs_db.enabled
            && self.config.probe.logs_db.auto_compact_when_codex_closed;
        ProbeLogsDbVacuumPlan {
            enabled,
            requires_codex_closed: true,
            compact_interval_hours: self.config.probe.logs_db.compact_interval_hours,
            compact_min_freelist_mb: self.config.probe.logs_db.compact_min_freelist_mb,
            compact_min_freelist_ratio_percent: self
                .config
                .probe
                .logs_db
                .compact_min_freelist_ratio_percent,
            minimum_free_space_mb: self.config.probe.logs_db.minimum_free_space_mb,
            skip_reason: if enabled {
                None
            } else if !self.config.probe.logs_db.enabled {
                Some("logs_db_disabled".to_string())
            } else {
                Some("vacuum_disabled".to_string())
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProbeActionPlanKind {
    InstallHooks,
    LogsDbMaintain,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeActionPlan {
    pub plan_id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub steps: Vec<String>,
    pub payload: Value,
    pub requires_confirmation: bool,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeLifecycleStatus {
    pub status: String,
    pub lifecycle_status: String,
    pub platform: PlatformKind,
    pub service_kind: String,
    pub service_name: String,
    pub enabled: bool,
    pub hooks_enabled: bool,
    pub notifications_enabled: bool,
    pub logs_db_enabled: bool,
    pub poll_seconds: u64,
    pub recent_limit: usize,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeStatus {
    pub label: String,
    pub enabled: bool,
    pub available: bool,
    pub flavor: String,
    pub platform: PlatformKind,
    pub service_kind: String,
    pub service_name: String,
    pub binary_path: Option<PathBuf>,
    pub hook_status: String,
    pub bark_status: String,
    pub hooks_enabled: bool,
    pub hook_stop_enabled: bool,
    pub bark_enabled: bool,
    pub bark_server_url: String,
    pub bark_notify_completion: bool,
    pub bark_notify_reply_needed: bool,
    pub bark_notify_recoverable: bool,
    pub logs_db_status: String,
    pub lifecycle_status: String,
    pub doctor_status: String,
    pub runtime_version: String,
    pub poll_seconds: u64,
    pub recent_event_count: usize,
    pub running_count: usize,
    pub reply_needed_count: usize,
    pub recoverable_count: usize,
    pub running_threads: Vec<ThreadSummary>,
    pub reply_needed_threads: Vec<ThreadSummary>,
    pub recoverable_threads: Vec<ThreadSummary>,
    pub config_path: PathBuf,
    pub codex_home: PathBuf,
    pub configured_codex_home: Option<String>,
    pub resolved_codex_home: PathBuf,
    pub codex_home_source: String,
    pub logs_db_source: String,
    pub configured_app_server_socket: Option<PathBuf>,
    pub resolved_app_server_socket: Option<PathBuf>,
    pub app_server_socket_source: Option<String>,
    pub discovery_warnings: Vec<String>,
    pub host_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeDiagnostics {
    pub doctor_status: String,
    pub lifecycle_status: String,
    pub app_server_service: String,
    pub app_server_socket: Option<PathBuf>,
    pub configured_app_server_socket: Option<PathBuf>,
    pub resolved_app_server_socket: Option<PathBuf>,
    pub app_server_socket_source: Option<String>,
    pub configured_codex_home: Option<String>,
    pub resolved_codex_home: PathBuf,
    pub codex_home_source: String,
    pub discovery_warnings: Vec<String>,
    pub host_label: String,
    pub runtime_version: String,
    pub managed_boundaries: Vec<String>,
    pub effective_constants: Value,
    pub repair_suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeHookStatus {
    pub status: String,
    pub hook_status: String,
    pub installed: bool,
    pub managed: bool,
    pub hook_command: String,
    pub reload_app_server_after_install: bool,
    pub supported_events: Vec<String>,
    pub dedupe_namespace: String,
    pub dedupe_ttl_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeLogsDbStatus {
    pub target: String,
    pub status: String,
    pub logs_db_status: String,
    pub path: PathBuf,
    pub configured_codex_home: Option<String>,
    pub resolved_codex_home: PathBuf,
    pub codex_home_source: String,
    pub logs_db_source: String,
    pub discovery_warnings: Vec<String>,
    pub enabled: bool,
    pub retention_days: u32,
    pub maintenance_interval_hours: u32,
    pub cutoff_ts: i64,
    pub cutoff_utc: String,
    pub total_rows: u64,
    pub old_rows: u64,
    pub retained_rows: u64,
    pub database_size: u64,
    pub db_size_bytes: u64,
    pub size_bytes: Option<u64>,
    pub wal_size: u64,
    pub wal_size_bytes: u64,
    pub shm_size: u64,
    pub shm_size_bytes: u64,
    pub page_count: u64,
    pub freelist_count: u64,
    pub page_size: u64,
    pub journal_mode: Option<String>,
    pub deletion: ProbeLogsDbDeletionPlan,
    pub vacuum: ProbeLogsDbVacuumPlan,
    pub skip_reason: Option<String>,
    pub error: Option<String>,
    pub last_run_at: Option<String>,
    pub last_maintain_at: Option<String>,
    pub next_run_at: Option<String>,
    pub next_maintain_at: Option<String>,
    pub recent_result: Option<String>,
    pub last_result: Option<String>,
    pub last_run: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeLogsDbMaintenanceResult {
    pub ok: bool,
    pub target: String,
    pub status: String,
    pub path: PathBuf,
    pub configured_codex_home: Option<String>,
    pub resolved_codex_home: PathBuf,
    pub codex_home_source: String,
    pub logs_db_source: String,
    pub discovery_warnings: Vec<String>,
    pub dry_run: bool,
    pub retention_days: u32,
    pub cutoff_ts: i64,
    pub old_rows_before: u64,
    pub deleted_rows: u64,
    pub would_delete_rows: u64,
    pub remaining_old_rows: u64,
    pub total_rows_after: u64,
    pub chunks: u64,
    pub database_size_before: u64,
    pub database_size_after: u64,
    pub page_count_before: u64,
    pub page_count_after: u64,
    pub freelist_count_before: u64,
    pub freelist_count_after: u64,
    pub checkpoint_attempted: bool,
    pub checkpoint_result: Option<String>,
    pub vacuumed: bool,
    pub quick_check_before_vacuum: Option<String>,
    pub quick_check_timeout_seconds: Option<u64>,
    pub skip_reason: Option<String>,
    pub error: Option<String>,
    pub ran_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeLogsDbDeletionPlan {
    pub enabled: bool,
    pub retention_days: u32,
    pub chunk_rows: u32,
    pub max_rows_per_run: u32,
    pub busy_timeout_ms: u64,
    pub skip_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeLogsDbVacuumPlan {
    pub enabled: bool,
    pub requires_codex_closed: bool,
    pub compact_interval_hours: u32,
    pub compact_min_freelist_mb: u64,
    pub compact_min_freelist_ratio_percent: u32,
    pub minimum_free_space_mb: u64,
    pub skip_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeEventInput {
    kind: ProbeEventInputKind,
    thread_id: Option<String>,
    turn_id: Option<String>,
    session_id: Option<String>,
    transcript_path: Option<String>,
    last_assistant_message: Option<String>,
    hook_kind: String,
}

impl ProbeEventInput {
    pub fn hook_stop(thread_id: Option<&str>, turn_id: Option<&str>, kind: &str) -> Self {
        Self::hook_stop_with_context(thread_id, turn_id, None, None, None, kind)
    }

    pub fn hook_stop_with_context(
        thread_id: Option<&str>,
        turn_id: Option<&str>,
        session_id: Option<&str>,
        transcript_path: Option<&str>,
        last_assistant_message: Option<&str>,
        kind: &str,
    ) -> Self {
        Self {
            kind: ProbeEventInputKind::HookStop,
            thread_id: thread_id.map(ToString::to_string),
            turn_id: turn_id.map(ToString::to_string),
            session_id: session_id.map(ToString::to_string),
            transcript_path: transcript_path.map(ToString::to_string),
            last_assistant_message: last_assistant_message.map(ToString::to_string),
            hook_kind: if kind.trim().is_empty() {
                "hook-stop".to_string()
            } else {
                kind.trim().to_string()
            },
        }
    }

    pub fn notify_completion(thread_id: Option<&str>, turn_id: Option<&str>) -> Self {
        Self {
            kind: ProbeEventInputKind::NotifyCompletion,
            thread_id: thread_id.map(ToString::to_string),
            turn_id: turn_id.map(ToString::to_string),
            session_id: None,
            transcript_path: None,
            last_assistant_message: None,
            hook_kind: "completion".to_string(),
        }
    }

    fn event_kind(&self) -> &str {
        match self.kind {
            ProbeEventInputKind::HookStop => self.hook_kind.as_str(),
            ProbeEventInputKind::NotifyCompletion => "completion",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ProbeEventInputKind {
    HookStop,
    NotifyCompletion,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeBuiltEvent {
    pub kind: String,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub title: String,
    pub message: String,
    pub dedupe_namespace: String,
    pub dedupe_key: String,
    pub ttl_seconds: i64,
    pub source: String,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeEventOutcome {
    pub ok: bool,
    pub recorded: bool,
    pub duplicate: bool,
    pub kind: String,
    pub dedupe_namespace: String,
    pub dedupe_key: String,
    pub thread_id: Option<String>,
}

impl ProbeEventOutcome {
    pub fn from_claim(event: &ProbeBuiltEvent, recorded: bool) -> Self {
        Self {
            ok: true,
            recorded,
            duplicate: !recorded,
            kind: event.kind.clone(),
            dedupe_namespace: event.dedupe_namespace.clone(),
            dedupe_key: event.dedupe_key.clone(),
            thread_id: event.thread_id.clone(),
        }
    }
}

impl ProbeLogsDbStatus {
    fn from_resolved_paths(
        resolved: &ResolvedCodexPaths,
        logs_config: &ProbeLogsDbConfig,
        now: i64,
    ) -> Self {
        let path = resolved.logs_db.clone();
        let cutoff = logs_cutoff(logs_config.retention_days, now);
        let mut status = Self::base(resolved, path.clone(), logs_config, cutoff);
        if !logs_config.enabled {
            return status.with_error("disabled", Some("logs_db_disabled".to_string()));
        }
        if !path.exists() {
            return status.with_error(
                "missing_db",
                Some(format!("Codex logs DB not found: {}", path.display())),
            );
        }
        let conn = match open_codex_logs_connection(&path, logs_config, true) {
            Ok(conn) => conn,
            Err(err) => {
                return status.with_error(sqlite_error_status(&err), Some(err.to_string()));
            }
        };
        match inspect_codex_logs_connection(&conn, &path, cutoff) {
            Ok(snapshot) => {
                status.status = "ok".to_string();
                status.logs_db_status = "ok".to_string();
                status.total_rows = snapshot.total_rows;
                status.old_rows = snapshot.old_rows;
                status.retained_rows = snapshot.total_rows.saturating_sub(snapshot.old_rows);
                status.database_size = file_size(&path);
                status.db_size_bytes = status.database_size;
                status.size_bytes = Some(status.database_size);
                status.wal_size = file_size(&wal_path(&path));
                status.wal_size_bytes = status.wal_size;
                status.shm_size = file_size(&shm_path(&path));
                status.shm_size_bytes = status.shm_size;
                status.page_count = snapshot.page_count;
                status.freelist_count = snapshot.freelist_count;
                status.page_size = snapshot.page_size;
                status.journal_mode = snapshot.journal_mode;
                status
            }
            Err(err) => status.with_error(sqlite_error_status(&err), Some(err.to_string())),
        }
    }

    fn base(
        resolved: &ResolvedCodexPaths,
        path: PathBuf,
        config: &ProbeLogsDbConfig,
        cutoff: i64,
    ) -> Self {
        Self {
            target: "codex_logs_2".to_string(),
            status: "unknown".to_string(),
            logs_db_status: "unknown".to_string(),
            path,
            configured_codex_home: resolved.configured_codex_home.clone(),
            resolved_codex_home: resolved.home.clone(),
            codex_home_source: resolved.codex_home_source.clone(),
            logs_db_source: resolved.logs_db_source.clone(),
            discovery_warnings: resolved.discovery_warnings.clone(),
            enabled: config.enabled,
            retention_days: config.retention_days,
            maintenance_interval_hours: config.maintenance_interval_hours,
            cutoff_ts: cutoff,
            cutoff_utc: ts_to_rfc3339(cutoff),
            total_rows: 0,
            old_rows: 0,
            retained_rows: 0,
            database_size: 0,
            db_size_bytes: 0,
            size_bytes: Some(0),
            wal_size: 0,
            wal_size_bytes: 0,
            shm_size: 0,
            shm_size_bytes: 0,
            page_count: 0,
            freelist_count: 0,
            page_size: 0,
            journal_mode: None,
            deletion: ProbeLogsDbDeletionPlan {
                enabled: config.enabled,
                retention_days: config.retention_days,
                chunk_rows: config.delete_chunk_rows,
                max_rows_per_run: config.max_delete_rows_per_run,
                busy_timeout_ms: config.busy_timeout_ms,
                skip_reason: if config.enabled {
                    None
                } else {
                    Some("logs_db_disabled".to_string())
                },
            },
            vacuum: ProbeLogsDbVacuumPlan {
                enabled: config.enabled && config.auto_compact_when_codex_closed,
                requires_codex_closed: true,
                compact_interval_hours: config.compact_interval_hours,
                compact_min_freelist_mb: config.compact_min_freelist_mb,
                compact_min_freelist_ratio_percent: config.compact_min_freelist_ratio_percent,
                minimum_free_space_mb: config.minimum_free_space_mb,
                skip_reason: if config.enabled && config.auto_compact_when_codex_closed {
                    None
                } else if config.enabled {
                    Some("vacuum_disabled".to_string())
                } else {
                    Some("logs_db_disabled".to_string())
                },
            },
            skip_reason: if config.enabled {
                None
            } else {
                Some("logs_db_disabled".to_string())
            },
            error: None,
            last_run_at: None,
            last_maintain_at: None,
            next_run_at: None,
            next_maintain_at: None,
            recent_result: None,
            last_result: None,
            last_run: None,
        }
    }

    fn with_error(mut self, status: &str, error: Option<String>) -> Self {
        self.status = status.to_string();
        self.logs_db_status = status.to_string();
        self.error = error.clone();
        self.skip_reason = error.or_else(|| self.skip_reason.clone());
        self.database_size = file_size(&self.path);
        self.db_size_bytes = self.database_size;
        self.size_bytes = Some(self.database_size);
        self.wal_size = file_size(&wal_path(&self.path));
        self.wal_size_bytes = self.wal_size;
        self.shm_size = file_size(&shm_path(&self.path));
        self.shm_size_bytes = self.shm_size;
        self.deletion.skip_reason = Some(status.to_string());
        self.vacuum.skip_reason = Some(status.to_string());
        self
    }
}

#[derive(Debug)]
struct CodexLogsSnapshot {
    total_rows: u64,
    old_rows: u64,
    page_count: u64,
    freelist_count: u64,
    page_size: u64,
    journal_mode: Option<String>,
}

fn maintain_codex_logs_db_with_quick_check_timeout(
    resolved: &ResolvedCodexPaths,
    logs_config: &ProbeLogsDbConfig,
    dry_run: bool,
    compact: bool,
    now: i64,
    quick_check_timeout: Duration,
) -> Result<ProbeLogsDbMaintenanceResult> {
    let path = resolved.logs_db.clone();
    let cutoff = logs_cutoff(logs_config.retention_days, now);
    let ran_at = ts_to_rfc3339(now);
    let mut result = ProbeLogsDbMaintenanceResult {
        ok: false,
        target: "codex_logs_2".to_string(),
        status: "unknown".to_string(),
        path: path.clone(),
        configured_codex_home: resolved.configured_codex_home.clone(),
        resolved_codex_home: resolved.home.clone(),
        codex_home_source: resolved.codex_home_source.clone(),
        logs_db_source: resolved.logs_db_source.clone(),
        discovery_warnings: resolved.discovery_warnings.clone(),
        dry_run,
        retention_days: logs_config.retention_days,
        cutoff_ts: cutoff,
        old_rows_before: 0,
        deleted_rows: 0,
        would_delete_rows: 0,
        remaining_old_rows: 0,
        total_rows_after: 0,
        chunks: 0,
        database_size_before: 0,
        database_size_after: 0,
        page_count_before: 0,
        page_count_after: 0,
        freelist_count_before: 0,
        freelist_count_after: 0,
        checkpoint_attempted: false,
        checkpoint_result: None,
        vacuumed: false,
        quick_check_before_vacuum: None,
        quick_check_timeout_seconds: None,
        skip_reason: None,
        error: None,
        ran_at,
    };
    if !logs_config.enabled {
        result.status = "disabled".to_string();
        result.skip_reason = Some("logs_db_disabled".to_string());
        result.error = Some("logs_db_disabled".to_string());
        return Ok(result);
    }
    if !path.exists() {
        result.status = "missing_db".to_string();
        result.error = Some(format!("Codex logs DB not found: {}", path.display()));
        return Ok(result);
    }
    let conn = match open_codex_logs_connection(&path, logs_config, false) {
        Ok(conn) => conn,
        Err(err) => {
            result.status = sqlite_error_status(&err).to_string();
            result.error = Some(err.to_string());
            return Ok(result);
        }
    };
    if let Err(err) = ensure_logs_table(&conn) {
        result.status = sqlite_error_status(&err).to_string();
        result.error = Some(err.to_string());
        return Ok(result);
    }
    set_result_before_metrics(&conn, &path, &mut result);

    let old_rows_before = match count_old_logs(&conn, cutoff) {
        Ok(value) => value,
        Err(err) => {
            result.status = sqlite_error_status(&err).to_string();
            result.error = Some(format!("count old Codex logs in {}: {err}", path.display()));
            return Ok(result);
        }
    };
    result.old_rows_before = old_rows_before;
    if dry_run {
        result.ok = true;
        result.status = "ok".to_string();
        result.would_delete_rows =
            old_rows_before.min(u64::from(logs_config.max_delete_rows_per_run.max(1)));
        result.remaining_old_rows = old_rows_before;
        match count_total_logs(&conn) {
            Ok(value) => result.total_rows_after = value,
            Err(err) => {
                result.ok = false;
                result.status = sqlite_error_status(&err).to_string();
                result.error = Some(format!("count Codex logs in {}: {err}", path.display()));
            }
        }
        set_result_after_metrics(&conn, &path, &mut result);
        return Ok(result);
    }

    let max_rows = u64::from(logs_config.max_delete_rows_per_run.max(1));
    let chunk_rows = u64::from(logs_config.delete_chunk_rows.max(1));
    while result.deleted_rows < max_rows {
        let limit = (max_rows - result.deleted_rows).min(chunk_rows);
        let changed = match conn.execute(
            "DELETE FROM logs WHERE rowid IN (
                SELECT rowid FROM logs WHERE ts < ?1 ORDER BY ts ASC, ts_nanos ASC, id ASC LIMIT ?2
            )",
            params![cutoff, limit as i64],
        ) {
            Ok(changed) => changed as u64,
            Err(err) => {
                result.status = sqlite_error_status(&err).to_string();
                result.error = Some(format!(
                    "delete old Codex logs from {}: {err}",
                    path.display()
                ));
                result.remaining_old_rows = old_rows_before.saturating_sub(result.deleted_rows);
                result.total_rows_after = count_total_logs(&conn).unwrap_or(0);
                return Ok(result);
            }
        };
        if changed == 0 {
            break;
        }
        result.deleted_rows += changed;
        result.chunks += 1;
    }
    result.ok = true;
    result.status = "ok".to_string();
    match count_old_logs(&conn, cutoff) {
        Ok(value) => result.remaining_old_rows = value,
        Err(err) => {
            result.ok = false;
            result.status = sqlite_error_status(&err).to_string();
            result.error = Some(format!(
                "count remaining old Codex logs in {}: {err}",
                path.display()
            ));
            return Ok(result);
        }
    }
    match count_total_logs(&conn) {
        Ok(value) => result.total_rows_after = value,
        Err(err) => {
            result.ok = false;
            result.status = sqlite_error_status(&err).to_string();
            result.error = Some(format!("count Codex logs in {}: {err}", path.display()));
            return Ok(result);
        }
    }

    if compact {
        maybe_vacuum_codex_logs(&conn, &path, logs_config, &mut result, quick_check_timeout);
    }
    if result.ok {
        result.checkpoint_attempted = true;
        result.checkpoint_result = wal_checkpoint(&conn, "TRUNCATE").ok();
    }
    set_result_after_metrics(&conn, &path, &mut result);
    Ok(result)
}

fn maybe_vacuum_codex_logs(
    conn: &Connection,
    path: &Path,
    config: &ProbeLogsDbConfig,
    result: &mut ProbeLogsDbMaintenanceResult,
    quick_check_timeout: Duration,
) {
    if !config.auto_compact_when_codex_closed {
        result.skip_reason = Some("vacuum_disabled".to_string());
        return;
    }
    let page_size = pragma_u64(conn, "page_size").ok().flatten().unwrap_or(4096);
    let page_count = pragma_u64(conn, "page_count").ok().flatten().unwrap_or(0);
    let freelist_count = pragma_u64(conn, "freelist_count")
        .ok()
        .flatten()
        .unwrap_or(0);
    let freelist_bytes = freelist_count.saturating_mul(page_size);
    let min_freelist_bytes = config.compact_min_freelist_mb.saturating_mul(1024 * 1024);
    let freelist_ratio_percent = freelist_count
        .saturating_mul(100)
        .checked_div(page_count)
        .unwrap_or(0);
    if freelist_bytes < min_freelist_bytes {
        result.skip_reason = Some("vacuum_freelist_below_minimum".to_string());
        return;
    }
    if freelist_ratio_percent < u64::from(config.compact_min_freelist_ratio_percent) {
        result.skip_reason = Some("vacuum_freelist_ratio_below_minimum".to_string());
        return;
    }
    if config.minimum_free_space_mb > 0 {
        let minimum_free_bytes = config.minimum_free_space_mb.saturating_mul(1024 * 1024);
        match free_space_bytes(path) {
            Some(free_bytes) if free_bytes >= minimum_free_bytes => {}
            Some(_) => {
                result.skip_reason = Some("vacuum_insufficient_free_space".to_string());
                return;
            }
            None => {
                result.skip_reason = Some("vacuum_free_space_unknown".to_string());
                return;
            }
        }
    }
    result.quick_check_timeout_seconds = Some(quick_check_timeout.as_secs());
    match quick_check(conn, quick_check_timeout) {
        Ok(()) => result.quick_check_before_vacuum = Some("ok".to_string()),
        Err(reason) => {
            result.quick_check_before_vacuum = Some(reason.clone());
            result.skip_reason = Some(reason);
            return;
        }
    }
    match conn.execute_batch("VACUUM") {
        Ok(()) => {
            result.vacuumed = true;
            result.skip_reason = None;
        }
        Err(err) => {
            result.ok = false;
            result.status = sqlite_error_status(&err).to_string();
            result.error = Some(format!("vacuum Codex logs DB {}: {err}", path.display()));
        }
    }
}

fn set_result_before_metrics(
    conn: &Connection,
    path: &Path,
    result: &mut ProbeLogsDbMaintenanceResult,
) {
    result.database_size_before = file_size(path);
    result.page_count_before = pragma_u64(conn, "page_count").ok().flatten().unwrap_or(0);
    result.freelist_count_before = pragma_u64(conn, "freelist_count")
        .ok()
        .flatten()
        .unwrap_or(0);
}

fn set_result_after_metrics(
    conn: &Connection,
    path: &Path,
    result: &mut ProbeLogsDbMaintenanceResult,
) {
    result.database_size_after = file_size(path);
    result.page_count_after = pragma_u64(conn, "page_count").ok().flatten().unwrap_or(0);
    result.freelist_count_after = pragma_u64(conn, "freelist_count")
        .ok()
        .flatten()
        .unwrap_or(0);
}

fn wal_checkpoint(conn: &Connection, mode: &str) -> rusqlite::Result<String> {
    let sql = format!("PRAGMA wal_checkpoint({mode})");
    conn.query_row(&sql, [], |row| {
        let busy: i64 = row.get(0)?;
        let log: i64 = row.get(1)?;
        let checkpointed: i64 = row.get(2)?;
        Ok(format!(
            "mode={mode}, busy={busy}, log={log}, checkpointed={checkpointed}"
        ))
    })
}

fn quick_check(conn: &Connection, timeout: Duration) -> std::result::Result<(), String> {
    let started = Instant::now();
    conn.progress_handler(1_000, Some(move || started.elapsed() >= timeout));
    let checked = conn.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0));
    conn.progress_handler(0, None::<fn() -> bool>);
    match checked {
        Ok(value) if value == "ok" => Ok(()),
        Ok(value) => Err(format!("quick_check_failed:{value}")),
        Err(_err) if started.elapsed() >= timeout => Err("quick_check_timeout".to_string()),
        Err(err) => Err(format!("quick_check_failed:{err}")),
    }
}

fn free_space_bytes(path: &Path) -> Option<u64> {
    let target = path.parent().unwrap_or(path);
    let output = StdCommand::new("df").arg("-Pk").arg(target).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().nth(1)?;
    let available_kb = line.split_whitespace().nth(3)?.parse::<u64>().ok()?;
    Some(available_kb.saturating_mul(1024))
}

fn inspect_codex_logs_connection(
    conn: &Connection,
    path: &Path,
    cutoff: i64,
) -> rusqlite::Result<CodexLogsSnapshot> {
    ensure_logs_table(conn)?;
    Ok(CodexLogsSnapshot {
        total_rows: count_total_logs(conn)?,
        old_rows: count_old_logs(conn, cutoff)?,
        page_count: pragma_u64(conn, "page_count")?
            .unwrap_or_else(|| file_size(path).div_ceil(4096)),
        freelist_count: pragma_u64(conn, "freelist_count")?.unwrap_or(0),
        page_size: pragma_u64(conn, "page_size")?.unwrap_or(0),
        journal_mode: pragma_string(conn, "journal_mode")?,
    })
}

fn open_codex_logs_connection(
    path: &Path,
    config: &ProbeLogsDbConfig,
    readonly: bool,
) -> rusqlite::Result<Connection> {
    let flags = if readonly {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    let conn = Connection::open_with_flags(path, flags)?;
    conn.busy_timeout(Duration::from_millis(config.busy_timeout_ms))?;
    Ok(conn)
}

fn ensure_logs_table(conn: &Connection) -> rusqlite::Result<()> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='logs'",
        [],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(())
}

fn count_total_logs(conn: &Connection) -> rusqlite::Result<u64> {
    conn.query_row("SELECT COUNT(*) FROM logs", [], |row| row.get::<_, i64>(0))
        .map(|value| value.max(0) as u64)
}

fn count_old_logs(conn: &Connection, cutoff: i64) -> rusqlite::Result<u64> {
    conn.query_row(
        "SELECT COUNT(*) FROM logs WHERE ts < ?1",
        params![cutoff],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value.max(0) as u64)
}

fn pragma_u64(conn: &Connection, name: &str) -> rusqlite::Result<Option<u64>> {
    let sql = format!("PRAGMA {name}");
    conn.query_row(&sql, [], |row| row.get::<_, i64>(0))
        .map(|value| Some(value.max(0) as u64))
}

fn pragma_string(conn: &Connection, name: &str) -> rusqlite::Result<Option<String>> {
    let sql = format!("PRAGMA {name}");
    conn.query_row(&sql, [], |row| row.get::<_, String>(0))
        .map(Some)
}

fn logs_cutoff(retention_days: u32, now: i64) -> i64 {
    now - i64::from(retention_days.max(1)) * 86_400
}

fn wal_path(path: &Path) -> PathBuf {
    path.with_extension("sqlite-wal")
}

fn shm_path(path: &Path) -> PathBuf {
    path.with_extension("sqlite-shm")
}

fn file_size(path: &Path) -> u64 {
    fs::metadata(path).map(|meta| meta.len()).unwrap_or(0)
}

fn now_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn probe_event_text(event_kind: &str, notify_completion: bool) -> (String, String) {
    if notify_completion || event_kind == "completion" {
        return (
            "任务完成".to_string(),
            "Codex completion event recorded by NexusHub Probe".to_string(),
        );
    }
    match event_kind {
        "reply-needed" | "reply_needed" => (
            "需要回复".to_string(),
            "Codex reply-needed event recorded by NexusHub Probe".to_string(),
        ),
        "recoverable" => (
            "可恢复任务".to_string(),
            "Codex recoverable event recorded by NexusHub Probe".to_string(),
        ),
        _ => (
            "Codex Stop Hook".to_string(),
            "Stop Hook event recorded by NexusHub Probe".to_string(),
        ),
    }
}

fn sanitize_probe_event_assistant_message(value: &str) -> String {
    truncate_utf8_with_marker(
        &redact_output(value),
        PROBE_EVENT_ASSISTANT_MESSAGE_MAX_BYTES,
    )
}

fn truncate_utf8_with_marker(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let marker = "\n[truncated]";
    let limit = max_bytes.saturating_sub(marker.len());
    let mut end = limit.min(value.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    let mut truncated = value[..end].to_string();
    truncated.push_str(marker);
    truncated
}

fn ts_to_rfc3339(ts: i64) -> String {
    Utc.timestamp_opt(ts, 0)
        .single()
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| ts.to_string())
}

fn sqlite_error_status(err: &rusqlite::Error) -> &'static str {
    match err {
        rusqlite::Error::InvalidQuery => "missing_logs_table",
        rusqlite::Error::SqliteFailure(error, _) => match error.code {
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => "busy",
            ErrorCode::CannotOpen => "permission_denied",
            _ => "error",
        },
        _ => "error",
    }
}

pub fn safe_thread_probe_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
}

fn dedupe_component(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if normalized.is_empty() {
        "unknown".to_string()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_uses_caller_supplied_quick_check_timeout() {
        let root = unique_temp_dir("nexushub-probe-quick-check-timeout");
        let codex_home = root.join(".codex");
        fs::create_dir_all(&codex_home).unwrap();
        let logs_path = codex_home.join("logs_2.sqlite");
        seed_codex_logs_db(&logs_path, &[100, 200_000]);

        let resolved = ResolvedCodexPaths {
            configured_codex_home: None,
            home: codex_home.clone(),
            logs_db: logs_path.clone(),
            state_db: codex_home.join("state_5.sqlite"),
            session_index: codex_home.join("session_index.jsonl"),
            sessions_dir: codex_home.join("sessions"),
            configured_app_server_socket: None,
            app_server_socket: None,
            codex_home_source: "test".to_string(),
            logs_db_source: "test".to_string(),
            app_server_socket_source: None,
            discovery_warnings: Vec::new(),
        };
        let mut config = ProbeLogsDbConfig {
            retention_days: 1,
            delete_chunk_rows: 10,
            max_delete_rows_per_run: 10,
            compact_min_freelist_mb: 0,
            compact_min_freelist_ratio_percent: 0,
            minimum_free_space_mb: 0,
            ..ProbeLogsDbConfig::default()
        };
        config.auto_compact_when_codex_closed = true;

        let result = maintain_codex_logs_db_with_quick_check_timeout(
            &resolved,
            &config,
            false,
            true,
            200_000,
            Duration::from_secs(600),
        )
        .unwrap();

        assert!(result.vacuumed);
        assert_eq!(result.quick_check_before_vacuum.as_deref(), Some("ok"));
        assert_eq!(result.quick_check_timeout_seconds, Some(600));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn build_event_redacts_and_truncates_last_assistant_message_and_names_status_events() {
        let mut config = Config::default();
        config.codex.host_label = "probe-test".to_string();
        let runtime = ProbeRuntime::new(config, PlatformPaths::current());
        let message = format!("first line\nTOKEN=secret-token\n{}", "完成".repeat(3000));

        let event = runtime.build_event(ProbeEventInput::hook_stop_with_context(
            Some("thread-a"),
            Some("turn-a"),
            Some("session-a"),
            Some("/tmp/transcript.jsonl"),
            Some(&message),
            "reply-needed",
        ));

        assert_eq!(event.kind, "reply-needed");
        assert_eq!(event.title, "需要回复");
        assert_eq!(
            event.message,
            "Codex reply-needed event recorded by NexusHub Probe"
        );
        let stored = event.payload["last_assistant_message"].as_str().unwrap();
        assert!(stored.contains("[redacted sensitive line]"));
        assert!(!stored.contains("secret-token"));
        assert!(stored.contains("[truncated]"));
        assert!(stored.len() <= PROBE_EVENT_ASSISTANT_MESSAGE_MAX_BYTES);

        let recoverable = runtime.build_event(ProbeEventInput::hook_stop(
            Some("thread-a"),
            Some("turn-b"),
            "recoverable",
        ));
        assert_eq!(recoverable.title, "可恢复任务");
        assert_eq!(
            recoverable.message,
            "Codex recoverable event recorded by NexusHub Probe"
        );
    }

    fn seed_codex_logs_db(path: &Path, timestamps: &[i64]) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts INTEGER NOT NULL,
                ts_nanos INTEGER NOT NULL,
                level TEXT NOT NULL,
                target TEXT NOT NULL,
                estimated_bytes INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX idx_logs_ts ON logs(ts DESC, ts_nanos DESC, id DESC);
            "#,
        )
        .unwrap();
        for ts in timestamps {
            conn.execute(
                "INSERT INTO logs(ts, ts_nanos, level, target, estimated_bytes) VALUES(?1, 0, 'INFO', 'test', 1)",
                params![ts],
            )
            .unwrap();
        }
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{unique}"))
    }
}
