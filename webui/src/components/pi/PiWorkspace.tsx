import { UserMessage, UserMessageScope } from "../common/UserMessage";
import { ToolOutput } from "../common/FilePathLink";
import { MarkdownPathScope } from "../common/FilePathLink";
import { ActivityDetails, DisclosureScope } from "../common/ActivityDetails";
import { isInstructionFileActivity, visibleMarkdown } from "../../lib/domain/visibleMarkdown";
import { Check, ChevronLeft, Copy, Pencil, RefreshCw, Search, Terminal, Trash2, X } from "lucide-react";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { shouldAutoFollowMessageStream } from "../../lib/domain/conversationViewModel";
import { usePiActions, usePiDetail, usePiSessions } from "../../lib/query/pi";
import type { PiDeletePreview, PiHistoryEvent, PiSessionSummary, SessionSearchResult } from "../../types";
import { ConfirmDialog } from "../common/ConfirmDialog";
import { MarkdownContent } from "../common/MarkdownContent";
import { CopyReplyButton } from "../common/CopyReplyButton";
import { ExecutionGroupView } from "../common/ExecutionGroupView";
import { TaskMenu } from "../common/TaskMenu";
import { useSessionSelection } from "../../lib/query/sessions";
import { SessionBatchControls, SessionCheckbox } from "../common/SessionBatchControls";
import { RunningIndicator } from "../common/RunningIndicator";
import { RenameableSession } from "../common/RenameableSession";
import { groupPiCommandEvents } from "../../lib/domain/executionGroups";
import type { ExecutionRenderItem } from "../../lib/domain/executionGroups";
import { TimelineRail, type TimelineEntry } from "../common/TimelineRail";
import { locateTimelineTarget, useSearchWorkspace } from "../common/SessionSearch";

export function PiWorkspace({}: { }) {
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
  const [pendingSearch, setPendingSearch] = useState<SessionSearchResult | null>(null);
  const sessions = usePiSessions(query);
  const batch = useSessionSelection("pi", query, (sessions.data ?? []).map(s => s.sessionKey), keys => {
    if (selectedKey && keys.includes(selectedKey)) setSelectedKey(null);
    setRenaming(false);
    setPreview(null);
  });
  const selected = sessions.data?.find((item) => item.sessionKey === selectedKey) ?? sessions.data?.[0];
  const detailVisible = !narrow || Boolean(selectedKey);
  const detail = usePiDetail(detailVisible ? selected?.sessionKey : undefined);
  const groupedEvents = groupPiCommandEvents(detail.data?.events ?? []);
  const timelineEntries: TimelineEntry[] = groupedEvents.map(entry => entry.kind === "group"
    ? { id: entry.group.id, title: `${entry.group.provider} ${entry.group.kind === "tool" ? "工具" : "命令"}`, preview: entry.group.commands[0]?.preview ?? entry.group.commands[0]?.title, status: entry.group.running ? "进行中" : entry.group.failedCount ? "失败" : "完成" }
    : { id: entry.key, title: entry.item.kind, preview: entry.item.text ?? entry.item.detail ?? undefined });
  const actions = usePiActions();
  const error = actions.rename.error ?? actions.preview.error ?? actions.remove.error ?? detail.error ?? sessions.error;
  const selectSearchResult = useCallback((result: SessionSearchResult) => {
    setSelectedKey(result.sessionKey);
    setPendingSearch(result);
    setRenaming(false);
    setPreview(null);
    setFeedback("");
  }, []);
  useSearchWorkspace(useMemo(() => ({ provider: "pi" as const, sessionKey: selected?.sessionKey ?? null, selectResult: selectSearchResult }), [selected?.sessionKey, selectSearchResult]));

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
  useEffect(() => {
    if (!pendingSearch || pendingSearch.sessionKey !== selected?.sessionKey || detail.isLoading) return;
    if (locateTimelineTarget(pendingSearch.positionKey)) setPendingSearch(null);
    else { setFeedback("结果所在历史未加载，请刷新后重试"); setPendingSearch(null); }
  }, [pendingSearch, selected?.sessionKey, detail.isLoading, detail.data?.events]);

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
        {sessions.data?.map((item) => <div key={item.sessionKey} className="selectable-session">{batch.selecting && <SessionCheckbox batch={batch} sessionKey={item.sessionKey} title={piSessionLabel(item)} />}<RenameableSession
          title={piSessionLabel(item)}
          className={`provider-session ${item.sessionKey === selected?.sessionKey ? "selected" : ""}`}
          disabled={batch.busy}
          selecting={batch.selecting}
          selected={item.sessionKey === selected?.sessionKey}
          renameBlockReason={!item.canRename ? item.renameBlockReason : undefined}
          onSelect={() => { if (batch.selecting) { batch.toggle(item.sessionKey); return; } setSelectedKey(item.sessionKey); setRenaming(false); setPreview(null); setFeedback(""); }}
          onRename={(next) => actions.rename.mutateAsync({ sessionKey: item.sessionKey, title: next })}
        ><strong>{piSessionLabel(item)}</strong><span>{item.id}</span><span>{item.cwd}</span>{item.status === "running" ? <RunningIndicator /> : <small>{piStatusLabel(item)}</small>}</RenameableSession></div>)}
        {sessions.isLoading && <div className="muted-row">正在读取任务...</div>}
        {!sessions.isLoading && !sessions.data?.length && <div className="muted-row">未发现 Pi 会话</div>}
      </div>
    </aside>
    <main className="provider-detail">
      {!selected && <div className="empty-state"><Terminal size={28} /><strong>选择一个 Pi 任务</strong></div>}
      {selected && <>
        <header className="conversation-header" data-timeline-id="session">
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
        <div className="timeline-reading-shell"><TimelineRail entries={timelineEntries} streamRef={stream} /><div ref={stream} className="provider-events" onScroll={(event) => { scrollState.current.follow = shouldAutoFollowMessageStream(event.currentTarget); }}>
          <UserMessageScope.Provider value={{ provider: "pi", sessionKey: selected.sessionKey }}><MarkdownPathScope.Provider value={selected.cwd}><DisclosureScope.Provider value={`pi:${selected.sessionKey}`}>{renderPiEvents(groupedEvents)}</DisclosureScope.Provider></MarkdownPathScope.Provider></UserMessageScope.Provider>
          {detail.isLoading && <div className="muted-row">正在读取消息...</div>}
          {!detail.isLoading && !detail.data?.events.length && <div className="muted-row">暂无历史活动</div>}
        </div></div>
      </>}
    </main>
    {preview && <ConfirmDialog labelledBy="pi-delete-title" busy={actions.remove.isPending} onCancel={() => setPreview(null)} returnFocus={menuTrigger}>
      <h2 id="pi-delete-title">删除任务文件</h2><strong>{preview.title}</strong><code className="delete-path">{preview.path}</code><p>{preview.fileCount} 个 JSONL 文件，{preview.bytes.toLocaleString()} 字节。只删除此会话文件；工作目录、其他会话和 Pi 配置保留。</p>
      {actions.remove.error && <div role="alert" className="form-error">{actions.remove.error.message}</div>}
      <div className="button-row"><button className="secondary-button" disabled={actions.remove.isPending} onClick={() => setPreview(null)}>取消</button><button className="danger-button" disabled={actions.remove.isPending} onClick={() => actions.remove.mutate({ sessionKey: preview.sessionKey, confirmed: true, fingerprint: preview.fingerprint }, { onSuccess: () => { setPreview(null); setSelectedKey(null); } })}><Trash2 size={16} />{actions.remove.isPending ? "正在删除..." : "确认删除任务文件"}</button></div>
    </ConfirmDialog>}
  </div>;
}

