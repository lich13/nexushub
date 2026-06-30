use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use super::{
    extract_proposed_plan_text, session_index::SessionIndexEntry, CodexMessage, MessageBlock,
    PendingElicitation, ThreadDetail, ThreadSummary, UserInputAnswer, UserInputOption,
    UserInputQuestion,
};

pub fn thread_detail_from_summary(summary: ThreadSummary) -> Result<ThreadDetail> {
    let mut messages = Vec::new();
    let mut block_builder = MessageBlockBuilder::default();
    let mut raw_event_count = 0;
    if let Some(path) = &summary.rollout_path {
        let text =
            fs::read_to_string(path).with_context(|| format!("read rollout {}", path.display()))?;
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            raw_event_count += 1;
            let Ok(value) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            if let Some(message) = parse_message_event(&value) {
                messages.push(message);
            }
            block_builder.push_event(&value, raw_event_count);
        }
    }
    let blocks = block_builder.finish();
    let total_blocks = blocks.len();
    Ok(ThreadDetail {
        summary,
        messages,
        blocks,
        raw_event_count,
        total_blocks,
        has_more_blocks: false,
        before_cursor: None,
    })
}

pub fn window_thread_detail(
    mut detail: ThreadDetail,
    limit: Option<usize>,
    before: Option<&str>,
) -> ThreadDetail {
    let total = detail.blocks.len();
    detail.total_blocks = total;
    let Some(limit) = limit.filter(|value| *value > 0) else {
        detail.has_more_blocks = false;
        detail.before_cursor = None;
        return detail;
    };
    let end = before
        .and_then(|cursor| cursor.strip_prefix("b:"))
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(total)
        .min(total);
    let start = end.saturating_sub(limit);
    detail.blocks = detail.blocks[start..end].to_vec();
    detail.messages.clear();
    detail.has_more_blocks = start > 0;
    detail.before_cursor = (start > 0).then(|| format!("b:{start}"));
    detail
}

pub fn message_blocks_from_events<'a>(
    events: impl IntoIterator<Item = &'a Value>,
) -> Vec<MessageBlock> {
    let mut block_builder = MessageBlockBuilder::default();
    for (index, value) in events.into_iter().enumerate() {
        block_builder.push_event(value, index + 1);
    }
    block_builder.finish()
}

pub fn is_macos_network_volume_path(path: &Path) -> bool {
    #[cfg(target_os = "macos")]
    {
        if path.starts_with("/Volumes") {
            return true;
        }
        let mut current = Some(path);
        while let Some(candidate) = current {
            if candidate.exists() {
                return fs::canonicalize(candidate)
                    .map(|canonical| canonical.starts_with("/Volumes"))
                    .unwrap_or(true);
            }
            current = candidate.parent();
        }
        false
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        false
    }
}

#[derive(Default)]
pub(crate) struct RolloutScan {
    pub(crate) message_count: usize,
    pub(crate) latest_message: Option<String>,
    pub(crate) reply_needed: bool,
    pub(crate) recoverable: bool,
    pub(crate) running: bool,
    pub(crate) cwd: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) first_user_message_title: Option<String>,
    pub(crate) active_turn_id: Option<String>,
    pub(crate) pending_elicitation: Option<PendingElicitation>,
    pub(crate) last_event_kind: Option<String>,
    pub(crate) is_subagent: bool,
}

#[derive(Debug, Clone)]
enum PendingAction {
    Plan {
        turn_id: Option<String>,
        item_id: Option<String>,
    },
    Elicitation {
        turn_id: Option<String>,
        item_id: Option<String>,
        call_id: Option<String>,
        elicitation: PendingElicitation,
    },
}

#[derive(Debug, Clone)]
struct PendingHookStopAction {
    action: PendingAction,
    message: String,
    source: &'static str,
    turn_id: Option<String>,
    line: usize,
}

#[derive(Debug, Clone)]
pub struct RolloutMessageSelection {
    pub message: String,
    pub source: String,
    pub strategy: String,
    pub selected_turn_id: Option<String>,
    pub selected_line: Option<usize>,
    pub candidate_count: usize,
}

pub(crate) fn scan_rollout(path: &Path, max_messages: usize) -> Result<RolloutScan> {
    let text =
        fs::read_to_string(path).with_context(|| format!("read rollout {}", path.display()))?;
    let mut scan = RolloutScan::default();
    let mut pending_action: Option<PendingAction> = None;
    let mut current_plan_marker: Option<(Option<String>, Option<String>)> = None;
    let mut last_task_status: Option<String> = None;
    let mut active_tasks: Vec<Option<String>> = Vec::new();
    let mut pending_tool_turns: HashMap<String, Option<String>> = HashMap::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(payload) = value.get("session_meta").and_then(|v| v.get("payload")) {
            scan.is_subagent |= is_subagent_session_meta(payload);
            scan.cwd = payload
                .get("cwd")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or(scan.cwd);
            scan.model = payload
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or(scan.model);
            if scan.title.is_none() {
                scan.title = rollout_metadata_title(payload);
            }
        }
        let event_type = rollout_event_type(&value);
        if !event_type.is_empty() {
            scan.last_event_kind = Some(event_type.to_string());
        }
        update_turn_state(&value, &mut scan);
        if is_turn_terminal_event(event_type) {
            let mut active_tasks_changed = false;
            let completed_turn_id = event_turn_id(&value);
            if let Some(turn_id) = &completed_turn_id {
                if let Some(index) = active_tasks
                    .iter()
                    .position(|active| active.as_deref() == Some(turn_id.as_str()))
                {
                    active_tasks.drain(..=index);
                    active_tasks_changed = true;
                }
            } else if !active_tasks.is_empty() {
                active_tasks.clear();
                active_tasks_changed = true;
            }
            if active_tasks_changed {
                scan.active_turn_id = latest_active_task_turn(&active_tasks);
            }
            clear_pending_tools_for_turn(&mut pending_tool_turns, completed_turn_id.as_deref());
            if scan.active_turn_id.is_none() {
                scan.recoverable = false;
            }
        }
        if event_type == "task_started" {
            scan.recoverable = false;
            if let Some(turn_id) = event_turn_id(&value) {
                scan.active_turn_id = Some(turn_id.clone());
                if !active_tasks
                    .iter()
                    .any(|active| active.as_deref() == Some(turn_id.as_str()))
                {
                    active_tasks.push(Some(turn_id));
                }
            } else {
                active_tasks.push(None);
            }
            scan.running = true;
        }
        if event_type == "item_completed"
            && value
                .get("item")
                .and_then(|item| item.get("type"))
                .and_then(Value::as_str)
                == Some("Plan")
        {
            current_plan_marker = Some((event_turn_id(&value), event_item_id(&value)));
        }
        if event_type == "task_complete" {
            let completed_turn_id = event_turn_id(&value);
            if let Some(turn_id) = completed_turn_id.as_deref() {
                if let Some(index) = active_tasks
                    .iter()
                    .position(|active| active.as_deref() == Some(turn_id))
                {
                    active_tasks.drain(..=index);
                }
                scan.active_turn_id = latest_active_task_turn(&active_tasks);
            } else {
                if let Some(index) = active_tasks.iter().rposition(Option::is_none) {
                    active_tasks.remove(index);
                    scan.active_turn_id = latest_active_task_turn(&active_tasks);
                }
            }
            let payload = value.get("payload").unwrap_or(&value);
            last_task_status = value
                .get("status")
                .and_then(Value::as_str)
                .or_else(|| payload.get("status").and_then(Value::as_str))
                .or_else(|| value.get("turn_status").and_then(Value::as_str))
                .or_else(|| payload.get("turn_status").and_then(Value::as_str))
                .map(str::to_string);
            let last_agent_null = value
                .get("last_agent_message")
                .or_else(|| payload.get("last_agent_message"))
                .map(Value::is_null)
                .unwrap_or(false);
            let last_agent_message = value
                .get("last_agent_message")
                .or_else(|| payload.get("last_agent_message"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|message| !message.is_empty());
            if let Some(turn_id) = completed_turn_id.as_deref() {
                clear_pending_tools_for_turn(&mut pending_tool_turns, Some(turn_id));
            }
            if last_agent_message.is_some() && scan.active_turn_id.is_none() {
                clear_anonymous_pending_tools(&mut pending_tool_turns);
            }
            if last_agent_null {
                if let Some((turn_id, item_id)) =
                    plan_marker_for_event(&current_plan_marker, &value)
                {
                    pending_action = Some(PendingAction::Plan { turn_id, item_id });
                }
            }
            if last_agent_message.is_some() {
                scan.recoverable = false;
            } else if last_agent_null
                && last_task_status
                    .as_deref()
                    .is_some_and(is_recoverable_terminal_status)
            {
                scan.recoverable = true;
            }
            current_plan_marker = None;
        }
        if is_request_user_input(&value) {
            if let Some(elicitation) = parse_pending_elicitation(&value) {
                pending_action = Some(PendingAction::Elicitation {
                    turn_id: elicitation.turn_id.clone(),
                    item_id: elicitation.item_id.clone(),
                    call_id: event_call_id(&value),
                    elicitation,
                });
            }
        }
        update_pending_tool_calls(&value, &mut pending_tool_turns);
        if clears_pending_action(&value, pending_action.as_ref()) {
            pending_action = None;
        }
        if let Some(message) = parse_message_event(&value) {
            scan.message_count += 1;
            let text = trim_text(&message.text, 500);
            if message.role == "user" && scan.first_user_message_title.is_none() {
                scan.first_user_message_title = first_user_message_title(&text);
            }
            if !text.is_empty() {
                if text.contains("<proposed_plan>") {
                    let (turn_id, item_id) = plan_marker_for_event(&current_plan_marker, &value)
                        .unwrap_or_else(|| (event_turn_id(&value), event_item_id(&value)));
                    pending_action = Some(PendingAction::Plan { turn_id, item_id });
                    scan.latest_message = Some(
                        extract_proposed_plan_text(&text)
                            .map(|plan| trim_text(&plan, 500))
                            .unwrap_or_else(|| text.clone()),
                    );
                } else {
                    scan.recoverable = false;
                    scan.latest_message = Some(text);
                }
            }
        }
    }
    if let Some(pending) = pending_action {
        scan.reply_needed = true;
        scan.recoverable = false;
        if let PendingAction::Elicitation { elicitation, .. } = pending {
            scan.pending_elicitation = Some(elicitation);
        }
    }
    let _ = max_messages;
    if scan.active_turn_id.is_none() {
        scan.active_turn_id = pending_tool_turns.values().find_map(Clone::clone);
    }
    scan.running = scan.active_turn_id.is_some()
        || !active_tasks.is_empty()
        || !pending_tool_turns.is_empty()
        || matches!(
            last_task_status.as_deref(),
            Some("running" | "in_progress" | "inProgress")
        );
    Ok(scan)
}

