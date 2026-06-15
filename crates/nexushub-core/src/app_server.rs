use crate::{
    codex::resolve_codex_paths,
    config::{BridgeTransport, Config},
    uploads::{prompt_with_attachment_context, PreparedAttachment},
};
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    process::{Command, Stdio},
    time::Duration,
};
use tokio::task;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeActionResult {
    pub bridge: bool,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub job_id: Option<String>,
    pub fallback: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeTurnOptions {
    pub message: String,
    #[serde(default)]
    pub attachments: Vec<PreparedAttachment>,
    pub model: Option<String>,
    pub service_tier: Option<String>,
    pub reasoning_effort: Option<String>,
    pub cwd: Option<String>,
    pub permission_profile: Option<String>,
    pub approval_policy: Option<String>,
    pub sandbox_mode: Option<String>,
    pub network_access: Option<bool>,
    pub collaboration_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeThreadEvent {
    pub event_id: String,
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub kind: String,
    pub item_id: Option<String>,
    pub text: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Clone)]
pub struct AppServerBridge {
    config: Config,
}

impl AppServerBridge {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn enabled(&self) -> bool {
        self.config.codex.bridge_enabled
    }

    pub async fn start_thread(&self, options: BridgeTurnOptions) -> Result<BridgeActionResult> {
        let config = self.config.clone();
        task::spawn_blocking(move || {
            let client = BlockingClient::new(&config)?;
            let result = client.request("thread/start", thread_start_params(&config, &options))?;
            let thread_id = result
                .pointer("/thread/id")
                .and_then(Value::as_str)
                .map(str::to_string);
            let Some(thread_id_value) = thread_id.clone() else {
                return Err(anyhow!("thread/start response did not include thread.id"));
            };
            let turn_result =
                client.request("turn/start", turn_start_params(&thread_id_value, &options))?;
            let turn_id = turn_result
                .pointer("/turn/id")
                .and_then(Value::as_str)
                .map(str::to_string);
            Ok(BridgeActionResult {
                bridge: true,
                thread_id,
                turn_id,
                job_id: None,
                fallback: false,
                message: None,
            })
        })
        .await?
    }

    pub async fn send_turn(
        &self,
        thread_id: String,
        options: BridgeTurnOptions,
    ) -> Result<BridgeActionResult> {
        let config = self.config.clone();
        task::spawn_blocking(move || {
            let client = BlockingClient::new(&config)?;
            let _ = client.request("thread/resume", thread_resume_params(&thread_id, &options))?;
            let turn_result =
                client.request("turn/start", turn_start_params(&thread_id, &options))?;
            let turn_id = turn_result
                .pointer("/turn/id")
                .and_then(Value::as_str)
                .map(str::to_string);
            Ok(BridgeActionResult {
                bridge: true,
                thread_id: Some(thread_id),
                turn_id,
                job_id: None,
                fallback: false,
                message: None,
            })
        })
        .await?
    }

    pub async fn steer_turn(
        &self,
        thread_id: String,
        expected_turn_id: String,
        options: BridgeTurnOptions,
    ) -> Result<BridgeActionResult> {
        let config = self.config.clone();
        task::spawn_blocking(move || {
            let client = BlockingClient::new(&config)?;
            let result = client.request(
                "turn/steer",
                turn_steer_params(&thread_id, &expected_turn_id, &options),
            )?;
            Ok(BridgeActionResult {
                bridge: true,
                thread_id: Some(thread_id),
                turn_id: result
                    .pointer("/turn/id")
                    .or_else(|| result.get("turnId"))
                    .or_else(|| result.get("turn_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or(Some(expected_turn_id)),
                job_id: None,
                fallback: false,
                message: Some("follow-up steered into the active Codex turn".to_string()),
            })
        })
        .await?
    }

    pub async fn stop_turn(&self, thread_id: String, turn_id: String) -> Result<()> {
        let config = self.config.clone();
        task::spawn_blocking(move || {
            let client = BlockingClient::new(&config)?;
            client.request(
                "turn/interrupt",
                json!({ "threadId": thread_id, "turnId": turn_id }),
            )?;
            Ok(())
        })
        .await?
    }

    pub async fn archive_thread(&self, thread_id: String) -> Result<()> {
        self.simple_thread_request("thread/archive", thread_id)
            .await
    }

    pub async fn unarchive_thread(&self, thread_id: String) -> Result<()> {
        self.simple_thread_request("thread/unarchive", thread_id)
            .await
    }

    pub async fn rename_thread(&self, thread_id: String, name: String) -> Result<()> {
        let config = self.config.clone();
        task::spawn_blocking(move || {
            let client = BlockingClient::new(&config)?;
            client.request(
                "thread/name/set",
                json!({ "threadId": thread_id, "name": name }),
            )?;
            Ok(())
        })
        .await?
    }

    pub async fn fork_thread(&self, thread_id: String) -> Result<BridgeActionResult> {
        let config = self.config.clone();
        task::spawn_blocking(move || {
            let client = BlockingClient::new(&config)?;
            let result = client.request(
                "thread/fork",
                json!({ "threadId": thread_id, "persistExtendedHistory": false }),
            )?;
            Ok(BridgeActionResult {
                bridge: true,
                thread_id: result
                    .pointer("/thread/id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                turn_id: None,
                job_id: None,
                fallback: false,
                message: None,
            })
        })
        .await?
    }

    pub async fn thread_list(
        &self,
        limit: usize,
        archived: Option<bool>,
        q: Option<&str>,
    ) -> Result<Value> {
        let config = self.config.clone();
        let search_term = q
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        task::spawn_blocking(move || {
            let client = BlockingClient::new(&config)?;
            let mut params = serde_json::Map::new();
            params.insert("limit".to_string(), json!(limit.clamp(1, 500)));
            if let Some(archived) = archived {
                params.insert("archived".to_string(), Value::Bool(archived));
            }
            if let Some(search_term) = search_term {
                params.insert("searchTerm".to_string(), Value::String(search_term));
            }
            add_interactive_source_filter(&mut params);
            client.request("thread/list", Value::Object(params))
        })
        .await?
    }

    pub async fn thread_read(&self, thread_id: String, include_turns: bool) -> Result<Value> {
        let config = self.config.clone();
        task::spawn_blocking(move || {
            let client = BlockingClient::new(&config)?;
            client.request(
                "thread/read",
                json!({ "threadId": thread_id, "includeTurns": include_turns }),
            )
        })
        .await?
    }

    pub async fn goal_get(&self, thread_id: String) -> Result<Value> {
        self.thread_value_request("thread/goal/get", thread_id, json!({}))
            .await
    }

    pub async fn goal_set(
        &self,
        thread_id: String,
        objective: String,
        status: Option<String>,
        token_budget: Option<u64>,
    ) -> Result<Value> {
        let mut params = serde_json::Map::new();
        params.insert("threadId".to_string(), Value::String(thread_id));
        params.insert("objective".to_string(), Value::String(objective));
        if let Some(status) = empty_as_null(status.as_deref()) {
            params.insert("status".to_string(), status);
        }
        if let Some(token_budget) = token_budget {
            params.insert("tokenBudget".to_string(), json!(token_budget));
        }
        self.value_request("thread/goal/set", Value::Object(params))
            .await
    }

    pub async fn goal_clear(&self, thread_id: String) -> Result<Value> {
        self.thread_value_request("thread/goal/clear", thread_id, json!({}))
            .await
    }

    pub async fn goal_resume(&self, thread_id: String) -> Result<Value> {
        self.thread_value_request("thread/goal/resume", thread_id, json!({}))
            .await
    }

    pub async fn model_list(&self) -> Result<Value> {
        self.value_request("model/list", json!({"includeHidden": false}))
            .await
    }

    pub async fn permission_profile_list(&self, cwd: Option<String>) -> Result<Value> {
        let mut params = serde_json::Map::new();
        if let Some(cwd) = empty_as_null(cwd.as_deref()) {
            params.insert("cwd".to_string(), cwd);
        }
        self.value_request("permissionProfile/list", Value::Object(params))
            .await
    }

    pub async fn config_read(&self, cwd: Option<String>) -> Result<Value> {
        let mut params = serde_json::Map::new();
        if let Some(cwd) = empty_as_null(cwd.as_deref()) {
            params.insert("cwd".to_string(), cwd);
        }
        self.value_request("config/read", Value::Object(params))
            .await
    }

    async fn thread_value_request(
        &self,
        method: &'static str,
        thread_id: String,
        extra: Value,
    ) -> Result<Value> {
        let mut params = match extra {
            Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        params.insert("threadId".to_string(), Value::String(thread_id));
        self.value_request(method, Value::Object(params)).await
    }

    async fn value_request(&self, method: &'static str, params: Value) -> Result<Value> {
        let config = self.config.clone();
        task::spawn_blocking(move || {
            let client = BlockingClient::new(&config)?;
            client.request(method, params)
        })
        .await?
    }

    async fn simple_thread_request(&self, method: &'static str, thread_id: String) -> Result<()> {
        let config = self.config.clone();
        task::spawn_blocking(move || {
            let client = BlockingClient::new(&config)?;
            client.request(method, json!({ "threadId": thread_id }))?;
            Ok(())
        })
        .await?
    }

    pub async fn health_check(&self) -> Result<()> {
        let config = self.config.clone();
        task::spawn_blocking(move || {
            let client = BlockingClient::new(&config)?;
            client.request("thread/list", json!({"limit": 1, "useStateDbOnly": true}))?;
            Ok(())
        })
        .await?
    }
}

fn thread_start_params(config: &Config, options: &BridgeTurnOptions) -> Value {
    let mut params = serde_json::Map::new();
    if let Some(model) = empty_as_null(options.model.as_deref()) {
        params.insert("model".to_string(), model);
    }
    if let Some(service_tier) = service_tier_value(options.service_tier.as_deref()) {
        params.insert("serviceTier".to_string(), service_tier);
    }
    params.insert(
        "cwd".to_string(),
        empty_as_null(options.cwd.as_deref())
            .unwrap_or_else(|| Value::String(config.codex.workspace.display().to_string())),
    );
    if let Some(approval) = approval_policy_value(options.approval_policy.as_deref()) {
        params.insert("approvalPolicy".to_string(), approval);
    }
    if !insert_permission_profile(&mut params, options.permission_profile.as_deref()) {
        if let Some(sandbox) = thread_sandbox_mode_value(options.sandbox_mode.as_deref()) {
            params.insert("sandbox".to_string(), sandbox);
        }
    }
    params.insert("experimentalRawEvents".to_string(), Value::Bool(true));
    params.insert("persistExtendedHistory".to_string(), Value::Bool(false));
    Value::Object(params)
}

fn thread_resume_params(thread_id: &str, options: &BridgeTurnOptions) -> Value {
    let mut params = serde_json::Map::new();
    params.insert("threadId".to_string(), Value::String(thread_id.to_string()));
    params.insert(
        "model".to_string(),
        empty_as_null(options.model.as_deref()).unwrap_or(Value::Null),
    );
    params.insert(
        "serviceTier".to_string(),
        service_tier_value(options.service_tier.as_deref()).unwrap_or(Value::Null),
    );
    params.insert(
        "cwd".to_string(),
        empty_as_null(options.cwd.as_deref()).unwrap_or(Value::Null),
    );
    params.insert(
        "approvalPolicy".to_string(),
        approval_policy_value(options.approval_policy.as_deref()).unwrap_or(Value::Null),
    );
    if !insert_permission_profile(&mut params, options.permission_profile.as_deref()) {
        params.insert(
            "sandbox".to_string(),
            thread_sandbox_mode_value(options.sandbox_mode.as_deref()).unwrap_or(Value::Null),
        );
    }
    params.insert("persistExtendedHistory".to_string(), Value::Bool(false));
    Value::Object(params)
}

fn turn_start_params(thread_id: &str, options: &BridgeTurnOptions) -> Value {
    let mut params = serde_json::Map::new();
    params.insert("threadId".to_string(), Value::String(thread_id.to_string()));
    params.insert("input".to_string(), turn_input_items(options));
    if let Some(cwd) = empty_as_null(options.cwd.as_deref()) {
        params.insert("cwd".to_string(), cwd);
    }
    if let Some(model) = empty_as_null(options.model.as_deref()) {
        params.insert("model".to_string(), model);
    }
    if let Some(service_tier) = service_tier_value(options.service_tier.as_deref()) {
        params.insert("serviceTier".to_string(), service_tier);
    }
    if let Some(effort) = empty_as_null(options.reasoning_effort.as_deref()) {
        params.insert("effort".to_string(), effort);
    }
    if let Some(mode) = collaboration_mode_value(options.collaboration_mode.as_deref()) {
        params.insert("collaborationMode".to_string(), mode);
    }
    if let Some(approval) = approval_policy_value(options.approval_policy.as_deref()) {
        params.insert("approvalPolicy".to_string(), approval);
    }
    if !insert_permission_profile(&mut params, options.permission_profile.as_deref()) {
        if let Some(sandbox) = sandbox_policy_value(
            options.sandbox_mode.as_deref(),
            options.cwd.as_deref(),
            options.network_access,
        ) {
            params.insert("sandboxPolicy".to_string(), sandbox);
        }
    }
    Value::Object(params)
}

fn turn_steer_params(
    thread_id: &str,
    expected_turn_id: &str,
    options: &BridgeTurnOptions,
) -> Value {
    json!({
        "threadId": thread_id,
        "expectedTurnId": expected_turn_id,
        "input": turn_input_items(options)
    })
}

fn turn_input_items(options: &BridgeTurnOptions) -> Value {
    let mut items = Vec::new();
    items.push(json!({
        "type": "text",
        "text": prompt_with_attachment_context(&options.message, &options.attachments),
        "text_elements": []
    }));
    for attachment in &options.attachments {
        if let Some(path) = &attachment.local_image_path {
            items.push(json!({
                "type": "localImage",
                "path": path.display().to_string()
            }));
        }
    }
    Value::Array(items)
}

fn add_interactive_source_filter(params: &mut serde_json::Map<String, Value>) {
    params.insert("sourceKinds".to_string(), json!(["cli", "vscode"]));
    params.insert("modelProviders".to_string(), json!([]));
}

fn collaboration_mode_value(value: Option<&str>) -> Option<Value> {
    empty_as_null(value).and_then(|value| {
        let Value::String(text) = value else {
            return None;
        };
        Some(json!({
            "id": text,
            "settings": {
                "developer_instructions": Value::Null
            }
        }))
    })
}

fn insert_permission_profile(
    params: &mut serde_json::Map<String, Value>,
    value: Option<&str>,
) -> bool {
    let Some(Value::String(profile)) = empty_as_null(value) else {
        return false;
    };
    params.insert("permissions".to_string(), Value::String(profile));
    true
}

fn approval_policy_value(value: Option<&str>) -> Option<Value> {
    empty_as_null(value).and_then(|value| {
        let Value::String(text) = value else {
            return None;
        };
        match text.as_str() {
            "untrusted" | "on-failure" | "on-request" | "never" => Some(Value::String(text)),
            _ => None,
        }
    })
}

fn service_tier_value(value: Option<&str>) -> Option<Value> {
    empty_as_null(value).and_then(|value| {
        let Value::String(text) = value else {
            return None;
        };
        match text.as_str() {
            "priority" | "default" => Some(Value::String(text)),
            "fast" => Some(Value::String("priority".to_string())),
            _ => None,
        }
    })
}

fn thread_sandbox_mode_value(value: Option<&str>) -> Option<Value> {
    empty_as_null(value).and_then(|value| {
        let Value::String(text) = value else {
            return None;
        };
        match text.as_str() {
            "read-only" | "workspace-write" | "danger-full-access" => Some(Value::String(text)),
            _ => None,
        }
    })
}

fn sandbox_policy_value(
    value: Option<&str>,
    cwd: Option<&str>,
    network_access: Option<bool>,
) -> Option<Value> {
    let network = network_access.unwrap_or(true);
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some("danger-full-access") => Some(json!({"type": "dangerFullAccess"})),
        Some("read-only") => Some(json!({"type": "readOnly", "networkAccess": network})),
        Some("workspace-write") => {
            let root = cwd
                .map(str::trim)
                .filter(|value| value.starts_with('/'))
                .unwrap_or("/");
            Some(json!({
                "type": "workspaceWrite",
                "writableRoots": [root],
                "networkAccess": network,
                "excludeTmpdirEnvVar": false,
                "excludeSlashTmp": false
            }))
        }
        _ => None,
    }
}

fn empty_as_null(value: Option<&str>) -> Option<Value> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| Value::String(value.to_string()))
}

struct BlockingClient {
    config: Config,
    transport: BridgeTransport,
}

impl BlockingClient {
    fn new(config: &Config) -> Result<Self> {
        if !config.codex.bridge_enabled {
            anyhow::bail!("app-server bridge disabled");
        }
        Ok(Self {
            config: config.clone(),
            transport: config.codex.bridge_transport.clone(),
        })
    }

    fn request(&self, method: &str, params: Value) -> Result<Value> {
        let request_id = if method == "initialize" { "1" } else { "2" };
        let request = json!({
            "id": request_id,
            "method": method,
            "params": params
        });
        let messages = if method == "initialize" {
            vec![request]
        } else {
            vec![
                initialize_request(),
                json!({"method":"initialized"}),
                request,
            ]
        };
        self.exchange(&messages, Some(request_id))
            .with_context(|| format!("app-server request {method} failed"))
    }

    fn exchange(&self, messages: &[Value], wait_for_id: Option<&str>) -> Result<Value> {
        match self.transport {
            BridgeTransport::Websocket => self.exchange_websocket(messages, wait_for_id),
            BridgeTransport::JsonLine | BridgeTransport::Lsp => {
                self.exchange_proxy(messages, wait_for_id)
            }
        }
    }

    fn exchange_proxy(&self, messages: &[Value], wait_for_id: Option<&str>) -> Result<Value> {
        let resolved = resolve_codex_paths(
            &self.config.codex.home,
            self.config.codex.app_server_socket.as_deref(),
        );
        let socket = resolved
            .app_server_socket
            .as_ref()
            .context("codex.app_server_socket is not configured")?;
        let mut child = Command::new("sudo")
            .args(["-n", "env"])
            .arg(format!("CODEX_HOME={}", resolved.home.display()))
            .arg("codex")
            .args(["app-server", "proxy", "--sock"])
            .arg(socket)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("spawn codex app-server proxy")?;
        {
            let mut stdin = child.stdin.take().context("proxy stdin closed")?;
            for message in messages {
                write_message(&mut stdin, &self.transport, message)?;
            }
        }
        let result = if let Some(id) = wait_for_id {
            let stdout = child.stdout.take().context("proxy stdout closed")?;
            match self.transport {
                BridgeTransport::JsonLine => read_json_line_response(stdout, id),
                BridgeTransport::Lsp => read_lsp_response(stdout, id),
                BridgeTransport::Websocket => unreachable!("websocket transport is handled above"),
            }
        } else {
            Ok(Value::Null)
        };
        let _ = child.kill();
        let _ = child.wait();
        result
    }

    fn exchange_websocket(&self, messages: &[Value], wait_for_id: Option<&str>) -> Result<Value> {
        let resolved = resolve_codex_paths(
            &self.config.codex.home,
            self.config.codex.app_server_socket.as_deref(),
        );
        let socket = resolved
            .app_server_socket
            .as_ref()
            .context("codex.app_server_socket is not configured")?;
        let timeout = Duration::from_secs(self.config.codex.bridge_timeout_seconds.max(1));
        let mut stream = UnixStream::connect(socket)
            .with_context(|| format!("connect app-server socket {}", socket.display()))?;
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        websocket_handshake(&mut stream)?;
        for message in messages {
            write_websocket_text(&mut stream, message)?;
        }
        let Some(id) = wait_for_id else {
            return Ok(Value::Null);
        };
        loop {
            let text = read_websocket_text(&mut stream)?;
            if text.trim().is_empty() {
                continue;
            }
            let value: Value = serde_json::from_str(&text)
                .with_context(|| format!("parse app-server websocket message: {text}"))?;
            if let Some(result) = response_result(&value, id)? {
                return Ok(result);
            }
        }
    }
}

fn initialize_request() -> Value {
    json!({
        "id": "1",
        "method": "initialize",
        "params": {
            "clientInfo": {
                "name": "nexushub",
                "title": "NexusHub",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {
                "experimentalApi": true,
                "requestAttestation": false
            }
        }
    })
}

fn write_message(
    writer: &mut impl Write,
    transport: &BridgeTransport,
    value: &Value,
) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    match transport {
        BridgeTransport::JsonLine => {
            writer.write_all(&body)?;
            writer.write_all(b"\n")?;
        }
        BridgeTransport::Lsp => {
            write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
            writer.write_all(&body)?;
        }
        BridgeTransport::Websocket => {
            anyhow::bail!("websocket transport does not use raw proxy writes");
        }
    }
    writer.flush()?;
    Ok(())
}

fn read_json_line_response<R: Read>(reader: R, id: &str) -> Result<Value> {
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            anyhow::bail!("proxy closed before response");
        }
        let value: Value = serde_json::from_str(line.trim())?;
        if let Some(result) = response_result(&value, id)? {
            return Ok(result);
        }
    }
}

