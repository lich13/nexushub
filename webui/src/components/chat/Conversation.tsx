import { Bot, ClipboardCheck, MessageSquare, Play, Send, SlidersHorizontal, Square, TriangleAlert, X } from "lucide-react";
import { FormEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ComposerAttachmentList,
  SlashCommandTextarea,
  composerActionLabel,
  composerActionMode,
  composerActionTitle,
  composerSubmitDraftValue,
  slashCommandForComposerSubmit,
  useComposerAttachments
} from "../composer/ComposerControls";
import { CurrentActionCard } from "./CurrentActionCard";
import { ApprovalCard, MessageBlockView, UnsupportedApprovalCard } from "./MessageStream";
import { RunConfigControls } from "./RunConfigControls";
import { ThreadInspectorPanels } from "./ThreadInspectorPanels";
import { useConversationRealtimeController } from "../../hooks/useConversationController";
import { threadInspectorActionState } from "../../lib/domain/runtimeViewModel";
import {
  actionMessage,
  buildPayload,
  conversationTitleText,
  extractPlanText,
  isThreadRunning,
  makeRunConfig,
  mergeRunConfigFromDefaults,
  modelSupportsServiceTier,
  runConfigAfterSuccessfulSend,
  runConfigWithSupportedServiceTier,
  threadStatusLabel,
  type RunConfig,
  type SelectedThread,
  type View
} from "../../lib/domain/codexViewModel";
import {
  blocksWithCurrentPending,
  compactConversationBlocks,
  currentActionKey,
  currentActionKindFromBlocks,
  currentPendingElicitation,
  followUpMessagePreview,
  followUpStatusLabel,
  initialMessageBlockState,
  isActionablePlanBlock,
  isActionableQuestionBlock,
  isApprovalBlock,
  isPlanBlock,
  isResolvedActionBlock,
  isRunningToolBlock,
  isToolBlock,
  latestActionBlock,
  latestAssistantCopyText,
  mergeSavedThreadTitle,
  nextRenameDraftValue,
  pendingFromBlocks,
  prioritizeCurrentActionBlocks,
  roleLabel,
  segmentInternalReferences,
  shouldAutoFollowMessageStream,
  shouldRenderConversationMessage,
  shouldShowCurrentActionCard,
  visibleConversationBlocksForHistory,
  type MessageBlockState
} from "../../lib/domain/conversationViewModel";
import { slashCommandExecutionPlan } from "../../lib/domain/slashCommands";
import { useCodexConfigQuery, useCodexModelQuery, useCodexPermissionProfilesQuery } from "../../lib/query/codex";
import {
  useCreateThreadMutation,
  useFollowUpsQuery,
  usePluginsQuery,
  useThreadBlockPageMutation,
  useThreadConversationActions,
  type ThreadMessageSlot,
  type ThreadMessageStoreController
} from "../../lib/query/threads";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";
import type {
  BridgeActionResult,
  FollowUpQueueItem,
  MessageBlock,
  PendingElicitation,
  ThreadDetail,
  ThreadStatus,
  ThreadSummary
} from "../../types";

export {
  blockKindLabel,
  blocksWithCurrentPending,
  combinedQuestionAnswers,
  compactConversationBlocks,
  conversationMessagePresentation,
  currentActionKey,
  currentActionKindFromBlocks,
  currentPendingElicitation,
  currentPlanActionOptions,
  followUpMessagePreview,
  followUpStatusLabel,
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
  moveActionSelection,
  nextRenameDraftValue,
  pendingFromBlocks,
  planActionSubmission,
  planModeButtonState,
  prioritizeCurrentActionBlocks,
  questionAnswerPayload,
  questionAnswerLabels,
  questionAnswersReady,
  renderCurrentActionCardSnapshot,
  roleLabel,
  segmentInternalReferences,
  selectionFromDigitKey,
  shouldAutoFollowMessageStream,
  shouldRenderActionStackBlock,
  shouldRenderConversationBlock,
  shouldRenderConversationMessage,
  shouldShowCurrentActionCard,
  threadCopyId,
  threadInspectorPanelTitles,
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
  CurrentActionQuestion,
  InternalReferenceSegment,
  MessageBlockState,
  MessageScrollSnapshot,
  PlanActionSubmission
} from "../../lib/domain/conversationViewModel";

