//! Short-lived native helper for catalogs and empty-thread creation only.
//! It has no resume/turn/queue execution entrypoint. All conversation execution
//! is routed to the already running desktop owner through the IPC session.
use super::{DesktopError, Result};
use crate::resource_limits::CODEX_DESKTOP_FRAME_BYTES;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::OnceLock,
    time::Duration,
};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Semaphore,
    time::timeout,
};

const HELPER_WAIT: Duration = Duration::from_secs(20);
static HELPER_GATE: OnceLock<Semaphore> = OnceLock::new();

pub enum CatalogQuery {
    Models {
        cursor: Option<String>,
    },
    Modes,
    Skills {
        cwd: PathBuf,
    },
    Plugins {
        cwd: PathBuf,
    },
    Apps {
        thread_id: Option<String>,
        cursor: Option<String>,
    },
}

pub struct DesktopHelper {
    executable: PathBuf,
    codex_home: PathBuf,
}

impl DesktopHelper {
    pub async fn installed(codex_home: PathBuf) -> Result<Self> {
        let executable = crate::read_tasks::spawn_blocking(|| {
            let bundle = Path::new("/Applications/ChatGPT.app/Contents/Resources");
            for relative in ["codex-cli/bin/codex", "codex"] {
                let candidate = bundle.join(relative);
                if let Ok(metadata) = candidate.metadata() {
                    use std::os::unix::fs::PermissionsExt;
                    if metadata.is_file()
                        && metadata.permissions().mode() & 0o111 != 0
                        && metadata.permissions().mode() & 0o022 == 0
                    {
                        let canonical = candidate
                            .canonicalize()
                            .map_err(|_| DesktopError::Unavailable)?;
                        if canonical.starts_with(bundle) {
                            return Ok(canonical);
                        }
                    }
                }
            }
            Err(DesktopError::Unavailable)
        })
        .await
        .map_err(|_| DesktopError::Unavailable)??;
        Ok(Self {
            executable,
            codex_home,
        })
    }

    pub async fn catalog(&self, query: CatalogQuery) -> Result<Value> {
        let (method, params) = match query {
            CatalogQuery::Models { cursor } => (
                "model/list",
                json!({"includeHidden":false,"limit":100,"cursor":cursor}),
            ),
            CatalogQuery::Modes => ("collaborationMode/list", json!({})),
            CatalogQuery::Skills { cwd } => {
                ("skills/list", json!({"cwds":[cwd],"forceReload":false}))
            }
            CatalogQuery::Plugins { cwd } => {
                ("plugin/list", json!({"cwds":[cwd],"forceRefetch":false}))
            }
            CatalogQuery::Apps { thread_id, cursor } => {
                if thread_id
                    .as_ref()
                    .is_some_and(|id| uuid::Uuid::parse_str(id).is_err())
                    || cursor.as_ref().is_some_and(|cursor| cursor.len() > 4096)
                {
                    return Err(DesktopError::InvalidInput);
                }
                (
                    "app/list",
                    json!({"threadId":thread_id,"cursor":cursor,"limit":100,"forceRefetch":false}),
                )
            }
        };
        self.with_process(method, params).await
    }

    /// Native thread/start creates an empty persistent thread; it never starts a
    /// turn. Loading it into a desktop window remains a separate explicit action.
    pub async fn create_empty(&self, cwd: &Path) -> Result<String> {
        if !cwd.is_absolute() {
            return Err(DesktopError::InvalidInput);
        }
        let result = self
            .with_process(
                "thread/start",
                json!({
                    "cwd":cwd,"ephemeral":false,"historyMode":"paginated"
                }),
            )
            .await?;
        let id = result["thread"]["id"]
            .as_str()
            .ok_or(DesktopError::InvalidFrame)?;
        uuid::Uuid::parse_str(id).map_err(|_| DesktopError::InvalidIdentity)?;
        Ok(id.into())
    }

