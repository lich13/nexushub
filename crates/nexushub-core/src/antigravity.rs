//! Pure projection of the native Antigravity App's read-only trajectory API.
pub mod sessions;
use crate::native_probe::{self, NativeProvider, NativeStreamSnapshot, NativeTurnEvent};
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub fn identity(id: &str, summary: &Value) -> Result<String> {
    ensure!(
        uuid::Uuid::parse_str(id).is_ok(),
        "invalid Antigravity identity"
    );
    let trajectory = summary["trajectoryId"]
        .as_str()
        .context("missing Antigravity trajectory identity")?;
    let created = summary["createdTime"]
        .as_str()
        .context("missing Antigravity creation time")?;
    let root = summary["trajectoryMetadata"]["rootConversationId"]
        .as_str()
        .context("missing Antigravity conversation origin")?;
    ensure!(
        !trajectory.is_empty() && native_probe::timestamp(&summary["createdTime"]).is_some(),
        "invalid Antigravity source identity"
    );
    ensure!(
        root.is_empty() || root == id,
        "Antigravity subagent is not a main conversation"
    );
    Ok(hex::encode(Sha256::digest(format!(
        "{id}\0{trajectory}\0{created}"
    ))))
}
fn safe_text(value: &Value) -> String {
    crate::security::redact_output(value.as_str().unwrap_or(""))
        .chars()
        .take(1200)
        .collect()
}
fn execution(step: &Value) -> &str {
    step["metadata"]["executionId"].as_str().unwrap_or("")
}
fn settled(step: &Value) -> bool {
    matches!(
        step["status"].as_str(),
        Some(
            "CORTEX_STEP_STATUS_DONE"
                | "CORTEX_STEP_STATUS_ERROR"
                | "CORTEX_STEP_STATUS_CANCELED"
                | "CORTEX_STEP_STATUS_CLEARED"
                | "CORTEX_STEP_STATUS_INTERRUPTED"
                | "CORTEX_STEP_STATUS_HALTED"
        )
    )
}
fn interaction(step: &Value, index: usize) -> Result<Option<NativeTurnEvent>> {
    if step["status"] != "CORTEX_STEP_STATUS_WAITING" {
        return Ok(None);
    }
    let Some(request) = step["requestedInteraction"]
        .as_object()
        .filter(|r| !r.is_empty())
    else {
        return Ok(None);
    };
    let (kind, body) = if let Some(questions) = request
        .get("askQuestion")
        .and_then(|v| v["questions"].as_array())
    {
        let texts = questions
            .iter()
            .filter_map(|q| q["question"].as_str())
            .collect::<Vec<_>>();
        ("reply_needed", texts.join("\n"))
    } else if request.contains_key("elicitation") {
        ("reply_needed", "需要回复，请打开 Antigravity 处理。".into())
    } else if [
        "permission",
        "filePermission",
        "approvalInteraction",
        "runCommand",
        "sendCommandInput",
        "readUrlContent",
        "openBrowserUrl",
        "runExtensionCode",
        "executeBrowserJavascript",
        "captureBrowserScreenshot",
        "clickBrowserPixel",
        "browserAction",
        "mcp",
        "deploy",
    ]
    .iter()
    .any(|k| request.contains_key(*k))
    {
        (
            "authorization",
            "操作需要授权，请打开 Antigravity 处理。".into(),
        )
    } else {
        anyhow::bail!("unsupported Antigravity interaction");
    };
    let execution = execution(step);
    ensure!(
        !execution.is_empty(),
        "missing Antigravity execution identity"
    );
    let proof = hex::encode(Sha256::digest(serde_json::to_vec(request)?));
    let completed = step["completedInteractions"].as_array().map_or(0, Vec::len);
    Ok(Some(NativeTurnEvent {
        prompt_id: None,
        position: index as u64 + 1,
        turn_id: format!("{execution}:step:{index}:request:{completed}:{proof}"),
        kind: kind.into(),
        body: safe_text(&Value::String(body)),
        timestamp_ms: native_probe::timestamp(&step["metadata"]["createdAt"])
            .context("missing Antigravity interaction time")?,
    }))
}

