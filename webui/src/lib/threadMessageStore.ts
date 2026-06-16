import type { BridgeActionResult, MessageBlock, ThreadBlockPage, ThreadDetail, ThreadSummary } from "../types";

export type ThreadMessageSlot = {
  summary: ThreadSummary | null;
  blocks: MessageBlock[];
  totalBlocks: number;
  hasMoreBlocks: boolean;
  beforeCursor: string | null;
  visibleUpdateRevision: number;
  bottomFollowRevision: number;
  loadingEarlier: boolean;
  loadError: string | null;
  feedback: string | null;
  lastResult: BridgeActionResult | null;
  showAllHistory: boolean;
  hiddenActionKey: string | null;
  fetchedAt: number | null;
};

export type ThreadMessageStoreState = {
  activeThreadId: string | null;
  slots: Map<string, ThreadMessageSlot>;
};

export function createThreadMessageStoreState(): ThreadMessageStoreState {
  return {
    activeThreadId: null,
    slots: new Map()
  };
}

export function createThreadMessageSlot(): ThreadMessageSlot {
  return {
    summary: null,
    blocks: [],
    totalBlocks: 0,
    hasMoreBlocks: false,
    beforeCursor: null,
    visibleUpdateRevision: 0,
    bottomFollowRevision: 0,
    loadingEarlier: false,
    loadError: null,
    feedback: null,
    lastResult: null,
    showAllHistory: false,
    hiddenActionKey: null,
    fetchedAt: null
  };
}

export function setActiveThreadSlot(store: ThreadMessageStoreState, threadId: string | null): ThreadMessageSlot | null {
  store.activeThreadId = threadId;
  return threadId ? getThreadSlot(store, threadId) : null;
}

export function getThreadSlot(store: ThreadMessageStoreState, threadId: string): ThreadMessageSlot {
  let slot = store.slots.get(threadId);
  if (!slot) {
    slot = createThreadMessageSlot();
    store.slots.set(threadId, slot);
  }
  return slot;
}

export function clearThreadSlot(store: ThreadMessageStoreState, threadId: string): void {
  store.slots.delete(threadId);
  if (store.activeThreadId === threadId) {
    store.activeThreadId = null;
  }
}

export function applyThreadDetailToSlot(
  store: ThreadMessageStoreState,
  threadId: string,
  detail: ThreadDetail,
  legacyBlocks: (detail: ThreadDetail) => MessageBlock[] = defaultLegacyBlocks
): ThreadMessageSlot {
  const slot = getThreadSlot(store, threadId);
  if (detail.summary.id !== threadId) {
    return slot;
  }
  const incomingBlocks = detail.blocks.length ? detail.blocks : legacyBlocks(detail);
  const previousBefore = slot.beforeCursor;
  const mergedBlocks = mergeBlocksPreservingHistory(slot.blocks, incomingBlocks);
  const blocksChanged = mergedBlocks !== slot.blocks;
  const changed = blocksChanged
    || slot.summary !== detail.summary
    || slot.totalBlocks !== (detail.total_blocks ?? Math.max(slot.totalBlocks, mergedBlocks.length))
    || slot.hasMoreBlocks !== Boolean(detail.has_more_blocks ?? slot.hasMoreBlocks);

  slot.summary = mergeSummary(slot.summary, detail.summary);
  slot.blocks = mergedBlocks;
  slot.totalBlocks = detail.total_blocks ?? Math.max(slot.totalBlocks, mergedBlocks.length);
  slot.hasMoreBlocks = Boolean(detail.has_more_blocks ?? slot.hasMoreBlocks);
  if (detail.before_cursor) {
    if (!previousBefore || cursorIndex(detail.before_cursor) < cursorIndex(previousBefore)) {
      slot.beforeCursor = detail.before_cursor;
    }
  } else if (!slot.blocks.length || slot.blocks.length >= slot.totalBlocks) {
    slot.beforeCursor = null;
    slot.hasMoreBlocks = false;
  }
  slot.fetchedAt = Date.now();
  if (changed) slot.visibleUpdateRevision += 1;
  if (blocksChanged) slot.bottomFollowRevision += 1;
  return slot;
}

export function applyThreadBlockPageToSlot(
  store: ThreadMessageStoreState,
  threadId: string,
  page: ThreadBlockPage,
  expectedCursor?: string | null
): ThreadMessageSlot {
  const slot = getThreadSlot(store, threadId);
  if (page.thread_id !== threadId) {
    return slot;
  }
  if (expectedCursor && slot.beforeCursor && slot.beforeCursor !== expectedCursor) {
    return slot;
  }
  const nextBlocks = mergeMessageBlocks(slot.blocks, page.blocks, "prepend");
  const changed = nextBlocks !== slot.blocks;
  slot.blocks = nextBlocks;
  slot.totalBlocks = page.total_blocks ?? Math.max(slot.totalBlocks, nextBlocks.length);
  slot.hasMoreBlocks = Boolean(page.has_more_blocks);
  slot.beforeCursor = page.before_cursor ?? null;
  slot.loadingEarlier = false;
  slot.loadError = null;
  if (changed) slot.visibleUpdateRevision += 1;
  return slot;
}

