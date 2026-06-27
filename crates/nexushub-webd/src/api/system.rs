use super::{api_error, ok, ApiResponse};
use crate::{
    auth::{require_auth, require_csrf},
    linux_adapter,
    state::AppState,
};
use anyhow::Result as AnyhowResult;
use axum::{
    extract::{Query, State},
    http::HeaderMap,
};
use nexushub_core::{
    local,
    platform::{PlatformKind, PlatformPaths},
    services::{
        system::{require_capability_for_surface, Capability},
        updates::{self as update_service, UpdateAction},
    },
};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

pub(crate) async fn system_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let platform = state.platform().clone();
    ok(nexushub_core::system::system_status_with_surface(
        &state.config(),
        &platform,
        state.host_surface(),
    )
    .await?)
}

pub(crate) async fn system_version(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(http_version_info().await?)
}

pub(crate) async fn system_update_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let version = http_version_info().await?;
    let platform = state.platform().clone();
    ok(update_service::update_status_for_surface(
        &state.config(),
        &platform,
        state.host_surface(),
        version.panel_latest.as_deref(),
        None,
    ))
}

async fn http_version_info() -> AnyhowResult<nexushub_core::system::VersionInfo> {
    let inputs = nexushub_core::system::VersionInfoInputs {
        panel_latest: github_latest_release("lich13", "nexushub").await.ok(),
        codex_latest: npm_latest_version("@openai/codex").await.ok(),
    };
    nexushub_core::system::version_info_with_inputs(inputs).await
}

async fn github_latest_release(owner: &str, repo: &str) -> AnyhowResult<String> {
    #[derive(Deserialize)]
    struct Release {
        tag_name: String,
    }
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    let release: Release = reqwest::Client::new()
        .get(url)
        .header("user-agent", "nexushub")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(release.tag_name)
}

async fn npm_latest_version(package: &str) -> AnyhowResult<String> {
    #[derive(Deserialize)]
    struct DistTags {
        latest: String,
    }
    #[derive(Deserialize)]
    struct PackageInfo {
        #[serde(rename = "dist-tags")]
        dist_tags: DistTags,
    }
    let encoded = package.replace('/', "%2F");
    let url = format!("https://registry.npmjs.org/{encoded}");
    let package: PackageInfo = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()?
        .get(url)
        .header("user-agent", "nexushub")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(package.dist_tags.latest)
}

#[derive(Debug, Deserialize)]
pub(crate) struct CwdQuery {
    pub(crate) cwd: Option<String>,
}

pub(crate) async fn codex_models(State(state): State<AppState>, headers: HeaderMap) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(local::default_codex_models())
}

pub(crate) async fn codex_permission_profiles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CwdQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let _ = query.cwd;
    ok(local::default_permission_profiles())
}

pub(crate) async fn codex_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CwdQuery>,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(local::local_codex_config(
        &state.config(),
        query.cwd.as_deref(),
    ))
}

pub(crate) async fn start_update_action(
    state: AppState,
    headers: HeaderMap,
    action: UpdateAction,
    audit_action: Option<&str>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let platform = state.platform().clone();
    require_capability_for_surface(
        &platform,
        state.host_surface(),
        match action {
            UpdateAction::Prune => Capability::PruneBackups,
            UpdateAction::Check | UpdateAction::Install => Capability::LinuxUpdateJob,
        },
    )?;
    let plan = linux_adapter::linux_update_action_plan(&state, &platform, action)?;
    let id = linux_adapter::start_update_action_plan(&state, &auth, plan, audit_action)?;
    ok(json!({"job_id": id}))
}

pub(crate) fn http_update_platform() -> PlatformPaths {
    PlatformPaths::for_kind(PlatformKind::Linux)
}
