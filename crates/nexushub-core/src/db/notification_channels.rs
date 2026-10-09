//! Durable channel claims. Payloads needed for bounded retry are encrypted at rest.
use super::PanelDb;
use crate::config::Config;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    Bark,
    Gotify,
}

impl NotificationChannel {
    pub const ALL: [Self; 2] = [Self::Bark, Self::Gotify];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bark => "bark",
            Self::Gotify => "gotify",
        }
    }
    pub fn enabled(self, config: &Config) -> bool {
        match self {
            Self::Bark => config.probe.notifications.enabled,
            Self::Gotify => config.probe.notifications.gotify.enabled,
        }
    }
}

pub struct NotificationDelivery {
    pub event_key: String,
    pub channel: NotificationChannel,
    pub payload: Value,
}

impl PanelDb {
    pub(super) fn migrate_notification_channels(&self) -> Result<()> {
        let mut conn = self.conn.lock().expect("db mutex");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS probe_notification_channels (
                channel TEXT PRIMARY KEY, enabled INTEGER NOT NULL, activated_ms INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS probe_notification_deliveries (
                event_key TEXT NOT NULL, channel TEXT NOT NULL,
                ciphertext BLOB, nonce BLOB, status TEXT NOT NULL,
                outcome_json TEXT, attempts INTEGER NOT NULL DEFAULT 0,
                created_ms INTEGER NOT NULL, updated_ms INTEGER NOT NULL, next_ms INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY(event_key, channel)
             );
             CREATE INDEX IF NOT EXISTS idx_probe_notification_pending
                ON probe_notification_deliveries(status,next_ms);
             INSERT OR IGNORE INTO probe_notification_deliveries
                (event_key,channel,status,outcome_json,created_ms,updated_ms)
             SELECT 'native:' || event_key,'bark',CASE WHEN status='delivering' THEN 'unknown' ELSE status END,
                NULL,created_at*1000,updated_at*1000 FROM native_probe_deliveries
                WHERE status IN ('sent','skipped','delivering') AND NOT EXISTS(SELECT 1 FROM settings WHERE key='probe_channel_migration_v1');
             INSERT OR IGNORE INTO probe_notification_deliveries
                (event_key,channel,status,outcome_json,created_ms,updated_ms)
             SELECT key,'bark','legacy',NULL,updated_at*1000,updated_at*1000 FROM settings
                WHERE (key LIKE 'probe_async_question_delivery:%' OR key LIKE 'probe_feedback_delivery:%') AND NOT EXISTS(SELECT 1 FROM settings WHERE key='probe_channel_migration_v1');
             INSERT OR IGNORE INTO probe_notification_deliveries
                (event_key,channel,status,outcome_json,created_ms,updated_ms)
             SELECT 'codex:' || json_extract(payload_json,'$.dedupe.namespace') || ':' || dedupe_key,
                'bark','sent',NULL,created_at*1000,created_at*1000 FROM probe_events
                WHERE json_valid(payload_json) AND json_extract(payload_json,'$.bark.sent')=1
                AND json_extract(payload_json,'$.dedupe.namespace') IS NOT NULL AND dedupe_key IS NOT NULL
                AND NOT EXISTS(SELECT 1 FROM settings WHERE key='probe_channel_migration_v1');
             INSERT OR IGNORE INTO settings(key,value,updated_at) VALUES('probe_channel_migration_v1','1',unixepoch());"
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn sync_notification_channels(&self, config: &Config) -> Result<()> {
        let now = Utc::now().timestamp_millis();
        let mut conn = self.conn.lock().expect("db mutex");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for channel in NotificationChannel::ALL {
            let enabled = channel.enabled(config);
            let prior = tx
                .query_row(
                    "SELECT enabled,activated_ms FROM probe_notification_channels WHERE channel=?1",
                    [channel.as_str()],
                    |r| Ok((r.get::<_, bool>(0)?, r.get::<_, i64>(1)?)),
                )
                .optional()?;
            let activated = match prior {
                Some((false, _)) if enabled => now,
                Some((_, activated)) => activated,
                None if channel == NotificationChannel::Bark => 0, // Preserve the existing Bark baseline.
                None => now,
            };
            tx.execute("INSERT INTO probe_notification_channels(channel,enabled,activated_ms) VALUES(?1,?2,?3) ON CONFLICT(channel) DO UPDATE SET enabled=excluded.enabled,activated_ms=excluded.activated_ms", params![channel.as_str(),enabled,activated])?;
            if !enabled {
                tx.execute("UPDATE probe_notification_deliveries SET status='skipped',ciphertext=NULL,nonce=NULL,updated_ms=?2 WHERE channel=?1 AND status='pending'",params![channel.as_str(),now])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn stage_notification_delivery(
        &self,
        channel: NotificationChannel,
        key: &str,
        event_ms: i64,
        payload: &Value,
    ) -> Result<bool> {
        let (ciphertext, nonce) = self.crypto.encrypt(&serde_json::to_vec(payload)?)?;
        let now = Utc::now().timestamp_millis();
        let conn = self.conn.lock().expect("db mutex");
        let eligible = conn.query_row("SELECT enabled=1 AND ?2>=activated_ms FROM probe_notification_channels WHERE channel=?1", params![channel.as_str(),event_ms], |r|r.get::<_,bool>(0)).optional()?.unwrap_or(false);
        if !eligible {
            return Ok(false);
        }
        Ok(conn.execute("INSERT OR IGNORE INTO probe_notification_deliveries(event_key,channel,ciphertext,nonce,status,created_ms,updated_ms) VALUES(?1,?2,?3,?4,'pending',?5,?5)", params![key,channel.as_str(),ciphertext,nonce,now])? > 0)
    }

    pub fn claim_notification_delivery(
        &self,
        channel: NotificationChannel,
        key: &str,
    ) -> Result<bool> {
        let now = Utc::now().timestamp_millis();
        Ok(self.conn.lock().expect("db mutex").execute("UPDATE probe_notification_deliveries SET status='delivering',attempts=attempts+1,updated_ms=?3 WHERE event_key=?1 AND channel=?2 AND status='pending' AND next_ms<=?3 AND attempts<3",params![key,channel.as_str(),now])? == 1)
    }

    pub fn finish_notification_delivery(
        &self,
        channel: NotificationChannel,
        key: &str,
        outcome: &Value,
    ) -> Result<()> {
        let sent = outcome["sent"].as_bool() == Some(true);
        let skipped = outcome["skipped"].as_bool() == Some(true);
        let retryable = !sent
            && !skipped
            && outcome["request_count"].as_u64().unwrap_or(1) <= 1
            && matches!(
                outcome["http_status"].as_u64(),
                Some(429 | 500 | 502 | 503 | 504)
            );
        let uncertain = !sent && !skipped && outcome["http_status"].is_null();
        let status = if sent {
            "sent"
        } else if skipped {
            "skipped"
        } else if uncertain {
            "unknown"
        } else {
            "failed"
        };
        let now = Utc::now().timestamp_millis();
        let conn = self.conn.lock().expect("db mutex");
        conn.execute("UPDATE probe_notification_deliveries SET status=CASE WHEN ?4 AND attempts<3 THEN 'pending' ELSE ?3 END,outcome_json=?5,updated_ms=?6,next_ms=?6+60000,ciphertext=CASE WHEN ?4 AND attempts<3 THEN ciphertext ELSE NULL END,nonce=CASE WHEN ?4 AND attempts<3 THEN nonce ELSE NULL END WHERE event_key=?1 AND channel=?2 AND status='delivering'",params![key,channel.as_str(),status,retryable,outcome.to_string(),now])?;
        // Refresh channel feedback without adding duplicate business events.
        conn.execute("UPDATE probe_events SET payload_json=json_set(payload_json,?2,json(?3)) WHERE json_extract(payload_json,'$.notification_event_key')=?1",params![key,format!("$.{}",channel.as_str()),outcome.to_string()])?;
        Ok(())
    }

    pub fn due_notification_deliveries(&self, limit: usize) -> Result<Vec<NotificationDelivery>> {
        let now = Utc::now().timestamp_millis();
        let conn = self.conn.lock().expect("db mutex");
        conn.execute("UPDATE probe_notification_deliveries SET status='unknown',ciphertext=NULL,nonce=NULL,outcome_json=?1 WHERE status='delivering' AND updated_ms<?2",params![json!({"sent":false,"skipped":false,"reason":"delivery_interrupted_outcome_unknown"}).to_string(),now-120_000])?;
        let mut stmt=conn.prepare("SELECT event_key,channel,ciphertext,nonce FROM probe_notification_deliveries WHERE status='pending' AND next_ms<=?1 AND attempts<3 AND ciphertext IS NOT NULL ORDER BY created_ms,event_key,channel LIMIT ?2")?;
        let rows = stmt
            .query_map(params![now, limit.min(100)], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Vec<u8>>(2)?,
                    r.get::<_, Vec<u8>>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(event_key, channel, ciphertext, nonce)| {
                Ok(NotificationDelivery {
                    event_key,
                    channel: if channel == "bark" {
                        NotificationChannel::Bark
                    } else {
                        NotificationChannel::Gotify
                    },
                    payload: serde_json::from_slice(&self.crypto.decrypt(&ciphertext, &nonce)?)?,
                })
            })
            .collect()
    }

    pub fn notification_first_seen_ms(&self, dedupe_key: &str) -> Result<Option<i64>> {
        self.conn
            .lock()
            .expect("db mutex")
            .query_row(
                "SELECT min(created_at)*1000 FROM probe_events WHERE dedupe_key=?1",
                [dedupe_key],
                |r| r.get(0),
            )
            .map_err(Into::into)
    }

    pub fn notification_channel_status(&self) -> Result<Value> {
        let conn = self.conn.lock().expect("db mutex");
        let mut result = serde_json::Map::new();
        for channel in NotificationChannel::ALL {
            let mut stmt=conn.prepare("SELECT status,count(*) FROM probe_notification_deliveries WHERE channel=?1 GROUP BY status")?;
            let counts = stmt
                .query_map([channel.as_str()], |r| {
                    Ok((r.get::<_, String>(0)?, json!(r.get::<_, u64>(1)?)))
                })?
                .collect::<rusqlite::Result<serde_json::Map<String, Value>>>()?;
            result.insert(channel.as_str().into(), Value::Object(counts));
        }
        Ok(Value::Object(result))
    }
}
