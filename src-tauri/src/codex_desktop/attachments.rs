//! OS attachment ingress. A receiver is scoped to one visible main-window
//! composer, machine revision and desktop session. Channels never carry paths.
use super::*;
use nexushub_core::codex::desktop::references::NativeReference;
use tauri::{Manager, WebviewWindow};

#[derive(Clone)]
pub(super) struct Receiver {
    id: String,
    revision: u64,
    intent: u64,
    session: String,
    thread: String,
    bounds: DesktopAttachmentBounds,
    handles: Vec<String>,
    channel: Channel<DesktopAttachmentEvent>,
}
impl Receiver {
    pub(super) fn session(&self) -> &str {
        &self.session
    }
    pub(super) fn id(&self) -> &str {
        &self.id
    }
}

fn visible(window: &WebviewWindow) -> bool {
    window.label() == "main"
        && window.is_visible().unwrap_or(false)
        && window.is_focused().unwrap_or(false)
}

pub(super) fn valid_bounds(bounds: &DesktopAttachmentBounds) -> bool {
    [bounds.x, bounds.y, bounds.width, bounds.height]
        .iter()
        .all(|value| value.is_finite() && *value >= 0.0 && *value <= 65536.0)
        && bounds.width > 0.0
        && bounds.height > 0.0
}

fn contains(bounds: &DesktopAttachmentBounds, x: f64, y: f64) -> bool {
    x.is_finite()
        && y.is_finite()
        && x >= bounds.x
        && y >= bounds.y
        && x < bounds.x + bounds.width
        && y < bounds.y + bounds.height
}

async fn context(
    window: &WebviewWindow,
    bridge: &CodexDesktopBridge,
    remote: &RemoteConnections,
    runtime: &BridgeRuntime,
    receiver: &Receiver,
) -> Result<(), String> {
    if !visible(window) || bridge.intent.load(Ordering::SeqCst) != receiver.intent {
        return Err("附件接收区已变化，请重新连接".into());
    }
    remote.require_local(receiver.revision)?;
    let view = runtime.worker(&receiver.session).await?.current();
    if !view.connected || !view.writable || view.thread_id != receiver.thread {
        return Err("当前线程仅支持阅读".into());
    }
    if !bridge
        .receiver
        .lock()
        .await
        .as_ref()
        .is_some_and(|current| current.id == receiver.id)
    {
        return Err("附件接收区已变化，请重试".into());
    }
    Ok(())
}

async fn references(
    bridge: &CodexDesktopBridge,
    receiver: &Receiver,
    handles: &[String],
) -> Result<Vec<NativeReference>, String> {
    if handles.len() > 32 {
        return Err("引用数量超出限制".into());
    }
    let grants = bridge.grants.lock().await;
    handles
        .iter()
        .map(
            |handle| match grants.resolve(receiver.revision, Some(&receiver.thread), handle)? {
                GrantValue::Reference(reference) => Ok(reference),
                _ => Err("所选引用不可用于发送".into()),
            },
        )
        .collect()
}

async fn release(runtime: &BridgeRuntime, references: Vec<NativeReference>) -> Result<(), String> {
    let store = runtime.attachments.clone();
    nexushub_core::read_tasks::spawn_blocking(move || {
        store
            .lock()
            .map_err(|_| "无法释放粘贴附件")?
            .release(&references)
    })
    .await
    .map_err(|_| "无法释放粘贴附件")?
    .map_err(str::to_owned)
}

pub(super) async fn unbind(bridge: &CodexDesktopBridge, id: &str) {
    let mut current = bridge.receiver.lock().await;
    if current.as_ref().is_some_and(|current| current.id == id) {
        current.take();
    }
    // Hiding/reconnecting preserves the visible draft for its existing 30 min
    // grant lifetime. Explicit chip removal releases; expired drafts are GC'd.
}