fn parse_json_or_value(raw: &Value) -> Value {
    match raw {
        Value::String(s) => serde_json::from_str(s).unwrap_or(Value::Null),
        other => other.clone(),
    }
}

fn step_tool_name(step: &Value) -> &str {
    step["generic"]["name"]
        .as_str()
        .or_else(|| step["generic"]["toolName"].as_str())
        .or_else(|| step["toolCall"]["name"].as_str())
        .or_else(|| step["metadata"]["toolCall"]["name"].as_str())
        .unwrap_or("")
}

fn step_tool_args(step: &Value) -> Value {
    for candidate in [
        &step["generic"]["arguments"],
        &step["generic"]["args"],
        &step["toolCall"]["arguments"],
        &step["toolCall"]["args"],
        &step["metadata"]["toolCall"]["arguments"],
    ] {
        if !candidate.is_null() {
            let parsed = parse_json_or_value(candidate);
            if !parsed.is_null() {
                return parsed;
            }
        }
    }
    Value::Null
}

fn step_is_daemon_task(step: &Value, args: &Value) -> bool {
    step["taskDetails"]["isDaemon"].as_bool() == Some(true)
        || step["runCommand"]["isDaemon"].as_bool() == Some(true)
        || step["schedule"]["isDaemon"].as_bool() == Some(true)
        || args["IsDaemon"].as_bool() == Some(true)
        || args["isDaemon"].as_bool() == Some(true)
}

fn collect_spawned_subagent_ids(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            if let Some(cid) = map
                .get("conversationId")
                .or_else(|| map.get("conversation_id"))
                .and_then(Value::as_str)
            {
                if uuid::Uuid::parse_str(cid).is_ok() {
                    out.insert(cid.to_owned());
                }
            }
            for child in map.values() {
                collect_spawned_subagent_ids(child, out);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_spawned_subagent_ids(child, out);
            }
        }
        Value::String(raw) if raw.trim_start().starts_with('{') || raw.trim_start().starts_with('[') => {
            if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
                collect_spawned_subagent_ids(&parsed, out);
            }
        }
        _ => {}
    }
}

fn cascade_status_is_active(status: Option<&str>) -> bool {
    matches!(
        status,
        Some(
            "CASCADE_RUN_STATUS_RUNNING"
                | "CASCADE_RUN_STATUS_BUSY"
                | "CASCADE_RUN_STATUS_WAITING"
                | "CASCADE_RUN_STATUS_CANCELING"
        )
    )
}

