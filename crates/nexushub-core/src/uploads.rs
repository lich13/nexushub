use anyhow::{anyhow, bail, Context, Result};
use calamine::{open_workbook_auto, Reader};
use csv::ReaderBuilder;
use lopdf::Document;
use quick_xml::{events::Event, Reader as XmlReader};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use uuid::Uuid;
use zip::ZipArchive;

pub const MAX_UPLOAD_FILES: usize = 5;
pub const MAX_UPLOAD_FILE_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_TOTAL_UPLOAD_BYTES: usize = 30 * 1024 * 1024;
pub const UPLOAD_TTL_SECONDS: u64 = 24 * 60 * 60;
const MAX_TEXT_CHARS: usize = 65_536;
const MAX_TABLE_ROWS: usize = 30;
const MAX_TABLE_COLS: usize = 12;
const MAX_SHEETS: usize = 8;
const MAX_PDF_PAGES: usize = 25;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UploadKind {
    Text,
    Markdown,
    Spreadsheet,
    Document,
    Pdf,
    Image,
    File,
}

impl UploadKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Markdown => "markdown",
            Self::Spreadsheet => "spreadsheet",
            Self::Document => "document",
            Self::Pdf => "pdf",
            Self::Image => "image",
            Self::File => "file",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadRecord {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub sha256: String,
    pub kind: UploadKind,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedAttachment {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub size: u64,
    pub sha256: String,
    pub kind: UploadKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_image_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_file_path: Option<PathBuf>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadOutcome {
    pub files: Vec<UploadRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredUploadMeta {
    record: UploadRecord,
    content_file: String,
}

pub fn upload_root(codex_home: &Path) -> PathBuf {
    codex_home.join("nexushub").join("uploads")
}

pub fn classify_upload(name: &str, mime: &str) -> Result<UploadKind> {
    let ext = extension(name);
    let mime = mime.trim().to_ascii_lowercase();
    if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp")
        || matches!(mime.as_str(), "image/png" | "image/jpeg" | "image/webp")
    {
        return Ok(UploadKind::Image);
    }
    if matches!(ext.as_str(), "md" | "markdown") {
        return Ok(UploadKind::Markdown);
    }
    if matches!(ext.as_str(), "csv" | "tsv" | "xlsx" | "xls")
        || matches!(
            mime.as_str(),
            "text/csv"
                | "text/tab-separated-values"
                | "application/vnd.ms-excel"
                | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        )
    {
        return Ok(UploadKind::Spreadsheet);
    }
    if ext == "docx"
        || mime == "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    {
        return Ok(UploadKind::Document);
    }
    if ext == "pdf" || mime == "application/pdf" {
        return Ok(UploadKind::Pdf);
    }
    if is_text_extension(&ext) || mime.starts_with("text/") {
        return Ok(UploadKind::Text);
    }
    Ok(UploadKind::File)
}

pub fn store_upload(
    root: &Path,
    name: &str,
    mime: Option<&str>,
    bytes: &[u8],
) -> Result<UploadRecord> {
    if bytes.is_empty() {
        bail!("空文件不能上传");
    }
    if bytes.len() > MAX_UPLOAD_FILE_BYTES {
        bail!("单个文件不能超过 10 MiB");
    }
    let safe_name = sanitize_file_name(name);
    let guessed = mime_guess::from_path(name)
        .first_raw()
        .or_else(|| mime_guess::from_path(&safe_name).first_raw())
        .unwrap_or("application/octet-stream");
    let mime = mime
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(guessed)
        .to_string();
    let kind = classify_upload(name, &mime)?;
    let id = Uuid::new_v4().to_string();
    let dir = root.join(&id);
    fs::create_dir_all(&dir).with_context(|| format!("create upload dir {}", dir.display()))?;
    let content_file = content_file_name(&safe_name);
    let content_path = dir.join(&content_file);
    fs::write(&content_path, bytes)
        .with_context(|| format!("write upload {}", content_path.display()))?;
    let sha256 = hex::encode(Sha256::digest(bytes));
    let record = UploadRecord {
        id,
        name: safe_name,
        mime,
        size: bytes.len() as u64,
        sha256,
        kind,
        status: "ready".to_string(),
        error_preview: None,
    };
    let meta = StoredUploadMeta {
        record: record.clone(),
        content_file,
    };
    fs::write(
        dir.join("meta.json"),
        serde_json::to_vec_pretty(&meta).context("serialize upload metadata")?,
    )
    .with_context(|| format!("write upload metadata {}", dir.display()))?;
    Ok(record)
}

pub fn delete_upload(root: &Path, id: &str) -> Result<bool> {
    let dir = upload_dir(root, id)?;
    if !dir.exists() {
        return Ok(false);
    }
    fs::remove_dir_all(&dir).with_context(|| format!("delete upload {}", dir.display()))?;
    Ok(true)
}

pub fn prepare_uploads(root: &Path, ids: &[String]) -> Result<Vec<PreparedAttachment>> {
    let mut prepared = Vec::new();
    for id in ids {
        if id.trim().is_empty() {
            continue;
        }
        prepared.push(prepare_upload(root, id)?);
    }
    Ok(prepared)
}

pub fn cleanup_upload_ids(root: &Path, ids: &[String]) {
    for id in ids {
        let _ = delete_upload(root, id);
    }
}

pub fn cleanup_stale_uploads(root: &Path, ttl: Duration) -> Result<usize> {
    cleanup_stale_uploads_except(root, ttl, &HashSet::new())
}

pub fn cleanup_stale_uploads_except(
    root: &Path,
    ttl: Duration,
    protected_ids: &HashSet<String>,
) -> Result<usize> {
    if !root.exists() {
        return Ok(0);
    }
    let cutoff = SystemTime::now()
        .checked_sub(ttl)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut removed = 0usize;
    for entry in
        fs::read_dir(root).with_context(|| format!("read upload root {}", root.display()))?
    {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if Uuid::parse_str(name).is_err() {
            continue;
        }
        if protected_ids.contains(name) {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(SystemTime::now());
        if modified < cutoff && fs::remove_dir_all(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

pub fn prompt_with_attachment_context(message: &str, attachments: &[PreparedAttachment]) -> String {
    let message = message.trim();
    let mut out = String::new();
    if message.is_empty() && !attachments.is_empty() {
        out.push_str("请根据以下附件内容继续处理。");
    } else {
        out.push_str(message);
    }
    let context = attachment_context(attachments);
    if !context.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(&context);
    }
    out
}

pub fn attachment_context(attachments: &[PreparedAttachment]) -> String {
    if attachments.is_empty() {
        return String::new();
    }
    let mut out = String::from("## 附加文件上下文\n");
    for attachment in attachments {
        out.push_str(&format!(
            "\n### 附件: {}\n- 类型: {}\n- MIME: {}\n- 大小: {} bytes\n- SHA-256: {}\n",
            attachment.name,
            attachment.kind.as_str(),
            attachment.mime,
            attachment.size,
            attachment.sha256
        ));
        if let Some(path) = &attachment.local_image_path {
            out.push_str(&format!("- 本地图片路径: {}\n", path.display()));
        }
        if let Some(path) = &attachment.local_file_path {
            out.push_str(&format!("- 本地文件路径: {}\n", path.display()));
        }
        if attachment.truncated {
            out.push_str("- 注意: 内容已截断，仅包含前部可读片段。\n");
        }
        if let Some(text) = attachment
            .text
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            out.push('\n');
            out.push_str(text);
            out.push('\n');
        }
    }
    out
}

fn prepare_upload(root: &Path, id: &str) -> Result<PreparedAttachment> {
    let dir = upload_dir(root, id)?;
    let meta_path = dir.join("meta.json");
    let meta: StoredUploadMeta = serde_json::from_slice(
        &fs::read(&meta_path)
            .with_context(|| format!("read upload metadata {}", meta_path.display()))?,
    )
    .context("parse upload metadata")?;
    let path = dir.join(&meta.content_file);
    let bytes = fs::read(&path).with_context(|| format!("read upload {}", path.display()))?;
    let (text, truncated, local_image_path, local_file_path) = match meta.record.kind {
        UploadKind::Image => (None, false, Some(path), None),
        UploadKind::File => (None, false, None, Some(path)),
        UploadKind::Markdown => {
            let (text, truncated) = decode_text(&bytes)?;
            (Some(text), truncated, None, None)
        }
        UploadKind::Text => {
            let (text, truncated) = decode_text(&bytes)?;
            (
                Some(format!("```text\n{}\n```", text)),
                truncated,
                None,
                None,
            )
        }
        UploadKind::Spreadsheet => {
            let (text, truncated) = extract_spreadsheet_text(&path, &meta.record.name, &bytes)?;
            (Some(text), truncated, None, None)
        }
        UploadKind::Document => {
            let (text, truncated) = extract_docx_text(&bytes)?;
            (Some(text), truncated, None, None)
        }
        UploadKind::Pdf => {
            let (text, truncated) = extract_pdf_text(&path)?;
            (Some(text), truncated, None, None)
        }
    };
    Ok(PreparedAttachment {
        id: meta.record.id,
        name: meta.record.name,
        mime: meta.record.mime,
        size: meta.record.size,
        sha256: meta.record.sha256,
        kind: meta.record.kind,
        text,
        local_image_path,
        local_file_path,
        truncated,
    })
}

fn upload_dir(root: &Path, id: &str) -> Result<PathBuf> {
    if Uuid::parse_str(id).is_err() {
        bail!("invalid upload id");
    }
    Ok(root.join(id))
}

fn sanitize_file_name(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("attachment");
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
    let trimmed = cleaned_stem.trim_matches([' ', '_']).trim();
    let stem = if trimmed.is_empty() {
        "attachment".to_string()
    } else {
        trimmed.chars().take(160).collect()
    };
    if extension.is_empty() {
        stem
    } else {
        format!("{stem}.{extension}")
    }
}

fn content_file_name(name: &str) -> String {
    let ext = extension(name);
    if ext.is_empty() {
        "payload.bin".to_string()
    } else {
        format!("payload.{ext}")
    }
}

fn extension(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn is_text_extension(ext: &str) -> bool {
    matches!(
        ext,
        "txt"
            | "json"
            | "jsonl"
            | "yaml"
            | "yml"
            | "toml"
            | "log"
            | "rs"
            | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "py"
            | "go"
            | "java"
            | "kt"
            | "swift"
            | "c"
            | "cc"
            | "cpp"
            | "h"
            | "hpp"
            | "css"
            | "scss"
            | "html"
            | "xml"
            | "sql"
            | "sh"
            | "zsh"
            | "bash"
            | "env"
    )
}

fn decode_text(bytes: &[u8]) -> Result<(String, bool)> {
    if bytes.iter().filter(|byte| **byte == 0).count() > bytes.len().max(1) / 100 {
        bail!("文件看起来是二进制内容，无法作为文本附件读取");
    }
    let raw = std::str::from_utf8(bytes).context("附件不是有效 UTF-8 文本")?;
    Ok(truncate_chars(raw))
}

fn truncate_chars(text: &str) -> (String, bool) {
    let mut iter = text.chars();
    let truncated: String = iter.by_ref().take(MAX_TEXT_CHARS).collect();
    let is_truncated = iter.next().is_some();
    (truncated, is_truncated)
}

fn extract_spreadsheet_text(path: &Path, name: &str, bytes: &[u8]) -> Result<(String, bool)> {
    match extension(name).as_str() {
        "csv" => extract_delimited_text(bytes, b','),
        "tsv" => extract_delimited_text(bytes, b'\t'),
        "xlsx" | "xls" => extract_workbook_text(path),
        _ => bail!("不支持的表格格式"),
    }
}

fn extract_delimited_text(bytes: &[u8], delimiter: u8) -> Result<(String, bool)> {
    let mut reader = ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(bytes);
    let mut rows = Vec::new();
    let mut truncated = false;
    for (index, record) in reader.records().enumerate() {
        let record = record?;
        if index >= MAX_TABLE_ROWS {
            truncated = true;
            break;
        }
        if record.len() > MAX_TABLE_COLS {
            truncated = true;
        }
        rows.push(
            record
                .iter()
                .take(MAX_TABLE_COLS)
                .map(str::to_string)
                .collect::<Vec<_>>(),
        );
    }
    Ok((markdown_table("Sheet1", rows), truncated))
}

fn extract_workbook_text(path: &Path) -> Result<(String, bool)> {
    let mut workbook =
        open_workbook_auto(path).with_context(|| format!("open workbook {}", path.display()))?;
    let mut out = String::new();
    let mut truncated = false;
    let sheets = workbook.sheet_names().to_owned();
    for sheet in sheets.iter().take(MAX_SHEETS) {
        if let Ok(range) = workbook.worksheet_range(sheet) {
            let mut rows = Vec::new();
            for row in range.rows().take(MAX_TABLE_ROWS) {
                rows.push(
                    row.iter()
                        .take(MAX_TABLE_COLS)
                        .map(|cell| cell.to_string())
                        .collect::<Vec<_>>(),
                );
            }
            out.push_str(&markdown_table(sheet, rows));
            out.push('\n');
            if range.height() > MAX_TABLE_ROWS || range.width() > MAX_TABLE_COLS {
                truncated = true;
            }
        }
    }
    if sheets.len() > MAX_SHEETS {
        truncated = true;
    }
    if out.trim().is_empty() {
        bail!("表格没有可读取内容");
    }
    Ok((out, truncated))
}

fn markdown_table(sheet: &str, rows: Vec<Vec<String>>) -> String {
    if rows.is_empty() {
        return format!("#### Sheet: {}\n\n（空表格）\n", sheet);
    }
    let width = rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let mut normalized = rows;
    for row in &mut normalized {
        row.resize(width, String::new());
    }
    let mut out = format!("#### Sheet: {}\n\n", sheet);
    out.push('|');
    for cell in &normalized[0] {
        out.push(' ');
        out.push_str(&escape_markdown_cell(cell));
        out.push_str(" |");
    }
    out.push('\n');
    out.push('|');
    for _ in 0..width {
        out.push_str(" --- |");
    }
    out.push('\n');
    for row in normalized.iter().skip(1) {
        out.push('|');
        for cell in row {
            out.push(' ');
            out.push_str(&escape_markdown_cell(cell));
            out.push_str(" |");
        }
        out.push('\n');
    }
    out
}

fn escape_markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn extract_docx_text(bytes: &[u8]) -> Result<(String, bool)> {
    let reader = Cursor::new(bytes);
    let mut archive = ZipArchive::new(reader).context("读取 docx zip 结构失败")?;
    let mut document = String::new();
    archive
        .by_name("word/document.xml")
        .context("docx 缺少 word/document.xml")?
        .read_to_string(&mut document)
        .context("读取 docx document.xml")?;
    let mut xml = XmlReader::from_str(&document);
    xml.config_mut().trim_text(true);
    let mut text = String::new();
    let mut in_text = false;
    loop {
        match xml.read_event() {
            Ok(Event::Start(event)) if event.name().as_ref() == b"w:t" => in_text = true,
            Ok(Event::End(event)) if event.name().as_ref() == b"w:t" => in_text = false,
            Ok(Event::End(event)) if event.name().as_ref() == b"w:p" => text.push('\n'),
            Ok(Event::End(event)) if event.name().as_ref() == b"w:tc" => text.push('\t'),
            Ok(Event::Text(event)) if in_text => {
                text.push_str(&event.unescape().unwrap_or_default());
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(anyhow!("解析 docx XML 失败: {err}")),
            _ => {}
        }
    }
    let text = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() {
        bail!("docx 没有可读取文本");
    }
    Ok(truncate_chars(&text))
}

fn extract_pdf_text(path: &Path) -> Result<(String, bool)> {
    let document = Document::load(path).with_context(|| format!("读取 PDF {}", path.display()))?;
    let pages = document.get_pages();
    let mut text = String::new();
    let mut truncated = false;
    for (index, page_number) in pages.keys().enumerate() {
        if index >= MAX_PDF_PAGES {
            truncated = true;
            break;
        }
        if let Ok(page_text) = document.extract_text(&[*page_number]) {
            if !page_text.trim().is_empty() {
                text.push_str(&format!(
                    "\n\n#### Page {}\n{}",
                    page_number,
                    page_text.trim()
                ));
            }
        }
    }
    if text.trim().is_empty() {
        bail!("PDF 没有可复制文本，暂不支持扫描版/OCR");
    }
    let (text, text_truncated) = truncate_chars(text.trim());
    Ok((text, truncated || text_truncated))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_root_uses_nexushub_runtime_dir() {
        assert_eq!(
            upload_root(Path::new("/tmp/codex-home")),
            PathBuf::from("/tmp/codex-home/nexushub/uploads")
        );
    }

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!("nexushub-upload-test-{}", Uuid::new_v4()))
    }

    #[test]
    fn markdown_upload_preserves_markdown_context() {
        let root = temp_root();
        let record = store_upload(
            &root,
            "plan.md",
            Some("text/markdown"),
            b"# Plan\n\n- ship it",
        )
        .unwrap();
        assert_eq!(record.kind, UploadKind::Markdown);

        let prepared = prepare_uploads(&root, std::slice::from_ref(&record.id)).unwrap();

        assert_eq!(prepared[0].text.as_deref(), Some("# Plan\n\n- ship it"));
        assert!(
            prompt_with_attachment_context("", &prepared).contains("请根据以下附件内容继续处理。")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn csv_upload_becomes_markdown_table() {
        let root = temp_root();
        let record = store_upload(
            &root,
            "data.csv",
            Some("text/csv"),
            b"name,count\nalpha,2\nbeta,3",
        )
        .unwrap();
        assert_eq!(record.kind, UploadKind::Spreadsheet);

        let prepared = prepare_uploads(&root, std::slice::from_ref(&record.id)).unwrap();
        let text = prepared[0].text.as_deref().unwrap();

        assert!(text.contains("| name | count |"));
        assert!(text.contains("| alpha | 2 |"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn non_ascii_file_names_keep_extension_and_kind() {
        let root = temp_root();
        let image = store_upload(&root, "截图.png", Some("image/png"), b"fake-png").unwrap();
        let table =
            store_upload(&root, "数据.csv", Some("text/csv"), b"name,count\nalpha,2").unwrap();

        assert_eq!(image.name, "截图.png");
        assert_eq!(image.kind, UploadKind::Image);
        assert_eq!(table.name, "数据.csv");
        assert_eq!(table.kind, UploadKind::Spreadsheet);

        let prepared = prepare_uploads(&root, &[image.id.clone(), table.id.clone()]).unwrap();
        assert!(prepared[0]
            .local_image_path
            .as_ref()
            .unwrap()
            .ends_with("payload.png"));
        assert!(prepared[1]
            .text
            .as_deref()
            .unwrap()
            .contains("| alpha | 2 |"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn csv_upload_marks_row_or_column_truncation() {
        let root = temp_root();
        let mut many_rows = String::new();
        for index in 0..=MAX_TABLE_ROWS {
            many_rows.push_str(&format!("{index},value\n"));
        }
        let rows = store_upload(&root, "rows.csv", Some("text/csv"), many_rows.as_bytes()).unwrap();
        let many_cols = (0..=MAX_TABLE_COLS)
            .map(|index| format!("col{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let cols = store_upload(&root, "cols.csv", Some("text/csv"), many_cols.as_bytes()).unwrap();

        let prepared = prepare_uploads(&root, &[rows.id.clone(), cols.id.clone()]).unwrap();

        assert!(prepared[0].truncated);
        assert!(prepared[1].truncated);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn image_upload_prepares_local_image_path() {
        let root = temp_root();
        let record = store_upload(&root, "screen.png", Some("image/png"), b"fake-png").unwrap();
        assert_eq!(record.kind, UploadKind::Image);

        let prepared = prepare_uploads(&root, std::slice::from_ref(&record.id)).unwrap();

        assert!(prepared[0]
            .local_image_path
            .as_ref()
            .unwrap()
            .ends_with("payload.png"));
        assert!(prepared[0].text.is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stale_cleanup_preserves_protected_upload_ids() {
        let root = temp_root();
        let keep = store_upload(&root, "keep.md", Some("text/markdown"), b"# keep").unwrap();
        let remove = store_upload(&root, "remove.md", Some("text/markdown"), b"# remove").unwrap();
        let protected = HashSet::from([keep.id.clone()]);

        let removed = cleanup_stale_uploads_except(&root, Duration::ZERO, &protected).unwrap();

        assert_eq!(removed, 1);
        assert!(root.join(&keep.id).exists());
        assert!(!root.join(&remove.id).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn generic_binary_upload_prepares_local_file_reference_without_decoding() {
        let root = temp_root();

        let binary = store_upload(&root, "archive.zip", Some("application/zip"), b"\0zip").unwrap();
        let octet =
            store_upload(&root, "payload", Some("application/octet-stream"), b"\0raw").unwrap();
        let doc = store_upload(&root, "old.doc", Some("application/msword"), b"doc").unwrap();

        assert_eq!(binary.kind, UploadKind::File);
        assert_eq!(octet.kind, UploadKind::File);
        assert_eq!(doc.kind, UploadKind::File);

        let prepared = prepare_uploads(&root, std::slice::from_ref(&binary.id)).unwrap();
        assert_eq!(prepared[0].kind, UploadKind::File);
        assert!(prepared[0].text.is_none());
        assert!(prepared[0].local_image_path.is_none());
        assert!(prepared[0]
            .local_file_path
            .as_ref()
            .unwrap()
            .ends_with("payload.zip"));

        let context = attachment_context(&prepared);
        assert!(context.contains("### 附件: archive.zip"));
        assert!(context.contains("- 类型: file"));
        assert!(context.contains("- 本地文件路径:"));
        assert!(context.contains(&binary.sha256));
        let _ = fs::remove_dir_all(root);
    }
}