pub(super) async fn handle(
    window: WebviewWindow,
    state: &DesktopState,
    remote: &RemoteConnections,
    bridge: &CodexDesktopBridge,
    request: DesktopAttachmentRequest,
    channel: Channel<DesktopAttachmentEvent>,
) -> Result<DesktopAttachmentResult, String> {
    if window.label() != "main" {
        return Err("附件仅可从主窗口添加".into());
    }
    if let DesktopAttachmentAction::Unbind { binding_id } = &request.action {
        unbind(bridge, binding_id).await;
        return Ok(DesktopAttachmentResult {
            binding_id: binding_id.clone(),
        });
    }
    let _guard = remote.begin_local_write(request.revision)?;
    let runtime = bridge
        .runtime
        .lock()
        .await
        .clone()
        .ok_or("请先连接 Codex 桌面")?;
    let worker = runtime.worker(&request.session_id).await?;
    let view = worker.current();
    if !visible(&window) || !view.connected || !view.writable {
        return Err("请在当前协作输入框中添加附件".into());
    }
    if matches!(
        &request.action,
        DesktopAttachmentAction::Bind { .. } | DesktopAttachmentAction::Paste { .. }
    ) {
        verify_thread(state, &view.thread_id).await?;
    }
    if let DesktopAttachmentAction::Bind { bounds, handles } = request.action {
        if !valid_bounds(&bounds) {
            return Err("附件接收区域无效".into());
        }
        let receiver = Receiver {
            id: nexushub_core::security::hash_token(&SecretBox::random_key_material()),
            revision: request.revision,
            intent: bridge.intent.load(Ordering::SeqCst),
            session: request.session_id.clone(),
            thread: view.thread_id,
            bounds,
            handles,
            channel,
        };
        references(bridge, &receiver, &receiver.handles).await?;
        remote.require_local(request.revision)?;
        runtime.worker(&request.session_id).await?;
        let id = receiver.id.clone();
        *bridge.receiver.lock().await = Some(receiver);
        return Ok(DesktopAttachmentResult { binding_id: id });
    }
    let receiver = bridge
        .receiver
        .lock()
        .await
        .clone()
        .ok_or("附件接收区已关闭")?;
    let binding_id = match &request.action {
        DesktopAttachmentAction::Bounds { binding_id, .. }
        | DesktopAttachmentAction::Paste { binding_id }
        | DesktopAttachmentAction::Release { binding_id, .. }
        | DesktopAttachmentAction::Unbind { binding_id } => binding_id,
        DesktopAttachmentAction::Bind { .. } => unreachable!(),
    };
    if receiver.id != *binding_id
        || receiver.session != request.session_id
        || receiver.revision != request.revision
    {
        return Err("附件接收区已变化".into());
    }
    context(&window, bridge, remote, &runtime, &receiver).await?;
    match request.action {
        DesktopAttachmentAction::Bounds {
            bounds, handles, ..
        } => {
            if !valid_bounds(&bounds) {
                return Err("附件接收区域无效".into());
            }
            references(bridge, &receiver, &handles).await?;
            let mut current = bridge.receiver.lock().await;
            let current = current
                .as_mut()
                .filter(|current| current.id == receiver.id)
                .ok_or("附件接收区已变化")?;
            current.bounds = bounds;
            current.handles = handles;
        }
        DesktopAttachmentAction::Paste { .. } => {
            let _serial = bridge.importing.lock().await;
            context(&window, bridge, remote, &runtime, &receiver).await?;
            if receiver.handles.len() >= 32 {
                return Err("引用数量超出限制".into());
            }
            let store = runtime.attachments.clone();
            let reference = nexushub_core::read_tasks::spawn_blocking(move || {
                let png = super::clipboard::paste_png()?;
                store
                    .lock()
                    .map_err(|_| "无法保存粘贴图片")?
                    .import_png(&png, chrono::Utc::now().timestamp().max(0) as u64)
            })
            .await
            .map_err(|_| "无法读取剪贴板图片")??;
            if let Err(error) = deliver(
                &window,
                state,
                remote,
                bridge,
                &runtime,
                &receiver,
                vec![reference.clone()],
            )
            .await
            {
                let _ = release(&runtime, vec![reference]).await;
                return Err(error);
            }
        }
        DesktopAttachmentAction::Release { handles, .. } => {
            if handles
                .iter()
                .any(|handle| !receiver.handles.contains(handle))
            {
                return Err("附件授权已变化".into());
            }
            let values = references(bridge, &receiver, &handles).await?;
            release(&runtime, values).await?;
            if let Some(current) = bridge
                .receiver
                .lock()
                .await
                .as_mut()
                .filter(|current| current.id == receiver.id)
            {
                current.handles.retain(|handle| !handles.contains(handle));
            }
        }
        _ => unreachable!(),
    }
    Ok(DesktopAttachmentResult {
        binding_id: receiver.id,
    })
}

