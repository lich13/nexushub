import { SessionSize } from "../common/SessionSize";
import { UserMessage, UserMessageScope } from "../common/UserMessage";
import { MarkdownPathScope } from "../common/FilePathLink";
import { DisclosureRevealContext, DisclosureScope } from "../common/ActivityDetails";
import { visibleMarkdown } from "../../lib/domain/visibleMarkdown";
import { Check, ChevronLeft, Copy, Pencil, RefreshCw, Search, Terminal, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useGrokActions, useGrokDetail, useGrokSessions } from "../../lib/query/grok";
import { useHistoryScroll } from "../../lib/useHistoryScroll";
import { HistoryLoading } from "../common/HistoryLoading";
import { machineScope } from "../../lib/query/connection";
import { MarkdownContent } from "../common/MarkdownContent";
import { CopyReplyButton } from "../common/CopyReplyButton";
import { ExecutionGroupView } from "../common/ExecutionGroupView";
import { PlanActions } from "../common/PlanActions";
import { TaskMenu } from "../common/TaskMenu";
import { ConfirmDialog } from "../common/ConfirmDialog";
import type { GrokDeletePreview, GrokHistoryEvent, GrokSessionSummary, SessionSearchResult } from "../../types";
import { useSessionSelection } from "../../lib/query/sessions";
import { SessionBatchControls, SessionCheckbox } from "../common/SessionBatchControls";
import { RunningIndicator } from "../common/RunningIndicator";
import { RenameableSession } from "../common/RenameableSession";
import { groupGrokCommandEvents } from "../../lib/domain/executionGroups";
import type { ExecutionRenderItem } from "../../lib/domain/executionGroups";
import { TimelineRail } from "../common/TimelineRail";
import { userTimelineEntries } from "../../lib/domain/timelineViewModel";
import { locateTimelineTarget, useSearchWorkspace } from "../common/SessionSearch";

