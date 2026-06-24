use crate::overview::DesktopState;
use anyhow::Result;
use nexushub_core::{
    probe::{ProbeRuntime, ProbeStatus},
    services::use_cases::NexusHubUseCases,
};

pub async fn desktop_probe_status_with_state(state: &DesktopState) -> Result<ProbeStatus> {
    let config = state.config();
    let limit = config.probe.recent_limit.clamp(1, 200);
    let facade_status = NexusHubUseCases::with_config(&config, state.platform())
        .probe()?
        .status()?
        .status;
    let running_count = facade_status.running_threads.len();
    let reply_needed_count = facade_status.reply_needed_threads.len();
    let recoverable_count = facade_status.recoverable_threads.len();
    let mut status = ProbeRuntime::new(config, state.platform().clone())
        .status()
        .await?;
    status.recent_event_count = facade_status.recent_event_count;
    status.running_count = running_count;
    status.reply_needed_count = reply_needed_count;
    status.recoverable_count = recoverable_count;
    status.running_threads = facade_status.running_threads;
    status.reply_needed_threads = facade_status.reply_needed_threads;
    status.recoverable_threads = facade_status.recoverable_threads;
    if let Ok(events) = state.db.list_probe_events(limit as u32) {
        status.recent_event_count = events.len();
    }
    Ok(status)
}
