import { UserMessage, UserMessageScope } from "../common/UserMessage";
import { ToolOutput } from "../common/FilePathLink";
import { MarkdownPathScope } from "../common/FilePathLink";
import { ActivityDetails, DisclosureRevealContext, DisclosureScope } from "../common/ActivityDetails";
import { visibleMarkdown } from "../../lib/domain/visibleMarkdown";
import { Check, ChevronLeft, Copy, FolderOpen, Pencil, RefreshCw, Search, Terminal, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useRef, useMemo, useState, type ReactNode } from "react";
import { useHistoryScroll } from "../../lib/useHistoryScroll";
import { HistoryLoading } from "../common/HistoryLoading";
import { SessionSize } from "../common/SessionSize";
import { machineScope, useConnection } from "../../lib/query/connection";
import { useClaudeActions, useClaudeDetail, useClaudeSessions } from "../../lib/query/claude";
import type { ClaudeDeletePreview, ClaudeHistoryEvent, ClaudeSessionSummary, SessionSearchResult } from "../../types";
import { ConfirmDialog } from "../common/ConfirmDialog";
import { MarkdownContent } from "../common/MarkdownContent";
import { PlanActions } from "../common/PlanActions";
import { CopyReplyButton } from "../common/CopyReplyButton";
import { ExecutionGroupView } from "../common/ExecutionGroupView";
import { TaskMenu, ThreadMenuItems, type ThreadMenuAction } from "../common/TaskMenu";
import { canRevealLocalPath, revealLocalPath } from "../../lib/api/filePaths";
import { claudeResumeCommand } from "../../lib/domain/claudeSession";
import { useSessionSelection } from "../../lib/query/sessions";
import { SessionBatchControls, SessionCheckbox } from "../common/SessionBatchControls";
import { RunningIndicator } from "../common/RunningIndicator";
import { RenameableSession } from "../common/RenameableSession";
import { groupClaudeEvents } from "../../lib/domain/executionGroups";
import type { ExecutionRenderItem } from "../../lib/domain/executionGroups";
import { TimelineRail } from "../common/TimelineRail";
import { claudeTimelinePage, useTimelineWindow } from "../../lib/useTimelineWindow";
import { getClaudeSession } from "../../lib/api/claude";
import { TimelineHistoryStatus } from "../common/TimelineHistoryStatus";
import { userTimelineEntries } from "../../lib/domain/timelineViewModel";
import { locateTimelineTarget, useSearchWorkspace } from "../common/SessionSearch";