async fn deliver(
    window: &WebviewWindow,
    state: &DesktopState,
    remote: &RemoteConnections,
    bridge: &CodexDesktopBridge,
    runtime: &BridgeRuntime,
    receiver: &Receiver,
    values: Vec<NativeReference>,
) -> Result<(), String> {
    context(window, bridge, remote, runtime, receiver).await?;
    verify_thread(state, &receiver.thread).await?;
    context(window, bridge, remote, runtime, receiver).await?;
    let mut current = bridge.receiver.lock().await;
    let current = current
        .as_mut()
        .filter(|current| current.id == receiver.id)
        .ok_or("附件接收区已变化")?;
    // Validate the entire batch before issuing a single grant. Never silently
    // truncate a drop or admit only the valid prefix of an invalid batch.
    if current.handles.len() + values.len() > 32 {
        return Err("引用数量超出限制，本次附件未添加".into());
    }
    let mut grants = bridge.grants.lock().await;
    let items = values
        .into_iter()
        .map(|reference| {
            let name = match &reference {
                NativeReference::File { name, .. } => name.clone(),
                _ => unreachable!(),
            };
            let kind = reference.kind().to_owned();
            let handle = grants.insert(
                receiver.revision,
                Some(receiver.thread.clone()),
                GrantValue::Reference(reference),
            );
            DesktopChosenItem { handle, name, kind }
        })
        .collect::<Vec<_>>();
    receiver
        .channel
        .send(DesktopAttachmentEvent {
            binding_id: receiver.id.clone(),
            items: items.clone(),
            error: None,
        })
        .map_err(|_| "附件接收区已关闭")?;
    for item in items {
        if !current.handles.contains(&item.handle) {
            current.handles.push(item.handle);
        }
    }
    Ok(())
}

fn capture_drop(paths: Vec<PathBuf>, remaining: usize) -> Result<Vec<NativeReference>, String> {
    if paths.is_empty() || paths.len() > remaining.min(32) {
        return Err("引用数量超出限制，本次附件未添加".into());
    }
    paths
        .into_iter()
        .map(NativeReference::selected_file_detailed)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|(_, message)| message.into())
}

