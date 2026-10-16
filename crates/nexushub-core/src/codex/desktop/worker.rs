//! Serialized owner of a live desktop session and NexusHub's outgoing queue.
//! Dropping UI callbacks never replays a command. Native execution remains in
//! the existing desktop process; this worker only follows and submits to it.
use super::{
    commands::{CollaborationMode, DesktopCommand, DesktopReply, MessageInput},
    projection::{self, bounded, state_request_key},
    references::NativeReference,
    DesktopError, DesktopSession, Result,
};
use crate::{
    db::desktop_submissions::{DesktopLedger, Submission},
    services::codex_desktop::{DesktopAction, DesktopMode, DesktopQueuedMessage, DesktopView},
};
use serde_json::json;
use tokio::sync::{mpsc, oneshot, watch};

struct ActionRequest {
    action: DesktopAction,
    references: Vec<NativeReference>,
    response: oneshot::Sender<Result<DesktopView>>,
}

#[derive(Clone)]
pub struct DesktopWorker {
    actions: mpsc::Sender<ActionRequest>,
    view: watch::Receiver<DesktopView>,
    project_directory: Option<std::path::PathBuf>,
}

impl DesktopWorker {
    #[cfg(test)]
    pub async fn spawn(
        session: DesktopSession,
        ledger: DesktopLedger,
        source: String,
    ) -> Result<Self> {
        Self::spawn_inner(session, ledger, source, None).await
    }

    pub async fn spawn_checked(
        session: DesktopSession,
        ledger: DesktopLedger,
        source: String,
        paths: crate::codex::CodexPaths,
    ) -> Result<Self> {
        Self::spawn_inner(session, ledger, source, Some(paths)).await
    }

    async fn spawn_inner(
        session: DesktopSession,
        ledger: DesktopLedger,
        source: String,
        paths: Option<crate::codex::CodexPaths>,
    ) -> Result<Self> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let project_directory = session.state().project_directory().ok();
        let thread_id = session.state().thread_id().to_owned();
        let list_source = source.clone();
        let list_thread = thread_id.clone();
        let pending = storage(&ledger, move |ledger| {
            ledger.list(&list_source, &list_thread)
        })
        .await?;
        let initial = projection::view(session.state(), &session_id, queue_view(&pending));
        let (view_tx, view) = watch::channel(initial);
        let (actions, action_rx) = mpsc::channel(16);
        tokio::spawn(
            WorkerLoop {
                session,
                ledger,
                source,
                thread_id,
                session_id,
                pending,
                paths,
                wait_for_turn: None,
                last_error: None,
                view: view_tx,
                actions: action_rx,
            }
            .run(),
        );
        Ok(Self {
            actions,
            view,
            project_directory,
        })
    }

    pub fn current(&self) -> DesktopView {
        self.view.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<DesktopView> {
        self.view.clone()
    }
    pub fn is_closed(&self) -> bool {
        self.actions.is_closed()
    }
    pub fn project_directory(&self) -> Option<&std::path::Path> {
        self.project_directory.as_deref()
    }

    pub async fn act(&self, action: DesktopAction) -> Result<DesktopView> {
        self.act_with_references(action, Vec::new()).await
    }

    pub async fn act_with_references(
        &self,
        action: DesktopAction,
        references: Vec<NativeReference>,
    ) -> Result<DesktopView> {
        validate_action(&action)?;
        let count = match &action {
            DesktopAction::Send { references, .. } | DesktopAction::Steer { references, .. } => {
                references.len()
            }
            _ => 0,
        };
        if count != references.len() {
            return Err(DesktopError::InvalidInput);
        }
        let (response, receive) = oneshot::channel();
        self.actions
            .try_send(ActionRequest {
                action,
                references,
                response,
            })
            .map_err(|_| DesktopError::Busy)?;
        receive.await.map_err(|_| DesktopError::Disconnected)?
    }
}

struct WorkerLoop {
    session: DesktopSession,
    ledger: DesktopLedger,
    source: String,
    thread_id: String,
    session_id: String,
    pending: Vec<Submission>,
    paths: Option<crate::codex::CodexPaths>,
    wait_for_turn: Option<String>,
    last_error: Option<DesktopError>,
    view: watch::Sender<DesktopView>,
    actions: mpsc::Receiver<ActionRequest>,
}

