use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    archive,
    codex::CodexPaths,
    platform::PlatformPaths,
    services::commands,
    services::system::{require_capability, Capability},
};

pub use crate::archive::{
    ArchiveDeletePlan, ArchiveDeleteResult, HiddenThreadDeletePlan, HiddenThreadDeleteResult,
};

pub const ARCHIVE_DELETE_CONFIRMATION_MESSAGE: &str = "archive deletion must be confirmed";
pub const HIDDEN_DELETE_CONFIRMATION_MESSAGE: &str = "hidden thread deletion must be confirmed";
pub const UNIFIED_CLEANUP_CONFIRMATION_MESSAGE: &str = "unified thread cleanup must be confirmed";

pub const SCOPE_CODEX_ARCHIVED: &str = "codex_archived";
pub const SCOPE_CODEX_HIDDEN: &str = "codex_hidden";
pub const SCOPE_ANTIGRAVITY_ARCHIVED: &str = "antigravity_archived";
pub const SCOPE_DSH_ARCHIVED: &str = "dsh_archived";
pub const SCOPE_CLAUDE_EMPTY: &str = "claude_empty";
pub const SCOPE_GROK_EMPTY: &str = "grok_empty";

pub const ALL_UNIFIED_SCOPES: &[&str] = &[
    SCOPE_CODEX_ARCHIVED,
    SCOPE_CODEX_HIDDEN,
    SCOPE_ANTIGRAVITY_ARCHIVED,
    SCOPE_DSH_ARCHIVED,
    SCOPE_CLAUDE_EMPTY,
    SCOPE_GROK_EMPTY,
];

#[derive(Debug, Clone)]
pub struct UnifiedCleanupPaths {
    pub codex: CodexPaths,
    pub antigravity: crate::antigravity::sessions::AntigravityPaths,
    pub dsh: crate::dsh::DshPaths,
    pub claude: crate::claude::ClaudePaths,
    pub grok: crate::grok::GrokPaths,
}

