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
    r["status"] = json!("CASCADE_RUN_STATUS_BUSY");
    let scan = snapshot(ID, &s, &r).unwrap();
    assert_eq!(scan.events.len(), 1);
    assert_eq!(scan.events[0].body, "Fixture completed");
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