fn has_unsettled_background_work(
    id: &str,
    summary_trajectory_id: &str,
    steps: &[Value],
) -> bool {
    let turn_start = steps
        .iter()
        .enumerate()
        .rev()
        .find(|(_, step)| {
            let source = &step["metadata"]["sourceTrajectoryStepInfo"];
            let inherited = source["trajectoryId"]
                .as_str()
                .is_some_and(|orig| orig != summary_trajectory_id)
                || source["cascadeId"]
                    .as_str()
                    .is_some_and(|orig| orig != id);
            !inherited && step["type"] == "CORTEX_STEP_TYPE_USER_INPUT"
        })
        .map_or(0, |(idx, _)| idx + 1);

    let mut pending_tasks: BTreeSet<String> = BTreeSet::new();
    let mut pending_subagents: BTreeSet<String> = BTreeSet::new();

    for step in &steps[turn_start..] {
        let source = &step["metadata"]["sourceTrajectoryStepInfo"];
        if source["trajectoryId"]
            .as_str()
            .is_some_and(|orig| orig != summary_trajectory_id)
            || source["cascadeId"]
                .as_str()
                .is_some_and(|orig| orig != id)
        {
            continue;
        }
        if step["type"] == "CORTEX_STEP_TYPE_SYSTEM_MESSAGE" {
            if let Some(sender) = step["systemMessage"]["agentMessage"]["sender"]
                .as_str()
                .or_else(|| step["systemMessage"]["sender"].as_str())
                .filter(|s| !s.is_empty())
            {
                pending_tasks.remove(sender);
                pending_subagents.remove(sender);
            }
            continue;
        }
        let tool_name = step_tool_name(step);
        let args = step_tool_args(step);
        if let Some(task_id) = step["taskDetails"]["id"]
            .as_str()
            .filter(|s| !s.is_empty())
        {
            if !step_is_daemon_task(step, &args) {
                pending_tasks.insert(task_id.to_owned());
            }
        }
        if tool_name == "manage_task" && settled(step) {
            let action = args["Action"]
                .as_str()
                .or_else(|| args["action"].as_str())
                .unwrap_or("");
            if action == "kill" {
                if let Some(tid) = args["TaskId"]
                    .as_str()
                    .or_else(|| args["taskId"].as_str())
                {
                    pending_tasks.remove(tid);
                }
            }
        } else if (tool_name == "invoke_subagent" || !step["invokeSubagent"].is_null())
            && settled(step)
        {
            collect_spawned_subagent_ids(&step["generic"]["result"], &mut pending_subagents);
            collect_spawned_subagent_ids(&step["invokeSubagent"], &mut pending_subagents);
        } else if tool_name == "send_message" && settled(step) {
            if let Some(recipient) = args["Recipient"]
                .as_str()
                .or_else(|| args["recipient"].as_str())
                .filter(|cid| uuid::Uuid::parse_str(cid).is_ok())
            {
                pending_subagents.insert(recipient.to_owned());
            }
        } else if tool_name == "manage_subagents" && settled(step) {
            let action = args["Action"]
                .as_str()
                .or_else(|| args["action"].as_str())
                .unwrap_or("");
            if action == "kill_all" {
                pending_subagents.clear();
            } else if action == "kill" {
                if let Some(ids) = args["ConversationIds"]
                    .as_array()
                    .or_else(|| args["conversationIds"].as_array())
                {
                    for cid in ids.iter().filter_map(Value::as_str) {
                        pending_subagents.remove(cid);
                    }
                }
            }
        }
    }

    !pending_tasks.is_empty() || !pending_subagents.is_empty()
}

pub fn snapshot(id: &str, summary: &Value, response: &Value) -> Result<NativeStreamSnapshot> {
    snapshot_with_subagents(id, summary, response, false)
}