impl UnifiedCleanupPaths {
    pub fn default_for_codex(codex: CodexPaths) -> Self {
        Self {
            codex,
            antigravity: crate::antigravity::sessions::AntigravityPaths::default_for_user(),
            dsh: crate::dsh::DshPaths::default_for_user(),
            claude: crate::claude::ClaudePaths::default_for_user(),
            grok: crate::grok::GrokPaths::default_for_user(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedCleanupDryRunRequest {
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub include_codex_hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedCleanupCandidate {
    pub id: String,
    pub title: String,
    pub estimated_bytes: u64,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedCleanupScopePlan {
    pub scope: String,
    pub provider: String,
    pub label: String,
    pub supported: bool,
    pub scanned: bool,
    pub unavailable_reason: Option<String>,
    pub candidate_count: usize,
    pub deletable_count: usize,
    pub estimated_bytes: u64,
    pub candidates: Vec<UnifiedCleanupCandidate>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedCleanupPlan {
    pub total_candidates: usize,
    pub total_deletable: usize,
    pub total_estimated_bytes: u64,
    pub scopes: Vec<UnifiedCleanupScopePlan>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedCleanupExecuteRequest {
    pub confirmed: bool,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedCleanupScopeResult {
    pub scope: String,
    pub provider: String,
    pub label: String,
    pub deleted_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    pub deleted_bytes: u64,
    pub residuals: Vec<String>,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedCleanupResult {
    pub deleted_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    pub deleted_bytes: u64,
    pub stopped_early: bool,
    pub scopes: Vec<UnifiedCleanupScopeResult>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CleanupAction {
    #[serde(rename = "archiveDeleteDryRun", alias = "archive-delete-dry-run")]
    ArchiveDeleteDryRun,
    #[serde(rename = "archiveDeleteExecute", alias = "archive-delete-execute")]
    ArchiveDeleteExecute,
    #[serde(rename = "hiddenDeleteDryRun", alias = "hidden-delete-dry-run")]
    HiddenDeleteDryRun,
    #[serde(rename = "hiddenDeleteExecute", alias = "hidden-delete-execute")]
    HiddenDeleteExecute,
}

impl CleanupAction {
    pub fn as_rpc_action(self) -> &'static str {
        match self {
            Self::ArchiveDeleteDryRun => commands::CLEANUP_ARCHIVE_DRY_RUN,
            Self::ArchiveDeleteExecute => commands::CLEANUP_ARCHIVE_EXECUTE,
            Self::HiddenDeleteDryRun => commands::CLEANUP_HIDDEN_DRY_RUN,
            Self::HiddenDeleteExecute => commands::CLEANUP_HIDDEN_EXECUTE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupTarget {
    Archived,
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupActionPlan {
    pub required_capability: Capability,
    pub action: CleanupAction,
    pub command: String,
    pub target: CleanupTarget,
    pub execute: bool,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupExecuteRequest {
    pub confirmed: bool,
    #[serde(default, alias = "expectedCount", alias = "expected_count")]
    pub expected_count: Option<u64>,
    #[serde(default)]
    pub candidates: Option<Vec<archive::HiddenThreadSelection>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupConfirmationPlan {
    pub confirmed: bool,
    pub expected_count: Option<u64>,
    pub payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupOperationKind {
    DryRun,
    Execute,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupOperationPlan {
    pub required_capability: Capability,
    pub action: CleanupAction,
    pub command: String,
    pub target: CleanupTarget,
    pub operation: CleanupOperationKind,
    pub execute: bool,
    pub requires_confirmation: bool,
    pub requires_prior_dry_run: bool,
    pub requires_expected_count: bool,
    pub confirmation_message: Option<String>,
    pub confirmation: CleanupConfirmationPlan,
}

pub fn plan_cleanup_action(
    platform: &PlatformPaths,
    action: CleanupAction,
) -> Result<CleanupActionPlan> {
    require_capability(platform, Capability::ThreadCleanup)?;
    let (target, execute) = match action {
        CleanupAction::ArchiveDeleteDryRun => (CleanupTarget::Archived, false),
        CleanupAction::ArchiveDeleteExecute => (CleanupTarget::Archived, true),
        CleanupAction::HiddenDeleteDryRun => (CleanupTarget::Hidden, false),
        CleanupAction::HiddenDeleteExecute => (CleanupTarget::Hidden, true),
    };
    Ok(CleanupActionPlan {
        required_capability: Capability::ThreadCleanup,
        action,
        command: action.as_rpc_action().to_string(),
        target,
        execute,
        requires_confirmation: execute,
    })
}

pub fn plan_cleanup_operation(
    platform: &PlatformPaths,
    target: CleanupTarget,
    operation: CleanupOperationKind,
) -> Result<CleanupOperationPlan> {
    plan_cleanup_operation_with_confirmation(
        platform,
        target,
        operation,
        CleanupExecuteRequest::default(),
    )
}

pub fn plan_cleanup_execute_operation(
    platform: &PlatformPaths,
    target: CleanupTarget,
    request: CleanupExecuteRequest,
) -> Result<CleanupOperationPlan> {
    plan_cleanup_operation_with_confirmation(
        platform,
        target,
        CleanupOperationKind::Execute,
        request,
    )
}

pub fn plan_cleanup_operation_with_confirmation(
    platform: &PlatformPaths,
    target: CleanupTarget,
    operation: CleanupOperationKind,
    confirmation: CleanupExecuteRequest,
) -> Result<CleanupOperationPlan> {
    if matches!(operation, CleanupOperationKind::Execute) {
        validate_cleanup_execute_confirmation(target, &confirmation)?;
    }

    let action = match (target, operation) {
        (CleanupTarget::Archived, CleanupOperationKind::DryRun) => {
            CleanupAction::ArchiveDeleteDryRun
        }
        (CleanupTarget::Archived, CleanupOperationKind::Execute) => {
            CleanupAction::ArchiveDeleteExecute
        }
        (CleanupTarget::Hidden, CleanupOperationKind::DryRun) => CleanupAction::HiddenDeleteDryRun,
        (CleanupTarget::Hidden, CleanupOperationKind::Execute) => {
            CleanupAction::HiddenDeleteExecute
        }
    };
    let action_plan = plan_cleanup_action(platform, action)?;
    Ok(CleanupOperationPlan {
        required_capability: action_plan.required_capability,
        action: action_plan.action,
        command: action_plan.command,
        target: action_plan.target,
        operation,
        execute: action_plan.execute,
        requires_confirmation: action_plan.requires_confirmation,
        requires_prior_dry_run: action_plan.execute,
        requires_expected_count: false,
        confirmation_message: action_plan
            .execute
            .then(|| cleanup_confirmation_message(target).to_string()),
        confirmation: cleanup_confirmation_plan(confirmation),
    })
}

pub fn cleanup_confirmation_plan(request: CleanupExecuteRequest) -> CleanupConfirmationPlan {
    CleanupConfirmationPlan {
        confirmed: request.confirmed,
        expected_count: request.expected_count,
        payload: json!({
            "confirmed": request.confirmed,
            "expectedCount": request.expected_count,
            "candidates": request.candidates,
        }),
    }
}

pub fn cleanup_confirmation_message(target: CleanupTarget) -> &'static str {
    match target {
        CleanupTarget::Archived => ARCHIVE_DELETE_CONFIRMATION_MESSAGE,
        CleanupTarget::Hidden => HIDDEN_DELETE_CONFIRMATION_MESSAGE,
    }
}

fn validate_cleanup_execute_confirmation(
    target: CleanupTarget,
    request: &CleanupExecuteRequest,
) -> Result<()> {
    if !request.confirmed {
        anyhow::bail!(cleanup_confirmation_message(target));
    }
    if request.candidates.is_none() {
        anyhow::bail!("清理需要明确的线程列表");
    }
    Ok(())
}

pub fn hidden_candidates(
    plan: &CleanupOperationPlan,
) -> Result<Vec<archive::HiddenThreadSelection>> {
    let candidates = plan
        .confirmation
        .payload
        .get("candidates")
        .filter(|value| !value.is_null())
        .ok_or_else(|| anyhow::anyhow!("隐藏清理缺少预览候选"))?;
    Ok(serde_json::from_value(candidates.clone())?)
}

pub fn dry_run_archived_with_capability(
    platform: &PlatformPaths,
    paths: &CodexPaths,
) -> Result<ArchiveDeletePlan> {
    plan_cleanup_action(platform, CleanupAction::ArchiveDeleteDryRun)?;
    archive::plan_delete_archived(paths)
}

pub fn execute_archived_with_capability(
    platform: &PlatformPaths,
    paths: &CodexPaths,
    candidates: &[archive::HiddenThreadSelection],
) -> Result<ArchiveDeleteResult> {
    plan_cleanup_action(platform, CleanupAction::ArchiveDeleteExecute)?;
    archive::execute_delete_archived(paths, candidates)
}

pub fn dry_run_hidden_with_capability(
    platform: &PlatformPaths,
    paths: &CodexPaths,
) -> Result<HiddenThreadDeletePlan> {
    plan_cleanup_action(platform, CleanupAction::HiddenDeleteDryRun)?;
    archive::plan_delete_hidden(paths)
}

pub fn execute_hidden_with_capability(
    platform: &PlatformPaths,
    paths: &CodexPaths,
    candidates: &[archive::HiddenThreadSelection],
) -> Result<HiddenThreadDeleteResult> {
    plan_cleanup_action(platform, CleanupAction::HiddenDeleteExecute)?;
    archive::execute_delete_hidden(paths, candidates)
}

fn scope_meta(scope: &str) -> Result<(&'static str, &'static str)> {
    match scope {
        SCOPE_CODEX_ARCHIVED => Ok(("codex", "Codex 已归档线程")),
        SCOPE_CODEX_HIDDEN => Ok(("codex", "Codex 隐藏线程")),
        SCOPE_ANTIGRAVITY_ARCHIVED => Ok(("antigravity", "Antigravity 已归档线程")),
        SCOPE_DSH_ARCHIVED => Ok(("dsh", "DeepSeek Harness 已归档线程")),
        SCOPE_CLAUDE_EMPTY => Ok(("claude", "Claude Code 空残余会话")),
        SCOPE_GROK_EMPTY => Ok(("grok", "Grok Build 空残余会话")),
        _ => anyhow::bail!("未知的清理范围: {scope}"),
    }
}

fn dry_run_single_scope(
    platform: &PlatformPaths,
    paths: &UnifiedCleanupPaths,
    scope: &str,
    scan_hidden: bool,
) -> Result<UnifiedCleanupScopePlan> {
    let (provider, label) = scope_meta(scope)?;
    let mut out = UnifiedCleanupScopePlan {
        scope: scope.to_owned(),
        provider: provider.to_owned(),
        label: label.to_owned(),
        supported: true,
        scanned: true,
        unavailable_reason: None,
        candidate_count: 0,
        deletable_count: 0,
        estimated_bytes: 0,
        candidates: Vec::new(),
        issues: Vec::new(),
    };
    match scope {
        SCOPE_CODEX_ARCHIVED => match archive::plan_delete_archived(&paths.codex) {
            Ok(plan) => {
                for id in plan.archived_ids {
                    let (title, bytes) =
                        crate::selected_codex::tree::preview(&paths.codex, &id, false)
                            .map(|p| {
                                let t = if p.selected.title.trim().is_empty() {
                                    id.clone()
                                } else {
                                    p.selected.title
                                };
                                (t, p.selected.bytes)
                            })
                            .unwrap_or_else(|_| (id.clone(), 0));
                    out.estimated_bytes = out.estimated_bytes.saturating_add(bytes);
                    out.candidates.push(UnifiedCleanupCandidate {
                        id,
                        title,
                        estimated_bytes: bytes,
                        blocked_reason: None,
                    });
                }
                out.candidate_count = out.candidates.len();
                out.deletable_count = out.candidate_count;
            }
            Err(err) => out.issues.push(err.to_string()),
        },
        SCOPE_CODEX_HIDDEN => {
            if !scan_hidden {
                out.scanned = false;
                return Ok(out);
            }
            match archive::plan_delete_hidden(&paths.codex) {
                Ok(plan) => {
                    for c in plan.candidates {
                        let bytes =
                            crate::selected_codex::tree::preview(&paths.codex, &c.id, false)
                                .map(|p| p.selected.bytes)
                                .unwrap_or(0);
                        let blocked_reason = (!c.allowed)
                            .then(|| c.reason.unwrap_or_else(|| "当前线程暂不可清理".into()));
                        if blocked_reason.is_none() {
                            out.deletable_count += 1;
                            out.estimated_bytes = out.estimated_bytes.saturating_add(bytes);
                        }
                        let title = if c.title.trim().is_empty() {
                            c.id.clone()
                        } else {
                            c.title
                        };
                        out.candidates.push(UnifiedCleanupCandidate {
                            id: c.id,
                            title,
                            estimated_bytes: bytes,
                            blocked_reason,
                        });
                    }
                    out.candidate_count = out.candidates.len();
                }
                Err(err) => out.issues.push(err.to_string()),
            }
        }
        SCOPE_ANTIGRAVITY_ARCHIVED => {
            if platform.kind != crate::platform::PlatformKind::Macos {
                out.supported = false;
                out.scanned = false;
                out.unavailable_reason = Some("仅本机 macOS 支持 Antigravity 会话清理".into());
                return Ok(out);
            }
            match crate::antigravity::sessions::list_sessions(
                &paths.antigravity,
                &crate::provider_sessions::ProviderListRequest {
                    archived: Some(true),
                    limit: Some(1000),
                    q: None,
                },
            ) {
                Ok(sessions) => {
                    for summary in sessions.into_iter().filter(|s| s.archived) {
                        match crate::antigravity::sessions::preview_batch_item(
                            &paths.antigravity,
                            &summary.session_key,
                            crate::services::sessions::SessionOperation::Delete,
                            true,
                        ) {
                            Ok(preview) => {
                                let blocked_reason = (!preview.allowed).then(|| {
                                    preview.reason.unwrap_or_else(|| "当前无法安全清理".into())
                                });
                                if blocked_reason.is_none() {
                                    out.deletable_count += 1;
                                    out.estimated_bytes =
                                        out.estimated_bytes.saturating_add(preview.bytes);
                                }
                                out.candidates.push(UnifiedCleanupCandidate {
                                    id: summary.session_key,
                                    title: summary.title,
                                    estimated_bytes: preview.bytes,
                                    blocked_reason,
                                });
                            }
                            Err(err) => {
                                let bytes = summary
                                    .storage_size
                                    .as_ref()
                                    .and_then(|size| size.bytes)
                                    .unwrap_or(0);
                                out.candidates.push(UnifiedCleanupCandidate {
                                    id: summary.session_key,
                                    title: summary.title,
                                    estimated_bytes: bytes,
                                    blocked_reason: Some(err.to_string()),
                                });
                            }
                        }
                    }
                    out.candidate_count = out.candidates.len();
                }
                Err(err) => out.issues.push(err.to_string()),
            }
        }
        SCOPE_DSH_ARCHIVED => match crate::dsh::preview_with_bytes(&paths.dsh) {
            Ok((plan, sizes)) => {
                out.issues = plan.issues;
                for c in plan.candidates {
                    let bytes = sizes.get(&c.id).copied().unwrap_or(0);
                    if c.reason.is_none() {
                        out.deletable_count += 1;
                        out.estimated_bytes = out.estimated_bytes.saturating_add(bytes);
                    }
                    let title = match &c.parent_id {
                        Some(parent) => format!("{} (子会话 ← {parent})", c.id),
                        None => c.id.clone(),
                    };
                    out.candidates.push(UnifiedCleanupCandidate {
                        id: c.id,
                        title,
                        estimated_bytes: bytes,
                        blocked_reason: c.reason,
                    });
                }
                out.candidate_count = out.candidates.len();
            }
            Err(err) => out.issues.push(err.to_string()),
        },
        SCOPE_CLAUDE_EMPTY => match crate::claude::find_empty_inactive_sessions(&paths.claude) {
            Ok(items) => {
                for (summary, _fp, bytes) in items {
                    out.estimated_bytes = out.estimated_bytes.saturating_add(bytes);
                    out.candidates.push(UnifiedCleanupCandidate {
                        id: summary.session_key,
                        title: summary.title,
                        estimated_bytes: bytes,
                        blocked_reason: None,
                    });
                }
                out.candidate_count = out.candidates.len();
                out.deletable_count = out.candidate_count;
            }
            Err(err) => out.issues.push(err.to_string()),
        },
        SCOPE_GROK_EMPTY => match crate::grok::find_empty_inactive_sessions(&paths.grok) {
            Ok(items) => {
                for (summary, bytes) in items {
                    out.estimated_bytes = out.estimated_bytes.saturating_add(bytes);
                    out.candidates.push(UnifiedCleanupCandidate {
                        id: summary.id,
                        title: summary.title,
                        estimated_bytes: bytes,
                        blocked_reason: None,
                    });
                }
                out.candidate_count = out.candidates.len();
                out.deletable_count = out.candidate_count;
            }
            Err(err) => out.issues.push(err.to_string()),
        },
        _ => anyhow::bail!("未知的清理范围: {scope}"),
    }
    Ok(out)
}

pub fn dry_run_unified_with_capability(
    platform: &PlatformPaths,
    paths: &UnifiedCleanupPaths,
    request: UnifiedCleanupDryRunRequest,
) -> Result<UnifiedCleanupPlan> {
    require_capability(platform, Capability::ThreadCleanup)?;
    let requested: Vec<&str> = if request.scopes.is_empty() {
        ALL_UNIFIED_SCOPES.to_vec()
    } else {
        for s in &request.scopes {
            scope_meta(s)?;
        }
        ALL_UNIFIED_SCOPES
            .iter()
            .copied()
            .filter(|s| request.scopes.iter().any(|req| req == s))
            .collect()
    };
    let explicit_hidden = request.scopes.iter().any(|s| s == SCOPE_CODEX_HIDDEN);
    let scan_hidden = request.include_codex_hidden || explicit_hidden;
    let mut plan = UnifiedCleanupPlan::default();
    for scope in requested {
        let item = dry_run_single_scope(platform, paths, scope, scan_hidden)?;
        plan.total_candidates += item.candidate_count;
        plan.total_deletable += item.deletable_count;
        plan.total_estimated_bytes = plan
            .total_estimated_bytes
            .saturating_add(item.estimated_bytes);
        plan.scopes.push(item);
    }
    Ok(plan)
}

pub fn execute_unified_with_capability(
    platform: &PlatformPaths,
    paths: &UnifiedCleanupPaths,
    request: UnifiedCleanupExecuteRequest,
) -> Result<UnifiedCleanupResult> {
    require_capability(platform, Capability::ThreadCleanup)?;
    anyhow::ensure!(request.confirmed, UNIFIED_CLEANUP_CONFIRMATION_MESSAGE);
    anyhow::ensure!(!request.scopes.is_empty(), "请至少选择一个待清理平台范围");
    let mut ordered_scopes = Vec::new();
    for s in &request.scopes {
        scope_meta(s)?;
    }
    for &known in ALL_UNIFIED_SCOPES {
        if request.scopes.iter().any(|s| s == known) {
            ordered_scopes.push(known);
        }
    }

    let mut total = UnifiedCleanupResult::default();
    for scope in ordered_scopes {
        let (provider, label) = scope_meta(scope)?;
        let mut scope_res = UnifiedCleanupScopeResult {
            scope: scope.to_owned(),
            provider: provider.to_owned(),
            label: label.to_owned(),
            ..Default::default()
        };
        if total.stopped_early {
            scope_res
                .issues
                .push("前序清理步骤出现失败或残留，已停止后续清理".into());
            total.scopes.push(scope_res);
            continue;
        }
        match scope {
            SCOPE_CODEX_ARCHIVED => {
                let recomputed = archive::plan_delete_archived(&paths.codex)?;
                if !recomputed.archived_ids.is_empty() {
                    let mut bytes_by_id = std::collections::BTreeMap::new();
                    let selections: Vec<_> = recomputed
                        .archived_ids
                        .iter()
                        .map(|id| {
                            let bytes =
                                crate::selected_codex::tree::preview(&paths.codex, id, false)
                                    .map(|p| p.selected.bytes)
                                    .unwrap_or(0);
                            bytes_by_id.insert(id.clone(), bytes);
                            archive::HiddenThreadSelection {
                                id: id.clone(),
                                fingerprint: None,
                            }
                        })
                        .collect();
                    let res = archive::execute_delete_archived(&paths.codex, &selections)?;
                    for item in res.items {
                        match item.status.as_str() {
                            "deleted" => {
                                scope_res.deleted_count += 1;
                                scope_res.deleted_bytes = scope_res.deleted_bytes.saturating_add(
                                    bytes_by_id.get(&item.id).copied().unwrap_or(0),
                                );
                            }
                            "skipped" => {
                                scope_res.skipped_count += 1;
                                if let Some(r) = item.reason {
                                    scope_res.issues.push(format!("{}: {r}", item.id));
                                }
                            }
                            _ => {
                                scope_res.failed_count += 1;
                                if let Some(r) = item.reason {
                                    scope_res.issues.push(format!("{}: {r}", item.id));
                                }
                                scope_res.residuals.extend(item.residuals);
                            }
                        }
                    }
                    if scope_res.failed_count > 0 || !scope_res.residuals.is_empty() {
                        total.stopped_early = true;
                    }
                }
            }
            SCOPE_CODEX_HIDDEN => {
                let recomputed = archive::plan_delete_hidden(&paths.codex)?;
                let mut bytes_by_id = std::collections::BTreeMap::new();
                let mut selections = Vec::new();
                for c in recomputed.candidates {
                    if !c.allowed {
                        scope_res.skipped_count += 1;
                        if let Some(r) = c.reason {
                            scope_res.issues.push(format!("{}: {r}", c.id));
                        }
                        continue;
                    }
                    let bytes = crate::selected_codex::tree::preview(&paths.codex, &c.id, false)
                        .map(|p| p.selected.bytes)
                        .unwrap_or(0);
                    bytes_by_id.insert(c.id.clone(), bytes);
                    selections.push(archive::HiddenThreadSelection {
                        id: c.id,
                        fingerprint: c.fingerprint,
                    });
                }
                if !selections.is_empty() {
                    let res = archive::execute_delete_hidden(&paths.codex, &selections)?;
                    for item in res.items {
                        match item.status.as_str() {
                            "deleted" => {
                                scope_res.deleted_count += 1;
                                scope_res.deleted_bytes = scope_res.deleted_bytes.saturating_add(
                                    bytes_by_id.get(&item.id).copied().unwrap_or(0),
                                );
                            }
                            "skipped" => {
                                scope_res.skipped_count += 1;
                                if let Some(r) = item.reason {
                                    scope_res.issues.push(format!("{}: {r}", item.id));
                                }
                            }
                            _ => {
                                scope_res.failed_count += 1;
                                if let Some(r) = item.reason {
                                    scope_res.issues.push(format!("{}: {r}", item.id));
                                }
                                scope_res.residuals.extend(item.residuals);
                            }
                        }
                    }
                    if scope_res.failed_count > 0 || !scope_res.residuals.is_empty() {
                        total.stopped_early = true;
                    }
                }
            }
            SCOPE_ANTIGRAVITY_ARCHIVED => {
                if platform.kind != crate::platform::PlatformKind::Macos {
                    scope_res
                        .issues
                        .push("仅本机 macOS 支持 Antigravity 会话清理".into());
                } else {
                    let sessions = crate::antigravity::sessions::list_sessions(
                        &paths.antigravity,
                        &crate::provider_sessions::ProviderListRequest {
                            archived: Some(true),
                            limit: Some(1000),
                            q: None,
                        },
                    )?;
                    for summary in sessions.into_iter().filter(|s| s.archived) {
                        let preview = match crate::antigravity::sessions::preview_batch_item(
                            &paths.antigravity,
                            &summary.session_key,
                            crate::services::sessions::SessionOperation::Delete,
                            true,
                        ) {
                            Ok(p) if p.allowed => p,
                            Ok(p) => {
                                scope_res.skipped_count += 1;
                                if let Some(r) = p.reason {
                                    scope_res
                                        .issues
                                        .push(format!("{}: {r}", summary.session_key));
                                }
                                continue;
                            }
                            Err(err) => {
                                scope_res.skipped_count += 1;
                                scope_res
                                    .issues
                                    .push(format!("{}: {err}", summary.session_key));
                                continue;
                            }
                        };
                        let current = crate::antigravity::sessions::session_summary(
                            &paths.antigravity,
                            &summary.session_key,
                        )?;
                        if !current.archived {
                            scope_res.skipped_count += 1;
                            scope_res
                                .issues
                                .push(format!("{}: 会话已恢复为非归档状态", summary.session_key));
                            continue;
                        }
                        match crate::antigravity::sessions::execute_batch_item(
                            &paths.antigravity,
                            &summary.session_key,
                            crate::services::sessions::SessionOperation::Delete,
                            &preview.descendants,
                            true,
                        ) {
                            Ok(item) if item.status == "succeeded" && item.residuals.is_empty() => {
                                scope_res.deleted_count += 1;
                                scope_res.deleted_bytes =
                                    scope_res.deleted_bytes.saturating_add(preview.bytes);
                            }
                            Ok(item) => {
                                scope_res.failed_count += 1;
                                if let Some(msg) = item.message {
                                    scope_res
                                        .issues
                                        .push(format!("{}: {msg}", summary.session_key));
                                }
                                scope_res.residuals.extend(item.residuals);
                                total.stopped_early = true;
                                break;
                            }
                            Err(err) => {
                                scope_res.failed_count += 1;
                                scope_res
                                    .issues
                                    .push(format!("{}: {err}", summary.session_key));
                                total.stopped_early = true;
                                break;
                            }
                        }
                    }
                }
            }
            SCOPE_DSH_ARCHIVED => {
                let recomputed = crate::dsh::preview(&paths.dsh)?;
                scope_res.issues.extend(recomputed.issues);
                let mut targets = Vec::new();
                for c in recomputed.candidates {
                    if let Some(reason) = c.reason {
                        scope_res.skipped_count += 1;
                        scope_res.issues.push(format!("{}: {reason}", c.id));
                    } else {
                        targets.push(crate::dsh::DshArchiveSelection {
                            id: c.id,
                            archive_id: c.archive_id,
                        });
                    }
                }
                if !targets.is_empty() {
                    match crate::dsh::execute(
                        &paths.dsh,
                        crate::dsh::DshArchiveDeleteRequest {
                            confirmed: true,
                            targets,
                        },
                    ) {
                        Ok(res) => {
                            scope_res.deleted_bytes = res.deleted_bytes;
                            for item in res.items {
                                match item.status.as_str() {
                                    "deleted" => scope_res.deleted_count += 1,
                                    "skipped" => {
                                        scope_res.skipped_count += 1;
                                        if let Some(r) = item.reason {
                                            scope_res.issues.push(format!("{}: {r}", item.id));
                                        }
                                    }
                                    _ => {
                                        scope_res.failed_count += 1;
                                        if let Some(r) = item.reason {
                                            scope_res.issues.push(format!("{}: {r}", item.id));
                                        }
                                        scope_res.residuals.extend(item.residuals);
                                    }
                                }
                            }
                            if scope_res.failed_count > 0 || !scope_res.residuals.is_empty() {
                                total.stopped_early = true;
                            }
                        }
                        Err(err) => {
                            scope_res.failed_count += 1;
                            scope_res.issues.push(err.to_string());
                            total.stopped_early = true;
                        }
                    }
                }
            }
            SCOPE_CLAUDE_EMPTY => {
                let empty = crate::claude::find_empty_inactive_sessions(&paths.claude)?;
                for (summary, fp, _bytes) in empty {
                    match crate::claude::execute_claude_delete(
                        &paths.claude,
                        crate::claude::ClaudeDeleteRequest {
                            session_key: summary.session_key.clone(),
                            confirmed: true,
                            fingerprint: fp,
                        },
                    ) {
                        Ok(res) if res.deleted && res.residuals.is_empty() => {
                            scope_res.deleted_count += 1;
                            scope_res.deleted_bytes =
                                scope_res.deleted_bytes.saturating_add(res.bytes);
                        }
                        Ok(res) => {
                            scope_res.failed_count += 1;
                            scope_res.residuals.extend(res.residuals);
                            total.stopped_early = true;
                            break;
                        }
                        Err(err) => {
                            scope_res.failed_count += 1;
                            scope_res
                                .issues
                                .push(format!("{}: {err}", summary.session_key));
                            total.stopped_early = true;
                            break;
                        }
                    }
                }
            }
            SCOPE_GROK_EMPTY => {
                let empty = crate::grok::find_empty_inactive_sessions(&paths.grok)?;
                for (summary, _bytes) in empty {
                    match crate::grok::execute_grok_delete(
                        &paths.grok,
                        crate::grok::GrokDeleteRequest {
                            id: summary.id.clone(),
                            confirmed: true,
                            fingerprint: String::new(),
                        },
                    ) {
                        Ok(res) if res.deleted && res.residuals.is_empty() => {
                            scope_res.deleted_count += 1;
                            scope_res.deleted_bytes =
                                scope_res.deleted_bytes.saturating_add(res.bytes);
                        }
                        Ok(res) => {
                            scope_res.failed_count += 1;
                            scope_res.residuals.extend(res.residuals);
                            total.stopped_early = true;
                            break;
                        }
                        Err(err) => {
                            scope_res.failed_count += 1;
                            scope_res.issues.push(format!("{}: {err}", summary.id));
                            total.stopped_early = true;
                            break;
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
        scope_res.residuals.sort();
        scope_res.residuals.dedup();
        total.deleted_count += scope_res.deleted_count;
        total.skipped_count += scope_res.skipped_count;
        total.failed_count += scope_res.failed_count;
        total.deleted_bytes = total.deleted_bytes.saturating_add(scope_res.deleted_bytes);
        total.scopes.push(scope_res);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use crate::{
        archive::HiddenThreadSelection,
        platform::{PlatformKind, PlatformPaths},
        services::{
            cleanup::{
                cleanup_confirmation_message, plan_cleanup_execute_operation,
                plan_cleanup_operation, CleanupAction, CleanupExecuteRequest, CleanupOperationKind,
                CleanupTarget, HIDDEN_DELETE_CONFIRMATION_MESSAGE,
            },
            commands,
            system::Capability,
        },
    };

    #[test]
    fn cleanup_operation_plan_keeps_dry_run_and_confirmed_execute_boundaries_shared() {
        let linux = PlatformPaths::for_kind(PlatformKind::Linux);
        let macos = PlatformPaths::for_kind(PlatformKind::Macos);
        let windows = PlatformPaths::for_kind(PlatformKind::Windows);

        let dry_run = plan_cleanup_operation(
            &linux,
            CleanupTarget::Archived,
            CleanupOperationKind::DryRun,
        )
        .unwrap();
        assert_eq!(dry_run.required_capability, Capability::ThreadCleanup);
        assert_eq!(dry_run.target, CleanupTarget::Archived);
        assert_eq!(dry_run.action, CleanupAction::ArchiveDeleteDryRun);
        assert_eq!(dry_run.command, commands::CLEANUP_ARCHIVE_DRY_RUN);
        assert!(!dry_run.execute);
        assert!(!dry_run.requires_confirmation);
        assert!(!dry_run.requires_prior_dry_run);
        assert!(!dry_run.requires_expected_count);
        assert_eq!(dry_run.confirmation_message, None);

        let unconfirmed_execute =
            plan_cleanup_operation(&macos, CleanupTarget::Hidden, CleanupOperationKind::Execute)
                .expect_err("execute cleanup operation must require explicit confirmation");
        assert!(
            unconfirmed_execute
                .to_string()
                .contains(HIDDEN_DELETE_CONFIRMATION_MESSAGE),
            "{unconfirmed_execute}"
        );

        let execute = plan_cleanup_execute_operation(
            &macos,
            CleanupTarget::Hidden,
            CleanupExecuteRequest {
                confirmed: true,
                expected_count: Some(2),
                candidates: Some(vec![
                    HiddenThreadSelection {
                        id: "candidate-one".into(),
                        fingerprint: Some("fingerprint-one".into()),
                    },
                    HiddenThreadSelection {
                        id: "candidate-two".into(),
                        fingerprint: Some("fingerprint-two".into()),
                    },
                ]),
            },
        )
        .unwrap();
        assert_eq!(execute.target, CleanupTarget::Hidden);
        assert_eq!(execute.action, CleanupAction::HiddenDeleteExecute);
        assert_eq!(execute.command, commands::CLEANUP_HIDDEN_EXECUTE);
        assert!(execute.execute);
        assert!(execute.requires_confirmation);
        assert!(execute.requires_prior_dry_run);
        assert!(!execute.requires_expected_count);
        assert_eq!(
            execute.confirmation_message.as_deref(),
            Some(HIDDEN_DELETE_CONFIRMATION_MESSAGE)
        );
        assert!(execute.confirmation.confirmed);
        assert_eq!(execute.confirmation.expected_count, Some(2));
        assert_eq!(
            execute.confirmation.payload["candidates"]
                .as_array()
                .unwrap()
                .len(),
            2
        );

        let archived_execute = plan_cleanup_execute_operation(
            &macos,
            CleanupTarget::Archived,
            CleanupExecuteRequest {
                confirmed: true,
                expected_count: Some(2),
                candidates: Some(Vec::new()),
            },
        )
        .unwrap();
        assert!(!archived_execute.requires_expected_count);
        plan_cleanup_execute_operation(
            &macos,
            CleanupTarget::Hidden,
            CleanupExecuteRequest {
                confirmed: true,
                expected_count: None,
                candidates: Some(Vec::new()),
            },
        )
        .unwrap();
        assert_eq!(
            cleanup_confirmation_message(CleanupTarget::Hidden),
            HIDDEN_DELETE_CONFIRMATION_MESSAGE
        );

        let missing_candidates = plan_cleanup_execute_operation(
            &macos,
            CleanupTarget::Hidden,
            CleanupExecuteRequest {
                confirmed: true,
                expected_count: Some(0),
                candidates: None,
            },
        )
        .expect_err("hidden cleanup must be bound to a preview candidate list");
        assert!(missing_candidates
            .to_string()
            .contains("清理需要明确的线程列表"));

        plan_cleanup_execute_operation(
            &macos,
            CleanupTarget::Hidden,
            CleanupExecuteRequest {
                confirmed: true,
                expected_count: Some(1),
                candidates: Some(Vec::new()),
            },
        )
        .unwrap();

        assert!(plan_cleanup_operation(
            &windows,
            CleanupTarget::Archived,
            CleanupOperationKind::DryRun
        )
        .is_err());
    }

    #[test]
    fn unified_cleanup_recomputes_targets_preserves_non_empty_and_stops_on_first_failure() {
        use std::fs;
        let tmp =
            std::env::temp_dir().join(format!("nexushub-unified-cleanup-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&tmp).unwrap();

        let codex = crate::codex::CodexPaths::new(tmp.join("codex"));
        fs::create_dir_all(&codex.home).unwrap();
        let antigravity = crate::antigravity::sessions::AntigravityPaths {
            home: tmp.join("antigravity"),
            rpc_mode: crate::antigravity::sessions::RpcMode::OfflineOnly,
        };
        fs::create_dir_all(antigravity.conversations_dir()).unwrap();
        fs::create_dir_all(antigravity.annotations_dir()).unwrap();
        let dsh = crate::dsh::DshPaths {
            home: tmp.join("dsh"),
        };
        fs::create_dir_all(&dsh.home).unwrap();
        let claude = crate::claude::ClaudePaths {
            config: tmp.join("claude"),
            projects: tmp.join("claude/projects"),
        };
        fs::create_dir_all(claude.projects.join("proj")).unwrap();
        let grok = crate::grok::GrokPaths {
            home: tmp.join("grok"),
        };
        fs::create_dir_all(grok.sessions().join("ws")).unwrap();

        // Seed 1 empty Claude session + 1 non-empty Claude session
        let claude_empty_id = "11111111-1111-4111-8111-111111111111";
        let claude_active_id = "22222222-2222-4222-8222-222222222222";
        let claude_empty_file = claude
            .projects
            .join("proj")
            .join(format!("{claude_empty_id}.jsonl"));
        let claude_active_file = claude
            .projects
            .join("proj")
            .join(format!("{claude_active_id}.jsonl"));
        fs::write(&claude_empty_file, "").unwrap();
        fs::write(
            &claude_active_file,
            concat!(
                "{\"type\":\"user\",\"uuid\":\"u1\",\"sessionId\":\"22222222-2222-4222-8222-222222222222\",\"timestamp\":\"2026-10-29T00:00:00Z\",\"message\":{\"role\":\"user\",\"content\":\"hello\"}}\n",
                "{\"type\":\"assistant\",\"uuid\":\"a1\",\"parentUuid\":\"u1\",\"sessionId\":\"22222222-2222-4222-8222-222222222222\",\"timestamp\":\"2026-10-29T00:00:01Z\",\"message\":{\"role\":\"assistant\",\"stop_reason\":\"end_turn\",\"content\":[{\"type\":\"text\",\"text\":\"world\"}]}}\n"
            ),
        )
        .unwrap();

        // Seed 1 empty Grok session + 1 non-empty Grok session
        let grok_empty_id = "33333333-3333-4333-8333-333333333333";
        let grok_active_id = "44444444-4444-4444-8444-444444444444";
        let grok_empty_dir = grok.sessions().join("ws").join(grok_empty_id);
        let grok_active_dir = grok.sessions().join("ws").join(grok_active_id);
        fs::create_dir_all(&grok_empty_dir).unwrap();
        fs::create_dir_all(&grok_active_dir).unwrap();
        fs::write(
            grok_empty_dir.join("summary.json"),
            serde_json::json!({"info":{"id":grok_empty_id,"cwd":"/work"},"generated_title":"Empty Grok","num_messages":0}).to_string(),
        )
        .unwrap();
        fs::write(grok_empty_dir.join("updates.jsonl"), "").unwrap();
        fs::write(
            grok_active_dir.join("summary.json"),
            serde_json::json!({"info":{"id":grok_active_id,"cwd":"/work"},"generated_title":"Active Grok","num_messages":2}).to_string(),
        )
        .unwrap();
        fs::write(
            grok_active_dir.join("updates.jsonl"),
            "{\"timestamp\":\"2026-10-29T00:00:00Z\",\"sessionUpdate\":\"user_message_chunk\",\"messageId\":\"m1\",\"content\":{\"text\":\"hi\"}}\n",
        )
        .unwrap();

        let paths = super::UnifiedCleanupPaths {
            codex,
            antigravity,
            dsh,
            claude,
            grok,
        };
        let linux = PlatformPaths::for_kind(PlatformKind::Linux);
        let macos = PlatformPaths::for_kind(PlatformKind::Macos);

        // Linux marks Antigravity scope unsupported; macOS marks it supported.
        let linux_plan = super::dry_run_unified_with_capability(
            &linux,
            &paths,
            super::UnifiedCleanupDryRunRequest {
                scopes: Vec::new(),
                include_codex_hidden: true,
            },
        )
        .unwrap();
        let ag_linux = linux_plan
            .scopes
            .iter()
            .find(|s| s.scope == super::SCOPE_ANTIGRAVITY_ARCHIVED)
            .unwrap();
        assert!(!ag_linux.supported);

        let mac_plan = super::dry_run_unified_with_capability(
            &macos,
            &paths,
            super::UnifiedCleanupDryRunRequest {
                scopes: Vec::new(),
                include_codex_hidden: true,
            },
        )
        .unwrap();
        assert_eq!(mac_plan.total_deletable, 2);

        // Unconfirmed execute must fail closed.
        assert!(super::execute_unified_with_capability(
            &macos,
            &paths,
            super::UnifiedCleanupExecuteRequest {
                confirmed: false,
                scopes: vec![super::SCOPE_CLAUDE_EMPTY.into()],
            },
        )
        .is_err());

        // Stop-on-first-failure: locking Claude's project dir stops before Grok empty is deleted!
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let claude_proj = paths.claude.projects.join("proj");
            fs::set_permissions(&claude_proj, fs::Permissions::from_mode(0o555)).unwrap();
            let stopped = super::execute_unified_with_capability(
                &macos,
                &paths,
                super::UnifiedCleanupExecuteRequest {
                    confirmed: true,
                    scopes: vec![
                        super::SCOPE_CLAUDE_EMPTY.into(),
                        super::SCOPE_GROK_EMPTY.into(),
                    ],
                },
            )
            .unwrap();
            fs::set_permissions(&claude_proj, fs::Permissions::from_mode(0o755)).unwrap();
            assert!(stopped.stopped_early);
            assert_eq!(stopped.failed_count, 1);
            assert!(
                grok_empty_dir.exists(),
                "subsequent scope must not delete after earlier scope failure"
            );
        }

        // Normal confirmed execution cleans empty Claude & Grok while preserving non-empty sessions.
        let ok = super::execute_unified_with_capability(
            &macos,
            &paths,
            super::UnifiedCleanupExecuteRequest {
                confirmed: true,
                scopes: vec![
                    super::SCOPE_CLAUDE_EMPTY.into(),
                    super::SCOPE_GROK_EMPTY.into(),
                ],
            },
        )
        .unwrap();
        assert!(!ok.stopped_early);
        assert_eq!(ok.deleted_count, 2);
        assert!(!claude_empty_file.exists());
        assert!(!grok_empty_dir.exists());
        assert!(claude_active_file.exists());
        assert!(grok_active_dir.exists());

        let _ = fs::remove_dir_all(&tmp);
    }
}