function PiEvent({ event, activityId, timelineAliases = [] }: { event: PiHistoryEvent; activityId: string; timelineAliases?: string[] }) {
  if (event.kind === "user_message") return <UserMessage message={event.userMessage} text={event.text ?? ""} activityId={activityId} timelineId={activityId} timelineAliases={timelineAliases} />;
  if (event.kind === "tool_call" || event.kind === "tool_result") {
    const instructionFile = isInstructionFileActivity(event.role, event.text, event.detail);
    return <ActivityDetails timelineId={activityId} timelineAliases={timelineAliases} className="grok-tool execution-command" stateKey={activityId} initiallyOpen={false} summary={<>
      <span className="tool-title">{instructionFile ? "AGENTS.md" : visibleMarkdown(event.text ?? "工具活动")}</span>
      {event.status === "in_progress" && <RunningIndicator />}
      <small>{event.status === "completed" ? "完成" : event.status === "failed" ? "失败" : event.status === "in_progress" ? "进行中" : ""}</small>
    </>}>{event.detail && <ToolOutput text={event.detail} />}</ActivityDetails>;
  }
  if (!visibleMarkdown(event.text ?? "").trim()) return null;
  return <article className={`provider-event ${event.kind}`} data-timeline-id={activityId} data-timeline-aliases={timelineAliases.length ? timelineAliases.join(" ") : undefined}><div className="chat-meta">{piEventLabel(event)}</div><MarkdownContent text={event.text ?? ""} activityId={activityId} foldInstructions={event.kind !== "compaction" && event.kind !== "branch_summary"} />{event.kind.startsWith("assistant_message") && <CopyReplyButton text={event.text ?? ""} />}</article>;
}

function renderPiEvents(entries: ExecutionRenderItem<PiHistoryEvent>[]): ReactNode {
  return entries.map(entry => entry.kind === "group"
    ? <ExecutionGroupView key={entry.group.id} group={entry.group} />
    : <PiEvent key={entry.key} activityId={entry.key} event={entry.item} timelineAliases={entry.sourceIds} />);
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
