use std::path::Path;

use anyhow::Result;

use crate::{
    codex::ThreadDetail,
    db::{PanelDb, ThreadFollowUp, ThreadGoal},
    platform::PlatformPaths,
    services::{
        cleanup::{self, CleanupAction, CleanupActionPlan},
        goals::{
            self, GoalCommandFacadePlan, GoalGetPlan, GoalGetRequest, GoalUpdateRequest, GoalView,
        },
        jobs::{
            self, ActionResponse, FollowUpCancelPlan, FollowUpCancelRequest, FollowUpClaimPlan,
            FollowUpClaimRequest, FollowUpEnqueueFacadePlan, FollowUpErrorPlan,
            FollowUpErrorRequest, FollowUpListPlan, FollowUpListRequest, FollowUpSubmitPlan,
            FollowUpSubmitResultPlan, FollowUpSubmitResultRequest, ThreadCommandFacadePlan,
            ThreadCommandKind, ThreadCommandRequest, ThreadMessageRequest, ThreadRenameRequest,
            ThreadSendRequest, ThreadStateActionPlan, ThreadSteerRequest, ThreadStopJobPlan,
            ThreadStopPlan, ThreadStopRequest,
        },
        threads::{
            self, ThreadBlocksPage, ThreadDetailPlan, ThreadDetailRequest, ThreadListPlan,
            ThreadsQuery,
        },
        uploads::{
            self, UploadBatchItem, UploadDeletePlan, UploadFacadePlan, UploadRetentionPlan,
            UploadRetentionRequest, UploadStorePlan, UploadValidationPlan,
        },
    },
    uploads::UploadOutcome,
};

#[derive(Debug, Clone, Copy)]
pub struct NexusHubUseCases<'a> {
    platform: &'a PlatformPaths,
}

impl<'a> NexusHubUseCases<'a> {
    pub fn new(platform: &'a PlatformPaths) -> Self {
        Self { platform }
    }

    pub fn threads(self) -> ThreadUseCases<'a> {
        ThreadUseCases {
            platform: self.platform,
        }
    }

    pub fn goals(self) -> GoalUseCases<'a> {
        GoalUseCases {
            platform: self.platform,
        }
    }

    pub fn uploads(self) -> UploadUseCases<'a> {
        UploadUseCases {
            platform: self.platform,
        }
    }

    pub fn cleanup(self) -> CleanupUseCases<'a> {
        CleanupUseCases {
            platform: self.platform,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ThreadUseCases<'a> {
    platform: &'a PlatformPaths,
}

impl<'a> ThreadUseCases<'a> {
    pub fn list(self, query: ThreadsQuery) -> Result<ThreadListPlan> {
        threads::plan_threads_list_request(self.platform, query)
    }

    pub fn detail(self, request: ThreadDetailRequest) -> Result<ThreadDetailPlan> {
        threads::plan_thread_detail_request(self.platform, request)
    }

    pub fn blocks(
        self,
        thread_id: &str,
        limit: Option<usize>,
        before: Option<String>,
    ) -> Result<ThreadDetailPlan> {
        threads::plan_thread_blocks_request(self.platform, thread_id, limit, before)
    }

    pub fn blocks_page(self, detail: ThreadDetail, plan: &ThreadDetailPlan) -> ThreadBlocksPage {
        threads::thread_blocks_page_for_plan(detail, plan)
    }

    pub fn create(self, message: ThreadMessageRequest) -> Result<ThreadCommandFacadePlan> {
        jobs::plan_thread_command_with_capability(
            self.platform,
            ThreadCommandRequest {
                command: ThreadCommandKind::Create,
                thread_id: None,
                message,
            },
        )
    }

    pub fn send(self, request: ThreadSendRequest) -> Result<ThreadCommandFacadePlan> {
        jobs::plan_thread_send_with_capability(self.platform, request)
    }

    pub fn steer(self, request: ThreadSteerRequest) -> Result<ThreadCommandFacadePlan> {
        jobs::plan_thread_steer_with_capability(self.platform, request)
    }

    pub fn stop(self, request: ThreadStopRequest) -> Result<ThreadStopPlan> {
        jobs::plan_thread_stop_with_capability(self.platform, request)
    }

    pub fn resolve_stop(
        self,
        plan: &ThreadStopPlan,
        active_job_id: Option<String>,
    ) -> Result<ThreadStopJobPlan> {
        jobs::resolve_thread_stop_job(plan, active_job_id)
    }

    pub fn archive(self, thread_id: &str) -> Result<ThreadStateActionPlan> {
        jobs::plan_thread_archive_with_capability(self.platform, thread_id)
    }

    pub fn restore(self, thread_id: &str) -> Result<ThreadStateActionPlan> {
        jobs::plan_thread_restore_with_capability(self.platform, thread_id)
    }

    pub fn rename(self, request: ThreadRenameRequest) -> Result<ThreadStateActionPlan> {
        jobs::plan_thread_rename_with_capability(self.platform, request)
    }

    pub fn followups(self, request: FollowUpListRequest) -> Result<FollowUpListPlan> {
        jobs::plan_followup_list_with_capability(self.platform, request)
    }

    pub fn enqueue_followup(
        self,
        request: ThreadSteerRequest,
    ) -> Result<FollowUpEnqueueFacadePlan> {
        jobs::plan_followup_enqueue_with_capability(self.platform, request)
    }

