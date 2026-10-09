//! One event, independent durable channel deliveries. No secrets enter queue payloads.
use super::{
    async_question_monitor, question_monitor, send_bark_notification,
    task_notification_suppression_reason, ProbeBarkOutcome, ProbeBarkRequest,
};
use anyhow::Result;
use chrono::Utc;
use nexushub_core::{
    codex,
    config::{valid_gotify_server_url, Config},
    db::{NativeDelivery, NotificationChannel, PanelDb},
    native_probe,
    probe::ProbeBuiltEvent,
    services::{
        probe as probe_service,
        settings::{PROBE_BARK_DEVICE_KEY_SETTING, PROBE_GOTIFY_TOKEN_SETTING},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, UNIX_EPOCH};

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
enum Source {
    Codex {
        event: Box<ProbeBuiltEvent>,
        fingerprint: Option<String>,
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
}

fn codex_fingerprint(config: &Config, event: &ProbeBuiltEvent) -> Option<String> {
    let paths = codex::resolve_codex_paths(&config.codex.home).codex_paths();
    let thread = codex::notification_thread_headers(&paths)
        .ok()?
        .into_iter()
        .find(|t| Some(&t.id) == event.thread_id.as_ref())?;
    let path = thread.rollout_path?;
    let identity = native_probe::file_identity(&path).ok()?;
    let metadata = std::fs::metadata(&path).ok()?;
    Some(format!(
        "{}:{}:{}",
        identity,
        metadata.len(),
        metadata
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_nanos()
    ))
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
) -> Result<(ProbeBarkOutcome, ProbeBarkOutcome)> {
    db.sync_notification_channels(config)?;
    let key = codex_key(event);
    let origin_ms = event.payload["question_created_at_ms"]
        .as_i64()
        .or_else(|| event.payload["feedback_completed_at_ms"].as_i64())
        .or_else(|| event.payload["source_log_ts"].as_i64().map(|ts| ts * 1000))
        .or_else(|| event.payload["notification_source_ms"].as_i64())
        .or(db.notification_first_seen_ms(&event.dedupe_key)?)
        .unwrap_or_else(|| Utc::now().timestamp_millis());
    let candidate = Candidate {
        source: Source::Codex {
            event: Box::new(event.clone()),
            fingerprint: codex_fingerprint(config, event),
        },
        request: ProbeBarkRequest {
            title: event.bark_title.clone(),
            body: event.bark_body.clone(),
            dedupe_key: event.dedupe_key.clone(),
        },
        event_ms: origin_ms,
    };
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
    let gotify = deliver(
        config,
        db,
        &key,
        NotificationChannel::Gotify,
        &candidate,
        true,
        timeout,
    )
    .await?;
    Ok((bark, gotify))
}

pub async fn native(
    config: &Config,
    db: &PanelDb,
    delivery: &NativeDelivery,
    timeout: Duration,
) -> Result<(ProbeBarkOutcome, ProbeBarkOutcome)> {
    db.sync_notification_channels(config)?;
    let label = match delivery.event.kind.as_str() {
        "reply_needed" => "需要回复",
        "completion" => "完成",
        _ => "失败",
    };
    let candidate = Candidate {
        source: Source::Native {
            delivery: delivery.clone(),
        },
        event_ms: delivery.event.timestamp_ms,
        request: ProbeBarkRequest {
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
    };
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
    let gotify = deliver(
        config,
        db,
        &key,
        NotificationChannel::Gotify,
        &candidate,
        true,
        timeout,
    )
    .await?;
    Ok((bark, gotify))
}

fn event_enabled(config: &Config, candidate: &Candidate) -> bool {
    match &candidate.source {
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
    db.stage_notification_delivery(
        channel,
        key,
        candidate.event_ms,
        &serde_json::to_value(candidate)?,
    )?;
    if !db.claim_notification_delivery(channel, key)? {
        return Ok(ProbeBarkOutcome::skipped("dedupe", enabled, true, true));
    }
    let outcome = send(config, db, channel, &candidate.request, timeout).await?;
    db.finish_notification_delivery(channel, key, &serde_json::to_value(&outcome)?)?;
    Ok(outcome)
}

async fn send(
    config: &Config,
    db: &PanelDb,
    channel: NotificationChannel,
    request: &ProbeBarkRequest,
    timeout: Duration,
) -> Result<ProbeBarkOutcome> {
    let secret = db.get_secret_setting_bytes(match channel {
        NotificationChannel::Bark => PROBE_BARK_DEVICE_KEY_SETTING,
        NotificationChannel::Gotify => PROBE_GOTIFY_TOKEN_SETTING,
    })?;
    let Some(secret) = secret.filter(|secret| !secret.is_empty()) else {
        return Ok(ProbeBarkOutcome::skipped(
            "device_key_missing",
            channel.enabled(config),
            true,
            false,
        ));
    };
    match channel {
        NotificationChannel::Bark => {
            send_bark_notification(config, &secret, request, timeout).await
        }
        NotificationChannel::Gotify => send_gotify(config, &secret, request, timeout).await,
    }
}

pub async fn gotify_test(config: &Config, db: &PanelDb) -> Result<ProbeBarkOutcome> {
    if !config.probe.notifications.gotify.enabled {
        return Ok(ProbeBarkOutcome::skipped(
            "notifications_disabled",
            false,
            true,
            false,
        ));
    }
    send(
        config,
        db,
        NotificationChannel::Gotify,
        &ProbeBarkRequest {
            title: "NexusHub · Gotify 测试".into(),
            body: "Gotify 推送通道测试。".into(),
            dedupe_key: uuid::Uuid::new_v4().to_string(),
        },
        Duration::from_secs(8),
    )
    .await
}

async fn send_gotify(
    config: &Config,
    secret: &[u8],
    request: &ProbeBarkRequest,
    timeout: Duration,
) -> Result<ProbeBarkOutcome> {
    let fail = |reason: &str| {
        ProbeBarkOutcome::failed_request(reason, true, true, true, Some(request.dedupe_key.clone()))
    };
    let Ok(token) = std::str::from_utf8(secret) else {
        return Ok(fail("invalid_token"));
    };
    let Ok(mut key) = reqwest::header::HeaderValue::from_bytes(token.trim().as_bytes()) else {
        return Ok(fail("invalid_token"));
    };
    key.set_sensitive(true);
    let server = config.probe.notifications.gotify.server_url.trim();
    if !valid_gotify_server_url(server) {
        return Ok(fail("invalid_server_url"));
    }
    let Ok(base) = reqwest::Url::parse(&format!("{}/", server.trim_end_matches('/'))) else {
        return Ok(fail("invalid_server_url"));
    };
    if !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
    {
        return Ok(fail("invalid_server_url"));
    }
    let Ok(url) = base.join("message") else {
        return Ok(fail("invalid_server_url"));
    };
    let Ok(client) = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(timeout)
        .build()
    else {
        return Ok(fail("client_build_error"));
    };
    let machine = if cfg!(target_os = "macos") {
        "本机"
    } else {
        "腾讯云"
    };
    let attempted = |mut outcome: ProbeBarkOutcome| {
        outcome.request_count = 1;
        outcome.chunk_count = 1;
        outcome
    };
    let response = client
        .post(url)
        .header("X-Gotify-Key", key)
        .json(&json!({
            "title":format!("{} · {}",machine,request.title),"message":request.body,
            "priority":config.probe.notifications.gotify.priority.min(10)
        }))
        .send()
        .await;
    let Ok(response) = response else {
        return Ok(attempted(fail("delivery_outcome_unknown")));
    };
    let status = response.status().as_u16();
    if !response.status().is_success() {
        return Ok(attempted(ProbeBarkOutcome::failed_status(
            status,
            true,
            true,
            true,
            Some(request.dedupe_key.clone()),
        )));
    }
    // Read a bounded acknowledgement. Never retain echoed notification content or tokens.
    let max_response_bytes = request
        .body
        .len()
        .saturating_add(request.title.len())
        .saturating_mul(6)
        .saturating_add(16 * 1024)
        .min(20 * 1024 * 1024);
    if response
        .content_length()
        .is_some_and(|size| size > max_response_bytes as u64)
    {
        return Ok(attempted(fail("response_invalid")));
    }
    let mut response = response;
    let mut bytes = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) if bytes.len() + chunk.len() <= max_response_bytes => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(None) => break,
            _ => return Ok(attempted(fail("response_invalid"))),
        }
    }
    let value = serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null);
    if value["id"].as_u64().is_none() {
        return Ok(attempted(fail("response_invalid")));
    }
    Ok(attempted(ProbeBarkOutcome::sent(
        status,
        true,
        true,
        true,
        Some(request.dedupe_key.clone()),
    )))
}

