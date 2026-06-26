use super::{api_error, http_update_platform, ok, ApiResponse};
use crate::{
    auth::{require_auth, require_csrf},
    linux_adapter,
    state::{AppState, CachedProbeStatus},
};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use nexushub_core::{
    config::Config,
    platform::{PlatformKind, PlatformPaths},
    probe::{redact_probe_event_for_output, ProbeRuntime},
    services::{
        probe::{self as probe_service},
        settings as settings_service,
        use_cases::NexusHubUseCases,
    },
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub(crate) struct ProbeStatusQuery {
    pub(crate) refresh: Option<bool>,
}

pub(crate) async fn get_probe_status(
    State(state): State<AppState>,
    Query(query): Query<ProbeStatusQuery>,
    headers: HeaderMap,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(probe_status_cached_value(state, query.refresh.unwrap_or(false)).await)
}

pub(crate) async fn get_probe_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    ok(probe_settings_value(&state)?)
}

pub(crate) async fn patch_probe_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<settings_service::ProbeSettingsSaveRequest>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let config_path = probe_config_path();
    if !config_path.exists() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            &format!("config file not found: {}", config_path.display()),
        ));
    }
    let platform = http_update_platform();
    let config = state.config();
    let plan = NexusHubUseCases::with_config(&config, &platform)
        .settings()
        .and_then(|settings| settings.save_probe_settings(request))
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    let response_config =
        linux_adapter::apply_probe_settings_save_plan(&state, &auth, &config_path, plan)?;
    ok(probe_settings_value_for_config(&state, &response_config)?)
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProbeEventsQuery {
    pub(crate) limit: Option<u32>,
}

pub(crate) async fn get_probe_events(
    State(state): State<AppState>,
    Query(query): Query<ProbeEventsQuery>,
    headers: HeaderMap,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let limit = query
        .limit
        .unwrap_or(state.config().probe.recent_limit as u32)
        .clamp(1, 500);
    let events = state
        .db
        .list_probe_events(limit)?
        .into_iter()
        .map(redact_probe_event)
        .collect::<Vec<_>>();
    ok(json!({
        "events": events,
        "limit": limit,
    }))
}

pub(crate) async fn get_probe_logs_db_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResponse {
    require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    let config = state.config();
    let status = ProbeRuntime::new(config.clone(), PlatformPaths::current()).logs_db_status();
    let last_maintain = state
        .db
        .get_setting_with_updated_at("probe_logs_db_last_maintain")?
        .map(
            |(raw, updated_at_unix)| probe_service::ProbeLogsDbLastMaintain {
                raw,
                updated_at_unix,
            },
        );
    let value = probe_service::probe_logs_db_status_view(status, &config, last_maintain)?;
    ok(value)
}

pub(crate) async fn start_probe_action(
    state: AppState,
    headers: HeaderMap,
    action: probe_service::ProbeAction,
) -> ApiResponse {
    start_probe_action_with_compact(state, headers, action, None).await
}

async fn start_probe_action_with_compact(
    state: AppState,
    headers: HeaderMap,
    action: probe_service::ProbeAction,
    compact: Option<bool>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let platform = http_update_platform();
    let config_path = probe_config_path();
    let maintenance = matches!(
        action,
        probe_service::ProbeAction::LogsDbDryRun | probe_service::ProbeAction::LogsDbExecute
    )
    .then_some(probe_service::ProbeLogsDbMaintenanceRequest {
        dry_run: matches!(action, probe_service::ProbeAction::LogsDbDryRun),
        compact: compact.unwrap_or(false),
    });
    let plan = linux_adapter::linux_probe_action_plan(
        &state,
        &platform,
        action,
        &config_path,
        maintenance,
    )?;
    let id = linux_adapter::start_probe_action_plan(&state, &auth, plan)
        .map_err(|err| api_error(StatusCode::CONFLICT, &err.to_string()))?;
    ok(json!({"job_id": id}))
}

fn probe_runtime(state: &AppState) -> ProbeRuntime {
    ProbeRuntime::new(state.config(), PlatformPaths::current())
}

