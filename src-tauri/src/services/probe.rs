use crate::overview::DesktopState;
use anyhow::Result;
use nexushub_core::probe::{ProbeRuntime, ProbeStatus};

pub async fn desktop_probe_status_with_state(state: &DesktopState) -> Result<ProbeStatus> {
    let config = state.config();
    let limit = config.probe.recent_limit.clamp(1, 200);
    let mut status = ProbeRuntime::new(config, state.platform().clone())
        .status()
        .await?;
    if let Ok(events) = state.db.list_probe_events(limit as u32) {
        status.recent_event_count = events.len();
    }
    Ok(status)
}
