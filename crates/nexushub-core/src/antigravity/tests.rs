use super::*;
use serde_json::json;
const ID: &str = "11111111-1111-4111-8111-111111111111";
fn fixture() -> (Value, Value) {
    let summary = json!({"trajectoryId":"fixture-trajectory","createdTime":"2026-01-01T00:00:00Z","summary":"Fixture","trajectoryMetadata":{"rootConversationId":ID}});
    let response = json!({"status":"CASCADE_RUN_STATUS_IDLE","numTotalSteps":2,"trajectory":{"cascadeId":ID,"trajectoryId":"fixture-trajectory","steps":[
        {"type":"CORTEX_STEP_TYPE_USER_INPUT","status":"CORTEX_STEP_STATUS_DONE","metadata":{"executionId":"execution-1","createdAt":"2026-01-01T00:00:00Z"}},
        {"type":"CORTEX_STEP_TYPE_PLANNER_RESPONSE","status":"CORTEX_STEP_STATUS_DONE","metadata":{"executionId":"execution-1","createdAt":"2026-01-01T00:00:01Z","completedAt":"2026-01-01T00:00:02Z"},"plannerResponse":{"response":"Fixture completed","thinking":"private reasoning"}}
    ],"executorMetadatas":[{"executionId":"execution-1","lastStepIdx":1,"terminationReason":"EXECUTOR_TERMINATION_REASON_NO_TOOL_CALL"}]}});
    (summary, response)
}
#[test]
fn main_completion_uses_explicit_execution_terminal_and_visible_response() {
    let (s, mut r) = fixture();
    let scan = snapshot(ID, &s, &r).unwrap();
    assert_eq!(scan.events.len(), 1);
    assert_eq!(scan.events[0].body, "Fixture completed");
    assert_eq!(scan.settled_count, 2);

    // Active cascade status suppresses premature completion and holds settled_count
    r["status"] = json!("CASCADE_RUN_STATUS_BUSY");
    let busy = snapshot(ID, &s, &r).unwrap();
    assert!(busy.events.is_empty());
    assert_eq!(busy.settled_count, 0);
    r["status"] = json!("CASCADE_RUN_STATUS_IDLE");

    // Active child subtrajectory suppresses premature completion
    let sub_busy = snapshot_with_subagents(ID, &s, &r, true).unwrap();
    assert!(sub_busy.events.is_empty());
    assert_eq!(sub_busy.settled_count, 0);

    for reason in [
        "EXECUTOR_TERMINATION_REASON_USER_CANCELED",
        "EXECUTOR_TERMINATION_REASON_EARLY_CONTINUE",
        "EXECUTOR_TERMINATION_REASON_UNSPECIFIED",
        "unknown",
    ] {
        r["trajectory"]["executorMetadatas"][0]["terminationReason"] = json!(reason);
        assert!(snapshot(ID, &s, &r).unwrap().events.is_empty());
    }
}

