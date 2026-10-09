//! Background detection and final validation of native asynchronous questions.
use super::{probe_runtime, record_probe_event_with_bark, task_notification_suppression_reason};
use anyhow::Result;
use nexushub_core::{
    codex::{self, AsyncQuestionCall, ThreadSummary},
    config::Config,
    db::PanelDb,
    probe::{ProbeBuiltEvent, ProbeEventInput},
    services::probe::format_probe_pending_elicitation,
};
use serde_json::json;
use sha2::{Digest, Sha256};

fn enabled(config: &Config) -> bool {
    config.probe.enabled
        && config.probe.notifications.any_channel_enabled()
        && config.probe.notifications.notify_codex
        && config.probe.notifications.notify_reply_needed
}

fn event_for(
    config: &Config,
    thread: &ThreadSummary,
    call: &AsyncQuestionCall,
) -> Option<ProbeBuiltEvent> {
    let body = format_probe_pending_elicitation(&call.pending()?);
    let mut event = probe_runtime(config).build_event(
        ProbeEventInput::hook_stop_with_context(
            Some(&thread.id),
            Some(&call.turn_id),
            Some(&thread.id),
            thread.rollout_path.as_deref().and_then(|p| p.to_str()),
            Some(&body),
            "reply-needed",
        )
        .with_thread_title(Some(&thread.title))
        .with_body_source(Some("request_user_input"))
        .with_call_id(Some(&call.call_id))
        .with_passive_scan_source(),
    );
    event.payload["question_tool"] = json!("request_user_input_async");
    event.payload["question_content_hash"] = json!(call.content_hash);
    event.payload["question_created_at_ms"] = json!(call.created_at_ms);
    Some(event)
}

pub fn delivery_key(event: &ProbeBuiltEvent) -> Option<String> {
    if event.payload["question_tool"] != "request_user_input_async" {
        return None;
    }
    let identity = format!(
        "{}:{}:{}:{}",
        event.thread_id.as_deref()?,
        event.turn_id.as_deref()?,
        event.payload["call_id"].as_str()?,
        event.payload["question_content_hash"].as_str()?
    );
    Some(format!(
        "probe_async_question_delivery:{}",
        hex::encode(Sha256::digest(identity.as_bytes()))
    ))
}

pub fn prepare_event(
    config: &Config,
    db: &PanelDb,
    event: ProbeBuiltEvent,
) -> Result<ProbeBuiltEvent> {
    if !matches!(
        event.event_type.as_str(),
        "completion" | "hook_stop" | "reply_needed"
    ) {
        return Ok(event);
    }
    if event.payload["body_source"] == "proposed_plan" {
        return Ok(event);
    }
    let mut unconfirmed = event.clone();
    if event.payload["question_tool"]
        .as_str()
        .is_some_and(codex::is_async_question_tool)
    {
        unconfirmed.suppression_reason = Some("unconfirmed_question_transcript".into());
    }
    let Some(id) = event.thread_id.as_deref() else {
        return Ok(event);
    };
    let paths = codex::resolve_codex_paths(&config.codex.home).codex_paths();
    let Ok(threads) = codex::notification_thread_headers(&paths) else {
        return Ok(unconfirmed);
    };
    let Some(thread) = threads.into_iter().find(|t| t.id == id) else {
        return Ok(unconfirmed);
    };
    let Some(path) = thread.rollout_path.as_deref() else {
        return Ok(unconfirmed);
    };
    let calls = match codex::rollout_async_questions(path) {
        Ok(calls) => calls,
        Err(_) => {
            let mut event = event;
            // A newly appended answer must never be ignored by a stale sender.
            if event.payload["body_source"] == "request_user_input" {
                event.suppression_reason = Some("unconfirmed_question_transcript".into());
            }
            return Ok(event);
        }
    };
    let call_id = event.payload["call_id"]
        .as_str()
        .or_else(|| event.payload["item_id"].as_str());
    let candidate = calls.iter().find(|c| {
        event
            .turn_id
            .as_deref()
            .is_none_or(|turn| turn == c.turn_id)
            && match call_id {
                Some(id) => id == c.call_id,
                None => c.pending().is_some(),
            }
    });
    let Some(call) = candidate else {
        return Ok(unconfirmed);
    };
    if call.session_id.as_deref() != Some(id) {
        let mut event = event;
        event.suppression_reason = Some("question_transcript_identity_mismatch".into());
        return Ok(event);
    }
    let since = db.notification_baseline("probe_async_question_baseline", enabled(config))?;
    let eligible = since
        .zip(call.created_at_ms)
        .is_some_and(|(since, created)| {
            created > since && created <= chrono::Utc::now().timestamp_millis()
        });
    if !eligible || call.pending().is_none() {
        let mut event = event;
        event.suppression_reason = Some(
            if call.pending().is_none() {
                "question_already_resolved"
            } else {
                "question_before_baseline_or_disabled"
            }
            .into(),
        );
        return Ok(event);
    }
    let mut prepared = event_for(config, &thread, call).expect("pending question checked");
    if let Some(source) = event.payload.get("scan_source") {
        prepared.payload["scan_source"] = source.clone();
    }
    Ok(prepared)
}

pub async fn run(config: &Config, db: &PanelDb) -> Result<()> {
    let Some(since) = db.notification_baseline("probe_async_question_baseline", enabled(config))?
    else {
        return Ok(());
    };
    let paths = codex::resolve_codex_paths(&config.codex.home).codex_paths();
    let mut threads = codex::notification_thread_headers(&paths)?;
    threads.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    for thread in threads
        .into_iter()
        .take(config.probe.recent_limit.clamp(1, 200))
    {
        if task_notification_suppression_reason(config, Some(&thread.id)).is_some() {
            continue;
        }
        let Some(path) = thread.rollout_path.as_deref() else {
            continue;
        };
        let Ok(calls) = codex::rollout_async_questions(path) else {
            continue;
        };
        for call in calls
            .iter()
            .filter(|c| c.created_at_ms.is_some_and(|created| created > since))
        {
            let Some(event) = event_for(config, &thread, call) else {
                continue;
            };
            if delivery_key(&event)
                .map(|key| db.get_setting(&key))
                .transpose()?
                .flatten()
                .is_some()
            {
                continue;
            }
            record_probe_event_with_bark(config, db, event).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
