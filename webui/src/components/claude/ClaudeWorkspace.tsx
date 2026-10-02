import { UserMessage, UserMessageScope } from "../common/UserMessage";
import { ToolOutput } from "../common/FilePathLink";
import { MarkdownPathScope } from "../common/FilePathLink";
import { ActivityDetails, DisclosureScope } from "../common/ActivityDetails";
import { visibleMarkdown } from "../../lib/domain/visibleMarkdown";
import { Check, ChevronLeft, Copy, Pencil, RefreshCw, Search, Terminal, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useRef, useMemo, useState, type ReactNode } from "react";
import { shouldAutoFollowMessageStream } from "../../lib/domain/conversationViewModel";
import { useClaudeActions, useClaudeDetail, useClaudeSessions } from "../../lib/query/claude";
import type { ClaudeDeletePreview, ClaudeHistoryEvent, ClaudeSessionSummary, SessionSearchResult } from "../../types";
import { ConfirmDialog } from "../common/ConfirmDialog";
import { MarkdownContent } from "../common/MarkdownContent";
import { PlanActions } from "../common/PlanActions";
import { CopyReplyButton } from "../common/CopyReplyButton";
import { ExecutionGroupView } from "../common/ExecutionGroupView";
import { TaskMenu } from "../common/TaskMenu";
import { useSessionSelection } from "../../lib/query/sessions";
import { SessionBatchControls, SessionCheckbox } from "../common/SessionBatchControls";
import { RunningIndicator } from "../common/RunningIndicator";
import { RenameableSession } from "../common/RenameableSession";
import { groupClaudeEvents } from "../../lib/domain/executionGroups";
import type { ExecutionRenderItem } from "../../lib/domain/executionGroups";
import { TimelineRail } from "../common/TimelineRail";
import { userTimelineEntries } from "../../lib/domain/timelineViewModel";
import { locateTimelineTarget, useSearchWorkspace } from "../common/SessionSearch";