fn read_lsp_response<R: Read>(mut reader: R, id: &str) -> Result<Value> {
    loop {
        let mut headers = Vec::new();
        let mut byte = [0_u8; 1];
        while !headers.ends_with(b"\r\n\r\n") {
            if reader.read(&mut byte)? == 0 {
                anyhow::bail!("proxy closed before response");
            }
            headers.push(byte[0]);
        }
        let headers_text = String::from_utf8_lossy(&headers);
        let Some(length) = headers_text.lines().find_map(|line| {
            line.strip_prefix("Content-Length:")
                .and_then(|value| value.trim().parse::<usize>().ok())
        }) else {
            anyhow::bail!("missing Content-Length");
        };
        let mut body = vec![0_u8; length];
        reader.read_exact(&mut body)?;
        let value: Value = serde_json::from_slice(&body)?;
        if let Some(result) = response_result(&value, id)? {
            return Ok(result);
        }
    }
}

fn response_result(value: &Value, id: &str) -> Result<Option<Value>> {
    if !response_id_matches(value.get("id"), id) {
        return Ok(None);
    }
    if let Some(error) = value.get("error") {
        return Err(anyhow!("app-server error: {error}"));
    }
    Ok(Some(value.get("result").cloned().unwrap_or(Value::Null)))
}

