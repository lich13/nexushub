use anyhow::{anyhow, Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    cmp::Ordering,
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{ChildStdin, Command},
    sync::Mutex,
    task::JoinSet,
    time::{timeout, timeout_at, Instant},
};

const MIN_CODEX_GOAL_VERSION: ParsedVersion = ParsedVersion::stable(0, 144, 2);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_PROTOCOL_LINE_BYTES: usize = 1024 * 1024;
const MAX_STDOUT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_STDERR_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexThreadGoal {
    pub thread_id: String,
    pub objective: String,
    pub status: String,
    pub token_budget: Option<u64>,
    #[serde(default)]
    pub tokens_used: u64,
    #[serde(default)]
    pub time_used_seconds: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodexGoalAction {
    Get,
    Save {
        objective: String,
        token_budget: Option<u64>,
    },
    Clear,
    Pause,
    Resume,
    RecoverRestricted,
}

impl CodexGoalAction {
    fn is_mutation(&self) -> bool {
        !matches!(self, Self::Get)
    }
}

#[derive(Clone)]
pub struct CodexGoalClient {
    inner: Arc<CodexGoalClientInner>,
}

struct CodexGoalClientInner {
    candidates: Option<Vec<PathBuf>>,
    request_timeout: Duration,
    selected_executable: Mutex<Option<PathBuf>>,
    operation_gate: Mutex<()>,
}

impl Default for CodexGoalClient {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexGoalClient {
    pub fn new() -> Self {
        Self::with_optional_candidates(None, DEFAULT_REQUEST_TIMEOUT)
    }

    #[doc(hidden)]
    pub fn with_candidates(candidates: Vec<PathBuf>, request_timeout: Duration) -> Self {
        Self::with_optional_candidates(Some(candidates), request_timeout)
    }

    #[cfg(test)]
    fn with_resolved_executable(executable: PathBuf, request_timeout: Duration) -> Self {
        Self {
            inner: Arc::new(CodexGoalClientInner {
                candidates: Some(vec![executable.clone()]),
                request_timeout,
                selected_executable: Mutex::new(Some(executable)),
                operation_gate: Mutex::new(()),
            }),
        }
    }

    fn with_optional_candidates(
        candidates: Option<Vec<PathBuf>>,
        request_timeout: Duration,
    ) -> Self {
        Self {
            inner: Arc::new(CodexGoalClientInner {
                candidates,
                request_timeout,
                selected_executable: Mutex::new(None),
                operation_gate: Mutex::new(()),
            }),
        }
    }

    pub async fn resolve_executable(&self) -> Result<PathBuf> {
        if let Some(path) = self.inner.selected_executable.lock().await.clone() {
            return Ok(path);
        }
        let candidates = self
            .inner
            .candidates
            .clone()
            .unwrap_or_else(discover_codex_candidates);
        let mut best: Option<(ParsedVersion, usize, PathBuf)> = None;
        let mut seen = HashSet::new();
        let mut probes = JoinSet::new();
        for (priority, candidate) in candidates.into_iter().enumerate() {
            let identity = fs::canonicalize(&candidate).unwrap_or_else(|_| candidate.clone());
            if !seen.insert(identity) || !is_executable_file(&candidate) {
                continue;
            }
            probes.spawn(async move {
                let version = probe_codex_version(&candidate).await;
                (priority, candidate, version)
            });
        }
        while let Some(probe) = probes.join_next().await {
            let Ok((priority, candidate, Some(version))) = probe else {
                continue;
            };
            if version < MIN_CODEX_GOAL_VERSION {
                continue;
            }
            let replace = best.as_ref().is_none_or(|(current, current_priority, _)| {
                version > *current || (version == *current && priority < *current_priority)
            });
            if replace {
                best = Some((version, priority, candidate));
            }
        }
        let path = best.map(|(_, _, path)| path).ok_or_else(|| {
            anyhow!("Codex CLI 0.144.2 or newer is required for thread Goal support")
        })?;
        *self.inner.selected_executable.lock().await = Some(path.clone());
        Ok(path)
    }

    pub async fn execute(
        &self,
        codex_home: &Path,
        thread_id: &str,
        action: CodexGoalAction,
    ) -> Result<Option<CodexThreadGoal>> {
        let thread_id = thread_id.trim();
        if thread_id.is_empty() {
            return Err(anyhow!("thread_id is required"));
        }
        let _operation = self.inner.operation_gate.lock().await;
        let deadline = Instant::now() + self.inner.request_timeout;
        let executable = timeout_at(deadline, self.resolve_executable())
            .await
            .map_err(|_| anyhow!("Codex CLI discovery timed out"))??;
        let mut command = Command::new(&executable);
        command
            .arg("app-server")
            .arg("--stdio")
            .env("CODEX_HOME", codex_home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().context("start Codex app-server")?;
        let Some(mut stdin) = child.stdin.take() else {
            stop_child(&mut child).await;
            return Err(anyhow!("open Codex app-server stdin"));
        };
        let Some(stdout) = child.stdout.take() else {
            drop(stdin);
            stop_child(&mut child).await;
            return Err(anyhow!("open Codex app-server stdout"));
        };
        let Some(stderr) = child.stderr.take() else {
            drop(stdin);
            drop(stdout);
            stop_child(&mut child).await;
            return Err(anyhow!("open Codex app-server stderr"));
        };
        let stderr_task = tokio::spawn(async move {
            let mut bytes = Vec::new();
            let _ = stderr.take(MAX_STDERR_BYTES).read_to_end(&mut bytes).await;
            bytes
        });
        let mut stdout = BufReader::new(stdout.take(MAX_STDOUT_BYTES));
        let mutation = action.is_mutation();
        let result = match timeout_at(
            deadline,
            run_goal_session(&mut stdin, &mut stdout, thread_id, action),
        )
        .await
        {
            Ok(result) => result,
            Err(_) if mutation => Err(anyhow!(
                "Codex Goal mutation timed out; the resulting state is unknown"
            )),
            Err(_) => Err(anyhow!("Codex Goal query timed out")),
        };
        drop(stdin);
        stop_child(&mut child).await;
        stderr_task.abort();
        let _ = stderr_task.await;
        result
    }
}

async fn stop_child(child: &mut tokio::process::Child) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}

async fn run_goal_session(
    stdin: &mut ChildStdin,
    stdout: &mut (impl tokio::io::AsyncBufRead + Unpin),
    thread_id: &str,
    action: CodexGoalAction,
) -> Result<Option<CodexThreadGoal>> {
    write_message(
        stdin,
        &json!({
            "id": 1,
            "method": "initialize",
            "params": {
                "clientInfo": {"name": "nexushub", "version": env!("CARGO_PKG_VERSION")},
                "capabilities": {"experimentalApi": true}
            }
        }),
    )
    .await?;
    let _: Value = read_response(stdout, 1).await?;
    write_message(stdin, &json!({"method": "initialized"})).await?;

    let mut next_id = 2_i64;
    match action {
        CodexGoalAction::Get => get_goal(stdin, stdout, &mut next_id, thread_id).await,
        CodexGoalAction::Save {
            objective,
            token_budget,
        } => {
            let objective = objective.trim();
            if objective.is_empty() {
                return Err(anyhow!("objective is required"));
            }
            set_goal(
                stdin,
                stdout,
                &mut next_id,
                json!({
                    "threadId": thread_id,
                    "objective": objective,
                    "tokenBudget": token_budget
                }),
            )
            .await
            .map(Some)
        }
        CodexGoalAction::Clear => {
            let _: Value = call(
                stdin,
                stdout,
                &mut next_id,
                "thread/goal/clear",
                json!({"threadId": thread_id}),
            )
            .await?;
            get_goal(stdin, stdout, &mut next_id, thread_id).await
        }
        CodexGoalAction::Pause => {
            let goal = required_goal(
                get_goal(stdin, stdout, &mut next_id, thread_id).await?,
                thread_id,
            )?;
            if goal.status != "active" {
                return Err(anyhow!(
                    "Goal can only be paused from active status, current status is {}",
                    goal.status
                ));
            }
            set_goal(
                stdin,
                stdout,
                &mut next_id,
                json!({"threadId": thread_id, "status": "paused"}),
            )
            .await
            .map(Some)
        }
        CodexGoalAction::Resume => {
            let goal = required_goal(
                get_goal(stdin, stdout, &mut next_id, thread_id).await?,
                thread_id,
            )?;
            if !matches!(
                goal.status.as_str(),
                "paused" | "blocked" | "usageLimited" | "budgetLimited" | "complete"
            ) {
                return Err(anyhow!(
                    "Goal cannot be resumed from {} status",
                    goal.status
                ));
            }
            set_goal(
                stdin,
                stdout,
                &mut next_id,
                json!({"threadId": thread_id, "status": "active"}),
            )
            .await
            .map(Some)
        }
        CodexGoalAction::RecoverRestricted => {
            let Some(goal) = get_goal(stdin, stdout, &mut next_id, thread_id).await? else {
                return Ok(None);
            };
            if !matches!(
                goal.status.as_str(),
                "blocked" | "usageLimited" | "budgetLimited"
            ) {
                return Ok(Some(goal));
            }
            set_goal(
                stdin,
                stdout,
                &mut next_id,
                json!({"threadId": thread_id, "status": "active"}),
            )
            .await
            .map(Some)
        }
    }
}

fn required_goal(goal: Option<CodexThreadGoal>, thread_id: &str) -> Result<CodexThreadGoal> {
    goal.ok_or_else(|| anyhow!("no Goal exists for thread {thread_id}"))
}

async fn get_goal(
    stdin: &mut ChildStdin,
    stdout: &mut (impl tokio::io::AsyncBufRead + Unpin),
    next_id: &mut i64,
    thread_id: &str,
) -> Result<Option<CodexThreadGoal>> {
    #[derive(Deserialize)]
    struct GetResponse {
        goal: Option<CodexThreadGoal>,
    }
    let response: GetResponse = call(
        stdin,
        stdout,
        next_id,
        "thread/goal/get",
        json!({"threadId": thread_id}),
    )
    .await?;
    Ok(response.goal)
}

async fn set_goal(
    stdin: &mut ChildStdin,
    stdout: &mut (impl tokio::io::AsyncBufRead + Unpin),
    next_id: &mut i64,
    params: Value,
) -> Result<CodexThreadGoal> {
    #[derive(Deserialize)]
    struct SetResponse {
        goal: CodexThreadGoal,
    }
    let response: SetResponse = call(stdin, stdout, next_id, "thread/goal/set", params).await?;
    Ok(response.goal)
}

async fn call<T: DeserializeOwned>(
    stdin: &mut ChildStdin,
    stdout: &mut (impl tokio::io::AsyncBufRead + Unpin),
    next_id: &mut i64,
    method: &str,
    params: Value,
) -> Result<T> {
    let id = *next_id;
    *next_id += 1;
    write_message(
        stdin,
        &json!({"id": id, "method": method, "params": params}),
    )
    .await?;
    read_response(stdout, id).await
}

pub(super) async fn write_message(stdin: &mut ChildStdin, value: &Value) -> Result<()> {
    let mut encoded = serde_json::to_vec(value)?;
    encoded.push(b'\n');
    stdin.write_all(&encoded).await?;
    stdin.flush().await?;
    Ok(())
}

pub(super) async fn read_response<T: DeserializeOwned>(
    stdout: &mut (impl tokio::io::AsyncBufRead + Unpin),
    expected_id: i64,
) -> Result<T> {
    loop {
        let mut line = String::new();
        let read = stdout.read_line(&mut line).await?;
        if read == 0 {
            return Err(anyhow!(
                "Codex app-server closed before response {expected_id}"
            ));
        }
        if line.len() > MAX_PROTOCOL_LINE_BYTES {
            return Err(anyhow!("Codex app-server response exceeded size limit"));
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            return Err(anyhow!("Codex app-server returned invalid JSON"));
        };
        if value.get("id").and_then(Value::as_i64) != Some(expected_id) {
            continue;
        }
        if let Some(error) = value.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown Codex app-server error");
            return Err(anyhow!("Codex app-server request failed: {message}"));
        }
        let result = value
            .get("result")
            .cloned()
            .ok_or_else(|| anyhow!("Codex app-server response did not include result"))?;
        return serde_json::from_value(result).context("decode Codex app-server response");
    }
}

async fn probe_codex_version(path: &Path) -> Option<ParsedVersion> {
    let mut command = Command::new(path);
    command.arg("--version").kill_on_drop(true);
    let output = timeout(VERSION_PROBE_TIMEOUT, command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_codex_version(&stdout)
}

fn discover_codex_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("PATH") {
        candidates.extend(env::split_paths(&path).map(|entry| entry.join("codex")));
    }
    if let Some(home) = dirs::home_dir() {
        let nvm_root = home.join(".nvm/versions/node");
        if let Ok(entries) = fs::read_dir(nvm_root) {
            let mut paths = entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path().join("bin/codex")))
                .collect::<Vec<_>>();
            paths.sort();
            candidates.extend(paths);
        }
        candidates.push(home.join(".volta/bin/codex"));
        candidates.push(home.join(".local/bin/codex"));
    }
    candidates.extend([
        PathBuf::from("/opt/homebrew/bin/codex"),
        PathBuf::from("/usr/local/bin/codex"),
        PathBuf::from("/usr/bin/codex"),
        PathBuf::from("/Applications/ChatGPT.app/Contents/Resources/codex"),
    ]);
    candidates
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ParsedVersion {
    major: u64,
    minor: u64,
    patch: u64,
    stable: bool,
    suffix: String,
}