    async fn with_process(&self, method: &'static str, params: Value) -> Result<Value> {
        let _permit = timeout(
            HELPER_WAIT,
            HELPER_GATE.get_or_init(|| Semaphore::new(1)).acquire(),
        )
        .await
        .map_err(|_| DesktopError::Timeout)?
        .map_err(|_| DesktopError::Unavailable)?;
        let mut process = HelperProcess::start(&self.executable, &self.codex_home)?;
        let mut submitted = false;
        let operation = async {
            process
                .request(
                    1,
                    "initialize",
                    json!({
                        "clientInfo":{"name":"nexushub","version":env!("CARGO_PKG_VERSION")},
                        "capabilities":{"experimentalApi":true}
                    }),
                )
                .await?;
            process.write(&json!({"method":"initialized"})).await?;
            submitted = true;
            process.request(2, method, params).await
        };
        let result = timeout(HELPER_WAIT, operation)
            .await
            .map_err(|_| DesktopError::Timeout)
            .and_then(|result| result);
        // Reap only the helper we created. kill_on_drop also covers cancellation.
        process.stdin.take();
        if timeout(Duration::from_secs(1), process.child.wait())
            .await
            .is_err()
        {
            let _ = process.child.kill().await;
        }
        if method == "thread/start"
            && submitted
            && result
                .as_ref()
                .is_err_and(|error| !matches!(error, DesktopError::Rejected))
        {
            return Err(DesktopError::OutcomeUnknown);
        }
        result
    }
}

struct HelperProcess {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl HelperProcess {
    fn start(executable: &Path, codex_home: &Path) -> Result<Self> {
        let mut child = Command::new(executable)
            .args(["app-server", "--listen", "stdio://"])
            .env("CODEX_HOME", codex_home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|_| DesktopError::Unavailable)?;
        let stdin = child.stdin.take().ok_or(DesktopError::Unavailable)?;
        let stdout = child.stdout.take().ok_or(DesktopError::Unavailable)?;
        Ok(Self {
            child,
            stdin: Some(stdin),
            stdout: BufReader::new(stdout),
        })
    }

    async fn write(&mut self, message: &Value) -> Result<()> {
        let mut bytes = serde_json::to_vec(message).map_err(|_| DesktopError::InvalidInput)?;
        if bytes.len() > CODEX_DESKTOP_FRAME_BYTES {
            return Err(DesktopError::FrameTooLarge);
        }
        bytes.push(b'\n');
        let stdin = self.stdin.as_mut().ok_or(DesktopError::Disconnected)?;
        stdin
            .write_all(&bytes)
            .await
            .map_err(|_| DesktopError::Disconnected)?;
        stdin.flush().await.map_err(|_| DesktopError::Disconnected)
    }

    async fn request(&mut self, id: u64, method: &'static str, params: Value) -> Result<Value> {
        self.write(&json!({"id":id,"method":method,"params":params}))
            .await?;
        loop {
            let mut value = read_bounded_line(&mut self.stdout).await?;
            if value["id"].as_u64() != Some(id) {
                continue;
            }
            if value.get("error").is_some() {
                return Err(DesktopError::Rejected);
            }
            return value
                .as_object_mut()
                .and_then(|value| value.remove("result"))
                .ok_or(DesktopError::InvalidFrame);
        }
    }
}

async fn read_bounded_line(reader: &mut (impl AsyncBufRead + Unpin)) -> Result<Value> {
    let mut bytes = Vec::new();
    loop {
        let available = reader
            .fill_buf()
            .await
            .map_err(|_| DesktopError::Disconnected)?;
        if available.is_empty() {
            return Err(DesktopError::Disconnected);
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let length = newline.map_or(available.len(), |index| index + 1);
        if bytes.len().saturating_add(length) > CODEX_DESKTOP_FRAME_BYTES {
            return Err(DesktopError::FrameTooLarge);
        }
        bytes.extend_from_slice(&available[..length]);
        reader.consume(length);
        if newline.is_some() {
            let value: Value =
                serde_json::from_slice(&bytes).map_err(|_| DesktopError::InvalidFrame)?;
            if !value.is_object() {
                return Err(DesktopError::InvalidFrame);
            }
            return Ok(value);
        }
    }
}

#[cfg(test)]
#[path = "helper_tests.rs"]
mod tests;
