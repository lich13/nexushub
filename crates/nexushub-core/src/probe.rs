use crate::{
    config::{Config, ProbeLogsDbConfig},
    platform::{PlatformKind, PlatformPaths},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use uuid::Uuid;

pub const PROBE_EVENT_DEDUPE_NAMESPACE: &str = "probe_event";
pub const PROBE_EVENT_TTL_SECONDS: i64 = 300;

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
            logs_db_status: self.logs_db_status_text(),
            lifecycle_status: self.lifecycle_status_text(),
            doctor_status: self.doctor_status_text(),
            runtime_version: env!("CARGO_PKG_VERSION").to_string(),
            recent_event_count: 0,
            running_count: 0,
            reply_needed_count: 0,
            recoverable_count: 0,
            config_path: self.paths.config_file.clone(),
            codex_home: self.config.codex.home.clone(),
            host_label: self.config.codex.host_label.clone(),
        })
    }

    pub fn diagnostics(&self) -> ProbeDiagnostics {
        ProbeDiagnostics {
            doctor_status: self.doctor_status_text(),
            lifecycle_status: self.lifecycle_status_text(),
            app_server_service: self.config.codex.app_server_service.clone(),
            app_server_socket: self.config.codex.app_server_socket.clone(),
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
        ProbeLogsDbStatus::from_config(&self.paths, &self.config.probe.logs_db)
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
                        self.config.codex.home.join("hooks.json").display()
                    ),
                    format!("Stop Hook 命令包含 `{}`", self.hook_command()),
                    if self.config.probe.hooks.reload_app_server_after_install {
                        format!("重载 {}", self.config.codex.app_server_service)
                    } else {
                        "不重载 app-server".to_string()
                    },
                ],
                payload: json!({
                    "codex_home": self.config.codex.home,
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
            ProbeActionPlanKind::LegacyCleanup => Ok(ProbeActionPlan {
                plan_id: format!("probe-legacy-cleanup-{suffix}"),
                kind: "legacy-cleanup".to_string(),
                title: "旧 Sentinel 清理".to_string(),
                summary: "健康门禁通过后备份并清理旧 codex-sentinel-server 运行物".to_string(),
                steps: legacy_cleanup_paths()
                    .into_iter()
                    .map(|path| format!("检查并备份 {path}"))
                    .collect(),
                payload: json!({
                    "paths": legacy_cleanup_paths(),
                    "requires_health_gate": true,
                    "backup_root": "/opt/nexushub/backups/probe-legacy"
                }),
                requires_confirmation: true,
                command: "nexushubd probe legacy-cleanup".to_string(),
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
        let thread_component = input.thread_id.as_deref().unwrap_or("unknown-thread");
        let turn_component = input.turn_id.as_deref().unwrap_or("unknown-turn");
        let dedupe_key = format!(
            "{event_kind}:{}:{}",
            dedupe_component(thread_component),
            dedupe_component(turn_component)
        );
        let notify_completion = matches!(input.kind, ProbeEventInputKind::NotifyCompletion);
        ProbeBuiltEvent {
            kind: event_kind.to_string(),
            thread_id: input.thread_id.clone(),
            turn_id: input.turn_id.clone(),
            title: if notify_completion {
                "任务完成".to_string()
            } else {
                "Codex Stop Hook".to_string()
            },
            message: if notify_completion {
                "Codex completion event recorded by NexusHub Probe".to_string()
            } else {
                "Stop Hook event recorded by NexusHub Probe".to_string()
            },
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

    fn logs_db_status_text(&self) -> String {
        if self.config.probe.logs_db.enabled {
            "maintenance_ready"
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
    LegacyCleanup,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub logs_db_status: String,
    pub lifecycle_status: String,
    pub doctor_status: String,
    pub runtime_version: String,
    pub recent_event_count: usize,
    pub running_count: usize,
    pub reply_needed_count: usize,
    pub recoverable_count: usize,
    pub config_path: PathBuf,
    pub codex_home: PathBuf,
    pub host_label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProbeDiagnostics {
    pub doctor_status: String,
    pub lifecycle_status: String,
    pub app_server_service: String,
    pub app_server_socket: Option<PathBuf>,
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
    pub status: String,
    pub logs_db_status: String,
    pub path: PathBuf,
    pub enabled: bool,
    pub retention_days: u32,
    pub maintenance_interval_hours: u32,
    pub size_bytes: Option<u64>,
    pub deletion: ProbeLogsDbDeletionPlan,
    pub vacuum: ProbeLogsDbVacuumPlan,
    pub skip_reason: Option<String>,
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
    hook_kind: String,
}

impl ProbeEventInput {
    pub fn hook_stop(thread_id: Option<&str>, turn_id: Option<&str>, kind: &str) -> Self {
        Self {
            kind: ProbeEventInputKind::HookStop,
            thread_id: thread_id.map(ToString::to_string),
            turn_id: turn_id.map(ToString::to_string),
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
    fn from_config(paths: &PlatformPaths, config: &ProbeLogsDbConfig) -> Self {
        let path = paths.data_dir.join("nexushub.sqlite");
        let size_bytes = std::fs::metadata(&path).ok().map(|meta| meta.len());
        let status = if config.enabled {
            "maintenance_ready"
        } else {
            "disabled"
        }
        .to_string();
        Self {
            logs_db_status: status.clone(),
            status,
            path,
            enabled: config.enabled,
            retention_days: config.retention_days,
            maintenance_interval_hours: config.maintenance_interval_hours,
            size_bytes,
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
        }
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

fn legacy_cleanup_paths() -> Vec<&'static str> {
    vec![
        "/etc/systemd/system/codex-sentinel-server.service",
        "/opt/codex-sentinel-server",
        "/etc/codex-sentinel-server",
        "/var/lib/codex-sentinel-server",
        "/usr/local/bin/codex-sentinel-server",
        "/root/.codex/hooks.json",
    ]
}