fn response_id_matches(value: Option<&Value>, id: &str) -> bool {
    value
        .and_then(Value::as_str)
        .map(|value| value == id)
        .unwrap_or(false)
        || value
            .and_then(Value::as_i64)
            .map(|value| value.to_string() == id)
            .unwrap_or(false)
}

fn websocket_handshake(stream: &mut UnixStream) -> Result<()> {
    let mut nonce = [0_u8; 16];
    rand::thread_rng().fill_bytes(&mut nonce);
    let key = general_purpose::STANDARD.encode(nonce);
    let request = format!(
        "GET / HTTP/1.1\r\n\
         Host: localhost\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n"
    );
    stream.write_all(request.as_bytes())?;
    stream.flush()?;

    let mut response = Vec::new();
    let mut byte = [0_u8; 1];
    while !response.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte)? == 0 {
            anyhow::bail!("app-server socket closed during websocket handshake");
        }
        response.push(byte[0]);
        if response.len() > 8192 {
            anyhow::bail!("websocket handshake response exceeded 8KiB");
        }
    }
    let response_text = String::from_utf8_lossy(&response);
    if !response_text.starts_with("HTTP/1.1 101") {
        anyhow::bail!("websocket handshake failed: {}", response_text.trim());
    }
    Ok(())
}