pub(super) fn drop_files(
    window: WebviewWindow,
    paths: Vec<PathBuf>,
    position: tauri::PhysicalPosition<f64>,
) {
    // Called only by Tauri's native OS event, never by an RPC path parameter.
    if !visible(&window) {
        return;
    }
    let app = window.app_handle().clone();
    tauri::async_runtime::spawn(async move {
        let bridge = app.state::<CodexDesktopBridge>();
        let remote = app.state::<RemoteConnections>();
        let state = app.state::<DesktopState>();
        let Some(receiver) = bridge.receiver.lock().await.clone() else {
            return;
        };
        let scale = window.scale_factor().unwrap_or(1.0);
        if !contains(&receiver.bounds, position.x / scale, position.y / scale) {
            return;
        }
        let result = async {
            let _serial = bridge.importing.lock().await;
            let _guard = remote.begin_local_write(receiver.revision)?;
            let runtime = bridge
                .runtime
                .lock()
                .await
                .clone()
                .ok_or("请先连接 Codex 桌面")?;
            context(&window, &bridge, &remote, &runtime, &receiver).await?;
            let remaining = 32usize.saturating_sub(receiver.handles.len());
            let values =
                nexushub_core::read_tasks::spawn_blocking(move || capture_drop(paths, remaining))
                    .await
                    .map_err(|_| "无法读取拖入附件")??;
            deliver(
                &window, &state, &remote, &bridge, &runtime, &receiver, values,
            )
            .await
        }
        .await;
        if let Err(error) = result {
            if bridge
                .receiver
                .lock()
                .await
                .as_ref()
                .is_some_and(|current| current.id == receiver.id)
            {
                let _ = receiver.channel.send(DesktopAttachmentEvent {
                    binding_id: receiver.id,
                    items: Vec::new(),
                    error: Some(error),
                });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn receiver_bounds_are_finite_and_drop_is_inside_the_composer() {
        let mut bounds = DesktopAttachmentBounds {
            x: 10.,
            y: 20.,
            width: 100.,
            height: 40.,
        };
        assert!(valid_bounds(&bounds));
        assert!(contains(&bounds, 10., 20.));
        assert!(!contains(&bounds, 110., 20.));
        assert!(!contains(&bounds, 10., 60.));
        assert!(!contains(&bounds, f64::NAN, 20.));
        bounds.width = f64::INFINITY;
        assert!(!valid_bounds(&bounds));
        bounds.width = 0.;
        assert!(!valid_bounds(&bounds));
    }

    #[test]
    fn native_drop_batch_has_no_partial_grants_and_rechecks_identity() {
        let temporary = tempfile::tempdir().unwrap();
        let file = temporary.path().join("fixture.txt");
        std::fs::write(&file, "fixture").unwrap();
        assert!(capture_drop(vec![file.clone(), temporary.path().into()], 32).is_err());
        assert!(capture_drop(vec![file.clone()], 0).is_err());
        assert!(capture_drop(vec![file.clone(); 33], 32).is_err());
        let captured = capture_drop(vec![file.clone()], 1).unwrap();
        let replacement = temporary.path().join("replacement.txt");
        std::fs::write(&replacement, "changed").unwrap();
        std::fs::rename(replacement, file).unwrap();
        assert!(captured[0].validate().is_err());
    }

    #[test]
    fn attachment_contract_rejects_client_paths_bytes_and_unknown_actions() {
        for action in [
            serde_json::json!({"type":"paste","bindingId":"fixture","path":"/fixture/file.png"}),
            serde_json::json!({"type":"paste","bindingId":"fixture","bytes":[0,1]}),
            serde_json::json!({"type":"drop","paths":["/fixture/file.png"]}),
        ] {
            assert!(serde_json::from_value::<DesktopAttachmentRequest>(
                serde_json::json!({"revision":1,"sessionId":"fixture","action":action})
            )
            .is_err());
        }
        let event = DesktopAttachmentEvent {
            binding_id: "fixture".into(),
            error: None,
            items: vec![DesktopChosenItem {
                handle: "opaque".into(),
                name: "fixture.png".into(),
                kind: "image".into(),
            }],
        };
        let value = serde_json::to_value(event).unwrap();
        assert_eq!(value["items"][0].as_object().unwrap().len(), 3);
        assert!(value["items"][0].get("path").is_none());
    }

    #[test]
    fn raw_tauri_events_cannot_be_subscribed_from_the_webview() {
        let capabilities: serde_json::Value =
            serde_json::from_str(include_str!("../../capabilities/default.json")).unwrap();
        assert!(capabilities["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "core:event:deny-listen"));
        let frontend = include_str!("../../../webui/src/lib/api/desktop.ts");
        assert!(!frontend.contains("tauri://drag"));
        assert!(!frontend.contains("getPathForFile"));
    }
}
