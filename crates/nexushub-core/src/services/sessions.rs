use crate::{
    codex::{self, CodexPaths},
    grok::{self, GrokPaths},
    selected_codex,
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};

pub const MAX_BATCH_SIZE: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionProvider {
    Codex,
    #[serde(rename = "claude_code")]
    Claude,
    Grok,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionOperation {
    Archive,
    Restore,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionBatchRequest {
    pub provider: SessionProvider,
    pub operation: SessionOperation,
    pub session_keys: Vec<String>,
    #[serde(default)]
    pub include_descendants: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBatchItem {
    pub session_key: String,
    pub id: String,
    pub title: String,
    pub paths: Vec<PathBuf>,
    pub bytes: u64,
    pub allowed: bool,
    pub reason: Option<String>,
    pub fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub descendants: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBatchPreview {
    pub provider: SessionProvider,
    pub operation: SessionOperation,
    pub items: Vec<SessionBatchItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionBatchSelection {
    pub session_key: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionBatchExecuteRequest {
    pub provider: SessionProvider,
    pub operation: SessionOperation,
    pub items: Vec<SessionBatchSelection>,
    #[serde(default)]
    pub confirmed: bool,
    #[serde(default)]
    pub include_descendants: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBatchItemResult {
    pub session_key: String,
    pub status: String,
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deleted_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub residuals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBatchResult {
    pub items: Vec<SessionBatchItemResult>,
}

pub struct SessionUseCases {
    pub platform: crate::platform::PlatformPaths,
    pub codex: CodexPaths,
    pub grok: GrokPaths,
    pub claude: crate::claude::ClaudePaths,
}

fn validate(provider: SessionProvider, operation: SessionOperation, keys: &[String]) -> Result<()> {
    ensure!(
        !keys.is_empty() && keys.len() <= MAX_BATCH_SIZE,
        "每批请选择 1 至 100 个线程"
    );
    ensure!(
        provider == SessionProvider::Codex || operation == SessionOperation::Delete,
        "此 provider 不支持归档或恢复"
    );
    let mut unique = HashSet::new();
    for key in keys {
        ensure!(
            !key.trim().is_empty() && key.len() <= 512 && !key.chars().any(char::is_control),
            "无效的线程定位键"
        );
        ensure!(unique.insert(key), "同一批次不能重复选择线程");
    }
    Ok(())
}

impl SessionUseCases {
    pub fn attachment_read(
        &self,
        request: crate::user_message::SessionAttachmentRequest,
    ) -> Result<crate::user_message::SessionAttachmentResponse> {
        for value in [
            &request.session_key,
            &request.message_id,
            &request.attachment_id,
        ] {
            ensure!(
                !value.is_empty() && value.len() <= 512 && !value.chars().any(char::is_control),
                "无效的附件定位键"
            );
        }
        if request.root_thread_id.is_some() {
            ensure!(
                request.provider == SessionProvider::Codex,
                "子智能体附件仅支持 Codex"
            );
            return codex::subagents::read_attachment(&self.codex, &request);
        }
        let message = match request.provider {
            SessionProvider::Claude => {
                return crate::claude::read_attachment(&self.claude, &request);
            }
            SessionProvider::Codex => codex::thread_detail(&self.codex, &request.session_key)?
                .into_iter()
                .flat_map(|detail| detail.blocks)
                .filter_map(|block| block.user_message)
                .find(|message| message.id == request.message_id),
            SessionProvider::Grok => {
                grok::grok_session_detail(&self.grok, &request.session_key, None)?
                    .events
                    .iter()
                    .filter_map(|event| event.user_message.as_ref())
                    .find(|message| message.id == request.message_id)
                    .cloned()
            }
        }
        .ok_or_else(|| anyhow::anyhow!("会话消息已变化或不存在，请刷新会话"))?;
        message.read_attachment(&request.attachment_id)
    }

    pub fn bulk_preview(&self, request: SessionBatchRequest) -> Result<SessionBatchPreview> {
        crate::services::system::require_capability(
            &self.platform,
            crate::services::system::Capability::ThreadCleanup,
        )?;
        validate(request.provider, request.operation, &request.session_keys)?;
        let grok = (request.provider == SessionProvider::Grok)
            .then(|| grok::DeletionContext::new(&self.grok));
        let items = request
            .session_keys
            .iter()
            .map(|key| {
                self.preview_one(
                    request.provider,
                    request.operation,
                    key,
                    request.include_descendants,
                    grok.as_ref(),
                )
                .unwrap_or_else(|error| {
                    self.blocked_item(request.provider, key, format!("{error:#}"), grok.as_ref())
                })
            })
            .collect();
        Ok(SessionBatchPreview {
            provider: request.provider,
            operation: request.operation,
            items,
        })
    }

    fn blocked_item(
        &self,
        provider: SessionProvider,
        key: &str,
        reason: String,
        grok: Option<&grok::DeletionContext<'_>>,
    ) -> SessionBatchItem {
        let mut item = SessionBatchItem {
            session_key: key.to_string(),
            id: key.to_string(),
            title: key.to_string(),
            paths: vec![],
            bytes: 0,
            allowed: false,
            reason: Some(reason),
            fingerprint: None,
            descendants: vec![],
            warnings: vec![],
        };
        let summary = match provider {
            SessionProvider::Claude => crate::claude::claude_session_summary(&self.claude, key)
                .ok()
                .map(|s| (s.id, s.title, vec![s.path])),
            SessionProvider::Codex => codex::local_thread_summary(&self.codex, key)
                .ok()
                .flatten()
                .map(|s| {
                    (
                        s.id,
                        s.title,
                        s.rollout_path.into_iter().collect::<Vec<_>>(),
                    )
                }),
            SessionProvider::Grok => grok
                .map_or_else(
                    || grok::grok_session_summary(&self.grok, key),
                    |context| context.summary(key),
                )
                .ok()
                .map(|s| (s.id, s.title, vec![s.path])),
        };
        if let Some((id, title, paths)) = summary {
            item.id = id;
            item.title = title;
            item.paths = paths;
            item.bytes = item
                .paths
                .iter()
                .flat_map(|p| {
                    walkdir::WalkDir::new(p)
                        .follow_links(false)
                        .max_depth(16)
                        .into_iter()
                        .take(10_001)
                })
                .filter_map(Result::ok)
                .filter(|e| e.file_type().is_file())
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum();
        }
        item
    }

    fn preview_one(
        &self,
        provider: SessionProvider,
        operation: SessionOperation,
        key: &str,
        include_descendants: bool,
        grok: Option<&grok::DeletionContext<'_>>,
    ) -> Result<SessionBatchItem> {
        let mut descendants = Vec::new();
        let mut warnings = Vec::new();
        let (id, title, paths, bytes, fingerprint) = match provider {
            SessionProvider::Claude => {
                let p = crate::claude::preview_claude_delete(&self.claude, key)?;
                (p.id, p.title, vec![p.path], p.bytes, p.fingerprint)
            }
            SessionProvider::Codex => {
                ensure!(
                    codex::codex_task_identity(&self.codex, key)? == codex::CodexTaskIdentity::Main,
                    "线程身份无法确认为主任务"
                );
                if operation == SessionOperation::Delete {
                    let tree =
                        selected_codex::tree::preview(&self.codex, key, include_descendants)?;
                    descendants = tree.ids.into_iter().filter(|id| id != key).collect();
                    warnings = tree.warnings;
                    let p = tree.selected;
                    (p.id, p.title, p.paths, p.bytes, p.fingerprint)
                } else {
                    let p = selected_codex::preview_archive(
                        &self.codex,
                        key,
                        operation == SessionOperation::Archive,
                    )?;
                    (p.id, p.title, p.paths, p.bytes, p.fingerprint)
                }
            }
            SessionProvider::Grok => {
                let p = grok.map_or_else(
                    || grok::preview_grok_delete(&self.grok, key),
                    |context| context.preview(key),
                )?;
                (p.id, p.title, vec![p.path], p.bytes, p.fingerprint)
            }
        };
        Ok(SessionBatchItem {
            session_key: key.to_string(),
            id,
            title,
            paths,
            bytes,
            allowed: true,
            reason: None,
            fingerprint: Some(fingerprint),
            descendants,
            warnings,
        })
    }

    pub fn bulk_execute(&self, request: SessionBatchExecuteRequest) -> Result<SessionBatchResult> {
        crate::services::system::require_capability(
            &self.platform,
            crate::services::system::Capability::ThreadCleanup,
        )?;
        let keys = request
            .items
            .iter()
            .map(|s| s.session_key.clone())
            .collect::<Vec<_>>();
        validate(request.provider, request.operation, &keys)?;
        ensure!(
            request.operation != SessionOperation::Delete || request.confirmed,
            "删除必须经过预览并明确确认"
        );
        ensure!(
            request
                .items
                .iter()
                .all(|s| !s.fingerprint.is_empty() && s.fingerprint.len() <= 256),
            "缺少预览指纹，请重新预览"
        );
        let mut results = Vec::with_capacity(request.items.len());
        let mut storage_failed = false;
        for item in &request.items {
            if storage_failed {
                results.push(SessionBatchItemResult {
                    session_key: item.session_key.clone(),
                    status: "blocked".into(),
                    message: Some("共享存储核验失败，已停止后续删除，请检查后重新预览".into()),
                    deleted_ids: vec![],
                    residuals: vec![],
                });
                continue;
            }
            let check = self
                .preview_one(
                    request.provider,
                    request.operation,
                    &item.session_key,
                    request.include_descendants,
                    None,
                )
                .and_then(|current| {
                    ensure!(
                        current.fingerprint.as_deref() == Some(&item.fingerprint),
                        "线程或文件已变化，请重新预览"
                    );
                    Ok(())
                });
            if let Err(error) = check {
                results.push(SessionBatchItemResult {
                    session_key: item.session_key.clone(),
                    status: "blocked".into(),
                    message: Some(format!("{error:#}")),
                    deleted_ids: vec![],
                    residuals: vec![],
                });
                continue;
            }
            if request.provider == SessionProvider::Codex
                && request.operation == SessionOperation::Delete
            {
                let outcome = selected_codex::tree::execute(
                    &self.codex,
                    &item.session_key,
                    request.include_descendants,
                    &item.fingerprint,
                );
                results.push(match outcome {
                    Ok(outcome) => {
                        storage_failed = outcome.storage_failure;
                        SessionBatchItemResult {
                            session_key: item.session_key.clone(),
                            status: if outcome.error.is_none() {
                                "succeeded"
                            } else {
                                "failed"
                            }
                            .into(),
                            message: outcome.error,
                            deleted_ids: outcome.deleted_ids,
                            residuals: outcome.residuals,
                        }
                    }
                    Err(error) => SessionBatchItemResult {
                        session_key: item.session_key.clone(),
                        status: "blocked".into(),
                        message: Some(format!("{error:#}")),
                        deleted_ids: vec![],
                        residuals: vec![],
                    },
                });
                continue;
            }
            let result = match (request.provider, request.operation) {
                (SessionProvider::Claude, _) => crate::claude::execute_claude_delete(
                    &self.claude,
                    crate::claude::ClaudeDeleteRequest {
                        session_key: item.session_key.clone(),
                        confirmed: true,
                        fingerprint: item.fingerprint.clone(),
                    },
                )
                .map(|_| ()),
                (SessionProvider::Codex, SessionOperation::Delete) => {
                    unreachable!("Codex deletion is handled by the tree verifier above")
                }
                (SessionProvider::Codex, action) => selected_codex::execute_archive(
                    &self.codex,
                    &item.session_key,
                    action == SessionOperation::Archive,
                    &item.fingerprint,
                ),
                (SessionProvider::Grok, _) => grok::execute_grok_delete(
                    &self.grok,
                    grok::GrokDeleteRequest {
                        id: item.session_key.clone(),
                        confirmed: true,
                        fingerprint: item.fingerprint.clone(),
                    },
                )
                .map(|_| ()),
            };
            results.push(SessionBatchItemResult {
                session_key: item.session_key.clone(),
                status: if result.is_ok() {
                    "succeeded"
                } else {
                    "failed"
                }
                .into(),
                message: result.err().map(|e| format!("{e:#}")),
                deleted_ids: vec![],
                residuals: vec![],
            });
        }
        Ok(SessionBatchResult { items: results })
    }
}