fn write_websocket_text(stream: &mut UnixStream, value: &Value) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    let mut frame = Vec::with_capacity(body.len() + 14);
    frame.push(0x81);
    let len = body.len();
    if len < 126 {
        frame.push(0x80 | len as u8);
    } else if len <= u16::MAX as usize {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(0x80 | 127);
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }
    let mut mask = [0_u8; 4];
    rand::thread_rng().fill_bytes(&mut mask);
    frame.extend_from_slice(&mask);
    frame.extend(
        body.iter()
            .enumerate()
            .map(|(index, byte)| byte ^ mask[index % 4]),
    );
    stream.write_all(&frame)?;
    stream.flush()?;
    Ok(())
}

fn read_websocket_text(stream: &mut UnixStream) -> Result<String> {
    loop {
        let mut header = [0_u8; 2];
        stream.read_exact(&mut header)?;
        let opcode = header[0] & 0x0f;
        let masked = header[1] & 0x80 != 0;
        let mut len = (header[1] & 0x7f) as u64;
        if len == 126 {
            let mut extended = [0_u8; 2];
            stream.read_exact(&mut extended)?;
            len = u16::from_be_bytes(extended) as u64;
        } else if len == 127 {
            let mut extended = [0_u8; 8];
            stream.read_exact(&mut extended)?;
            len = u64::from_be_bytes(extended);
        }
        let mut mask = [0_u8; 4];
        if masked {
            stream.read_exact(&mut mask)?;
        }
        let mut payload = vec![0_u8; len as usize];
        stream.read_exact(&mut payload)?;
        if masked {
            for (index, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[index % 4];
            }
        }
        match opcode {
            0x1 => return String::from_utf8(payload).context("decode websocket text frame"),
            0x8 => anyhow::bail!("app-server websocket closed"),
            0x9 => write_websocket_pong(stream, &payload)?,
            0xa => continue,
            _ => continue,
        }
    }
}

