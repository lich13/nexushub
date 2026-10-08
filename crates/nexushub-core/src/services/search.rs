use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    claude::{self, ClaudeDetailRequest},
    codex, grok,
    platform::PlatformPaths,
    services::sessions::SessionProvider,
};

const MAX_QUERY_BYTES: usize = 512;
const MAX_RESULTS: usize = 200;
const MAX_SESSIONS: usize = 200;
const MAX_CLAUDE_PAGES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchScope {
    Thread,
    Provider,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionSearchRequest {
    pub provider: SessionProvider,
    pub scope: SearchScope,
    pub session_key: Option<String>,
    pub query: String,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchResult {
    pub result_id: String,
    pub session_key: String,
    pub native_id: String,
    pub title: String,
    pub cwd: Option<String>,
    pub match_kind: String,
    pub position_key: String,
    pub snippet: String,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSearchResponse {
    pub provider: SessionProvider,
    pub scope: SearchScope,
    pub query: String,
    pub results: Vec<SessionSearchResult>,
    pub next_cursor: Option<String>,
    pub truncated: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SearchUseCases {
    codex: codex::CodexPaths,
    grok: grok::GrokPaths,
    claude: claude::ClaudePaths,
}

impl SearchUseCases {
    pub fn new(_platform: &PlatformPaths, codex: codex::CodexPaths) -> Self {
        Self {
            codex,
            grok: grok::GrokPaths::default_for_user(),
            claude: claude::ClaudePaths::default_for_user(),
        }
    }

    pub fn search(&self, request: SessionSearchRequest) -> Result<SessionSearchResponse> {
        validate_request(&request)?;
        let needle = request.query.trim().to_lowercase();
        let mut matches = Vec::new();
        let mut warnings = Vec::new();
        match request.provider {
            SessionProvider::Codex => {
                self.search_codex(&request, &needle, &mut matches, &mut warnings)?
            }
            SessionProvider::Claude => {
                self.search_claude(&request, &needle, &mut matches, &mut warnings)?
            }
            SessionProvider::Grok => {
                self.search_grok(&request, &needle, &mut matches, &mut warnings)?
            }
        }
        let offset = request
            .cursor
            .as_deref()
            .unwrap_or("0")
            .parse::<usize>()
            .context("无效的搜索游标")?;
        ensure!(offset <= matches.len(), "搜索游标已失效，请重新搜索");
        let limit = request.limit.unwrap_or(50).clamp(1, MAX_RESULTS);
        let end = (offset + limit).min(matches.len());
        let next_cursor = (end < matches.len()).then(|| end.to_string());
        Ok(SessionSearchResponse {
            provider: request.provider,
            scope: request.scope,
            query: request.query.trim().to_string(),
            results: matches[offset..end].to_vec(),
            next_cursor,
            truncated: matches.len() >= MAX_RESULTS,
            warnings,
        })
    }

    fn search_codex(
        &self,
        request: &SessionSearchRequest,
        needle: &str,
        output: &mut Vec<SessionSearchResult>,
        warnings: &mut Vec<String>,
    ) -> Result<()> {
        let summaries = codex::list_threads(
            &self.codex,
            None,
            request.session_key.as_deref(),
            MAX_SESSIONS,
        )?;
        for summary in summaries {
            if request.scope == SearchScope::Thread
                && request.session_key.as_deref() != Some(summary.id.as_str())
            {
                continue;
            }
            let detail = match codex::thread_detail_from_summary(summary.clone()) {
                Ok(detail) => detail,
                Err(_) => {
                    push_warning(warnings);
                    continue;
                }
            };
            let metadata = strip_memory_metadata(&format!(
                "{} {} {}",
                summary.title,
                summary.id,
                summary.cwd.clone().unwrap_or_default()
            ));
            if metadata.to_lowercase().contains(needle) {
                push_metadata(
                    output,
                    &summary.id,
                    &summary.id,
                    &summary.title,
                    summary.cwd.clone(),
                    &metadata,
                    needle,
                    summary.updated_at.clone(),
                );
            }
            for block in detail.blocks {
                let corpus = json_corpus(&block)?;
                if let Some(snippet) = find_snippet(&corpus, needle) {
                    let position = block.id.clone();
                    push_result(
                        output,
                        &summary.id,
                        &summary.id,
                        &summary.title,
                        summary.cwd.clone(),
                        &block.kind,
                        position,
                        snippet,
                        block.created_at.clone(),
                    );
                }
            }
        }
        Ok(())
    }

    fn search_grok(
        &self,
        request: &SessionSearchRequest,
        needle: &str,
        output: &mut Vec<SessionSearchResult>,
        warnings: &mut Vec<String>,
    ) -> Result<()> {
        let summaries = grok::list_grok_sessions(&self.grok, MAX_SESSIONS, None)?;
        for summary in summaries {
            if request.scope == SearchScope::Thread
                && request.session_key.as_deref() != Some(summary.id.as_str())
            {
                continue;
            }
            let metadata =
                strip_memory_metadata(&format!("{} {} {}", summary.title, summary.id, summary.cwd));
            if metadata.to_lowercase().contains(needle) {
                push_metadata(
                    output,
                    &summary.id,
                    &summary.id,
                    &summary.title,
                    Some(summary.cwd.clone()),
                    &metadata,
                    needle,
                    summary.updated_at.clone(),
                );
            }
            let detail = match grok::grok_session_detail(&self.grok, &summary.id, None) {
                Ok(detail) => detail,
                Err(_) => {
                    push_warning(warnings);
                    continue;
                }
            };
            for (index, event) in detail.events.into_iter().enumerate() {
                let corpus = json_corpus(&event)?;
                if let Some(snippet) = find_snippet(&corpus, needle) {
                    let position = event
                        .call_id
                        .clone()
                        .unwrap_or_else(|| format!("event:{index}"));
                    push_result(
                        output,
                        &summary.id,
                        &summary.id,
                        &summary.title,
                        Some(summary.cwd.clone()),
                        &event.kind,
                        position,
                        snippet,
                        event.timestamp.clone(),
                    );
                }
            }
        }
        Ok(())
    }

    fn search_claude(
        &self,
        request: &SessionSearchRequest,
        needle: &str,
        output: &mut Vec<SessionSearchResult>,
        warnings: &mut Vec<String>,
    ) -> Result<()> {
        let summaries = claude::list_claude_sessions(&self.claude, MAX_SESSIONS, None)?;
        for summary in summaries {
            if request.scope == SearchScope::Thread
                && request.session_key.as_deref() != Some(summary.session_key.as_str())
            {
                continue;
            }
            let metadata =
                strip_memory_metadata(&format!("{} {} {}", summary.title, summary.id, summary.cwd));
            if metadata.to_lowercase().contains(needle) {
                push_metadata(
                    output,
                    &summary.session_key,
                    &summary.id,
                    &summary.title,
                    Some(summary.cwd.clone()),
                    &metadata,
                    needle,
                    summary.updated_at.clone(),
                );
            }
            let mut before = None;
            for _ in 0..MAX_CLAUDE_PAGES {
                let page = match claude::claude_session_detail(
                    &self.claude,
                    &ClaudeDetailRequest {
                        session_key: summary.session_key.clone(),
                        limit: Some(500),
                        before: before.clone(),
                    },
                ) {
                    Ok(page) => page,
                    Err(_) => {
                        push_warning(warnings);
                        break;
                    }
                };
                for event in &page.events {
                    let corpus = json_corpus(event)?;
                    if let Some(snippet) = find_snippet(&corpus, needle) {
                        push_result(
                            output,
                            &summary.session_key,
                            &summary.id,
                            &summary.title,
                            Some(summary.cwd.clone()),
                            &event.kind,
                            event.id.clone(),
                            snippet,
                            event.timestamp.clone(),
                        );
                    }
                }
                if !page.has_more {
                    break;
                }
                before = page.before_cursor;
                if before.is_none() {
                    break;
                }
            }
        }
        Ok(())
    }
}

fn validate_request(request: &SessionSearchRequest) -> Result<()> {
    ensure!(!request.query.trim().is_empty(), "搜索内容不能为空");
    ensure!(request.query.len() <= MAX_QUERY_BYTES, "搜索内容过长");
    if request.scope == SearchScope::Thread {
        let key = request
            .session_key
            .as_deref()
            .context("当前线程搜索需要线程定位键")?;
        ensure!(
            !key.is_empty() && key.len() <= 512 && !key.chars().any(char::is_control),
            "无效的线程定位键"
        );
    }
    if let Some(key) = request.session_key.as_deref() {
        ensure!(
            key.len() <= 512 && !key.chars().any(char::is_control),
            "无效的线程定位键"
        );
    }
    Ok(())
}

fn json_corpus<T: Serialize>(value: &T) -> Result<String> {
    let value = serde_json::to_value(value)?;
    let mut strings = Vec::new();
    collect_strings(&value, &mut strings);
    Ok(strip_memory_metadata(&strings.join("\n")))
}

fn collect_strings(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::String(text) if !text.trim().is_empty() => output.push(text.clone()),
        Value::Array(values) => values
            .iter()
            .for_each(|value| collect_strings(value, output)),
        Value::Object(values) => values.iter().for_each(|(key, value)| {
            if !matches!(
                key.to_ascii_lowercase().as_str(),
                "memory_citation" | "citation_entries" | "rollout_ids" | "oai-mem-citation"
            ) {
                collect_strings(value, output);
            }
        }),
        _ => {}
    }
}

fn push_warning(warnings: &mut Vec<String>) {
    if warnings.len() < 8
        && !warnings
            .iter()
            .any(|warning| warning == "部分记录无法读取，已跳过")
    {
        warnings.push("部分记录无法读取，已跳过".to_string());
    }
}

fn strip_memory_metadata(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut fence: Option<(u8, usize)> = None;
    let mut literal_line = false;
    let mut metadata_depth: Vec<&str> = Vec::new();
    let lower = input.to_ascii_lowercase();
    let mut cursor = 0;
    while cursor < input.len() {
        let at_line_start =
            cursor == 0 || input.as_bytes().get(cursor.saturating_sub(1)) == Some(&b'\n');
        if metadata_depth.is_empty() && at_line_start {
            let line_end = input[cursor..]
                .find('\n')
                .map(|offset| cursor + offset + 1)
                .unwrap_or(input.len());
            let trimmed = input[cursor..line_end].trim_start();
            let line = &input[cursor..line_end];
            literal_line =
                trimmed.starts_with('>') || line.starts_with("    ") || line.starts_with('\t');
            let marker = trimmed.as_bytes().first().copied();
            let marker_len = trimmed
                .bytes()
                .take_while(|byte| Some(*byte) == marker)
                .count();
            let is_fence = !literal_line && matches!(marker, Some(b'`' | b'~')) && marker_len >= 3;
            if fence.is_some() || is_fence {
                result.push_str(&input[cursor..line_end]);
                if let Some((active, length)) = fence {
                    if marker == Some(active)
                        && marker_len >= length
                        && trimmed[marker_len..].trim().is_empty()
                    {
                        fence = None;
                    }
                } else {
                    fence = marker.map(|marker| (marker, marker_len));
                }
                cursor = line_end;
                continue;
            }
            let field = trimmed.to_ascii_lowercase();
            if field.starts_with("memory_citation")
                || field.starts_with("\"memory_citation\"")
                || field.starts_with("'memory_citation'")
            {
                if let Some(newline) = input[cursor..line_end].find('\n') {
                    result.push('\n');
                    cursor += newline + 1;
                } else {
                    cursor = line_end;
                }
                continue;
            }
        }
        if !metadata_depth.is_empty() {
            let token = &lower[cursor..];
            if let Some((true, name, end)) = match_metadata_tag(token) {
                if let Some(index) = metadata_depth.iter().rposition(|item| *item == name) {
                    metadata_depth.truncate(index);
                }
                cursor += end;
                continue;
            }
            if let Some((false, name, end)) = match_metadata_tag(token) {
                metadata_depth.push(name);
                cursor += end;
                continue;
            }
            cursor += input[cursor..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
            continue;
        }
        if input[cursor..].starts_with('`') {
            let run = input[cursor..].chars().take_while(|ch| *ch == '`').count();
            let delimiter = "`".repeat(run);
            let content_start = cursor + delimiter.len();
            let end = input[content_start..]
                .find(&delimiter)
                .map(|offset| content_start + offset + delimiter.len())
                .unwrap_or(input.len());
            result.push_str(&input[cursor..end]);
            cursor = end;
            continue;
        }
        if !literal_line {
            if let Some(length) = page_metadata_length(&input[cursor..]) {
                cursor += length;
                continue;
            }
        }
        if let Some((closing, name, end)) = match_metadata_tag(&lower[cursor..]) {
            if !closing {
                metadata_depth.push(name);
                cursor += end;
                continue;
            }
        }
        result.push_str(
            &input[cursor
                ..cursor
                    + input[cursor..]
                        .chars()
                        .next()
                        .map(char::len_utf8)
                        .unwrap_or(1)],
        );
        cursor += input[cursor..]
            .chars()
            .next()
            .map(char::len_utf8)
            .unwrap_or(1);
    }
    result
}

// Match only the complete native transport envelope. Examples and incomplete
// JSON stay readable; this never changes the source transcript.
fn page_metadata_length(input: &str) -> Option<usize> {
    const OPEN: &str = "<external_codex_apps_open_page>";
    const CLOSE: &str = "</external_codex_apps_open_page>";
    let body = input.strip_prefix(OPEN)?;
    let end = body.find(CLOSE)?;
    let value: Value = serde_json::from_str(&body[..end]).ok()?;
    let object = value.as_object()?;
    let page_id = object.get("page_id")?;
    (object.len() == 1 && (page_id.is_null() || page_id.is_string()))
        .then_some(OPEN.len() + end + CLOSE.len())
}

fn match_metadata_tag(input: &str) -> Option<(bool, &'static str, usize)> {
    let mut cursor = if input.starts_with("&lt;") {
        4
    } else if input.starts_with('<') {
        1
    } else {
        return None;
    };
    let closing = input[cursor..].starts_with('/');
    if closing {
        cursor += 1;
    }
    let names: [(&str, &str); 3] = [
        ("oai-mem-citation", "oai-mem-citation"),
        ("citation_entries", "citation_entries"),
        ("rollout_ids", "rollout_ids"),
    ];
    let (_, name) = names
        .iter()
        .find(|(prefix, _)| input[cursor..].starts_with(prefix))?;
    cursor += name.len();
    while cursor < input.len() {
        if input[cursor..].starts_with('>') {
            return Some((closing, name, cursor + 1));
        }
        if input[cursor..].starts_with("&gt;") {
            return Some((closing, name, cursor + 4));
        }
        cursor += input[cursor..].chars().next()?.len_utf8();
    }
    None
}

fn find_snippet(text: &str, needle: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let folded_needle = needle.to_lowercase();
    let index = lower.find(&folded_needle)?;
    let folded_start = lower[..index].chars().count();
    let folded_end = folded_start + folded_needle.chars().count();
    let mut folded_to_source = Vec::new();
    for (source_index, character) in text.char_indices() {
        let folded = character.to_lowercase().collect::<String>();
        folded_to_source.extend(std::iter::repeat_n(source_index, folded.chars().count()));
    }
    let start_index = folded_start.saturating_sub(90);
    let end_index = folded_end.saturating_add(160).min(folded_to_source.len());
    let start = folded_to_source.get(start_index).copied().unwrap_or(0);
    let end = folded_to_source
        .get(end_index)
        .copied()
        .unwrap_or(text.len());
    Some(
        text.get(start..end)
            .unwrap_or(text)
            .replace('\n', " ")
            .trim()
            .to_string(),
    )
}

#[allow(clippy::too_many_arguments)]
fn push_metadata(
    output: &mut Vec<SessionSearchResult>,
    key: &str,
    native_id: &str,
    title: &str,
    cwd: Option<String>,
    metadata: &str,
    needle: &str,
    timestamp: Option<String>,
) {
    let snippet = find_snippet(metadata, needle).unwrap_or_else(|| metadata.to_string());
    push_result(
        output,
        key,
        native_id,
        title,
        cwd,
        "metadata",
        "session".to_string(),
        snippet,
        timestamp,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_result(
    output: &mut Vec<SessionSearchResult>,
    key: &str,
    native_id: &str,
    title: &str,
    cwd: Option<String>,
    kind: &str,
    position: String,
    snippet: String,
    timestamp: Option<String>,
) {
    if output.len() >= MAX_RESULTS {
        return;
    }
    let result_id = format!("{key}:{position}:{kind}");
    if output.iter().any(|item| item.result_id == result_id) {
        return;
    }
    output.push(SessionSearchResult {
        result_id,
        session_key: key.to_string(),
        native_id: native_id.to_string(),
        title: title.to_string(),
        cwd,
        match_kind: kind.to_string(),
        position_key: position,
        snippet,
        timestamp,
    });
}

#[cfg(test)]
mod tests {
    use super::strip_memory_metadata;

    #[test]
    fn removes_memory_blocks_but_keeps_code_examples() {
        let text = "before\n<oai-mem-citation>secret</oai-mem-citation>\nafter\n```\n<oai-mem-citation>example</oai-mem-citation>\n```";
        let output = strip_memory_metadata(text);
        assert!(output.contains("before"));
        assert!(output.contains("after"));
        assert!(output.contains("example"));
        assert!(!output.contains("secret"));
    }

    #[test]
    fn removes_adjacent_and_multiline_metadata_without_dropping_visible_text() {
        let text = "before <citation_entries>secret</citation_entries><rollout_ids>id</rollout_ids> after\n<oai-mem-citation>\ninternal\n</oai-mem-citation>\n`<rollout_ids>literal</rollout_ids>`\nmemory_citation: {\"private\":true}\nafter";
        let output = strip_memory_metadata(text);
        assert!(output.contains("before  after"));
        assert!(output.contains("`<rollout_ids>literal</rollout_ids>`"));
        assert!(output.contains("after"));
        assert!(!output.contains("secret"));
        assert!(!output.contains("internal"));
        assert!(!output.contains("private"));
    }

    #[test]
    fn excludes_native_page_envelopes_from_search_without_losing_adjacent_text() {
        let empty =
            "<external_codex_apps_open_page>{\"page_id\":null}</external_codex_apps_open_page>";
        let named = "<external_codex_apps_open_page>\n{\"page_id\":\"fixture-page\"}\n</external_codex_apps_open_page>";
        let output = strip_memory_metadata(&format!("Before {empty}{named} after."));
        assert_eq!(output, "Before  after.");
        assert_eq!(strip_memory_metadata(empty), "");
    }

    #[test]
    fn page_envelope_examples_and_unconfirmed_content_remain_searchable() {
        let envelope =
            "<external_codex_apps_open_page>{\"page_id\":null}</external_codex_apps_open_page>";
        for source in [
            format!("`{envelope}`"),
            format!("```xml\n{envelope}\n```"),
            format!("````xml\n```\n{envelope}\n````"),
            format!("~~~xml\n{envelope}\n~~~"),
            format!("> {envelope}"),
            format!("    {envelope}"),
            "<external_codex_apps_open_page>invalid</external_codex_apps_open_page>".into(),
            "<external_codex_apps_open_page>{\"page_id\":null}".into(),
            "<external_codex_apps_open_page>{\"request\":\"keep\"}</external_codex_apps_open_page>"
                .into(),
        ] {
            assert_eq!(strip_memory_metadata(&source), source);
        }
    }

    #[test]
    fn finds_unicode_snippets_on_character_boundaries() {
        let text = "前置🙂 中文搜索命中后置";
        let snippet = super::find_snippet(text, "中文搜索").expect("unicode match");
        assert!(snippet.contains("中文搜索"));
        assert!(std::str::from_utf8(snippet.as_bytes()).is_ok());
    }
}