impl ParsedVersion {
    const fn stable(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            stable: true,
            suffix: String::new(),
        }
    }
}

impl Ord for ParsedVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            self.major,
            self.minor,
            self.patch,
            self.stable,
            self.suffix.as_str(),
        )
            .cmp(&(
                other.major,
                other.minor,
                other.patch,
                other.stable,
                other.suffix.as_str(),
            ))
    }
}

impl PartialOrd for ParsedVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn parse_codex_version(value: &str) -> Option<ParsedVersion> {
    value.split_whitespace().find_map(|part| {
        let part = part.trim_start_matches('v');
        let (core, suffix) = part.split_once('-').unwrap_or((part, ""));
        let mut pieces = core.split('.');
        let major = pieces.next()?.parse().ok()?;
        let minor = pieces.next()?.parse().ok()?;
        let patch = pieces.next()?.parse().ok()?;
        if pieces.next().is_some() {
            return None;
        }
        Some(ParsedVersion {
            major,
            minor,
            patch,
            stable: suffix.is_empty(),
            suffix: suffix.to_string(),
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{CodexGoalAction, CodexGoalClient};
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        process::{Command as StdCommand, Stdio as StdStdio},
        sync::OnceLock,
        time::Duration,
    };

    static PROCESS_TEST_GATE: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

    async fn process_test_guard() -> tokio::sync::MutexGuard<'static, ()> {
        PROCESS_TEST_GATE
            .get_or_init(|| tokio::sync::Mutex::new(()))
            .lock()
            .await
    }

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "nexushub-{name}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_executable(path: &Path, body: &str) {
        fs::write(path, body).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
    }

    #[tokio::test]
    async fn codex_goal_client_prefers_supported_cli_over_old_bundle() {
        let _guard = process_test_guard().await;
        let root = temp_dir("goal-cli-version");
        let old = root.join("bundle-codex");
        let current = root.join("nvm-codex");
        write_executable(&old, "#!/bin/sh\necho 'codex-cli 0.144.0-alpha.4'\n");
        write_executable(&current, "#!/bin/sh\necho 'codex-cli 0.144.2'\n");

        let client =
            CodexGoalClient::with_candidates(vec![old, current.clone()], Duration::from_secs(2));
        let selected = client.resolve_executable().await.unwrap();

        assert_eq!(selected, current);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn codex_goal_client_rejects_cli_versions_without_goal_support() {
        let _guard = process_test_guard().await;
        let root = temp_dir("goal-cli-unsupported");
        let executable = root.join("codex");
        write_executable(&executable, "#!/bin/sh\necho 'codex-cli 0.144.2-alpha.1'\n");
        let client = CodexGoalClient::with_candidates(vec![executable], Duration::from_secs(10));

        let error = client.resolve_executable().await.unwrap_err();

        assert!(error
            .to_string()
            .contains("Codex CLI 0.144.2 or newer is required"));
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn codex_goal_recover_restricted_only_mutates_recoverable_statuses() {
        for (status, should_set) in [
            ("blocked", true),
            ("usageLimited", true),
            ("budgetLimited", true),
            ("active", false),
            ("paused", false),
            ("complete", false),
        ] {
            let _guard = process_test_guard().await;
            let root = temp_dir(&format!("goal-recover-{status}"));
            let executable = root.join("codex");
            write_executable(
                &executable,
                &format!(
                    r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'codex-cli 0.144.2'
  exit 0
fi
LOG='{}'
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$LOG"
  case "$line" in
    *'"method":"initialize"'*)
      echo '{{"id":1,"result":{{"userAgent":"fake","codexHome":"/tmp/codex-home","platformFamily":"unix","platformOs":"macos"}}}}'
      ;;
    *'"method":"thread/goal/get"'*)
      echo '{{"id":2,"result":{{"goal":{{"threadId":"thread-live","objective":"Recover me","status":"{status}","tokenBudget":9000,"tokensUsed":10,"timeUsedSeconds":2,"createdAt":100,"updatedAt":200}}}}}}'
      ;;
    *'"method":"thread/goal/set"'*)
      echo '{{"id":3,"result":{{"goal":{{"threadId":"thread-live","objective":"Recover me","status":"active","tokenBudget":9000,"tokensUsed":10,"timeUsedSeconds":2,"createdAt":100,"updatedAt":201}}}}}}'
      ;;
  esac
done
"#,
                    root.join("app-server-input.log").display()
                ),
            );
            let client = CodexGoalClient::with_resolved_executable(
                executable.clone(),
                Duration::from_secs(2),
            );
            let goal = client
                .execute(&root, "thread-live", CodexGoalAction::RecoverRestricted)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(goal.status, if should_set { "active" } else { status });
            let input = fs::read_to_string(root.join("app-server-input.log")).unwrap_or_default();
            assert_eq!(input.contains("\"method\":\"thread/goal/set\""), should_set);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[tokio::test]
    async fn codex_goal_recover_restricted_leaves_missing_goal_unchanged() {
        let _guard = process_test_guard().await;
        let root = temp_dir("goal-recover-missing");
        let executable = root.join("codex");
        write_executable(
            &executable,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'codex-cli 0.144.2'
  exit 0
fi
while IFS= read -r line; do
  case "$line" in
    *'"method":"initialize"'*)
      echo '{"id":1,"result":{"userAgent":"fake"}}'
      ;;
    *'"method":"thread/goal/get"'*)
      echo '{"id":2,"result":{"goal":null}}'
      ;;
    *'"method":"thread/goal/set"'*)
      exit 91
      ;;
  esac
done
"#,
        );
        let client = CodexGoalClient::with_resolved_executable(executable, Duration::from_secs(2));

