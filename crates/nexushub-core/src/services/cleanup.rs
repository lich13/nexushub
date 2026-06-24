use anyhow::Result;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CleanupOperationKind {
    DryRun,
    Execute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    })
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
) -> Result<HiddenThreadDeleteResult> {
    plan_cleanup_action(platform, CleanupAction::HiddenDeleteExecute)?;
    archive::execute_delete_hidden(paths)
}

#[cfg(test)]
mod tests {
    use crate::{
        platform::{PlatformKind, PlatformPaths},
        services::{
            cleanup::{plan_cleanup_operation, CleanupAction, CleanupOperationKind, CleanupTarget},
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

        let execute =
            plan_cleanup_operation(&macos, CleanupTarget::Hidden, CleanupOperationKind::Execute)
                .unwrap();
        assert_eq!(execute.target, CleanupTarget::Hidden);
        assert_eq!(execute.action, CleanupAction::HiddenDeleteExecute);
        assert_eq!(execute.command, commands::CLEANUP_HIDDEN_EXECUTE);
        assert!(execute.execute);
        assert!(execute.requires_confirmation);
        assert!(execute.requires_prior_dry_run);

        assert!(plan_cleanup_operation(
            &windows,
            CleanupTarget::Archived,
            CleanupOperationKind::DryRun
        )
        .is_err());
    }
}
