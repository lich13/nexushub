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