#[test]
fn background_tasks_and_subagents_delay_completion_until_reactive_wakeup_finishes() {
    let sub_id = "22222222-2222-4222-8222-222222222222";
    let task_id = format!("{ID}/task-101");
    let summary = json!({
        "trajectoryId": "fixture-trajectory",
        "createdTime": "2026-01-01T00:00:00Z",
        "summary": "Long background workflow",
        "status": "CASCADE_RUN_STATUS_IDLE",
        "trajectoryMetadata": { "rootConversationId": ID }
    });

    // Phase 1: Executor 1 launched a background task + subagent and ended with NO_TOOL_CALL while waiting
    let mut response = json!({
        "status": "CASCADE_RUN_STATUS_IDLE",
        "numTotalSteps": 4,
        "trajectory": {
            "cascadeId": ID,
            "trajectoryId": "fixture-trajectory",
            "steps": [
                {
                    "type": "CORTEX_STEP_TYPE_USER_INPUT",
                    "status": "CORTEX_STEP_STATUS_DONE",
                    "metadata": { "executionId": "exec-1", "createdAt": "2026-01-01T00:00:00Z" }
                },
                {
                    "type": "CORTEX_STEP_TYPE_GENERIC",
                    "status": "CORTEX_STEP_STATUS_DONE",
                    "taskDetails": { "id": task_id, "movedToBackgroundAt": "2026-01-01T00:00:01Z" },
                    "generic": { "name": "run_command", "arguments": "{\"IsDaemon\":false}" },
                    "metadata": { "executionId": "exec-1", "createdAt": "2026-01-01T00:00:01Z" }
                },
                {
                    "type": "CORTEX_STEP_TYPE_GENERIC",
                    "status": "CORTEX_STEP_STATUS_DONE",
                    "generic": {
                        "name": "invoke_subagent",
                        "result": { "payload": { "results": [{ "conversationId": sub_id }] } }
                    },
                    "metadata": { "executionId": "exec-1", "createdAt": "2026-01-01T00:00:01Z" }
                },
                {
                    "type": "CORTEX_STEP_TYPE_PLANNER_RESPONSE",
                    "status": "CORTEX_STEP_STATUS_DONE",
                    "metadata": {
                        "executionId": "exec-1",
                        "createdAt": "2026-01-01T00:00:02Z",
                        "completedAt": "2026-01-01T00:00:03Z"
                    },
                    "plannerResponse": { "response": "Waiting for background command and subagent..." }
                }
            ],
            "executorMetadatas": [
                {
                    "executionId": "exec-1",
                    "lastStepIdx": 3,
                    "terminationReason": "EXECUTOR_TERMINATION_REASON_NO_TOOL_CALL"
                }
            ]
        }
    });
    let mid = snapshot(ID, &summary, &response).unwrap();
    assert!(
        mid.events.is_empty(),
        "must not notify while waiting on background task/subagent"
    );
    assert_eq!(mid.settled_count, 0);

    // Phase 2: Background task woke up via SYSTEM_MESSAGE, but subagent is still unsettled
    response["trajectory"]["steps"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "type": "CORTEX_STEP_TYPE_SYSTEM_MESSAGE",
            "status": "CORTEX_STEP_STATUS_DONE",
            "systemMessage": { "agentMessage": { "sender": task_id } },
            "metadata": { "executionId": "exec-2", "createdAt": "2026-01-01T00:00:10Z" }
        }));
    response["trajectory"]["steps"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "type": "CORTEX_STEP_TYPE_PLANNER_RESPONSE",
            "status": "CORTEX_STEP_STATUS_DONE",
            "metadata": {
                "executionId": "exec-2",
                "createdAt": "2026-01-01T00:00:11Z",
                "completedAt": "2026-01-01T00:00:12Z"
            },
            "plannerResponse": { "response": "Command done, still waiting for subagent..." }
        }));
    response["trajectory"]["executorMetadatas"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "executionId": "exec-2",
            "lastStepIdx": 5,
            "terminationReason": "EXECUTOR_TERMINATION_REASON_NO_TOOL_CALL"
        }));
    response["numTotalSteps"] = json!(6);
    let mid2 = snapshot(ID, &summary, &response).unwrap();
    assert!(
        mid2.events.is_empty(),
        "must not notify while subagent wakeup is still pending"
    );
    assert_eq!(mid2.settled_count, 4);

    // Phase 3: Subagent delivers SYSTEM_MESSAGE and final executor completes cleanly
    response["trajectory"]["steps"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "type": "CORTEX_STEP_TYPE_SYSTEM_MESSAGE",
            "status": "CORTEX_STEP_STATUS_DONE",
            "systemMessage": { "agentMessage": { "sender": sub_id } },
            "metadata": { "executionId": "exec-3", "createdAt": "2026-01-01T00:00:20Z" }
        }));
    response["trajectory"]["steps"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "type": "CORTEX_STEP_TYPE_PLANNER_RESPONSE",
            "status": "CORTEX_STEP_STATUS_DONE",
            "metadata": {
                "executionId": "exec-3",
                "createdAt": "2026-01-01T00:00:21Z",
                "completedAt": "2026-01-01T00:00:22Z"
            },
            "plannerResponse": { "response": "All tasks and subagents finished!" }
        }));
    response["trajectory"]["executorMetadatas"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "executionId": "exec-3",
            "lastStepIdx": 7,
            "terminationReason": "EXECUTOR_TERMINATION_REASON_NO_TOOL_CALL"
        }));
    response["numTotalSteps"] = json!(8);
    let done = snapshot(ID, &summary, &response).unwrap();
    assert_eq!(
        done.events.len(),
        1,
        "must emit exactly one final completion event"
    );
    assert_eq!(done.events[0].turn_id, "exec-3");
    assert_eq!(done.events[0].position, 8);
    assert_eq!(done.events[0].body, "All tasks and subagents finished!");
    assert_eq!(done.settled_count, 8);
}
#[test]
fn failure_is_obsolete_after_resume_and_tool_errors_are_not_terminal() {
    let (s, mut r) = fixture();
    r["trajectory"]["executorMetadatas"][0]["terminationReason"] =
        json!("EXECUTOR_TERMINATION_REASON_ERROR");
    assert_eq!(snapshot(ID, &s, &r).unwrap().events[0].kind, "failure");
    r["trajectory"]["executorMetadatas"]
        .as_array_mut()
        .unwrap()
        .push(json!({"executionId":"execution-2"}));
    assert!(snapshot(ID, &s, &r).unwrap().events.is_empty());
    r["trajectory"]["executorMetadatas"][0]["terminationReason"] = Value::Null;
    r["trajectory"]["steps"][1]["status"] = json!("CORTEX_STEP_STATUS_ERROR");
    assert!(snapshot(ID, &s, &r).unwrap().events.is_empty());
}
#[test]
fn waiting_requires_native_interaction_and_clears_after_answer() {
    let (s, mut r) = fixture();
    r["trajectory"]["executorMetadatas"][0]["terminationReason"] = Value::Null;
    let step = &mut r["trajectory"]["steps"][1];
    step["status"] = json!("CORTEX_STEP_STATUS_WAITING");
    assert!(snapshot(ID, &s, &r).unwrap().events.is_empty());
    r["trajectory"]["steps"][1]["requestedInteraction"] =
        json!({"askQuestion":{"questions":[{"question":"Continue?"}]}});
    let before = snapshot(ID, &s, &r).unwrap();
    assert_eq!(before.events[0].kind, "reply_needed");
    assert_eq!(before.settled_count, 0);
    r["trajectory"]["steps"][1]["status"] = json!("CORTEX_STEP_STATUS_DONE");
    assert!(snapshot(ID, &s, &r).unwrap().events.is_empty());
}
#[test]
fn permission_request_identity_changes_and_generating_tool_does_not_notify() {
    let (s, mut r) = fixture();
    r["trajectory"]["executorMetadatas"][0]["terminationReason"] = Value::Null;
    r["trajectory"]["steps"][1]["requestedInteraction"] =
        json!({"permission":{"reason":"fixture approval"}});
    r["trajectory"]["steps"][1]["status"] = json!("CORTEX_STEP_STATUS_RUNNING");
    assert!(snapshot(ID, &s, &r).unwrap().events.is_empty());
    r["trajectory"]["steps"][1]["status"] = json!("CORTEX_STEP_STATUS_WAITING");
    let a = snapshot(ID, &s, &r).unwrap();
    assert_eq!(a.events[0].kind, "authorization");
    r["trajectory"]["steps"][1]["requestedInteraction"]["permission"]["reason"] =
        json!("different fixture request");
    let b = snapshot(ID, &s, &r).unwrap();
    assert_ne!(a.event_key(&a.events[0]), b.event_key(&b.events[0]));
}
#[test]
fn mismatched_identity_subagents_and_incomplete_snapshots_are_rejected() {
    let (mut s, mut r) = fixture();
    r["numTotalSteps"] = json!(3);
    assert!(snapshot(ID, &s, &r).is_err());
    r["numTotalSteps"] = json!(2);
    s["trajectoryMetadata"]["rootConversationId"] = json!("parent");
    assert!(snapshot(ID, &s, &r).is_err());
    s["trajectoryMetadata"]["rootConversationId"] = json!(ID);
    r["trajectory"]["cascadeId"] = json!("other");
    assert!(snapshot(ID, &s, &r).is_err());
}