export function GrokWorkspace({}: { }) {
  const menuTrigger = useRef<HTMLElement>(null);
  const stream = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [narrow, setNarrow] = useState(() => typeof window !== "undefined" && window.matchMedia("(max-width: 767px)").matches);
  const [renaming, setRenaming] = useState(false);
  const [title, setTitle] = useState("");
  const [preview, setPreview] = useState<GrokDeletePreview | null>(null);
  const [feedback, setFeedback] = useState("");
  const [pendingSearch, setPendingSearch] = useState<SessionSearchResult | null>(null);
  const sessions = useGrokSessions(query);
  const batch = useSessionSelection("grok", query, (sessions.data ?? []).map(s => s.id), (keys) => { if (selectedId && keys.includes(selectedId)) setSelectedId(null); setPreview(null); setRenaming(false); });
  const selected = sessions.data?.find((item) => item.id === selectedId) ?? sessions.data?.[0];
  const detailVisible = !narrow || Boolean(selectedId);
  const detail = useGrokDetail(detailVisible ? selected?.id : undefined);
  const events = useMemo(() => {
    const rows = new Map<string, GrokHistoryEvent>();
    for (const [pageIndex, page] of [...(detail.data?.pages ?? [])].reverse().entries()) for (const [index, event] of page.events.entries()) rows.set(event.id ?? `${pageIndex}:${index}`, event);
    return [...rows.values()];
  }, [detail.data]);
  const groupedEvents = useMemo(() => groupGrokCommandEvents(events), [events]);
  const timelineEntries = useMemo(() => userTimelineEntries("grok", groupedEvents.flatMap(entry => entry.kind === "item"
    ? [{ id: entry.key, kind: entry.item.kind, text: entry.item.text, userMessage: entry.item.userMessage }] : [])), [groupedEvents]);
  const actions = useGrokActions();
  const error = actions.rename.error ?? actions.preview.error ?? actions.remove.error ?? (detail.isFetchNextPageError ? null : detail.error) ?? sessions.error;
  const selectSearchResult = useCallback((result: SessionSearchResult) => {
    setSelectedId(result.sessionKey);
    setPendingSearch(result);
    setRenaming(false);
    setPreview(null);
    setFeedback("");
  }, []);
  useSearchWorkspace(useMemo(() => ({ provider: "grok" as const, sessionKey: selected?.id ?? null, selectResult: selectSearchResult }), [selected?.id, selectSearchResult]));
  const copyId = async () => {
    if (!selected) return;
    try {
      await navigator.clipboard.writeText(selected.id);
      setFeedback("已复制线程 ID");
    } catch (copyError) {
      setFeedback(`复制失败：${copyError instanceof Error ? copyError.message : "剪贴板不可用"}`);
    }
  };
  useEffect(() => {
    const media = window.matchMedia("(max-width: 767px)");
    const update = () => setNarrow(media.matches);
    media.addEventListener("change", update);
    update();
    return () => media.removeEventListener("change", update);
  }, []);
  const oldestPage = detail.data?.pages[detail.data.pages.length - 1];
  const history = useHistoryScroll({
    scopeKey: `${machineScope()}:grok:${selected?.id ?? ""}`, streamRef: stream,
    cursor: oldestPage?.beforeCursor, hasMore: Boolean(detail.hasNextPage), ready: detailVisible && Boolean(detail.data),
    busy: detail.isFetching, searchActive: Boolean(pendingSearch),
    loadPage: async () => {
      const result = await detail.fetchNextPage({ cancelRefetch: false });
      if (result.isError) throw result.error;
      const page = result.data?.pages[result.data.pages.length - 1];
      if (!page) throw new Error("历史读取失败");
      return { cursor: page.beforeCursor, hasMore: Boolean(page.hasMore) };
    }
  });
  useEffect(() => {
    if (!pendingSearch || pendingSearch.sessionKey !== selected?.id || detail.isLoading || detail.isFetching || history.loading) return;
    history.follow.current = false;
    if (locateTimelineTarget(pendingSearch.positionKey, stream.current?.closest(".provider-detail") ?? undefined, "auto")) { history.capture(); setFeedback(""); setPendingSearch(null); }
    else if (detail.hasNextPage) { if (!history.error) void history.loadOlder("search"); }
    else { setFeedback("结果所在历史未加载，请刷新后重试"); setPendingSearch(null); }
  }, [pendingSearch, selected?.id, detail.isLoading, detail.isFetching, detail.hasNextPage, history.loadOlder, history.loading, history.error, history.follow, history.capture, events]);
  return <div className={`provider-workspace ${selectedId ? "has-selection" : ""}`}>
    <aside className="provider-list">
      <header className="workspace-heading"><h1>Grok Build</h1><button className="icon-button" onClick={() => { void sessions.refetch(); if (selected && detailVisible) void detail.refetch(); }} title="刷新任务"><RefreshCw size={16} /></button></header>
      <label className="search-box"><Search size={16} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索任务" /></label>
      <div className="provider-list-scroll">
        <SessionBatchControls batch={batch} operations={["delete"]} />
        {sessions.error && <div className="form-error" role="alert">{sessions.error.message}</div>}
        {sessions.data?.map((item) => <div key={item.id} className="selectable-session">{batch.selecting && <SessionCheckbox batch={batch} sessionKey={item.id} title={grokSessionLabel(item)} />}<RenameableSession
          title={grokSessionLabel(item)}
          className={`provider-session ${item.id === selected?.id ? "selected" : ""}`}
          disabled={batch.busy}
          selecting={batch.selecting}
          selected={item.id === selected?.id}
          onSelect={() => { if (batch.selecting) { batch.toggle(item.id); return; } setSelectedId(item.id); setRenaming(false); setPreview(null); setFeedback(""); }}
          onRename={(next) => actions.rename.mutateAsync({ id: item.id, title: next })}
          menuActions={[
            { id: "copy-id", label: "复制线程 ID", icon: <Copy size={15} />, run: () => {
              void navigator.clipboard.writeText(item.id).then(() => setFeedback("已复制线程 ID")).catch(() => setFeedback("复制失败：剪贴板不可用"));
            } },
            { id: "delete", label: "删除任务文件", icon: <Trash2 size={15} />, danger: true, disabled: actions.preview.isPending || !(item.status === "recent"),
              reason: "运行中或状态未知的任务不能删除", run: () => { actions.remove.reset(); actions.preview.mutate(item.id, { onSuccess: setPreview }); } },
          ]}
        ><strong>{grokSessionLabel(item)}</strong><span>{item.cwd}</span><span className="session-status-row">{item.status === "running" ? <RunningIndicator /> : <small>{item.status === "unknown" ? "状态未知" : item.updatedAt ? new Date(item.updatedAt).toLocaleString() : "最近"}</small>}<SessionSize size={item.storageSize} /></span></RenameableSession></div>)}
        {sessions.isLoading && <div className="muted-row">正在读取任务...</div>}
        {!sessions.isLoading && !sessions.data?.length && <div className="muted-row">暂无 Grok 任务</div>}
      </div>
    </aside>
    <main className="provider-detail">
      {!selected && <div className="empty-state"><Terminal size={28} /><strong>选择一个 Grok 任务</strong></div>}
      {selected && <>
        <header className="conversation-header" data-timeline-id="session">
          <button className="icon-button mobile-back" title="返回任务列表" onClick={() => setSelectedId(null)}><ChevronLeft size={18} /></button>
          <div className="conversation-title-copy"><h1 className="conversation-title">{grokSessionLabel(selected)}</h1></div>
          <TaskMenu label="Grok 任务操作" triggerRef={menuTrigger}>
            <button onClick={() => { void copyId(); }}><Copy size={15} />复制线程 ID</button>
            <button onClick={() => { setTitle(selected.title); setRenaming(true); actions.rename.reset(); }}><Pencil size={15} />改名</button>
            <button disabled={selected.status !== "recent" || actions.preview.isPending} onClick={() => { actions.remove.reset(); actions.preview.mutate(selected.id, { onSuccess: setPreview }); }}><Trash2 size={15} />删除任务文件</button>
          </TaskMenu>
        </header>
        {feedback && <div role="status" className="task-feedback">{feedback}</div>}
        {renaming && <form className="inline-rename" onSubmit={(event) => { event.preventDefault(); actions.rename.mutate({ id: selected.id, title }, { onSuccess: () => setRenaming(false) }); }}><input aria-label="Grok 任务名称" maxLength={200} value={title} onChange={(event) => setTitle(event.target.value)} autoFocus /><button className="icon-button" title="保存名称" disabled={actions.rename.isPending || !title.trim()}><Check size={17} /></button><button className="icon-button" type="button" title="取消改名" onClick={() => setRenaming(false)}><X size={17} /></button></form>}
        {error && <div className="form-error" role="alert">{error.message}</div>}
        <div className="timeline-reading-shell"><TimelineRail entries={timelineEntries} streamRef={stream} /><div ref={stream} className="provider-events" onScroll={history.onScroll} onScrollCapture={history.onScrollCapture} onWheel={history.onWheel} onTouchStart={history.onTouchStart} onTouchMove={history.onTouchMove} onKeyDown={history.onKeyDown} tabIndex={0}><HistoryLoading loading={history.loading} error={history.error} retry={history.retry} /><UserMessageScope.Provider value={{ provider: "grok", sessionKey: selected.id }}><MarkdownPathScope.Provider value={selected.cwd}><DisclosureScope.Provider value={`grok:${selected.id}`}><DisclosureRevealContext.Provider value={pendingSearch?.sessionKey === selected.id ? pendingSearch.positionKey : null}>{renderGrokEvents(groupedEvents, grokSessionLabel(selected))}</DisclosureRevealContext.Provider></DisclosureScope.Provider></MarkdownPathScope.Provider></UserMessageScope.Provider>{detail.isLoading && <div className="muted-row">正在读取消息...</div>}{!detail.isLoading && !events.length && <div className="muted-row">暂无历史活动</div>}</div></div>
      </>}
    </main>
    {preview && <ConfirmDialog labelledBy="grok-delete-title" busy={actions.remove.isPending} onCancel={() => setPreview(null)} returnFocus={menuTrigger}>
      <h2 id="grok-delete-title">删除任务文件</h2><strong>{preview.title}</strong><code className="delete-path">{preview.path}</code><p>{preview.fileCount} 个文件，{preview.bytes.toLocaleString()} 字节。仅删除此 session 目录；工作目录、worktree、配置与云端任务保留。</p>
      {actions.remove.error && <div role="alert" className="form-error">{actions.remove.error.message}</div>}
      <div className="button-row"><button className="secondary-button" disabled={actions.remove.isPending} onClick={() => setPreview(null)}>取消</button><button className="danger-button" disabled={actions.remove.isPending} onClick={() => actions.remove.mutate({ id: preview.id, confirmed: true, fingerprint: preview.fingerprint }, { onSuccess: () => { setPreview(null); setSelectedId(current => current === preview.id ? null : current); } })}><Trash2 size={16} />{actions.remove.isPending ? "正在删除..." : "确认删除任务文件"}</button></div>
    </ConfirmDialog>}
  </div>;
}