pub fn rollout_latest_assistant_message(path: &Path) -> Result<Option<String>> {
    let scan = scan_rollout(path, 1)?;
    Ok(scan.latest_message)
}

pub fn rollout_hook_stop_message(path: &Path, turn_id: Option<&str>) -> Result<Option<String>> {
    Ok(rollout_hook_stop_message_with_source(path, turn_id)?.map(|(message, _)| message))
}

pub fn rollout_hook_stop_message_with_source(
    path: &Path,
    turn_id: Option<&str>,
) -> Result<Option<(String, String)>> {
    Ok(rollout_hook_stop_message_selection(path, turn_id)?
        .map(|selection| (selection.message, selection.source)))
}

pub fn rollout_hook_stop_message_selection(
    path: &Path,
    turn_id: Option<&str>,
) -> Result<Option<RolloutMessageSelection>> {
    let text =
        fs::read_to_string(path).with_context(|| format!("read rollout {}", path.display()))?;
    Ok(select_rollout_message(&text, turn_id, true))
}

fn select_rollout_message(
    text: &str,
    turn_id: Option<&str>,
    allow_global_fallback: bool,
) -> Option<RolloutMessageSelection> {
    let scoped = select_rollout_message_inner(text, turn_id);
    if scoped.is_some()
        || turn_id.is_none()
        || !allow_global_fallback
        || rollout_has_turn_signal(text, turn_id)
    {
        return scoped;
    }
    select_rollout_message_inner(text, None).map(|mut selection| {
        selection.strategy = format!("global_fallback.{}", selection.strategy);
        selection
    })
}

