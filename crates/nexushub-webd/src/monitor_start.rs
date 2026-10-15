//! Existing monitor start implementation, split without changing dispatch order.
use super::*;

pub(super) fn spawn_probe_error_monitor(state: AppState) {
    tokio::spawn(async move {
        loop {
            let config = state.config();
            match run_probe_error_monitor_once(&state.db, &|| Ok(state.config())).await {
                Ok(outcome) if outcome.new_incidents > 0 => tracing::info!(
                    new_incidents = outcome.new_incidents,
                    "Probe error monitor processed Codex turn errors"
                ),
                Ok(_) => tracing::debug!("Probe error monitor scan completed"),
                Err(err) => tracing::warn!("Probe error monitor scan failed: {err}"),
            }
            time::sleep(Duration::from_secs(
                config.probe.poll_seconds.clamp(5, 3_600),
            ))
            .await;
        }
    });
}

pub(super) async fn run_probe_error_monitor_daemon(config_path: PathBuf) -> Result<()> {
    let mut upgraded_hooks = false;
    loop {
        let poll_seconds = match Config::load(&config_path) {
            Ok(config) => {
                if !upgraded_hooks {
                    upgrade_managed_question_hooks_on_start(&config);
                    upgraded_hooks = true;
                }
                match open_panel_db(&config) {
                    Ok(db) => {
                        if let Err(err) =
                            run_probe_error_monitor_once(&db, &|| Config::load(&config_path)).await
                        {
                            tracing::warn!("Probe error monitor scan failed: {err}");
                        }
                    }
                    Err(err) => tracing::warn!("Probe error monitor DB open failed: {err}"),
                }
                config.probe.poll_seconds.clamp(5, 3_600)
            }
            Err(err) => {
                tracing::warn!("Probe error monitor config load failed: {err}");
                15
            }
        };
        time::sleep(Duration::from_secs(poll_seconds)).await;
    }
}