impl WorkerLoop {
    async fn run(mut self) {
        if let Err(error) = self.reconcile().await {
            self.publish(Some(error));
        }
        loop {
            let event = tokio::select! {
                event = self.session.next_change() => event,
                action = self.actions.recv() => {
                    let Some(action) = action else {break};
                    let result = self.action(action.action,action.references).await;
                    if result.is_ok() { self.last_error = None; }
                    self.publish(result.as_ref().err().copied());
                    let _ = action.response.send(result.map(|()|self.view.borrow().clone()));
                    Ok(())
                },
            };
            if let Err(error) = event {
                let source = self.source.clone();
                let thread = self.thread_id.clone();
                let _ = storage(&self.ledger, move |ledger| ledger.hold(&source, &thread)).await;
                let _ = self.refresh_queue().await;
                let mut view = self.make_view();
                view.connected = false;
                view.writable = false;
                view.error = Some(error.to_string());
                self.view.send_replace(view);
                break;
            }
            if let Err(error) = self.reconcile().await {
                self.publish(Some(error));
            }
            if let Err(error) = self.dispatch_queued().await {
                self.publish(Some(error));
            } else {
                self.publish(None);
            }
        }
        self.session.close().await;
    }

    fn make_view(&self) -> DesktopView {
        projection::view(
            self.session.state(),
            &self.session_id,
            queue_view(&self.pending),
        )
    }
    fn publish(&mut self, error: Option<DesktopError>) {
        if error.is_some() {
            self.last_error = error;
        }
        let mut view = self.make_view();
        view.error = self.last_error.map(|error| error.to_string());
        self.view.send_replace(view);
    }
    async fn refresh_queue(&mut self) -> Result<()> {
        let source = self.source.clone();
        let thread = self.thread_id.clone();
        self.pending = storage(&self.ledger, move |ledger| ledger.list(&source, &thread)).await?;
        Ok(())
    }

    async fn action(
        &mut self,
        action: DesktopAction,
        resolved: Vec<NativeReference>,
    ) -> Result<()> {
        self.session.state().check_writable()?;
        match action {
            DesktopAction::Send {
                message_id, text, ..
            } => {
                let source = self.source.clone();
                let thread = self.thread_id.clone();
                storage(&self.ledger, move |ledger| {
                    ledger.enqueue(
                        &source,
                        &thread,
                        &message_id,
                        &json!({"kind":"queue","text":text,"references":resolved}),
                    )
                })
                .await?;
                self.refresh_queue().await?;
            }
            DesktopAction::Steer {
                message_id,
                text,
                turn_id,
                ..
            } => {
                if self.session.state().active_turn_id()? != turn_id {
                    return Err(DesktopError::RequestExpired);
                }
                let source = self.source.clone();
                let thread = self.thread_id.clone();
                let id = message_id.clone();
                let body_text = text.clone();
                let body_references = resolved.clone();
                storage(&self.ledger, move |ledger| {
                    ledger.enqueue(
                        &source,
                        &thread,
                        &id,
                        &json!({"kind":"steer","text":body_text,"references":body_references}),
                    )
                })
                .await?;
                self.refresh_queue().await?;
                let source = self.source.clone();
                let thread = self.thread_id.clone();
                let id = message_id.clone();
                if !storage(&self.ledger, move |ledger| {
                    ledger.claim_immediate(&source, &thread, &id)
                })
                .await?
                {
                    let source = self.source.clone();
                    let thread = self.thread_id.clone();
                    let id = message_id.clone();
                    storage(&self.ledger, move |ledger| {
                        ledger.hold_queued(&source, &thread, &id)
                    })
                    .await?;
                    self.refresh_queue().await?;
                    return Err(DesktopError::Busy);
                }
                self.submit(
                    &message_id,
                    DesktopCommand::Steer {
                        message: MessageInput {
                            client_id: message_id.clone(),
                            text,
                            references: resolved,
                        },
                        expected_turn_id: turn_id,
                    },
                )
                .await?;
            }
            DesktopAction::Stop { turn_id } => {
                let source = self.source.clone();
                let thread = self.thread_id.clone();
                storage(&self.ledger, move |ledger| ledger.hold(&source, &thread)).await?;
                self.refresh_queue().await?;
                self.execute(&DesktopCommand::Stop {
                    expected_turn_id: turn_id,
                })
                .await?;
            }
            DesktopAction::CancelQueued { message_id } => {
                let source = self.source.clone();
                let thread = self.thread_id.clone();
                if !storage(&self.ledger, move |ledger| {
                    ledger.cancel(&source, &thread, &message_id)
                })
                .await?
                {
                    return Err(DesktopError::RequestExpired);
                }
                self.refresh_queue().await?;
            }
            DesktopAction::ResumeQueue => {
                let source = self.source.clone();
                let thread = self.thread_id.clone();
                let ids: Vec<_> = self
                    .pending
                    .iter()
                    .filter(|item| {
                        item.status == "held"
                            && item
                                .body
                                .as_ref()
                                .is_some_and(|body| body["kind"] == "queue")
                    })
                    .map(|item| item.id.clone())
                    .collect();
                storage(&self.ledger, move |ledger| {
                    ledger.resume_selected(&source, &thread, &ids)
                })
                .await?;
                self.refresh_queue().await?;
            }
            DesktopAction::DismissUnknown { message_id } => {
                let source = self.source.clone();
                let thread = self.thread_id.clone();
                if !storage(&self.ledger, move |ledger| {
                    ledger.dismiss_unknown(&source, &thread, &message_id)
                })
                .await?
                {
                    return Err(DesktopError::RequestExpired);
                }
                self.refresh_queue().await?;
            }
            DesktopAction::Settings {
                model,
                reasoning_effort,
                mode,
            } => {
                self.execute(&DesktopCommand::Settings {
                    model,
                    reasoning_effort,
                    mode: match mode {
                        DesktopMode::Default => CollaborationMode::Default,
                        DesktopMode::Plan => CollaborationMode::Plan,
                    },
                })
                .await?;
            }
            DesktopAction::Decide { request_key, allow } => {
                self.reply(&request_key, DesktopReply::Decision { allow })
                    .await?;
            }
            DesktopAction::Answer {
                request_key,
                answers,
            } => {
                self.reply(
                    &request_key,
                    DesktopReply::Answers {
                        answers: answers
                            .into_iter()
                            .map(|answer| (answer.question_id, answer.answers))
                            .collect(),
                    },
                )
                .await?;
            }
            DesktopAction::Elicitation {
                request_key,
                accept,
                content,
            } => {
                self.reply(&request_key, DesktopReply::Elicitation { accept, content })
                    .await?;
            }
        }
        Ok(())
    }

