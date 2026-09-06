import { Archive, ArchiveRestore, Check, Copy, MoreHorizontal, Pencil, X } from "lucide-react";
import { useLayoutEffect, useRef, useState } from "react";
import { MessageBlockView } from "./MessageStream";
import { useReadOnlyThreadActions, useThreadBlockPageMutation, type ThreadMessageSlot, type ThreadMessageStoreController } from "../../lib/query/threads";
import { threadStatusLabel, type SelectedThread, type View } from "../../lib/domain/codexViewModel";
import { latestAssistantCopyText, shouldAutoFollowMessageStream, threadResumeCommand, visibleConversationBlocksForHistory } from "../../lib/domain/conversationViewModel";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";
import type { ThreadDetail } from "../../types";

export {
  blockKindLabel,
  blocksWithCurrentPending,
  compactConversationBlocks,
  conversationMessagePresentation,
  currentActionKey,
  currentActionKindFromBlocks,
  currentPendingElicitation,
  formatPayload,
  formatTime,
  historyCollapseKind,
  initialMessageBlockState,
  isActionablePlanBlock,
  isActionableQuestionBlock,
  isApprovalBlock,
  isHistoryCollapsedBlock,
  isPlanBlock,
  isQuestionBlock,
  isQuestionResultBlock,
  isResolvedActionBlock,
  isRunningToolBlock,
  isToolBlock,
  latestActionBlock,
  latestAssistantCopyText,
  messageBlockText,
  mergeSavedThreadTitle,
  nextRenameDraftValue,
  pendingFromBlocks,
  prioritizeCurrentActionBlocks,
  questionAnswerLabels,
  roleLabel,
  segmentInternalReferences,
  shouldAutoFollowMessageStream,
  shouldRenderActionStackBlock,
  shouldRenderConversationBlock,
  shouldRenderConversationMessage,
  shouldShowCurrentActionCard,
  threadCopyId,
  threadResumeCommand,
  threadRolloutPath,
  toolBlockDetailText,
  toolBlockStatus,
  toolBlockSummary,
  toolBlockTitle,
  visibleConversationBlocksForHistory
} from "../../lib/domain/conversationViewModel";
export type {
  ConversationMessagePresentation,
  CurrentActionKind,
  InternalReferenceSegment,
  MessageBlockState,
  MessageScrollSnapshot,
} from "../../lib/domain/conversationViewModel";


export function Conversation(props: {
  threadId: string;
  detail: ThreadDetail;
  slot: ThreadMessageSlot;
  messageStore: ThreadMessageStoreController;
  csrfToken?: string | null;
  onSelect: (id: SelectedThread) => void;
  onPanelSelect: (view: View) => void;
  nextThreadAfterArchive: string | null;
  capabilities: RuntimeCapabilityMatrix;
}) {
  const { detail, slot, csrfToken } = props;
  const [renaming, setRenaming] = useState(false);
  const [title, setTitle] = useState("");
  const [feedback, setFeedback] = useState<string | null>(null);
  const [historyExpanded, setHistoryExpanded] = useState(false);
  const stream = useRef<HTMLDivElement>(null);
  const scrollState = useRef({ threadId: "", follow: true, prepend: null as number | null });
  const blocks = slot.blocks.length ? slot.blocks : detail.blocks;
  const visibleBlocks = visibleConversationBlocksForHistory(blocks, historyExpanded);
  const actions = useReadOnlyThreadActions({ csrfToken, onSuccess: () => setRenaming(false) });
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
  const copy = async (text: string | null | undefined) => {
    if (!text) return;
    try { await navigator.clipboard.writeText(text); setFeedback("已复制"); }
    catch { setFeedback("复制失败"); }
  };
  const archived = detail.summary.status === "Archived";
  return <div className="conversation-shell compact-readonly-conversation">
    <main className="conversation-main">
      <header className="conversation-header">
        <div className="conversation-title-copy"><h1 className="conversation-title">{detail.summary.title}</h1><span className="muted-text">{detail.summary.cwd ?? detail.summary.id}</span></div>
        <div className="conversation-header-actions">
          <span className={`status-chip ${detail.summary.status}`}>{threadStatusLabel(detail.summary.status)}</span>
          <details className="task-menu"><summary className="icon-button" aria-label="任务操作" title="任务操作"><MoreHorizontal size={18} /></summary>
            <div className="task-menu-items">
              <button onClick={() => { setTitle(detail.summary.title); setRenaming(true); }}><Pencil size={15} />改名</button>
              <button disabled={actions.isPending} onClick={() => actions.mutate({ kind: archived ? "restore" : "archive", id: props.threadId })}>{archived ? <ArchiveRestore size={15} /> : <Archive size={15} />}{archived ? "取消归档" : "归档"}</button>
              <button onClick={() => copy(latestAssistantCopyText(blocks))}><Copy size={15} />复制答复</button>
              <button onClick={() => copy(detail.summary.id)}><Copy size={15} />复制 ID</button>
              <button onClick={() => copy(detail.summary.rollout_path)}><Copy size={15} />复制路径</button>
              <button onClick={() => copy(threadResumeCommand(detail.summary.id))}><Copy size={15} />复制恢复命令</button>
            </div>
          </details>
        </div>
      </header>
      {renaming && <form className="inline-rename" onSubmit={(event) => { event.preventDefault(); actions.mutate({ kind: "rename", id: props.threadId, title }); }}>
        <input aria-label="任务名称" maxLength={200} value={title} onChange={(event) => setTitle(event.target.value)} autoFocus />
        <button className="icon-button" title="保存名称" disabled={actions.isPending || !title.trim()}><Check size={17} /></button>
        <button type="button" className="icon-button" title="取消改名" onClick={() => setRenaming(false)}><X size={17} /></button>
      </form>}
      {actions.error && <div role="alert" className="form-error">{actions.error.message}</div>}
      {feedback && <div role="status" className="task-feedback">{feedback}</div>}
      <div ref={stream} className="message-stream readonly-message-stream" onScroll={(event) => { scrollState.current.follow = shouldAutoFollowMessageStream(event.currentTarget); }}>
        {slot.hasMoreBlocks && slot.beforeCursor && <button className="secondary-button" disabled={older.isPending} onClick={() => older.mutate({ threadId: props.threadId, cursor: slot.beforeCursor! })}>较早消息</button>}
        {older.error && <div role="alert" className="form-error">{older.error.message}</div>}
        {visibleBlocks.map((block) => <MessageBlockView key={block.id} block={block} historyExpanded={historyExpanded} onShowHistory={() => {
          if (stream.current) scrollState.current.prepend = stream.current.scrollHeight - stream.current.scrollTop;
          setHistoryExpanded(true);
        }} />)}
        {!blocks.length && <div className="muted-row">暂无消息</div>}
      </div>
    </main>
  </div>;
}
