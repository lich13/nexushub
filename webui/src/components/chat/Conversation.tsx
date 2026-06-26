import {
  Archive,
  Bot,
  CheckCircle2,
  ChevronRight,
  ClipboardCheck,
  Copy,
  Edit3,
  Files,
  GitFork,
  Lock,
  MessageSquare,
  Play,
  Plus,
  RefreshCw,
  Send,
  ShieldCheck,
  SlidersHorizontal,
  Square,
  TerminalSquare,
  Trash2,
  TriangleAlert,
  Undo2,
  X
} from "lucide-react";
import { FormEvent, ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Metric, Panel } from "../common/Panel";
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
import { useConversationRealtimeController } from "../../hooks/useConversationController";
import {
  formatGoalTimestamp,
  goalControlState,
  goalStatusLabel,
  goalStatusTone,
  threadInspectorActionState
} from "../../lib/domain/runtimeViewModel";
import {
  actionMessage,
  applyPermissionPreset,
  buildPayload,
  conversationTitleText,
  extractPlanText,
  isThreadRunning,
  makeRunConfig,
  mergeIncomingThreadSummary,
  mergeRunConfigFromDefaults,
  modelSupportsServiceTier,
  reasoningOptions,
  runConfigAfterSuccessfulSend,
  runConfigWithSupportedServiceTier,
  threadStatusLabel,
  type PermissionPresetId,
  type RunConfig,
  type SelectedThread,
  type View
} from "../../lib/domain/codexViewModel";
import { slashCommandExecutionPlan } from "../../lib/domain/slashCommands";
import { useCodexConfigQuery, useCodexModelQuery, useCodexPermissionProfilesQuery } from "../../lib/query/codex";
import {
  useCreateThreadMutation,
  useFollowUpsQuery,
  usePluginsQuery,
  useThreadBlockPageMutation,
  useThreadConversationActions,
  useThreadGoalActions,
  useThreadGoalQuery,
  type ThreadMessageSlot,
  type ThreadMessageStoreController
} from "../../lib/query/threads";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";
import type {
  BridgeActionResult,
  CodexGoal,
  CodexGoalSaveInput,
  CodexModel,
  FollowUpQueueItem,
  MessageBlock,
  PendingElicitation,
  PermissionProfile,
  ThreadDetail,
  ThreadStatus,
  ThreadSummary
} from "../../types";

type MessageScrollSnapshot = {
  scrollTop: number;
  clientHeight: number;
  scrollHeight: number;
};

type MessageBlockState = {
  blocks: MessageBlock[];
  totalBlocks: number;
  hasMoreBlocks: boolean;
  beforeCursor: string | null;
  visibleUpdateRevision: number;
  bottomFollowRevision: number;
};


const permissionPresets: Array<{ id: PermissionPresetId; label: string; description: string; icon: ReactNode }> = [
  { id: "ask", label: "请求批准", description: "编辑外部文件和使用互联网时始终询问", icon: <Lock size={17} /> },
  { id: "auto", label: "替我审批", description: "仅对检测到的风险操作请求批准", icon: <ShieldCheck size={17} /> },
  { id: "full", label: "完全访问权限", description: "可不受限制地访问互联网和文件", icon: <CheckCircle2 size={17} /> },
  { id: "custom", label: "自定义 (config.toml)", description: "使用 config.toml 中定义的权限", icon: <SlidersHorizontal size={17} /> }
];


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

export function shouldAutoFollowMessageStream(snapshot: MessageScrollSnapshot, threshold = 96): boolean {
  return snapshot.scrollHeight - snapshot.scrollTop - snapshot.clientHeight <= threshold;
}

function initialMessageBlockState(detail: ThreadDetail): MessageBlockState {
  const blocks = detail.blocks.length ? detail.blocks : legacyBlocks(detail);
  return {
    blocks,
    totalBlocks: detail.total_blocks ?? blocks.length,
    hasMoreBlocks: Boolean(detail.has_more_blocks),
    beforeCursor: detail.before_cursor ?? null,
    visibleUpdateRevision: 0,
    bottomFollowRevision: 0
  };
}

export function isRunningToolBlock(block: MessageBlock): boolean {
  if (!isToolBlock(block)) return false;
  const status = block.status?.trim();
  return Boolean(status && ["pending", "running", "in_progress", "inProgress", "active"].includes(status));
}


export type InternalReferenceSegment = {
  type: "text" | "internal_reference";
  text: string;
  copyText?: string;
  kind?: "path" | "thread" | "turn" | "job";
};