function renderGrokEvents(entries: ExecutionRenderItem<GrokHistoryEvent>[], sessionTitle: string): ReactNode {
  return entries.map(entry => entry.kind === "group"
    ? <ExecutionGroupView key={entry.group.id} group={entry.group} />
    : renderGrokEvent(entry.item, entry.key, sessionTitle, entry.sourceIds));
}

function renderGrokEvent(event: GrokHistoryEvent, key: string, sessionTitle: string, sourceIds: string[]): ReactNode {
  const timelineProps = { "data-timeline-id": key, "data-timeline-aliases": sourceIds.join(" ") || undefined };
  if (event.kind === "user_message_chunk") return <UserMessage key={event.userMessage?.id ?? key} message={event.userMessage} text={event.text ?? ""} activityId={key} timelineId={key} timelineAliases={sourceIds} />;
  if (!visibleMarkdown(event.text ?? "").trim() && event.kind !== "plan") return null;
  return <article className={`provider-event ${event.kind}${event.kind.startsWith("agent_message") ? " assistant-reply" : ""}`} {...timelineProps} key={key}>
    {event.kind === "plan" && <div className="plan-header">
      <span className="chat-meta">计划</span>
      <PlanActions markdown={event.text ?? ""} fallbackTitle={sessionTitle} />
    </div>}
    <MarkdownContent text={event.text ?? ""} activityId={key} timelineAliases={sourceIds} foldInstructions={event.kind !== "plan"} />
    {event.kind.startsWith("agent_message") && <CopyReplyButton text={event.text ?? ""} />}
  </article>;
}

export function grokSessionLabel(session: GrokSessionSummary): string { return session.title || session.id; }
