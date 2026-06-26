use anyhow::Result;
use std::collections::HashSet;

mod mutations;
mod paths;
mod rollout_events;
mod session_index;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
mod thread_rows;
mod types;

pub use mutations::{db_integrity, set_thread_archived, set_thread_title};
pub use paths::{
    resolve_codex_paths, resolve_codex_paths_with_options, CodexPathDiscoveryOptions, CodexPaths,
    ResolvedCodexPaths,
};
use rollout_events::{
    first_user_message_title, is_usable_thread_title, scan_rollout,
    should_repair_thread_title_from_local_metadata,
};
pub(crate) use rollout_events::{
    hidden_thread_metadata_category, rollout_has_running_signal, ThreadVisibilityMetadata,
};
pub use rollout_events::{
    is_macos_network_volume_path, message_blocks_from_events,
    rollout_completion_last_agent_message, rollout_completion_last_agent_message_with_source,
    rollout_has_completed_turn, rollout_hook_stop_message, rollout_hook_stop_message_with_source,
    rollout_latest_assistant_message, thread_detail_from_summary, window_thread_detail,
};
#[cfg(test)]
use rollout_events::{is_request_user_input, parse_message_event, RolloutScan};
use session_index::{read_session_index, SessionIndexEntry};
use thread_rows::read_thread_rows;
pub use thread_rows::{archived_thread_ids, hidden_thread_ids, thread_source_counts};
pub use types::{
    extract_proposed_plan_text, CodexMessage, MessageBlock, PendingElicitation, ThreadDetail,
    ThreadStatus, ThreadSummary, UserInputAnswer, UserInputOption, UserInputQuestion,
};

pub fn list_threads(
    paths: &CodexPaths,
    status: Option<&str>,
    q: Option<&str>,
    limit: usize,
) -> Result<Vec<ThreadSummary>> {
    let mut rows = read_thread_rows(paths)?;
    let session_index = read_session_index(paths).unwrap_or_default();
    let mut hidden_subagents = HashSet::new();
    for row in &mut rows {
        let index_entry = session_index.get(&row.summary.id);
        if row.summary.rollout_path.is_none() {
            row.summary.rollout_path = index_entry.and_then(|entry| entry.path.clone());
        }
        row.summary.rollout_path = row
            .summary
            .rollout_path
            .take()
            .filter(|path| paths.contains_path(path));
        if should_repair_thread_title_from_local_metadata(
            row.db_title.as_deref(),
            row.first_user_message.as_deref(),
            index_entry,
        ) {
            row.summary.title = index_entry
                .and_then(SessionIndexEntry::title_candidate)
                .or_else(|| {
                    row.first_user_message
                        .as_deref()
                        .and_then(first_user_message_title)
                })
                .unwrap_or_else(|| "未命名线程".to_string());
        }
        if enrich_thread_from_rollout(&mut row.summary).unwrap_or(false) {
            hidden_subagents.insert(row.summary.id.clone());
        }
    }
    let mut rows = rows.into_iter().map(|row| row.summary).collect::<Vec<_>>();
    rows.retain(|row| !hidden_subagents.contains(&row.id));

    let needle = q
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty());
    rows.retain(|row| {
        if let Some(status) = status {
            if !matches_status(row, status) {
                return false;
            }
            if status == "all" && matches!(row.status, ThreadStatus::Archived) {
                return false;
            }
        } else if matches!(row.status, ThreadStatus::Archived) {
            return false;
        }
        if let Some(needle) = &needle {
            let title = row.title.to_ascii_lowercase();
            let id = row.id.to_ascii_lowercase();
            let latest = row
                .latest_message
                .clone()
                .unwrap_or_default()
                .to_ascii_lowercase();
            title.contains(needle) || id.contains(needle) || latest.contains(needle)
        } else {
            true
        }
    });
    rows.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    rows.truncate(limit.max(1));
    Ok(rows)
}

pub fn enrich_thread_from_rollout(row: &mut ThreadSummary) -> Result<bool> {
    let Some(path) = &row.rollout_path else {
        return Ok(false);
    };
    let scan = scan_rollout(path, 80)?;
    row.message_count = scan.message_count;
    row.latest_message = scan.latest_message;
    if !matches!(row.status, ThreadStatus::Archived) {
        if scan.recoverable {
            row.status = ThreadStatus::Recoverable;
        } else if scan.reply_needed {
            row.status = ThreadStatus::ReplyNeeded;
        } else if scan.running {
            row.status = ThreadStatus::Running;
        } else if matches!(
            row.status,
            ThreadStatus::Running | ThreadStatus::ReplyNeeded | ThreadStatus::Recoverable
        ) {
            row.status = ThreadStatus::Recent;
        }
    }
    row.cwd = scan.cwd;
    row.model = scan.model;
    if !is_usable_thread_title(&row.title) {
        row.title = scan
            .title
            .or(scan.first_user_message_title)
            .unwrap_or_else(|| "未命名线程".to_string());
    }
    row.active_turn_id = scan.active_turn_id;
    row.pending_elicitation = scan.pending_elicitation;
    row.last_event_kind = scan.last_event_kind;
    Ok(scan.is_subagent)
}

pub fn thread_detail(paths: &CodexPaths, id: &str) -> Result<Option<ThreadDetail>> {
    let Some(mut summary) = list_threads(paths, None, Some(id), 500)?
        .into_iter()
        .find(|thread| thread.id == id)
    else {
        return Ok(None);
    };
    let _ = enrich_thread_from_rollout(&mut summary);
    thread_detail_from_summary(summary).map(Some)
}

fn matches_status(row: &ThreadSummary, status: &str) -> bool {
    match status {
        "recent" => matches!(row.status, ThreadStatus::Recent),
        "running" => matches!(row.status, ThreadStatus::Running),
        "reply-needed" | "reply_needed" => matches!(row.status, ThreadStatus::ReplyNeeded),
        "recoverable" => matches!(row.status, ThreadStatus::Recoverable),
        "archived" => matches!(row.status, ThreadStatus::Archived),
        _ => true,
    }
}