pub(crate) fn probe_config_path() -> PathBuf {
    std::env::var_os("NEXUSHUB_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PlatformPaths::current().config_file)
}

async fn probe_status_cached_value(state: AppState, force_refresh: bool) -> Value {
    if force_refresh {
        let value = probe_status_fresh_value(state.clone()).await;
        store_probe_status_snapshot(&state, value.clone());
        return probe_service::probe_status_snapshot_view(
            value,
            0,
            false,
            probe_service::ProbeSnapshotStatus::Fresh,
        );
    }

    if let Some(snapshot) = current_probe_status_snapshot(&state) {
        spawn_probe_status_refresh(state);
        let now = chrono::Utc::now().timestamp();
        let age = now.saturating_sub(snapshot.refreshed_at_unix).max(0);
        return probe_service::probe_status_snapshot_view(
            snapshot.value,
            age,
            true,
            probe_service::ProbeSnapshotStatus::Cached,
        );
    }

    let value = probe_status_base_value(state.clone()).await;
    spawn_probe_status_refresh(state);
    probe_service::probe_status_snapshot_view(
        value,
        0,
        true,
        probe_service::ProbeSnapshotStatus::Initial,
    )
}

async fn probe_status_fresh_value(state: AppState) -> Value {
    match probe_runtime(&state).status().await {
        Ok(status) => json!(status),
        Err(err) => json!({
            "label": "Probe",
            "enabled": state.config().probe.enabled,
            "available": false,
            "flavor": "builtin",
            "error": err.to_string(),
        }),
    }
}

async fn probe_status_base_value(state: AppState) -> Value {
    match probe_runtime(&state).status().await {
        Ok(status) => json!(status),
        Err(err) => json!({
            "label": "Probe",
            "enabled": state.config().probe.enabled,
            "available": false,
            "flavor": "builtin",
            "error": err.to_string(),
        }),
    }
}

fn current_probe_status_snapshot(state: &AppState) -> Option<CachedProbeStatus> {
    state
        .probe_status_cache
        .lock()
        .expect("probe status cache")
        .snapshot
        .clone()
}

fn store_probe_status_snapshot(state: &AppState, value: Value) {
    let mut cache = state.probe_status_cache.lock().expect("probe status cache");
    cache.snapshot = Some(CachedProbeStatus {
        value,
        refreshed_at_unix: chrono::Utc::now().timestamp(),
    });
    cache.refreshing = false;
}

pub(crate) fn spawn_probe_status_refresh(state: AppState) {
    {
        let mut cache = state.probe_status_cache.lock().expect("probe status cache");
        if cache.refreshing {
            return;
        }
        cache.refreshing = true;
    }
    tokio::spawn(async move {
        let value = probe_status_fresh_value(state.clone()).await;
        store_probe_status_snapshot(&state, value);
    });
}

fn probe_settings_value(state: &AppState) -> anyhow::Result<Value> {
    probe_settings_value_for_config(state, &state.config())
}

fn probe_settings_value_for_config(state: &AppState, config: &Config) -> anyhow::Result<Value> {
    let secret_state = settings_service::ProbeSecretState::from_secret_bytes(
        state
            .db
            .get_secret_setting_bytes(settings_service::PROBE_BARK_DEVICE_KEY_SETTING)?
            .as_deref(),
    );
    let platform = PlatformPaths::for_kind(PlatformKind::Linux);
    let plan = NexusHubUseCases::with_config(config, &platform)
        .settings()?
        .probe_settings_view(secret_state)?;
    serde_json::to_value(plan.settings).map_err(anyhow::Error::from)
}

pub(crate) async fn load_probe_threads(
    state: &AppState,
    status: &'static str,
    limit: usize,
) -> anyhow::Result<Vec<nexushub_core::codex::ThreadSummary>> {
    linux_adapter::probe_threads_read_model(state, status, limit)
}

fn redact_probe_event(event: nexushub_core::db::ProbeEvent) -> nexushub_core::db::ProbeEvent {
    redact_probe_event_for_output(event)
}
