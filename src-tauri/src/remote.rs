#![allow(non_snake_case)]

use nexushub_core::services::{
    commands::{is_allowed_rpc_command, is_mutating_rpc_command},
    remote::{
        credentials::{CredentialAccess, CredentialAccessError},
        *,
    },
    system::{HostSurface, SystemCapabilitiesResponse, API_VERSION},
};
use reqwest::{header, redirect::Policy, Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

const KEYCHAIN_SERVICE: &str = "com.lich13.nexushub.remote";
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
const CREDENTIAL_WAIT: Duration = Duration::from_secs(15);

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Preferences {
    target: MachineTarget,
    base_url: Option<String>,
    revision: u64,
}

#[derive(Default)]
struct ConnectionState {
    prefs: Preferences,
    writes: usize,
    changing: bool,
    verified_revision: Option<u64>,
}

#[derive(Clone)]
pub struct RemoteConnections {
    state: Arc<Mutex<ConnectionState>>,
    path: PathBuf,
    http: Client,
    credentials: CredentialAccess,
}

fn safe_error(message: &'static str) -> String {
    message.into()
}

fn credential_access_error(error: CredentialAccessError) -> String {
    safe_error(match error {
        CredentialAccessError::TimedOut => "钥匙串未及时响应，请检查系统授权后重试",
        CredentialAccessError::WorkerUnavailable => "钥匙串操作失败，请重试",
    })
}

pub fn normalized_base_url(raw: &str) -> Result<String, String> {
    let url = Url::parse(raw.trim()).map_err(|_| safe_error("请输入有效的 HTTPS 地址"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || raw.contains('\\')
        || raw.chars().any(char::is_control)
    {
        return Err(safe_error(
            "地址必须使用 HTTPS，且不能包含凭据、查询参数或片段",
        ));
    }
    Ok(format!("{}/", url.as_str().trim_end_matches('/')))
}

fn credential(base_url: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, base_url).map_err(|_| safe_error("无法访问钥匙串"))
}

fn read_credential(entry: &keyring::Entry) -> Result<Option<String>, String> {
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(safe_error("无法读取钥匙串")),
    }
}

fn restore_credential(entry: &keyring::Entry, value: Option<&str>) -> Result<(), String> {
    match value {
        Some(value) => entry.set_password(value),
        None => entry.delete_credential().or_else(|error| {
            if matches!(error, keyring::Error::NoEntry) {
                Ok(())
            } else {
                Err(error)
            }
        }),
    }
    .map_err(|_| safe_error("无法更新钥匙串，请检查钥匙串权限"))
}

impl RemoteConnections {
    pub fn load(data_dir: &Path) -> Result<Self, String> {
        fs::create_dir_all(data_dir).map_err(|_| safe_error("无法创建连接配置目录"))?;
        let path = data_dir.join("remote-connection.json");
        let prefs = if path.exists() {
            let value: Preferences = serde_json::from_slice(
                &fs::read(&path).map_err(|_| safe_error("无法读取连接配置"))?,
            )
            .map_err(|_| safe_error("连接配置损坏"))?;
            if let Some(base) = &value.base_url {
                normalized_base_url(base)?;
            }
            value
        } else {
            Preferences::default()
        };
        let http = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|_| safe_error("无法初始化远程连接"))?;
        Ok(Self {
            state: Arc::new(Mutex::new(ConnectionState {
                prefs,
                ..Default::default()
            })),
            path,
            http,
            credentials: CredentialAccess::default(),
        })
    }

    fn view(&self) -> RemoteConnectionView {
        let state = self.state.lock().expect("connection state");
        RemoteConnectionView {
            target: state.prefs.target,
            revision: state.prefs.revision,
            base_url: state.prefs.base_url.clone(),
            configured: state.prefs.base_url.is_some(),
        }
    }

    fn persist(&self, prefs: &Preferences) -> Result<(), String> {
        let temp = self.path.with_extension("json.pending");
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| {
            let mut file = options.open(&temp)?;
            file.write_all(&serde_json::to_vec(prefs)?)?;
            file.sync_all()?;
            fs::rename(&temp, &self.path)?;
            Ok::<_, anyhow::Error>(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result.map_err(|_| safe_error("保存连接配置失败"))
    }

    fn begin_change(&self, revision: u64) -> Result<ChangeGuard, String> {
        let mut state = self.state.lock().expect("connection state");
        if state.prefs.revision != revision {
            return Err(safe_error("连接已变化，请重试"));
        }
        if state.writes > 0 || state.changing {
            return Err(safe_error("操作进行中，请稍后切换"));
        }
        state.changing = true;
        Ok(ChangeGuard(self.state.clone()))
    }

    fn require_current_remote(&self, revision: u64, base: &str) -> Result<(), String> {
        let current = self.state.lock().expect("connection state");
        if current.changing
            || current.prefs.revision != revision
            || current.prefs.target != MachineTarget::Remote
            || current.prefs.base_url.as_deref() != Some(base)
        {
            return Err(safe_error("连接已变化，操作已取消"));
        }
        Ok(())
    }

    async fn call(
        &self,
        base: &str,
        key: &str,
        command: &str,
        args: &Value,
    ) -> Result<Value, String> {
        if !is_allowed_rpc_command(command) {
            return Err(safe_error("不支持的远程操作"));
        }
        let mut key =
            header::HeaderValue::from_str(key).map_err(|_| safe_error("API Key 格式无效"))?;
        key.set_sensitive(true);
        let endpoint = format!("{base}api/rpc/{command}");
        let mut response = self
            .http
            .post(endpoint)
            .header("x-api-key", key)
            .json(args)
            .send()
            .await
            .map_err(|_| safe_error("无法连接腾讯云，请检查地址和网络"))?;
        let status = response.status();
        if status.is_redirection() {
            return Err(safe_error("服务器返回重定向，请填写最终 HTTPS 地址"));
        }
        if status == 401 || status == 403 {
            return Err(safe_error("API Key 无效或已撤销"));
        }
        if status == 429 {
            return Err(safe_error("鉴权请求过于频繁，请稍后重试"));
        }
        if !response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                v.split(';')
                    .next()
                    .is_some_and(|mime| mime.trim() == "application/json")
            })
        {
            return Err(safe_error("远程地址未返回 NexusHub API 数据"));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return Err(safe_error("远程响应超出大小限制"));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| safe_error("远程响应读取失败"))?
        {
            if bytes.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err(safe_error("远程响应超出大小限制"));
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| safe_error("远程响应格式无效"))?;
        if !status.is_success() {
            return Err(value
                .get("error")
                .and_then(Value::as_str)
                .map(nexushub_core::security::redact_output)
                .unwrap_or_else(|| format!("远程操作失败，HTTP {}", status.as_u16())));
        }
        Ok(value)
    }

    async fn verify(
        &self,
        request: &RemoteConnectionCredentials,
    ) -> Result<SystemCapabilitiesResponse, String> {
        let base = normalized_base_url(&request.base_url)?;
        if request.api_key.len() != 68
            || !request.api_key.starts_with("nhk_")
            || !request.api_key[4..].bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err(safe_error("API Key 格式无效"));
        }
        let value = self
            .call(&base, &request.api_key, "system.capabilities", &json!({}))
            .await?;
        compatible_capabilities(value)
    }

    // Both methods run as one serialized blocking transaction. Their caller's
    // ChangeGuard stays in that worker through commit or rollback, even if the
    // frontend stops awaiting the command.
    fn save_connection(
        &self,
        request: RemoteConnectionCredentials,
        base: String,
    ) -> Result<RemoteConnectionView, String> {
        let mut next = self.state.lock().expect("connection state").prefs.clone();
        let entry = credential(&base)?;
        let old_key = read_credential(&entry)?;
        let previous_entry = next
            .base_url
            .as_deref()
            .filter(|previous| *previous != base)
            .map(credential)
            .transpose()?;
        let previous_key = previous_entry
            .as_ref()
            .map(read_credential)
            .transpose()?
            .flatten();
        entry
            .set_password(&request.api_key)
            .map_err(|_| safe_error("无法保存 API Key 到钥匙串"))?;
        if let Some(previous) = &previous_entry {
            if let Err(error) = restore_credential(previous, None) {
                restore_credential(&entry, old_key.as_deref())?;
                return Err(error);
            }
        }
        next.base_url = Some(base);
        next.revision = next.revision.wrapping_add(1);
        if let Err(error) = self.persist(&next) {
            let restore_new = restore_credential(&entry, old_key.as_deref());
            let restore_previous = previous_entry
                .as_ref()
                .map(|previous| restore_credential(previous, previous_key.as_deref()))
                .transpose();
            restore_new?;
            restore_previous?;
            return Err(error);
        }
        self.state.lock().expect("connection state").prefs = next;
        Ok(self.view())
    }

    fn remove_connection(&self) -> Result<RemoteConnectionView, String> {
        let previous = self.state.lock().expect("connection state").prefs.clone();
        let entry = previous.base_url.as_deref().map(credential).transpose()?;
        let old_key = entry.as_ref().map(read_credential).transpose()?.flatten();
        if let Some(entry) = &entry {
            restore_credential(entry, None)?;
        }
        let next = Preferences {
            revision: previous.revision.wrapping_add(1),
            ..Default::default()
        };
        if let Err(error) = self.persist(&next) {
            if let (Some(entry), Some(key)) = (entry, old_key) {
                restore_credential(&entry, Some(&key))?;
            }
            return Err(error);
        }
        self.state.lock().expect("connection state").prefs = next;
        Ok(self.view())
    }
}