fn write_websocket_pong(stream: &mut UnixStream, payload: &[u8]) -> Result<()> {
    let mut frame = Vec::with_capacity(payload.len() + 14);
    frame.push(0x8a);
    if payload.len() < 126 {
        frame.push(0x80 | payload.len() as u8);
    } else {
        anyhow::bail!("websocket ping payload too large");
    }
    let mut mask = [0_u8; 4];
    rand::thread_rng().fill_bytes(&mut mask);
    frame.extend_from_slice(&mask);
    frame.extend(
        payload
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ mask[index % 4]),
    );
    stream.write_all(&frame)?;
    stream.flush()?;
    Ok(())
}

pub fn event_from_notification(value: &Value, fallback_id: usize) -> Option<BridgeThreadEvent> {
    let method = value.get("method").and_then(Value::as_str)?;
    let params = value.get("params").cloned().unwrap_or(Value::Null);
    let thread_id = params
        .get("threadId")
        .and_then(Value::as_str)
        .or_else(|| params.pointer("/thread/id").and_then(Value::as_str))?
        .to_string();
    let turn_id = params
        .get("turnId")
        .and_then(Value::as_str)
        .or_else(|| params.pointer("/turn/id").and_then(Value::as_str))
        .map(str::to_string);
    let item_id = params
        .get("itemId")
        .and_then(Value::as_str)
        .or_else(|| params.pointer("/item/id").and_then(Value::as_str))
        .map(str::to_string);
    let text = params
        .get("delta")
        .and_then(Value::as_str)
        .or_else(|| params.pointer("/item/text").and_then(Value::as_str))
        .or_else(|| params.pointer("/error/message").and_then(Value::as_str))
        .map(str::to_string);
    Some(BridgeThreadEvent {
        event_id: format!("{fallback_id}"),
        thread_id,
        turn_id,
        kind: method.to_string(),
        item_id,
        text,
        payload: params,
    })
}