const internalReferencePattern = /((?:\/(?:Users|Volumes|home|root|tmp|var|opt|srv|etc|run|private)\/[^\s,，。；;）)]+)|\b(?:thread|turn|job)[\s:=#-]+[A-Za-z0-9._:-]{3,})/gi;

export function segmentInternalReferences(text: string): InternalReferenceSegment[] {
  const segments: InternalReferenceSegment[] = [];
  let lastIndex = 0;
  for (const match of text.matchAll(internalReferencePattern)) {
    const value = match[0];
    const index = match.index ?? 0;
    if (index > lastIndex) {
      segments.push({ type: "text", text: text.slice(lastIndex, index) });
    }
    segments.push({
      type: "internal_reference",
      text: value,
      copyText: value,
      kind: internalReferenceKind(value)
    });
    lastIndex = index + value.length;
  }
  if (lastIndex < text.length) {
    segments.push({ type: "text", text: text.slice(lastIndex) });
  }
  return segments.length ? segments : [{ type: "text", text }];
}

function internalReferenceKind(value: string): InternalReferenceSegment["kind"] {
  const lower = value.toLowerCase();
  if (lower.startsWith("/")) return "path";
  if (lower.startsWith("thread")) return "thread";
  if (lower.startsWith("turn")) return "turn";
  return "job";
}

export function planModeButtonState(nextMessagePlan: boolean, threadStatus?: string, hasPendingPlan = false, hasPendingQuestion = false): { pressed: boolean; label: string; statusText: string } {
  if (threadStatus === "ReplyNeeded" && hasPendingPlan) {
    return { pressed: nextMessagePlan, label: "Plan Mode", statusText: "当前线程正在等待计划确认" };
  }
  if (threadStatus === "ReplyNeeded" && hasPendingQuestion) {
    return { pressed: nextMessagePlan, label: "Plan Mode", statusText: "当前线程正在等待问题回复" };
  }
  return {
    pressed: nextMessagePlan,
    label: "Plan Mode",
    statusText: nextMessagePlan ? "下一条消息将使用 Plan Mode" : "下一条消息将直接发送"
  };
}

export function latestAssistantCopyText(blocks: MessageBlock[]): string | null {
  const latest = [...blocks].reverse().find((block) =>
    block.role === "assistant" && shouldRenderConversationMessage(block)
  );
  const text = latest ? messageBlockText(latest).trim() : "";
  return text || null;
}

export function nextRenameDraftValue(input: {
  previousThreadId: string;
  threadId: string;
  currentDraft: string;
  incomingTitle: string;
  dirty: boolean;
}): string {
  if (input.previousThreadId !== input.threadId) return input.incomingTitle;
  if (input.dirty) return input.currentDraft;
  const merged = mergeIncomingThreadSummary(
    { id: input.threadId, title: input.currentDraft },
    { id: input.threadId, title: input.incomingTitle }
  );
  return merged.title ?? input.currentDraft;
}

export function mergeSavedThreadTitle(threads: ThreadSummary[], threadId: string, title: string): ThreadSummary[] {
  return threads.map((thread) => thread.id === threadId ? { ...thread, title } : thread);
}

export function threadInspectorPanelTitles(): string[] {
  return ["名称与归档", "Goal", "复制与路径"];
}

export function threadResumeCommand(threadId?: string | null): string | null {
  const id = threadId?.trim();
  return id ? `codex resume ${id}` : null;
}

export function threadCopyId(threadId?: string | null): string | null {
  return threadId?.trim() || null;
}

export function threadRolloutPath(rolloutPath?: string | null): string | null {
  return rolloutPath?.trim() || null;
}

type CurrentActionKind = "plan" | "question";
type CurrentActionQuestion = PendingElicitation["questions"][number];
type PlanActionSubmission = { action: "accept" } | { action: "revise"; instructions: string } | { action: "keep_plan" };

export function currentActionKey(plan: MessageBlock | null | undefined, pending: PendingElicitation | null | undefined): string | null {
  if (plan) {
    return `plan:${plan.turn_id ?? "turn"}:${plan.item_id ?? plan.call_id ?? plan.id}`;
  }
  if (pending) {
    return `question:${pending.turn_id ?? "turn"}:${pending.item_id ?? pending.questions[0]?.id ?? "request"}`;
  }
  return null;
}

export function shouldShowCurrentActionCard(actionKey: string | null | undefined, hiddenActionKey: string | null | undefined): boolean {
  return Boolean(actionKey && actionKey !== hiddenActionKey);
}

export function selectionFromDigitKey(key: string, total: number): number | null {
  if (!/^[1-9]$/.test(key)) return null;
  const index = Number(key) - 1;
  return index >= 0 && index < total ? index : null;
}

export function moveActionSelection(current: number, total: number, delta: number): number {
  if (total <= 0) return 0;
  return (current + delta + total) % total;
}

export function currentPlanActionOptions(): { label: string; description: string }[] {
  return [
    { label: "接受计划", description: "按聊天记录里的 Proposed Plan 继续执行" },
    { label: "修改计划", description: "补充修改要求后重新生成计划" },
    { label: "保持计划模式", description: "不提交回复，继续让本线程使用 Plan Mode" }
  ];
}

export function planActionSubmission(selected: number, revision: string): PlanActionSubmission | null {
  if (selected === 0) return { action: "accept" };
  if (selected === 1 && revision.trim()) return { action: "revise", instructions: revision.trim() };
  if (selected === 2) return { action: "keep_plan" };
  return null;
}

export function questionAnswersReady(questions: CurrentActionQuestion[], answers: Record<string, string | string[] | undefined>): boolean {
  return questions.every((question) => {
    const value = answers[question.id];
    if (Array.isArray(value)) return value.some((item) => item.trim().length > 0);
    return typeof value === "string" && value.trim().length > 0;
  });
}

function combinedQuestionAnswers(
  questions: CurrentActionQuestion[],
  answers: Record<string, string | string[] | undefined>,
  notes: Record<string, string>
): Record<string, string[]> {
  return Object.fromEntries(questions.map((question) => {
    const answer = answers[question.id];
    const selected = Array.isArray(answer) ? answer : answer ? [answer] : [];
    const note = notes[question.id]?.trim();
    return [question.id, note ? [...selected, note] : selected];
  }));
}

export function questionAnswerPayload(questions: CurrentActionQuestion[], answers: Record<string, string | string[] | undefined>): Record<string, string[]> {
  return Object.fromEntries(questions.map((question) => {
    const value = answers[question.id];
    return [question.id, Array.isArray(value) ? value : value ? [value] : []];
  }));
}

export function renderCurrentActionCardSnapshot(input: {
  kind: CurrentActionKind;
  questions?: CurrentActionQuestion[];
}): { buttons: string[]; supplementalInput: boolean } {
  if (input.kind === "plan") {
    return {
      buttons: currentPlanActionOptions().map((option) => option.label),
      supplementalInput: false
    };
  }
  return {
    buttons: (input.questions ?? []).flatMap((question) => question.options.map((option) => option.label)),
    supplementalInput: true
  };
}

function currentActionKindFromBlocks(
  blocks: MessageBlock[],
  plan: MessageBlock | null | undefined,
  pending: PendingElicitation | null | undefined
): CurrentActionKind | null {
  if (!plan && !pending) return null;
  if (plan && !pending) return "plan";
  if (!plan && pending) return "question";
  const planIndex = blocks.findIndex((block) => isActionablePlanBlock(block, plan));
  const questionIndex = blocks.findIndex((block) => isActionableQuestionBlock(block, pending));
  if (planIndex === -1 && questionIndex === -1) return plan ? "plan" : "question";
  if (questionIndex === -1) return "plan";
  if (planIndex === -1) return "question";
  return questionIndex >= planIndex ? "question" : "plan";
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

function ThreadInspectorPanels({
  summary,
  csrfToken,
  renameValue,
  setRenameValue,
  setRenameDirty,
  renamePending,
  onRename,
  archivePending,
  onArchive,
  forkPending,
  onFork,
  showFork,
  showArchive,
  onFeedback
}: {
  summary: ThreadSummary;
  csrfToken?: string | null;
  renameValue: string;
  setRenameValue: (value: string) => void;
  setRenameDirty: (dirty: boolean) => void;
  renamePending: boolean;
  onRename: () => void;
  archivePending: boolean;
  onArchive: () => void;
  forkPending: boolean;
  onFork: () => void;
  showFork: boolean;
  showArchive: boolean;
  onFeedback: (message: string | null) => void;
}) {
  const copyText = useCallback((text: string | null, message: string) => {
    if (!text) return;
    navigator.clipboard?.writeText(text);
    onFeedback(message);
  }, [onFeedback]);
  const copyId = threadCopyId(summary.id);
  const rolloutPath = threadRolloutPath(summary.rollout_path);
  const resumeCommand = threadResumeCommand(summary.id);

  return (
    <>
      <Panel title="名称与归档" icon={<SlidersHorizontal size={18} />}>
        <label className="field-label">线程标题<input value={renameValue} onChange={(event) => {
          setRenameDirty(true);
          setRenameValue(event.target.value);
        }} /></label>
        <div className="button-row">
          <button className="secondary-button" onClick={onRename} disabled={!renameValue.trim() || renamePending}><Edit3 size={17} />重命名</button>
          <button className={summary.status === "Archived" ? "secondary-button" : "danger-button soft"} onClick={onArchive} disabled={archivePending || !showArchive}>
            {summary.status === "Archived" ? <Undo2 size={17} /> : <Archive size={17} />}
            {summary.status === "Archived" ? "恢复" : "归档"}
          </button>
        </div>
        {showFork && (
          <button className="secondary-button full-width-action" onClick={onFork} disabled={forkPending}>
            <GitFork size={17} />Fork
          </button>
        )}
      </Panel>

      <ThreadGoalPanel threadId={summary.id} csrfToken={csrfToken} onFeedback={onFeedback} />

      <Panel title="复制与路径" icon={<Files size={18} />}>
        <Metric label="线程 ID" value={copyId || "无"} wide />
        <Metric label="会话文件" value={rolloutPath || "无会话文件"} wide />
        <div className="copy-row">
          <button className="secondary-button" onClick={() => copyText(copyId, "已复制线程 ID")} disabled={!copyId}>
            <Copy size={17} />复制 ID
          </button>
          <button className="secondary-button" onClick={() => copyText(rolloutPath, "已复制文件路径")} disabled={!rolloutPath}>
            <Copy size={17} />复制文件路径
          </button>
          <button className="secondary-button" onClick={() => copyText(resumeCommand, "已复制 resume 命令")} disabled={!resumeCommand}>
            <TerminalSquare size={17} />复制 codex resume+ID
          </button>
        </div>
      </Panel>
    </>
  );
}

function ThreadGoalPanel({ threadId, csrfToken, onFeedback }: {
  threadId: string;
  csrfToken?: string | null;
  onFeedback: (message: string | null) => void;
}) {
  const goal = useThreadGoalQuery(threadId);
  const [objective, setObjective] = useState("");
  const [tokenBudget, setTokenBudget] = useState("");
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const currentGoal = goal.data;

  useEffect(() => {
    if (!currentGoal || dirty) return;
    setObjective(currentGoal.objective ?? "");
    setTokenBudget(currentGoal.token_budget === null || currentGoal.token_budget === undefined ? "" : String(currentGoal.token_budget));
  }, [currentGoal, dirty]);

  useEffect(() => {
    setDirty(false);
    setError(null);
  }, [threadId]);

  const afterGoalSuccess = useCallback((next: CodexGoal, message: string) => {
    setDirty(false);
    setObjective(next.objective ?? "");
    setTokenBudget(next.token_budget === null || next.token_budget === undefined ? "" : String(next.token_budget));
    setError(null);
    onFeedback(message);
  }, [onFeedback]);

  const onGoalError = useCallback((err: Error) => {
    setError(err.message);
    onFeedback(err.message);
  }, [onFeedback]);

  const goalActions = useThreadGoalActions({
    threadId,
    csrfToken,
    saveInput: () => goalSaveInput(objective, tokenBudget),
    onSuccess: afterGoalSuccess,
    onError: onGoalError
  });
  const saveGoalMutation = goalActions.save;
  const clearGoalMutation = goalActions.clear;
  const pauseGoalMutation = goalActions.pause;
  const resumeGoalMutation = goalActions.resume;

  const busy = saveGoalMutation.isPending || clearGoalMutation.isPending || pauseGoalMutation.isPending || resumeGoalMutation.isPending;
  const controls = goalControlState(currentGoal, { busy, objective, tokenBudget });
  const unavailable = currentGoal?.available === false;

  return (
    <Panel title="Goal" icon={<ClipboardCheck size={18} />}>
      <div className="settings-meta-grid">
        <Metric label="状态" value={goalStatusLabel(currentGoal, goal.isLoading)} tone={goalStatusTone(currentGoal)} />
        <Metric label="预算" value={currentGoal?.token_budget === null || currentGoal?.token_budget === undefined ? "无" : String(currentGoal.token_budget)} />
        {currentGoal?.completed_at ? <Metric label="完成时间" value={formatGoalTimestamp(currentGoal.completed_at)} /> : null}
        {currentGoal?.blocked_reason ? <Metric label="阻塞原因" value={currentGoal.blocked_reason} tone="danger" /> : null}
      </div>
      <label className="field-label">目标<input value={objective} onChange={(event) => {
        setDirty(true);
        setObjective(event.target.value);
      }} placeholder={goal.isLoading ? "正在读取 Goal" : "输入当前线程目标"} /></label>
      <label className="field-label">Token budget<input type="number" min={1} value={tokenBudget} onChange={(event) => {
        setDirty(true);
        setTokenBudget(event.target.value);
      }} placeholder="可选" /></label>
      <div className="button-row">
        <button className="primary-button" disabled={controls.saveDisabled || unavailable} onClick={() => saveGoalMutation.mutate()}><CheckCircle2 size={17} />保存</button>
        <button className="secondary-button" disabled={controls.clearDisabled || unavailable} onClick={() => clearGoalMutation.mutate()}><Trash2 size={17} />清除</button>
        <button className="secondary-button" disabled={controls.pauseDisabled || unavailable} onClick={() => pauseGoalMutation.mutate()}><Square size={17} />暂停</button>
        <button className="secondary-button" disabled={controls.resumeDisabled || unavailable} onClick={() => resumeGoalMutation.mutate()}><Play size={17} />恢复</button>
      </div>
      {error && <div className="form-error">{error}</div>}
      {unavailable && <div className="muted-row">Goal 接口未接入</div>}
    </Panel>
  );
}

function goalSaveInput(objective: string, tokenBudget: string): CodexGoalSaveInput {
  return {
    objective: objective.trim(),
    token_budget: tokenBudget.trim() ? Number.isFinite(Number(tokenBudget.trim())) && Number(tokenBudget.trim()) > 0 ? Math.floor(Number(tokenBudget.trim())) : null : null
  };
}

function RunConfigControls({ config, setConfig, models, unavailable, onPickFiles, uploadInProgress = false, threadStatus, hasPendingPlan = false, hasPendingQuestion = false }: {
  config: RunConfig;
  setConfig: (config: RunConfig) => void;
  models: CodexModel[];
  profiles: PermissionProfile[];
  unavailable: { models?: boolean; profiles?: boolean; config?: boolean };
  onPickFiles?: () => void;
  uploadInProgress?: boolean;
  threadStatus?: ThreadStatus | string;
  hasPendingPlan?: boolean;
  hasPendingQuestion?: boolean;
}) {
  const modelList = models.some((item) => item.id === config.model)
    ? models
    : config.model
      ? [{ id: config.model, label: config.model }, ...models]
      : models;
  const activePreset = permissionPresets.find((item) => item.id === config.permissionPreset) ?? permissionPresets[2];
  const supportsFast = modelSupportsServiceTier(modelList, config.model, "priority");
  const serviceTier = supportsFast ? config.serviceTier : "";
  const planButton = planModeButtonState(config.collaborationMode === "plan", threadStatus, hasPendingPlan, hasPendingQuestion);
  return (
    <div className="composer-config">
      <div className="composer-toolbar">
        <button
          type="button"
          className="composer-chip icon-only"
          title={uploadInProgress ? "附件上传中" : "上传本地文件"}
          onClick={onPickFiles}
          disabled={!onPickFiles || uploadInProgress}
        >
          <Plus size={15} />
        </button>
        {supportsFast && (
          <button
            type="button"
            className={serviceTier === "priority" ? "composer-chip active" : "composer-chip"}
            onClick={() => setConfig({ ...config, serviceTier: serviceTier === "priority" ? "" : "priority" })}
            title="使用 Codex priority service tier"
          >
            <RefreshCw size={15} />Fast
          </button>
        )}
        <button
          type="button"
          className={planButton.pressed ? "composer-chip active" : "composer-chip"}
          aria-pressed={planButton.pressed}
          title={planButton.statusText}
          onClick={() => setConfig({ ...config, collaborationMode: config.collaborationMode === "plan" ? "" : "plan" })}
        >
          <ClipboardCheck size={15} />{planButton.label}
        </button>
        <span className="composer-chip muted">{planButton.statusText}</span>
        <label className="permission-menu-trigger">
          <ShieldCheck size={15} />
          <select value={config.permissionPreset} onChange={(event) => setConfig(applyPermissionPreset(config, event.target.value as PermissionPresetId))}>
            {permissionPresets.map((preset) => <option key={preset.id} value={preset.id}>{preset.label}</option>)}
          </select>
        </label>
      </div>
      <div className="composer-grid main-config">
        <label>
          <span>模型</span>
          {modelList.length > 0 ? (
            <select value={config.model} onChange={(event) => {
              const model = event.target.value;
              setConfig({
                ...config,
                model,
                serviceTier: modelSupportsServiceTier(modelList, model, "priority") ? config.serviceTier : ""
              });
            }}>
              {modelList.map((item) => <option key={item.id} value={item.id}>{item.label ?? item.id}</option>)}
            </select>
          ) : (
            <input value={config.model} onChange={(event) => setConfig({ ...config, model: event.target.value })} placeholder={unavailable.models ? "模型接口不可用" : "model"} />
          )}
        </label>
        <label>
          <span>Reasoning</span>
          <select value={config.reasoning} onChange={(event) => setConfig({ ...config, reasoning: event.target.value })}>
            {reasoningOptions.map((value) => <option key={value || "default"} value={value}>{value || "default"}</option>)}
          </select>
        </label>
      </div>
      <div className="permission-summary">
        <div className="permission-summary-icon">{activePreset.icon}</div>
        <div>
          <strong>{activePreset.label}</strong>
          <span>{activePreset.description}</span>
        </div>
      </div>
      {unavailable.config && <div className="config-note">Codex 默认配置接口不可用，使用当前表单值发送。</div>}
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

export function followUpStatusLabel(status?: string | null): string {
  if (status === "pending") return "待跟进";
  if (status === "submitting") return "提交中";
  if (status === "submitted") return "已提交";
  if (status === "cancelled") return "已取消";
  if (status === "error") return "失败";
  return status || "未知";
}

export function followUpMessagePreview(item: Pick<FollowUpQueueItem, "message" | "error" | "status">): string {
  const source = item.status === "error" && item.error ? item.error : item.message;
  const compact = source.replace(/\s+/g, " ").trim();
  if (compact.length <= 120) return compact || "空跟进";
  return `${compact.slice(0, 120)}...`;
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

function MessageBlockView({
  block,
  activePlan = false,
  planPending = false,
  activeQuestion = false,
  questionPending = false,
  onShowHistory,
  historyExpanded = false
}: {
  block: MessageBlock;
  activePlan?: boolean;
  planPending?: boolean;
  activeQuestion?: boolean;
  questionPending?: boolean;
  onShowHistory?: () => void;
  historyExpanded?: boolean;
}) {
  if (isHistoryCollapsedBlock(block)) {
    return <HistoryCollapseCell block={block} onShowHistory={onShowHistory} expanded={historyExpanded} />;
  }
  if (isPlanBlock(block)) {
    return (
      <ProposedPlanCell
        block={block}
        active={activePlan}
        pending={planPending}
      />
    );
  }
  if (isQuestionBlock(block)) {
    if (activeQuestion) return <QuestionCell block={block} pendingSubmit={questionPending} />;
    return <QuestionResultCell block={block} />;
  }
  if (isQuestionResultBlock(block)) {
    return <QuestionResultCell block={block} />;
  }
  if (isToolBlock(block)) {
    return <ToolBlockView block={block} />;
  }
  if (!shouldRenderConversationMessage(block)) {
    return null;
  }
  const presentation = conversationMessagePresentation(block);
  return (
    <article className={presentation.rowClassName}>
      <div className="chat-meta">
        <span>{roleLabel(block.role)}</span>
        <small>{blockKindLabel(block.kind)}{block.created_at ? ` · ${formatTime(block.created_at)}` : ""}</small>
      </div>
      <div className={presentation.bodyClassName}>
        <MessageContent text={messageBlockText(block)} />
      </div>
    </article>
  );
}

function ToolBlockView({ block }: { block: MessageBlock }) {
  const [open, setOpen] = useState(false);
  const summary = toolBlockSummary(block);
  return (
    <details
      className={`tool-card ${isRunningToolBlock(block) ? "running" : ""}`}
      onToggle={(event) => setOpen((event.currentTarget as HTMLDetailsElement).open)}
    >
      <summary>
        <span className="tool-title">{toolBlockTitle(block)}</span>
        <small>{toolBlockStatus(block)}</small>
        <ChevronRight size={16} />
      </summary>
      {summary && <div className="tool-summary">{summary}</div>}
      {open && <pre>{toolBlockDetailText(block)}</pre>}
    </details>
  );
}

function MessageContent({ text }: { text: string }) {
  const [copied, setCopied] = useState<string | null>(null);
  const segments = useMemo(() => segmentInternalReferences(text), [text]);
  return (
    <>
      {segments.map((segment, index) => {
        if (segment.type === "text") {
          return <span key={`text-${index}`}>{segment.text}</span>;
        }
        return (
          <button
            key={`ref-${index}-${segment.text}`}
            type="button"
            className="internal-reference"
            title="复制内部引用"
            onClick={async () => {
              const copyText = segment.copyText ?? segment.text;
              await navigator.clipboard?.writeText(copyText);
              setCopied(copyText);
              window.setTimeout(() => setCopied((current) => current === copyText ? null : current), 1600);
            }}
          >
            {segment.text}
            {copied === (segment.copyText ?? segment.text) && <small>已复制</small>}
          </button>
        );
      })}
    </>
  );
}

function HistoryCollapseCell({ block, onShowHistory, expanded }: { block: MessageBlock; onShowHistory?: () => void; expanded: boolean }) {
  const kind = historyCollapseKind(block);
  const label = firstDisplayLine(block.summary) ?? firstDisplayLine(block.text) ?? (kind === "tool" ? "历史工具活动已折叠" : kind === "action" ? "历史计划和问题已折叠" : "较早消息已折叠");
  const eyebrow = kind === "tool" ? "Tool activity" : kind === "action" ? "Plan & questions" : "Earlier messages";
  return (
    <article className="history-collapse-cell">
      <div>
        <span>{eyebrow}</span>
        <strong>{label}</strong>
      </div>
      {onShowHistory && (
        <button className="secondary-button" disabled={expanded} onClick={onShowHistory} type="button">
          {expanded ? "已显示全部" : "显示全部历史"}
        </button>
      )}
    </article>
  );
}

function ProposedPlanCell({ block, active, pending }: { block: MessageBlock; active: boolean; pending: boolean }) {
  return (
    <article className={active ? "plan-cell active" : "plan-cell"}>
      <div className="message-meta">
        <span>Proposed Plan</span>
        <small>{block.plan_status || block.status || block.turn_id || block.item_id || block.kind}</small>
      </div>
      <div className="plan-body">{extractPlanText(block.text || "")}</div>
      {active && pending && <div className="action-inline-status">正在提交计划操作...</div>}
    </article>
  );
}

function QuestionResultCell({ block }: { block: MessageBlock }) {
  const answers = block.answers ?? [];
  return (
    <article className="question-result-cell">
      <div className="message-meta">
        <span>Questions</span>
        <small>{block.status || "completed"}</small>
      </div>
      {answers.length > 0 ? (
        <div className="answered-list">
          {answers.map((answer) => (
            <div className="answered-row" key={answer.question_id}>
              <span>{answer.question_id}</span>
              <strong>{answer.answers.length ? answer.answers.join(", ") : "未回答"}</strong>
              {answer.note && <small>{answer.note}</small>}
            </div>
          ))}
        </div>
      ) : (
        <p>Questions answered</p>
      )}
    </article>
  );
}

function QuestionCell({ block, pendingSubmit }: { block: MessageBlock; pendingSubmit: boolean }) {
  return (
    <article className="question-cell active-choice">
      <div className="message-meta">
        <span>Questions</span>
        <small>{block.turn_id || block.item_id || block.call_id || "request_user_input"}</small>
      </div>
      {block.questions.map((question) => (
        <div key={question.id} className="question-block">
          <strong>{question.question}</strong>
          <div className="choice-grid">
            {question.options.map((option, index) => (
              <button
                key={`${question.id}-${option.label}`}
                className="choice-option"
                disabled
                type="button"
              >
                <span>{index + 1}</span>
                <strong>{option.label}</strong>
                {option.description && <small>{option.description}</small>}
              </button>
            ))}
          </div>
        </div>
      ))}
      {pendingSubmit && <div className="action-inline-status">正在提交选择...</div>}
    </article>
  );
}

function CurrentActionCard({
  plan,
  pending,
  onAcceptPlan,
  onRevisePlan,
  planPending,
  onSubmitQuestion,
  questionPending,
  onDismiss
}: {
  plan?: MessageBlock | null;
  pending?: PendingElicitation | null;
  onAcceptPlan: (block: MessageBlock) => void;
  onRevisePlan: (block: MessageBlock, instructions: string) => void;
  planPending: boolean;
  onSubmitQuestion: (answers: Record<string, string[]>) => void;
  questionPending: boolean;
  onDismiss: () => void;
}) {
  const isPlan = Boolean(plan);
  const busy = isPlan ? planPending : questionPending;
  const questions = pending?.questions ?? [];
  const questionSignature = questions.map((question) => `${question.id}:${question.options.map((option) => option.label).join("|")}`).join(";");
  const [selected, setSelected] = useState(0);
  const [revision, setRevision] = useState("");
  const [questionAnswers, setQuestionAnswers] = useState<Record<string, string | string[] | undefined>>({});
  const [questionNotes, setQuestionNotes] = useState<Record<string, string>>({});
  const options = isPlan ? currentPlanActionOptions() : questions[0]?.options ?? [];
  const selectedPlanRequiresRevision = isPlan && selected === 1;
  const ready = isPlan
    ? Boolean(plan && planActionSubmission(selected, revision))
    : questionAnswersReady(questions, combinedQuestionAnswers(questions, questionAnswers, questionNotes));

  function submitAction() {
    if (busy || !ready) return;
    if (plan) {
      const submission = planActionSubmission(selected, revision);
      if (!submission) return;
      if (submission.action === "accept") {
        onAcceptPlan(plan);
      } else if (submission.action === "revise") {
        onRevisePlan(plan, submission.instructions);
      } else {
        onDismiss();
      }
      return;
    }
    if (pending) onSubmitQuestion(questionAnswerPayload(questions, combinedQuestionAnswers(questions, questionAnswers, questionNotes)));
  }

  useEffect(() => {
    setSelected(0);
    setRevision("");
    setQuestionAnswers((current) => {
      const initial: Record<string, string | string[] | undefined> = {};
      for (const question of questions) {
        initial[question.id] = current[question.id] ?? question.options[0]?.label;
      }
      return initial;
    });
    setQuestionNotes({});
  }, [plan?.id, pending?.turn_id, pending?.item_id, questionSignature]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const editable = target?.closest("input, textarea, select, [contenteditable='true']");
      if (event.key === "Escape") {
        event.preventDefault();
        onDismiss();
        return;
      }
      if (!editable && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
        event.preventDefault();
        setSelected((current) => moveActionSelection(current, options.length, event.key === "ArrowDown" ? 1 : -1));
        return;
      }
      if (!editable) {
        const digitSelection = selectionFromDigitKey(event.key, options.length);
        if (digitSelection !== null) {
          event.preventDefault();
          setSelected(digitSelection);
          if (!isPlan && questions[0]?.options[digitSelection]) {
            setQuestionAnswers((current) => ({ ...current, [questions[0].id]: questions[0].options[digitSelection].label }));
          }
          return;
        }
      }
      if (event.key === "Enter" && !event.shiftKey) {
        event.preventDefault();
        submitAction();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [busy, isPlan, onDismiss, options.length, questions, ready, revision, selected, questionAnswers, questionNotes]);

  const chooseQuestionOption = (questionId: string, label: string, index: number) => {
    setSelected(index);
    setQuestionAnswers((current) => ({ ...current, [questionId]: label }));
  };

  return (
    <section className="current-action-card" aria-live="polite">
      <div className="current-action-header">
        <div>
          <span>{isPlan ? "Plan Mode" : "Questions"}</span>
          <strong>{isPlan ? "实施此计划?" : questions[0]?.question ?? "Codex 正在等待选择"}</strong>
        </div>
        <small>↑↓ 选择 · 1-9 快选</small>
      </div>
      <div className="current-action-options">
        {isPlan ? options.map((option, index) => (
          <button
            type="button"
            key={option.label}
            className={selected === index ? "current-action-option selected" : "current-action-option"}
            onClick={() => setSelected(index)}
          >
            <span>{index + 1}</span>
            <div>
              <strong>{option.label}</strong>
              <small>{option.description}</small>
            </div>
          </button>
        )) : questions.map((question) => (
          <div className="current-action-question" key={question.id}>
            {questions.length > 1 && <strong>{question.question}</strong>}
            {question.options.map((option, index) => (
              <button
                type="button"
                key={`${question.id}-${option.label}`}
                className={questionAnswers[question.id] === option.label ? "current-action-option selected" : "current-action-option"}
                onClick={() => chooseQuestionOption(question.id, option.label, index)}
              >
                <span>{index + 1}</span>
                <div>
                  <strong>{option.label}</strong>
                  {option.description && <small>{option.description}</small>}
                </div>
              </button>
            ))}
            <textarea
              className="current-action-textarea"
              value={questionNotes[question.id] ?? ""}
              onChange={(event) => setQuestionNotes((current) => ({ ...current, [question.id]: event.target.value }))}
              placeholder="补充输入"
            />
          </div>
        ))}
      </div>
      {selectedPlanRequiresRevision && (
        <textarea
          className="current-action-textarea"
          value={revision}
          onChange={(event) => setRevision(event.target.value)}
          placeholder="告诉 Codex 需要怎样调整计划"
        />
      )}
      <div className="current-action-footer">
        <button className="secondary-button ghost" type="button" onClick={onDismiss}>
          忽略 <kbd>ESC</kbd>
        </button>
        <button className="primary-button" type="button" disabled={!ready || busy} onClick={submitAction}>
          提交 <kbd>↵</kbd>
        </button>
      </div>
    </section>
  );
}

function ApprovalCard({ block, onDecision, pending }: { block: MessageBlock; onDecision: (decision: string) => void; pending: boolean }) {
  return (
    <article className="approval-card action-request">
      <div className="message-meta">
        <span>审批请求</span>
        <small>{block.call_id || block.item_id || block.turn_id || block.kind}</small>
      </div>
      <pre>{block.text || formatPayload(block.payload) || "Codex 正在等待权限审批。"}</pre>
      <div className="button-row">
        <button className="primary-button" disabled={pending} onClick={() => onDecision("accept")}>
          <ClipboardCheck size={17} />批准
        </button>
        <button className="danger-button soft" disabled={pending} onClick={() => onDecision("decline")}>
          <X size={17} />拒绝
        </button>
      </div>
    </article>
  );
}

function UnsupportedApprovalCard({ block }: { block: MessageBlock }) {
  return (
    <article className="approval-card action-request">
      <div className="message-meta">
        <span>审批请求</span>
        <small>{block.call_id || block.item_id || block.turn_id || block.kind}</small>
      </div>
      <pre>{block.text || formatPayload(block.payload) || "Codex 正在等待权限审批。"}</pre>
      <div className="muted-row">macOS App 当前不支持在此面板处理权限审批，请在 Codex 原生会话中处理。</div>
    </article>
  );
}


function StatusChip({ status }: { status: ThreadStatus }) {
  return <span className={`status-chip ${status}`}>{threadStatusLabel(status)}</span>;
}

function legacyBlocks(detail: ThreadDetail): MessageBlock[] {
  return detail.messages.map((message, index) => ({
    id: `legacy-${index}`,
    role: message.role,
    kind: message.kind,
    text: message.text,
    created_at: message.created_at,
    questions: []
  }));
}

export function pendingFromBlocks(blocks: MessageBlock[], status: ThreadStatus, activeTurnId: string | null | undefined): PendingElicitation | null {
  void status;
  if (!activeTurnId) return null;
  const block = [...blocks].reverse().find((item) => item.turn_id === activeTurnId && isQuestionBlock(item) && !isResolvedActionBlock(item));
  if (!block) return null;
  return {
    turn_id: block.turn_id,
    item_id: block.item_id ?? block.call_id,
    questions: block.questions
  };
}

export function latestActionBlock(blocks: MessageBlock[], status: ThreadStatus, activeTurnId: string | null | undefined, predicate: (block: MessageBlock) => boolean): MessageBlock | null {
  const reversed = [...blocks].reverse();
  if (activeTurnId) {
    const active = reversed.find((block) => block.turn_id === activeTurnId && predicate(block) && !isResolvedActionBlock(block));
    if (active) return active;
    return null;
  }
  if (status !== "ReplyNeeded") return null;
  if (!reversed.some((block) => predicate(block) && isPlanBlock(block))) return null;

  for (const block of reversed) {
    if (predicate(block) && isPlanBlock(block) && !isResolvedActionBlock(block)) return block;
    if (isExternalProgressAfterPlan(block)) return null;
  }
  return null;
}

export function currentPendingElicitation(pending: PendingElicitation | null | undefined, activeTurnId: string | null | undefined): PendingElicitation | null {
  if (!pending || !activeTurnId) return null;
  if (pending.turn_id !== activeTurnId) return null;
  return pending;
}

export function isPlanBlock(block: MessageBlock): boolean {
  const kind = normalizedBlockKind(block);
  const displayKind = normalizedDisplayKind(block);
  return displayKind === "plan" || kind.includes("plan") || Boolean(block.text?.includes("<proposed_plan>"));
}

export function isApprovalBlock(block: MessageBlock): boolean {
  const kind = normalizedBlockKind(block);
  const displayKind = normalizedDisplayKind(block);
  return displayKind === "approval" || kind.includes("requestapproval") || kind.includes("approval") || kind.includes("permissions/request");
}

export function isQuestionBlock(block: MessageBlock): boolean {
  const kind = normalizedBlockKind(block);
  const displayKind = normalizedDisplayKind(block);
  return displayKind === "question"
    || (kind === "request_user_input" || kind === "requestuserinput")
    || kind.includes("request_user_input")
    || kind.includes("requestuserinput")
    || (block.questions?.length ?? 0) > 0;
}

export function isQuestionResultBlock(block: MessageBlock): boolean {
  const kind = normalizedBlockKind(block);
  const displayKind = normalizedDisplayKind(block);
  return displayKind === "question_result"
    || kind === "request_user_input_result"
    || (block.answers?.length ?? 0) > 0;
}

export function isToolBlock(block: MessageBlock): boolean {
  const kind = normalizedBlockKind(block);
  return block.role === "tool"
    || kind.includes("tool")
    || kind.includes("command")
    || kind.includes("function_call")
    || kind.includes("web_search");
}

export function shouldRenderConversationMessage(block: MessageBlock): boolean {
  const role = block.role || "";
  const kind = normalizedBlockKind(block);
  if (!["assistant", "user"].includes(role)) return false;
  if ((block.questions?.length ?? 0) > 0) return false;
  if (kind === "request_user_input" || kind === "requestuserinput") return false;
  if (isToolBlock(block) || isPlanBlock(block) || isApprovalBlock(block)) return false;
  if (isInternalContextText(block.text)) return false;
  return !["reasoning", "agent_reasoning", "session_meta"].includes(kind);
}

export function shouldRenderConversationBlock(block: MessageBlock): boolean {
  if (isApprovalBlock(block)) return false;
  if (isPlanBlock(block) || isQuestionBlock(block) || isQuestionResultBlock(block)) return true;
  return isToolBlock(block) || shouldRenderConversationMessage(block);
}

export function shouldRenderActionStackBlock(block: MessageBlock): boolean {
  return isApprovalBlock(block) && !isResolvedActionBlock(block);
}

export function isResolvedActionBlock(block: MessageBlock): boolean {
  if (block.resolved === true) return true;
  const status = (block.plan_status ?? block.status ?? "").toLowerCase();
  return ["completed", "complete", "succeeded", "success", "done", "approved", "declined", "rejected", "cancelled", "canceled", "failed"].includes(status);
}

export function isActionablePlanBlock(block: MessageBlock, current: MessageBlock | null | undefined): boolean {
  return Boolean(current && isPlanBlock(block) && !isResolvedActionBlock(block) && sameActionBlock(block, current));
}

export function isActionableQuestionBlock(block: MessageBlock, current: PendingElicitation | MessageBlock | null | undefined): boolean {
  if (!current || !isQuestionBlock(block) || isResolvedActionBlock(block)) return false;
  const currentTurnId = current.turn_id ?? null;
  if (currentTurnId && block.turn_id !== currentTurnId) return false;
  const currentItemId = "id" in current ? current.item_id ?? current.call_id ?? current.id : current.item_id ?? null;
  if (!currentItemId) return Boolean(currentTurnId);
  return [block.item_id, block.call_id, block.id].some((value) => value === currentItemId);
}

export function questionAnswerLabels(block: MessageBlock, questionId: string): string[] {
  return block.answers?.find((answer) => answer.question_id === questionId)?.answers ?? [];
}

export function blocksWithCurrentPending(blocks: MessageBlock[], pending: PendingElicitation | null): MessageBlock[] {
  if (!pending || !pending.questions.length) return blocks;
  const existing = blocks.some((block) => {
    if (!isQuestionBlock(block)) return false;
    if (pending.item_id && (block.item_id === pending.item_id || block.call_id === pending.item_id)) return true;
    if (pending.turn_id && block.turn_id === pending.turn_id) return true;
    return false;
  });
  if (existing) return blocks;
  return [
    ...blocks,
    {
      id: `pending-question-${pending.turn_id ?? pending.item_id ?? "current"}`,
      role: "assistant",
      kind: "request_user_input",
      display_kind: "question",
      status: "pending",
      resolved: false,
      turn_id: pending.turn_id,
      item_id: pending.item_id,
      questions: pending.questions
    }
  ];
}

function sameActionBlock(left: MessageBlock, right: MessageBlock): boolean {
  const leftIds = [left.id, left.item_id, left.call_id].filter(Boolean);
  const rightIds = [right.id, right.item_id, right.call_id].filter(Boolean);
  const sameId = leftIds.some((leftId) => rightIds.includes(leftId));
  if (!sameId) return false;
  if (left.turn_id && right.turn_id) return left.turn_id === right.turn_id;
  return true;
}

function isExternalProgressAfterPlan(block: MessageBlock): boolean {
  if (isPlanBlock(block) || isQuestionBlock(block) || isQuestionResultBlock(block) || isApprovalBlock(block)) return false;
  if (isHistoryCollapsedBlock(block)) return false;
  if (isToolBlock(block)) return true;
  return shouldRenderConversationMessage(block);
}

function normalizedBlockKind(block: Pick<MessageBlock, "kind">): string {
  return block.kind.toLowerCase();
}

function normalizedDisplayKind(block: Pick<MessageBlock, "display_kind">): string {
  return block.display_kind?.toLowerCase() ?? "";
}

export type ConversationMessagePresentation = {
  kind: "user" | "assistant";
  rowClassName: string;
  bodyClassName: string;
};

export function conversationMessagePresentation(block: Pick<MessageBlock, "role">): ConversationMessagePresentation {
  const kind = block.role === "user" ? "user" : "assistant";
  return {
    kind,
    rowClassName: `chat-row ${kind}`,
    bodyClassName: kind === "user" ? "chat-bubble" : "assistant-message-body"
  };
}

export function visibleConversationBlocksForHistory(
  blocks: MessageBlock[],
  showAllHistory: boolean,
  currentPlan?: MessageBlock | null,
  currentQuestion?: PendingElicitation | MessageBlock | null
): MessageBlock[] {
  const renderable = blocks.filter(shouldRenderConversationBlock);
  if (showAllHistory) return renderable;
  return compactConversationBlocks(renderable, 4, 60, 3, currentPlan, currentQuestion);
}

export function prioritizeCurrentActionBlocks(
  blocks: MessageBlock[],
  currentPlan: MessageBlock | null | undefined,
  currentQuestion: PendingElicitation | MessageBlock | null | undefined
): MessageBlock[] {
  const promoted: MessageBlock[] = [];
  const rest: MessageBlock[] = [];
  for (const block of blocks) {
    if (isActionablePlanBlock(block, currentPlan) || isActionableQuestionBlock(block, currentQuestion)) {
      promoted.push(block);
    } else {
      rest.push(block);
    }
  }
  return promoted.length ? [...rest, ...promoted] : blocks;
}

export function compactConversationBlocks(
  blocks: MessageBlock[],
  maxCompletedTools = 4,
  maxChatMessages = 60,
  maxActionBlocks = 3,
  currentPlan?: MessageBlock | null,
  currentQuestion?: PendingElicitation | MessageBlock | null
): MessageBlock[] {
  const hasToolHistoryCollapse = blocks.some((block) => historyCollapseKind(block) === "tool");
  const completedToolIndexes = hasToolHistoryCollapse
    ? []
    : blocks
      .map((block, index) => ({ block, index }))
      .filter(({ block }) => isToolBlock(block) && !isHistoryCollapsedBlock(block) && !isRunningToolBlock(block))
      .map(({ index }) => index);
  const toolCompacted = hasToolHistoryCollapse
    ? blocks
    : compactIndexedBlocks(
      blocks,
      completedToolIndexes,
      maxCompletedTools,
      "completed-tool-history-collapsed",
      "tool_history_collapsed",
      "tool_history",
      "个历史工具调用已折叠"
    );

  if (toolCompacted.some((block) => historyCollapseKind(block) === "action")) {
    return toolCompacted;
  }

  const actionIndexes = toolCompacted
    .map((block, index) => ({ block, index }))
    .filter(({ block }) => {
      if (isHistoryCollapsedBlock(block)) return false;
      if (isActionablePlanBlock(block, currentPlan)) return false;
      if (isActionableQuestionBlock(block, currentQuestion)) return false;
      return isPlanBlock(block) || isQuestionBlock(block) || isQuestionResultBlock(block);
    })
    .map(({ index }) => index);
  const actionCompacted = compactIndexedBlocks(
    toolCompacted,
    actionIndexes,
    maxActionBlocks,
    "action-history-collapsed",
    "action_history_collapsed",
    "action_history",
    "条历史计划/问题已折叠"
  );

  if (actionCompacted.some((block) => historyCollapseKind(block) === "chat")) {
    return actionCompacted;
  }

  const chatIndexes = actionCompacted
    .map((block, index) => ({ block, index }))
    .filter(({ block }) => shouldRenderConversationMessage(block))
    .map(({ index }) => index);
  return compactIndexedBlocks(
    actionCompacted,
    chatIndexes,
    maxChatMessages,
    "chat-history-collapsed",
    "chat_history_collapsed",
    "chat_history",
    "条历史对话已折叠"
  );
}

function compactIndexedBlocks(
  blocks: MessageBlock[],
  indexes: number[],
  maxVisible: number,
  id: string,
  kind: string,
  toolName: string,
  label: string
): MessageBlock[] {
  if (indexes.length <= maxVisible) return blocks;
  const keep = new Set(indexes.slice(-maxVisible));
  const hide = new Set(indexes.slice(0, -maxVisible));
  const hidden = indexes.length - keep.size;
  const collapsed: MessageBlock = {
    id,
    role: "tool",
    kind,
    status: "completed",
    text: `${hidden} ${label}`,
    summary: `${hidden} ${label}`,
    tool_name: toolName,
    truncated: false,
    questions: []
  };
  const compacted: MessageBlock[] = [];
  let inserted = false;
  for (const [index, block] of blocks.entries()) {
    if (hide.has(index) && !keep.has(index)) {
      if (!inserted) {
        compacted.push(collapsed);
        inserted = true;
      }
      continue;
    }
    compacted.push(block);
  }
  return compacted;
}

function isHistoryCollapsedBlock(block: MessageBlock): boolean {
  return historyCollapseKind(block) !== null;
}

function historyCollapseKind(block: MessageBlock): "chat" | "tool" | "action" | null {
  const kind = block.kind.toLowerCase();
  if (kind === "chat_history_collapsed") return "chat";
  if (kind === "tool_history_collapsed") return "tool";
  if (kind === "action_history_collapsed") return "action";
  return null;
}

function isInternalContextText(value?: string | null): boolean {
  const text = value?.trimStart().toLowerCase();
  if (!text) return false;
  return [
    "<environment_context>",
    "<permissions instructions>",
    "<app-context>",
    "<collaboration_mode>",
    "<skills_instructions>",
    "<plugins_instructions>",
    "<subagent_notification>",
    "<subagent_context>",
    "<codex_internal_context",
    "<goal_context>",
    "<additional_context>",
    "<user_instructions>",
    "<turn_aborted>",
    "<user_shell_command>",
    "<legacy_unified_exec_process_limit_warning>",
    "<legacy_apply_patch_exec_command_warning>",
    "<legacy_model_mismatch_warning>",
    "========= memory_summary begins ========="
  ].some((prefix) => text.startsWith(prefix));
}

export function toolBlockTitle(block: MessageBlock): string {
  if (isHistoryCollapsedBlock(block)) {
    return firstDisplayLine(block.summary)
      ?? firstDisplayLine(block.text)
      ?? block.tool_name?.trim()
      ?? block.kind
      ?? "tool";
  }
  return block.tool_name?.trim() || block.kind || "tool";
}

export function toolBlockStatus(block: MessageBlock): string {
  return block.status?.trim() || block.call_id || "completed";
}

export function toolBlockSummary(block: MessageBlock): string | null {
  return firstDisplayLine(block.summary)
    ?? firstDisplayLine(block.text)
    ?? firstDisplayLine(block.input)
    ?? null;
}

export function toolBlockDetailText(block: MessageBlock): string {
  const sections: string[] = [];
  const input = block.input?.trim();
  const output = (block.text?.trim() || formatPayload(block.payload).trim());
  if (input) sections.push(`Input\n${input}`);
  if (output) sections.push(`${input ? "Output\n" : ""}${output}`);
  if (block.truncated) sections.push("[output truncated]");
  return sections.join("\n\n") || "No output";
}

export function messageBlockText(block: MessageBlock): string {
  return block.text?.trim() || formatPayload(block.payload) || "";
}

function firstDisplayLine(value?: string | null): string | null {
  const line = value?.split(/\r?\n/).map((item) => item.trim()).find(Boolean);
  return line || null;
}

function blockKindLabel(kind: string): string {
  if (kind.includes("agentMessage")) return "assistant";
  if (kind.includes("userMessage")) return "user";
  if (kind.includes("function")) return "tool";
  return kind;
}

function roleLabel(role: string): string {
  if (role === "assistant") return "Codex";
  if (role === "user") return "User";
  if (role === "tool") return "Tool";
  return role || "System";
}

function formatPayload(payload: unknown): string {
  if (!payload) return "";
  if (typeof payload === "string") return payload;
  return JSON.stringify(payload, null, 2);
}

function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}