        let goal = client
            .execute(&root, "thread-live", CodexGoalAction::RecoverRestricted)
            .await
            .unwrap();

        assert_eq!(goal, None);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn codex_goal_client_get_matches_response_id_and_ignores_notifications() {
        let _guard = process_test_guard().await;
        let root = temp_dir("goal-get");
        let executable = root.join("codex");
        write_executable(
            &executable,
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'codex-cli 0.144.2'
  exit 0
fi
while IFS= read -r line; do
  case "$line" in
    *'"method":"initialize"'*)
      echo '{"id":1,"result":{"userAgent":"fake","codexHome":"/tmp/codex-home","platformFamily":"unix","platformOs":"macos"}}'
      ;;
    *'"method":"initialized"'*)
      echo '{"method":"remoteControl/status/changed","params":{"status":"disabled"}}'
      ;;
    *'"method":"thread/goal/get"'*)
      echo '{"id":999,"result":{"goal":null}}'
      echo '{"id":2,"result":{"goal":{"threadId":"thread-live","objective":"Ship the real goal","status":"active","tokenBudget":12000,"tokensUsed":321,"timeUsedSeconds":45,"createdAt":100,"updatedAt":200}}}'
      ;;
  esac
done
"#,
        );
        let client = CodexGoalClient::with_resolved_executable(executable, Duration::from_secs(2));

        let goal = client
            .execute(&root, "thread-live", CodexGoalAction::Get)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(goal.thread_id, "thread-live");
        assert_eq!(goal.objective, "Ship the real goal");
        assert_eq!(goal.status, "active");
        assert_eq!(goal.token_budget, Some(12_000));
        assert_eq!(goal.tokens_used, 321);
        assert_eq!(goal.time_used_seconds, 45);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn codex_goal_client_pause_reads_official_state_before_setting_paused() {
        let _guard = process_test_guard().await;
        let root = temp_dir("goal-pause");
        let executable = root.join("codex");
        let capture = root.join("stdin.jsonl");
        let script = format!(
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'codex-cli 0.144.2'
  exit 0
fi
while IFS= read -r line; do
  printf '%s\n' "$line" >> '{}'
  case "$line" in
    *'"method":"initialize"'*)
      echo '{{"id":1,"result":{{"userAgent":"fake","codexHome":"/tmp/codex-home","platformFamily":"unix","platformOs":"linux"}}}}'
      ;;
    *'"method":"thread/goal/get"'*)
      echo '{{"id":2,"result":{{"goal":{{"threadId":"thread-live","objective":"Keep objective","status":"active","tokenBudget":9000,"tokensUsed":10,"timeUsedSeconds":2,"createdAt":100,"updatedAt":200}}}}}}'
      ;;
    *'"method":"thread/goal/set"'*)
      echo '{{"id":3,"result":{{"goal":{{"threadId":"thread-live","objective":"Keep objective","status":"paused","tokenBudget":9000,"tokensUsed":10,"timeUsedSeconds":2,"createdAt":100,"updatedAt":201}}}}}}'
      ;;
  esac
