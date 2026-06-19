#![allow(non_snake_case)]

use crate::overview::{
    desktop_job_detail_with_state, desktop_jobs_with_state, DesktopJobDetailRequest,
    DesktopJobResponse, DesktopJobsRequest, DesktopState,
};

#[tauri::command]
pub fn desktop_jobs(
    state: tauri::State<'_, DesktopState>,
    request: DesktopJobsRequest,
) -> Result<Vec<DesktopJobResponse>, String> {
    desktop_jobs_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn desktop_job_detail(
    state: tauri::State<'_, DesktopState>,
    request: DesktopJobDetailRequest,
) -> Result<Option<DesktopJobResponse>, String> {
    desktop_job_detail_with_state(&state, request).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn listJobs(
    state: tauri::State<'_, DesktopState>,
    limit: Option<u32>,
) -> Result<Vec<DesktopJobResponse>, String> {
    desktop_jobs_with_state(&state, DesktopJobsRequest { limit }).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn getJob(
    state: tauri::State<'_, DesktopState>,
    id: String,
) -> Result<Option<DesktopJobResponse>, String> {
    desktop_job_detail_with_state(&state, DesktopJobDetailRequest { id })
        .map_err(|err| err.to_string())
}
