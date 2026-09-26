import { Check, ChevronLeft, Copy, Pencil, RefreshCw, Search, Terminal, Trash2, X } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { shouldAutoFollowMessageStream } from "../../lib/domain/conversationViewModel";
import { usePiActions, usePiDetail, usePiSessions } from "../../lib/query/pi";
import type { PiDeletePreview, PiHistoryEvent, PiSessionSummary } from "../../types";
import { ConfirmDialog } from "../common/ConfirmDialog";
import { MarkdownContent } from "../common/MarkdownContent";
import { TaskMenu } from "../common/TaskMenu";
import { useSessionSelection } from "../../lib/query/sessions";
import { SessionBatchControls, SessionCheckbox } from "../common/SessionBatchControls";
import { RunningIndicator } from "../common/RunningIndicator";
import { groupPiCommandEvents, type ExecutionGroup } from "../../lib/domain/executionGroups";

export function PiWorkspace({ csrfToken }: { csrfToken?: string | null }) {
  const menuTrigger = useRef<HTMLElement>(null);
  const stream = useRef<HTMLDivElement>(null);
  const scrollState = useRef({ key: "", follow: true });
  const [query, setQuery] = useState("");
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [narrow, setNarrow] = useState(() => typeof window !== "undefined" && window.matchMedia("(max-width: 767px)").matches);
  const [renaming, setRenaming] = useState(false);
  const [title, setTitle] = useState("");
  const [preview, setPreview] = useState<PiDeletePreview | null>(null);
  const [feedback, setFeedback] = useState("");
  const sessions = usePiSessions(query);
  const batch = useSessionSelection("pi", query, (sessions.data ?? []).map(s => s.sessionKey), csrfToken, keys => {
    if (selectedKey && keys.includes(selectedKey)) setSelectedKey(null);
    setRenaming(false);
    setPreview(null);
  });
  const selected = sessions.data?.find((item) => item.sessionKey === selectedKey) ?? sessions.data?.[0];
  const detailVisible = !narrow || Boolean(selectedKey);
  const detail = usePiDetail(detailVisible ? selected?.sessionKey : undefined);
  const actions = usePiActions(csrfToken);
  const error = actions.rename.error ?? actions.preview.error ?? actions.remove.error ?? detail.error ?? sessions.error;

  useEffect(() => {
    const media = window.matchMedia("(max-width: 767px)");
    const update = () => setNarrow(media.matches);
    media.addEventListener("change", update);
    update();
    return () => media.removeEventListener("change", update);
  }, []);

  useLayoutEffect(() => {
    const element = stream.current;
    if (!element || !element.getClientRects().length) return;
    if (scrollState.current.key !== selected?.sessionKey) {
      scrollState.current = { key: selected?.sessionKey ?? "", follow: true };
    }
    if (scrollState.current.follow) element.scrollTop = element.scrollHeight;
  }, [selected?.sessionKey, selectedKey, detailVisible, detail.data?.events]);

  const copyId = async () => {
    if (!selected) return;
    try {
      await navigator.clipboard.writeText(selected.id);
      setFeedback("已复制线程 ID");
    } catch (copyError) {
      setFeedback(`复制失败：${copyError instanceof Error ? copyError.message : "剪贴板不可用"}`);
    }
  };

  return <div className={`provider-workspace ${selectedKey ? "has-selection" : ""}`}>
    <aside className="provider-list">
      <header className="workspace-heading"><h1>Pi</h1><button className="icon-button" onClick={() => { void sessions.refetch(); if (selected && detailVisible) void detail.refetch(); }} title="刷新任务"><RefreshCw size={16} /></button></header>
      <label className="search-box"><Search size={16} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索标题、ID 或工作目录" /></label>
      <div className="provider-list-scroll">
        <SessionBatchControls batch={batch} operations={["delete"]} />
        {sessions.error && <div className="form-error" role="alert">{sessions.error.message}</div>}
        {sessions.data?.map((item) => <div key={item.sessionKey} className="selectable-session">{batch.selecting && <SessionCheckbox batch={batch} sessionKey={item.sessionKey} title={piSessionLabel(item)} />}<button disabled={batch.busy} className={`provider-session ${item.sessionKey === selected?.sessionKey ? "selected" : ""}`} onClick={() => { if (batch.selecting) { batch.toggle(item.sessionKey); return; } setSelectedKey(item.sessionKey); setRenaming(false); setPreview(null); setFeedback(""); }}><strong>{piSessionLabel(item)}</strong><span>{item.id}</span><span>{item.cwd}</span>{item.status === "running" ? <RunningIndicator /> : <small>{piStatusLabel(item)}</small>}</button></div>)}
        {sessions.isLoading && <div className="muted-row">正在读取任务...</div>}
        {!sessions.isLoading && !sessions.data?.length && <div className="muted-row">未发现 Pi 会话</div>}
      </div>
    </aside>
    <main className="provider-detail">
      {!selected && <div className="empty-state"><Terminal size={28} /><strong>选择一个 Pi 任务</strong></div>}
      {selected && <>
        <header className="conversation-header">
          <button className="icon-button mobile-back" title="返回任务列表" onClick={() => setSelectedKey(null)}><ChevronLeft size={18} /></button>
          <div className="conversation-title-copy"><h1 className="conversation-title">{piSessionLabel(selected)}</h1><span className="muted-text">{selected.cwd}</span></div>
          <TaskMenu label="Pi 任务操作" triggerRef={menuTrigger}>
            <button onClick={() => { void copyId(); }}><Copy size={15} />复制线程 ID</button>
            <button disabled={!selected.canRename} title={selected.renameBlockReason ?? undefined} onClick={() => { setTitle(selected.title); setRenaming(true); actions.rename.reset(); }}><Pencil size={15} />改名</button>
            <button disabled={!selected.canDelete || actions.preview.isPending} title={selected.deleteBlockReason ?? undefined} onClick={() => { actions.remove.reset(); actions.preview.mutate(selected.sessionKey, { onSuccess: setPreview }); }}><Trash2 size={15} />删除任务文件</button>
          </TaskMenu>
        </header>
        {feedback && <div role="status" className="task-feedback">{feedback}</div>}
        {renaming && <form className="inline-rename" onSubmit={(event) => { event.preventDefault(); actions.rename.mutate({ sessionKey: selected.sessionKey, title }, { onSuccess: () => setRenaming(false) }); }}><input aria-label="Pi 任务名称" maxLength={200} value={title} onChange={(event) => setTitle(event.target.value)} autoFocus /><button className="icon-button" title="保存名称" disabled={actions.rename.isPending || !title.trim()}><Check size={17} /></button><button className="icon-button" type="button" title="取消改名" onClick={() => setRenaming(false)}><X size={17} /></button></form>}
        {selected.readError && <div className="form-error" role="alert">{selected.readError}</div>}
        {!selected.readError && (selected.renameBlockReason || selected.deleteBlockReason) && <div className="task-feedback" role="status">{selected.renameBlockReason ?? selected.deleteBlockReason}</div>}
        {error && <div className="form-error" role="alert">{error.message}</div>}
        <div ref={stream} className="provider-events" onScroll={(event) => { scrollState.current.follow = shouldAutoFollowMessageStream(event.currentTarget); }}>
          {renderPiEvents(detail.data?.events ?? [])}
          {detail.isLoading && <div className="muted-row">正在读取消息...</div>}
          {!detail.isLoading && !detail.data?.events.length && <div className="muted-row">暂无历史活动</div>}
        </div>
      </>}
    </main>
    {preview && <ConfirmDialog labelledBy="pi-delete-title" busy={actions.remove.isPending} onCancel={() => setPreview(null)} returnFocus={menuTrigger}>
      <h2 id="pi-delete-title">删除任务文件</h2><strong>{preview.title}</strong><code className="delete-path">{preview.path}</code><p>{preview.fileCount} 个 JSONL 文件，{preview.bytes.toLocaleString()} 字节。只删除此会话文件；工作目录、其他会话和 Pi 配置保留。</p>
      {actions.remove.error && <div role="alert" className="form-error">{actions.remove.error.message}</div>}
      <div className="button-row"><button className="secondary-button" disabled={actions.remove.isPending} onClick={() => setPreview(null)}>取消</button><button className="danger-button" disabled={actions.remove.isPending} onClick={() => actions.remove.mutate({ sessionKey: preview.sessionKey, confirmed: true, fingerprint: preview.fingerprint }, { onSuccess: () => { setPreview(null); setSelectedKey(null); } })}><Trash2 size={16} />{actions.remove.isPending ? "正在删除..." : "确认删除任务文件"}</button></div>
    </ConfirmDialog>}
  </div>;
}

