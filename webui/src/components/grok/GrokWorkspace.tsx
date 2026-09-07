import { Check, ChevronLeft, Pencil, RefreshCw, Search, Terminal, Trash2, X } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useGrokActions, useGrokDetail, useGrokSessions } from "../../lib/query/grok";
import { shouldAutoFollowMessageStream } from "../../lib/domain/conversationViewModel";
import { MarkdownContent } from "../common/MarkdownContent";
import { TaskMenu } from "../common/TaskMenu";
import { ConfirmDialog } from "../common/ConfirmDialog";
import type { GrokDeletePreview, GrokSessionSummary } from "../../types";

export function GrokWorkspace({ csrfToken }: { csrfToken?: string | null }) {
  const menuTrigger = useRef<HTMLElement>(null);
  const stream = useRef<HTMLDivElement>(null);
  const scrollState = useRef({ id: "", follow: true });
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [narrow, setNarrow] = useState(() => typeof window !== "undefined" && window.matchMedia("(max-width: 767px)").matches);
  const [renaming, setRenaming] = useState(false);
  const [title, setTitle] = useState("");
  const [preview, setPreview] = useState<GrokDeletePreview | null>(null);
  const sessions = useGrokSessions(query);
  const selected = sessions.data?.find((item) => item.id === selectedId) ?? sessions.data?.[0];
  const detailVisible = !narrow || Boolean(selectedId);
  const detail = useGrokDetail(detailVisible ? selected?.id : undefined);
  const actions = useGrokActions(csrfToken);
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
    if (scrollState.current.id !== selected?.id) {
      scrollState.current = { id: selected?.id ?? "", follow: true };
    }
    if (scrollState.current.follow) element.scrollTop = element.scrollHeight;
  }, [selected?.id, selectedId, detailVisible, detail.data?.events]);
  return <div className={`provider-workspace ${selectedId ? "has-selection" : ""}`}>
    <aside className="provider-list">
      <header className="workspace-heading"><h1>Grok Build</h1><button className="icon-button" onClick={() => { void sessions.refetch(); if (selected && detailVisible) void detail.refetch(); }} title="刷新任务"><RefreshCw size={16} /></button></header>
      <label className="search-box"><Search size={16} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索任务" /></label>
      <div className="provider-list-scroll">
        {sessions.error && <div className="form-error" role="alert">{sessions.error.message}</div>}
        {sessions.data?.map((item) => <button key={item.id} className={`provider-session ${item.id === selected?.id ? "selected" : ""}`} onClick={() => { setSelectedId(item.id); setRenaming(false); setPreview(null); }}><strong>{grokSessionLabel(item)}</strong><span>{item.cwd}</span><small>{item.status === "running" ? "运行中" : item.status === "unknown" ? "状态未知" : item.updatedAt ? new Date(item.updatedAt).toLocaleString() : "最近"}</small></button>)}
        {sessions.isLoading && <div className="muted-row">正在读取任务...</div>}
        {!sessions.isLoading && !sessions.data?.length && <div className="muted-row">暂无 Grok 任务</div>}
      </div>
    </aside>
    <main className="provider-detail">
      {!selected && <div className="empty-state"><Terminal size={28} /><strong>选择一个 Grok 任务</strong></div>}
      {selected && <>
        <header className="conversation-header">
          <button className="icon-button mobile-back" title="返回任务列表" onClick={() => setSelectedId(null)}><ChevronLeft size={18} /></button>
          <div className="conversation-title-copy"><h1 className="conversation-title">{grokSessionLabel(selected)}</h1><span className="muted-text">{selected.cwd}</span></div>
          <TaskMenu label="Grok 任务操作" triggerRef={menuTrigger}>
            <button onClick={() => { setTitle(selected.title); setRenaming(true); actions.rename.reset(); }}><Pencil size={15} />改名</button>
            <button disabled={selected.status !== "recent" || actions.preview.isPending} onClick={() => { actions.remove.reset(); actions.preview.mutate(selected.id, { onSuccess: setPreview }); }}><Trash2 size={15} />删除任务文件</button>
          </TaskMenu>
        </header>
        {renaming && <form className="inline-rename" onSubmit={(event) => { event.preventDefault(); actions.rename.mutate({ id: selected.id, title }, { onSuccess: () => setRenaming(false) }); }}><input aria-label="Grok 任务名称" maxLength={200} value={title} onChange={(event) => setTitle(event.target.value)} autoFocus /><button className="icon-button" title="保存名称" disabled={actions.rename.isPending || !title.trim()}><Check size={17} /></button><button className="icon-button" type="button" title="取消改名" onClick={() => setRenaming(false)}><X size={17} /></button></form>}
        {error && <div className="form-error" role="alert">{error.message}</div>}
        <div ref={stream} className="provider-events" onScroll={event => { scrollState.current.follow = shouldAutoFollowMessageStream(event.currentTarget); }}>{(detail.data?.events ?? []).map((event, index) =>
          event.kind.startsWith("tool_") ? <details className="grok-tool" key={event.callId ?? index}><summary>{event.text ?? "工具活动"}<small>{event.status === "completed" ? "完成" : event.status === "failed" ? "失败" : event.status === "in_progress" ? "进行中" : ""}</small></summary>{event.detail && <pre>{event.detail}</pre>}</details>
            : <article className={`provider-event ${event.kind}`} key={index}><div className="chat-meta">{event.kind === "user_message_chunk" ? "你" : event.kind === "plan" ? "计划" : "Grok"}</div><MarkdownContent text={event.text ?? ""} /></article>
        )}{detail.isLoading && <div className="muted-row">正在读取消息...</div>}{!detail.isLoading && !detail.data?.events.length && <div className="muted-row">暂无历史活动</div>}</div>
      </>}
    </main>
    {preview && <ConfirmDialog labelledBy="grok-delete-title" busy={actions.remove.isPending} onCancel={() => setPreview(null)} returnFocus={menuTrigger}>
      <h2 id="grok-delete-title">删除任务文件</h2><strong>{preview.title}</strong><code className="delete-path">{preview.path}</code><p>{preview.fileCount} 个文件，{preview.bytes.toLocaleString()} 字节。仅删除此 session 目录；工作目录、worktree、配置与云端任务保留。</p>
      {actions.remove.error && <div role="alert" className="form-error">{actions.remove.error.message}</div>}
      <div className="button-row"><button className="secondary-button" disabled={actions.remove.isPending} onClick={() => setPreview(null)}>取消</button><button className="danger-button" disabled={actions.remove.isPending} onClick={() => actions.remove.mutate({ id: preview.id, confirmed: true, fingerprint: preview.fingerprint }, { onSuccess: () => { setPreview(null); setSelectedId(null); } })}><Trash2 size={16} />{actions.remove.isPending ? "正在删除..." : "确认删除任务文件"}</button></div>
    </ConfirmDialog>}
  </div>;
}

export function grokSessionLabel(session: GrokSessionSummary): string { return session.title || session.id; }