pub fn collapse_notifications(values: &[Value], thread_id: &str) -> Vec<BridgeThreadEvent> {
    values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| event_from_notification(value, index))
        .filter(|event| event.thread_id == thread_id)
        .collect()
}

pub fn active_turn_id_from_events(events: &[BridgeThreadEvent]) -> Option<String> {
    let mut active: HashMap<String, bool> = HashMap::new();
    let mut last_active: Option<String> = None;
    for event in events {
        let Some(turn_id) = &event.turn_id else {
            continue;
        };
        match event.kind.as_str() {
            "turn/started" => {
                active.insert(turn_id.clone(), true);
                last_active = Some(turn_id.clone());
            }
            "turn/completed" | "error" => {
                active.insert(turn_id.clone(), false);
                if last_active.as_deref() == Some(turn_id) {
                    last_active = None;
                }
            }
            _ => {}
        }
    }
    if last_active
        .as_ref()
        .is_some_and(|turn_id| active.get(turn_id).copied().unwrap_or(false))
    {
        return last_active;
    }
    active
        .into_iter()
        .find_map(|(turn_id, is_active)| is_active.then_some(turn_id))
}

#[cfg(test)]
mod tests {
    use super::{
        active_turn_id_from_events, add_interactive_source_filter, event_from_notification,
        thread_resume_params, thread_start_params, turn_start_params, turn_steer_params,
        BridgeTurnOptions,
    };
    use crate::config::Config;
    use crate::uploads::{PreparedAttachment, UploadKind};
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn builds_turn_start_text_input() {
        let options = BridgeTurnOptions {
            message: "hello".to_string(),
            attachments: Vec::new(),
            model: Some("gpt-5.5".to_string()),
            service_tier: Some("priority".to_string()),
            reasoning_effort: Some("xhigh".to_string()),
            cwd: Some("/tmp".to_string()),
            permission_profile: None,
            approval_policy: Some("never".to_string()),
            sandbox_mode: Some("workspace-write".to_string()),
            network_access: Some(true),
            collaboration_mode: None,
        };
        let params = turn_start_params("thread-a", &options);
        assert_eq!(params["threadId"], "thread-a");
        assert_eq!(params["input"][0]["type"], "text");
        assert_eq!(params["input"][0]["text"], "hello");
        assert_eq!(params["cwd"], "/tmp");
        assert_eq!(params["model"], "gpt-5.5");
        assert_eq!(params["serviceTier"], "priority");
        assert_eq!(params["effort"], "xhigh");
        assert_eq!(params["approvalPolicy"], "never");
        assert_eq!(params["sandboxPolicy"]["type"], "workspaceWrite");
        assert_eq!(params["sandboxPolicy"]["writableRoots"][0], "/tmp");
        assert_eq!(params["sandboxPolicy"]["networkAccess"], true);
    }

