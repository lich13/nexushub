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