fn compatible_capabilities(value: Value) -> Result<SystemCapabilitiesResponse, String> {
    let capabilities: SystemCapabilitiesResponse =
        serde_json::from_value(value).map_err(|_| safe_error("远程 API 协议不兼容"))?;
    if capabilities.api_version != API_VERSION
        || capabilities.host_surface != HostSurface::LinuxServerApi
    {
        return Err(safe_error("远程 API 协议不兼容，请更新服务"));
    }
    Ok(capabilities)
}

struct ChangeGuard(Arc<Mutex<ConnectionState>>);
impl Drop for ChangeGuard {
    fn drop(&mut self) {
        self.0.lock().expect("connection state").changing = false;
    }
}
struct WriteGuard<'a>(&'a RemoteConnections, bool);
impl Drop for WriteGuard<'_> {
    fn drop(&mut self) {
        if self.1 {
            self.0.state.lock().expect("connection state").writes -= 1;
        }
    }
}

#[tauri::command(rename = "remote.get")]
pub fn remoteGet(state: tauri::State<'_, RemoteConnections>) -> RemoteConnectionView {
    state.view()
}

#[tauri::command(rename = "remote.verify")]
pub async fn remoteVerify(
    state: tauri::State<'_, RemoteConnections>,
    request: RemoteConnectionCredentials,
) -> Result<SystemCapabilitiesResponse, String> {
    let _guard = state.begin_change(request.revision)?;
    state.verify(&request).await
}

