import { ArrowUp, FolderPlus, Paperclip, Square, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { chooseDesktopItem, createDesktopThread, desktopCatalog, type DesktopCatalogItem, type DesktopChosenItem, type DesktopMode } from "../../lib/query/desktop";
import { useCodexDesktop } from "../../hooks/useCodexDesktop";
import { DesktopRequests } from "./DesktopRequests";
import { ConfirmDialog } from "../common/ConfirmDialog";

export function NewDesktopThread({ onCreated }: { onCreated: (id: string) => void | Promise<void> }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const create = async () => {
    setBusy(true); setError("");
    try {
      const { item } = await chooseDesktopItem("project");
      if (!item) return;
      const result = await createDesktopThread(item.handle);
      await onCreated(result.threadId);
    } catch (reason) { setError(reason instanceof Error ? reason.message : "无法新建线程"); }
    finally { setBusy(false); }
  };
  return <><button className="icon-button compact" title="新建 Codex 线程" disabled={busy} onClick={() => { void create(); }}><FolderPlus size={16} /></button>
    {error && <span role="alert" className="desktop-create-error">{error}</span>}</>;
}

export function DesktopComposer({ threadId }: { threadId: string }) {
  const desktop = useCodexDesktop(threadId);
  const { view, busy } = desktop;
  const [text, setText] = useState("");
  const [references, setReferences] = useState<DesktopChosenItem[]>([]);
  const [localError, setLocalError] = useState("");
  const [choosing, setChoosing] = useState(false);
  const [catalogKind, setCatalogKind] = useState("");
  const [discard, setDiscard] = useState<string | null>(null);
  const identity = useRef<{ payload: string; id: string }>();
  const discardTrigger = useRef<HTMLButtonElement>(null);
  const writable = Boolean(view?.connected && view.writable);
  const active = Boolean(view?.activeTurnId);
  const send = async (steer = false) => {
    if (!view) return;
    const handles = references.map(item => item.handle);
    const payload = JSON.stringify({ text, handles, steer, turn: steer ? view.activeTurnId : null });
    if (identity.current?.payload !== payload) identity.current = { payload, id: crypto.randomUUID() };
    setLocalError("");
    try {
      if (steer && view.activeTurnId) await desktop.act({ type: "steer", messageId: identity.current.id, text, references: handles, turnId: view.activeTurnId });
      else await desktop.act({ type: "send", messageId: identity.current.id, text, references: handles });
      setText(""); setReferences([]); identity.current = undefined;
    } catch { /* The hook owns the fixed error; keep the same ID for a retry. */ }
  };
  const attachFile = async () => {
    if (!view) return;
    setChoosing(true); setLocalError("");
    try {
      const { item } = await chooseDesktopItem("file", view.sessionId);
      if (item) setReferences(items => items.some(value => value.handle === item.handle) ? items : [...items, item].slice(0, 32));
    } catch (reason) { setLocalError(reason instanceof Error ? reason.message : "无法添加附件"); }
    finally { setChoosing(false); }
  };
  const locked = busy || choosing;
  return <section className="desktop-composer" aria-label="与 Codex 协作">
    {!writable ? <div className="desktop-connect-row"><span>{busy ? "正在连接 Codex…" : "连接本机 Codex 后继续此线程"}</span><button disabled={locked} onClick={() => { void desktop.open(); }}>在 Codex 中打开</button><button disabled={locked} onClick={desktop.connect}>连接</button></div> : <>
      <DesktopRequests requests={view!.requests} busy={locked} act={desktop.act} open={desktop.open} />
      {view!.queue.length > 0 && <details className="desktop-outbox" open={view!.queue.some(item => item.status !== "queued")}>
        <summary>待发送 · {view!.queue.length}</summary>
        <ol>{view!.queue.map(item => <li key={item.id}>
          <span><small>{item.status === "outcome_unknown" ? "结果待核对" : item.status === "held" ? item.kind === "steer" ? "补充未发送，请取消后重试" : "已暂停" : item.status === "dispatching" ? "正在发送" : "等待线程空闲"}</small>{item.preview || "引用内容"}</span>
          {item.status === "outcome_unknown" ? <button ref={discardTrigger} disabled={locked} onClick={() => setDiscard(item.id)}>核对后移除</button>
            : ["queued", "held"].includes(item.status) && <button disabled={locked} onClick={() => { void desktop.act({ type: "cancelQueued", messageId: item.id }).catch(() => {}); }}>取消</button>}
        </li>)}</ol>
        {view!.queue.some(item => item.status === "held" && item.kind === "queue") && <button disabled={locked} onClick={() => { void desktop.act({ type: "resumeQueue" }).catch(() => {}); }}>继续发送队列</button>}
      </details>}
      <form onSubmit={event => { event.preventDefault(); void send(); }}>
        {references.length > 0 && <div className="desktop-reference-chips">{references.map(item => <span key={item.handle}>{item.name}<button type="button" aria-label={`移除 ${item.name}`} disabled={locked} onClick={() => setReferences(values => values.filter(value => value.handle !== item.handle))}><X size={12} /></button></span>)}</div>}
        <textarea aria-label="发送给 Codex" placeholder={active ? "添加下一条消息，或补充当前回合…" : "与 Codex 协作…"} rows={2} maxLength={131072} value={text} disabled={locked} onChange={event => setText(event.target.value)} onKeyDown={event => {
          if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing && (event.metaKey || event.ctrlKey)) { event.preventDefault(); if (text.trim() || references.length) void send(); }
        }} />
        <div className="desktop-composer-toolbar">
          <button type="button" className="icon-button compact" title="添加文件或图片" disabled={locked || references.length >= 32} onClick={() => { void attachFile(); }}><Paperclip size={16} /></button>
          <button type="button" disabled={locked || references.length >= 32} onClick={() => setCatalogKind(catalogKind ? "" : "skills")}>引用</button>
          <DesktopSettings sessionId={view!.sessionId} viewModel={view!.model} viewEffort={view!.reasoningEffort} viewMode={view!.mode} busy={locked} act={desktop.act} />
          <span className="desktop-composer-spacer" />
          {active && <><button type="button" disabled={locked || !text.trim() && !references.length} onClick={() => { void send(true); }}>补充</button><button type="button" className="icon-button compact" title="停止当前回合并暂停队列" disabled={locked} onClick={() => { void desktop.act({ type: "stop", turnId: view!.activeTurnId! }).catch(() => {}); }}><Square size={15} /></button></>}
          <button className="desktop-send" title={active ? "排队发送" : "发送（⌘ Enter）"} disabled={locked || !text.trim() && !references.length}><ArrowUp size={17} /><span>{active ? "排队" : "发送"}</span></button>
        </div>
      </form>
      {catalogKind && <DesktopReferences sessionId={view!.sessionId} kind={catalogKind} setKind={setCatalogKind} onSelect={item => { setReferences(items => [...items.filter(value => value.handle !== item.handle), item].slice(0, 32)); setCatalogKind(""); }} />}
    </>}
    {(localError || desktop.error) && <p className="form-error" role="alert">{localError || desktop.error}</p>}
    {discard && <ConfirmDialog labelledBy="desktop-discard-title" busy={busy} onCancel={() => setDiscard(null)} returnFocus={discardTrigger}>
      <h3 id="desktop-discard-title">已在 Codex 中核对发送结果？</h3><p>移除此记录后，NexusHub 不会重新发送它。请先确认 Codex 中是否已有这条消息。</p>
      <div className="dialog-actions"><button disabled={busy} onClick={() => setDiscard(null)}>返回</button><button disabled={busy} onClick={() => { void desktop.act({ type: "dismissUnknown", messageId: discard }).then(() => setDiscard(null)).catch(() => {}); }}>已核对，移除记录</button></div>
    </ConfirmDialog>}
  </section>;
}