    #[test]
    fn turn_start_prefers_permission_profile_over_sandbox_policy() {
        let options = BridgeTurnOptions {
            message: "hello".to_string(),
            attachments: Vec::new(),
            model: Some("gpt-5.5".to_string()),
            service_tier: None,
            reasoning_effort: Some("xhigh".to_string()),
            cwd: Some("/tmp".to_string()),
            permission_profile: Some(":workspace".to_string()),
            approval_policy: Some("never".to_string()),
            sandbox_mode: Some("workspace-write".to_string()),
            network_access: Some(true),
            collaboration_mode: Some("plan".to_string()),
        };
        let params = turn_start_params("thread-a", &options);

        assert_eq!(params["permissions"], ":workspace");
        assert_eq!(params["collaborationMode"]["id"], "plan");
        assert!(params["collaborationMode"]["settings"]["developer_instructions"].is_null());
        assert!(params.get("sandboxPolicy").is_none());
    }

    #[test]
    fn sandbox_policy_defaults_network_access_to_true() {
        let options = BridgeTurnOptions {
            message: "hello".to_string(),
            attachments: Vec::new(),
            model: None,
            service_tier: None,
            reasoning_effort: None,
            cwd: Some("/tmp/work".to_string()),
            permission_profile: None,
            approval_policy: Some("on-request".to_string()),
            sandbox_mode: Some("workspace-write".to_string()),
            network_access: None,
            collaboration_mode: None,
        };

        let params = turn_start_params("thread-a", &options);

        assert_eq!(params["sandboxPolicy"]["type"], "workspaceWrite");
        assert_eq!(params["sandboxPolicy"]["networkAccess"], true);
    }

