import { UserMessageScope } from "../common/UserMessage";
import { MarkdownPathScope } from "../common/FilePathLink";
import { DisclosureScope } from "../common/ActivityDetails";
import { Archive, ArchiveRestore, Check, ChevronLeft, Copy, Pencil, Trash2, X } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { MessageBlockView } from "./MessageStream";
import { ExecutionGroupView } from "../common/ExecutionGroupView";
import { useSessionSelection } from "../../lib/query/sessions";
import { SessionBatchControls } from "../common/SessionBatchControls";
import { TaskMenu } from "../common/TaskMenu";
import { useReadOnlyThreadActions, useThreadBlockPageMutation, type ThreadMessageSlot, type ThreadMessageStoreController } from "../../lib/query/threads";
import { threadStatusLabel, type SelectedThread, type View } from "../../lib/domain/codexViewModel";
import { sharedDisabledStates } from "../../lib/domain/visualContract";
import { latestAssistantCopyText, shouldAutoFollowMessageStream, threadResumeCommand, visibleConversationBlocksForHistory } from "../../lib/domain/conversationViewModel";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";
import type { SessionSearchResult, SubagentActivity, ThreadDetail, ThreadSummary } from "../../types";
import { groupCodexCommandBlocks } from "../../lib/domain/executionGroups";
import { TimelineRail } from "../common/TimelineRail";
import { userTimelineEntries } from "../../lib/domain/timelineViewModel";
import { locateTimelineTarget } from "../common/SessionSearch";
import { SubagentPanel } from "./SubagentPanel";
import { machineScope } from "../../lib/query/connection";



