use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::uploads::{
    self as upload_core, UploadKind, UploadOutcome, UploadRecord, MAX_TOTAL_UPLOAD_BYTES,
    MAX_UPLOAD_FILES, MAX_UPLOAD_FILE_BYTES,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadBatchItem {
    pub name: String,
    pub mime: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadStoreItem {
    pub name: String,
    pub mime: String,
    pub kind: UploadKind,
    pub size: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadStorePlan {
    pub items: Vec<UploadStoreItem>,
    pub total_files: usize,
    pub total_bytes: usize,
}

pub fn validate_upload_batch(items: &[UploadBatchItem]) -> Result<usize> {
    if items.is_empty() {
        bail!("没有可上传的文件");
    }
    if items.len() > MAX_UPLOAD_FILES {
        bail!("一次最多上传 5 个文件");
    }

    let mut total_bytes = 0usize;
    for item in items {
        if item.bytes.is_empty() {
            bail!("空文件不能上传");
        }
        if item.bytes.len() > MAX_UPLOAD_FILE_BYTES {
            bail!("单个文件不能超过 10 MiB");
        }
        total_bytes += item.bytes.len();
        if total_bytes > MAX_TOTAL_UPLOAD_BYTES {
            bail!("一次上传总大小不能超过 30 MiB");
        }
    }
    Ok(total_bytes)
}

pub fn plan_store_uploads(items: Vec<UploadBatchItem>) -> Result<UploadStorePlan> {
    let total_bytes = validate_upload_batch(&items)?;
    let total_files = items.len();
    let mut planned = Vec::with_capacity(total_files);
    for item in items {
        let name = sanitize_file_name(&item.name);
        let mime = upload_mime(&item.name, &name, item.mime.as_deref());
        let kind = upload_core::classify_upload(&name, &mime)?;
        planned.push(UploadStoreItem {
            size: item.bytes.len() as u64,
            name,
            mime,
            kind,
            bytes: item.bytes,
        });
    }

    Ok(UploadStorePlan {
        items: planned,
        total_files,
        total_bytes,
    })
}

pub fn plan_desktop_batch_uploads(items: Vec<UploadBatchItem>) -> Result<UploadStorePlan> {
    plan_store_uploads(items)
}

pub fn store_upload_plan(root: &Path, plan: UploadStorePlan) -> Result<UploadOutcome> {
    let mut files: Vec<UploadRecord> = Vec::with_capacity(plan.items.len());
    let result: Result<UploadOutcome> = (|| {
        for item in plan.items {
            files.push(upload_core::store_upload(
                root,
                &item.name,
                Some(&item.mime),
                &item.bytes,
            )?);
        }
        Ok(UploadOutcome {
            files: files.clone(),
        })
    })();

    if result.is_err() {
        let ids = files.iter().map(|file| file.id.clone()).collect::<Vec<_>>();
        upload_core::cleanup_upload_ids(root, &ids);
    }

    result.with_context(|| format!("store uploads in {}", root.display()))
}

fn upload_mime(original_name: &str, safe_name: &str, mime: Option<&str>) -> String {
    mime.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            mime_guess::from_path(original_name.trim())
                .first_raw()
                .or_else(|| mime_guess::from_path(safe_name).first_raw())
                .unwrap_or("application/octet-stream")
                .to_string()
        })
}

fn sanitize_file_name(name: &str) -> String {
    let trimmed = name.trim();
    let base = Path::new(trimmed)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("attachment")
        .trim();
    let extension = extension(base);
    let stem = Path::new(base)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("attachment");
    let cleaned_stem: String = stem
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || matches!(ch, '-' | '_' | ' ') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let trimmed_stem = cleaned_stem.trim_matches([' ', '_']).trim();
    let stem = if trimmed_stem.is_empty() {
        "attachment".to_string()
    } else {
        trimmed_stem.chars().take(160).collect()
    };
    if extension.is_empty() {
        stem
    } else {
        format!("{stem}.{extension}")
    }
}

fn extension(name: &str) -> String {
    PathBuf::from(name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use crate::{
        services::uploads::{
            plan_desktop_batch_uploads, plan_store_uploads, validate_upload_batch, UploadBatchItem,
            UploadStorePlan,
        },
        uploads::UploadKind,
    };

    #[test]
    fn upload_batch_validation_rejects_empty_too_many_and_oversized_payloads() {
        assert!(validate_upload_batch(&[])
            .unwrap_err()
            .to_string()
            .contains("没有可上传的文件"));

        let too_many = (0..6)
            .map(|idx| UploadBatchItem {
                name: format!("file-{idx}.txt"),
                mime: Some("text/plain".to_string()),
                bytes: b"hello".to_vec(),
            })
            .collect::<Vec<_>>();
        assert!(validate_upload_batch(&too_many)
            .unwrap_err()
            .to_string()
            .contains("一次最多上传 5 个文件"));

        let oversized = vec![UploadBatchItem {
            name: "huge.bin".to_string(),
            mime: Some("application/octet-stream".to_string()),
            bytes: vec![0; 10 * 1024 * 1024 + 1],
        }];
        assert!(validate_upload_batch(&oversized)
            .unwrap_err()
            .to_string()
            .contains("单个文件不能超过 10 MiB"));

        let too_large_total = (0..4)
            .map(|idx| UploadBatchItem {
                name: format!("part-{idx}.bin"),
                mime: Some("application/octet-stream".to_string()),
                bytes: vec![idx as u8; 8 * 1024 * 1024],
            })
            .collect::<Vec<_>>();
        assert!(validate_upload_batch(&too_large_total)
            .unwrap_err()
            .to_string()
            .contains("一次上传总大小不能超过 30 MiB"));
    }

    #[test]
    fn upload_store_plan_classifies_and_summarizes_without_transport_dependencies() {
        let items = vec![
            UploadBatchItem {
                name: " ../notes.md ".to_string(),
                mime: Some("text/markdown".to_string()),
                bytes: b"# Notes".to_vec(),
            },
            UploadBatchItem {
                name: "table.csv".to_string(),
                mime: None,
                bytes: b"name,count\nalpha,2".to_vec(),
            },
        ];

        let plan: UploadStorePlan = plan_store_uploads(items).unwrap();

        assert_eq!(plan.total_files, 2);
        assert_eq!(plan.total_bytes, 25);
        assert_eq!(plan.items[0].name, "notes.md");
        assert_eq!(plan.items[0].mime, "text/markdown");
        assert_eq!(plan.items[0].kind, UploadKind::Markdown);
        assert_eq!(plan.items[1].mime, "text/csv");
        assert_eq!(plan.items[1].kind, UploadKind::Spreadsheet);
    }

    #[test]
    fn desktop_batch_uploads_share_http_store_plan_validation_and_mime_inference() {
        let batch = vec![
            UploadBatchItem {
                name: "notes.md".to_string(),
                mime: None,
                bytes: b"# Notes".to_vec(),
            },
            UploadBatchItem {
                name: "report.csv".to_string(),
                mime: None,
                bytes: b"name,count\nalpha,2".to_vec(),
            },
        ];

        let http_plan = plan_store_uploads(batch.clone()).unwrap();
        let desktop_plan = plan_desktop_batch_uploads(batch).unwrap();

        assert_eq!(desktop_plan, http_plan);
        assert_eq!(desktop_plan.items[0].mime, "text/markdown");
        assert_eq!(desktop_plan.items[0].kind, UploadKind::Markdown);
        assert_eq!(desktop_plan.items[1].mime, "text/csv");
        assert_eq!(desktop_plan.items[1].kind, UploadKind::Spreadsheet);

        let too_many = (0..6)
            .map(|idx| UploadBatchItem {
                name: format!("file-{idx}.txt"),
                mime: None,
                bytes: b"hello".to_vec(),
            })
            .collect::<Vec<_>>();
        assert!(plan_desktop_batch_uploads(too_many)
            .unwrap_err()
            .to_string()
            .contains("一次最多上传 5 个文件"));

        assert!(plan_desktop_batch_uploads(vec![UploadBatchItem {
            name: "huge.bin".to_string(),
            mime: None,
            bytes: vec![0; 10 * 1024 * 1024 + 1],
        }])
        .unwrap_err()
        .to_string()
        .contains("单个文件不能超过 10 MiB"));

        let too_large_total = (0..4)
            .map(|idx| UploadBatchItem {
                name: format!("part-{idx}.bin"),
                mime: None,
                bytes: vec![idx as u8; 8 * 1024 * 1024],
            })
            .collect::<Vec<_>>();
        assert!(plan_desktop_batch_uploads(too_large_total)
            .unwrap_err()
            .to_string()
            .contains("一次上传总大小不能超过 30 MiB"));
    }
}