export function ClaudeWorkspace() {
  const connection = useConnection();
  const menuTrigger = useRef<HTMLElement>(null);
  const stream = useRef<HTMLDivElement>(null);
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
  const loadedEvents = useMemo(() => {
    const latest = new Map<string, ClaudeHistoryEvent>();
    for (const page of [...(detail.data?.pages ?? [])].reverse()) for (const event of page.events) latest.set(event.id, event);
    return [...latest.values()];
  }, [detail.data]);
  const navigation = useTimelineWindow({
    scope: `${machineScope()}:claude:${selected?.sessionKey ?? ""}`, streamRef: stream, history: detail.data?.pages[0]?.history, latestId: loadedEvents[loadedEvents.length - 1]?.id,
    id: (event: ClaudeHistoryEvent) => event.id ?? "",
    read: async cursor => claudeTimelinePage(await getClaudeSession(selected!.sessionKey, cursor)),
    onNavigate: () => { history.follow.current = false; setPendingSearch(null); },
    onLocated: follow => { history.follow.current = Boolean(follow); history.capture(); }
  });
  const events = navigation.items ?? loadedEvents;
  const groupedEvents = useMemo(() => groupClaudeEvents(events), [events]);
  const timelineEntries = useMemo(() => detail.data?.pages[0]?.timeline ?? userTimelineEntries("claude_code", groupedEvents.flatMap(entry => entry.kind === "item"
    ? [{ id: entry.key, kind: entry.item.kind, text: entry.item.text, userMessage: entry.item.userMessage }] : [])), [detail.data, groupedEvents]);
  const actions = useClaudeActions();
  const error = actions.rename.error ?? actions.preview.error ?? actions.remove.error ?? (detail.isFetchNextPageError ? null : detail.error) ?? sessions.error;
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

  const oldestPage = detail.data?.pages[detail.data.pages.length - 1];
  const history = useHistoryScroll({
    scopeKey: `${machineScope()}:claude:${selected?.sessionKey ?? ""}:${navigation.generation}`, streamRef: stream,
    cursor: navigation.active ? navigation.before : oldestPage?.beforeCursor, hasMore: navigation.active ? navigation.hasMore : Boolean(detail.hasNextPage), ready: detailVisible && Boolean(detail.data),
    initialPosition: navigation.active ? { top: 0, follow: false } : undefined,
    busy: navigation.busy || (!navigation.active && detail.isFetching), searchActive: Boolean(pendingSearch),
    loadPage: async (cursor, isCurrent) => {
      if (navigation.active) return navigation.loadOlder(cursor, isCurrent);
      const result = await detail.fetchNextPage({ cancelRefetch: false });
      if (result.isError) throw result.error;
      const page = result.data?.pages[result.data.pages.length - 1];
      if (!page) throw new Error("历史读取失败");
      return { cursor: page.beforeCursor, hasMore: page.hasMore };
    }
  });
  useEffect(() => {
    if (!pendingSearch || pendingSearch.sessionKey !== selected?.sessionKey || detail.isLoading || detail.isFetching || history.loading) return;
    if (navigation.active) { navigation.latest(); return; }
    history.follow.current = false;
    if (locateTimelineTarget(pendingSearch.positionKey, stream.current?.closest(".provider-detail") ?? undefined, "auto")) { history.capture(); setFeedback(""); setPendingSearch(null); }
    else if (detail.hasNextPage) { if (!history.error) void history.loadOlder("search"); }
    else { setFeedback("结果所在历史未加载，请刷新后重试"); setPendingSearch(null); }
  }, [navigation.active, navigation.latest, pendingSearch, selected?.sessionKey, detail.isLoading, detail.isFetching, detail.hasNextPage, history.loadOlder, history.loading, history.error, history.follow, history.capture, events]);

  const copy = async (value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      setFeedback("已复制");
    } catch { setFeedback("复制失败：剪贴板不可用"); }
  };
  const menuActions = (item: ClaudeSessionSummary): ThreadMenuAction[] => [
    { id: "copy-id", label: "复制线程 ID", icon: <Copy size={15} />, run: () => { void copy(item.id); } },
    { id: "copy-path", label: "复制文件路径", icon: <Copy size={15} />, run: () => { void copy(item.path); } },
    { id: "copy-resume", label: "复制恢复命令", icon: <Terminal size={15} />, run: () => { void copy(claudeResumeCommand(item.cwd, item.id)); } },
    { id: "reveal", label: "在 Finder 中显示", icon: <FolderOpen size={15} />, disabled: !canRevealLocalPath(),
      reason: canRevealLocalPath() ? null : "仅可在本机 Finder 中显示本机会话", run: () => { void revealLocalPath(item.path).catch(() => setFeedback("无法在 Finder 中显示，请检查文件是否仍存在")); } },
    { id: "delete", label: "删除任务文件", icon: <Trash2 size={15} />, danger: true, disabled: actions.preview.isPending || !item.canDelete,
      reason: item.deleteBlockReason, run: () => { actions.remove.reset(); actions.preview.mutate(item.sessionKey, { onSuccess: setPreview }); } },
  ];

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
          onSelect={event => batch.select(item.sessionKey, event, () => { setSelectedKey(item.sessionKey); setRenaming(false); setPreview(null); setFeedback(""); })}
          onRename={(next) => actions.rename.mutateAsync({ sessionKey: item.sessionKey, title: next })}
          menuActions={menuActions(item)}
        ><strong>{claudeSessionLabel(item)}</strong><span>{item.id}</span><span>{item.cwd}</span><span className="session-status-row">{item.status === "running" ? <RunningIndicator /> : <small>{claudeStatusLabel(item)}</small>}<SessionSize size={item.storageSize} /></span></RenameableSession></div>)}
        {sessions.isLoading && <div className="muted-row">正在读取任务...</div>}
        {!sessions.isLoading && !sessions.data?.length && <div className="muted-row">未发现 Claude Code 会话</div>}
      </div>
    </aside>
    <main className="provider-detail">
      {!selected && <div className="empty-state"><Terminal size={28} /><strong>选择一个 Claude Code 任务</strong></div>}
      {selected && <>
        <header className="conversation-header" data-timeline-id="session">
          <button className="icon-button mobile-back" title="返回任务列表" onClick={() => setSelectedKey(null)}><ChevronLeft size={18} /></button>
          <div className="conversation-title-copy"><h1 className="conversation-title">{claudeSessionLabel(selected)}</h1></div>
          <TaskMenu label="Claude Code 任务操作" triggerRef={menuTrigger}>
            <button disabled={!selected.canRename} title={selected.renameBlockReason ?? undefined} onClick={() => { setTitle(selected.title); setRenaming(true); actions.rename.reset(); }}><Pencil size={15} />改名</button>
            <ThreadMenuItems actions={menuActions(selected)} />
          </TaskMenu>
        </header>
        {feedback && <div role="status" className="task-feedback">{feedback}</div>}
        {renaming && <form className="inline-rename" onSubmit={(event) => { event.preventDefault(); actions.rename.mutate({ sessionKey: selected.sessionKey, title }, { onSuccess: () => setRenaming(false) }); }}><input aria-label="Claude Code 任务名称" maxLength={200} value={title} onChange={(event) => setTitle(event.target.value)} autoFocus /><button className="icon-button" title="保存名称" disabled={actions.rename.isPending || !title.trim()}><Check size={17} /></button><button className="icon-button" type="button" title="取消改名" onClick={() => setRenaming(false)}><X size={17} /></button></form>}
        {selected.readError && <div className="form-error" role="alert">{selected.readError}</div>}
        {selected.readWarning && <div className="task-feedback" role="status">{selected.readWarning}</div>}
        {!selected.readError && (selected.renameBlockReason || selected.deleteBlockReason) && <div className="task-feedback" role="status">{[selected.renameBlockReason, selected.deleteBlockReason].filter((reason, index, reasons) => reason && reasons.indexOf(reason) === index).join("；")}{connection.target === "local" && <button disabled={actions.diagnostics.isPending} onClick={() => actions.diagnostics.mutate(selected.sessionKey)}>查看占用详情</button>}</div>}
        {connection.target === "local" && actions.diagnostics.variables === selected.sessionKey && <>
          {actions.diagnostics.error && <div className="form-error" role="alert">{actions.diagnostics.error.message}</div>}
          {actions.diagnostics.data && <ActivityDetails className="task-feedback" summary="原生进程归属" initiallyOpen stateKey={`ownership:${selected.sessionKey}`}>
            <p>{actions.diagnostics.data.ownership === "inactive" ? "未发现关联的活跃会话进程" : actions.diagnostics.data.ownership === "open" ? "存在活跃会话或文件占用" : "归属尚未确认"}</p>
            {actions.diagnostics.data.processes.map((process, index) => <p key={`${process.pid}:${index}`}>PID {process.pid} · {({ session: "会话", daemon: "后台服务", pty_host: "终端宿主", unknown: "未核验进程" } as Record<string, string>)[process.role] ?? process.role} · {process.registrationVerified ? "登记已核验" : "登记未核验"}{process.mismatchedFields.length > 0 && ` · 不匹配字段：${process.mismatchedFields.join("、")}`}</p>)}
          </ActivityDetails>}
        </>}
        {error && <div className="form-error" role="alert">{error.message}</div>}
        <div className="timeline-reading-shell"><TimelineRail entries={timelineEntries} streamRef={stream} onSelect={detail.data?.pages[0]?.timeline ? navigation.select : undefined} loadOlder={!detail.data?.pages[0]?.timeline && detail.hasNextPage ? () => { void history.loadOlder("search"); } : undefined} loading={history.loading} /><div ref={stream} className="provider-events" onScroll={history.onScroll} onScrollCapture={history.onScrollCapture} onWheel={history.onWheel} onTouchStart={history.onTouchStart} onTouchMove={history.onTouchMove} onKeyDown={history.onKeyDown} tabIndex={0}>
          <HistoryLoading loading={history.loading} error={history.error} retry={history.retry} />
          <UserMessageScope.Provider value={{ provider: "claude_code", sessionKey: selected.sessionKey }}><MarkdownPathScope.Provider value={selected.cwd}><DisclosureScope.Provider value={`claude:${selected.sessionKey}`}><DisclosureRevealContext.Provider value={pendingSearch?.sessionKey === selected.sessionKey ? pendingSearch.positionKey : null}>{renderClaudeEvents(groupedEvents, selected.title)}</DisclosureRevealContext.Provider></DisclosureScope.Provider></MarkdownPathScope.Provider></UserMessageScope.Provider>
          <TimelineHistoryStatus navigation={{ ...navigation, busy: navigation.busy || history.loading }} />
          {detail.isLoading && <div className="muted-row">正在读取消息...</div>}
          {!detail.isLoading && !events.length && <div className="muted-row">暂无历史活动</div>}
        </div></div>
      </>}
    </main>
    {preview && <ConfirmDialog labelledBy="claude-delete-title" busy={actions.remove.isPending} onCancel={() => setPreview(null)} returnFocus={menuTrigger}>
      <h2 id="claude-delete-title">删除任务文件</h2><strong>{preview.title}</strong>{(preview.paths ?? [preview.path]).map(path => <code className="delete-path" key={path}>{path}</code>)}<p>{preview.fileCount} 个会话及专属文件，{preview.bytes.toLocaleString()} 字节。</p>
      {actions.remove.error && <div role="alert" className="form-error">{actions.remove.error.message}</div>}
      {preview.warnings?.map(warning => <p className="form-error" key={warning}>{warning}</p>)}
      <div className="button-row"><button className="secondary-button" disabled={actions.remove.isPending} onClick={() => setPreview(null)}>取消</button><button className="danger-button" disabled={actions.remove.isPending} onClick={() => actions.remove.mutate({ sessionKey: preview.sessionKey, confirmed: true, fingerprint: preview.fingerprint }, { onSuccess: () => { setPreview(null); setSelectedKey(current => current === preview.sessionKey ? null : current); } })}><Trash2 size={16} />{actions.remove.isPending ? "正在删除..." : "确认删除任务文件"}</button></div>
    </ConfirmDialog>}
  </div>;
}