export function Conversation(props: {
  threadId: string;
  detail: ThreadDetail;
  selectedSummary?: ThreadSummary | null;
  archivedView?: boolean;
  slot: ThreadMessageSlot;
  messageStore: ThreadMessageStoreController;
  onSelect: (id: SelectedThread) => void;
  onPanelSelect: (view: View) => void;
  nextThreadAfterArchive: string | null;
  capabilities: RuntimeCapabilityMatrix;
  onBack?: () => void;
  searchTarget?: SessionSearchResult | null;
  searchReady?: boolean;
  onSearchResolved?: () => void;
  onSearchMiss?: () => void;
}) {
  const { detail, slot } = props;
  const summary = props.selectedSummary?.id === detail.summary.id
    ? { ...detail.summary, ...props.selectedSummary }
    : detail.summary;
  const [renaming, setRenaming] = useState(false);
  const [title, setTitle] = useState("");
  const [feedback, setFeedback] = useState<string | null>(null);
  const [historyExpanded, setHistoryExpanded] = useState(false);
  const stream = useRef<HTMLDivElement>(null);
  const shell = useRef<HTMLDivElement>(null);
  const [agentSelection, setAgentSelection] = useState<{ root: string; machine: string; agent: SubagentActivity; trigger: HTMLButtonElement } | null>(null);
  const [docked, setDocked] = useState(false);
  const machine = machineScope();
  const selectedAgent = agentSelection?.root === props.threadId && agentSelection.machine === machine ? agentSelection : null;
  useEffect(() => { setAgentSelection(null); }, [props.threadId, machine]);
  const viewportAnchor = useRef<{ element: HTMLElement; offset: number } | null>(null);
  const captureViewport = () => {
    const element = stream.current;
    if (!element) return;
    const top = element.getBoundingClientRect().top;
    const anchor = Array.from(element.querySelectorAll<HTMLElement>("[data-timeline-id]"))
      .find(item => item.getBoundingClientRect().bottom > top);
    viewportAnchor.current = anchor ? { element: anchor, offset: anchor.getBoundingClientRect().top - top } : null;
  };
  const openAgent = (agent: SubagentActivity, trigger: HTMLButtonElement) => {
    captureViewport();
    setAgentSelection({ root: props.threadId, machine: machineScope(), agent, trigger });
  };
  const closeAgent = () => {
    captureViewport();
    const trigger = selectedAgent?.trigger;
    setAgentSelection(null);
    requestAnimationFrame(() => { if (trigger?.isConnected) trigger.focus({ preventScroll: true }); });
  };
  const scrollState = useRef({ threadId: "", follow: true, prepend: null as number | null });
  const blocks = slot.blocks.length ? slot.blocks : detail.blocks;
  const visibleBlocks = visibleConversationBlocksForHistory(blocks, historyExpanded);
  const visibleItems = groupCodexCommandBlocks(visibleBlocks);
  const needsSubagentUpgrade = props.capabilities.hostSurface === "linux_server_api"
    && props.capabilities.threadSubagents !== true
    && visibleBlocks.some(block => block.role === "tool" && block.tool_name?.split(/[.:/]/).pop() === "spawn_agent");
  const timelineEntries = userTimelineEntries("codex", visibleItems.flatMap(entry => entry.kind === "item"
    ? [{ id: entry.item.id, role: entry.item.role, text: entry.item.text, userMessage: entry.item.user_message }] : []));
  const deletion = useSessionSelection("codex", props.threadId, [props.threadId], (keys) => {
    if (keys.includes(props.threadId)) props.onSelect(null);
  });
  const actions = useReadOnlyThreadActions({ onSuccess: () => setRenaming(false) });
  const older = useThreadBlockPageMutation({
    onBeforeLoad: () => stream.current ? stream.current.scrollHeight - stream.current.scrollTop : 0,
    onSuccess: ({ threadId, cursor, page, beforeHeight }) => {
      if (threadId === props.threadId) scrollState.current.prepend = beforeHeight;
      props.messageStore.applyBlockPage(threadId, page, cursor);
    },
    onError: (error) => setFeedback(error.message)
  });
  useLayoutEffect(() => {
    const element = stream.current;
    if (!element) return;
    const state = scrollState.current;
    if (state.threadId !== props.threadId) {
      state.threadId = props.threadId;
      state.follow = true;
      state.prepend = null;
      setHistoryExpanded(false);
      setRenaming(false);
      setFeedback(null);
    }
    if (state.prepend !== null) {
      element.scrollTop = element.scrollHeight - state.prepend;
      state.prepend = null;
    } else if (state.follow) {
      element.scrollTop = element.scrollHeight;
    }
  }, [props.threadId, blocks, historyExpanded]);
  useLayoutEffect(() => {
    const anchor = viewportAnchor.current;
    const element = stream.current;
    if (element && anchor?.element.isConnected) {
      element.scrollTop += anchor.element.getBoundingClientRect().top - element.getBoundingClientRect().top - anchor.offset;
    }
    viewportAnchor.current = null;
  }, [selectedAgent, docked]);
  useEffect(() => {
    if (!shell.current || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(entries => {
      captureViewport();
      setDocked(entries[0].contentRect.width >= 960);
    });
    observer.observe(shell.current);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    const target = props.searchTarget;
    if (!target || target.sessionKey !== props.threadId || !props.searchReady || older.isPending) return;
    if (locateTimelineTarget(target.positionKey)) {
      props.onSearchResolved?.();
      return;
    }
    if (slot.hasMoreBlocks && slot.beforeCursor) {
      older.mutate({ threadId: props.threadId, cursor: slot.beforeCursor });
      return;
    }
    setFeedback("结果所在历史未加载，请刷新后重试");
    props.onSearchMiss?.();
  }, [props.searchTarget, props.threadId, props.searchReady, props.onSearchResolved, props.onSearchMiss, slot.hasMoreBlocks, slot.beforeCursor, older.isPending, blocks.length]);
  const copy = async (text: string | null | undefined) => {
    if (!text) return;
    try { await navigator.clipboard.writeText(text); setFeedback("已复制"); }
    catch { setFeedback("复制失败"); }
  };
  const archived = Boolean(props.archivedView) || summary.status === "Archived";
  return <div ref={shell} className="conversation-shell compact-readonly-conversation">
    <main className="conversation-main">
      <header className="conversation-header" data-timeline-id="session">
        <button className="icon-button mobile-back" title="返回任务列表" onClick={props.onBack}><ChevronLeft size={18} /></button>
          <div className="conversation-title-copy"><h1 className="conversation-title">{summary.title}</h1></div>
          <div className="conversation-header-actions">
          <span className={`status-chip ${summary.status}`}>{threadStatusLabel(summary.status)}</span>
          <TaskMenu triggerRef={deletion.returnFocus}>
              <button disabled={archived || actions.isPending} title={archived ? sharedDisabledStates.renameArchivedThread : undefined} onClick={() => { setTitle(summary.title); setRenaming(true); }}><Pencil size={15} />改名</button>
              <button disabled={actions.isPending} onClick={() => actions.mutate({ kind: archived ? "restore" : "archive", id: props.threadId })}>{archived ? <ArchiveRestore size={15} /> : <Archive size={15} />}{archived ? "取消归档" : "归档"}</button>
              <button onClick={() => copy(latestAssistantCopyText(blocks))}><Copy size={15} />复制答复</button>
              <button onClick={() => copy(detail.summary.id)}><Copy size={15} />复制 ID</button>
              <button onClick={() => copy(detail.summary.rollout_path)}><Copy size={15} />复制路径</button>
              <button onClick={() => copy(threadResumeCommand(detail.summary.id))}><Copy size={15} />复制恢复命令</button>
              <button className="danger-menu-item" disabled={actions.isPending || deletion.busy} onClick={() => deletion.prepareSingle(props.threadId, "delete", deletion.returnFocus.current)}><Trash2 size={15} />永久删除</button>
          </TaskMenu>
        </div>
      </header>
      <SessionBatchControls batch={deletion} operations={[]} showSelection={false} />
      {renaming && !archived && <form className="inline-rename" onSubmit={(event) => { event.preventDefault(); actions.mutate({ kind: "rename", id: props.threadId, title }); }}>
        <input aria-label="任务名称" maxLength={200} value={title} onChange={(event) => setTitle(event.target.value)} autoFocus />
        <button className="icon-button" title="保存名称" disabled={actions.isPending || !title.trim()}><Check size={17} /></button>
        <button type="button" className="icon-button" title="取消改名" onClick={() => setRenaming(false)}><X size={17} /></button>
      </form>}
      {actions.error && <div role="alert" className="form-error">{actions.error.message}</div>}
      {feedback && <div role="status" className="task-feedback">{feedback}</div>}
      <div className="timeline-reading-shell"><TimelineRail entries={timelineEntries} streamRef={stream} /><div ref={stream} className="message-stream readonly-message-stream" onScroll={(event) => { scrollState.current.follow = shouldAutoFollowMessageStream(event.currentTarget); }}>
        {needsSubagentUpgrade && <div className="muted-row" role="status">子智能体详情需要更新当前机器服务。</div>}
        {slot.hasMoreBlocks && slot.beforeCursor && <button className="secondary-button" disabled={older.isPending} onClick={() => older.mutate({ threadId: props.threadId, cursor: slot.beforeCursor! })}>较早消息</button>}
        {older.error && <div role="alert" className="form-error">{older.error.message}</div>}
        <UserMessageScope.Provider value={{ provider: "codex", sessionKey: summary.id }}><MarkdownPathScope.Provider value={summary.cwd}><DisclosureScope.Provider value={`codex:${props.threadId}`}>{visibleItems.map((entry) => entry.kind === "group" ? <ExecutionGroupView key={entry.group.id} group={entry.group} /> : <MessageBlockView key={entry.item.id} block={entry.item} onOpenSubagent={openAgent} subagentsSupported={props.capabilities.threadSubagents === true} planFallbackTitle={summary.title} historyExpanded={historyExpanded} onShowHistory={() => {
          if (stream.current) scrollState.current.prepend = stream.current.scrollHeight - stream.current.scrollTop;
          setHistoryExpanded(true);
        }} />)}</DisclosureScope.Provider></MarkdownPathScope.Provider></UserMessageScope.Provider>
        {!blocks.length && <div className="muted-row">暂无消息</div>}
      </div></div>
    </main>
    {selectedAgent && <SubagentPanel key={`${selectedAgent.machine}:${props.threadId}:${selectedAgent.agent.agentId}`} rootThreadId={props.threadId} initialAgent={selectedAgent.agent} docked={docked} onClose={closeAgent} />}
  </div>;
}
