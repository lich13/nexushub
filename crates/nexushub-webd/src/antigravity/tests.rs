use super::*;
use axum::{http::StatusCode, response::IntoResponse, routing::post, Router};
#[tokio::test]
async fn transport_only_reads_allowlisted_methods_and_refuses_redirects() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let router = Router::new()
        .route(
            &format!("{SERVICE}GetAllCascadeTrajectories"),
            post(|| async { axum::Json(json!({"trajectorySummaries":{}})) }),
        )
        .route(
            &format!("{SERVICE}GetCascadeTrajectory"),
            post(|| async {
                (
                    StatusCode::FOUND,
                    [("location", "http://example.invalid/redirect")],
                )
                    .into_response()
            }),
        );
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let client = client().unwrap();
    assert!(query(
        &client,
        port,
        "fixture-token",
        "GetAllCascadeTrajectories",
        json!({})
    )
    .await
    .is_ok());
    assert!(query(
        &client,
        port,
        "fixture-token",
        "GetCascadeTrajectory",
        json!({})
    )
    .await
    .is_err());
    assert!(
        query(&client, port, "fixture-token", "SendUserMessage", json!({}))
            .await
            .is_err()
    );
    task.abort();
}
#[tokio::test]
async fn unavailable_native_endpoint_returns_a_safe_error() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let error = query(
        &client().unwrap(),
        port,
        "fixture-token",
        "GetAllCascadeTrajectories",
        json!({}),
    )
    .await
    .unwrap_err();
    assert_eq!(error.to_string(), "Antigravity read unavailable");
}

#[tokio::test]
async fn scan_paginates_steps_and_suppresses_completion_while_subtrajectory_runs() {
    let root_id = "11111111-1111-4111-8111-111111111111";
    let sub_id = "22222222-2222-4222-8222-222222222222";
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let router = Router::new()
        .route(
            &format!("{SERVICE}GetCascadeTrajectory"),
            post(move || async move {
                axum::Json(json!({
                    "status": "CASCADE_RUN_STATUS_IDLE",
                    "numTotalSteps": 2,
                    "trajectory": {
                        "cascadeId": root_id,
                        "trajectoryId": "traj-root",
                        "steps": [
                            {
                                "type": "CORTEX_STEP_TYPE_USER_INPUT",
                                "status": "CORTEX_STEP_STATUS_DONE",
                                "metadata": { "executionId": "exec-1", "createdAt": "2026-01-01T00:00:00Z" }
                            }
                        ],
                        "executorMetadatas": [
                            {
                                "executionId": "exec-1",
                                "lastStepIdx": 1,
                                "terminationReason": "EXECUTOR_TERMINATION_REASON_NO_TOOL_CALL"
                            }
                        ]
                    }
                }))
            }),
        )
        .route(
            &format!("{SERVICE}GetCascadeTrajectorySteps"),
            post(|axum::Json(body): axum::Json<Value>| async move {
                assert_eq!(body["stepOffset"], json!(1));
                axum::Json(json!({
                    "steps": [
                        {
                            "type": "CORTEX_STEP_TYPE_PLANNER_RESPONSE",
                            "status": "CORTEX_STEP_STATUS_DONE",
                            "metadata": {
                                "executionId": "exec-1",
                                "createdAt": "2026-01-01T00:00:01Z",
                                "completedAt": "2026-01-01T00:00:02Z"
                            },
                            "plannerResponse": { "response": "Final paginated reply" }
                        }
                    ]
                }))
            }),
        );
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let http = client().unwrap();

    // 1. Subagent is RUNNING -> root scan skips subagent as stream AND suppresses root completion
    let summaries_busy = json!({
        "trajectorySummaries": {
            root_id: {
                "trajectoryId": "traj-root",
                "createdTime": "2026-01-01T00:00:00Z",
                "summary": "Root task",
                "status": "CASCADE_RUN_STATUS_IDLE",
                "trajectoryMetadata": { "rootConversationId": root_id }
            },
            sub_id: {
                "trajectoryId": "traj-sub",
                "createdTime": "2026-01-01T00:00:00Z",
                "summary": "Child research",
                "status": "CASCADE_RUN_STATUS_RUNNING",
                "trajectoryMetadata": {
                    "rootConversationId": root_id,
                    "parentConversationId": root_id,
                    "nestingDepth": 1
                }
            }
        }
    });
    let scan_busy = scan_at(&http, port, "fixture-token", 42, &summaries_busy, None)
        .await
        .unwrap();
    assert_eq!(scan_busy.errors, 0);
    assert_eq!(scan_busy.streams.len(), 1);
    assert!(scan_busy.streams[0].events.is_empty());

    // 2. Subagent finishes -> root scan paginates step 1 and emits completion
    let mut summaries_idle = summaries_busy.clone();
    summaries_idle["trajectorySummaries"][sub_id]["status"] = json!("CASCADE_RUN_STATUS_IDLE");
    let scan_idle = scan_at(&http, port, "fixture-token", 42, &summaries_idle, None)
        .await
        .unwrap();
    assert_eq!(scan_idle.errors, 0);
    assert_eq!(scan_idle.streams.len(), 1);
    assert_eq!(scan_idle.streams[0].events.len(), 1);
    assert_eq!(scan_idle.streams[0].events[0].body, "Final paginated reply");

    task.abort();
}