export function ClaudeWorkspace() {
  const menuTrigger = useRef<HTMLElement>(null);
  const stream = useRef<HTMLDivElement>(null);
  const scrollState = useRef({ key: "", follow: true });
  const [query, setQuery] = useState("");
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [narrow, setNarrow] = useState(() => typeof window !== "undefined" && window.matchMedia("(max-width: 767px)").matches);
  const [renaming, setRenaming] = useState(false);
  const [title, setTitle] = useState("");
  const [preview, setPreview] = useState<ClaudeDeletePreview | null>(null);
  const [feedback, setFeedback] = useState("");
  const [pendingSearch, setPendingSearch] = useState<SessionSearchResult | null>(null);
  const sessions = useClaudeSessions(query);
  const batch = useSessionSelection("claude_code", query, (sessions.data ?? []).map(s => s.sessionKey), keys => {
    if (selectedKey && keys.includes(selectedKey)) setSelectedKey(null);
    setRenaming(false);
    setPreview(null);
  });
  const listed = sessions.data?.find((item) => item.sessionKey === selectedKey) ?? sessions.data?.[0];
  const detailVisible = !narrow || Boolean(selectedKey);
  const detail = useClaudeDetail(detailVisible ? listed?.sessionKey : undefined);
  const selected = detail.data?.pages[0]?.summary ?? listed;
  const events = useMemo(() => {
    const seen = new Set<string>();
    return [...(detail.data?.pages ?? [])].reverse().flatMap(page => page.events).filter(event => {
      if (seen.has(event.id)) return false;
      seen.add(event.id); return true;
    });
  }, [detail.data]);
  const groupedEvents = useMemo(() => groupClaudeEvents(events), [events]);
  const timelineEntries = userTimelineEntries("claude_code", groupedEvents.flatMap(entry => entry.kind === "item"
    ? [{ id: entry.key, kind: entry.item.kind, text: entry.item.text, userMessage: entry.item.userMessage }] : []));
  const olderScroll = useRef<{ key: string; height: number; top: number } | null>(null);
  const actions = useClaudeActions();
  const error = actions.rename.error ?? actions.preview.error ?? actions.remove.error ?? detail.error ?? sessions.error;
  const selectSearchResult = useCallback((result: SessionSearchResult) => {
    setSelectedKey(result.sessionKey);
    setPendingSearch(result);
    setRenaming(false);
    setPreview(null);
    setFeedback("");
  }, []);
  useSearchWorkspace(useMemo(() => ({ provider: "claude_code" as const, sessionKey: selected?.sessionKey ?? null, selectResult: selectSearchResult }), [selected?.sessionKey, selectSearchResult]));

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
    const older = olderScroll.current;
    if (older && older.key === selected?.sessionKey && !detail.isFetchingNextPage) {
      element.scrollTop = older.top + element.scrollHeight - older.height;
      olderScroll.current = null;
    } else if (scrollState.current.follow && !older) element.scrollTop = element.scrollHeight;
  }, [selected?.sessionKey, selectedKey, detailVisible, events, detail.isFetchingNextPage]);
  useEffect(() => {
    if (!pendingSearch || pendingSearch.sessionKey !== selected?.sessionKey || detail.isLoading || detail.isFetchingNextPage) return;
    if (locateTimelineTarget(pendingSearch.positionKey)) setPendingSearch(null);
    else if (detail.hasNextPage) void detail.fetchNextPage();
    else { setFeedback("结果所在历史未加载，请刷新后重试"); setPendingSearch(null); }
  }, [pendingSearch, selected?.sessionKey, detail.isLoading, detail.isFetchingNextPage, detail.hasNextPage, detail.fetchNextPage, events]);

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
      <header className="workspace-heading"><h1>Claude Code</h1><button className="icon-button" onClick={() => { void sessions.refetch(); if (selected && detailVisible) void detail.refetch(); }} title="刷新任务"><RefreshCw size={16} /></button></header>
      <label className="search-box"><Search size={16} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="搜索标题、ID 或工作目录" /></label>
      <div className="provider-list-scroll">
        <SessionBatchControls batch={batch} operations={["delete"]} />
        {sessions.error && <div className="form-error" role="alert">{sessions.error.message}</div>}
        {sessions.data?.map((item) => <div key={item.sessionKey} className="selectable-session">{batch.selecting && <SessionCheckbox batch={batch} sessionKey={item.sessionKey} title={claudeSessionLabel(item)} />}<RenameableSession
          title={claudeSessionLabel(item)}
          className={`provider-session ${item.sessionKey === selected?.sessionKey ? "selected" : ""}`}
          disabled={batch.busy}
          selecting={batch.selecting}
          selected={item.sessionKey === selected?.sessionKey}
          renameBlockReason={!item.canRename ? item.renameBlockReason : undefined}
          onSelect={() => { if (batch.selecting) { batch.toggle(item.sessionKey); return; } setSelectedKey(item.sessionKey); setRenaming(false); setPreview(null); setFeedback(""); }}
          onRename={(next) => actions.rename.mutateAsync({ sessionKey: item.sessionKey, title: next })}
        ><strong>{claudeSessionLabel(item)}</strong><span>{item.id}</span><span>{item.cwd}</span>{item.status === "running" ? <RunningIndicator /> : <small>{claudeStatusLabel(item)}</small>}</RenameableSession></div>)}
        {sessions.isLoading && <div className="muted-row">正在读取任务...</div>}
        {!sessions.isLoading && !sessions.data?.length && <div className="muted-row">未发现 Claude Code 会话</div>}
      </div>
    </aside>
    <main className="provider-detail">
      {!selected && <div className="empty-state"><Terminal size={28} /><strong>选择一个 Claude Code 任务</strong></div>}
      {selected && <>
        <header className="conversation-header" data-timeline-id="session">
          <button className="icon-button mobile-back" title="返回任务列表" onClick={() => setSelectedKey(null)}><ChevronLeft size={18} /></button>
          <div className="conversation-title-copy"><h1 className="conversation-title">{claudeSessionLabel(selected)}</h1><span className="muted-text">{selected.cwd}</span></div>
          <TaskMenu label="Claude Code 任务操作" triggerRef={menuTrigger}>
            <button onClick={() => { void copyId(); }}><Copy size={15} />复制线程 ID</button>
            <button disabled={!selected.canRename} title={selected.renameBlockReason ?? undefined} onClick={() => { setTitle(selected.title); setRenaming(true); actions.rename.reset(); }}><Pencil size={15} />改名</button>
            <button disabled={!selected.canDelete || actions.preview.isPending} title={selected.deleteBlockReason ?? undefined} onClick={() => { actions.remove.reset(); actions.preview.mutate(selected.sessionKey, { onSuccess: setPreview }); }}><Trash2 size={15} />删除任务文件</button>
          </TaskMenu>
        </header>
        {feedback && <div role="status" className="task-feedback">{feedback}</div>}
        {renaming && <form className="inline-rename" onSubmit={(event) => { event.preventDefault(); actions.rename.mutate({ sessionKey: selected.sessionKey, title }, { onSuccess: () => setRenaming(false) }); }}><input aria-label="Claude Code 任务名称" maxLength={200} value={title} onChange={(event) => setTitle(event.target.value)} autoFocus /><button className="icon-button" title="保存名称" disabled={actions.rename.isPending || !title.trim()}><Check size={17} /></button><button className="icon-button" type="button" title="取消改名" onClick={() => setRenaming(false)}><X size={17} /></button></form>}
        {selected.readError && <div className="form-error" role="alert">{selected.readError}</div>}
        {selected.readWarning && <div className="task-feedback" role="status">{selected.readWarning}</div>}
        {!selected.readError && !selected.readWarning && (selected.renameBlockReason || selected.deleteBlockReason) && <div className="task-feedback" role="status">{selected.renameBlockReason ?? selected.deleteBlockReason}</div>}
        {error && <div className="form-error" role="alert">{error.message}</div>}
        <div className="timeline-reading-shell"><TimelineRail entries={timelineEntries} streamRef={stream} /><div ref={stream} className="provider-events" onScroll={(event) => { scrollState.current.follow = shouldAutoFollowMessageStream(event.currentTarget); }}>
          {detail.hasNextPage && <button className="secondary-button history-load-button" disabled={detail.isFetchingNextPage} onClick={() => {
            const element = stream.current;
            if (element) { olderScroll.current = { key: selected.sessionKey, height: element.scrollHeight, top: element.scrollTop }; scrollState.current.follow = false; }
            void detail.fetchNextPage();
          }}>{detail.isFetchingNextPage ? "正在加载..." : "加载较早消息"}</button>}
          <UserMessageScope.Provider value={{ provider: "claude_code", sessionKey: selected.sessionKey }}><MarkdownPathScope.Provider value={selected.cwd}><DisclosureScope.Provider value={`claude:${selected.sessionKey}`}>{renderClaudeEvents(groupedEvents, selected.title)}</DisclosureScope.Provider></MarkdownPathScope.Provider></UserMessageScope.Provider>
          {detail.isLoading && <div className="muted-row">正在读取消息...</div>}
          {!detail.isLoading && !events.length && <div className="muted-row">暂无历史活动</div>}
        </div></div>
      </>}
    </main>
    {preview && <ConfirmDialog labelledBy="claude-delete-title" busy={actions.remove.isPending} onCancel={() => setPreview(null)} returnFocus={menuTrigger}>
      <h2 id="claude-delete-title">删除任务文件</h2><strong>{preview.title}</strong><code className="delete-path">{preview.path}</code><p>{preview.fileCount} 个 JSONL 文件，{preview.bytes.toLocaleString()} 字节。只删除此会话文件；工作目录、其他会话和 Claude 配置保留。</p>
      {actions.remove.error && <div role="alert" className="form-error">{actions.remove.error.message}</div>}
      <div className="button-row"><button className="secondary-button" disabled={actions.remove.isPending} onClick={() => setPreview(null)}>取消</button><button className="danger-button" disabled={actions.remove.isPending} onClick={() => actions.remove.mutate({ sessionKey: preview.sessionKey, confirmed: true, fingerprint: preview.fingerprint }, { onSuccess: () => { setPreview(null); setSelectedKey(null); } })}><Trash2 size={16} />{actions.remove.isPending ? "正在删除..." : "确认删除任务文件"}</button></div>
    </ConfirmDialog>}
  </div>;
}