    pub fn claim_followup(self, request: FollowUpClaimRequest) -> Result<FollowUpClaimPlan> {
        jobs::plan_followup_claim_with_capability(self.platform, request)
    }

    pub fn claim_next_followup(
        self,
        db: &PanelDb,
        request: FollowUpClaimRequest,
    ) -> Result<Option<ThreadFollowUp>> {
        jobs::claim_next_followup_with_capability(db, self.platform, request)
    }

    pub fn submit_followup(self, followup: &ThreadFollowUp) -> Result<FollowUpSubmitPlan> {
        jobs::plan_followup_submit_with_capability(self.platform, followup)
    }

    pub fn mark_followup_submitted(
        self,
        request: FollowUpSubmitResultRequest,
    ) -> Result<FollowUpSubmitResultPlan> {
        jobs::plan_followup_submitted_with_capability(self.platform, request)
    }

    pub fn apply_followup_submitted(
        self,
        db: &PanelDb,
        request: FollowUpSubmitResultRequest,
    ) -> Result<ActionResponse> {
        jobs::mark_followup_submitted_with_capability(db, self.platform, request)
    }

    pub fn mark_followup_error(self, request: FollowUpErrorRequest) -> Result<FollowUpErrorPlan> {
        jobs::plan_followup_error_with_capability(self.platform, request)
    }

    pub fn apply_followup_error(
        self,
        db: &PanelDb,
        request: FollowUpErrorRequest,
    ) -> Result<ActionResponse> {
        jobs::mark_followup_error_with_capability(db, self.platform, request)
    }

    pub fn cancel_followup(self, request: FollowUpCancelRequest) -> Result<FollowUpCancelPlan> {
        jobs::plan_followup_cancel_with_capability(self.platform, request)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GoalUseCases<'a> {
    platform: &'a PlatformPaths,
}

impl<'a> GoalUseCases<'a> {
    pub fn get(self, request: GoalGetRequest) -> Result<GoalGetPlan> {
        goals::plan_goal_get_with_capability(self.platform, request)
    }

    pub fn save(self, request: GoalUpdateRequest) -> Result<GoalCommandFacadePlan> {
        goals::plan_goal_save_with_capability(self.platform, request)
    }

    pub fn clear(self, thread_id: Option<&str>) -> Result<GoalCommandFacadePlan> {
        goals::plan_goal_clear_with_capability(self.platform, thread_id)
    }

    pub fn pause(
        self,
        thread_id: &str,
        existing: Option<&ThreadGoal>,
    ) -> Result<GoalCommandFacadePlan> {
        goals::plan_goal_pause_with_capability(self.platform, thread_id, existing)
    }

    pub fn resume(
        self,
        thread_id: &str,
        existing: Option<&ThreadGoal>,
    ) -> Result<GoalCommandFacadePlan> {
        goals::plan_goal_resume_with_capability(self.platform, thread_id, existing)
    }

    pub fn apply(self, db: &PanelDb, command: goals::GoalCommandPlan) -> Result<GoalView> {
        goals::apply_goal_command(db, command)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UploadUseCases<'a> {
    platform: &'a PlatformPaths,
}

impl<'a> UploadUseCases<'a> {
    pub fn validate(self, items: &[UploadBatchItem]) -> Result<UploadValidationPlan> {
        uploads::plan_upload_validation_with_capability(self.platform, items)
    }

    pub fn store(self, items: Vec<UploadBatchItem>) -> Result<UploadFacadePlan> {
        uploads::plan_store_uploads_with_capability(self.platform, items)
    }

    pub fn store_to_root(self, root: &Path, plan: UploadStorePlan) -> Result<UploadOutcome> {
        uploads::store_upload_plan(root, plan)
    }

    pub fn delete(self, id: impl AsRef<str>) -> Result<UploadDeletePlan> {
        uploads::plan_delete_upload_with_capability(self.platform, id)
    }

    pub fn retention(self, request: UploadRetentionRequest) -> Result<UploadRetentionPlan> {
        uploads::plan_upload_retention_with_capability(self.platform, request)
    }

    pub fn execute_retention(self, root: &Path, plan: &UploadRetentionPlan) -> Result<usize> {
        uploads::execute_upload_retention_plan(root, plan)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CleanupUseCases<'a> {
    platform: &'a PlatformPaths,
}

impl<'a> CleanupUseCases<'a> {
    pub fn action(self, action: CleanupAction) -> Result<CleanupActionPlan> {
        cleanup::plan_cleanup_action(self.platform, action)
    }

    pub fn archive_delete_dry_run(self) -> Result<CleanupActionPlan> {
        self.action(CleanupAction::ArchiveDeleteDryRun)
    }

    pub fn archive_delete_execute(self) -> Result<CleanupActionPlan> {
        self.action(CleanupAction::ArchiveDeleteExecute)
    }

    pub fn hidden_delete_dry_run(self) -> Result<CleanupActionPlan> {
        self.action(CleanupAction::HiddenDeleteDryRun)
    }

    pub fn hidden_delete_execute(self) -> Result<CleanupActionPlan> {
        self.action(CleanupAction::HiddenDeleteExecute)
    }
}