done
"#,
            capture.display()
        );
        write_executable(&executable, &script);
        let client = CodexGoalClient::with_resolved_executable(executable, Duration::from_secs(2));

        let goal = client
            .execute(&root, "thread-live", CodexGoalAction::Pause)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(goal.status, "paused");
        let input = fs::read_to_string(capture).unwrap();
        assert!(input.contains("\"experimentalApi\":true"));
        assert!(input.contains("\"method\":\"thread/goal/get\""));
        assert!(input.contains("\"method\":\"thread/goal/set\""));
        assert!(input.contains("\"status\":\"paused\""));
        assert!(!input.contains("\"objective\":\"Keep objective\""));
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn codex_goal_client_save_preserves_paused_status() {
        let _guard = process_test_guard().await;
        let root = temp_dir("goal-save-paused");
        let executable = root.join("codex");
        let capture = root.join("stdin.jsonl");
        let script = format!(
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'codex-cli 0.144.2'
  exit 0
fi
while IFS= read -r line; do
  printf '%s\n' "$line" >> '{}'
  case "$line" in
    *'"method":"initialize"'*)
      echo '{{"id":1,"result":{{"userAgent":"fake"}}}}'
      ;;
    *'"method":"thread/goal/set"'*)
      echo '{{"id":2,"result":{{"goal":{{"threadId":"thread-live","objective":"Updated objective","status":"paused","tokenBudget":8000,"tokensUsed":10,"timeUsedSeconds":2,"createdAt":100,"updatedAt":201}}}}}}'
      ;;
  esac
