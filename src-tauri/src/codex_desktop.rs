//! The only desktop mutation entrypoint. It is not registered with webd or the
//! remote bridge. Paths and credentials originate in Rust/native choosers.
use crate::{overview::DesktopState, remote::RemoteConnections};
use nexushub_core::{
    codex::{
        self,
        desktop::{helper::DesktopHelper, worker::DesktopWorker, DesktopError, DesktopSession},
    },
    crypto::SecretBox,
    db::desktop_submissions::{DesktopLedger, DesktopWorkerLease},
    resource_limits::CREDENTIAL_WAIT,
    services::{codex_desktop::*, remote::credentials::CredentialAccess},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tauri::{ipc::Channel, State};
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex;

mod catalogs;
mod selection;
use selection::{GrantValue, Grants};

const KEY_SERVICE: &str = "com.lich13.nexushub.codex-desktop";
const KEY_ACCOUNT: &str = "submission-encryption-v1";
const MAX_WORKERS: usize = 4;

pub struct CodexDesktopBridge {
    runtime: Mutex<Option<Arc<BridgeRuntime>>>,
    credentials: CredentialAccess,
    intent: AtomicU64,
    grants: Mutex<Grants>,
    catalogs: Mutex<catalogs::CatalogCache>,
}

struct BridgeRuntime {
    _lease: DesktopWorkerLease,
    ledger: DesktopLedger,
    codex_home: PathBuf,
    source: String,
    workers: Mutex<HashMap<String, DesktopWorker>>,
    selection: Mutex<Option<Selection>>,
}

struct Selection {
    id: String,
    worker: DesktopWorker,
    listener: tokio::task::AbortHandle,
}

impl CodexDesktopBridge {
    pub fn new(credentials: CredentialAccess) -> Self {
        Self {
            runtime: Mutex::new(None),
            credentials,
            intent: AtomicU64::new(0),
            grants: Mutex::new(Grants::default()),
            catalogs: Mutex::new(catalogs::CatalogCache::default()),
        }
    }

    async fn runtime(&self, state: &DesktopState) -> Result<Arc<BridgeRuntime>, String> {
        let mut slot = self.runtime.lock().await;
        if let Some(runtime) = slot.as_ref() {
            return Ok(runtime.clone());
        }
        let db = state.db.clone();
        let home = state.codex_paths().home;
        let (lease, home, existing) = nexushub_core::read_tasks::spawn_blocking(move || {
            let lease = DesktopLedger::lock_database(&db)
                .map_err(|_| "另一个 NexusHub 正在使用本机发送队列")?;
            let existing =
                DesktopLedger::has_encrypted_messages(&db).map_err(|_| "无法读取本机发送队列")?;
            let home = home.canonicalize().map_err(|_| "无法定位本机 Codex")?;
            Ok::<_, String>((lease, home, existing))
        })
        .await
        .map_err(|_| "无法初始化本机发送队列")??;
        // The cross-process lease moves INTO the credential transaction: a
        // cancelled caller cannot release it while Keychain is still writing.
        let (crypto, lease) = self
            .credentials
            .transaction(CREDENTIAL_WAIT, move || {
                let entry = keyring::Entry::new(KEY_SERVICE, KEY_ACCOUNT)
                    .map_err(|_| "无法访问发送队列钥匙串")?;
                let material = match entry.get_password() {
                    Ok(value) => value,
                    Err(keyring::Error::NoEntry) if !existing => {
                        let value = SecretBox::random_key_material();
                        entry
                            .set_password(&value)
                            .map_err(|_| "无法保存发送队列密钥")?;
                        value
                    }
                    Err(keyring::Error::NoEntry) => {
                        return Err("发送队列密钥缺失，已保留待发送记录".to_owned())
                    }
                    Err(_) => return Err("无法读取发送队列密钥，请检查系统授权".to_owned()),
                };
                let crypto =
                    SecretBox::from_key_material(&material).map_err(|_| "发送队列密钥不可用")?;
                Ok::<_, String>((crypto, lease))
            })
            .await
            .map_err(|_| "钥匙串未及时响应，请检查系统授权")??;
        let source = nexushub_core::security::hash_token(&home.to_string_lossy());
        let ledger = DesktopLedger::new(state.db.clone(), crypto);
        let recovery = ledger.clone();
        let recovery_source = source.clone();
        nexushub_core::read_tasks::spawn_blocking(move || recovery.recover(&recovery_source))
            .await
            .map_err(|_| "无法恢复本机发送队列")?
            .map_err(|_| "无法恢复本机发送队列")?;
        let runtime = Arc::new(BridgeRuntime {
            _lease: lease,
            ledger,
            codex_home: home,
            source,
            workers: Mutex::new(HashMap::new()),
            selection: Mutex::new(None),
        });
        *slot = Some(runtime.clone());
        Ok(runtime)
    }
}

impl BridgeRuntime {
    async fn attach(&self, thread_id: &str) -> Result<DesktopWorker, String> {
        let mut workers = self.workers.lock().await;
        workers.retain(|_, worker| !worker.is_closed());
        if let Some(worker) = workers.get(thread_id) {
            return Ok(worker.clone());
        }
        if workers.len() >= MAX_WORKERS {
            let stale = workers
                .iter()
                .find(|(_, worker)| {
                    let current = worker.current();
                    current.runtime == "idle" && current.queue.is_empty()
                })
                .map(|(key, _)| key.clone());
            if let Some(stale) = stale {
                workers.remove(&stale);
            } else {
                return Err("本机协作线程已达到上限，请稍后重试".into());
            }
        }
        let session = DesktopSession::attach(&self.codex_home, thread_id)
            .await
            .map_err(desktop_error)?;
        session.state().check_writable().map_err(desktop_error)?;
        let worker = DesktopWorker::spawn_checked(
            session,
            self.ledger.clone(),
            self.source.clone(),
            codex::CodexPaths::new(self.codex_home.clone()),
        )
        .await
        .map_err(desktop_error)?;
        workers.insert(thread_id.into(), worker.clone());
        Ok(worker)
    }

    async fn worker(&self, session_id: &str) -> Result<DesktopWorker, String> {
        self.selection
            .lock()
            .await
            .as_ref()
            .filter(|selection| selection.id == session_id && !selection.worker.is_closed())
            .map(|selection| selection.worker.clone())
            .ok_or_else(|| "桌面连接已变化，请重新连接".into())
    }
}

pub(crate) fn desktop_error(error: DesktopError) -> String {
    match error {
        DesktopError::OwnerUnavailable => "请先在 Codex 中打开此线程，再重新连接",
        DesktopError::Unavailable => "Codex 桌面当前不可用",
        DesktopError::ReadOnly => "此线程仅支持阅读",
        DesktopError::Busy => "线程正在处理其他请求，请稍后重试",
        DesktopError::InvalidInput => "输入内容无效或超出限制",
        DesktopError::InvalidIdentity | DesktopError::UnsafeSocket => {
            "无法确认 Codex 桌面或线程身份"
        }
        DesktopError::Timeout => "Codex 桌面未及时响应",
        DesktopError::RequestExpired => "该请求已失效，请查看最新状态",
        DesktopError::Rejected => "Codex 未接受此操作",
        DesktopError::OutcomeUnknown => "发送结果未确认，请先在 Codex 中核对，勿重复发送",
        DesktopError::Disconnected => "Codex 桌面连接已断开，请重新连接",
        DesktopError::InvalidFrame
        | DesktopError::UnsupportedProtocol
        | DesktopError::OutOfSync
        | DesktopError::FrameTooLarge => {
            "当前 Codex 桌面状态无法安全处理，请重新连接或在 Codex 中继续"
        }
    }
    .into()
}

async fn verify_thread(state: &DesktopState, thread_id: &str) -> Result<(), String> {
    let paths = state.codex_paths();
    let id = thread_id.to_owned();
    nexushub_core::read_tasks::spawn_blocking(move || {
        let summary = codex::local_thread_summary(&paths, &id)
            .map_err(|_| "无法核对本机线程")?
            .ok_or("此线程当前不可用于桌面协作")?;
        if matches!(summary.status, codex::ThreadStatus::Archived) {
            return Err("归档线程仅支持阅读");
        }
        Ok(())
    })
    .await
    .map_err(|_| "无法核对本机线程")?
    .map_err(str::to_owned)
}

#[tauri::command(rename = "codex.desktop.attach")]
pub async fn attach(
    state: State<'_, DesktopState>,
    remote: State<'_, RemoteConnections>,
    bridge: State<'_, CodexDesktopBridge>,
    request: DesktopThreadRequest,
    on_event: Channel<DesktopView>,
) -> Result<DesktopView, String> {
    let intent = bridge.intent.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
    remote.require_local(request.revision)?;
    verify_thread(&state, &request.thread_id).await?;
    let runtime = bridge.runtime(&state).await?;
    remote.require_local(request.revision)?;
    let worker = runtime.attach(&request.thread_id).await?;
    remote.require_local(request.revision)?;
    if bridge.intent.load(Ordering::SeqCst) != intent {
        return Err("所选线程已变化，已丢弃旧连接".into());
    }
    let token = nexushub_core::security::hash_token(&SecretBox::random_key_material());
    let mut updates = worker.subscribe();
    // Read the initial value from this subscription so a change between the
    // snapshot and subscription cannot be missed while native Codex is idle.
    let mut initial = updates.borrow_and_update().clone();
    initial.session_id = token.clone();
    let connection = remote.inner().clone();
    let event_token = token.clone();
    let listener = tokio::spawn(async move {
        while updates.changed().await.is_ok() {
            // Coalesce token patches into at most ten full projected views per
            // second; the native state actor keeps processing every revision.
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if connection.require_local(request.revision).is_err() {
                break;
            }
            let mut value = updates.borrow_and_update().clone();
            value.session_id = event_token.clone();
            let closed = !value.connected;
            if on_event.send(value).is_err() || closed {
                break;
            }
        }
    })
    .abort_handle();
    let mut selection = runtime.selection.lock().await;
    if bridge.intent.load(Ordering::SeqCst) != intent {
        listener.abort();
        return Err("所选线程已变化，已丢弃旧连接".into());
    }
    if let Some(previous) = selection.replace(Selection {
        id: token,
        worker,
        listener,
    }) {
        previous.listener.abort();
    }
    Ok(initial)
}

#[tauri::command(rename = "codex.desktop.action")]
pub async fn action(
    state: State<'_, DesktopState>,
    remote: State<'_, RemoteConnections>,
    bridge: State<'_, CodexDesktopBridge>,
    request: DesktopActionRequest,
) -> Result<DesktopView, String> {
    let _guard = remote.begin_local_write(request.revision)?;
    let runtime = bridge.runtime(&state).await?;
    remote.require_local(request.revision)?;
    let worker = runtime.worker(&request.session_id).await?;
    let thread = worker.current().thread_id;
    verify_thread(&state, &thread).await?;
    let handles = match &request.action {
        DesktopAction::Send { references, .. } | DesktopAction::Steer { references, .. } => {
            references.as_slice()
        }
        _ => &[],
    };
    if handles.len() > 32 {
        return Err("引用数量超出限制".into());
    }
    let references = {
        let grants = bridge.grants.lock().await;
        handles
            .iter()
            .map(
                |handle| match grants.resolve(request.revision, Some(&thread), handle)? {
                    GrantValue::Reference(reference) => Ok(reference),
                    _ => Err("所选引用不可用于发送".into()),
                },
            )
            .collect::<Result<Vec<_>, String>>()?
    };
    let references = nexushub_core::read_tasks::spawn_blocking(move || {
        references
            .into_iter()
            .map(|reference| reference.capture())
            .collect::<Result<Vec<_>, _>>()
    })
    .await
    .map_err(|_| "无法核对所选引用")?
    .map_err(desktop_error)?;
    if let DesktopAction::Settings {
        model,
        reasoning_effort,
        mode,
    } = &request.action
    {
        let cwd = worker.project_directory().ok_or("无法确认当前工作目录")?;
        let helper = DesktopHelper::installed(state.codex_paths().home)
            .await
            .map_err(desktop_error)?;
        let mut catalogs = bridge.catalogs.lock().await;
        catalogs
            .read(
                &helper,
                cwd,
                &thread,
                &DesktopCatalogRequest {
                    revision: request.revision,
                    session_id: Some(request.session_id.clone()),
                    project_handle: None,
                    kind: "models".into(),
                    query: String::new(),
                    cursor: None,
                },
            )
            .await?;
        catalogs.validate_settings(
            request.revision,
            &thread,
            model,
            reasoning_effort.as_deref(),
            *mode,
        )?;
    }
    remote.require_local(request.revision)?;
    let worker = runtime.worker(&request.session_id).await?;
    let mut view = worker
        .act_with_references(request.action, references)
        .await
        .map_err(desktop_error)?;
    view.session_id = request.session_id;
    Ok(view)
}

#[tauri::command(rename = "codex.desktop.detach")]
pub async fn detach(
    bridge: State<'_, CodexDesktopBridge>,
    request: DesktopSessionRequest,
) -> Result<DesktopAck, String> {
    // Releasing an old callback is always safe, including after machine change.
    // Only the matching subscription is revoked; queued user work remains owned
    // by its original local worker and cannot migrate to a remote machine.
    if let Some(runtime) = bridge.runtime.lock().await.as_ref() {
        let mut selection = runtime.selection.lock().await;
        if selection
            .as_ref()
            .is_some_and(|selection| selection.id == request.session_id)
        {
            if let Some(selection) = selection.take() {
                selection.listener.abort();
            }
            bridge.intent.fetch_add(1, Ordering::SeqCst);
        }
    }
    Ok(DesktopAck { ok: true })
}

#[tauri::command(rename = "codex.desktop.open")]
pub async fn open(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
    remote: State<'_, RemoteConnections>,
    request: DesktopThreadRequest,
) -> Result<DesktopAck, String> {
    let _guard = remote.begin_local_write(request.revision)?;
    verify_thread(&state, &request.thread_id).await?;
    app.opener()
        .open_url(
            format!("codex://threads/{}", request.thread_id),
            None::<&str>,
        )
        .map_err(|_| "无法打开 Codex 桌面")?;
    Ok(DesktopAck { ok: true })
}

#[tauri::command(rename = "codex.desktop.choose")]
pub async fn choose(
    remote: State<'_, RemoteConnections>,
    bridge: State<'_, CodexDesktopBridge>,
    request: DesktopChooseRequest,
) -> Result<DesktopChoice, String> {
    let _guard = remote.begin_local_write(request.revision)?;
    let directory = match request.kind.as_str() {
        "project" => true,
        "file" => false,
        _ => return Err("选择类型无效".into()),
    };
    let context = if directory {
        None
    } else {
        let runtime = bridge
            .runtime
            .lock()
            .await
            .clone()
            .ok_or("请先连接 Codex 桌面")?;
        let id = request.session_id.as_deref().ok_or("请先连接 Codex 桌面")?;
        let worker = runtime.worker(id).await?;
        Some((runtime, id.to_owned(), worker.current().thread_id))
    };
    let Some(path) = selection::choose(directory).await? else {
        return Ok(DesktopChoice { item: None });
    };
    if let Some((runtime, id, _)) = &context {
        runtime.worker(id).await?;
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("所选目录")
        .to_owned();
    let value = nexushub_core::read_tasks::spawn_blocking(move || {
        if directory {
            selection::project(path)
        } else {
            nexushub_core::codex::desktop::references::NativeReference::selected_file(path)
                .map(GrantValue::Reference)
                .map_err(desktop_error)
        }
    })
    .await
    .map_err(|_| "无法读取所选项目")??;
    if let Some((runtime, id, _)) = &context {
        runtime.worker(id).await?;
    }
    remote.require_local(request.revision)?;
    let kind = match &value {
        GrantValue::Reference(reference) => reference.kind(),
        _ => "project",
    }
    .to_owned();
    let scope = context.map(|(_, _, thread)| thread);
    let handle = bridge
        .grants
        .lock()
        .await
        .insert(request.revision, scope, value);
    Ok(DesktopChoice {
        item: Some(DesktopChosenItem { handle, name, kind }),
    })
}

#[tauri::command(rename = "codex.desktop.create")]
pub async fn create(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
    remote: State<'_, RemoteConnections>,
    bridge: State<'_, CodexDesktopBridge>,
    request: DesktopCreateRequest,
) -> Result<DesktopCreatedThread, String> {
    let _guard = remote.begin_local_write(request.revision)?;
    let value =
        bridge
            .grants
            .lock()
            .await
            .resolve(request.revision, None, &request.project_handle)?;
    let cwd = nexushub_core::read_tasks::spawn_blocking(move || selection::project_path(value))
        .await
        .map_err(|_| "无法读取所选目录")??;
    let helper = DesktopHelper::installed(state.codex_paths().home)
        .await
        .map_err(desktop_error)?;
    let thread_id = helper.create_empty(&cwd).await.map_err(desktop_error)?;
    // Creation is already committed by the native helper. Failure to open a
    // window must not turn this into a retryable creation failure.
    let _ = app
        .opener()
        .open_url(format!("codex://threads/{thread_id}"), None::<&str>);
    Ok(DesktopCreatedThread { thread_id })
}

#[tauri::command(rename = "codex.desktop.catalog")]
pub async fn catalog(
    state: State<'_, DesktopState>,
    remote: State<'_, RemoteConnections>,
    bridge: State<'_, CodexDesktopBridge>,
    request: DesktopCatalogRequest,
) -> Result<DesktopCatalog, String> {
    remote.require_local(request.revision)?;
    if request.query.len() > 256 {
        return Err("搜索内容超出限制".into());
    }
    let runtime = bridge
        .runtime
        .lock()
        .await
        .clone()
        .ok_or("请先连接 Codex 桌面")?;
    let session_id = request.session_id.as_deref().ok_or("请先连接 Codex 桌面")?;
    let worker = runtime.worker(session_id).await?;
    let cwd = worker
        .project_directory()
        .ok_or("无法确认当前工作目录")?
        .to_owned();
    let thread = worker.current().thread_id;
    let helper = DesktopHelper::installed(state.codex_paths().home)
        .await
        .map_err(desktop_error)?;
    // Catalog I/O must not retain the grants lock used by sends and selection.
    let page = bridge
        .catalogs
        .lock()
        .await
        .read(&helper, &cwd, &thread, &request)
        .await?;
    remote.require_local(request.revision)?;
    runtime.worker(session_id).await?;
    Ok(page.project(request.revision, &thread, &mut *bridge.grants.lock().await))
}
