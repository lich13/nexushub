//! Pure projection of the native Antigravity App's read-only trajectory API.
use crate::native_probe::{self, NativeProvider, NativeStreamSnapshot, NativeTurnEvent};
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

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

pub fn snapshot(id: &str, summary: &Value, response: &Value) -> Result<NativeStreamSnapshot> {
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
    let mut by_execution: BTreeMap<&str, Vec<(usize, &Value)>> = BTreeMap::new();
    let mut events = Vec::new();
    let mut settled_count = count;
    for (index, step) in steps.iter().enumerate() {
        let source = &step["metadata"]["sourceTrajectoryStepInfo"];
        if source["trajectoryId"]
            .as_str()
            .is_some_and(|original| original != summary["trajectoryId"].as_str().unwrap_or(""))
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
            continue;
        }
        let Some((last_index, last)) = group.last() else {
            continue;
        };
        if end != Some(*last_index as u64) {
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
            | "EXECUTOR_TERMINATION_REASON_MAX_TOKEN_BUDGET_EXCEEDED"
                if exec == latest =>
            {
                "failure"
            }
            _ => continue,
        };
        // User cancellation or a new execution invalidates a previous failure.
        if kind == "failure"
            && (steps.iter().skip(last_index + 1).any(|s| {
                s["type"] == "CORTEX_STEP_TYPE_USER_INPUT"
                    || (!execution(s).is_empty() && execution(s) != exec)
            }) || matches!(
                response["status"].as_str(),
                Some("CASCADE_RUN_STATUS_RUNNING" | "CASCADE_RUN_STATUS_CANCELING")
            ))
        {
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