#[tauri::command(rename = "remote.save")]
pub async fn remoteSave(
    state: tauri::State<'_, RemoteConnections>,
    request: RemoteConnectionCredentials,
) -> Result<RemoteConnectionView, String> {
    let guard = state.begin_change(request.revision)?;
    state.verify(&request).await?;
    let base = normalized_base_url(&request.base_url)?;
    let credentials = state.credentials.clone();
    let connection = state.inner().clone();
    credentials
        .transaction(CREDENTIAL_WAIT, move || {
            let _guard = guard;
            connection.save_connection(request, base)
        })
        .await
        .map_err(credential_access_error)?
}

#[tauri::command(rename = "remote.remove")]
pub async fn remoteRemove(
    state: tauri::State<'_, RemoteConnections>,
    request: RemoteRevisionRequest,
) -> Result<RemoteConnectionView, String> {
    let guard = state.begin_change(request.revision)?;
    let credentials = state.credentials.clone();
    let connection = state.inner().clone();
    credentials
        .transaction(CREDENTIAL_WAIT, move || {
            let _guard = guard;
            connection.remove_connection()
        })
        .await
        .map_err(credential_access_error)?
}

#[tauri::command(rename = "remote.select")]
pub fn remoteSelect(
    state: tauri::State<'_, RemoteConnections>,
    request: RemoteSelectionRequest,
) -> Result<RemoteConnectionView, String> {
    let _guard = state.begin_change(request.revision)?;
    let mut next = state.state.lock().expect("connection state").prefs.clone();
    if request.target == MachineTarget::Remote && next.base_url.is_none() {
        return Err(safe_error("请先设置远程连接"));
    }
    next.target = request.target;
    next.revision = next.revision.wrapping_add(1);
    state.persist(&next)?;
    state.state.lock().expect("connection state").prefs = next;
    Ok(state.view())
}

