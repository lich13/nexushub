//! Shared by the Linux service and the desktop's port-free monitor helper.
use super::{send_bark_notification, ProbeBarkOutcome, ProbeBarkRequest};
use anyhow::Result;
use nexushub_core::{
    config::Config,
    db::PanelDb,
    native_probe::{self, NativeProvider, NativeScan},
};
use std::time::Duration;

pub async fn run(config: &Config, db: &PanelDb) -> Result<()> {
    db.maintain_notification_history_if_due(config.probe.observability.event_retention_days)?;
    db.recover_interrupted_native_deliveries()?;
    let mut snapshots = Vec::new();
    for provider in [NativeProvider::Grok, NativeProvider::Pi] {
        let scan = if provider.enabled(config) {
            match tokio::task::spawn_blocking(move || native_probe::scan(provider)).await? {
                Ok(scan) => scan,
                Err(_) => NativeScan {
                    streams: vec![],
                    errors: 1,
                },
            }
        } else {
            NativeScan::default()
        };
        db.stage_native_notifications(provider, &scan, config)?;
        snapshots.extend(scan.streams);
    }
    deliver_pending(config, db, &snapshots).await
}

async fn deliver_pending(
    config: &Config,
    db: &PanelDb,
    snapshots: &[native_probe::NativeStreamSnapshot],
) -> Result<()> {
    let key = db.get_secret_setting_bytes("probe_bark_device_key")?;
    for delivery in db.pending_native_deliveries(100)? {
        // Identity and branch are resolved by each native parser. Codex's
        // main-task identity filter must never be applied to these providers.
        let still_current = snapshots.iter().any(|stream| {
            stream.provider == delivery.provider
                && stream.session_key == delivery.session_key
                && stream.id == delivery.thread_id
                && stream.events.iter().any(|event| {
                    stream.event_key(event) == delivery.event_key && *event == delivery.event
                })
        });
        if !db.claim_native_delivery(&delivery.event_key)? {
            continue;
        }
        let enabled = delivery.provider.enabled(config)
            && delivery
                .provider
                .event_enabled(config, &delivery.event.kind);
        let configured = key.as_ref().is_some_and(|key| !key.is_empty());
        let outcome = if !still_current {
            ProbeBarkOutcome::skipped(
                "native_identity_or_branch_changed",
                config.probe.notifications.enabled,
                enabled,
                configured,
            )
        } else if !enabled {
            ProbeBarkOutcome::skipped(
                "event_switch_disabled",
                config.probe.notifications.enabled,
                false,
                configured,
            )
        } else if !configured {
            ProbeBarkOutcome::skipped(
                "device_key_missing",
                config.probe.notifications.enabled,
                true,
                false,
            )
        } else {
            let label = if delivery.event.kind == "completion" {
                "完成"
            } else {
                "失败"
            };
            let provider = delivery.provider.as_str();
            let request = ProbeBarkRequest {
                title: format!("{} · {} · {}", provider, label, delivery.title),
                body: format!(
                    "{}\n\n线程 ID：{}\n回合：{}",
                    delivery.event.body, delivery.thread_id, delivery.event.turn_id
                ),
                dedupe_key: delivery.event_key.clone(),
            };
            match send_bark_notification(
                config,
                key.as_deref().unwrap_or_default(),
                &request,
                Duration::from_secs(8),
            )
            .await
            {
                Ok(outcome) => outcome,
                Err(_) => ProbeBarkOutcome::failed_request(
                    "delivery_error",
                    true,
                    true,
                    true,
                    Some(delivery.event_key.clone()),
                ),
            }
        };
        db.finish_native_delivery(&delivery, serde_json::to_value(outcome)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use native_probe::{NativeStreamSnapshot, NativeTurnEvent};
    use serde_json::Value;

    #[tokio::test]
    async fn native_deliveries_use_provider_identity_and_do_not_replay_after_send() {
        for (provider, kind) in [
            (NativeProvider::Grok, "completion"),
            (NativeProvider::Grok, "failure"),
            (NativeProvider::Pi, "completion"),
        ] {
            let server = crate::tests::TestHttpServer::start_n(1, "HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: 12\r\n\r\n{\"code\":200}");
            let mut config = Config::default();
            config.probe.notifications.enabled = true;
            config.probe.notifications.server_url = server.url();
            let db = PanelDb::open(":memory:").unwrap();
            db.set_secret_setting_bytes("probe_bark_device_key", b"fixture-device")
                .unwrap();
            db.stage_native_notifications(provider, &NativeScan::default(), &config)
                .unwrap();
            let stream = NativeStreamSnapshot {
                provider,
                session_key: "opaque-native-key".into(),
                id: "custom-native-id".into(),
                title: "Isolated fixture".into(),
                identity: "fixture-file-identity".into(),
                record_count: 2,
                settled_count: 2,
                events: vec![NativeTurnEvent {
                    position: 2,
                    turn_id: "native-turn".into(),
                    kind: kind.into(),
                    body: "Final fixture answer".into(),
                    timestamp_ms: chrono::Utc::now().timestamp_millis() + 1000,
                }],
            };
            db.stage_native_notifications(
                provider,
                &NativeScan {
                    streams: vec![stream.clone()],
                    errors: 0,
                },
                &config,
            )
            .unwrap();
            deliver_pending(&config, &db, std::slice::from_ref(&stream))
                .await
                .unwrap();
            let request = server.request();
            let capture: Value =
                serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert!(capture["body"]
                .as_str()
                .unwrap()
                .contains("Final fixture answer"));
            assert!(capture["title"]
                .as_str()
                .unwrap()
                .contains(provider.as_str()));
            let events = db.list_probe_events(10).unwrap();
            assert_eq!(events.len(), 1);
            assert_eq!(events[0].payload["provider"], provider.as_str());
            assert_eq!(events[0].payload["session_key"], "opaque-native-key");
            assert_eq!(events[0].payload["bark_status"], "sent");
            db.stage_native_notifications(
                provider,
                &NativeScan {
                    streams: vec![stream.clone()],
                    errors: 0,
                },
                &config,
            )
            .unwrap();
            deliver_pending(&config, &db, &[stream]).await.unwrap();
            assert_eq!(db.list_probe_events(10).unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn failed_transport_is_visible_and_changed_branch_is_not_delivered() {
        let server = crate::tests::TestHttpServer::start_n(
            1,
            "HTTP/1.1 503 Service Unavailable\r\nConnection: close\r\nContent-Length: 0\r\n\r\n",
        );
        let mut config = Config::default();
        config.probe.notifications.enabled = true;
        config.probe.notifications.server_url = server.url();
        let db = PanelDb::open(":memory:").unwrap();
        db.set_secret_setting_bytes("probe_bark_device_key", b"fixture-device")
            .unwrap();
        db.stage_native_notifications(NativeProvider::Pi, &NativeScan::default(), &config)
            .unwrap();
        let mut stream = NativeStreamSnapshot {
            provider: NativeProvider::Pi,
            session_key: "p/a.jsonl".into(),
            id: "custom".into(),
            title: "Fixture".into(),
            identity: "file".into(),
            record_count: 2,
            settled_count: 2,
            events: vec![NativeTurnEvent {
                position: 2,
                turn_id: "u:a".into(),
                kind: "completion".into(),
                body: "Fixture reply".into(),
                timestamp_ms: chrono::Utc::now().timestamp_millis() + 1000,
            }],
        };
        db.stage_native_notifications(
            NativeProvider::Pi,
            &NativeScan {
                streams: vec![stream.clone()],
                errors: 0,
            },
            &config,
        )
        .unwrap();
        deliver_pending(&config, &db, std::slice::from_ref(&stream))
            .await
            .unwrap();
        assert!(server.request().contains("POST"));
        assert_eq!(
            db.list_probe_events(10).unwrap()[0].payload["bark_status"],
            "failed"
        );
        stream.record_count = 3;
        stream.settled_count = 3;
        stream.events[0].position = 3;
        stream.events[0].turn_id = "u:b".into();
        db.stage_native_notifications(
            NativeProvider::Pi,
            &NativeScan {
                streams: vec![stream],
                errors: 0,
            },
            &config,
        )
        .unwrap();
        deliver_pending(&config, &db, &[]).await.unwrap();
        let events = db.list_probe_events(10).unwrap();
        assert!(events
            .iter()
            .any(|event| event.payload["bark"]["reason"] == "native_identity_or_branch_changed"));
    }
}