pub fn snapshot_with_subagents(
    id: &str,
    summary: &Value,
    response: &Value,
    has_active_subagents: bool,
) -> Result<NativeStreamSnapshot> {
    let identity = identity(id, summary)?;
    let trajectory = &response["trajectory"];
    ensure!(
        trajectory["cascadeId"] == id && trajectory["trajectoryId"] == summary["trajectoryId"],
        "Antigravity source identity changed"
    );
    let steps = trajectory["steps"]
        .as_array()
        .context("missing Antigravity steps")?;
    let count = response["numTotalSteps"]
        .as_u64()
        .unwrap_or(steps.len() as u64);
    ensure!(
        count == steps.len() as u64 && count <= 200_000,
        "incomplete Antigravity trajectory"
    );
    let executors = trajectory["executorMetadatas"]
        .as_array()
        .context("missing Antigravity execution state")?;
    let latest = executors
        .last()
        .and_then(|e| e["executionId"].as_str())
        .unwrap_or("");
    let summary_trajectory_id = summary["trajectoryId"].as_str().unwrap_or("");
    let cascade_active = has_active_subagents
        || cascade_status_is_active(summary["status"].as_str())
        || cascade_status_is_active(response["status"].as_str())
        || summary["notFullyIdle"].as_bool() == Some(true)
        || summary["hasRunningSubagents"].as_bool() == Some(true)
        || summary["waitingSteps"]
            .as_array()
            .is_some_and(|arr| !arr.is_empty());
    let unsettled_background = has_unsettled_background_work(id, summary_trajectory_id, steps);
    let mut by_execution: BTreeMap<&str, Vec<(usize, &Value)>> = BTreeMap::new();
    let mut events = Vec::new();
    let mut settled_count = count;
    for (index, step) in steps.iter().enumerate() {
        let source = &step["metadata"]["sourceTrajectoryStepInfo"];
        if source["trajectoryId"]
            .as_str()
            .is_some_and(|original| original != summary_trajectory_id)
            || source["cascadeId"]
                .as_str()
                .is_some_and(|original| original != id)
        {
            continue; // Forked / inherited steps belong to the original conversation.
        }
        let exec = execution(step);
        if !exec.is_empty() {
            by_execution.entry(exec).or_default().push((index, step));
        }
        if !settled(step) {
            settled_count = settled_count.min(index as u64);
        }
        if let Some(event) = interaction(step, index)? {
            events.push(event);
        }
    }
    for meta in executors {
        let exec = meta["executionId"]
            .as_str()
            .context("missing Antigravity execution identity")?;
        let Some(group) = by_execution.get(exec) else {
            continue;
        };
        let end = meta["lastStepIdx"].as_u64();
        let reason = meta["terminationReason"].as_str().unwrap_or("");
        if reason.is_empty() || reason == "EXECUTOR_TERMINATION_REASON_UNSPECIFIED" {
            settled_count = settled_count.min(group[0].0 as u64);
            continue;
        }
        if meta["proceededWithAutoContinue"] == true || group.iter().any(|(_, step)| !settled(step))
        {
            if exec == latest {
                settled_count = settled_count.min(group[0].0 as u64);
            }
            continue;
        }
        let Some((last_index, last)) = group.last() else {
            continue;
        };
        if end != Some(*last_index as u64) {
            if exec == latest {
                settled_count = settled_count.min(group[0].0 as u64);
            }
            continue;
        }
        // Only the latest executor can represent the final terminal state of the thread.
        if exec != latest {
            continue;
        }
        let kind = match reason {
            "EXECUTOR_TERMINATION_REASON_NO_TOOL_CALL" => "completion",
            "EXECUTOR_TERMINATION_REASON_TERMINAL_STEP_TYPE"
                if last["type"] == "CORTEX_STEP_TYPE_FINISH" =>
            {
                "completion"
            }
            "EXECUTOR_TERMINATION_REASON_ERROR"
            | "EXECUTOR_TERMINATION_REASON_MAX_INVOCATIONS"
            | "EXECUTOR_TERMINATION_REASON_MAX_FORCED_INVOCATIONS"
            | "EXECUTOR_TERMINATION_REASON_MAX_TOKEN_BUDGET_EXCEEDED" => "failure",
            _ => continue,
        };
        if kind == "completion"
            && (*last_index + 1 < steps.len() || cascade_active || unsettled_background)
        {
            settled_count = settled_count.min(group[0].0 as u64);
            continue;
        }
        // User cancellation or a new execution invalidates a previous failure.
        if kind == "failure"
            && (steps.iter().skip(last_index + 1).any(|s| {
                s["type"] == "CORTEX_STEP_TYPE_USER_INPUT"
                    || s["type"] == "CORTEX_STEP_TYPE_SYSTEM_MESSAGE"
                    || (!execution(s).is_empty() && execution(s) != exec)
            }) || cascade_active)
        {
            settled_count = settled_count.min(group[0].0 as u64);
            continue;
        }
        let timestamp = native_probe::timestamp(&last["metadata"]["completedAt"])
            .or_else(|| native_probe::timestamp(&last["metadata"]["finishedGeneratingAt"]));
        let Some(timestamp_ms) = timestamp else {
            continue;
        };
        let body = if kind == "failure" {
            "主回合未能完成，请打开 Antigravity 查看详情。".into()
        } else {
            group
                .iter()
                .rev()
                .find(|(_, step)| step["type"] == "CORTEX_STEP_TYPE_PLANNER_RESPONSE")
                .map(|(_, step)| safe_text(&step["plannerResponse"]["response"]))
                .unwrap_or_default()
        };
        events.push(NativeTurnEvent {
            prompt_id: None,
            position: *last_index as u64 + 1,
            turn_id: exec.into(),
            kind: kind.into(),
            body,
            timestamp_ms,
        });
    }
    events.sort_by_key(|e| e.position);
    if events.len() > 1024 {
        events.drain(..events.len() - 1024);
    }
    Ok(NativeStreamSnapshot {
        provider: NativeProvider::Antigravity,
        session_key: id.into(),
        id: id.into(),
        title: safe_text(&summary["summary"]),
        identity,
        record_count: count,
        settled_count,
        events,
    })
}

#[cfg(test)]
mod tests;