fn select_rollout_message_inner(
    text: &str,
    turn_id: Option<&str>,
) -> Option<RolloutMessageSelection> {
    let mut latest_assistant = None;
    let mut latest_unresolved_action: Option<PendingHookStopAction> = None;
    let mut latest_task_complete = None;
    let mut candidate_count = 0usize;
    let mut current_turn_id: Option<String> = None;
    for (index, line) in text.lines().enumerate() {
        let line_number = index + 1;
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let Some(event_turn) = event_turn_id(&value) {
            current_turn_id = Some(event_turn);
        }
        let effective_turn_id = event_turn_id(&value).or_else(|| current_turn_id.clone());
        let matches_turn_scope = turn_id
            .map(|expected_turn_id| effective_turn_id.as_deref() == Some(expected_turn_id))
            .unwrap_or(true);
        if !matches_turn_scope {
            continue;
        }
        let mut event_added_candidate = false;
        if let Some(plan) = plan_text_from_event(&value) {
            let (action_turn_id, action_item_id) = plan_marker_for_event(&None, &value)
                .unwrap_or_else(|| (event_turn_id(&value), event_item_id(&value)));
            latest_unresolved_action = Some(PendingHookStopAction {
                action: PendingAction::Plan {
                    turn_id: action_turn_id.clone(),
                    item_id: action_item_id,
                },
                message: plan,
                source: "proposed_plan",
                turn_id: action_turn_id.or_else(|| effective_turn_id.clone()),
                line: line_number,
            });
            event_added_candidate = true;
        }
        if let Some(elicitation) = parse_pending_elicitation(&value) {
            let elicitation_turn_id = elicitation.turn_id.clone();
            latest_unresolved_action = Some(PendingHookStopAction {
                action: PendingAction::Elicitation {
                    turn_id: elicitation_turn_id.clone(),
                    item_id: elicitation.item_id.clone(),
                    call_id: event_call_id(&value),
                    elicitation,
                },
                message: request_user_input_hook_message(&value),
                source: "request_user_input",
                turn_id: elicitation_turn_id.or_else(|| effective_turn_id.clone()),
                line: line_number,
            });
            event_added_candidate = true;
        }
        if let Some(message) = parse_raw_message_event(&value) {
            if message.role == "assistant" && !message.text.trim().is_empty() {
                let source = if extract_proposed_plan_text(&message.text).is_some() {
                    "proposed_plan"
                } else {
                    "last_assistant_message"
                };
                let is_plan = source == "proposed_plan";
                if is_plan {
                    let pending_plan = PendingHookStopAction {
                        action: PendingAction::Plan {
                            turn_id: event_turn_id(&value),
                            item_id: event_item_id(&value),
                        },
                        message: extract_proposed_plan_text(&message.text).unwrap_or(message.text),
                        source: "proposed_plan",
                        turn_id: effective_turn_id.clone(),
                        line: line_number,
                    };
                    if should_replace_pending_hook_stop_action(
                        latest_unresolved_action.as_ref(),
                        &pending_plan,
                    ) {
                        latest_unresolved_action = Some(pending_plan);
                    }
                    event_added_candidate = true;
                } else {
                    latest_assistant = Some(RolloutMessageSelection {
                        message: message.text,
                        source: source.to_string(),
                        strategy: source.to_string(),
                        selected_turn_id: effective_turn_id.clone(),
                        selected_line: Some(line_number),
                        candidate_count: 0,
                    });
                    event_added_candidate = true;
                }
            }
        }
        if latest_unresolved_action
            .as_ref()
            .is_some_and(|pending| clears_pending_action(&value, Some(&pending.action)))
        {
            latest_unresolved_action = None;
        }
        if rollout_event_type(&value) != "task_complete" {
            if event_added_candidate {
                candidate_count += 1;
            }
            continue;
        }
        let payload = value.get("payload").unwrap_or(&value);
        let last_agent_message = value
            .get("last_agent_message")
            .or_else(|| payload.get("last_agent_message"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|message| !message.is_empty())
            .map(str::to_string);
        if last_agent_message.is_some() {
            latest_task_complete = last_agent_message.map(|message| RolloutMessageSelection {
                message,
                source: "task_complete.last_agent_message".to_string(),
                strategy: "task_complete.last_agent_message".to_string(),
                selected_turn_id: effective_turn_id.clone(),
                selected_line: Some(line_number),
                candidate_count: 0,
            });
            event_added_candidate = true;
        }
        if event_added_candidate {
            candidate_count += 1;
        }
    }
    let mut selection = latest_unresolved_action
        .map(|pending| RolloutMessageSelection {
            message: pending.message,
            source: pending.source.to_string(),
            strategy: pending.source.to_string(),
            selected_turn_id: pending.turn_id,
            selected_line: Some(pending.line),
            candidate_count: 0,
        })
        .or(latest_task_complete)
        .or(latest_assistant)?;
    selection.candidate_count = candidate_count;
    Some(selection)
}

fn rollout_has_turn_signal(text: &str, turn_id: Option<&str>) -> bool {
    let Some(expected_turn_id) = turn_id else {
        return false;
    };
    text.lines().any(|line| {
        if line.trim().is_empty() {
            return false;
        }
        serde_json::from_str::<Value>(line)
            .ok()
            .and_then(|value| event_turn_id(&value))
            .as_deref()
            == Some(expected_turn_id)
    })
}

fn should_replace_pending_hook_stop_action(
    current: Option<&PendingHookStopAction>,
    next: &PendingHookStopAction,
) -> bool {
    let Some(current) = current else {
        return true;
    };
    match (&current.action, &next.action) {
        (
            PendingAction::Plan {
                turn_id: current_turn,
                item_id: current_item,
            },
            PendingAction::Plan {
                turn_id: next_turn,
                item_id: next_item,
            },
        ) if current_turn == next_turn && (next_item.is_none() || current_item == next_item) => {
            next.message.len() > current.message.len()
        }
        _ => true,
    }
}

pub fn rollout_completion_last_agent_message(
    path: &Path,
    turn_id: Option<&str>,
) -> Result<Option<String>> {
    Ok(
        rollout_completion_last_agent_message_with_source(path, turn_id)?
            .map(|(message, _)| message),
    )
}

pub fn rollout_completion_last_agent_message_with_source(
    path: &Path,
    turn_id: Option<&str>,
) -> Result<Option<(String, String)>> {
    Ok(
        rollout_completion_last_agent_message_selection(path, turn_id)?
            .map(|selection| (selection.message, selection.source)),
    )
}

pub fn rollout_completion_last_agent_message_selection(
    path: &Path,
    turn_id: Option<&str>,
) -> Result<Option<RolloutMessageSelection>> {
    let text =
        fs::read_to_string(path).with_context(|| format!("read rollout {}", path.display()))?;
    Ok(select_rollout_message(&text, turn_id, false))
}

pub fn rollout_has_completed_turn(path: &Path, turn_id: Option<&str>) -> Result<bool> {
    let text =
        fs::read_to_string(path).with_context(|| format!("read rollout {}", path.display()))?;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let event_type = rollout_event_type(&value);
        if !matches!(
            event_type,
            "task_complete" | "turn_completed" | "turn/completed"
        ) {
            continue;
        }
        if let Some(expected_turn_id) = turn_id {
            let event_turn = event_turn_id(&value);
            if event_turn.as_deref() != Some(expected_turn_id) {
                continue;
            }
        }
        if event_type != "task_complete" || task_complete_has_last_agent_message(&value) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn task_complete_has_last_agent_message(value: &Value) -> bool {
    let payload = value.get("payload").unwrap_or(value);
    value
        .get("last_agent_message")
        .or_else(|| payload.get("last_agent_message"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|message| !message.is_empty())
}

fn latest_active_task_turn(active_tasks: &[Option<String>]) -> Option<String> {
    active_tasks.iter().rev().find_map(Clone::clone)
}

fn is_turn_terminal_event(event_type: &str) -> bool {
    matches!(
        event_type,
        "turn_completed" | "turn/completed" | "turn_aborted" | "turn/aborted"
    )
}

fn clear_pending_tools_for_turn(
    pending_tool_turns: &mut HashMap<String, Option<String>>,
    turn_id: Option<&str>,
) {
    match turn_id {
        Some(turn_id) => pending_tool_turns.retain(|_, pending_turn| {
            pending_turn
                .as_deref()
                .is_some_and(|value| value != turn_id)
        }),
        None => pending_tool_turns.clear(),
    }
}

fn clear_anonymous_pending_tools(pending_tool_turns: &mut HashMap<String, Option<String>>) {
    pending_tool_turns.retain(|_, pending_turn| pending_turn.is_some());
}

fn rollout_event_type(value: &Value) -> &str {
    let top_level = value.get("type").and_then(Value::as_str).unwrap_or("");
    let raw = if top_level == "event_msg" {
        value
            .pointer("/payload/type")
            .or_else(|| value.pointer("/payload/event_type"))
            .or_else(|| value.pointer("/payload/event/type"))
            .or_else(|| value.pointer("/payload/payload/type"))
            .and_then(Value::as_str)
            .unwrap_or(top_level)
    } else {
        top_level
    };
    match raw {
        "thread.started" => "thread_started",
        "TurnStarted" => "task_started",
        "turn.started" => "turn_started",
        "TurnComplete" | "TurnCompleted" | "turn_complete" | "turn/complete" | "turn.completed" => {
            "turn_completed"
        }
        "TurnAborted" | "turn.failed" | "error" => "turn_aborted",
        "item.started" => "item_started",
        "item.completed" => "item_completed",
        "item.plan.delta" => "item/plan/delta",
        "turn.plan.updated" => "turn/plan/updated",
        "RequestUserInput" | "requestUserInput" => "request_user_input",
        other => other,
    }
}

fn update_pending_tool_calls(
    value: &Value,
    pending_tool_turns: &mut HashMap<String, Option<String>>,
) {
    let Some(payload) = event_payload(value) else {
        return;
    };
    let payload_type = event_payload_type(value, payload);
    if is_tool_output_kind(payload_type) {
        if let Some(call_id) = payload_call_id(payload) {
            pending_tool_turns.remove(&call_id);
        }
        return;
    }
    if !is_tool_call_kind(payload_type) || is_request_user_input(value) {
        return;
    }
    let Some(call_id) = payload_call_id(payload) else {
        return;
    };
    if payload_status(payload)
        .as_deref()
        .is_some_and(is_finished_status)
    {
        return;
    }
    pending_tool_turns.insert(call_id, event_turn_id(value));
}

fn plan_marker_for_event(
    marker: &Option<(Option<String>, Option<String>)>,
    value: &Value,
) -> Option<(Option<String>, Option<String>)> {
    let (turn_id, item_id) = marker.clone()?;
    let event_turn = event_turn_id(value);
    if turn_id.is_some() && event_turn.is_some() && turn_id != event_turn {
        return None;
    }
    Some((turn_id, item_id))
}

fn clears_pending_action(value: &Value, pending: Option<&PendingAction>) -> bool {
    let Some(pending) = pending else {
        return false;
    };
    if user_input_answer_matches(value, pending) {
        return true;
    }
    if function_output_matches(value, pending) {
        return true;
    }
    let Some(message) = parse_message_event(value) else {
        return false;
    };
    let role = message.role.as_str();
    if role == "user" {
        return true;
    }
    if role == "assistant" && !message.text.contains("<proposed_plan>") {
        return match pending {
            PendingAction::Plan { turn_id, .. } | PendingAction::Elicitation { turn_id, .. } => {
                event_turn_id(value).as_deref() != turn_id.as_deref()
            }
        };
    }
    false
}

fn user_input_answer_matches(value: &Value, pending: &PendingAction) -> bool {
    if !is_user_input_answer(value) {
        return false;
    }
    match pending {
        PendingAction::Elicitation {
            call_id: Some(expected),
            ..
        } => event_call_id(value).as_deref() == Some(expected),
        PendingAction::Elicitation { item_id, .. } => item_id
            .as_deref()
            .is_some_and(|expected| event_item_id(value).as_deref() == Some(expected)),
        PendingAction::Plan { item_id, .. } => item_id
            .as_deref()
            .is_some_and(|expected| event_item_id(value).as_deref() == Some(expected)),
    }
}

fn function_output_matches(value: &Value, pending: &PendingAction) -> bool {
    let payload = value.get("payload").unwrap_or(value);
    let payload_type = payload
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| value.get("type").and_then(Value::as_str));
    if payload_type != Some("function_call_output") {
        return false;
    }
    let call_id = payload
        .get("call_id")
        .or_else(|| payload.get("callId"))
        .and_then(Value::as_str);
    match pending {
        PendingAction::Elicitation {
            call_id: Some(expected),
            ..
        } => call_id == Some(expected),
        PendingAction::Elicitation { item_id, .. } => item_id
            .as_deref()
            .is_some_and(|expected| event_item_id(value).as_deref() == Some(expected)),
        PendingAction::Plan { item_id, .. } => item_id
            .as_deref()
            .is_some_and(|expected| event_item_id(value).as_deref() == Some(expected)),
    }
}

fn event_turn_id(value: &Value) -> Option<String> {
    value
        .get("turn_id")
        .or_else(|| value.get("turnId"))
        .or_else(|| value.pointer("/payload/turn_id"))
        .or_else(|| value.pointer("/payload/turnId"))
        .or_else(|| value.pointer("/payload/event/turn_id"))
        .or_else(|| value.pointer("/payload/event/turnId"))
        .or_else(|| value.pointer("/payload/payload/turn_id"))
        .or_else(|| value.pointer("/payload/payload/turnId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn event_item_id(value: &Value) -> Option<String> {
    value
        .get("item_id")
        .or_else(|| value.get("itemId"))
        .or_else(|| value.pointer("/item/id"))
        .or_else(|| value.pointer("/payload/id"))
        .or_else(|| value.pointer("/payload/item_id"))
        .or_else(|| value.pointer("/payload/itemId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn event_call_id(value: &Value) -> Option<String> {
    value
        .get("call_id")
        .or_else(|| value.get("callId"))
        .or_else(|| value.pointer("/payload/call_id"))
        .or_else(|| value.pointer("/payload/callId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(crate) fn parse_message_event(value: &Value) -> Option<CodexMessage> {
    let mut message = parse_raw_message_event(value)?;
    message.text = trim_text(&message.text, 4000);
    Some(message)
}

fn parse_raw_message_event(value: &Value) -> Option<CodexMessage> {
    let event_type = value.get("type").and_then(Value::as_str).unwrap_or("");
    let payload = value
        .get("payload")
        .or_else(|| value.get("message"))
        .or_else(|| value.get("item"));
    let payload_type = payload
        .and_then(|p| p.get("type"))
        .and_then(Value::as_str)
        .unwrap_or(event_type);
    if !matches!(
        payload_type,
        "message" | "agent_message" | "user_message" | "assistant_message"
    ) {
        return None;
    }

    let role = payload
        .and_then(|p| p.get("role"))
        .and_then(Value::as_str)
        .or_else(|| value.get("role").and_then(Value::as_str))
        .unwrap_or_else(|| {
            if payload_type.contains("function") {
                "tool"
            } else {
                "assistant"
            }
        });

    let text = structured_text_raw(payload.unwrap_or(value)).unwrap_or_else(|| {
        let mut text = String::new();
        collect_text(value, &mut text);
        text
    });
    let text = trim_preserving_indentation(&text, usize::MAX).0;
    if text.is_empty() {
        return None;
    }
    if payload_type == "message" && !matches!(role, "user" | "assistant") {
        return None;
    }
    if is_internal_display_kind(payload_type) || is_internal_context_message_text(&text) {
        return None;
    }
    Some(CodexMessage {
        role: role.to_string(),
        kind: payload_type.to_string(),
        text,
        created_at: value
            .get("timestamp")
            .or_else(|| value.get("created_at"))
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

fn plan_text_from_event(value: &Value) -> Option<String> {
    if rollout_event_type(value) == "item_completed"
        && value
            .get("item")
            .and_then(|item| item.get("type"))
            .and_then(Value::as_str)
            == Some("Plan")
    {
        return structured_text_raw(value.get("item")?)
            .map(|text| trim_text(&text, usize::MAX))
            .filter(|text| !text.is_empty());
    }
    let payload = value
        .get("payload")
        .or_else(|| value.get("item"))
        .unwrap_or(value);
    let payload_type = payload
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| value.get("type").and_then(Value::as_str));
    if !matches!(payload_type, Some("Plan" | "plan")) {
        return None;
    }
    structured_text_raw(payload)
        .or_else(|| structured_text_raw(value))
        .map(|text| trim_text(&text, usize::MAX))
        .filter(|text| !text.is_empty())
}

const MESSAGE_TEXT_LIMIT: usize = 12_000;
const TOOL_TEXT_LIMIT: usize = 8_000;
const TOOL_INPUT_LIMIT: usize = 4_000;
const TOOL_SUMMARY_LIMIT: usize = 220;
const COMPLETED_TOOL_HISTORY_LIMIT: usize = 40;
const CHAT_HISTORY_LIMIT: usize = 80;

#[derive(Default)]
struct MessageBlockBuilder {
    blocks: Vec<MessageBlock>,
    pending_tools: HashMap<String, PendingToolCall>,
    suppressed_call_ids: HashSet<String>,
    current_plan_marker: Option<(Option<String>, Option<String>)>,
    plan_block_indexes: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
struct PendingToolCall {
    raw_index: usize,
    id: String,
    kind: String,
    status: Option<String>,
    tool_name: Option<String>,
    call_id: Option<String>,
    turn_id: Option<String>,
    item_id: Option<String>,
    created_at: Option<String>,
    input: Option<String>,
    summary: Option<String>,
    payload: Option<Value>,
}

impl MessageBlockBuilder {
    fn push_event(&mut self, value: &Value, raw_index: usize) {
        let event_type = rollout_event_type(value);
        if event_type == "task_complete" || is_turn_terminal_event(event_type) {
            self.clear_pending_tools_for_turn(event_turn_id(value).as_deref());
        }
        if event_type == "task_complete" && task_complete_has_last_agent_message(value) {
            self.resolve_pending_plans_for_external_progress(event_turn_id(value).as_deref());
        }

        if is_plan_item_completed(value) {
            self.current_plan_marker = Some((event_turn_id(value), event_item_id(value)));
            return;
        }

        if let Some(block) = plan_delta_block(value, raw_index) {
            self.push_plan_block(block);
            return;
        }

        if let Some(block) = user_input_answer_block(value, raw_index) {
            self.push_answer_block(block);
            return;
        }

        if let Some(block) = pending_elicitation_block(value, raw_index) {
            if let Some(call_id) = &block.call_id {
                self.suppressed_call_ids.insert(call_id.clone());
            }
            self.blocks.push(block);
            return;
        }

        let Some(payload) = event_payload(value) else {
            return;
        };
        let payload_type = event_payload_type(value, payload);

        if is_tool_call_kind(payload_type) {
            if let Some(call_id) = payload_call_id(payload) {
                self.pending_tools.insert(
                    call_id.clone(),
                    PendingToolCall::from_payload(value, payload, payload_type, raw_index, call_id),
                );
            }
            return;
        }

        if is_tool_output_kind(payload_type) {
            let call_id = payload_call_id(payload);
            if call_id
                .as_ref()
                .is_some_and(|id| self.suppressed_call_ids.remove(id))
            {
                self.push_answer_block(user_input_output_block(value, payload, raw_index, call_id));
                return;
            }
            let pending = call_id
                .as_ref()
                .and_then(|id| self.pending_tools.remove(id));
            self.blocks.push(tool_output_block(
                value,
                payload,
                payload_type,
                raw_index,
                pending,
            ));
            return;
        }

        if let Some(mut block) = parse_message_block(value, raw_index) {
            if block.text.as_deref().is_some_and(contains_proposed_plan) {
                block.kind = "plan".to_string();
                block.status = block.status.or_else(|| Some("pending".to_string()));
                block.display_kind = Some("plan".to_string());
                block.plan_status = block.status.clone();
                block.resolved = Some(false);
                if let Some((turn_id, item_id)) =
                    plan_marker_for_event(&self.current_plan_marker, value)
                {
                    block.turn_id = block.turn_id.or(turn_id);
                    block.item_id = block.item_id.or(item_id);
                }
                block.group_id = Some(plan_group_key(&block, raw_index));
                self.current_plan_marker = None;
                self.push_plan_block(block);
                return;
            }
            if matches!(block.role.as_str(), "user" | "assistant" | "tool") {
                self.resolve_pending_plans();
            }
            self.blocks.push(block);
        }
    }

    fn push_plan_block(&mut self, mut block: MessageBlock) {
        let key = plan_group_key(&block, self.blocks.len());
        block.group_id = Some(key.clone());
        block.display_kind = Some("plan".to_string());
        block.plan_status = block.plan_status.or_else(|| block.status.clone());
        if let Some(index) = self.plan_block_indexes.get(&key).copied() {
            let existing = &mut self.blocks[index];
            if block.status.as_deref() == Some("streaming") {
                let next_text = block.text.unwrap_or_default();
                if !next_text.trim().is_empty() {
                    let current = existing.text.get_or_insert_with(String::new);
                    current.push_str(&next_text);
                }
                existing.status = Some("streaming".to_string());
                existing.plan_status = Some("streaming".to_string());
                existing.resolved = Some(false);
            } else {
                if block
                    .text
                    .as_deref()
                    .is_some_and(|text| !text.trim().is_empty())
                {
                    existing.text = block.text;
                }
                existing.status = block.status.or_else(|| Some("pending".to_string()));
                existing.plan_status = block.plan_status.or_else(|| existing.status.clone());
                existing.resolved = block.resolved.or(Some(false));
                existing.item_id = block.item_id.or_else(|| existing.item_id.clone());
                existing.turn_id = block.turn_id.or_else(|| existing.turn_id.clone());
                existing.created_at = block.created_at.or_else(|| existing.created_at.clone());
                existing.kind = "plan".to_string();
            }
            return;
        }
        self.plan_block_indexes.insert(key, self.blocks.len());
        self.blocks.push(block);
    }

    fn resolve_pending_plans(&mut self) {
        for block in &mut self.blocks {
            if block.kind == "plan" && block.resolved != Some(true) {
                block.status = Some("completed".to_string());
                block.plan_status = Some("completed".to_string());
                block.resolved = Some(true);
            }
        }
    }

    fn resolve_pending_plans_for_external_progress(&mut self, progress_turn_id: Option<&str>) {
        let Some(progress_turn_id) = progress_turn_id else {
            return;
        };
        for block in &mut self.blocks {
            if block.kind == "plan"
                && block.resolved != Some(true)
                && block
                    .turn_id
                    .as_deref()
                    .is_some_and(|turn_id| turn_id != progress_turn_id)
            {
                block.status = Some("completed".to_string());
                block.plan_status = Some("completed".to_string());
                block.resolved = Some(true);
            }
        }
    }

    fn push_answer_block(&mut self, block: MessageBlock) {
        let key = block
            .call_id
            .as_deref()
            .or(block.item_id.as_deref())
            .map(str::to_string);
        if let Some(key) = key {
            if let Some(index) = self.blocks.iter().rposition(|candidate| {
                candidate.call_id.as_deref() == Some(key.as_str())
                    || candidate.item_id.as_deref() == Some(key.as_str())
            }) {
                let pending = &mut self.blocks[index];
                if pending.kind == "request_user_input" {
                    pending.kind = "request_user_input_result".to_string();
                    pending.status = Some("completed".to_string());
                    pending.display_kind = Some("question_result".to_string());
                    pending.resolved = Some(true);
                    pending.answers = block.answers.clone();
                    pending.summary = Some(answer_summary(&block.answers));
                    pending.text = block.text;
                    return;
                }
            }
        }
        self.blocks.push(block);
    }

    fn clear_pending_tools_for_turn(&mut self, turn_id: Option<&str>) {
        match turn_id {
            Some(turn_id) => self.pending_tools.retain(|_, call| {
                call.turn_id
                    .as_deref()
                    .is_some_and(|value| value != turn_id)
            }),
            None => self.pending_tools.clear(),
        }
    }

    fn finish(mut self) -> Vec<MessageBlock> {
        let mut pending = self.pending_tools.into_values().collect::<Vec<_>>();
        pending.sort_by_key(|call| call.raw_index);
        self.blocks
            .extend(pending.into_iter().map(PendingToolCall::into_running_block));
        let blocks = compact_completed_tool_history(self.blocks, COMPLETED_TOOL_HISTORY_LIMIT);
        compact_chat_history(blocks, CHAT_HISTORY_LIMIT)
    }
}

fn compact_completed_tool_history(
    blocks: Vec<MessageBlock>,
    max_completed_tools: usize,
) -> Vec<MessageBlock> {
    let completed_tool_indexes = blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            (block.role == "tool"
                && !is_history_collapsed_block(block)
                && !block.status.as_deref().is_some_and(is_running_status))
            .then_some(index)
        })
        .collect::<Vec<_>>();
    if completed_tool_indexes.len() <= max_completed_tools {
        return blocks;
    }

    let hidden = completed_tool_indexes
        .len()
        .saturating_sub(max_completed_tools);
    let keep_start = completed_tool_indexes
        .len()
        .saturating_sub(max_completed_tools);
    let keep_completed = completed_tool_indexes[keep_start..]
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let hide_completed = completed_tool_indexes[..keep_start]
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let collapsed = MessageBlock {
        id: "completed-tool-history-collapsed".to_string(),
        role: "tool".to_string(),
        kind: "tool_history_collapsed".to_string(),
        display_kind: Some("tool_group".to_string()),
        status: Some("completed".to_string()),
        text: Some(format!("{hidden} 个历史工具调用已折叠")),
        summary: Some(format!("{hidden} 个历史工具调用已折叠")),
        input: None,
        truncated: Some(false),
        resolved: Some(true),
        answers: Vec::new(),
        plan_status: None,
        group_id: Some("tool_history".to_string()),
        tool_name: Some("tool_history".to_string()),
        call_id: None,
        turn_id: None,
        item_id: None,
        created_at: None,
        questions: Vec::new(),
        payload: None,
    };

    let mut compacted = Vec::with_capacity(blocks.len().saturating_sub(hidden).saturating_add(1));
    let mut inserted = false;
    for (index, block) in blocks.into_iter().enumerate() {
        if hide_completed.contains(&index) && !keep_completed.contains(&index) {
            if !inserted {
                compacted.push(collapsed.clone());
                inserted = true;
            }
            continue;
        }
        compacted.push(block);
    }
    compacted
}

fn compact_chat_history(blocks: Vec<MessageBlock>, max_chat_messages: usize) -> Vec<MessageBlock> {
    let chat_indexes = blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| is_chat_history_block(block).then_some(index))
        .collect::<Vec<_>>();
    if chat_indexes.len() <= max_chat_messages {
        return blocks;
    }

    let hidden = chat_indexes.len().saturating_sub(max_chat_messages);
    let keep_start = chat_indexes.len().saturating_sub(max_chat_messages);
    let keep_chat = chat_indexes[keep_start..]
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let hide_chat = chat_indexes[..keep_start]
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let collapsed = MessageBlock {
        id: "chat-history-collapsed".to_string(),
        role: "tool".to_string(),
        kind: "chat_history_collapsed".to_string(),
        display_kind: Some("history_group".to_string()),
        status: Some("completed".to_string()),
        text: Some(format!("{hidden} 条历史对话已折叠")),
        summary: Some(format!("{hidden} 条历史对话已折叠")),
        input: None,
        truncated: Some(false),
        resolved: Some(true),
        answers: Vec::new(),
        plan_status: None,
        group_id: Some("chat_history".to_string()),
        tool_name: Some("chat_history".to_string()),
        call_id: None,
        turn_id: None,
        item_id: None,
        created_at: None,
        questions: Vec::new(),
        payload: None,
    };

    let mut compacted = Vec::with_capacity(blocks.len().saturating_sub(hidden).saturating_add(1));
    let mut inserted = false;
    for (index, block) in blocks.into_iter().enumerate() {
        if hide_chat.contains(&index) && !keep_chat.contains(&index) {
            if !inserted {
                compacted.push(collapsed.clone());
                inserted = true;
            }
            continue;
        }
        compacted.push(block);
    }
    compacted
}

fn is_chat_history_block(block: &MessageBlock) -> bool {
    if !matches!(block.role.as_str(), "user" | "assistant") {
        return false;
    }
    if !block.questions.is_empty() {
        return false;
    }
    if block.status.as_deref().is_some_and(is_running_status) {
        return false;
    }
    let kind = block.kind.trim().to_ascii_lowercase();
    !matches!(
        kind.as_str(),
        "reasoning"
            | "agent_reasoning"
            | "session_meta"
            | "request_user_input"
            | "requestuserinput"
    ) && !kind.contains("plan")
        && !kind.contains("approval")
        && !kind.contains("tool")
        && !kind.contains("function_call")
        && !kind.contains("command")
}

fn is_history_collapsed_block(block: &MessageBlock) -> bool {
    matches!(
        block.kind.trim().to_ascii_lowercase().as_str(),
        "chat_history_collapsed" | "tool_history_collapsed"
    )
}

impl PendingToolCall {
    fn from_payload(
        value: &Value,
        payload: &Value,
        payload_type: &str,
        raw_index: usize,
        call_id: String,
    ) -> Self {
        let input = tool_input_text(payload);
        let summary = input.as_deref().map(tool_summary);
        Self {
            raw_index,
            id: block_id(value, raw_index),
            kind: normalize_kind(payload_type).to_string(),
            status: payload_status(payload),
            tool_name: tool_name(payload, payload_type),
            call_id: Some(call_id),
            turn_id: event_turn_id(value),
            item_id: payload_item_id(value, payload),
            created_at: event_time(value),
            input,
            summary,
            payload: None,
        }
    }

    fn into_running_block(self) -> MessageBlock {
        MessageBlock {
            id: self.id,
            role: "tool".to_string(),
            kind: self.kind,
            display_kind: Some("tool".to_string()),
            status: self.status.or_else(|| Some("running".to_string())),
            text: None,
            summary: self.summary,
            input: self.input,
            truncated: Some(false),
            resolved: Some(false),
            answers: Vec::new(),
            plan_status: None,
            group_id: self.call_id.clone(),
            tool_name: self.tool_name,
            call_id: self.call_id,
            turn_id: self.turn_id,
            item_id: self.item_id,
            created_at: self.created_at,
            questions: Vec::new(),
            payload: self.payload,
        }
    }
}

fn parse_message_block(value: &Value, raw_index: usize) -> Option<MessageBlock> {
    if let Some(block) = pending_elicitation_block(value, raw_index) {
        return Some(block);
    }

    let payload = event_payload(value)?;
    let payload_type = event_payload_type(value, payload);
    if is_plan_delta_kind(payload_type) || is_user_input_answer(value) {
        return None;
    }
    if is_tool_call_kind(payload_type) || is_tool_output_kind(payload_type) {
        return None;
    }
    let role = payload
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or_else(|| role_for_kind(payload_type));
    if payload_type == "message" && !matches!(role, "user" | "assistant") {
        return None;
    }
    if is_internal_display_kind(payload_type) {
        return None;
    }
    if payload_type != "message" && !is_action_display_kind(payload_type) {
        return None;
    }
    let text = structured_text(payload)
        .or_else(|| structured_text(value))
        .or_else(|| parse_message_event(value).map(|message| message.text));
    if text
        .as_deref()
        .is_some_and(is_internal_context_message_text)
    {
        return None;
    }
    if text.as_deref().unwrap_or("").trim().is_empty() && !is_action_display_kind(payload_type) {
        return None;
    }
    Some(MessageBlock {
        id: block_id(value, raw_index),
        role: role.to_string(),
        kind: normalize_kind(payload_type).to_string(),
        display_kind: display_kind_for_payload(payload_type).map(str::to_string),
        status: payload_status(payload),
        text,
        summary: None,
        input: None,
        truncated: Some(false),
        resolved: resolved_for_payload(payload_type, payload),
        answers: Vec::new(),
        plan_status: plan_status(payload),
        group_id: event_turn_id(value).or_else(|| payload_item_id(value, payload)),
        tool_name: tool_name(payload, payload_type),
        call_id: payload_call_id(payload),
        turn_id: event_turn_id(value),
        item_id: payload_item_id(value, payload),
        created_at: event_time(value),
        questions: Vec::new(),
        payload: None,
    })
}

fn is_plan_item_completed(value: &Value) -> bool {
    rollout_event_type(value) == "item_completed"
        && value
            .get("item")
            .and_then(|item| item.get("type"))
            .and_then(Value::as_str)
            == Some("Plan")
}

fn pending_elicitation_block(value: &Value, raw_index: usize) -> Option<MessageBlock> {
    let elicitation = parse_pending_elicitation(value)?;
    Some(MessageBlock {
        id: block_id(value, raw_index),
        role: "assistant".to_string(),
        kind: "request_user_input".to_string(),
        display_kind: Some("question".to_string()),
        status: Some("pending".to_string()),
        text: elicitation
            .questions
            .first()
            .map(|question| question.question.clone()),
        summary: elicitation
            .questions
            .first()
            .map(|question| tool_summary(&question.question)),
        input: None,
        truncated: Some(false),
        resolved: Some(false),
        answers: Vec::new(),
        plan_status: None,
        group_id: event_call_id(value).or_else(|| elicitation.item_id.clone()),
        tool_name: Some("request_user_input".to_string()),
        call_id: event_call_id(value),
        turn_id: elicitation.turn_id.clone(),
        item_id: elicitation.item_id.clone(),
        created_at: event_time(value),
        questions: elicitation.questions,
        payload: Some(value.clone()),
    })
}

fn request_user_input_hook_message(value: &Value) -> String {
    let Some(elicitation) = parse_pending_elicitation(value) else {
        return "Request user input".to_string();
    };
    elicitation
        .questions
        .iter()
        .enumerate()
        .map(|(question_index, question)| {
            let mut text = format!("问题 {}：{}", question_index + 1, question.question);
            for (option_index, option) in question.options.iter().enumerate() {
                text.push('\n');
                text.push_str(&format!("选项 {}：{}", option_index + 1, option.label));
                if let Some(description) = option.description.as_deref() {
                    if !description.trim().is_empty() {
                        text.push('\n');
                        text.push_str("说明：");
                        text.push_str(description.trim());
                    }
                }
            }
            text
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn plan_delta_block(value: &Value, raw_index: usize) -> Option<MessageBlock> {
    let payload = event_payload(value).unwrap_or(value);
    let payload_type = event_payload_type(value, payload);
    if !is_plan_delta_kind(payload_type) {
        return None;
    }
    let text = structured_text_raw(payload)
        .or_else(|| structured_text_raw(value))
        .or_else(|| {
            payload
                .get("delta")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .get("markdown")
                .and_then(Value::as_str)
                .map(str::to_string)
        });
    let status = if is_final_plan_kind(payload_type) {
        payload_status(payload).unwrap_or_else(|| "pending".to_string())
    } else {
        "streaming".to_string()
    };
    Some(MessageBlock {
        id: plan_block_id(value, payload, raw_index),
        role: "assistant".to_string(),
        kind: "plan".to_string(),
        display_kind: Some("plan".to_string()),
        status: Some(status.clone()),
        text,
        summary: Some("Proposed Plan".to_string()),
        input: None,
        truncated: Some(false),
        resolved: Some(is_finished_status(&status)),
        answers: Vec::new(),
        plan_status: Some(status),
        group_id: plan_group_id(value, payload),
        tool_name: None,
        call_id: None,
        turn_id: event_turn_id(value)
            .or_else(|| {
                payload
                    .get("turnId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .get("turn_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
        item_id: payload_item_id(value, payload),
        created_at: event_time(value),
        questions: Vec::new(),
        payload: None,
    })
}

fn user_input_answer_block(value: &Value, raw_index: usize) -> Option<MessageBlock> {
    if !is_user_input_answer(value) {
        return None;
    }
    let payload = value.get("payload").unwrap_or(value);
    let answers = parse_user_input_answers(payload);
    let call_id = event_call_id(value)
        .or_else(|| payload_call_id(payload))
        .or_else(|| payload_item_id(value, payload));
    Some(MessageBlock {
        id: block_id(value, raw_index),
        role: "assistant".to_string(),
        kind: "request_user_input_result".to_string(),
        display_kind: Some("question_result".to_string()),
        status: Some("completed".to_string()),
        text: Some(answer_summary(&answers)),
        summary: Some(answer_summary(&answers)),
        input: None,
        truncated: Some(false),
        resolved: Some(true),
        answers,
        plan_status: None,
        group_id: call_id.clone(),
        tool_name: Some("request_user_input".to_string()),
        call_id,
        turn_id: event_turn_id(value)
            .or_else(|| {
                payload
                    .get("turnId")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .or_else(|| {
                payload
                    .get("turn_id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
        item_id: payload_item_id(value, payload),
        created_at: event_time(value),
        questions: Vec::new(),
        payload: Some(value.clone()),
    })
}

fn user_input_output_block(
    value: &Value,
    payload: &Value,
    raw_index: usize,
    call_id: Option<String>,
) -> MessageBlock {
    let answers = parse_user_input_output_answers(payload);
    MessageBlock {
        id: block_id(value, raw_index),
        role: "assistant".to_string(),
        kind: "request_user_input_result".to_string(),
        display_kind: Some("question_result".to_string()),
        status: Some("completed".to_string()),
        text: Some(answer_summary(&answers)),
        summary: Some(answer_summary(&answers)),
        input: None,
        truncated: Some(false),
        resolved: Some(true),
        answers,
        plan_status: None,
        group_id: call_id.clone(),
        tool_name: Some("request_user_input".to_string()),
        call_id,
        turn_id: event_turn_id(value),
        item_id: payload_item_id(value, payload),
        created_at: event_time(value),
        questions: Vec::new(),
        payload: Some(value.clone()),
    }
}

fn tool_output_block(
    value: &Value,
    payload: &Value,
    payload_type: &str,
    raw_index: usize,
    pending: Option<PendingToolCall>,
) -> MessageBlock {
    let raw_text = structured_text_raw(payload)
        .or_else(|| structured_text_raw(value))
        .or_else(|| pretty_json(payload));
    let (text, truncated) = raw_text
        .as_deref()
        .map(|text| trim_text_with_limit(text, TOOL_TEXT_LIMIT))
        .unwrap_or_else(|| (String::new(), false));
    let call_id =
        payload_call_id(payload).or_else(|| pending.as_ref().and_then(|call| call.call_id.clone()));
    let input = pending
        .as_ref()
        .and_then(|call| call.input.clone())
        .or_else(|| tool_input_text(payload));
    let summary = if text.trim().is_empty() {
        input.as_deref().map(tool_summary)
    } else {
        Some(tool_summary(&text))
    };
    MessageBlock {
        id: pending
            .as_ref()
            .map(|call| call.id.clone())
            .unwrap_or_else(|| block_id(value, raw_index)),
        role: "tool".to_string(),
        kind: normalize_kind(payload_type).to_string(),
        display_kind: Some("tool".to_string()),
        status: payload_status(payload)
            .or_else(|| pending.as_ref().and_then(|call| call.status.clone()))
            .or_else(|| Some("completed".to_string())),
        text: if text.is_empty() { None } else { Some(text) },
        summary,
        input,
        truncated: Some(truncated),
        resolved: Some(true),
        answers: Vec::new(),
        plan_status: None,
        group_id: call_id.clone(),
        tool_name: pending
            .as_ref()
            .and_then(|call| call.tool_name.clone())
            .or_else(|| tool_name(payload, payload_type)),
        call_id,
        turn_id: event_turn_id(value)
            .or_else(|| pending.as_ref().and_then(|call| call.turn_id.clone())),
        item_id: payload_item_id(value, payload)
            .or_else(|| pending.as_ref().and_then(|call| call.item_id.clone())),
        created_at: event_time(value)
            .or_else(|| pending.as_ref().and_then(|call| call.created_at.clone())),
        questions: Vec::new(),
        payload: None,
    }
}

fn parse_pending_elicitation(value: &Value) -> Option<PendingElicitation> {
    let payload = value.get("payload").unwrap_or(value);
    if !is_request_user_input(value) {
        return None;
    }
    let args = payload
        .get("arguments")
        .and_then(parse_arguments_value)
        .or_else(|| {
            payload
                .pointer("/input/arguments")
                .and_then(parse_arguments_value)
        })
        .or_else(|| payload.get("params").cloned())
        .or_else(|| value.get("params").cloned())
        .or_else(|| Some(payload.clone()))?;
    let question_values = args
        .get("questions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let questions = question_values
        .iter()
        .enumerate()
        .map(|(index, question)| {
            let options = question
                .get("options")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .map(|option| UserInputOption {
                            label: option
                                .get("label")
                                .or_else(|| option.get("text"))
                                .or_else(|| option.get("value"))
                                .and_then(Value::as_str)
                                .unwrap_or("选项")
                                .to_string(),
                            description: option
                                .get("description")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            UserInputQuestion {
                id: question
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("q{}", index + 1)),
                header: question
                    .get("header")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                question: question
                    .get("question")
                    .and_then(Value::as_str)
                    .unwrap_or("需要回复")
                    .to_string(),
                options,
            }
        })
        .collect::<Vec<_>>();
    if questions.is_empty() {
        return None;
    }
    Some(PendingElicitation {
        turn_id: payload
            .get("turnId")
            .or_else(|| payload.get("turn_id"))
            .or_else(|| value.get("turnId"))
            .or_else(|| value.get("turn_id"))
            .or_else(|| value.pointer("/params/turnId"))
            .or_else(|| value.pointer("/params/turn_id"))
            .and_then(Value::as_str)
            .map(str::to_string),
        item_id: payload
            .get("itemId")
            .or_else(|| payload.get("item_id"))
            .or_else(|| value.get("itemId"))
            .or_else(|| value.get("item_id"))
            .or_else(|| value.pointer("/params/itemId"))
            .or_else(|| value.pointer("/params/item_id"))
            .and_then(Value::as_str)
            .map(str::to_string),
        questions,
    })
}

fn parse_arguments_value(value: &Value) -> Option<Value> {
    match value {
        Value::String(text) => serde_json::from_str::<Value>(text).ok(),
        other => Some(other.clone()),
    }
}

fn update_turn_state(value: &Value, scan: &mut RolloutScan) {
    let event_type = rollout_event_type(value);
    let turn_id = event_turn_id(value);
    if matches!(event_type, "task_started" | "turn_started" | "turn/started") {
        scan.active_turn_id = turn_id.clone();
        scan.running = true;
    }
    if is_turn_terminal_event(event_type) && (turn_id.is_none() || scan.active_turn_id == turn_id) {
        scan.active_turn_id = None;
    }
}

fn block_id(value: &Value, raw_index: usize) -> String {
    value
        .pointer("/payload/id")
        .or_else(|| value.pointer("/item/id"))
        .or_else(|| value.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("raw-{raw_index}"))
}

fn event_time(value: &Value) -> Option<String> {
    value
        .get("timestamp")
        .or_else(|| value.get("created_at"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn role_for_kind(kind: &str) -> &'static str {
    match kind {
        "userMessage" | "input_text" => "user",
        "function_call"
        | "function_call_output"
        | "custom_tool_call"
        | "custom_tool_call_output"
        | "tool_search_call"
        | "tool_search_output"
        | "web_search_call"
        | "web_search_end"
        | "commandExecution"
        | "mcpToolCall"
        | "dynamicToolCall"
        | "fileChange" => "tool",
        _ => "assistant",
    }
}

fn normalize_kind(kind: &str) -> &str {
    match kind {
        "Plan" => "plan",
        "PlanDelta" | "planDelta" | "plan_delta" | "item/plan/delta" => "plan",
        "turn/plan/updated" => "plan",
        other => other,
    }
}

fn is_plan_delta_kind(kind: &str) -> bool {
    let normalized = kind.to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "plan" | "plandelta" | "plan_delta" | "item/plan/delta" | "turn/plan/updated"
    )
}

fn is_final_plan_kind(kind: &str) -> bool {
    matches!(
        kind,
        "Plan" | "plan" | "turn/plan/updated" | "plan/updated" | "planUpdated"
    )
}

fn plan_block_id(value: &Value, payload: &Value, raw_index: usize) -> String {
    payload_item_id(value, payload)
        .or_else(|| event_turn_id(value).map(|turn| format!("plan-{turn}")))
        .unwrap_or_else(|| block_id(value, raw_index))
}

fn plan_group_key(block: &MessageBlock, fallback_index: usize) -> String {
    block
        .turn_id
        .as_deref()
        .map(|turn| format!("plan-turn-{turn}"))
        .or_else(|| {
            block
                .item_id
                .as_deref()
                .map(|item| format!("plan-item-{item}"))
        })
        .or_else(|| block.group_id.clone())
        .or_else(|| Some(format!("plan-{fallback_index}")))
        .unwrap()
}

fn plan_group_id(value: &Value, payload: &Value) -> Option<String> {
    event_turn_id(value)
        .or_else(|| {
            payload
                .get("turnId")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .get("turn_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .map(|turn| format!("plan-turn-{turn}"))
        .or_else(|| payload_item_id(value, payload).map(|item| format!("plan-item-{item}")))
}

fn event_payload(value: &Value) -> Option<&Value> {
    value
        .get("payload")
        .or_else(|| value.get("item"))
        .or_else(|| value.get("message"))
}

fn event_payload_type<'a>(value: &'a Value, payload: &'a Value) -> &'a str {
    payload
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| value.get("type").and_then(Value::as_str))
        .unwrap_or("event")
}

fn payload_status(payload: &Value) -> Option<String> {
    payload
        .get("status")
        .and_then(|status| status.get("type").or(Some(status)))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn plan_status(payload: &Value) -> Option<String> {
    payload_status(payload)
        .or_else(|| {
            payload
                .get("plan_status")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .get("planStatus")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn display_kind_for_payload(kind: &str) -> Option<&'static str> {
    let normalized = normalize_kind(kind).to_ascii_lowercase();
    if normalized.contains("plan") {
        Some("plan")
    } else if normalized.contains("approval") || normalized.contains("permissions/request") {
        Some("approval")
    } else {
        None
    }
}

fn resolved_for_payload(kind: &str, payload: &Value) -> Option<bool> {
    let normalized = normalize_kind(kind).to_ascii_lowercase();
    if normalized.contains("plan") {
        return payload_status(payload)
            .as_deref()
            .map(is_finished_status)
            .or(Some(false));
    }
    None
}

fn payload_call_id(payload: &Value) -> Option<String> {
    payload
        .get("call_id")
        .or_else(|| payload.get("callId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn payload_item_id(value: &Value, payload: &Value) -> Option<String> {
    payload
        .get("id")
        .or_else(|| value.get("item_id"))
        .or_else(|| value.get("itemId"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn is_tool_call_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_call" | "custom_tool_call" | "tool_search_call" | "web_search_call"
    )
}

fn is_tool_output_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_call_output"
            | "custom_tool_call_output"
            | "tool_search_output"
            | "web_search_end"
            | "web_search_output"
    )
}

fn is_action_display_kind(kind: &str) -> bool {
    let normalized = kind.to_ascii_lowercase();
    matches!(normalized.as_str(), "plan" | "plandelta" | "plan_delta")
        || normalized.contains("plan/delta")
        || normalized.contains("turn/plan/updated")
        || normalized.contains("approval")
        || normalized.contains("permissions/request")
}

fn is_internal_display_kind(kind: &str) -> bool {
    let normalized = kind.to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "reasoning"
            | "reasoning_delta"
            | "agent_reasoning"
            | "internal"
            | "subagent"
            | "system"
            | "developer"
            | "session_meta"
    ) || normalized.contains("reasoning")
        || normalized.contains("internal")
        || normalized.contains("subagent")
}

fn is_internal_context_message_text(text: &str) -> bool {
    let lower = text.trim_start().to_ascii_lowercase();
    [
        "<environment_context>",
        "<permissions instructions>",
        "<app-context>",
        "<collaboration_mode>",
        "<skills_instructions>",
        "<plugins_instructions>",
        "<subagent_notification>",
        "<subagent_context>",
        "<codex_internal_context",
        "<goal_context>",
        "<additional_context>",
        "<user_instructions>",
        "<turn_aborted>",
        "<user_shell_command>",
        "<legacy_unified_exec_process_limit_warning>",
        "<legacy_apply_patch_exec_command_warning>",
        "<legacy_model_mismatch_warning>",
        "========= memory_summary begins =========",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn contains_proposed_plan(text: &str) -> bool {
    text.contains("<proposed_plan>")
}

fn rollout_metadata_title(payload: &Value) -> Option<String> {
    [
        "thread_title",
        "threadTitle",
        "title",
        "name",
        "session_title",
        "sessionTitle",
    ]
    .into_iter()
    .find_map(|field| {
        payload
            .get(field)
            .and_then(Value::as_str)
            .and_then(thread_title_candidate)
    })
}

pub(crate) fn first_user_message_title(text: &str) -> Option<String> {
    let value = trim_text(text, 80);
    is_usable_thread_title(&value).then_some(value)
}

pub(crate) fn choose_initial_thread_title(
    title: Option<&str>,
    first_user_message: Option<&str>,
) -> String {
    title
        .and_then(thread_title_candidate)
        .or_else(|| first_user_message.and_then(first_user_message_title))
        .unwrap_or_else(|| "未命名线程".to_string())
}

pub(crate) fn should_repair_thread_title_from_local_metadata(
    db_title: Option<&str>,
    first_user_message: Option<&str>,
    index_entry: Option<&SessionIndexEntry>,
) -> bool {
    if db_title.and_then(thread_title_candidate).is_none() {
        return true;
    }
    if index_entry
        .and_then(SessionIndexEntry::title_candidate)
        .is_none()
    {
        return false;
    }
    let Some(db_title) = db_title.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    let Some(first_user_message) = first_user_message
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    db_title == first_user_message
}

pub(crate) fn thread_title_candidate(text: &str) -> Option<String> {
    let value = trim_text(text, 80);
    (is_usable_thread_title(&value) && text.trim().chars().count() <= 120).then_some(value)
}

pub(crate) fn is_usable_thread_title(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "untitled" | "untitled thread" | "未命名线程" | "读取中" | "loading"
    ) || lower.starts_with("读取中")
        || lower.starts_with("loading")
        || lower.contains("<proposed_plan>")
        || looks_like_assistant_preview_title(trimmed)
        || is_internal_context_message_text(trimmed)
    {
        return false;
    }
    true
}

fn looks_like_assistant_preview_title(text: &str) -> bool {
    let lower = text.trim().to_ascii_lowercase();
    (lower.starts_with("assistant preview:") || lower.starts_with("assistant:"))
        && text.chars().count() > 40
}

fn is_subagent_session_meta(payload: &Value) -> bool {
    let source_value = payload
        .get("thread_source")
        .or_else(|| payload.get("threadSource"))
        .or_else(|| payload.get("source"))
        .or_else(|| payload.get("sourceKind"))
        .or_else(|| payload.get("source_kind"));
    let source = source_value.and_then(Value::as_str);
    is_subagent_metadata(
        source,
        payload
            .get("parent_thread_id")
            .or_else(|| payload.get("parentThreadId"))
            .and_then(Value::as_str),
        payload
            .get("agent_path")
            .or_else(|| payload.get("agentPath"))
            .and_then(Value::as_str),
        payload
            .get("agent_nickname")
            .or_else(|| payload.get("agentNickname"))
            .and_then(Value::as_str),
        payload
            .get("agent_role")
            .or_else(|| payload.get("agentRole"))
            .and_then(Value::as_str),
    ) || source_value.is_some_and(value_contains_subagent)
}

pub(crate) fn is_subagent_metadata(
    source: Option<&str>,
    parent_thread_id: Option<&str>,
    agent_path: Option<&str>,
    agent_nickname: Option<&str>,
    agent_role: Option<&str>,
) -> bool {
    non_empty(parent_thread_id).is_some()
        || non_empty(agent_path).is_some()
        || source.is_some_and(source_text_contains_subagent)
        || non_empty(agent_nickname).is_some()
        || non_empty(agent_role).is_some()
}

#[derive(Clone, Copy)]
pub(crate) struct ThreadVisibilityMetadata<'a> {
    pub(crate) thread_source: Option<&'a str>,
    pub(crate) source: Option<&'a str>,
    pub(crate) parent_thread_id: Option<&'a str>,
    pub(crate) agent_path: Option<&'a str>,
    pub(crate) agent_nickname: Option<&'a str>,
    pub(crate) agent_role: Option<&'a str>,
    pub(crate) has_user_event: Option<i64>,
    pub(crate) title: Option<&'a str>,
    pub(crate) first_user_message: Option<&'a str>,
    pub(crate) preview: Option<&'a str>,
}

pub(crate) fn is_hidden_thread_metadata(metadata: ThreadVisibilityMetadata<'_>) -> bool {
    hidden_thread_metadata_category(metadata).is_some()
}

pub(crate) fn hidden_thread_metadata_category(
    metadata: ThreadVisibilityMetadata<'_>,
) -> Option<String> {
    if is_internal_thread_metadata(metadata) {
        return Some("internal".to_string());
    }
    if is_subagent_metadata(
        metadata.thread_source,
        metadata.parent_thread_id,
        metadata.agent_path,
        metadata.agent_nickname,
        metadata.agent_role,
    ) || metadata.source.is_some_and(source_text_contains_subagent)
    {
        return Some("subagent".to_string());
    }
    None
}

pub(crate) fn rollout_has_running_signal(path: &Path) -> Result<bool> {
    let scan = scan_rollout(path, 80)?;
    Ok(scan.running)
}

pub(crate) fn is_internal_thread_metadata(metadata: ThreadVisibilityMetadata<'_>) -> bool {
    let source = metadata
        .source
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let thread_source = metadata
        .thread_source
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if source != "exec" || metadata.has_user_event.unwrap_or(0) != 0 {
        return false;
    }
    if !thread_source.is_empty() && thread_source != "user" {
        return false;
    }
    [
        metadata.title,
        metadata.first_user_message,
        metadata.preview,
    ]
    .into_iter()
    .flatten()
    .any(is_internal_thread_prompt_text)
}

fn is_internal_thread_prompt_text(text: &str) -> bool {
    let text = text.trim().to_ascii_lowercase();
    if text.is_empty() {
        return false;
    }
    let readonly_probe = text.contains("只读验证")
        || text.contains("只读核查")
        || text.contains("不要修改文件")
        || text.contains("不改文件")
        || text.contains("read-only")
        || text.contains("readonly");
    let agent_probe = text.contains("spawn_agent")
        || text.contains("子代理")
        || text.contains("subagent")
        || text.contains("model_reasoning_effort=xhigh");
    if readonly_probe && agent_probe {
        return true;
    }

    let strong_subagent_instruction = text.contains("你是子代理")
        || text.contains("你是并行子代理")
        || text.contains("you are a subagent");
    let fixed_agent_config = text.contains("gpt-5.5")
        || text.contains("xhigh")
        || text.contains("model_reasoning_effort=xhigh");
    strong_subagent_instruction && fixed_agent_config
}

pub(crate) fn source_text_contains_subagent(value: &str) -> bool {
    value.to_ascii_lowercase().contains("subagent")
        || serde_json::from_str::<Value>(value)
            .ok()
            .as_ref()
            .is_some_and(value_contains_subagent)
}

pub(crate) fn thread_source_label(value: Option<&str>) -> String {
    let Some(value) = non_empty(value) else {
        return "unknown".to_string();
    };
    if source_text_contains_subagent(value) {
        return "subagent".to_string();
    }
    serde_json::from_str::<Value>(value)
        .ok()
        .and_then(|value| {
            value
                .get("kind")
                .or_else(|| value.get("type"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_else(|| value.to_ascii_lowercase())
}

fn value_contains_subagent(value: &Value) -> bool {
    match value {
        Value::String(text) => text.to_ascii_lowercase().contains("subagent"),
        Value::Array(items) => items.iter().any(value_contains_subagent),
        Value::Object(map) => map.iter().any(|(key, value)| {
            key.to_ascii_lowercase().contains("subagent") || value_contains_subagent(value)
        }),
        _ => false,
    }
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn tool_name(payload: &Value, kind: &str) -> Option<String> {
    payload
        .get("name")
        .or_else(|| payload.get("tool"))
        .or_else(|| payload.get("server"))
        .or_else(|| payload.get("method"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| match kind {
            "tool_search_call" | "tool_search_output" => Some("tool_search".to_string()),
            "web_search_call" | "web_search_end" | "web_search_output" => {
                Some("web_search".to_string())
            }
            _ => None,
        })
}

fn tool_input_text(payload: &Value) -> Option<String> {
    let value = payload
        .get("arguments")
        .or_else(|| payload.get("input"))
        .or_else(|| payload.get("params"))
        .or_else(|| payload.get("command"))?;
    let raw = match value {
        Value::String(text) => text.clone(),
        other => pretty_json(other)?,
    };
    let (text, _) = trim_preserving_indentation(&raw, TOOL_INPUT_LIMIT);
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn structured_text(value: &Value) -> Option<String> {
    structured_text_raw(value).map(|text| trim_text(&text, MESSAGE_TEXT_LIMIT))
}

fn structured_text_raw(value: &Value) -> Option<String> {
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = value.get("output").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(text) = value.get("aggregatedOutput").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    if let Some(command) = value.get("command").and_then(Value::as_str) {
        return Some(command.to_string());
    }
    if let Some(content) = value.get("content").and_then(Value::as_array) {
        let text = content
            .iter()
            .filter_map(|item| {
                item.get("text")
                    .or_else(|| item.get("input_text"))
                    .and_then(Value::as_str)
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    None
}

fn pretty_json(value: &Value) -> Option<String> {
    serde_json::to_string_pretty(value).ok()
}

fn collect_text(value: &Value, out: &mut String) {
    match value {
        Value::String(s) => {
            if looks_like_message(s) {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(s);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_text(value, out);
            }
        }
        Value::Object(map) => {
            for key in [
                "payload", "item", "text", "message", "content", "summary", "output",
            ] {
                if let Some(value) = map.get(key) {
                    collect_text(value, out);
                }
            }
        }
        _ => {}
    }
}

fn looks_like_message(s: &str) -> bool {
    let s = s.trim();
    s.len() > 1
        && !s.starts_with("019")
        && !s.starts_with("call_")
        && !matches!(s, "assistant" | "user" | "system" | "function_call")
}

pub(crate) fn is_request_user_input(value: &Value) -> bool {
    let payload = value.get("payload").unwrap_or(value);
    [
        value.get("type").and_then(Value::as_str),
        value.get("name").and_then(Value::as_str),
        value.get("toolName").and_then(Value::as_str),
        value.get("tool_name").and_then(Value::as_str),
        value.get("method").and_then(Value::as_str),
        payload.get("type").and_then(Value::as_str),
        payload.get("name").and_then(Value::as_str),
        payload.get("toolName").and_then(Value::as_str),
        payload.get("tool_name").and_then(Value::as_str),
        payload.get("method").and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    .any(|name| {
        matches!(
            name,
            "RequestUserInput"
                | "request_user_input"
                | "requestUserInput"
                | "item/tool/requestUserInput"
        )
    })
}

fn is_user_input_answer(value: &Value) -> bool {
    let payload = value.get("payload").unwrap_or(value);
    [
        value.get("type").and_then(Value::as_str),
        value.get("name").and_then(Value::as_str),
        value.get("method").and_then(Value::as_str),
        payload.get("type").and_then(Value::as_str),
        payload.get("name").and_then(Value::as_str),
        payload.get("method").and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    .any(|name| {
        matches!(
            name,
            "UserInputAnswer" | "userInputAnswer" | "user_input_answer"
        )
    })
}

fn parse_user_input_answers(payload: &Value) -> Vec<UserInputAnswer> {
    let source = payload
        .get("answers")
        .or_else(|| {
            payload
                .get("response")
                .and_then(|response| response.get("answers"))
        })
        .or_else(|| {
            payload
                .get("result")
                .and_then(|result| result.get("answers"))
        })
        .unwrap_or(payload);
    match source {
        Value::Object(map) => map
            .iter()
            .map(|(question_id, value)| UserInputAnswer {
                question_id: question_id.clone(),
                answers: answer_values(value),
                note: answer_note(value),
            })
            .collect(),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(index, item)| {
                let question_id = item
                    .get("question_id")
                    .or_else(|| item.get("questionId"))
                    .or_else(|| item.get("id"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("q{}", index + 1));
                UserInputAnswer {
                    question_id,
                    answers: answer_values(
                        item.get("answers")
                            .or_else(|| item.get("answer"))
                            .unwrap_or(item),
                    ),
                    note: answer_note(item),
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_user_input_output_answers(payload: &Value) -> Vec<UserInputAnswer> {
    payload
        .get("output")
        .and_then(Value::as_str)
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .map(|value| parse_user_input_answers(&value))
        .filter(|answers| !answers.is_empty())
        .unwrap_or_else(|| parse_user_input_answers(payload))
}

fn answer_values(value: &Value) -> Vec<String> {
    match value {
        Value::String(text) => vec![text.clone()],
        Value::Array(items) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(text) => Some(text.clone()),
                Value::Object(map) => map
                    .get("label")
                    .or_else(|| map.get("text"))
                    .or_else(|| map.get("value"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                _ => None,
            })
            .collect(),
        Value::Object(map) => map
            .get("answers")
            .or_else(|| map.get("answer"))
            .map(answer_values)
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn answer_note(value: &Value) -> Option<String> {
    value
        .get("note")
        .or_else(|| value.get("user_note"))
        .or_else(|| value.get("userNote"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn answer_summary(answers: &[UserInputAnswer]) -> String {
    if answers.is_empty() {
        return "Questions answered".to_string();
    }
    let answered = answers
        .iter()
        .filter(|answer| !answer.answers.is_empty() || answer.note.is_some())
        .count();
    format!("{answered}/{} answered", answers.len())
}

fn is_finished_status(status: &str) -> bool {
    let status = status.trim().to_ascii_lowercase();
    matches!(
        status.as_str(),
        "completed"
            | "complete"
            | "done"
            | "finished"
            | "success"
            | "succeeded"
            | "failed"
            | "error"
            | "cancelled"
            | "canceled"
            | "interrupted"
    )
}

fn is_running_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "pending" | "running" | "in_progress" | "inprogress" | "active"
    )
}

fn is_recoverable_terminal_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "failed" | "error" | "cancelled" | "canceled" | "interrupted"
    )
}

fn trim_text(text: &str, max: usize) -> String {
    trim_text_with_limit(text, max).0
}

fn trim_text_with_limit(text: &str, max: usize) -> (String, bool) {
    let compact = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if compact.chars().count() <= max {
        (compact, false)
    } else {
        (compact.chars().take(max).collect::<String>() + "...", true)
    }
}

fn trim_preserving_indentation(text: &str, max: usize) -> (String, bool) {
    let compact = text.trim().to_string();
    if compact.chars().count() <= max {
        (compact, false)
    } else {
        (compact.chars().take(max).collect::<String>() + "...", true)
    }
}

fn tool_summary(text: &str) -> String {
    let compact = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    if compact.chars().count() <= TOOL_SUMMARY_LIMIT {
        compact.to_string()
    } else {
        compact.chars().take(TOOL_SUMMARY_LIMIT).collect::<String>() + "..."
    }
}
