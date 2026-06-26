use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    codex::{ThreadDetail, ThreadStatus},
    config::{Config, SecurityConfig},
    db::{JobRecord, PanelDb, ThreadFollowUp, ThreadGoal},
    platform::PlatformPaths,
    services::{
        cleanup::{
            self, CleanupAction, CleanupActionPlan, CleanupExecuteRequest, CleanupOperationKind,
            CleanupOperationPlan, CleanupTarget,
        },
        goals::{
            self, GoalCommandFacadePlan, GoalGetPlan, GoalGetRequest, GoalUpdateRequest, GoalView,
        },
        jobs::{
            self, ActionResponse, FollowUpAutoSubmitExecutionPlan, FollowUpCancelPlan,
            FollowUpCancelRequest, FollowUpClaimPlan, FollowUpClaimRequest,
            FollowUpEnqueueFacadePlan, FollowUpErrorPlan, FollowUpErrorRequest, FollowUpListPlan,
            FollowUpListRequest, FollowUpSubmitPlan, FollowUpSubmitResultPlan,
            FollowUpSubmitResultRequest, ThreadCommandExecutionPlan, ThreadCommandFacadePlan,
            ThreadCommandKind, ThreadCommandRequest, ThreadMessageRequest, ThreadRenameRequest,
            ThreadSendRequest, ThreadStateActionPlan, ThreadSteerRequest, ThreadStopJobPlan,
            ThreadStopPlan, ThreadStopRequest,
        },
        probe::{ProbeUseCases, ProbeUseCases as CoreProbeUseCases},
        security::{
            self, PasswordChangeFacadePlan, PasswordChangeRequest, PublicSecurityViewFacadePlan,
            SecurityPatch, SecurityPatchFacadePlan, SecurityView,
        },
        settings::{SettingsUseCases, SettingsUseCases as CoreSettingsUseCases},
        system::{self, Capability, CapabilityGatePlan, SystemCapabilities},
        threads::{
            self, ThreadBlocksPage, ThreadDetailPlan, ThreadDetailReadPlan, ThreadDetailRequest,
            ThreadListPlan, ThreadListReadPlan, ThreadsQuery,
        },
        updates::{UpdateUseCases, UpdateUseCases as CoreUpdateUseCases},
        uploads::{
            self, UploadBatchItem, UploadDeletePlan, UploadFacadePlan, UploadRetentionPlan,
            UploadRetentionRequest, UploadStorePlan, UploadValidationPlan,
        },
    },
    update::{analyze_job_failure, JobFailureAnalysis},
    uploads::UploadOutcome,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobListPlan {
    pub required_capability: Capability,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDetailPlan {
    pub required_capability: Capability,
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobResponse {
    #[serde(flatten)]
    pub job: JobRecord,
    pub failure_analysis: Option<JobFailureAnalysis>,
    pub analysis: Option<String>,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct NexusHubUseCases<'a> {
    config: Option<&'a Config>,
    platform: &'a PlatformPaths,
}

impl<'a> NexusHubUseCases<'a> {
    pub fn new(platform: &'a PlatformPaths) -> Self {
        Self {
            config: None,
            platform,
        }
    }

    pub fn with_config(config: &'a Config, platform: &'a PlatformPaths) -> Self {
        Self {
            config: Some(config),
            platform,
        }
    }

    pub fn threads(self) -> ThreadUseCases<'a> {
        ThreadUseCases {
            platform: self.platform,
        }
    }

    pub fn jobs(self) -> JobUseCases<'a> {
        JobUseCases {
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

    pub fn settings(self) -> Result<SettingsUseCases<'a>> {
        Ok(CoreSettingsUseCases::new(
            self.config_required()?,
            self.platform,
        ))
    }

    pub fn probe(self) -> Result<ProbeUseCases<'a>> {
        Ok(CoreProbeUseCases::new(
            self.config_required()?,
            self.platform,
        ))
    }

    pub fn updates(self) -> Result<UpdateUseCases<'a>> {
        Ok(CoreUpdateUseCases::new(
            self.config_required()?,
            self.platform,
        ))
    }

    pub fn system(self) -> Result<SystemUseCases<'a>> {
        Ok(SystemUseCases {
            config: self.config_required()?,
            platform: self.platform,
        })
    }

    pub fn security(self) -> Result<SecurityUseCases<'a>> {
        Ok(SecurityUseCases {
            config: &self.config_required()?.security,
            platform: self.platform,
        })
    }

    fn config_required(self) -> Result<&'a Config> {
        self.config
            .ok_or_else(|| anyhow::anyhow!("config is required for this NexusHub use case"))
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

    pub fn list_read(self, query: ThreadsQuery) -> Result<ThreadListReadPlan> {
        threads::plan_thread_list_read(self.platform, query)
    }

    pub fn detail(self, request: ThreadDetailRequest) -> Result<ThreadDetailPlan> {
        threads::plan_thread_detail_request(self.platform, request)
    }

    pub fn detail_read(self, request: ThreadDetailRequest) -> Result<ThreadDetailReadPlan> {
        threads::plan_thread_detail_read(self.platform, request)
    }

    pub fn blocks(
        self,
        thread_id: &str,
        limit: Option<usize>,
        before: Option<String>,
    ) -> Result<ThreadDetailPlan> {
        threads::plan_thread_blocks_request(self.platform, thread_id, limit, before)
    }

    pub fn blocks_read(
        self,
        thread_id: &str,
        limit: Option<usize>,
        before: Option<String>,
    ) -> Result<ThreadDetailReadPlan> {
        threads::plan_thread_blocks_read(self.platform, thread_id, limit, before)
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

    pub fn create_job(
        self,
        message: ThreadMessageRequest,
        default_workspace: PathBuf,
    ) -> Result<ThreadCommandExecutionPlan> {
        jobs::plan_thread_command_job_execution(
            self.platform,
            ThreadCommandRequest {
                command: ThreadCommandKind::Create,
                thread_id: None,
                message,
            },
            default_workspace,
        )
    }

    pub fn resume_job(
        self,
        message: ThreadMessageRequest,
        default_workspace: PathBuf,
    ) -> Result<ThreadCommandExecutionPlan> {
        jobs::plan_thread_command_job_execution(
            self.platform,
            ThreadCommandRequest {
                command: ThreadCommandKind::Resume,
                thread_id: message.thread_id.clone(),
                message,
            },
            default_workspace,
        )
    }

    pub fn send(self, request: ThreadSendRequest) -> Result<ThreadCommandFacadePlan> {
        jobs::plan_thread_send_with_capability(self.platform, request)
    }

    pub fn send_job(
        self,
        request: ThreadSendRequest,
        default_workspace: PathBuf,
    ) -> Result<ThreadCommandExecutionPlan> {
        jobs::plan_thread_send_job_execution(self.platform, request, default_workspace)
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

    pub fn list_followups(
        self,
        db: &PanelDb,
        request: FollowUpListRequest,
    ) -> Result<Vec<ThreadFollowUp>> {
        jobs::list_followups_with_capability(db, self.platform, request)
    }

    pub fn pending_followup(self, db: &PanelDb, thread_id: &str) -> Result<Option<ThreadFollowUp>> {
        let plan = self.followups(FollowUpListRequest {
            thread_id: thread_id.to_string(),
            limit: Some(1),
        })?;
        Ok(db
            .list_followups(&plan.thread_id, plan.limit)?
            .into_iter()
            .find(|followup| followup.status == "pending"))
    }

    pub fn enqueue_followup(
        self,
        request: ThreadSteerRequest,
    ) -> Result<FollowUpEnqueueFacadePlan> {
        jobs::plan_followup_enqueue_with_capability(self.platform, request)
    }

    pub fn apply_enqueue_followup(
        self,
        db: &PanelDb,
        request: ThreadSteerRequest,
    ) -> Result<ThreadFollowUp> {
        jobs::enqueue_followup_with_capability(db, self.platform, request)
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

    pub fn apply_cancel_followup(
        self,
        db: &PanelDb,
        request: FollowUpCancelRequest,
    ) -> Result<ActionResponse> {
        jobs::cancel_followup_with_capability(db, self.platform, request)
    }

    pub fn autosubmit_followup_job(
        self,
        thread_status: ThreadStatus,
        followup: &ThreadFollowUp,
        default_workspace: PathBuf,
    ) -> Result<FollowUpAutoSubmitExecutionPlan> {
        jobs::plan_followup_autosubmit_execution(
            self.platform,
            thread_status,
            followup,
            default_workspace,
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct JobUseCases<'a> {
    platform: &'a PlatformPaths,
}

impl<'a> JobUseCases<'a> {
    pub fn list(self, limit: Option<u32>) -> Result<JobListPlan> {
        system::require_capability(self.platform, Capability::JobHistory)?;
        Ok(JobListPlan {
            required_capability: Capability::JobHistory,
            limit: normalize_job_list_limit(limit),
        })
    }

    pub fn detail(self, job_id: &str) -> Result<JobDetailPlan> {
        system::require_capability(self.platform, Capability::JobHistory)?;
        Ok(JobDetailPlan {
            required_capability: Capability::JobHistory,
            job_id: required_job_id(job_id)?,
        })
    }

    pub fn response(self, job: JobRecord) -> JobResponse {
        job_response(job)
    }

    pub fn list_response(self, jobs: Vec<JobRecord>) -> Vec<JobResponse> {
        jobs.into_iter().map(job_response).collect()
    }

    pub fn detail_response(self, job: Option<JobRecord>) -> Option<JobResponse> {
        job.map(job_response)
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

    pub fn delete_execute(self, id: impl AsRef<str>) -> Result<UploadDeletePlan> {
        self.delete(id)
    }

    pub fn execute_delete(self, root: &Path, plan: &UploadDeletePlan) -> Result<bool> {
        uploads::execute_delete_upload_plan(root, plan)
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

    pub fn operation(
        self,
        target: CleanupTarget,
        operation: CleanupOperationKind,
    ) -> Result<CleanupOperationPlan> {
        cleanup::plan_cleanup_operation(self.platform, target, operation)
    }

    pub fn dry_run(self, target: CleanupTarget) -> Result<CleanupOperationPlan> {
        self.operation(target, CleanupOperationKind::DryRun)
    }

    pub fn execute(self, target: CleanupTarget) -> Result<CleanupOperationPlan> {
        self.operation(target, CleanupOperationKind::Execute)
    }

    pub fn execute_confirmed(
        self,
        target: CleanupTarget,
        request: CleanupExecuteRequest,
    ) -> Result<CleanupOperationPlan> {
        cleanup::plan_cleanup_execute_operation(self.platform, target, request)
    }

    pub fn validate_expected_count(
        self,
        plan: &CleanupOperationPlan,
        actual_count: u64,
    ) -> Result<()> {
        cleanup::validate_cleanup_expected_count(plan, actual_count)
    }

    pub fn dry_run_archived(
        self,
        paths: &crate::codex::CodexPaths,
    ) -> Result<cleanup::ArchiveDeletePlan> {
        cleanup::dry_run_archived_with_capability(self.platform, paths)
    }

    pub fn execute_archived(
        self,
        paths: &crate::codex::CodexPaths,
    ) -> Result<cleanup::ArchiveDeleteResult> {
        cleanup::execute_archived_with_capability(self.platform, paths)
    }

    pub fn dry_run_hidden(
        self,
        paths: &crate::codex::CodexPaths,
    ) -> Result<cleanup::HiddenThreadDeletePlan> {
        cleanup::dry_run_hidden_with_capability(self.platform, paths)
    }

    pub fn execute_hidden(
        self,
        paths: &crate::codex::CodexPaths,
    ) -> Result<cleanup::HiddenThreadDeleteResult> {
        cleanup::execute_hidden_with_capability(self.platform, paths)
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

pub fn normalize_job_list_limit(limit: Option<u32>) -> u32 {
    limit.unwrap_or(50).min(200)
}

pub fn required_job_id(value: &str) -> Result<String> {
    value
        .trim()
        .is_empty()
        .then(|| anyhow::anyhow!("job_id is required"))
        .map_or_else(|| Ok(value.trim().to_string()), Err)
}

pub fn job_response(job: JobRecord) -> JobResponse {
    let failure_analysis = if job.status == "failed" {
        analyze_job_failure(&job.kind, &job.output, job.error.as_deref(), job.exit_code)
    } else {
        None
    };
    let analysis = failure_analysis
        .as_ref()
        .map(|analysis| analysis.explanation.clone());
    let explanation = failure_analysis.as_ref().and_then(|analysis| {
        let suggestions = analysis.suggestions.join("\n");
        (!suggestions.is_empty()).then_some(suggestions)
    });
    JobResponse {
        job,
        failure_analysis,
        analysis,
        explanation,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SystemUseCases<'a> {
    config: &'a Config,
    platform: &'a PlatformPaths,
}

impl<'a> SystemUseCases<'a> {
    pub fn capabilities(self) -> SystemCapabilities {
        system::system_capabilities(self.config, self.platform)
    }

    pub fn capability_gate(self, capability: Capability) -> CapabilityGatePlan {
        system::capability_gate_plan(self.platform, capability)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SecurityUseCases<'a> {
    config: &'a SecurityConfig,
    platform: &'a PlatformPaths,
}

impl<'a> SecurityUseCases<'a> {
    pub fn view(
        self,
        settings: crate::db::SecuritySettings,
        stored_expected_hostname: Option<String>,
        stored_expected_action: Option<String>,
    ) -> Result<SecurityView> {
        security::security_view_with_capability(
            self.platform,
            settings,
            self.config,
            stored_expected_hostname,
            stored_expected_action,
        )
    }

    pub fn public_view(
        self,
        settings: crate::db::SecuritySettings,
        stored_turnstile_action: Option<String>,
        admin_configured: bool,
        base_url: Option<String>,
    ) -> Result<PublicSecurityViewFacadePlan> {
        security::public_security_view_with_capability(
            self.platform,
            settings,
            self.config,
            stored_turnstile_action,
            admin_configured,
            base_url,
        )
    }

    pub fn patch(self, patch: SecurityPatch) -> Result<SecurityPatchFacadePlan> {
        security::plan_security_patch_with_capability(self.platform, patch)
    }

    pub fn change_password(
        self,
        request: PasswordChangeRequest,
        current_password_matches: bool,
    ) -> Result<PasswordChangeFacadePlan> {
        security::plan_password_change_with_capability(
            self.platform,
            request,
            current_password_matches,
        )
    }
}