#[test]
fn unknown_origin_and_waiting_interaction_are_reported_as_unreadable() {
    let (mut s, mut r) = fixture();
    s["trajectoryMetadata"] = Value::Null;
    assert!(snapshot(ID, &s, &r).is_err());
    s["trajectoryMetadata"] = json!({"rootConversationId":ID});
    r["trajectory"]["steps"][1]["status"] = json!("CORTEX_STEP_STATUS_WAITING");
    r["trajectory"]["steps"][1]["requestedInteraction"] = json!({"unknown":{}});
    assert!(snapshot(ID, &s, &r).is_err());
}

#[test]
fn fork_inherited_executions_never_become_new_completion_events() {
    let (s, mut r) = fixture();
    for step in r["trajectory"]["steps"].as_array_mut().unwrap() {
        step["metadata"]["sourceTrajectoryStepInfo"] =
            json!({"cascadeId":"original-cascade","trajectoryId":"original-trajectory"});
    }
    assert!(snapshot(ID, &s, &r).unwrap().events.is_empty());
}

#[test]
fn silent_subagents_and_background_tasks_without_system_message_eventually_deliver_completion() {
    let sub_id = "22222222-2222-4222-8222-222222222222";
    let task_id = format!("{ID}/task-202");
    let summary = json!({
        "trajectoryId": "fixture-trajectory",
        "createdTime": "2026-01-01T00:00:00Z",
        "summary": "Silent subagent and task",
        "status": "CASCADE_RUN_STATUS_IDLE",
        "trajectoryMetadata": { "rootConversationId": ID }
    });
    // Base timestamp: 2026-01-01T00:00:10Z = 1_767_225_610_000 ms
    let base_ms = native_probe::timestamp(&json!("2026-01-01T00:00:10Z")).unwrap();
    let response = json!({
        "status": "CASCADE_RUN_STATUS_IDLE",
        "stepOffset": 2300,
        "numTotalSteps": 2304,
        "trajectory": {
            "cascadeId": ID,
            "trajectoryId": "fixture-trajectory",
            "steps": [
                {
                    "type": "CORTEX_STEP_TYPE_USER_INPUT",
                    "status": "CORTEX_STEP_STATUS_DONE",
                    "metadata": { "executionId": "exec-tail", "createdAt": "2026-01-01T00:00:00Z" }
                },
                {
                    "type": "CORTEX_STEP_TYPE_GENERIC",
                    "status": "CORTEX_STEP_STATUS_DONE",
                    "taskDetails": {
                        "id": task_id,
                        "movedToBackgroundAt": "2026-01-01T00:00:02Z",
                        "logUri": "file:///tmp/task-202.log"
                    },
                    "generic": { "name": "run_command", "arguments": "{\"IsDaemon\":false}" },
                    "metadata": { "executionId": "exec-tail", "createdAt": "2026-01-01T00:00:02Z" }
                },
                {
                    "type": "CORTEX_STEP_TYPE_GENERIC",
                    "status": "CORTEX_STEP_STATUS_DONE",
                    "generic": {
                        "name": "send_message",
                        "arguments": format!("{{\"Recipient\":\"{sub_id}\",\"Message\":\"ping\"}}")
                    },
                    "metadata": { "executionId": "exec-tail", "createdAt": "2026-01-01T00:00:05Z" }
                },
                {
                    "type": "CORTEX_STEP_TYPE_PLANNER_RESPONSE",
                    "status": "CORTEX_STEP_STATUS_DONE",
                    "metadata": {
                        "executionId": "exec-tail",
                        "createdAt": "2026-01-01T00:00:09Z",
                        "completedAt": "2026-01-01T00:00:10Z"
                    },
                    "plannerResponse": { "response": "Completed after subagent & task exit." }
                }
            ],
            "executorMetadatas": [
                {
                    "executionId": "exec-tail",
                    "lastStepIdx": 2303,
                    "terminationReason": "EXECUTOR_TERMINATION_REASON_NO_TOOL_CALL"
                }
            ]
        }
    });

    // 1. While subagent or task is still active (even 2 minutes later), never fire prematurely
    let mut ctx = TrajectoryLivenessContext {
        has_active_subagents: false,
        now_ms: Some(base_ms + 120_000),
        subagents: BTreeMap::from([(
            sub_id.into(),
            SubagentLiveness {
                active: true,
                last_modified_ms: Some(base_ms + 5_000),
            },
        )]),
        tasks: BTreeMap::from([(
            task_id.clone(),
            TaskLiveness {
                active: false,
                last_activity_ms: Some(base_ms + 4_000),
            },
        )]),
    };
    assert!(snapshot_with_context(ID, &summary, &response, &ctx)
        .unwrap()
        .events
        .is_empty());

    // 2. Both subagent and task become idle at base_ms + 5_000, inside 15s grace window -> hold
    ctx.subagents.get_mut(sub_id).unwrap().active = false;
    ctx.now_ms = Some(base_ms + 5_000 + BACKGROUND_HANDOFF_GRACE_MS - 1);
    let within_grace = snapshot_with_context(ID, &summary, &response, &ctx).unwrap();
    assert!(within_grace.events.is_empty());
    assert_eq!(within_grace.settled_count, 2300);

    // 3. Past 15s grace window without SYSTEM_MESSAGE -> guaranteed completion delivery!
    ctx.now_ms = Some(base_ms + 5_000 + BACKGROUND_HANDOFF_GRACE_MS);
    let settled = snapshot_with_context(ID, &summary, &response, &ctx).unwrap();
    assert_eq!(settled.record_count, 2304);
    assert_eq!(settled.settled_count, 2304);
    assert_eq!(settled.events.len(), 1);
    assert_eq!(settled.events[0].kind, "completion");
    assert_eq!(settled.events[0].position, 2304);
    assert_eq!(settled.events[0].turn_id, "exec-tail");
    assert_eq!(
        settled.events[0].body,
        "Completed after subagent & task exit."
    );
}