function DesktopSettings({ sessionId, viewModel, viewEffort, viewMode, busy, act }: { sessionId: string; viewModel: string | null; viewEffort: string | null; viewMode: DesktopMode | null; busy: boolean; act: ReturnType<typeof useCodexDesktop>["act"] }) {
  const [items, setItems] = useState<DesktopCatalogItem[]>([]);
  const [error, setError] = useState("");
  useEffect(() => {
    let current = true;
    void desktopCatalog({ sessionId, projectHandle: null, kind: "models", query: "", cursor: null }).then(result => { if (current) { setItems(result.items); setError(""); } }).catch(() => { if (current) setError("模型列表暂不可用"); });
    return () => { current = false; };
  }, [sessionId, viewModel]);
  const selected = items.find(item => item.handle === viewModel);
  const update = (model: string, reasoningEffort: string | null, mode: DesktopMode) => { void act({ type: "settings", model, reasoningEffort, mode }).catch(() => {}); };
  return <div className="desktop-model-settings" title={error || undefined}>
    <select aria-label="模型" disabled={busy || !items.length} value={selected?.handle ?? ""} onChange={event => {
      const item = items.find(item => item.handle === event.target.value); if (item) update(item.handle, item.defaultReasoningEffort, viewMode ?? "default");
    }}><option value="" disabled>{viewModel || error || "读取模型…"}</option>{items.map(item => <option key={item.handle} value={item.handle}>{item.name}</option>)}</select>
    {selected && <select aria-label="思考强度" disabled={busy || !selected.reasoningEfforts.length} value={viewEffort ?? selected.defaultReasoningEffort ?? ""} onChange={event => update(selected.handle, event.target.value || null, viewMode ?? "default")}>
      {!selected.reasoningEfforts.length && <option value="">默认</option>}{selected.reasoningEfforts.map(effort => <option key={effort} value={effort}>{effort}</option>)}
    </select>}
    <select aria-label="协作模式" disabled={busy || !selected} value={viewMode ?? "default"} onChange={event => selected && update(selected.handle, viewEffort, event.target.value as DesktopMode)}><option value="default">默认</option><option value="plan">Plan</option></select>
  </div>;
}

