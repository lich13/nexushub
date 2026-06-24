use crate::overview::DesktopState;
use anyhow::Result;
use nexushub_core::config::Config;
use nexushub_core::platform::PlatformPaths;
use nexushub_core::services::{
    commands,
    updates::{self, UpdateAction, UpdateState, UpdateStatus},
};
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

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

pub fn desktop_update_status_with_state(
    state: &DesktopState,
    latest_version: Option<&str>,
    last_error: Option<&str>,
) -> Result<UpdateStatus> {
    let config = state.config();
    let recent_check_job = if latest_version.is_none() && last_error.is_none() {
        state
            .db
            .list_jobs(25)?
            .into_iter()
            .find(|job| job.kind == "nexushub_update_check")
    } else {
        None
    };
    Ok(updates::update_status_with_recent_check_job(
        &config,
        state.platform(),
        latest_version,
        last_error,
        recent_check_job.as_ref(),
    ))
}

pub(crate) async fn check_update_status(
    app: AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> std::result::Result<DesktopUpdateCheckResponse, String> {
    let job =
        native_update_job_plan_for_action(&state.config(), state.platform(), UpdateAction::Check)
            .map_err(|err| err.to_string())?;
    let job_id = update_job_id(job.id_action);
    state
        .db
        .create_job(&job_id, &job.spec.kind, &job.spec.title)
        .map_err(|err| err.to_string())?;
    state
        .db
        .append_job_output(&job_id, &job.spec.initial_output)
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

pub(crate) async fn install_update_and_restart(
    app: AppHandle,
    state: tauri::State<'_, DesktopState>,
) -> std::result::Result<DesktopUpdateInstallResponse, String> {
    let job =
        native_update_job_plan_for_action(&state.config(), state.platform(), UpdateAction::Install)
            .map_err(|err| err.to_string())?;
    let job_id = update_job_id(job.id_action);
    state
        .db
        .create_job(&job_id, &job.spec.kind, &job.spec.title)
        .map_err(|err| err.to_string())?;
    state
        .db
        .append_job_output(&job_id, &job.spec.initial_output)
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

#[derive(Debug)]
struct NativeUpdateJobPlan {
    id_action: &'static str,
    spec: updates::MacosUpdaterJobSpec,
}

fn native_update_job_plan_for_action(
    config: &Config,
    platform: &PlatformPaths,
    action: UpdateAction,
) -> Result<NativeUpdateJobPlan> {
    let plan = updates::plan_update_action(config, platform, action)?;
    let native = plan
        .native
        .ok_or_else(|| anyhow::anyhow!("update action did not produce a native updater spec"))?;
    let spec = plan
        .macos_job
        .ok_or_else(|| anyhow::anyhow!("update action did not produce a macOS updater job spec"))?;
    Ok(NativeUpdateJobPlan {
        id_action: native_update_id_action(&native.command)?,
        spec,
    })
}

fn native_update_id_action(command: &str) -> Result<&'static str> {
    match command {
        commands::UPDATES_CHECK => Ok("check"),
        commands::UPDATES_INSTALL => Ok("install"),
        command => anyhow::bail!("unsupported native update command: {command}"),
    }
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
                &updates::macos_updater_update_available_output(&update.version),
            )?;
            status.latest_version = Some(update.version.clone());
            status.update_available =
                updates::update_available_for_versions(&status.current_version, &update.version);
            status.state = if status.update_available == Some(true) {
                UpdateState::Ready
            } else {
                UpdateState::Idle
            };
        }
        None => {
            state
                .db
                .append_job_output(job_id, updates::macos_updater_no_update_output())?;
            status.latest_version = Some(status.current_version.clone());
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
            .append_job_output(job_id, updates::macos_updater_no_update_output())?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use nexushub_core::{
        db::JobRecord,
        platform::{PlatformKind, PlatformPaths},
    };

    #[test]
    fn native_update_commands_are_validated_against_core_plan() {
        let config = Config::for_platform_kind(PlatformKind::Macos);
        let platform = PlatformPaths::for_kind(PlatformKind::Macos);

        let check =
            native_update_job_plan_for_action(&config, &platform, UpdateAction::Check).unwrap();
        assert_eq!(check.id_action, "check");
        assert_eq!(check.spec.kind, "nexushub_update_check");

        let install =
            native_update_job_plan_for_action(&config, &platform, UpdateAction::Install).unwrap();
        assert_eq!(install.id_action, "install");
        assert_eq!(install.spec.kind, "nexushub_update_install");

        let prune_action = {
            use UpdateAction as Action;
            Action::Prune
        };
        let prune = native_update_job_plan_for_action(&config, &platform, prune_action)
            .unwrap_err()
            .to_string();
        assert!(
            prune.contains("prune_backups is unavailable on macos"),
            "{prune}"
        );
    }

    #[test]
    fn recent_check_job_output_keeps_no_update_status_explicit() {
        let config = Config::for_platform_kind(PlatformKind::Macos);
        let platform = PlatformPaths::for_kind(PlatformKind::Macos);
        let job = job_record(
            "succeeded",
            "checking signed Tauri updater feed\nno signed app update available\n",
        );

        let status = updates::update_status_with_recent_check_job(
            &config,
            &platform,
            None,
            None,
            Some(&job),
        );

        assert_eq!(status.state, UpdateState::Idle);
        assert_eq!(
            status.latest_version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
        assert_eq!(status.update_available, Some(false));
    }

    #[test]
    fn update_command_source_uses_core_recent_check_and_marker_helpers() {
        let command_source = include_str!("updates.rs")
            .split("\n#[cfg(test)]")
            .next()
            .expect("updates production source");

        assert!(
            command_source.contains("updates::update_status_with_recent_check_job"),
            "Tauri commands should let core derive recent update check status"
        );
        assert!(
            command_source.contains(".macos_job"),
            "Tauri commands should use core action-plan job metadata for native updater jobs"
        );
        assert!(
            command_source.contains("updates::macos_updater_update_available_output")
                && command_source.contains("updates::macos_updater_no_update_output"),
            "Tauri commands should use core updater output marker helpers"
        );
        assert!(
            !command_source.contains("fn apply_recent_check_job")
                && !command_source.contains("fn signed_update_version_from_output"),
            "Tauri commands must not duplicate recent check status parsing"
        );
        assert!(
            !command_source.contains("macos_updater_job_spec(action)"),
            "Tauri commands must consume the core update action plan macos_job instead of rebuilding job metadata"
        );
    }

    fn job_record(status: &str, output: &str) -> JobRecord {
        JobRecord {
            id: "job-1".to_string(),
            kind: "nexushub_update_check".to_string(),
            status: status.to_string(),
            title: "NexusHub app update check".to_string(),
            thread_id: None,
            turn_id: None,
            started_at: 1,
            finished_at: Some(2),
            exit_code: Some(0),
            output: output.to_string(),
            error: None,
        }
    }
}