#[tauri::command(rename = "remote.invoke")]
pub async fn remoteInvoke(
    state: tauri::State<'_, RemoteConnections>,
    request: RemoteInvokeRequest,
) -> Result<RemoteInvokeResponse, String> {
    if !is_allowed_rpc_command(&request.command) {
        return Err(safe_error("不支持的远程操作"));
    }
    let write = is_mutating_rpc_command(&request.command);
    let base = {
        let mut current = state.state.lock().expect("connection state");
        if current.changing
            || current.prefs.revision != request.revision
            || current.prefs.target != MachineTarget::Remote
        {
            return Err(safe_error("连接已变化，操作已取消"));
        }
        let base = current
            .prefs
            .base_url
            .clone()
            .ok_or_else(|| safe_error("请先设置远程连接"))?;
        if write {
            current.writes += 1;
        }
        base
    };
    let _guard = WriteGuard(&state, write);
    let credential_base = base.clone();
    let key = state
        .credentials
        .read(CREDENTIAL_WAIT, move || {
            credential(&credential_base)?
                .get_password()
                .map_err(|_| safe_error("无法读取 API Key，请重新保存远程连接"))
        })
        .await
        .map_err(credential_access_error)??;
    state.require_current_remote(request.revision, &base)?;
    let needs_validation = state
        .state
        .lock()
        .expect("connection state")
        .verified_revision
        != Some(request.revision);
    if needs_validation && request.command != "system.capabilities" {
        compatible_capabilities(
            state
                .call(&base, &key, "system.capabilities", &json!({}))
                .await?,
        )?;
    }
    state.require_current_remote(request.revision, &base)?;
    let value = state
        .call(&base, &key, &request.command, &request.args)
        .await?;
    if request.command == "system.capabilities" {
        compatible_capabilities(value.clone())?;
    }
    let mut current = state.state.lock().expect("connection state");
    if current.changing || current.prefs.revision != request.revision {
        return Err(safe_error("连接已变化，已丢弃旧响应"));
    }
    if needs_validation {
        current.verified_revision = Some(request.revision);
    }
    Ok(value)
}