function useCodexRunOptions() {
  const models = useCodexModelQuery();
  const profiles = useCodexPermissionProfilesQuery();
  const config = useCodexConfigQuery();
  return {
    models: models.data?.available ? models.data.data ?? [] : [],
    profiles: profiles.data?.available ? profiles.data.data ?? [] : [],
    config: config.data?.available ? config.data.data : undefined,
    unavailable: {
      models: models.data && !models.data.available,
      profiles: profiles.data && !profiles.data.available,
      config: config.data && !config.data.available
    }
  };
}

export function Conversation({ threadId, detail, slot, messageStore, csrfToken, onSelect, onPanelSelect, nextThreadAfterArchive, capabilities }: {
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
  const messageStreamRef = useRef<HTMLDivElement | null>(null);
  const messageEndRef = useRef<HTMLDivElement | null>(null);
  const composerTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const shouldFollowMessagesRef = useRef(true);
  const {
    explicitBottomFollowRevision,
    previousThreadIdRef,
    updateMessageFollowState,
    followNextMessageUpdate
  } = useConversationRealtimeController({
    threadId,
    messageStore,
    messageStreamRef,
    shouldFollowMessagesRef,
    shouldAutoFollowMessageStream
  });
  const [draft, setDraft] = useState("");
  const runOptions = useCodexRunOptions();
  const pluginsQuery = usePluginsQuery();
  const [runConfig, setRunConfig] = useState<RunConfig>(() => makeRunConfig(undefined, detail.summary));
  const [renameValue, setRenameValue] = useState(detail.summary.title);
  const [renameDirty, setRenameDirty] = useState(false);
  const [toolsOpen, setToolsOpen] = useState(false);
  const attachComposerTextarea = useCallback((node: HTMLTextAreaElement | null) => {
    composerTextareaRef.current = node;
  }, []);
  const setActiveFeedback = useCallback((message: string | null) => {
    messageStore.setFeedback(threadId, message);
  }, [messageStore, threadId]);
  const attachments = useComposerAttachments(csrfToken, setActiveFeedback);
  const fallbackBlockState = initialMessageBlockState(detail);
  const summary = slot.summary ?? detail.summary;
  const blocks = slot.blocks.length ? slot.blocks : fallbackBlockState.blocks;
  const messageBlockState: MessageBlockState = {
    blocks,
    totalBlocks: slot.totalBlocks || fallbackBlockState.totalBlocks,
    hasMoreBlocks: slot.hasMoreBlocks || fallbackBlockState.hasMoreBlocks,
    beforeCursor: slot.beforeCursor ?? fallbackBlockState.beforeCursor,
    visibleUpdateRevision: slot.visibleUpdateRevision,
    bottomFollowRevision: slot.bottomFollowRevision || fallbackBlockState.bottomFollowRevision
  };
  const lastResult = slot.lastResult;
  const feedback = slot.feedback;
  const showAllHistory = slot.showAllHistory;
  const hiddenActionKey = slot.hiddenActionKey;
  const inspectorActions = threadInspectorActionState(capabilities);

  useEffect(() => {
    const defaults = makeRunConfig(runOptions.config, detail.summary);
    setRunConfig((current) => mergeRunConfigFromDefaults(current, defaults));
  }, [detail.summary, runOptions.config]);

  useEffect(() => {
    const sameThread = previousThreadIdRef.current === threadId;
    if (!sameThread) shouldFollowMessagesRef.current = true;
    setRenameValue((current) => {
      return nextRenameDraftValue({
        previousThreadId: previousThreadIdRef.current,
        threadId,
        currentDraft: current,
        incomingTitle: detail.summary.title,
        dirty: renameDirty
      });
    });
    if (!sameThread) {
      setRenameDirty(false);
      setDraft("");
      attachments.clearUploads();
      messageStore.setFeedback(threadId, null);
    }
    previousThreadIdRef.current = threadId;
  }, [detail.summary.title, messageStore, renameDirty, threadId]);

  const pending = useMemo(() => currentPendingElicitation(summary.pending_elicitation, summary.active_turn_id) ?? pendingFromBlocks(blocks, summary.status, summary.active_turn_id), [summary.pending_elicitation, summary.status, summary.active_turn_id, blocks]);
  const planBlock = useMemo(() => latestActionBlock(blocks, summary.status, summary.active_turn_id, isPlanBlock), [blocks, summary.status, summary.active_turn_id]);
  const approvalBlock = useMemo(() => latestActionBlock(blocks, summary.status, summary.active_turn_id, isApprovalBlock), [blocks, summary.status, summary.active_turn_id]);
  const currentActionKind = useMemo(() => currentActionKindFromBlocks(blocks, planBlock, pending), [blocks, planBlock, pending]);
  const currentActionPlan = currentActionKind === "plan" ? planBlock : null;
  const currentActionPending = currentActionKind === "question" ? pending : null;
  const currentActionId = currentActionKey(currentActionPlan, currentActionPending);
  const showCurrentActionCard = shouldShowCurrentActionCard(currentActionId, hiddenActionKey);
  const conversationSourceBlocks = useMemo(() => blocksWithCurrentPending(blocks, pending), [blocks, pending]);
  const visibleConversationBlocks = useMemo(() => (
    prioritizeCurrentActionBlocks(
      visibleConversationBlocksForHistory(conversationSourceBlocks, showAllHistory, planBlock, pending),
      planBlock,
      pending
    )
  ), [conversationSourceBlocks, showAllHistory, planBlock, pending]);

  useEffect(() => {
    if (!shouldFollowMessagesRef.current) return;
    requestAnimationFrame(() => {
      messageEndRef.current?.scrollIntoView({ block: "end" });
    });
  }, [messageBlockState.bottomFollowRevision, threadId, explicitBottomFollowRevision]);

  useEffect(() => {
    if (!currentActionId) messageStore.setHiddenActionKey(threadId, null);
  }, [currentActionId, messageStore, threadId]);

  const running = isThreadRunning(summary, blocks, lastResult);
  const canStop = running || Boolean(summary.active_turn_id || summary.active_job_id || lastResult?.turn_id || lastResult?.job_id);
  const actionMode = composerActionMode(running, draft, canStop, attachments.readyUploads.length);
  const followUps = useFollowUpsQuery(summary.id, running);
  const followUpItems = followUps.data?.items ?? [];
  const payloadRunConfig = useMemo(
    () => runConfigWithSupportedServiceTier(runConfig, runOptions.models),
    [runConfig, runOptions.models]
  );
  const loadEarlierMutation = useThreadBlockPageMutation({
    onBeforeLoad: (requestThreadId) => {
      const beforeHeight = messageStreamRef.current?.scrollHeight ?? 0;
      messageStore.setLoadingEarlier(requestThreadId, true);
      return beforeHeight;
    },
    onSuccess: ({ threadId: loadedThreadId, cursor, page, beforeHeight }) => {
      messageStore.applyBlockPage(loadedThreadId, page, cursor);
      requestAnimationFrame(() => {
        if (!messageStore.isActive(loadedThreadId)) return;
        const stream = messageStreamRef.current;
        if (!stream) return;
        stream.scrollTop += Math.max(0, stream.scrollHeight - beforeHeight);
      });
    },
    onError: (err, variables) => {
      const failedThreadId = variables?.threadId ?? summary.id;
      messageStore.setLoadingEarlier(failedThreadId, false, err.message);
      messageStore.setFeedback(failedThreadId, err.message);
    }
  });

  const threadActions = useThreadConversationActions({
    csrfToken,
    capabilities,
    messageStore,
    buildPayload,
    activeThreadId: summary.id,
    fallbackRenameTitle: detail.summary.title,
    nextThreadAfterArchive,
    onActiveMessageAccepted: () => {
      setDraft("");
      attachments.clearUploads();
      setRunConfig((current) => runConfigAfterSuccessfulSend(current));
    },
    onArchiveSelectionChange: onSelect,
    onRenameDraftCommitted: (title) => {
      setRenameValue(title);
      setRenameDirty(false);
    },
    onRenameDraftRestored: (title) => {
      setRenameValue(title);
      setRenameDirty(false);
    },
    onForkedThread: onSelect
  });

  const sendMutation = threadActions.send;
  const stopMutation = threadActions.stop;
  const steerMutation = threadActions.steer;
  const followUpCancelMutation = threadActions.followUpCancel;
  const archiveMutation = threadActions.archive;
  const renameMutation = threadActions.rename;
  const forkMutation = threadActions.fork;
  const answerMutation = threadActions.answer;
  const planAcceptMutation = threadActions.planAccept;
  const planReviseMutation = threadActions.planRevise;
  const approvalMutation = threadActions.approval;
  const executeSlashCommand = useCallback((command: string) => {
    const plan = slashCommandExecutionPlan({
      command,
      hasThread: Boolean(threadId),
      capabilities,
      inspectorActions,
      supportsFast: modelSupportsServiceTier(runOptions.models, runConfig.model, "priority"),
      serviceTier: runConfig.serviceTier,
      latestAssistantCopy: latestAssistantCopyText(blocks)
    });
    setDraft(plan.draft);
    switch (plan.kind) {
      case "toggle_plan_mode":
        setRunConfig((current) => ({
          ...current,
          collaborationMode: current.collaborationMode === "plan" ? "" : "plan"
        }));
        messageStore.setFeedback(threadId, plan.message);
        break;
      case "open_plugins":
        onPanelSelect("claude");
        messageStore.setFeedback(threadId, plan.message);
        break;
      case "open_status":
        onPanelSelect("codex");
        messageStore.setFeedback(threadId, plan.message);
        break;
      case "open_new_thread":
        onSelect("__new");
        if (plan.message) messageStore.setFeedback(threadId, plan.message);
        break;
      case "open_resume":
        onPanelSelect("codex");
        messageStore.setFeedback(threadId, plan.message);
        break;
      case "open_thread_settings":
        messageStore.setFeedback(threadId, plan.message);
        break;
      case "archive_thread":
        archiveMutation.mutate({ threadId: summary.id, status: summary.status });
        break;
      case "fork_thread":
        forkMutation.mutate({ threadId: summary.id });
        break;
      case "stop_thread":
        stopMutation.mutate({
          threadId: summary.id,
          turnId: lastResult?.turn_id ?? summary.active_turn_id,
          jobId: lastResult?.job_id ?? summary.active_job_id
        });
        break;
      case "copy_latest":
        navigator.clipboard?.writeText(plan.text);
        messageStore.setFeedback(threadId, plan.message);
        break;
      case "toggle_fast":
        setRunConfig({ ...runConfig, serviceTier: plan.serviceTier });
        messageStore.setFeedback(threadId, plan.message);
        break;
      case "insert_template":
        messageStore.setFeedback(threadId, plan.message);
        break;
      case "feedback":
      default:
        messageStore.setFeedback(threadId, plan.message);
        break;
    }
  }, [archiveMutation, blocks, capabilities, forkMutation, inspectorActions, lastResult?.job_id, lastResult?.turn_id, messageStore, onPanelSelect, onSelect, runConfig, runOptions.models, stopMutation, summary.id, summary.status, summary.active_job_id, summary.active_turn_id, threadId]);

  const loadEarlierPending = slot.loadingEarlier;
  const sendPending = sendMutation.isPending && sendMutation.variables?.threadId === summary.id;
  const stopPending = stopMutation.isPending && stopMutation.variables?.threadId === summary.id;
  const steerPending = steerMutation.isPending && steerMutation.variables?.threadId === summary.id;
  const followUpCancelPending = followUpCancelMutation.isPending && followUpCancelMutation.variables?.threadId === summary.id;
  const forkPending = forkMutation.isPending && forkMutation.variables?.threadId === summary.id;
  const renamePending = renameMutation.isPending && renameMutation.variables?.threadId === summary.id;
  const archivePending = archiveMutation.isPending && archiveMutation.variables?.threadId === summary.id;
  const answerPending = answerMutation.isPending && answerMutation.variables?.threadId === summary.id;
  const planAcceptPending = planAcceptMutation.isPending && planAcceptMutation.variables?.threadId === summary.id;
  const planRevisePending = planReviseMutation.isPending && planReviseMutation.variables?.threadId === summary.id;
  const approvalPending = approvalMutation.isPending && approvalMutation.variables?.threadId === summary.id;

  const submitComposer = useCallback((domValue?: string | null) => {
    if (attachments.uploadInProgress) return;
    const currentDraft = composerSubmitDraftValue(draft, domValue ?? composerTextareaRef.current?.value);
    if (currentDraft !== draft) setDraft(currentDraft);
    const exactSlash = slashCommandForComposerSubmit(currentDraft, capabilities);
    if (exactSlash) {
      executeSlashCommand(exactSlash);
      return;
    }
    if (actionMode === "send" && !sendPending) {
      followNextMessageUpdate();
      sendMutation.mutate({
        threadId: summary.id,
        message: currentDraft,
        config: payloadRunConfig,
        uploads: [...attachments.readyUploads]
      });
    } else if (actionMode === "followup" && !steerPending) {
      followNextMessageUpdate();
      steerMutation.mutate({
        threadId: summary.id,
        message: currentDraft,
        config: payloadRunConfig,
        uploads: [...attachments.readyUploads]
      });
    } else if (actionMode === "stop" && !stopPending) {
      stopMutation.mutate({
        threadId: summary.id,
        turnId: lastResult?.turn_id ?? summary.active_turn_id,
        jobId: lastResult?.job_id ?? summary.active_job_id
      });
    }
  }, [actionMode, attachments, capabilities, draft, executeSlashCommand, followNextMessageUpdate, lastResult?.job_id, lastResult?.turn_id, payloadRunConfig, sendMutation, sendPending, steerMutation, steerPending, stopMutation, stopPending, summary.active_job_id, summary.active_turn_id, summary.id]);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    submitComposer();
  };
  const conversationTitle = conversationTitleText(summary);
  const actionBusy = sendPending || stopPending || steerPending || attachments.uploadInProgress;
  const actionLabel = composerActionLabel(actionMode);
  const actionTitle = composerActionTitle(actionMode);
  const inspectorPanels = (
    <ThreadInspectorPanels
      summary={summary}
      csrfToken={csrfToken}
      renameValue={renameValue}
      setRenameValue={setRenameValue}
      setRenameDirty={setRenameDirty}
      renamePending={renamePending}
      onRename={() => renameMutation.mutate({ threadId: summary.id, title: renameValue })}
      archivePending={archivePending}
      onArchive={() => archiveMutation.mutate({ threadId: summary.id, status: summary.status })}
      forkPending={forkPending}
      onFork={() => forkMutation.mutate({ threadId: summary.id })}
      showFork={inspectorActions.showFork}
      showArchive={inspectorActions.showArchive}
      onFeedback={setActiveFeedback}
    />
  );

  return (
    <div className="conversation-shell">
      <div className="conversation-main">
        <header className="conversation-header">
          <div className="conversation-title-copy">
            <h2 className="conversation-title" title={conversationTitle}>{conversationTitle}</h2>
          </div>
          <div className="header-actions">
            <StatusChip status={summary.status} />
            <button className="secondary-button thread-tools-button" onClick={() => setToolsOpen(true)} type="button">
              <SlidersHorizontal size={17} />线程工具
            </button>
            <button
              className="icon-button"
              disabled={!canStop || stopPending}
              onClick={() => stopMutation.mutate({
                threadId: summary.id,
                turnId: lastResult?.turn_id ?? summary.active_turn_id,
                jobId: lastResult?.job_id ?? summary.active_job_id
              })}
              title="停止当前 turn"
            >
              <Square size={17} />
            </button>
          </div>
        </header>

        {toolsOpen && (
          <div className="thread-tools-backdrop" onClick={() => setToolsOpen(false)}>
            <aside className="thread-tools-drawer" onClick={(event) => event.stopPropagation()}>
              <div className="drawer-title-row">
                <strong>线程工具</strong>
                <button className="icon-button compact" onClick={() => setToolsOpen(false)} title="关闭线程工具" type="button"><X size={16} /></button>
              </div>
              {inspectorPanels}
            </aside>
          </div>
        )}

        {summary.status === "ReplyNeeded" && (
          <div className="reply-banner">
            <TriangleAlert size={18} />
            <span>{pending ? "Plan Mode 正在等待选择。" : "Plan Mode 正在等待确认。"}</span>
          </div>
        )}
        {feedback && <div className="feedback-banner">{feedback}</div>}

        {approvalBlock && (
          <div className="action-stack">
            {inspectorActions.approvalMode === "unsupported" ? (
              <UnsupportedApprovalCard block={approvalBlock} />
            ) : (
              <ApprovalCard
                key={`approval-${approvalBlock.id}`}
                block={approvalBlock}
                onDecision={(decision) => {
                  followNextMessageUpdate();
                  approvalMutation.mutate({ threadId: summary.id, block: approvalBlock, decision });
                }}
                pending={approvalPending}
              />
            )}
          </div>
        )}

        <div className="message-stream" ref={messageStreamRef} onScroll={updateMessageFollowState}>
          {messageBlockState.hasMoreBlocks && (
            <button
              className="load-earlier-button"
              disabled={slot.loadingEarlier || !messageBlockState.beforeCursor}
              onClick={() => {
                if (!messageBlockState.beforeCursor) return;
                loadEarlierMutation.mutate({ threadId: summary.id, cursor: messageBlockState.beforeCursor });
              }}
              type="button"
            >
              {slot.loadingEarlier ? "正在加载..." : "加载更早消息"}
            </button>
          )}
          {visibleConversationBlocks.map((block) => (
            <MessageBlockView
              key={block.id}
              block={block}
              activePlan={isActionablePlanBlock(block, planBlock)}
              planPending={planAcceptPending || planRevisePending}
              activeQuestion={isActionableQuestionBlock(block, pending)}
              questionPending={answerPending}
              onShowHistory={() => messageStore.setHistoryExpanded(threadId, true)}
              historyExpanded={showAllHistory}
            />
          ))}
          {visibleConversationBlocks.length === 0 && !approvalBlock && !planBlock && !pending && <div className="muted-row">没有可展示的 rollout 消息。</div>}
          <div ref={messageEndRef} aria-hidden="true" />
        </div>

        {showCurrentActionCard && (currentActionPlan || currentActionPending) && (
          <CurrentActionCard
            plan={currentActionPlan}
            pending={currentActionPending}
            onAcceptPlan={(block) => {
              followNextMessageUpdate();
              planAcceptMutation.mutate({ threadId: summary.id, block });
            }}
            onRevisePlan={(block, instructions) => {
              followNextMessageUpdate();
              planReviseMutation.mutate({ threadId: summary.id, block, instructions });
            }}
            planPending={planAcceptPending || planRevisePending}
            onSubmitQuestion={(answers) => {
              followNextMessageUpdate();
              answerMutation.mutate({ threadId: summary.id, answers });
            }}
            questionPending={answerPending}
            onDismiss={() => messageStore.setHiddenActionKey(threadId, currentActionId)}
          />
        )}

        <form className="composer" onSubmit={submit}>
          <input
            ref={attachments.inputRef}
            className="visually-hidden"
            type="file"
            multiple
            onChange={attachments.onFileInputChange}
          />
          <SlashCommandTextarea
            inputRef={attachComposerTextarea}
            value={draft}
            onChange={setDraft}
            placeholder={summary.status === "ReplyNeeded" ? "输入选择编号、确认语句或补充要求" : "发送给 Codex"}
            hasThread
            plugins={pluginsQuery.data ?? []}
            pluginsUnavailable={pluginsQuery.isError}
            capabilities={capabilities}
            onSlashCommand={executeSlashCommand}
            onSubmitShortcut={submitComposer}
          />
          <ComposerAttachmentList
            uploads={attachments.uploads}
            removingUploadId={attachments.removingUploadId}
            onRemove={attachments.removeUpload}
          />
          <RunConfigControls
            config={runConfig}
            setConfig={setRunConfig}
            models={runOptions.models}
            profiles={runOptions.profiles}
            unavailable={runOptions.unavailable}
            onPickFiles={attachments.openPicker}
            uploadInProgress={attachments.uploadInProgress}
            threadStatus={summary.status}
            hasPendingPlan={Boolean(planBlock)}
            hasPendingQuestion={Boolean(pending)}
          />
          {followUpItems.length > 0 && (
            <FollowUpQueue
              items={followUpItems}
              onCancel={(item) => followUpCancelMutation.mutate({ threadId: summary.id, followUpId: item.id })}
              cancelling={followUpCancelPending}
            />
          )}
          <div className="composer-actions">
            <span>{feedback || (lastResult ? actionMessage(lastResult) : "")}</span>
            <button className="primary-button composer-action-button" disabled={actionMode === "disabled" || actionBusy} title={actionTitle}>
              {actionMode === "stop" ? <Square size={17} /> : actionMode === "followup" ? <MessageSquare size={17} /> : <Send size={17} />}
              {actionLabel}
            </button>
          </div>
        </form>
      </div>

      <aside className="conversation-inspector">
        {inspectorPanels}
      </aside>
    </div>
  );
}

