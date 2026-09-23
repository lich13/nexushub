use super::PanelDb;
use crate::{
    config::Config,
    native_probe::{NativeProvider, NativeScan, NativeTurnEvent},
    security::redact_output,
};
use anyhow::Result;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeDelivery {
    pub event_key: String,
    pub provider: NativeProvider,
    pub session_key: String,
    pub thread_id: String,
    pub title: String,
    pub event: NativeTurnEvent,
}

impl PanelDb {
    /// Queue events and advance cursors together. First enable baselines every
    /// existing stream; new streams also ignore records older than activation.
    pub fn stage_native_notifications(
        &self,
        provider: NativeProvider,
        scan: &NativeScan,
        config: &Config,
    ) -> Result<usize> {
        let enabled = provider.enabled(config);
        let mut conn = self.conn.lock().expect("db mutex");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let prior = tx
            .query_row(
                "SELECT enabled,activated_ms FROM native_probe_providers WHERE provider=?1",
                [provider.as_str()],
                |r| Ok((r.get::<_, bool>(0)?, r.get::<_, i64>(1)?)),
            )
            .optional()?;
        let baseline = enabled && prior.is_none_or(|(was_enabled, _)| !was_enabled);
        let activated_ms = if baseline {
            chrono::Utc::now().timestamp_millis()
        } else {
            prior.map_or(0, |(_, ts)| ts)
        };
        tx.execute("INSERT INTO native_probe_providers(provider,enabled,activated_ms,scanned_at,stream_count,error_count) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(provider) DO UPDATE SET enabled=excluded.enabled,activated_ms=excluded.activated_ms,scanned_at=excluded.scanned_at,stream_count=excluded.stream_count,error_count=excluded.error_count",
            params![provider.as_str(), enabled, activated_ms, Self::now(), scan.streams.len(), scan.errors])?;
        if !enabled {
            tx.execute("UPDATE native_probe_deliveries SET status='skipped',updated_at=?2 WHERE provider=?1 AND status='pending'", params![provider.as_str(),Self::now()])?;
            tx.commit()?;
            return Ok(0);
        }
        let mut queued = 0;
        for stream in &scan.streams {
            let old = tx.query_row("SELECT identity,cursor FROM native_probe_streams WHERE provider=?1 AND session_key=?2", params![provider.as_str(),stream.session_key], |r| Ok((r.get::<_,String>(0)?,r.get::<_,u64>(1)?))).optional()?;
            let reset = baseline
                || old.as_ref().is_some_and(|(identity, cursor)| {
                    identity != &stream.identity || *cursor > stream.record_count
                });
            let cursor = if reset {
                stream.record_count
            } else {
                old.map_or(0, |(_, cursor)| cursor)
            };
            if !reset {
                for event in &stream.events {
                    if event.position <= cursor
                        || event.timestamp_ms < activated_ms
                        || !provider.event_enabled(config, &event.kind)
                    {
                        continue;
                    }
                    let delivery = NativeDelivery {
                        event_key: stream.event_key(event),
                        provider,
                        session_key: stream.session_key.clone(),
                        thread_id: stream.id.clone(),
                        title: redact_output(&stream.title),
                        event: event.clone(),
                    };
                    queued += tx.execute("INSERT OR IGNORE INTO native_probe_deliveries(event_key,provider,event_json,status,created_at,updated_at) VALUES(?1,?2,?3,'pending',?4,?4)", params![delivery.event_key,provider.as_str(),serde_json::to_string(&delivery)?,Self::now()])?;
                }
            }
            let cursor = cursor.max(stream.settled_count.min(stream.record_count));
            tx.execute("INSERT INTO native_probe_streams(provider,session_key,identity,cursor,updated_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(provider,session_key) DO UPDATE SET identity=excluded.identity,cursor=excluded.cursor,updated_at=excluded.updated_at",
                params![provider.as_str(),stream.session_key,stream.identity,cursor,Self::now()])?;
        }
        tx.commit()?;
        Ok(queued)
    }