#[cfg(test)]
#[path = "notification_delivery_tests.rs"]
mod tests;

async fn current(config: &Config, db: &PanelDb, candidate: &Candidate) -> bool {
    match &candidate.source {
        Source::Native { delivery } => {
            let provider = delivery.provider;
            tokio::task::spawn_blocking(move || native_probe::scan(provider))
                .await
                .ok()
                .and_then(Result::ok)
                .is_some_and(|scan| {
                    scan.streams.iter().any(|stream| {
                        stream.session_key == delivery.session_key
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
                fingerprint.is_some() && *fingerprint == codex_fingerprint(config, &event)
            }
        }
    }
}

pub async fn retry_pending(config: &Config, db: &PanelDb) -> Result<()> {
    db.sync_notification_channels(config)?;
    for job in db.due_notification_deliveries(100)? {
        let candidate = serde_json::from_value::<Candidate>(job.payload)?;
        if !db.claim_notification_delivery(job.channel, &job.event_key)? {
            continue;
        }
        let outcome = if !job.channel.enabled(config)
            || !event_enabled(config, &candidate)
            || !current(config, db, &candidate).await
        {
            ProbeBarkOutcome::skipped(
                "source_changed_or_disabled",
                job.channel.enabled(config),
                false,
                false,
            )
        } else {
            send(
                config,
                db,
                job.channel,
                &candidate.request,
                Duration::from_secs(8),
            )
            .await?
        };
        db.finish_notification_delivery(
            job.channel,
            &job.event_key,
            &serde_json::to_value(outcome)?,
        )?;
    }
    Ok(())
}
