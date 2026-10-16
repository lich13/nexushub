use super::{
    commands::{CommandResult, DesktopCommand, NativeRequest},
    state::NativeState,
    DesktopError, DesktopReadSession, Result,
};
use crate::resource_limits::CODEX_DESKTOP_CONNECT_WAIT;
use serde_json::{json, Value};
use std::path::Path;
use tokio::time::timeout;

/// A single actor must own this session. Commands are serialized with stream
/// events; awaiting a reply does not discard native state or approval changes.
pub struct DesktopSession {
    connection: DesktopReadSession,
    state: NativeState,
}

impl DesktopSession {
    pub async fn attach(codex_home: &Path, thread_id: &str) -> Result<Self> {
        let connection = DesktopReadSession::attach(codex_home, thread_id).await?;
        let mut session = Self {
            connection,
            state: NativeState::new(thread_id)?,
        };
        timeout(CODEX_DESKTOP_CONNECT_WAIT, async {
            while session.state.revision().is_none() {
                session.next_change().await?;
            }
            Ok(())
        })
        .await
        .map_err(|_| DesktopError::Timeout)??;
        Ok(session)
    }

    pub fn state(&self) -> &NativeState {
        &self.state
    }

    pub async fn next_change(&mut self) -> Result<()> {
        let event = self.connection.next_event().await?;
        self.state.apply(event).inspect_err(|_| {
            self.connection.stream.take();
        })
    }

    pub async fn execute(&mut self, command: &DesktopCommand) -> Result<CommandResult> {
        self.execute_observing(command, |_| {}).await
    }

    pub async fn execute_observing(
        &mut self,
        command: &DesktopCommand,
        mut changed: impl FnMut(&NativeState),
    ) -> Result<CommandResult> {
        let references = command.references().to_vec();
        if !references.is_empty() {
            crate::read_tasks::spawn_blocking(move || {
                references
                    .iter()
                    .try_for_each(|reference| reference.validate())
            })
            .await
            .map_err(|_| DesktopError::Unavailable)??;
        }
        // Discovery is read-only. An owner change aborts before any mutation;
        // the user must explicitly attach again and inspect the new state.
        self.recheck_owner(&mut changed).await?;
        let request = command.prepare(&self.state)?;
        let response = match self.exchange(request, true, &mut changed).await {
            Ok(response) => response,
            Err(error @ (DesktopError::OwnerUnavailable | DesktopError::UnsupportedProtocol)) => {
                return Err(error)
            }
            Err(_) => {
                self.connection.stream.take();
                return Err(DesktopError::OutcomeUnknown);
            }
        };
        command.result(&response["result"]).map_err(|error| {
            if error == DesktopError::Rejected {
                error
            } else {
                DesktopError::OutcomeUnknown
            }
        })
    }

    pub async fn close(self) {
        self.connection.close().await;
    }

    async fn recheck_owner(&mut self, changed: &mut impl FnMut(&NativeState)) -> Result<()> {
        let response = self
            .exchange(
                NativeRequest {
                    method: "thread-owner-discovery",
                    version: 1,
                    params: json!({"hostId":"local","conversationId":self.state.thread_id()}),
                },
                false,
                changed,
            )
            .await
            .inspect_err(|_| {
                self.connection.stream.take();
            })?;
        if response["handledByClientId"].as_str() != Some(&self.connection.owner_id) {
            self.connection.stream.take();
            return Err(DesktopError::OwnerUnavailable);
        }
        Ok(())
    }

    async fn exchange(
        &mut self,
        request: NativeRequest,
        target_owner: bool,
        changed: &mut impl FnMut(&NativeState),
    ) -> Result<Value> {
        let operation = async {
            let request_id = uuid::Uuid::new_v4().to_string();
            let mut envelope = json!({
                "type":"request", "requestId":request_id,
                "sourceClientId":self.connection.client_id,
                "method":request.method,"version":request.version,"params":request.params,
                "timeoutMs":CODEX_DESKTOP_CONNECT_WAIT.as_millis() as u64,
            });
            if target_owner {
                envelope["targetClientId"] = json!(self.connection.owner_id);
            }
            self.connection.send(&envelope).await?;
            loop {
                let message = self.connection.receive().await?;
                if message["type"] == "response" && message["requestId"] == request_id {
                    if target_owner
                        && message
                            .get("handledByClientId")
                            .is_some_and(|id| id.as_str() != Some(&self.connection.owner_id))
                    {
                        return Err(DesktopError::InvalidIdentity);
                    }
                    return match message["resultType"].as_str() {
                        Some("success") => Ok(message),
                        Some("error") => Err(match message["error"].as_str() {
                            Some("no-client-found") => DesktopError::OwnerUnavailable,
                            Some("unsupported-version" | "method-not-found") => {
                                DesktopError::UnsupportedProtocol
                            }
                            Some("timeout") => DesktopError::Timeout,
                            _ => DesktopError::Rejected,
                        }),
                        _ => Err(DesktopError::InvalidFrame),
                    };
                }
                if let Some(event) = self.connection.event_from_message(message)? {
                    self.state.apply(event)?;
                    changed(&self.state);
                }
            }
        };
        timeout(CODEX_DESKTOP_CONNECT_WAIT, operation)
            .await
            .map_err(|_| DesktopError::Timeout)?
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