    async fn reply(&mut self, key: &str, response: DesktopReply) -> Result<()> {
        let request = self
            .session
            .state()
            .pending_requests()
            .find(|request| state_request_key(self.session.state(), request) == key)
            .ok_or(DesktopError::RequestExpired)?;
        let command = DesktopCommand::Reply {
            request_id: request["id"].clone(),
            expected_request_key: key.into(),
            method: request["method"]
                .as_str()
                .ok_or(DesktopError::InvalidFrame)?
                .into(),
            expected_turn_id: request["params"]["turnId"].as_str().map(str::to_owned),
            response,
        };
        self.execute(&command).await?;
        Ok(())
    }

    async fn reconcile(&mut self) -> Result<()> {
        let confirmed: Vec<_> = self
            .pending
            .iter()
            .filter(|item| matches!(item.status.as_str(), "dispatching" | "outcome_unknown"))
            .filter_map(|item| {
                self.session
                    .state()
                    .submission_turn_id(&item.id)
                    .map(|turn| (item.id.clone(), turn))
            })
            .collect();
        if confirmed.is_empty() {
            return Ok(());
        }
        let source = self.source.clone();
        let thread = self.thread_id.clone();
        storage(&self.ledger, move |ledger| {
            for (id, turn) in confirmed {
                ledger.accepted(&source, &thread, &id, &turn)?;
            }
            Ok(())
        })
        .await?;
        self.refresh_queue().await
    }

    async fn dispatch_queued(&mut self) -> Result<()> {
        if self.session.state().check_idle().is_err() {
            return Ok(());
        }
        if let Some(turn) = &self.wait_for_turn {
            // The start response may arrive before the active-state broadcast.
            // Do not interpret that old idle snapshot as permission to send twice.
            if !self.session.state().turn_finished(turn) {
                return Ok(());
            }
            self.wait_for_turn = None;
        }
        let Some(item) = self.pending.first() else {
            return Ok(());
        };
        if item.status != "queued" {
            return Ok(());
        }
        let body = item.body.as_ref().ok_or(DesktopError::InvalidInput)?;
        if body["kind"] != "queue" {
            return Ok(());
        }
        let text = body["text"]
            .as_str()
            .ok_or(DesktopError::InvalidInput)?
            .to_owned();
        let references =
            serde_json::from_value(body.get("references").cloned().unwrap_or_else(|| json!([])))
                .map_err(|_| DesktopError::InvalidInput)?;
        let id = item.id.clone();
        let claimed_id = id.clone();
        let source = self.source.clone();
        let thread = self.thread_id.clone();
        if !storage(&self.ledger, move |ledger| {
            ledger.claim(&source, &thread, &claimed_id)
        })
        .await?
        {
            return Ok(());
        }
        self.submit(
            &id,
            DesktopCommand::Start(MessageInput {
                client_id: id.clone(),
                text,
                references,
            }),
        )
        .await
    }