done
"#,
            capture.display()
        );
        write_executable(&executable, &script);
        let client = CodexGoalClient::with_resolved_executable(executable, Duration::from_secs(2));

        let goal = client
            .execute(
                &root,
                "thread-live",
                CodexGoalAction::Save {
                    objective: " Updated objective ".to_string(),
                    token_budget: Some(8000),
                },
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(goal.status, "paused");
        let input = fs::read_to_string(capture).unwrap();
        let set = input
            .lines()
            .find(|line| line.contains("\"method\":\"thread/goal/set\""))
            .unwrap();
        assert!(set.contains("\"objective\":\"Updated objective\""));
        assert!(set.contains("\"tokenBudget\":8000"));
        assert!(!set.contains("\"status\":"));
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn codex_goal_client_resume_accepts_every_official_resumable_status() {
        let _guard = process_test_guard().await;
        for status in [
            "paused",
            "blocked",
            "usageLimited",
            "budgetLimited",
            "complete",
        ] {
            let root = temp_dir(&format!("goal-resume-{status}"));
            let executable = root.join("codex");
            let capture = root.join("stdin.jsonl");
            let script = format!(
                r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'codex-cli 0.144.2'
  exit 0
fi
while IFS= read -r line; do
  printf '%s\n' "$line" >> '{}'
  case "$line" in
    *'"method":"initialize"'*)
      echo '{{"id":1,"result":{{"userAgent":"fake"}}}}'
      ;;
    *'"method":"thread/goal/get"'*)
      echo '{{"id":2,"result":{{"goal":{{"threadId":"thread-live","objective":"Resume me","status":"{}","tokenBudget":9000,"tokensUsed":10,"timeUsedSeconds":2,"createdAt":100,"updatedAt":200}}}}}}'
      ;;
    *'"method":"thread/goal/set"'*)
      echo '{{"id":3,"result":{{"goal":{{"threadId":"thread-live","objective":"Resume me","status":"active","tokenBudget":9000,"tokensUsed":10,"timeUsedSeconds":2,"createdAt":100,"updatedAt":201}}}}}}'
      ;;
  esac
done
"#,
                capture.display(),
                status
            );
            write_executable(&executable, &script);
            let client =
                CodexGoalClient::with_resolved_executable(executable, Duration::from_secs(2));

            let goal = client
                .execute(&root, "thread-live", CodexGoalAction::Resume)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(goal.status, "active", "{status}");
            let input = fs::read_to_string(&capture).unwrap();
            let set = input
                .lines()
                .find(|line| line.contains("\"method\":\"thread/goal/set\""))
                .unwrap();
            assert!(set.contains("\"status\":\"active\""), "{status}");
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[tokio::test]
    async fn codex_goal_client_clear_returns_official_empty_state() {
        let _guard = process_test_guard().await;
        let root = temp_dir("goal-clear");
        let executable = root.join("codex");
        let capture = root.join("stdin.jsonl");
        let script = format!(
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'codex-cli 0.144.2'
  exit 0
fi
while IFS= read -r line; do
  printf '%s\n' "$line" >> '{}'
  case "$line" in
    *'"method":"initialize"'*)
      echo '{{"id":1,"result":{{"userAgent":"fake"}}}}'
      ;;
    *'"method":"thread/goal/clear"'*)
      echo '{{"id":2,"result":{{"cleared":true}}}}'
      ;;
    *'"method":"thread/goal/get"'*)
      echo '{{"id":3,"result":{{"goal":null}}}}'
      ;;
  esac