function FollowUpQueue({ items, onCancel, cancelling }: { items: FollowUpQueueItem[]; onCancel: (item: FollowUpQueueItem) => void; cancelling: boolean }) {
  const visible = items.filter((item) => item.status !== "submitted" || item.submitted_at);
  if (!visible.length) return null;
  return (
    <div className="follow-up-queue">
      {visible.slice(0, 4).map((item) => (
        <div className="follow-up-item" key={item.id}>
          <div className="follow-up-copy">
            <span>{followUpStatusLabel(item.status)}</span>
            <strong>{followUpMessagePreview(item)}</strong>
          </div>
          {item.status === "pending" && (
            <button type="button" className="icon-button compact" disabled={cancelling} onClick={() => onCancel(item)} title="取消跟进">
              <X size={15} />
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

export function EmptyConversation({ loading, csrfToken, onCreated, onPanelSelect, capabilities }: {
  loading: boolean;
  csrfToken?: string | null;
  onCreated: (id: string) => void;
  onPanelSelect: (view: View) => void;
  capabilities: RuntimeCapabilityMatrix;
}) {
  const composerTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const [draft, setDraft] = useState("");
  const runOptions = useCodexRunOptions();
  const pluginsQuery = usePluginsQuery();
  const [runConfig, setRunConfig] = useState<RunConfig>(() => makeRunConfig());
  const [result, setResult] = useState<BridgeActionResult | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const attachments = useComposerAttachments(csrfToken, setFeedback);
  useEffect(() => {
    if (runOptions.config) {
      const defaults = makeRunConfig(runOptions.config);
      setRunConfig((current) => mergeRunConfigFromDefaults(current, defaults));
    }
  }, [runOptions.config]);
  const payloadRunConfig = useMemo(
    () => runConfigWithSupportedServiceTier(runConfig, runOptions.models),
    [runConfig, runOptions.models]
  );
  const mutation = useCreateThreadMutation({
    csrfToken,
    payload: (message) => buildPayload(message, payloadRunConfig, attachments.readyUploads),
    onSuccess: (next) => {
      setResult(next);
      setDraft("");
      attachments.clearUploads();
      setRunConfig((current) => runConfigAfterSuccessfulSend(current));
      if (next.thread_id) onCreated(next.thread_id);
    },
    onError: (err: Error) => setFeedback(err.message)
  });
  const executeSlashCommand = (command: string) => {
    const plan = slashCommandExecutionPlan({
      command,
      hasThread: false,
      capabilities,
      inspectorActions: threadInspectorActionState(capabilities),
      supportsFast: modelSupportsServiceTier(runOptions.models, runConfig.model, "priority"),
      serviceTier: runConfig.serviceTier,
      latestAssistantCopy: null
    });
    setDraft(plan.draft);
    if (plan.kind === "toggle_plan_mode") {
      setRunConfig((current) => ({
        ...current,
        collaborationMode: current.collaborationMode === "plan" ? "" : "plan"
      }));
      setFeedback(plan.message);
      return;
    }
    if (plan.kind === "open_new_thread") {
      setFeedback(plan.message ?? "已经在新线程输入框");
      return;
    }
    if (plan.kind === "open_plugins") {
      onPanelSelect("claude");
      setFeedback(plan.message);
      return;
    }
    if (plan.kind === "open_status" || plan.kind === "open_resume") {
      onPanelSelect("codex");
      setFeedback(plan.message);
      return;
    }
    if (plan.kind === "toggle_fast") {
      setRunConfig({ ...runConfig, serviceTier: plan.serviceTier });
      setFeedback(plan.message);
      return;
    }
    setFeedback("message" in plan ? plan.message : "该命令需要已有线程");
  };
  const submitComposer = (domValue?: string | null) => {
    const currentDraft = composerSubmitDraftValue(draft, domValue ?? composerTextareaRef.current?.value);
    if (currentDraft !== draft) setDraft(currentDraft);
    const exactSlash = slashCommandForComposerSubmit(currentDraft, capabilities);
    if (exactSlash) {
      executeSlashCommand(exactSlash);
      return;
    }
    if (!attachments.uploadInProgress && (currentDraft.trim() || attachments.readyUploads.length)) {
      mutation.mutate({ message: currentDraft });
    }
  };
  if (loading) {
    return <div className="empty-state"><Bot size={32} /><strong>正在读取线程</strong></div>;
  }
  return (
    <div className="new-thread-state">
      <Bot size={34} />
      <strong>新建 Codex 线程</strong>
      <span>通过受控 Codex job 启动，并在任务历史中记录。</span>
      <form className="composer new-composer" onSubmit={(event) => {
        event.preventDefault();
        submitComposer();
      }}>
        <input
          ref={attachments.inputRef}
          className="visually-hidden"
          type="file"
          multiple
          onChange={attachments.onFileInputChange}
        />
        <SlashCommandTextarea
          inputRef={(node) => {
            composerTextareaRef.current = node;
          }}
          value={draft}
          onChange={setDraft}
          placeholder="输入第一条消息"
          hasThread={false}
          plugins={pluginsQuery.data ?? []}
          pluginsUnavailable={pluginsQuery.isError}
          capabilities={capabilities}
          onSlashCommand={executeSlashCommand}
          onSubmitShortcut={submitComposer}
        />
        <ComposerAttachmentList
          uploads={attachments.uploads}
          removingUploadId={attachments.removingUploadId}
          onRemove={attachments.removeUpload}
        />
        <RunConfigControls
          config={runConfig}
          setConfig={setRunConfig}
          models={runOptions.models}
          profiles={runOptions.profiles}
          unavailable={runOptions.unavailable}
          onPickFiles={attachments.openPicker}
          uploadInProgress={attachments.uploadInProgress}
          threadStatus="Recent"
        />
        <div className="composer-actions">
          <span>{feedback ?? (result ? actionMessage(result) : "新线程会在列表中自动出现")}</span>
          <button className="primary-button" disabled={(!draft.trim() && !attachments.readyUploads.length) || attachments.uploadInProgress || mutation.isPending}><Play size={17} />启动</button>
        </div>
      </form>
    </div>
  );
}

function StatusChip({ status }: { status: ThreadStatus }) {
  return <span className={`status-chip ${status}`}>{threadStatusLabel(status)}</span>;
}