export function applyRealtimeBlocksToThreadSlot(
  store: ThreadMessageStoreState,
  threadId: string,
  blocks: MessageBlock[]
): ThreadMessageSlot {
  const slot = getThreadSlot(store, threadId);
  const nextBlocks = mergeMessageBlocks(slot.blocks, blocks);
  if (nextBlocks !== slot.blocks) {
    slot.blocks = nextBlocks;
    slot.totalBlocks = Math.max(slot.totalBlocks, nextBlocks.length);
    slot.visibleUpdateRevision += 1;
    slot.bottomFollowRevision += 1;
  }
  return slot;
}

export function applyThreadSummaryToSlot(
  store: ThreadMessageStoreState,
  threadId: string,
  summary: ThreadSummary
): ThreadMessageSlot {
  const slot = getThreadSlot(store, threadId);
  if (summary.id !== threadId) {
    return slot;
  }
  slot.summary = mergeSummary(slot.summary, summary);
  slot.fetchedAt = Date.now();
  slot.visibleUpdateRevision += 1;
  return slot;
}

export function setThreadLastResult(
  store: ThreadMessageStoreState,
  threadId: string,
  result: BridgeActionResult | null
): ThreadMessageSlot {
  const slot = getThreadSlot(store, threadId);
  slot.lastResult = result;
  slot.visibleUpdateRevision += 1;
  return slot;
}

export function setThreadFeedback(
  store: ThreadMessageStoreState,
  threadId: string,
  feedback: string | null
): ThreadMessageSlot {
  const slot = getThreadSlot(store, threadId);
  slot.feedback = feedback;
  slot.visibleUpdateRevision += 1;
  return slot;
}

export function setThreadHistoryExpanded(
  store: ThreadMessageStoreState,
  threadId: string,
  showAllHistory: boolean
): ThreadMessageSlot {
  const slot = getThreadSlot(store, threadId);
  slot.showAllHistory = showAllHistory;
  slot.visibleUpdateRevision += 1;
  return slot;
}

export function setThreadHiddenActionKey(
  store: ThreadMessageStoreState,
  threadId: string,
  hiddenActionKey: string | null
): ThreadMessageSlot {
  const slot = getThreadSlot(store, threadId);
  slot.hiddenActionKey = hiddenActionKey;
  slot.visibleUpdateRevision += 1;
  return slot;
}

export function setThreadLoadingEarlier(
  store: ThreadMessageStoreState,
  threadId: string,
  loadingEarlier: boolean,
  loadError: string | null = null
): ThreadMessageSlot {
  const slot = getThreadSlot(store, threadId);
  slot.loadingEarlier = loadingEarlier;
  slot.loadError = loadError;
  slot.visibleUpdateRevision += 1;
  return slot;
}

function defaultLegacyBlocks(detail: ThreadDetail): MessageBlock[] {
  return detail.messages.map((message, index) => ({
    id: `legacy-${index}`,
    role: message.role,
    kind: message.kind,
    text: message.text,
    created_at: message.created_at,
    questions: []
  }));
}

function mergeSummary(current: ThreadSummary | null, incoming: ThreadSummary): ThreadSummary {
  if (!current || current.id !== incoming.id) return incoming;
  const next = { ...current, ...incoming };
  if (!isVisibleLastEventKind(incoming.last_event_kind) && isVisibleLastEventKind(current.last_event_kind)) {
    next.last_event_kind = current.last_event_kind;
  }
  return next;
}

function isVisibleLastEventKind(value?: string | null): boolean {
  const event = value?.trim();
  return Boolean(event && !event.startsWith("app-server.") && !event.startsWith("panel."));
}

function mergeBlocksPreservingHistory(current: MessageBlock[], incoming: MessageBlock[]): MessageBlock[] {
  if (!current.length) return incoming;
  if (!incoming.length) return current;
  const incomingIds = new Set(incoming.map((block) => block.id));
  const retainedHistory = current.filter((block) => !incomingIds.has(block.id));
  if (!retainedHistory.length) return mergeMessageBlocks(current, incoming);
  const next = mergeMessageBlocks(retainedHistory, incoming);
  return next === retainedHistory ? current : next;
}

function mergeMessageBlocks(current: MessageBlock[], incoming: MessageBlock[], mode: "append" | "prepend" = "append"): MessageBlock[] {
  if (!incoming.length) return current;
  let changed = false;
  let next = current;
  const ordered = mode === "prepend" ? [...incoming].reverse() : incoming;
  for (const block of ordered) {
    if (mode === "prepend" && !next.some((item) => item.id === block.id)) {
      next = [block, ...next];
      changed = true;
      continue;
    }
    const updated = upsertMessageBlock(next, block);
    if (updated !== next) {
      next = updated;
      changed = true;
    }
  }
  return changed ? next : current;
}

function upsertMessageBlock(current: MessageBlock[], next: MessageBlock): MessageBlock[] {
  const existingIndex = current.findIndex((block) => block.id === next.id);
  if (existingIndex === -1) return [...current, next];
  const existing = current[existingIndex];
  if (messageBlocksEqual(existing, next)) return current;
  const updated = [...current];
  updated[existingIndex] = next;
  return updated;
}

function messageBlocksEqual(left: MessageBlock, right: MessageBlock): boolean {
  try {
    return JSON.stringify(left) === JSON.stringify(right);
  } catch {
    return false;
  }
}

function cursorIndex(cursor: string | null | undefined): number {
  if (!cursor) return Number.POSITIVE_INFINITY;
  const parsed = Number(cursor.replace(/^b:/, ""));
  return Number.isFinite(parsed) ? parsed : Number.POSITIVE_INFINITY;
}