done
"#,
            capture.display()
        );
        write_executable(&executable, &script);
        let client = CodexGoalClient::with_resolved_executable(executable, Duration::from_secs(2));

        let goal = client
            .execute(&root, "thread-live", CodexGoalAction::Clear)
            .await
            .unwrap();

        assert_eq!(goal, None);
        let input = fs::read_to_string(capture).unwrap();
        assert_eq!(input.matches("\"method\":\"thread/goal/clear\"").count(), 1);
        assert_eq!(input.matches("\"method\":\"thread/goal/get\"").count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn codex_goal_client_reports_eof_invalid_json_and_app_server_errors() {
        let _guard = process_test_guard().await;
        for (name, response, expected) in [
            ("eof", None, "closed before response 2"),
            ("invalid", Some("not-json"), "returned invalid JSON"),
            (
                "server-error",
                Some(r#"{"id":2,"error":{"code":-32602,"message":"unknown thread"}}"#),
                "Codex app-server request failed: unknown thread",
            ),
        ] {
            let root = temp_dir(&format!("goal-protocol-{name}"));
            let executable = root.join("codex");
            let response = response.unwrap_or_default();
            let script = format!(
                r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'codex-cli 0.144.2'
  exit 0
fi
while IFS= read -r line; do
  case "$line" in
    *'"method":"initialize"'*)
      echo '{{"id":1,"result":{{"userAgent":"fake"}}}}'
      ;;
    *'"method":"thread/goal/get"'*)
      if [ -n '{}' ]; then
        printf '%s\n' '{}'
      fi
      exit 0
      ;;
  esac
done
"#,
                response, response
            );
            write_executable(&executable, &script);
            let client =
                CodexGoalClient::with_resolved_executable(executable, Duration::from_secs(2));

            let error = client
                .execute(&root, "thread-live", CodexGoalAction::Get)
                .await
                .unwrap_err();

            assert!(error.to_string().contains(expected), "{name}: {error:#}");
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[tokio::test]
    async fn codex_goal_client_does_not_retry_timed_out_mutation() {
        let _guard = process_test_guard().await;
        let root = temp_dir("goal-mutation-timeout");
        let executable = root.join("codex");
        let capture = root.join("stdin.jsonl");
        let script = format!(
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'codex-cli 0.144.2'
  exit 0
fi
: > '{}'
while IFS= read -r line; do
  printf '%s\n' "$line" >> '{}'
  case "$line" in
    *'"method":"initialize"'*)
      echo '{{"id":1,"result":{{"userAgent":"fake"}}}}'
      ;;
    *'"method":"thread/goal/set"'*)
      :
      ;;
  esac
done
"#,
            capture.display(),
            capture.display()
        );
        write_executable(&executable, &script);
        let client = CodexGoalClient::with_resolved_executable(executable, Duration::from_secs(5));

        let error = client
            .execute(
                &root,
                "thread-live",
                CodexGoalAction::Save {
                    objective: "Do not retry".to_string(),
                    token_budget: None,
                },
            )
            .await
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("mutation timed out; the resulting state is unknown"));
        let input = fs::read_to_string(capture).unwrap();
        assert_eq!(input.matches("\"method\":\"thread/goal/set\"").count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn codex_goal_client_reaps_app_server_after_timeout() {
        let _guard = process_test_guard().await;
        let root = temp_dir("goal-timeout-reap");
        let executable = root.join("codex");
        let pid_file = root.join("app-server.pid");
        let script = format!(
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo 'codex-cli 0.144.2'
  exit 0
fi
echo $$ > '{}'
while IFS= read -r line; do
  :
done
"#,
            pid_file.display()
        );
        write_executable(&executable, &script);
        let client = CodexGoalClient::with_resolved_executable(executable, Duration::from_secs(2));

        let error = client
            .execute(&root, "thread-live", CodexGoalAction::Get)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("query timed out"));
        let pid = fs::read_to_string(&pid_file).unwrap();
        let status = StdCommand::new("/bin/kill")
            .arg("-0")
            .arg(pid.trim())
            .stdout(StdStdio::null())
            .stderr(StdStdio::null())
            .status()
            .unwrap();
        assert!(
            !status.success(),
            "timed-out app-server process was not reaped"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
