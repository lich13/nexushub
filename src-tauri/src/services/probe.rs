use crate::overview::DesktopState;
use anyhow::Result;
use nexushub_core::{
    probe::{ProbeRuntime, ProbeStatus},
    services::{probe as probe_service, use_cases::NexusHubUseCases},
};

pub async fn desktop_probe_status_with_state(state: &DesktopState) -> Result<ProbeStatus> {
    let config = state.config();
    let limit = config.probe.recent_limit.clamp(1, 200);
    let facade_status = NexusHubUseCases::with_config(&config, state.platform())
        .probe()?
        .status()?
        .status;
    let status = ProbeRuntime::new(config, state.platform().clone())
        .status()
        .await?;
    let recent_event_count = state.db.list_probe_events(limit as u32).ok().map(|events| events.len());
    Ok(probe_service::probe_status_with_runtime_read_model(
        status,
        facade_status,
        recent_event_count,
    ))
}