#[tokio::test]
async fn long_session_scan_fetches_only_current_turn_tail_steps_without_full_trajectory() {
    let root_id = "33333333-3333-4333-8333-333333333333";
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let router = Router::new()
        .route(
            &format!("{SERVICE}GetCascadeTrajectory"),
            post(|| async { StatusCode::PAYLOAD_TOO_LARGE.into_response() }),
        )
        .route(
            &format!("{SERVICE}GetCascadeTrajectorySteps"),
            post(|axum::Json(body): axum::Json<Value>| async move {
                assert_eq!(body["stepOffset"], json!(2305));
                axum::Json(json!({
                    "steps": [
                        {
                            "type": "CORTEX_STEP_TYPE_USER_INPUT",
                            "status": "CORTEX_STEP_STATUS_DONE",
                            "metadata": { "executionId": "exec-99", "createdAt": "2026-10-29T04:00:00Z" }
                        },
                        {
                            "type": "CORTEX_STEP_TYPE_PLANNER_RESPONSE",
                            "status": "CORTEX_STEP_STATUS_DONE",
                            "metadata": {
                                "executionId": "exec-99",
                                "createdAt": "2026-10-29T04:00:02Z",
                                "completedAt": "2026-10-29T04:00:05Z"
                            },
                            "plannerResponse": { "response": "Long session completed via incremental steps." }
                        }
                    ]
                }))
            }),
        );
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let summaries = json!({
        "trajectorySummaries": {
            root_id: {
                "trajectoryId": "traj-long",
                "createdTime": "2026-10-28T00:00:00Z",
                "lastModifiedTime": "2026-10-29T04:00:05Z",
                "summary": "2300-step long session",
                "status": "CASCADE_RUN_STATUS_IDLE",
                "stepCount": 2307,
                "lastUserInputStepIndex": 2305,
                "latestExecutorTerminationReason": "EXECUTOR_TERMINATION_REASON_NO_TOOL_CALL",
                "trajectoryMetadata": { "rootConversationId": root_id }
            }
        }
    });
    let scan = scan_at(
        &client().unwrap(),
        port,
        "fixture-token",
        99,
        &summaries,
        None,
    )
    .await
    .unwrap();
    assert_eq!(scan.errors, 0);
    assert_eq!(scan.streams.len(), 1);
    assert_eq!(scan.streams[0].record_count, 2307);
    assert_eq!(scan.streams[0].settled_count, 2307);
    assert_eq!(scan.streams[0].events.len(), 1);
    assert_eq!(scan.streams[0].events[0].position, 2307);
    assert_eq!(scan.streams[0].events[0].turn_id, "exec-99");
    assert_eq!(
        scan.streams[0].events[0].body,
        "Long session completed via incremental steps."
    );
    task.abort();
}