    pub fn pending_native_deliveries(&self, limit: usize) -> Result<Vec<NativeDelivery>> {
        let conn = self.conn.lock().expect("db mutex");
        let mut stmt = conn.prepare("SELECT event_json FROM native_probe_deliveries WHERE status='pending' ORDER BY created_at,event_key LIMIT ?1")?;
        let rows = stmt
            .query_map([limit.min(100)], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|s| serde_json::from_str(&s).map_err(Into::into))
            .collect()
    }
    pub fn claim_native_delivery(&self, key: &str) -> Result<bool> {
        let conn = self.conn.lock().expect("db mutex");
        Ok(conn.execute("UPDATE native_probe_deliveries SET status='delivering',updated_at=?2 WHERE event_key=?1 AND status='pending'",params![key,Self::now()])?==1)
    }
    /// Delivery confirmation and the visible Probe event commit atomically.
    pub fn finish_native_delivery(&self, delivery: &NativeDelivery, bark: Value) -> Result<()> {
        let status = if bark.get("sent").and_then(Value::as_bool) == Some(true) {
            "sent"
        } else if bark.get("skipped").and_then(Value::as_bool) == Some(true) {
            "skipped"
        } else {
            "failed"
        };
        let mut conn = self.conn.lock().expect("db mutex");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed=tx.execute("UPDATE native_probe_deliveries SET status=?2,updated_at=?3 WHERE event_key=?1 AND status='delivering'",params![delivery.event_key,status,Self::now()])?;
        if changed > 0 {
            let summary = delivery.event.body.chars().take(240).collect::<String>();
            let payload = json!({"provider":delivery.provider,"session_key":delivery.session_key,"thread_id":delivery.thread_id,"turn_id":delivery.event.turn_id,"thread_title":delivery.title,"event_type":delivery.event.kind,"body_summary":summary,"body_source":"native_terminal_turn","bark":bark,"bark_status":status});
            tx.execute("INSERT INTO probe_events(id,kind,thread_id,title,message,dedupe_key,source,payload_json,created_at) VALUES(?1,?2,?3,?4,?5,?6,'native_provider_monitor',?7,?8)",params![uuid::Uuid::new_v4().to_string(),delivery.event.kind,delivery.thread_id,delivery.title,summary,delivery.event_key,payload.to_string(),Self::now()])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn recover_interrupted_native_deliveries(&self) -> Result<()> {
        let deliveries = {
            let conn = self.conn.lock().expect("db mutex");
            let mut stmt=conn.prepare("SELECT event_json FROM native_probe_deliveries WHERE status='delivering' AND updated_at < ?1")?;
            let rows = stmt
                .query_map([Self::now() - 120], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows.into_iter()
                .map(|s| serde_json::from_str::<NativeDelivery>(&s).map_err(anyhow::Error::from))
                .collect::<Result<Vec<_>>>()?
        };
        for delivery in deliveries {
            // Bark has no transport-level idempotency guarantee. A crashed
            // in-flight delivery is explicitly uncertain and is not replayed.
            self.finish_native_delivery(&delivery,json!({"sent":false,"skipped":false,"reason":"delivery_interrupted_outcome_unknown"}))?;
        }
        Ok(())
    }
    pub fn native_notification_status(&self) -> Result<Value> {
        let conn = self.conn.lock().expect("db mutex");
        let mut statuses = Vec::new();
        for provider in [NativeProvider::Grok, NativeProvider::Pi] {
            let row=conn.query_row("SELECT enabled,scanned_at,stream_count,error_count FROM native_probe_providers WHERE provider=?1",[provider.as_str()],|r|Ok((r.get::<_,bool>(0)?,r.get::<_,i64>(1)?,r.get::<_,u64>(2)?,r.get::<_,u64>(3)?))).optional()?;
            let (enabled, last_scan, streams, errors) = row.unwrap_or((false, 0, 0, 0));
            let failed:u64=conn.query_row("SELECT count(*) FROM native_probe_deliveries WHERE provider=?1 AND status='failed'",[provider.as_str()],|r|r.get(0))?;
            let pending:u64=conn.query_row("SELECT count(*) FROM native_probe_deliveries WHERE provider=?1 AND status IN ('pending','delivering')",[provider.as_str()],|r|r.get(0))?;
            statuses.push(json!({"provider":provider,"enabled":enabled,"last_scan_at":last_scan,"streams":streams,"read_errors":errors,"failed_deliveries":failed,"pending_deliveries":pending,"failure_supported":provider==NativeProvider::Grok}));
        }
        Ok(Value::Array(statuses))
    }
    pub fn maintain_notification_history_if_due(&self, retention_days: u32) -> Result<()> {
        let now = Self::now();
        let last = self
            .get_setting_with_updated_at("probe_event_retention_last_run")?
            .map(|(_, at)| at)
            .unwrap_or(0);
        if now.saturating_sub(last) < 3600 {
            return Ok(());
        }
        self.maintain_probe_events(retention_days, 100_000, false)?;
        let conn = self.conn.lock().expect("db mutex");
        conn.execute("DELETE FROM native_probe_deliveries WHERE status IN ('sent','failed','skipped') AND updated_at < ?1",[now-i64::from(retention_days.clamp(1,3650))*86400])?;
        drop(conn);
        self.set_setting("probe_event_retention_last_run", "ok")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_probe::{NativeStreamSnapshot, NativeTurnEvent};
    fn config() -> Config {
        let mut c = Config::default();
        c.probe.notifications.enabled = true;
        c
    }
    fn scan(id: &str, count: u64, now: i64) -> NativeScan {
        NativeScan {
            errors: 0,
            streams: vec![NativeStreamSnapshot {
                provider: NativeProvider::Pi,
                session_key: id.into(),
                id: "duplicate-native-id".into(),
                title: "test".into(),
                identity: id.into(),
                record_count: count,
                settled_count: count,
                events: (1..=count)
                    .map(|n| NativeTurnEvent {
                        position: n,
                        turn_id: format!("turn-{n}"),
                        kind: "completion".into(),
                        body: "test reply".into(),
                        timestamp_ms: now,
                    })
                    .collect(),
            }],
        }
    }
    #[test]
    fn baseline_restart_partial_failure_and_duplicate_native_ids_use_durable_keys() {
        let p = std::env::temp_dir().join(format!("native-db-{}.sqlite", uuid::Uuid::new_v4()));
        let db = PanelDb::open(&p).unwrap();
        let c = config();
        let now = chrono::Utc::now().timestamp_millis() + 1000;
        assert_eq!(
            db.stage_native_notifications(NativeProvider::Pi, &scan("a", 1, now), &c)
                .unwrap(),
            0
        );
        assert_eq!(
            db.stage_native_notifications(NativeProvider::Pi, &scan("a", 2, now), &c)
                .unwrap(),
            1
        );
        let d = db.pending_native_deliveries(100).unwrap().pop().unwrap();
        assert!(db.claim_native_delivery(&d.event_key).unwrap());
        assert!(!db.claim_native_delivery(&d.event_key).unwrap());
        db.finish_native_delivery(&d, json!({"sent":true})).unwrap();
        drop(db);
        let db = PanelDb::open(&p).unwrap();
        assert_eq!(
            db.stage_native_notifications(NativeProvider::Pi, &scan("a", 2, now), &c)
                .unwrap(),
            0
        );
        assert!(db.pending_native_deliveries(100).unwrap().is_empty());
        assert_eq!(
            db.stage_native_notifications(NativeProvider::Pi, &scan("b", 1, now), &c)
                .unwrap(),
            1
        );
        let other = db.pending_native_deliveries(100).unwrap().pop().unwrap();
        assert_ne!(d.event_key, other.event_key);
        db.claim_native_delivery(&other.event_key).unwrap();
        db.finish_native_delivery(
            &other,
            json!({"sent":false,"skipped":false,"reason":"http_status"}),
        )
        .unwrap();
        assert_eq!(
            db.native_notification_status().unwrap()[1]["failed_deliveries"],
            1
        );
        assert_eq!(db.list_probe_events(10).unwrap().len(), 2);
        drop(db);
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn disabled_provider_and_reenabled_provider_never_replay_history() {
        let db = PanelDb::open(":memory:").unwrap();
        let mut c = config();
        let now = chrono::Utc::now().timestamp_millis() + 1000;
        db.stage_native_notifications(NativeProvider::Pi, &scan("a", 1, now), &c)
            .unwrap();
        c.probe.notifications.notify_pi = false;
        db.stage_native_notifications(NativeProvider::Pi, &scan("a", 2, now), &c)
            .unwrap();
        c.probe.notifications.notify_pi = true;
        db.stage_native_notifications(NativeProvider::Pi, &scan("a", 3, now), &c)
            .unwrap();
        assert!(db.pending_native_deliveries(100).unwrap().is_empty());
        assert_eq!(
            db.stage_native_notifications(NativeProvider::Pi, &scan("a", 4, now), &c)
                .unwrap(),
            1
        );
        let old = scan("imported", 20, now - 86_400_000);
        assert_eq!(
            db.stage_native_notifications(NativeProvider::Pi, &old, &c)
                .unwrap(),
            0
        );
    }
}
