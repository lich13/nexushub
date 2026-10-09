//! Durable Bark delivery claims and bounded retries; credentials stay outside the queue.
use super::{
    async_question_monitor, bark, bark_body_chunks, question_monitor,
    task_notification_suppression_reason, ProbeBarkOutcome, ProbeBarkRequest,
    PROBE_BARK_BODY_CHUNK_BYTES,
};
use anyhow::Result;
use chrono::Utc;
use nexushub_core::{
    codex,
    config::Config,
    db::{NativeDelivery, NotificationChannel, PanelDb},
    native_probe,
    probe::ProbeBuiltEvent,
    services::{probe as probe_service, settings::PROBE_BARK_DEVICE_KEY_SETTING},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
enum Source {
    Test,
    Codex {
        event: Box<ProbeBuiltEvent>,
        fingerprint: Option<SourceProof>,
    },
    Native {
        delivery: NativeDelivery,
    },
}

#[derive(Clone, Serialize, Deserialize)]
struct Candidate {
    source: Source,
    request: ProbeBarkRequest,
    event_ms: i64,
    chunks: Vec<String>,
    instance_id: String,
    target_fingerprint: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct SourceProof {
    identity: String,
    length: u64,
    digest: String,
}

fn codex_path(config: &Config, event: &ProbeBuiltEvent) -> Option<PathBuf> {
    let paths = codex::resolve_codex_paths(&config.codex.home).codex_paths();
    codex::notification_thread_headers(&paths)
        .ok()?
        .into_iter()
        .find(|t| Some(&t.id) == event.thread_id.as_ref())?
        .rollout_path
}

fn prefix_digest(path: &Path, length: u64) -> Option<String> {
    if length > 128 * 1024 * 1024 {
        return None;
    }
    let mut file = std::fs::File::open(path).ok()?.take(length);
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 32768];
    let mut count = 0;
    loop {
        let size = file.read(&mut buffer).ok()?;
        if size == 0 {
            break;
        }
        count += size as u64;
        digest.update(&buffer[..size]);
    }
    (count == length).then(|| hex::encode(digest.finalize()))
}

fn codex_fingerprint(config: &Config, event: &ProbeBuiltEvent) -> Option<SourceProof> {
    let path = codex_path(config, event)?;
    let identity = native_probe::file_identity(&path).ok()?;
    let length = std::fs::metadata(&path).ok()?.len();
    let digest = prefix_digest(&path, length)?;
    Some(SourceProof {
        identity,
        length,
        digest,
    })
}

fn same_source(path: &Path, proof: &SourceProof) -> bool {
    native_probe::file_identity(path).ok().as_deref() == Some(&proof.identity)
        && prefix_digest(path, proof.length).as_deref() == Some(&proof.digest)
}

fn target_fingerprint(config: &Config, secret: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(config.probe.notifications.server_url.trim().as_bytes());
    hash.update([0]);
    hash.update(secret);
    hex::encode(hash.finalize())
}

fn queued_candidate(
    config: &Config,
    db: &PanelDb,
    source: Source,
    request: ProbeBarkRequest,
    event_ms: i64,
) -> Result<Candidate> {
    let secret = db
        .get_secret_setting_bytes(PROBE_BARK_DEVICE_KEY_SETTING)?
        .unwrap_or_default();
    Ok(Candidate {
        source,
        chunks: bark_body_chunks(&request.body, PROBE_BARK_BODY_CHUNK_BYTES),
        request,
        event_ms,
        instance_id: db.notification_instance_id()?,
        target_fingerprint: target_fingerprint(config, &secret),
    })
}

pub fn codex_key(event: &ProbeBuiltEvent) -> String {
    async_question_monitor::delivery_key(event)
        .or_else(|| question_monitor::delivery_key(event))
        .unwrap_or_else(|| format!("codex:{}:{}", event.dedupe_namespace, event.dedupe_key))
}

pub async fn codex(
    config: &Config,
    db: &PanelDb,
    event: &ProbeBuiltEvent,
    bark_allowed: bool,
    timeout: Duration,
) -> Result<ProbeBarkOutcome> {
    db.sync_notification_channels(config)?;
    let key = codex_key(event);
    let origin_ms = event.payload["question_created_at_ms"]
        .as_i64()
        .or_else(|| event.payload["feedback_completed_at_ms"].as_i64())
        .or_else(|| event.payload["source_log_ts"].as_i64().map(|ts| ts * 1000))
        .or_else(|| event.payload["notification_source_ms"].as_i64())
        .or(db.notification_first_seen_ms(&event.dedupe_key)?)
        .unwrap_or_else(|| Utc::now().timestamp_millis());
    let candidate = queued_candidate(
        config,
        db,
        Source::Codex {
            event: Box::new(event.clone()),
            fingerprint: codex_fingerprint(config, event),
        },
        ProbeBarkRequest {
            title: event.bark_title.clone(),
            body: event.bark_body.clone(),
            dedupe_key: event.dedupe_key.clone(),
        },
        origin_ms,
    )?;
    let bark = deliver(
        config,
        db,
        &key,
        NotificationChannel::Bark,
        &candidate,
        bark_allowed,
        timeout,
    )
    .await?;
    Ok(bark)
}

pub async fn native(
    config: &Config,
    db: &PanelDb,
    delivery: &NativeDelivery,
    timeout: Duration,
) -> Result<ProbeBarkOutcome> {
    db.sync_notification_channels(config)?;
    let label = match delivery.event.kind.as_str() {
        "reply_needed" => "需要回复",
        "completion" => "完成",
        _ => "失败",
    };
    let candidate = queued_candidate(
        config,
        db,
        Source::Native {
            delivery: delivery.clone(),
        },
        ProbeBarkRequest {
            title: format!(
                "{} · {} · {}",
                delivery.provider.as_str(),
                label,
                delivery.title
            ),
            body: format!(
                "{}\n\n线程 ID：{}\n回合：{}",
                delivery.event.body, delivery.thread_id, delivery.event.turn_id
            ),
            dedupe_key: delivery.event_key.clone(),
        },
        delivery.event.timestamp_ms,
    )?;
    let key = format!("native:{}", delivery.event_key);
    let bark = deliver(
        config,
        db,
        &key,
        NotificationChannel::Bark,
        &candidate,
        true,
        timeout,
    )
    .await?;
    Ok(bark)
}

fn event_enabled(config: &Config, candidate: &Candidate) -> bool {
    match &candidate.source {
        Source::Test => true,
        Source::Codex { event, .. } => {
            config.probe.enabled
                && probe_service::probe_event_bark_switch_enabled(config, &event.kind)
        }
        Source::Native { delivery } => {
            delivery.provider.enabled(config)
                && delivery
                    .provider
                    .event_enabled(config, &delivery.event.kind)
        }
    }
}

async fn deliver(
    config: &Config,
    db: &PanelDb,
    key: &str,
    channel: NotificationChannel,
    candidate: &Candidate,
    legacy_allowed: bool,
    timeout: Duration,
) -> Result<ProbeBarkOutcome> {
    let enabled = channel.enabled(config);
    if !enabled || !event_enabled(config, candidate) {
        return Ok(ProbeBarkOutcome::skipped(
            if enabled {
                "event_switch_disabled"
            } else {
                "notifications_disabled"
            },
            enabled,
            event_enabled(config, candidate),
            false,
        ));
    }
    if !legacy_allowed {
        return Ok(ProbeBarkOutcome::skipped("dedupe", enabled, true, true));
    }
    let staged = db.stage_notification_delivery(
        channel,
        key,
        candidate.event_ms,
        &serde_json::to_value(candidate)?,
    )?;
    if !staged {
        return Ok(db
            .notification_delivery_outcome(channel, key)?
            .and_then(|v| serde_json::from_value(v).ok())
            .filter(|v: &ProbeBarkOutcome| !v.sent && !v.skipped)
            .unwrap_or_else(|| ProbeBarkOutcome::skipped("dedupe", enabled, true, true)));
    }
    process(
        &|| Ok(config.clone()),
        db,
        key,
        channel,
        candidate,
        timeout,
        false,
    )
    .await
}

type ConfigLoader<'a> = dyn Fn() -> Result<Config> + Sync + 'a;

fn latest_config(load_config: &ConfigLoader<'_>) -> Result<Config> {
    // Do not expose configuration paths or values in monitor/job errors.
    load_config().map_err(|_| anyhow::anyhow!("notification configuration unavailable"))
}

async fn process(
    load_config: &ConfigLoader<'_>,
    db: &PanelDb,
    key: &str,
    channel: NotificationChannel,
    candidate: &Candidate,
    timeout: Duration,
    revalidate: bool,
) -> Result<ProbeBarkOutcome> {
    let started = Instant::now();
    let mut latest = None;
    loop {
        let checked_config = latest_config(load_config)?;
        db.sync_notification_channels(&checked_config)?;
        let Some(lease) = db.claim_notification_delivery(channel, key)? else {
            return Ok(db
                .notification_delivery_outcome(channel, key)?
                .and_then(|v| serde_json::from_value(v).ok())
                .or(latest)
                .unwrap_or_else(|| {
                    ProbeBarkOutcome::skipped(
                        "dedupe",
                        channel.enabled(&checked_config),
                        true,
                        true,
                    )
                }));
        };
        let valid_source = !(revalidate || lease.chunk_attempts > 1)
            || current(&checked_config, db, candidate).await;
        // Source checks can yield while a scan runs. Refresh again before any HTTP
        // request, including another segment of the same queued notification.
        let config = latest_config(load_config)?;
        db.sync_notification_channels(&config)?;
        let valid_source = valid_source
            && (!matches!(&candidate.source, Source::Codex { .. })
                || config.codex.home == checked_config.codex.home);
        let config = &config;
        let secret = db
            .get_secret_setting_bytes(PROBE_BARK_DEVICE_KEY_SETTING)?
            .unwrap_or_default();
        let valid_target = candidate.target_fingerprint == target_fingerprint(config, &secret);
        let outcome = if !channel.enabled(config)
            || !event_enabled(config, candidate)
            || !valid_target
            || !valid_source
        {
            ProbeBarkOutcome::skipped(
                "source_changed_or_disabled",
                channel.enabled(config),
                false,
                !secret.is_empty(),
            )
        } else if secret.is_empty() {
            ProbeBarkOutcome::skipped("device_key_missing", channel.enabled(config), true, false)
        } else if let Some(chunk) = candidate.chunks.get(lease.chunk_index) {
            let Some(remaining_ms) = db.notification_delivery_lease_remaining_ms(&lease)? else {
                return Ok(db
                    .notification_delivery_outcome(channel, key)?
                    .and_then(|value| serde_json::from_value(value).ok())
                    .unwrap_or_else(|| {
                        ProbeBarkOutcome::skipped(
                            "delivery_lease_expired",
                            channel.enabled(config),
                            true,
                            true,
                        )
                    }));
            };
            let id = hex::encode(Sha256::digest(format!(
                "{}\0{}\0{}",
                candidate.instance_id, key, lease.chunk_index
            )));
            bark::send_chunk(
                config,
                &secret,
                &candidate.request,
                chunk,
                lease.chunk_index,
                candidate.chunks.len(),
                &id,
                lease.chunk_attempts,
                timeout
                    .saturating_sub(started.elapsed())
                    .max(Duration::from_millis(1))
                    .min(Duration::from_millis(remaining_ms as u64)),
            )
            .await
        } else {
            ProbeBarkOutcome::skipped(
                "invalid_retry_context",
                channel.enabled(config),
                false,
                true,
            )
        };
        let chunk_accepted = outcome.sent;
        let mut confirmation = serde_json::to_value(&outcome)?;
        confirmation["retryable"] = json!(outcome.retryable);
        confirmation["retry_after_ms"] = json!(outcome.retry_after_ms);
        confirmation["uncertain"] = json!(outcome.uncertain);
        let Some(value) = db.finish_notification_delivery(&lease, &confirmation)? else {
            return Ok(db
                .notification_delivery_outcome(channel, key)?
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or_else(|| {
                    ProbeBarkOutcome::skipped(
                        "delivery_lease_expired",
                        channel.enabled(config),
                        true,
                        true,
                    )
                }));
        };
        let result: ProbeBarkOutcome = serde_json::from_value(value)?;
        if result.status.as_deref() != Some("waiting_retry")
            || !chunk_accepted
            || started.elapsed() >= timeout
        {
            return Ok(result);
        }
        latest = Some(result);
    }
}

/// Explicit user tests get a fresh identity and use the same durable queue.
pub async fn test(config_path: &Path, db: &PanelDb) -> Result<ProbeBarkOutcome> {
    let config = Config::load(config_path)?;
    db.sync_notification_channels(&config)?;
    let key = format!("test:{}", uuid::Uuid::new_v4());
    let candidate = queued_candidate(
        &config,
        db,
        Source::Test,
        ProbeBarkRequest {
            title: "NexusHub 推送测试".into(),
            body: nexushub_core::probe::bark_test_body().into(),
            dedupe_key: key.clone(),
        },
        Utc::now().timestamp_millis(),
    )?;
    let mut outcome = deliver(
        &config,
        db,
        &key,
        NotificationChannel::Bark,
        &candidate,
        true,
        Duration::from_secs(8),
    )
    .await?;
    let mut shown_attempt = 0;
    while matches!(
        outcome.status.as_deref(),
        Some("waiting_retry" | "retrying")
    ) {
        if shown_attempt != outcome.attempts {
            shown_attempt = outcome.attempts;
            println!("等待重试，已尝试 {} 次。", shown_attempt);
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
        retry_pending_with_config(db, &|| Config::load(config_path)).await?;
        if let Some(value) = db.notification_delivery_outcome(NotificationChannel::Bark, &key)? {
            outcome = serde_json::from_value(value)?;
        }
    }
    Ok(outcome)
}

#[cfg(test)]
#[path = "notification_delivery_tests.rs"]
mod tests;

async fn current(config: &Config, db: &PanelDb, candidate: &Candidate) -> bool {
    match &candidate.source {
        Source::Test => true,
        Source::Native { delivery } => {
            let provider = delivery.provider;
            tokio::task::spawn_blocking(move || native_probe::scan(provider))
                .await
                .ok()
                .and_then(Result::ok)
                .is_some_and(|scan| {
                    scan.streams.iter().any(|stream| {
                        native_probe::retry_event_is_current(
                            provider,
                            &delivery.thread_id,
                            &delivery.event,
                        ) && stream.session_key == delivery.session_key
                            && stream.id == delivery.thread_id
                            && stream.events.iter().any(|event| {
                                stream.event_key(event) == delivery.event_key
                                    && event == &delivery.event
                            })
                    })
                })
        }
        Source::Codex { event, fingerprint } => {
            if task_notification_suppression_reason(config, event.thread_id.as_deref()).is_some() {
                return false;
            }
            let (Some(proof), Some(path)) = (fingerprint, codex_path(config, event)) else {
                return false;
            };
            if !same_source(&path, proof) {
                return false;
            }
            let mut event = (**event).clone();
            event.bark_body = candidate.request.body.clone();
            if event.payload["question_tool"] == "request_user_input_async"
                || event.payload["body_source"] == "assistant_question"
            {
                async_question_monitor::prepare_event(config, db, event)
                    .and_then(|e| question_monitor::prepare_event(config, db, e))
                    .is_ok_and(|e| {
                        e.suppression_reason.is_none() && event_enabled(config, candidate)
                    })
            } else {
                if event.payload["body_source"] == "request_user_input"
                    || event.payload["body_source"] == "proposed_plan"
                {
                    let Some(path) = codex_path(config, &event) else {
                        return false;
                    };
                    let Ok(Some(selection)) =
                        codex::rollout_hook_stop_message_selection(&path, None)
                    else {
                        return false;
                    };
                    if selection.selected_turn_id != event.turn_id
                        || !matches!(
                            selection.source.as_str(),
                            "request_user_input" | "proposed_plan"
                        )
                    {
                        return false;
                    }
                }
                let (Some(proof), Some(path)) = (fingerprint, codex_path(config, &event)) else {
                    return false;
                };
                if !same_source(&path, proof) {
                    return false;
                }
                // Only append-only, unrelated records may leave an older notification current.
                let Ok(mut file) = std::fs::File::open(&path) else {
                    return false;
                };
                if file.seek(SeekFrom::Start(proof.length)).is_err() {
                    return false;
                }
                let mut tail = String::new();
                if file
                    .take(2 * 1024 * 1024 + 1)
                    .read_to_string(&mut tail)
                    .is_err()
                    || tail.len() > 2 * 1024 * 1024
                    || (!tail.is_empty() && !tail.ends_with('\n'))
                {
                    return false;
                }
                tail.lines()
                    .filter(|line| !line.trim().is_empty())
                    .all(|line| {
                        let Ok(row) = serde_json::from_str::<Value>(line) else {
                            return false;
                        };
                        let payload = row.get("payload").unwrap_or(&row);
                        let kind = payload["type"]
                            .as_str()
                            .or(row["type"].as_str())
                            .unwrap_or("");
                        let outer = row["type"].as_str().unwrap_or("");
                        let turn = payload["turn_id"].as_str().or(payload["turnId"].as_str());
                        !matches!(
                            kind,
                            "task_started"
                                | "turn_started"
                                | "turn/started"
                                | "turn_aborted"
                                | "turn_cancelled"
                                | "user_message"
                                | "function_call"
                                | "custom_tool_call"
                        ) && payload["role"] != "user"
                            && !(outer == "turn_context"
                                && turn.is_some_and(|id| Some(id) != event.turn_id.as_deref()))
                    })
            }
        }
    }
}

pub async fn retry_pending_with_config(db: &PanelDb, load_config: &ConfigLoader<'_>) -> Result<()> {
    db.sync_notification_channels(&latest_config(load_config)?)?;
    for job in db.due_notification_deliveries(10)? {
        let candidate = match serde_json::from_value::<Candidate>(job.payload) {
            Ok(candidate) => candidate,
            Err(_) => {
                if let Some(lease) = db.claim_notification_delivery(job.channel, &job.event_key)? {
                    db.finish_notification_delivery(
                        &lease,
                        &json!({"sent":false,"skipped":true,"reason":"invalid_retry_context"}),
                    )?;
                }
                continue;
            }
        };
        process(
            load_config,
            db,
            &job.event_key,
            job.channel,
            &candidate,
            Duration::from_secs(8),
            true,
        )
        .await?;
    }
    Ok(())
}

#[cfg(test)]
async fn retry_pending(config: &Config, db: &PanelDb) -> Result<()> {
    retry_pending_with_config(db, &|| Ok(config.clone())).await
}