function ClaudeEvent({ event, activityId, title, timelineAliases = [] }: { event: ClaudeHistoryEvent; activityId: string; title: string; timelineAliases?: string[] }) {
  const timelineProps = { "data-timeline-id": activityId, "data-timeline-aliases": timelineAliases.length ? timelineAliases.join(" ") : undefined };
  if (event.kind === "user_message") return <UserMessage message={event.userMessage} text={event.text ?? ""} activityId={activityId} timelineId={activityId} timelineAliases={timelineAliases} />;
  if (event.kind === "plan") return <article className="provider-event plan" {...timelineProps}><header className="plan-heading"><strong>计划</strong><PlanActions markdown={event.text ?? ""} fallbackTitle={title} /></header><MarkdownContent text={event.text ?? ""} activityId={activityId} foldInstructions={false} /></article>;
  if (event.detail) return <ActivityDetails timelineId={activityId} timelineAliases={timelineAliases} className="grok-tool" stateKey={activityId} initiallyOpen={false} summary={<span className="tool-title">{event.text ?? "活动记录"}</span>}>{() => <ToolOutput text={event.detail!} />}</ActivityDetails>;
  if (!visibleMarkdown(event.text ?? "").trim()) return null;
  return <article className={`provider-event ${event.kind}${event.kind.startsWith("assistant_message") ? " assistant-reply" : ""}`} {...timelineProps}>{!event.kind.startsWith("assistant_message") && <div className="chat-meta">{claudeEventLabel(event)}</div>}<MarkdownContent text={event.text ?? ""} activityId={activityId} timelineAliases={timelineAliases} foldInstructions={event.kind !== "compaction" && event.kind !== "branch_summary"} />{event.kind.startsWith("assistant_message") && <CopyReplyButton text={event.text ?? ""} />}</article>;
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
