use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    codex::ThreadDetail,
    config::{Config, SecurityConfig},
    db::JobRecord,
    platform::PlatformPaths,
    services::{
        cleanup::{
            self, CleanupAction, CleanupActionPlan, CleanupExecuteRequest, CleanupOperationKind,
            CleanupOperationPlan, CleanupTarget,
        },
        jobs::{self, ThreadRenameRequest, ThreadStateActionPlan},
        probe::{ProbeUseCases, ProbeUseCases as CoreProbeUseCases},
        security::{
            self, PasswordChangeFacadePlan, PasswordChangeRequest, PublicSecurityViewFacadePlan,
            SecurityPatch, SecurityPatchFacadePlan, SecurityView,
        },
        settings::{SettingsUseCases, SettingsUseCases as CoreSettingsUseCases},
        system::{self, Capability, CapabilityGatePlan, HostSurface, SystemCapabilities},
        threads::{
            self, ThreadBlocksPage, ThreadDetailPlan, ThreadDetailReadPlan, ThreadDetailRequest,
            ThreadListPlan, ThreadListReadPlan, ThreadsQuery,
        },
        updates::{UpdateUseCases, UpdateUseCases as CoreUpdateUseCases},
    },
    update::{analyze_job_failure, JobFailureAnalysis},
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
    host_surface: HostSurface,
}

impl<'a> NexusHubUseCases<'a> {
    pub fn new(platform: &'a PlatformPaths) -> Self {
        Self {
            config: None,
            platform,
            host_surface: HostSurface::default_for_platform(platform),
        }
    }

    pub fn new_for_surface(platform: &'a PlatformPaths, host_surface: HostSurface) -> Self {
        Self {
            config: None,
            platform,
            host_surface,
        }
    }

    pub fn with_config(config: &'a Config, platform: &'a PlatformPaths) -> Self {
        Self {
            config: Some(config),
            platform,
            host_surface: HostSurface::default_for_platform(platform),
        }
    }

    pub fn with_config_for_surface(
        config: &'a Config,
        platform: &'a PlatformPaths,
        host_surface: HostSurface,
    ) -> Self {
        Self {
            config: Some(config),
            platform,
            host_surface,
        }
    }

    pub fn threads(self) -> ThreadUseCases<'a> {
        ThreadUseCases {
            platform: self.platform,
        }
    }

    pub fn grok(self) -> GrokUseCases {
        GrokUseCases {
            paths: crate::grok::GrokPaths::default_for_user(),
        }
    }

    pub fn jobs(self) -> JobUseCases<'a> {
        JobUseCases {
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
        Ok(CoreUpdateUseCases::new_for_surface(
            self.config_required()?,
            self.platform,
            self.host_surface,
        ))
    }

    pub fn system(self) -> Result<SystemUseCases<'a>> {
        Ok(SystemUseCases {
            config: self.config_required()?,
            platform: self.platform,
            host_surface: self.host_surface,
        })
    }

    pub fn security(self) -> Result<SecurityUseCases<'a>> {
        Ok(SecurityUseCases {
            config: &self.config_required()?.security,
            platform: self.platform,
            host_surface: self.host_surface,
        })
    }

    fn config_required(self) -> Result<&'a Config> {
        self.config
            .ok_or_else(|| anyhow::anyhow!("config is required for this NexusHub use case"))
    }
}

pub struct GrokUseCases {
    paths: crate::grok::GrokPaths,
}

impl GrokUseCases {
    pub fn list(
        &self,
        limit: usize,
        query: Option<&str>,
    ) -> Result<Vec<crate::grok::GrokSessionSummary>> {
        crate::grok::list_grok_sessions(&self.paths, limit, query)
    }
    pub fn detail(&self, id: &str) -> Result<crate::grok::GrokSessionDetail> {
        crate::grok::grok_session_detail(&self.paths, id, None)
    }
    pub async fn rename(&self, id: &str, title: &str) -> Result<crate::grok::GrokSessionSummary> {
        crate::grok::rename_grok_session(&self.paths, id, title).await
    }
    pub fn delete_preview(&self, id: &str) -> Result<crate::grok::GrokDeletePreview> {
        crate::grok::preview_grok_delete(&self.paths, id)
    }
    pub fn delete_execute(
        &self,
        request: crate::grok::GrokDeleteRequest,
    ) -> Result<crate::grok::GrokDeleteResult> {
        crate::grok::execute_grok_delete(&self.paths, request)
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

    pub fn archive(self, thread_id: &str) -> Result<ThreadStateActionPlan> {
        jobs::plan_thread_archive_with_capability(self.platform, thread_id)
    }

    pub fn restore(self, thread_id: &str) -> Result<ThreadStateActionPlan> {
        jobs::plan_thread_restore_with_capability(self.platform, thread_id)
    }

    pub fn rename(self, request: ThreadRenameRequest) -> Result<ThreadStateActionPlan> {
        jobs::plan_thread_rename_with_capability(self.platform, request)
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
    host_surface: HostSurface,
}

impl<'a> SystemUseCases<'a> {
    pub fn capabilities(self) -> SystemCapabilities {
        system::system_capabilities_for_surface(self.config, self.platform, self.host_surface)
    }

    pub fn capability_gate(self, capability: Capability) -> CapabilityGatePlan {
        system::capability_gate_plan_for_surface(self.platform, self.host_surface, capability)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SecurityUseCases<'a> {
    config: &'a SecurityConfig,
    platform: &'a PlatformPaths,
    host_surface: HostSurface,
}

impl<'a> SecurityUseCases<'a> {
    pub fn view(
        self,
        settings: crate::db::SecuritySettings,
        stored_expected_hostname: Option<String>,
        stored_expected_action: Option<String>,
    ) -> Result<SecurityView> {
        security::security_view_with_surface(
            self.platform,
            self.host_surface,
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
        security::public_security_view_with_surface(
            self.platform,
            self.host_surface,
            settings,
            self.config,
            stored_turnstile_action,
            admin_configured,
            base_url,
        )
    }

    pub fn patch(self, patch: SecurityPatch) -> Result<SecurityPatchFacadePlan> {
        security::plan_security_patch_with_surface(self.platform, self.host_surface, patch)
    }

    pub fn change_password(
        self,
        request: PasswordChangeRequest,
        current_password_matches: bool,
    ) -> Result<PasswordChangeFacadePlan> {
        security::plan_password_change_with_surface(
            self.platform,
            self.host_surface,
            request,
            current_password_matches,
        )
    }
}