function PiEvent({ event }: { event: PiHistoryEvent }) {
  if (event.kind === "tool_call" || event.kind === "tool_result") {
    return <details className="grok-tool"><summary>{event.text ?? "工具活动"}<small>{event.status === "completed" ? "完成" : event.status === "failed" ? "失败" : event.status === "in_progress" ? "进行中" : ""}</small></summary>{event.detail && <pre>{event.detail}</pre>}</details>;
  }
  return <article className={`provider-event ${event.kind}`}><div className="chat-meta">{piEventLabel(event)}</div><MarkdownContent text={event.text ?? ""} /></article>;
}

function renderPiEvents(events: PiHistoryEvent[]): ReactNode {
  return groupPiCommandEvents(events).map((entry, index) => entry.kind === "group"
    ? <PiExecutionGroup key={entry.group.id} group={entry.group} />
    : <PiEvent key={`${entry.item.callId ?? entry.item.timestamp ?? entry.item.kind}-${index}`} event={entry.item} />);
}

function PiExecutionGroup({ group }: { group: ExecutionGroup<PiHistoryEvent> }) {
  return <details className="execution-group" open={group.running || group.failed}>
    <summary><span>命令执行组</span><small>{group.items.length} 条命令{group.failed ? ` · ${group.items.filter(item => item.status === "failed").length} 条失败` : ""}</small>{group.running && <RunningIndicator />}</summary>
    {group.items.map((event, index) => <PiEvent key={`${event.callId ?? event.timestamp ?? event.kind}-${index}`} event={event} />)}
  </details>;
}

function piEventLabel(event: PiHistoryEvent): string {
  if (event.kind === "user_message") return "你";
  if (event.kind === "assistant_message") return "Pi";
  if (event.kind === "thinking") return "思考";
  if (event.kind === "compaction") return "压缩摘要";
  if (event.kind === "branch_summary") return "分支摘要";
  return event.role || "Pi";
}

function piStatusLabel(session: PiSessionSummary): string {
  if (session.status === "running") return "运行中";
  if (session.status === "unknown") return "状态未知";
  return session.updatedAt ? new Date(session.updatedAt).toLocaleString() : "最近";
}

export function piSessionLabel(session: PiSessionSummary): string { return session.title || session.id; }
