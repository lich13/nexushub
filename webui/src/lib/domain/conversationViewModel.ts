import type {
  FollowUpQueueItem,
  MessageBlock,
  PendingElicitation,
  ThreadDetail,
  ThreadStatus,
  ThreadSummary
} from "../../types";
import { mergeIncomingThreadSummary } from "./codexViewModel";

export type MessageScrollSnapshot = {
  scrollTop: number;
  clientHeight: number;
  scrollHeight: number;
};

export type MessageBlockState = {
  blocks: MessageBlock[];
  totalBlocks: number;
  hasMoreBlocks: boolean;
  beforeCursor: string | null;
  visibleUpdateRevision: number;
  bottomFollowRevision: number;
};

export type InternalReferenceSegment = {
  type: "text" | "internal_reference";
  text: string;
  copyText?: string;
  kind?: "path" | "thread" | "turn" | "job";
};

export type CurrentActionKind = "plan" | "question";
export type CurrentActionQuestion = PendingElicitation["questions"][number];
export type PlanActionSubmission = { action: "accept" } | { action: "revise"; instructions: string } | { action: "keep_plan" };

export type ConversationMessagePresentation = {
  kind: "user" | "assistant";
  rowClassName: string;
  bodyClassName: string;
};

export function shouldAutoFollowMessageStream(snapshot: MessageScrollSnapshot, threshold = 96): boolean {
  return snapshot.scrollHeight - snapshot.scrollTop - snapshot.clientHeight <= threshold;
}

export function initialMessageBlockState(detail: ThreadDetail): MessageBlockState {
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

export function combinedQuestionAnswers(
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

export function currentActionKindFromBlocks(
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

export function isHistoryCollapsedBlock(block: MessageBlock): boolean {
  return historyCollapseKind(block) !== null;
}

export function historyCollapseKind(block: MessageBlock): "chat" | "tool" | "action" | null {
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

function firstDisplayLine(value?: string | null): string | null {
  const line = value?.split(/\r?\n/).map((item) => item.trim()).find(Boolean);
  return line || null;
}

export function blockKindLabel(kind: string): string {
  if (kind.includes("agentMessage")) return "assistant";
  if (kind.includes("userMessage")) return "user";
  if (kind.includes("function")) return "tool";
  return kind;
}

export function roleLabel(role: string): string {
  if (role === "assistant") return "Codex";
  if (role === "user") return "User";
  if (role === "tool") return "Tool";
  return role || "System";
}

export function formatPayload(payload: unknown): string {
  if (!payload) return "";
  if (typeof payload === "string") return payload;
  return JSON.stringify(payload, null, 2);
}

export function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}