#[cfg(test)]
#[path = "remote/credential_tests.rs"]
mod credential_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Read, net::TcpListener, thread};

    fn response_server(
        status: &str,
        headers: &str,
        body: String,
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n",
            body.len()
        );
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let size = socket.read(&mut chunk).unwrap();
                assert!(size > 0);
                request.extend_from_slice(&chunk[..size]);
                if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|value| value.parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            let _ = socket
                .write_all(head.as_bytes())
                .and_then(|_| socket.write_all(body.as_bytes()));
            String::from_utf8(request).unwrap()
        });
        (base, server)
    }

    #[test]
    fn remote_bridge_never_follows_redirect_or_exposes_key_in_errors() {
        let destination = TcpListener::bind("127.0.0.1:0").unwrap();
        destination.set_nonblocking(true).unwrap();
        let (base, server) = response_server(
            "302 Found",
            &format!(
                "Location: http://{}/stolen\r\n",
                destination.local_addr().unwrap()
            ),
            String::new(),
        );
        let dir = tempfile::tempdir().unwrap();
        let state = RemoteConnections::load(dir.path()).unwrap();
        let key = format!("nhk_{}", "a".repeat(64));
        let error = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(state.call(&base, &key, "system.capabilities", &json!({})))
            .unwrap_err();
        assert!(error.contains("重定向"));
        assert!(!error.contains(&key));
        assert!(server
            .join()
            .unwrap()
            .contains(&format!("x-api-key: {key}")));
        assert_eq!(
            destination.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn remote_bridge_covers_twenty_mib_attachments_and_bounds_response_memory() {
        let dir = tempfile::tempdir().unwrap();
        let state = RemoteConnections::load(dir.path()).unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let image_size = (20 * 1024 * 1024usize).div_ceil(3) * 4;
        let (base, server) = response_server(
            "200 OK",
            "Content-Type: application/json\r\n",
            format!("{{\"data\":\"{}\"}}", "A".repeat(image_size)),
        );
        let value = runtime
            .block_on(state.call(&base, "example", "sessions.attachmentRead", &json!({})))
            .unwrap();
        assert_eq!(value["data"].as_str().unwrap().len(), image_size);
        server.join().unwrap();
        let (base, server) = response_server(
            "200 OK",
            "Content-Type: application/json\r\n",
            " ".repeat(MAX_RESPONSE_BYTES + 1),
        );
        assert!(runtime
            .block_on(state.call(&base, "example", "threads.detail", &json!({})))
            .unwrap_err()
            .contains("大小限制"));
        server.join().unwrap();
        assert!(runtime
            .block_on(state.call("invalid", "example", "remote.invoke", &json!({})))
            .unwrap_err()
            .contains("不支持"));
    }

    #[test]
    fn connection_preferences_preserve_target_without_storing_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let state = RemoteConnections::load(dir.path()).unwrap();
        assert_eq!(state.view().target, MachineTarget::Local);
        state
            .persist(&Preferences {
                target: MachineTarget::Remote,
                base_url: Some("https://api.example.com/nexus/".into()),
                revision: 8,
            })
            .unwrap();
        let loaded = RemoteConnections::load(dir.path()).unwrap();
        assert_eq!(loaded.view().target, MachineTarget::Remote);
        assert_eq!(loaded.view().revision, 8);
        let saved: Value = serde_json::from_slice(&fs::read(&state.path).unwrap()).unwrap();
        assert_eq!(saved.as_object().unwrap().len(), 3);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&state.path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let invalid = json!({ "api_version": API_VERSION + 1, "host_surface": "linux_server_api", "capabilities": {} });
        assert!(compatible_capabilities(invalid).is_err());
    }
    #[test]
    fn connection_addresses_require_https_without_credentials_or_navigation_targets() {
        assert_eq!(
            normalized_base_url("https://api.example.com/nexus").unwrap(),
            "https://api.example.com/nexus/"
        );
        for invalid in [
            "http://api.example.com",
            "https://user.example.com:secret@example.com/",
            "https://example.com/?token=example",
            "file:///tmp/demo",
            "https://example.com/#part",
        ] {
            assert!(normalized_base_url(invalid).is_err());
        }
    }
    #[test]
    fn target_changes_reject_stale_revisions_and_pending_writes() {
        let dir = tempfile::tempdir().unwrap();
        let state = RemoteConnections::load(dir.path()).unwrap();
        assert!(state.begin_change(1).is_err());
        state.state.lock().unwrap().writes = 1;
        assert!(state.begin_change(0).is_err());
        state.state.lock().unwrap().writes = 0;
        let guard = state.begin_change(0).unwrap();
        assert!(state.begin_change(0).is_err());
        drop(guard);
        assert!(state.begin_change(0).is_ok());
    }
}