function DesktopReferences({ sessionId, kind, setKind, onSelect }: { sessionId: string; kind: string; setKind: (kind: string) => void; onSelect: (item: DesktopChosenItem) => void }) {
  const [query, setQuery] = useState("");
  const [items, setItems] = useState<DesktopCatalogItem[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const epoch = useRef(0);
  useEffect(() => {
    const generation = ++epoch.current;
    setItems([]); setCursor(null); setError(""); setBusy(true);
    const timer = setTimeout(() => {
      void desktopCatalog({ sessionId, projectHandle: null, kind, query, cursor: null }).then(result => {
        if (generation === epoch.current) { setItems(result.items); setCursor(result.nextCursor); }
      }).catch(reason => { if (generation === epoch.current) setError(reason instanceof Error ? reason.message : "读取引用失败"); })
        .finally(() => { if (generation === epoch.current) setBusy(false); });
    }, 250);
    return () => { clearTimeout(timer); ++epoch.current; };
  }, [sessionId, kind, query]);
  const more = async () => {
    const generation = epoch.current; setBusy(true);
    try { const result = await desktopCatalog({ sessionId, projectHandle: null, kind, query, cursor }); if (generation === epoch.current) { setItems(result.items); setCursor(result.nextCursor); } }
    catch (reason) { if (generation === epoch.current) setError(reason instanceof Error ? reason.message : "读取引用失败"); }
    finally { if (generation === epoch.current) setBusy(false); }
  };
  return <div className="desktop-reference-picker">
    <div className="desktop-reference-search"><select aria-label="引用类型" value={kind} onChange={event => { setQuery(""); setKind(event.target.value); }}><option value="skills">技能</option><option value="apps">应用</option><option value="plugins">插件</option></select><input aria-label="搜索引用" placeholder="搜索名称" value={query} onChange={event => setQuery(event.target.value)} /><button className="icon-button compact" title="关闭引用" onClick={() => setKind("")}><X size={14} /></button></div>
    {busy && <p className="muted-text">正在读取…</p>}{error && <p role="alert" className="form-error">{error}</p>}
    <ul>{items.map(item => <li key={item.handle}><button disabled={busy} onClick={() => onSelect(item)}><strong>{item.name}</strong><small>{item.description}</small></button></li>)}</ul>
    {!busy && !error && !items.length && <p className="muted-text">没有可用引用</p>}{cursor && <button disabled={busy} onClick={() => { void more(); }}>下一页</button>}
  </div>;
}
