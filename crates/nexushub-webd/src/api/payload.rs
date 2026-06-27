use super::{api_error, ApiError};
use crate::rpc_payload::{
    rpc_nested_payload as parse_rpc_nested_payload,
    rpc_nested_payload_or_empty as parse_rpc_nested_payload_or_empty,
    rpc_payload as parse_rpc_payload, rpc_payload_or_empty as parse_rpc_payload_or_empty,
    rpc_query_strings as parse_rpc_query_strings, rpc_required_string as parse_rpc_required_string,
    rpc_string as parse_rpc_string, rpc_wrapped_payload as parse_rpc_wrapped_payload,
    RpcPayloadError,
};
use axum::http::StatusCode;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashMap;

pub(super) fn rpc_payload<T: DeserializeOwned>(value: &Value) -> Result<T, ApiError> {
    api_rpc_payload(parse_rpc_payload(value))
}

pub(super) fn rpc_payload_or_empty<T: DeserializeOwned>(value: &Value) -> Result<T, ApiError> {
    api_rpc_payload(parse_rpc_payload_or_empty(value))
}

pub(super) fn rpc_nested_payload<T: DeserializeOwned>(
    value: &Value,
    key: &str,
) -> Result<T, ApiError> {
    api_rpc_payload(parse_rpc_nested_payload(value, key))
}

pub(super) fn rpc_wrapped_payload<T: DeserializeOwned>(
    value: &Value,
    keys: &[&str],
) -> Result<T, ApiError> {
    api_rpc_payload(parse_rpc_wrapped_payload(value, keys))
}

#[cfg(test)]
pub(super) fn rpc_wrapped_payload_or_empty<T: DeserializeOwned>(
    value: &Value,
    keys: &[&str],
) -> Result<T, ApiError> {
    api_rpc_payload(crate::rpc_payload::rpc_wrapped_payload_or_empty(
        value, keys,
    ))
}

pub(super) fn rpc_nested_payload_or_empty<T: DeserializeOwned>(
    value: &Value,
    key: &str,
) -> Result<T, ApiError> {
    api_rpc_payload(parse_rpc_nested_payload_or_empty(value, key))
}

pub(super) fn rpc_required_string(value: &Value, key: &str) -> Result<String, ApiError> {
    api_rpc_payload(parse_rpc_required_string(value, key))
}

pub(super) fn rpc_string(value: &Value, key: &str) -> Option<String> {
    parse_rpc_string(value, key)
}

pub(super) fn rpc_query_strings(value: &Value, keys: &[&str]) -> HashMap<String, String> {
    parse_rpc_query_strings(value, keys)
}

fn api_rpc_payload<T>(result: Result<T, RpcPayloadError>) -> Result<T, ApiError> {
    result.map_err(|err| api_error(StatusCode::BAD_REQUEST, err.message()))
}

#[cfg(test)]
mod tests {
    use super::{rpc_required_string, rpc_wrapped_payload, rpc_wrapped_payload_or_empty};
    use crate::api::{ApiError, LoginRequest};
    use nexushub_core::services::settings as settings_service;
    use nexushub_core::services::{goals as goal_service, jobs::ThreadMessageRequest};
    use serde_json::json;

    fn must<T>(result: Result<T, ApiError>) -> T {
        match result {
            Ok(value) => value,
            Err(_) => panic!("expected rpc compatibility conversion to succeed"),
        }
    }

    #[test]
    fn rpc_payload_compat_accepts_camel_case_login_token() {
        let payload: LoginRequest = must(rpc_wrapped_payload(
            &json!({
                "username": "admin",
                "password": "secret",
                "turnstileToken": "token-a"
            }),
            &[],
        ));

        assert_eq!(payload.username, "admin");
        assert_eq!(payload.turnstile_token.as_deref(), Some("token-a"));
    }

    #[test]
    fn rpc_payload_compat_accepts_nested_and_top_level_thread_payloads() {
        let nested: ThreadMessageRequest = must(rpc_wrapped_payload(
            &json!({
                "payload": {
                    "message": "start",
                    "serviceTier": "priority",
                    "reasoningEffort": "xhigh",
                    "permissionProfile": "danger-full-access",
                    "approvalPolicy": "never",
                    "sandboxMode": "danger-full-access",
                    "networkAccess": true,
                    "collaborationMode": "async"
                },
                "csrfToken": "ignored"
            }),
            &["payload"],
        ));
        assert_eq!(nested.message, "start");
        assert_eq!(nested.service_tier.as_deref(), Some("priority"));
        assert_eq!(nested.reasoning_effort.as_deref(), Some("xhigh"));
        assert_eq!(
            nested.permission_profile.as_deref(),
            Some("danger-full-access")
        );
        assert_eq!(nested.approval_policy.as_deref(), Some("never"));
        assert_eq!(nested.sandbox_mode.as_deref(), Some("danger-full-access"));
        assert_eq!(nested.network_access, Some(true));
        assert_eq!(nested.collaboration_mode.as_deref(), Some("async"));

        let top_level: ThreadMessageRequest = must(rpc_wrapped_payload(
            &json!({
                "message": "continue",
                "preparedAttachments": [],
                "serviceTier": "default"
            }),
            &["payload"],
        ));
        assert_eq!(top_level.message, "continue");
        assert_eq!(top_level.service_tier.as_deref(), Some("default"));
        assert!(top_level.prepared_attachments.is_empty());
    }

    #[test]
    fn rpc_payload_compat_accepts_goal_request_wrapper_and_aliases() {
        let payload: goal_service::GoalUpdateRequest = must(rpc_wrapped_payload(
            &json!({
                "request": {
                    "threadId": "thread-a",
                    "objective": "ship",
                    "tokenBudget": 4096
                }
            }),
            &["request", "payload"],
        ));

        assert_eq!(payload.thread_id.as_deref(), Some("thread-a"));
        assert_eq!(payload.objective.as_deref(), Some("ship"));
        assert_eq!(payload.token_budget, Some(4096));
    }

    #[test]
    fn rpc_payload_compat_accepts_probe_settings_wrapper() {
        let payload: settings_service::ProbeSettingsSaveRequest = must(rpc_wrapped_payload(
            &json!({
            "settings": {
                "probe": {
                    "pollSeconds": 20,
                    "recentLimit": 50
                    },
                    "notifications": {
                        "serverUrl": "https://example.invalid",
                        "deviceKey": "bark",
                        "notifyReplyNeeded": true
                    }
                }
            }),
            &["settings", "payload"],
        ));

        assert_eq!(payload.probe.unwrap().poll_seconds, Some(20));
        let notifications = payload.notifications.unwrap();
        assert_eq!(
            notifications.server_url.as_deref(),
            Some("https://example.invalid")
        );
        assert_eq!(notifications.device_key.as_deref(), Some("bark"));
        assert_eq!(notifications.notify_reply_needed, Some(true));
    }

    #[test]
    fn rpc_payload_compat_extracts_thread_id_from_payload_or_top_level() {
        assert_eq!(
            must(rpc_required_string(
                &json!({"payload": {"threadId": "nested-thread"}}),
                "threadId"
            )),
            "nested-thread"
        );
        assert_eq!(
            must(rpc_required_string(
                &json!({"thread_id": "snake-thread"}),
                "threadId"
            )),
            "snake-thread"
        );
        let empty: serde_json::Value = must(rpc_wrapped_payload_or_empty(
            &serde_json::Value::Null,
            &["payload"],
        ));
        assert_eq!(empty, json!({}));
    }
}
