use super::{api_error, http_update_platform, ok, ApiError, ApiResponse};
use crate::{
    auth::{require_auth, require_csrf},
    linux_adapter,
    state::AppState,
};
use axum::{
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
};
use nexushub_core::{
    services::{uploads as upload_service, use_cases::NexusHubUseCases},
    uploads,
};
use serde_json::json;

pub(crate) async fn upload_files(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    linux_adapter::cleanup_stale_uploads_plan(&state)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    let result: Result<uploads::UploadOutcome, ApiError> = async {
        let mut items = Vec::new();
        while let Some(field) = multipart.next_field().await.map_err(|err| {
            api_error(
                StatusCode::BAD_REQUEST,
                &format!("invalid upload body: {err}"),
            )
        })? {
            let Some(file_name) = field.file_name().map(str::to_string) else {
                continue;
            };
            let mime = field.content_type().map(str::to_string);
            let bytes = field.bytes().await.map_err(|err| {
                api_error(
                    StatusCode::BAD_REQUEST,
                    &format!("read upload failed: {err}"),
                )
            })?;
            items.push(upload_service::UploadBatchItem {
                name: file_name,
                mime,
                bytes: bytes.to_vec(),
            });
        }
        let platform = http_update_platform();
        let facade = NexusHubUseCases::new(&platform)
            .uploads()
            .store(items)
            .map_err(|err| upload_service_error(err.to_string()))?;
        let outcome = linux_adapter::store_upload_plan(&state, &auth, facade.plan)
            .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
        Ok(outcome)
    }
    .await;
    ok(result?)
}

fn upload_service_error(message: String) -> ApiError {
    let status = if message.contains("一次最多上传")
        || message.contains("单个文件不能超过")
        || message.contains("一次上传总大小不能超过")
    {
        StatusCode::PAYLOAD_TOO_LARGE
    } else {
        StatusCode::BAD_REQUEST
    };
    api_error(status, &message)
}

pub(crate) async fn delete_upload_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResponse {
    let auth = require_auth(&headers, &state).map_err(|s| api_error(s, "unauthorized"))?;
    require_csrf(&headers, &auth).map_err(|s| api_error(s, "csrf failed"))?;
    let platform = http_update_platform();
    let plan = NexusHubUseCases::new(&platform)
        .uploads()
        .delete(&id)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    let deleted = linux_adapter::delete_upload_plan(&state, &auth, plan)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, &err.to_string()))?;
    ok(json!({"ok": true, "deleted": deleted}))
}
