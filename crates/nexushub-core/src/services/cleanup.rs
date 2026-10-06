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
pub const CLEANUP_EXPECTED_COUNT_REQUIRED_MESSAGE: &str =
    "cleanup expectedCount is required before deletion";

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
        requires_expected_count: action_plan.execute,
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
    if request.expected_count.is_none() {
        anyhow::bail!(CLEANUP_EXPECTED_COUNT_REQUIRED_MESSAGE);
    }
    if target == CleanupTarget::Hidden {
        let candidates = request
            .candidates
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("隐藏清理需要预览候选，请重新扫描"))?;
        anyhow::ensure!(
            request.expected_count == Some(candidates.len() as u64),
            "预览候选数与确认数不一致"
        );
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

pub fn validate_cleanup_expected_count(
    plan: &CleanupOperationPlan,
    actual_count: u64,
) -> Result<()> {
    if !plan.requires_expected_count {
        return Ok(());
    }
    let Some(expected_count) = plan.confirmation.expected_count else {
        anyhow::bail!(CLEANUP_EXPECTED_COUNT_REQUIRED_MESSAGE);
    };
    if expected_count != actual_count {
        anyhow::bail!(
            "cleanup expectedCount mismatch: expected={expected_count} actual={actual_count}"
        );
    }
    Ok(())
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
) -> Result<ArchiveDeleteResult> {
    plan_cleanup_action(platform, CleanupAction::ArchiveDeleteExecute)?;
    archive::execute_delete_archived(paths)
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

#[cfg(test)]
mod tests {
    use crate::{
        archive::HiddenThreadSelection,
        platform::{PlatformKind, PlatformPaths},
        services::{
            cleanup::{
                cleanup_confirmation_message, plan_cleanup_execute_operation,
                plan_cleanup_operation, validate_cleanup_expected_count, CleanupAction,
                CleanupExecuteRequest, CleanupOperationKind, CleanupTarget,
                CLEANUP_EXPECTED_COUNT_REQUIRED_MESSAGE, HIDDEN_DELETE_CONFIRMATION_MESSAGE,
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
        assert!(execute.requires_expected_count);
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
        validate_cleanup_expected_count(&dry_run, 999).unwrap();

        let archived_execute = plan_cleanup_execute_operation(
            &macos,
            CleanupTarget::Archived,
            CleanupExecuteRequest {
                confirmed: true,
                expected_count: Some(2),
                candidates: None,
            },
        )
        .unwrap();
        validate_cleanup_expected_count(&archived_execute, 2).unwrap();
        let mismatch = validate_cleanup_expected_count(&archived_execute, 3)
            .expect_err("archive cleanup must reject stale dry-run counts");
        assert!(mismatch.to_string().contains("expected=2 actual=3"));

        let missing_count = plan_cleanup_execute_operation(
            &macos,
            CleanupTarget::Hidden,
            CleanupExecuteRequest {
                confirmed: true,
                expected_count: None,
                candidates: Some(Vec::new()),
            },
        )
        .expect_err("execute cleanup operation must carry the dry-run count");
        assert!(
            missing_count
                .to_string()
                .contains(CLEANUP_EXPECTED_COUNT_REQUIRED_MESSAGE),
            "{missing_count}"
        );
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
            .contains("隐藏清理需要预览候选"));

        let count_mismatch = plan_cleanup_execute_operation(
            &macos,
            CleanupTarget::Hidden,
            CleanupExecuteRequest {
                confirmed: true,
                expected_count: Some(1),
                candidates: Some(Vec::new()),
            },
        )
        .expect_err("hidden cleanup count must match submitted candidates");
        assert!(count_mismatch.to_string().contains("候选数"));

        assert!(plan_cleanup_operation(
            &windows,
            CleanupTarget::Archived,
            CleanupOperationKind::DryRun
        )
        .is_err());
    }
}