    async fn submit(&mut self, id: &str, command: DesktopCommand) -> Result<()> {
        let result = self.execute(&command).await;
        let source = self.source.clone();
        let thread = self.thread_id.clone();
        let id = id.to_owned();
        match result {
            Ok(result) => {
                let turn = result.turn_id.ok_or(DesktopError::OutcomeUnknown)?;
                self.wait_for_turn = Some(turn.clone());
                storage(&self.ledger, move |ledger| {
                    ledger.accepted(&source, &thread, &id, &turn)
                })
                .await?;
                self.refresh_queue().await?;
                Ok(())
            }
            Err(error) => {
                storage(&self.ledger, move |ledger| {
                    if matches!(
                        error,
                        DesktopError::OutcomeUnknown
                            | DesktopError::Timeout
                            | DesktopError::Disconnected
                    ) {
                        ledger.outcome_unknown(&source, &thread, &id)
                    } else {
                        ledger.not_sent(&source, &thread, &id)
                    }
                })
                .await?;
                self.refresh_queue().await?;
                Err(error)
            }
        }
    }

    async fn execute(
        &mut self,
        command: &DesktopCommand,
    ) -> Result<super::commands::CommandResult> {
        if let Some(paths) = self.paths.clone() {
            let thread = self.thread_id.clone();
            crate::read_tasks::spawn_blocking(move || {
                let summary = crate::codex::local_thread_summary(&paths, &thread)
                    .map_err(|_| DesktopError::Unavailable)?
                    .ok_or(DesktopError::ReadOnly)?;
                if summary.status == crate::codex::ThreadStatus::Archived {
                    return Err(DesktopError::ReadOnly);
                }
                Ok(())
            })
            .await
            .map_err(|_| DesktopError::Unavailable)??;
        }
        let session_id = &self.session_id;
        let queue = queue_view(&self.pending);
        let sender = &self.view;
        self.session
            .execute_observing(command, |state| {
                sender.send_replace(projection::view(state, session_id, queue.clone()));
            })
            .await
    }
}

fn queue_view(pending: &[Submission]) -> Vec<DesktopQueuedMessage> {
    pending
        .iter()
        .map(|item| DesktopQueuedMessage {
            id: item.id.clone(),
            status: item.status.clone(),
            kind: item
                .body
                .as_ref()
                .and_then(|body| body["kind"].as_str())
                .unwrap_or("queue")
                .into(),
            preview: bounded(
                item.body
                    .as_ref()
                    .and_then(|body| body["text"].as_str())
                    .unwrap_or(""),
                2048,
            ),
            created_at: item.created_at,
        })
        .collect()
}

async fn storage<T: Send + 'static>(
    ledger: &DesktopLedger,
    work: impl FnOnce(DesktopLedger) -> anyhow::Result<T> + Send + 'static,
) -> Result<T> {
    let ledger = ledger.clone();
    crate::read_tasks::spawn_blocking(move || work(ledger))
        .await
        .map_err(|_| DesktopError::Unavailable)?
        .map_err(|_| DesktopError::Unavailable)
}

fn validate_action(action: &DesktopAction) -> Result<()> {
    // Bound the channel before taking ownership; input validation is repeated
    // by the native command builder immediately before dispatch.
    let bytes = serde_json::to_vec(action).map_err(|_| DesktopError::InvalidInput)?;
    if bytes.len() > 1024 * 1024 {
        return Err(DesktopError::InvalidInput);
    }
    match action {
        DesktopAction::Send {
            message_id,
            text,
            references,
        }
        | DesktopAction::Steer {
            message_id,
            text,
            references,
            ..
        } if uuid::Uuid::parse_str(message_id).is_err()
            || (text.trim().is_empty() && references.is_empty())
            || text.len() > 256 * 1024
            || text.contains('\0')
            || references.len() > 32 =>
        {
            return Err(DesktopError::InvalidInput);
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
