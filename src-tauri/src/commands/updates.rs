use crate::overview::DesktopState;
use anyhow::Result;
use nexushub_core::config::Config;
use nexushub_core::platform::PlatformPaths;
use nexushub_core::services::updates::{self, UpdateState, UpdateStatus};
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

pub fn desktop_update_status_with_state(
    state: &DesktopState,
    latest_version: Option<&str>,
    last_error: Option<&str>,
) -> Result<UpdateStatus> {
    desktop_update_status_for(
        &state.config(),
        state.platform(),
        latest_version,
        last_error,
    )
}

pub fn desktop_update_status_for(
    config: &Config,
    platform: &PlatformPaths,
    latest_version: Option<&str>,
    last_error: Option<&str>,
) -> Result<UpdateStatus> {
    Ok(updates::update_status(
        config,
        platform,
        latest_version,
        last_error,
    ))
}

#[derive(Debug, Clone, Serialize)]
pub struct DesktopUpdateCheckResponse {
    pub job_id: String,
    pub status: UpdateStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesktopUpdateInstallResponse {
    pub job_id: String,
    pub installed: bool,
}

#[tauri::command]
pub async fn check_update_status(
    app: AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> std::result::Result<DesktopUpdateCheckResponse, String> {
    let job_id = update_job_id("check");
    state
        .db
        .create_job(
            &job_id,
            "nexushub_update_check",
            "NexusHub app update check",
        )
        .map_err(|err| err.to_string())?;
    state
        .db
        .append_job_output(&job_id, "checking signed Tauri updater feed\n")
        .map_err(|err| err.to_string())?;

    match check_update(&app, &state, &job_id).await {
        Ok(status) => {
            let _ = state.db.finish_job(&job_id, "succeeded", Some(0), None);
            Ok(DesktopUpdateCheckResponse { job_id, status })
        }
        Err(err) => {
            let message = err.to_string();
            let _ = state
                .db
                .append_job_output(&job_id, &format!("error: {message}\n"));
            let _ = state.db.finish_job(&job_id, "failed", None, Some(&message));
            Err(message)
        }
    }
}

#[tauri::command]
pub async fn install_update_and_restart(
    app: AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> std::result::Result<DesktopUpdateInstallResponse, String> {
    let job_id = update_job_id("install");
    state
        .db
        .create_job(
            &job_id,
            "nexushub_update_install",
            "NexusHub app update install",
        )
        .map_err(|err| err.to_string())?;
    state
        .db
        .append_job_output(&job_id, "checking signed Tauri updater feed\n")
        .map_err(|err| err.to_string())?;

    match install_update(&app, &state, &job_id).await {
        Ok(installed) => {
            let _ = state.db.finish_job(&job_id, "succeeded", Some(0), None);
            if installed {
                app.restart();
            }
            Ok(DesktopUpdateInstallResponse { job_id, installed })
        }
        Err(err) => {
            let message = err.to_string();
            let _ = state
                .db
                .append_job_output(&job_id, &format!("error: {message}\n"));
            let _ = state.db.finish_job(&job_id, "failed", None, Some(&message));
            Err(message)
        }
    }
}

fn update_job_id(action: &str) -> String {
    format!(
        "desktop-update-{action}-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        std::process::id()
    )
}

async fn check_update(app: &AppHandle, state: &DesktopState, job_id: &str) -> Result<UpdateStatus> {
    let updater = app
        .updater_builder()
        .build()
        .map_err(|err| anyhow::anyhow!("初始化更新器失败: {err}"))?;
    let mut status = desktop_update_status_with_state(state, None, None)?;
    match updater
        .check()
        .await
        .map_err(|err| anyhow::anyhow!("检查更新失败: {err}"))?
    {
        Some(update) => {
            state.db.append_job_output(
                job_id,
                &format!("signed app update available {}\n", update.version),
            )?;
            status.latest_version = Some(update.version.clone());
            status.update_available = Some(true);
            status.state = UpdateState::Ready;
        }
        None => {
            state
                .db
                .append_job_output(job_id, "no signed app update available\n")?;
            status.update_available = Some(false);
            status.state = UpdateState::Idle;
        }
    }
    Ok(status)
}

async fn install_update(app: &AppHandle, state: &DesktopState, job_id: &str) -> Result<bool> {
    let updater = app
        .updater_builder()
        .build()
        .map_err(|err| anyhow::anyhow!("初始化更新器失败: {err}"))?;
    let Some(update) = updater
        .check()
        .await
        .map_err(|err| anyhow::anyhow!("检查更新失败: {err}"))?
    else {
        state
            .db
            .append_job_output(job_id, "no signed app update available\n")?;
        return Ok(false);
    };

    state.db.append_job_output(
        job_id,
        &format!("downloading signed app update {}\n", update.version),
    )?;
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|err| anyhow::anyhow!("下载更新失败: {err}"))?;
    state.db.append_job_output(
        job_id,
        &format!("installing signed app update {}\n", update.version),
    )?;
    update
        .install(bytes)
        .map_err(|err| anyhow::anyhow!("安装更新失败: {err}"))?;
    state
        .db
        .append_job_output(job_id, "signed app update installed\n")?;
    Ok(true)
}