    #[test]
    fn thread_list_params_filter_to_interactive_sources() {
        let mut params = serde_json::Map::new();

        add_interactive_source_filter(&mut params);

        assert_eq!(params.get("sourceKinds"), Some(&json!(["cli", "vscode"])));
        assert_eq!(params.get("modelProviders"), Some(&json!([])));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn fast_legacy_service_tier_maps_to_priority() {
        let options = BridgeTurnOptions {
            message: "hello".to_string(),
            attachments: Vec::new(),
            model: Some("gpt-5.5".to_string()),
            service_tier: Some("fast".to_string()),
            reasoning_effort: None,
            cwd: None,
            permission_profile: None,
            approval_policy: None,
            sandbox_mode: None,
            network_access: None,
            collaboration_mode: None,
        };

        let params = turn_start_params("thread-a", &options);

        assert_eq!(params["serviceTier"], "priority");
    }

    #[test]
    fn thread_start_and_resume_params_include_service_tier() {
        let options = BridgeTurnOptions {
            message: "hello".to_string(),
            attachments: Vec::new(),
            model: Some("gpt-5.5".to_string()),
            service_tier: Some("priority".to_string()),
            reasoning_effort: Some("xhigh".to_string()),
            cwd: Some("/tmp/workspace".to_string()),
            permission_profile: None,
            approval_policy: Some("never".to_string()),
            sandbox_mode: Some("workspace-write".to_string()),
            network_access: Some(true),
            collaboration_mode: None,
        };
        let config = Config::default();

        let start = thread_start_params(&config, &options);
        let resume = thread_resume_params("thread-a", &options);

        assert_eq!(start["model"], "gpt-5.5");
        assert_eq!(start["serviceTier"], "priority");
        assert_eq!(start["cwd"], "/tmp/workspace");
        assert_eq!(resume["threadId"], "thread-a");
        assert_eq!(resume["model"], "gpt-5.5");
        assert_eq!(resume["serviceTier"], "priority");
        assert_eq!(resume["cwd"], "/tmp/workspace");
    }

    #[test]
    fn builds_turn_steer_params_with_expected_turn_id() {
        let options = BridgeTurnOptions {
            message: "continue".to_string(),
            attachments: Vec::new(),
            model: Some("gpt-5.5".to_string()),
            service_tier: Some("priority".to_string()),
            reasoning_effort: Some("xhigh".to_string()),
            cwd: Some("/tmp".to_string()),
            permission_profile: None,
            approval_policy: Some("never".to_string()),
            sandbox_mode: Some("danger-full-access".to_string()),
            network_access: Some(true),
            collaboration_mode: None,
        };

        let params = turn_steer_params("thread-a", "turn-live", &options);

        assert_eq!(params["threadId"], "thread-a");
        assert_eq!(params["expectedTurnId"], "turn-live");
        assert_eq!(params["input"][0]["type"], "text");
        assert_eq!(params["input"][0]["text"], "continue");
        assert!(params.get("model").is_none());
        assert!(params.get("serviceTier").is_none());
    }

    #[test]
    fn turn_start_and_steer_include_attachment_context_and_local_image() {
        let attachments = vec![
            PreparedAttachment {
                id: "upload-text".to_string(),
                name: "plan.md".to_string(),
                mime: "text/markdown".to_string(),
                size: 12,
                sha256: "sha-text".to_string(),
                kind: UploadKind::Markdown,
                text: Some("# Plan\n\n- ship it".to_string()),
                local_image_path: None,
                local_file_path: None,
                truncated: false,
            },
            PreparedAttachment {
                id: "upload-image".to_string(),
                name: "screen.png".to_string(),
                mime: "image/png".to_string(),
                size: 9,
                sha256: "sha-image".to_string(),
                kind: UploadKind::Image,
                text: None,
                local_image_path: Some(PathBuf::from("/tmp/screen.png")),
                local_file_path: None,
                truncated: false,
            },
        ];
        let options = BridgeTurnOptions {
            message: "review these".to_string(),
            attachments,
            model: Some("gpt-5.5".to_string()),
            service_tier: Some("priority".to_string()),
            reasoning_effort: Some("xhigh".to_string()),
            cwd: Some("/tmp".to_string()),
            permission_profile: None,
            approval_policy: None,
            sandbox_mode: None,
            network_access: None,
            collaboration_mode: None,
        };

        let start = turn_start_params("thread-a", &options);
        let steer = turn_steer_params("thread-a", "turn-live", &options);

        for params in [start, steer] {
            assert_eq!(params["input"][0]["type"], "text");
            assert!(params["input"][0]["text"]
                .as_str()
                .unwrap()
                .contains("review these"));
            assert!(params["input"][0]["text"]
                .as_str()
                .unwrap()
                .contains("### 附件: plan.md"));
            assert!(params["input"][0]["text"]
                .as_str()
                .unwrap()
                .contains("# Plan"));
            assert_eq!(params["input"][1]["type"], "localImage");
            assert_eq!(params["input"][1]["path"], "/tmp/screen.png");
        }
    }

    #[test]
    fn maps_agent_delta_notification() {
        let value = json!({
            "method": "item/agentMessage/delta",
            "params": {"threadId":"t1","turnId":"turn1","itemId":"i1","delta":"hi"}
        });
        let event = event_from_notification(&value, 7).unwrap();
        assert_eq!(event.thread_id, "t1");
        assert_eq!(event.turn_id.as_deref(), Some("turn1"));
        assert_eq!(event.item_id.as_deref(), Some("i1"));
        assert_eq!(event.text.as_deref(), Some("hi"));
        assert_eq!(event.kind, "item/agentMessage/delta");
    }

    #[test]
    fn active_turn_clears_on_completion() {
        let started = event_from_notification(
            &json!({"method":"turn/started","params":{"threadId":"t1","turn":{"id":"turn1"}}}),
            0,
        )
        .unwrap();
        assert_eq!(
            active_turn_id_from_events(std::slice::from_ref(&started)).as_deref(),
            Some("turn1")
        );
        let completed = event_from_notification(
            &json!({"method":"turn/completed","params":{"threadId":"t1","turn":{"id":"turn1"}}}),
            1,
        )
        .unwrap();
        assert_eq!(active_turn_id_from_events(&[started, completed]), None);
    }
}