function ClaudeEvent({ event, activityId, title, timelineAliases = [] }: { event: ClaudeHistoryEvent; activityId: string; title: string; timelineAliases?: string[] }) {
  const timelineProps = { "data-timeline-id": activityId, "data-timeline-aliases": timelineAliases.length ? timelineAliases.join(" ") : undefined };
  if (event.kind === "user_message") return <UserMessage message={event.userMessage} text={event.text ?? ""} activityId={activityId} timelineId={activityId} timelineAliases={timelineAliases} />;
  if (event.kind === "plan") return <article className="provider-event plan" {...timelineProps}><header className="plan-heading"><strong>计划</strong><PlanActions markdown={event.text ?? ""} fallbackTitle={title} /></header><MarkdownContent text={event.text ?? ""} activityId={activityId} foldInstructions={false} /></article>;
  if (event.detail) return <ActivityDetails timelineId={activityId} timelineAliases={timelineAliases} className="grok-tool" stateKey={activityId} initiallyOpen={false} summary={<span className="tool-title">{event.text ?? "活动记录"}</span>}>{() => <ToolOutput text={event.detail!} />}</ActivityDetails>;
  if (!visibleMarkdown(event.text ?? "").trim()) return null;
  return <article className={`provider-event ${event.kind}`} {...timelineProps}><div className="chat-meta">{claudeEventLabel(event)}</div><MarkdownContent text={event.text ?? ""} activityId={activityId} foldInstructions={event.kind !== "compaction" && event.kind !== "branch_summary"} />{event.kind.startsWith("assistant_message") && <CopyReplyButton text={event.text ?? ""} />}</article>;
}

function renderClaudeEvents(entries: ExecutionRenderItem<ClaudeHistoryEvent>[], title: string): ReactNode {
  return entries.map(entry => entry.kind === "group"
    ? <ExecutionGroupView key={entry.group.id} group={entry.group} />
    : <ClaudeEvent key={entry.key} activityId={entry.key} event={entry.item} title={title} timelineAliases={entry.sourceIds} />);
}

function claudeEventLabel(event: ClaudeHistoryEvent): string {
  if (event.kind === "user_message") return "你";
  if (event.kind === "assistant_message") return "Claude Code";
  if (event.kind === "thinking") return "思考";
  if (event.kind === "compaction") return "压缩摘要";
  if (event.kind === "branch_summary") return "分支摘要";
  return event.role || "Claude Code";
}

function claudeStatusLabel(session: ClaudeSessionSummary): string {
  if (session.status === "running") return "运行中";
  if (session.status === "unknown") return "状态未知";
  return session.updatedAt ? new Date(session.updatedAt).toLocaleString() : "最近";
}

export function claudeSessionLabel(session: ClaudeSessionSummary): string { return session.title || session.id; }
