#![allow(non_snake_case)]

use crate::overview::DesktopState;
use anyhow::Result;
use nexushub_core::{
    db::JobRecord,
    update::{analyze_job_failure, JobFailureAnalysis},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopJobResponse {
    #[serde(flatten)]
    pub job: JobRecord,
    pub failure_analysis: Option<JobFailureAnalysis>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopJobsRequest {
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopJobDetailRequest {
    pub id: String,
}

#[tauri::command(rename = "jobs.list")]
pub fn listJobs(
    state: tauri::State<'_, DesktopState>,
    limit: Option<u32>,
) -> Result<Vec<DesktopJobResponse>, String> {
    jobs_with_state(&state, DesktopJobsRequest { limit }).map_err(|err| err.to_string())
}

#[tauri::command(rename = "jobs.detail")]
pub fn getJob(
    state: tauri::State<'_, DesktopState>,
    id: String,
) -> Result<Option<DesktopJobResponse>, String> {
    job_detail_with_state(&state, DesktopJobDetailRequest { id }).map_err(|err| err.to_string())
}

fn jobs_with_state(
    state: &DesktopState,
    request: DesktopJobsRequest,
) -> Result<Vec<DesktopJobResponse>> {
    Ok(state
        .db
        .list_jobs(request.limit.unwrap_or(50).min(200))?
        .into_iter()
        .map(job_response)
        .collect())
}

fn job_detail_with_state(
    state: &DesktopState,
    request: DesktopJobDetailRequest,
) -> Result<Option<DesktopJobResponse>> {
    Ok(state.db.job(&request.id)?.map(job_response))
}

fn job_response(job: JobRecord) -> DesktopJobResponse {
    let failure_analysis = if job.status == "failed" {
        analyze_job_failure(&job.kind, &job.output, job.error.as_deref(), job.exit_code)
    } else {
        None
    };
    DesktopJobResponse {
        job,
        failure_analysis,
    }
}
