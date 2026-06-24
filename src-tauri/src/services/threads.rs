use anyhow::Result;
use nexushub_core::{
    codex::{archived_thread_ids, hidden_thread_ids, list_threads, ThreadSummary},
    services::threads::{self as thread_service, ThreadsQuery},
};

use crate::overview::DesktopState;

pub(crate) fn home_thread_summaries(state: &DesktopState) -> Result<Vec<ThreadSummary>> {
    thread_summaries_with_query(
        state,
        ThreadsQuery {
            status: None,
            q: None,
            limit: Some(40),
        },
    )
}

pub(crate) fn thread_summaries_with_query(
    state: &DesktopState,
    query: ThreadsQuery,
) -> Result<Vec<ThreadSummary>> {
    let paths = state.codex_paths();
    let plan = thread_service::plan_threads_list_request(state.platform(), query)?;
    let hidden_thread_ids = hidden_thread_ids(&paths).unwrap_or_default();
    let archived_thread_ids = archived_thread_ids(&paths).unwrap_or_default();
    Ok(thread_service::build_threads_overview(
        list_threads(&paths, None, plan.query.q.as_deref(), plan.fetch_limit)?,
        state.db.running_thread_jobs()?,
        plan.query,
        &hidden_thread_ids,
        &archived_thread_ids,
    )
    .threads)
}
