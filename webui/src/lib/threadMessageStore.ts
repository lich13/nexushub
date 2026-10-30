import { useCallback, useMemo, useRef, useState } from "react";
import type { MessageBlock, ThreadBlockPage, ThreadDetail, ThreadSummary } from "../types";
import { historyCursorIndex, stableHistoryCursor } from "./domain/historyCursor";

export type ThreadMessageSlot = {
  timeline?: ThreadDetail["timeline"];
  history?: ThreadDetail["history"];
  subagents?: ThreadDetail["subagents"];
  subagentUpdates?: ThreadDetail["subagent_updates"];
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
  showAllHistory: boolean;
  hiddenActionKey: string | null;
  fetchedAt: number | null;
};

export type ThreadMessageStoreState = {
  activeThreadId: string | null;
  slots: Map<string, ThreadMessageSlot>;
};

export type ThreadMessageStoreController = {
  store: ThreadMessageStoreState;
  setActive: (threadId: string | null) => void;
  getSlot: (threadId: string) => ThreadMessageSlot;
  isActive: (threadId: string) => boolean;
  applyDetail: (threadId: string, detail: ThreadDetail) => void;
  applySummary: (threadId: string, summary: ThreadSummary) => void;
  patchSummary: (threadId: string, patch: Partial<ThreadSummary> | ((current: ThreadSummary) => ThreadSummary)) => void;
  applyRealtimeBlocks: (threadId: string, blocks: MessageBlock[]) => void;
  applyBlockPage: (threadId: string, page: ThreadBlockPage, expectedCursor?: string | null) => void;
  setLoadingEarlier: (threadId: string, loading: boolean, error?: string | null) => void;
  setFeedback: (threadId: string, feedback: string | null) => void;
  setHistoryExpanded: (threadId: string, expanded: boolean) => void;
  setHiddenActionKey: (threadId: string, key: string | null) => void;
  clear: (threadId: string) => void;
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
    showAllHistory: false,
    hiddenActionKey: null,
    fetchedAt: null
  };
}

export function fallbackThreadSummary(threadId: string): ThreadSummary {
  return {
    id: threadId,
    title: "读取中",
    status: "Recent",
    message_count: 0
  };
}

export function threadDetailFromMessageSlot(
  threadId: string,
  slot: ThreadMessageSlot,
  fallback?: ThreadSummary | null
): ThreadDetail {
  const summary = slot.summary
    ? fallback ? { ...slot.summary, ...fallback } : slot.summary
    : fallback ?? fallbackThreadSummary(threadId);
  return {
    timeline: slot.timeline,
    history: slot.history,
    subagents: slot.subagents,
    subagent_updates: slot.subagentUpdates,
    summary,
    messages: [],
    blocks: slot.blocks,
    raw_event_count: slot.totalBlocks,
    total_blocks: slot.totalBlocks,
    has_more_blocks: slot.hasMoreBlocks,
    before_cursor: slot.beforeCursor
  };
}

export function useThreadMessageStoreController(
  legacyBlocks: (detail: ThreadDetail) => MessageBlock[] = defaultLegacyBlocks
): ThreadMessageStoreController {
  const storeRef = useRef<ThreadMessageStoreState>(createThreadMessageStoreState());
  const [, setRevision] = useState(0);
  const notify = useCallback((threadId: string | null) => {
    if (threadId && storeRef.current.activeThreadId !== threadId) return;
    setRevision((value) => value + 1);
  }, []);
  const setActive = useCallback((nextThreadId: string | null) => {
    setActiveThreadSlot(storeRef.current, nextThreadId);
  }, []);
  const getSlotForThread = useCallback((nextThreadId: string) => getThreadSlot(storeRef.current, nextThreadId), []);
  const isActive = useCallback((nextThreadId: string) => storeRef.current.activeThreadId === nextThreadId, []);
  const applyDetail = useCallback((nextThreadId: string, nextDetail: ThreadDetail) => {
    const prevRevision = getThreadSlot(storeRef.current, nextThreadId).visibleUpdateRevision;
    const slot = applyThreadDetailToSlot(storeRef.current, nextThreadId, nextDetail, legacyBlocks);
    if (slot.visibleUpdateRevision !== prevRevision) {
      notify(nextThreadId);
    }
  }, [legacyBlocks, notify]);
  const applySummary = useCallback((nextThreadId: string, nextSummary: ThreadSummary) => {
    const prevRevision = getThreadSlot(storeRef.current, nextThreadId).visibleUpdateRevision;
    const slot = applyThreadSummaryToSlot(storeRef.current, nextThreadId, nextSummary);
    if (slot.visibleUpdateRevision !== prevRevision) {
      notify(nextThreadId);
    }
  }, [notify]);
  const patchSummary = useCallback((nextThreadId: string, patch: Partial<ThreadSummary> | ((current: ThreadSummary) => ThreadSummary)) => {
    const slot = getThreadSlot(storeRef.current, nextThreadId);
    const base = slot.summary ?? fallbackThreadSummary(nextThreadId);
    const next = typeof patch === "function" ? patch(base) : { ...base, ...patch };
    applyThreadSummaryToSlot(storeRef.current, nextThreadId, next);
    notify(nextThreadId);
  }, [notify]);
  const applyRealtimeBlocks = useCallback((nextThreadId: string, blocks: MessageBlock[]) => {
    applyRealtimeBlocksToThreadSlot(storeRef.current, nextThreadId, blocks);
    notify(nextThreadId);
  }, [notify]);
  const applyBlockPage = useCallback((nextThreadId: string, page: ThreadBlockPage, expectedCursor?: string | null) => {
    applyThreadBlockPageToSlot(storeRef.current, nextThreadId, page, expectedCursor);
    notify(nextThreadId);
  }, [notify]);
  const setLoadingEarlierForThread = useCallback((nextThreadId: string, loading: boolean, error: string | null = null) => {
    setThreadLoadingEarlier(storeRef.current, nextThreadId, loading, error);
    notify(nextThreadId);
  }, [notify]);
  const setFeedbackForThread = useCallback((nextThreadId: string, feedback: string | null) => {
    setThreadFeedback(storeRef.current, nextThreadId, feedback);
    notify(nextThreadId);
  }, [notify]);
  const setHistoryExpandedForThread = useCallback((nextThreadId: string, expanded: boolean) => {
    setThreadHistoryExpanded(storeRef.current, nextThreadId, expanded);
    notify(nextThreadId);
  }, [notify]);
  const setHiddenActionKeyForThread = useCallback((nextThreadId: string, key: string | null) => {
    setThreadHiddenActionKey(storeRef.current, nextThreadId, key);
    notify(nextThreadId);
  }, [notify]);
  const clearThread = useCallback((nextThreadId: string) => {
    const wasActive = storeRef.current.activeThreadId === nextThreadId;
    clearThreadSlot(storeRef.current, nextThreadId);
    notify(wasActive ? null : nextThreadId);
  }, [notify]);

  return useMemo(() => ({
    store: storeRef.current,
    setActive,
    getSlot: getSlotForThread,
    isActive,
    applyDetail,
    applySummary,
    patchSummary,
    applyRealtimeBlocks,
    applyBlockPage,
    setLoadingEarlier: setLoadingEarlierForThread,
    setFeedback: setFeedbackForThread,
    setHistoryExpanded: setHistoryExpandedForThread,
    setHiddenActionKey: setHiddenActionKeyForThread,
    clear: clearThread
  }), [
    setActive,
    getSlotForThread,
    isActive,
    applyDetail,
    applySummary,
    patchSummary,
    applyRealtimeBlocks,
    applyBlockPage,
    setLoadingEarlierForThread,
    setFeedbackForThread,
    setHistoryExpandedForThread,
    setHiddenActionKeyForThread,
    clearThread
  ]);
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
  // A legacy count can lag pagination or a realtime tail. Only verified source
  // identity/change metadata can discard already loaded messages.
  const reset = detail.history?.reset || (slot.history && detail.history && slot.history.epoch !== detail.history.epoch);
  if (reset) { slot.blocks = []; slot.beforeCursor = null; slot.hasMoreBlocks = false; }
  const nextTimeline = detail.timeline ?? (reset ? undefined : slot.timeline);
  const incomingBlocks = detail.blocks.length ? detail.blocks : legacyBlocks(detail);
  const previousBefore = slot.beforeCursor;
  const tailAlreadyIdentical =
    !reset &&
    !detail.subagent_updates &&
    incomingBlocks.length > 0 &&
    slot.blocks.length >= incomingBlocks.length &&
    incomingBlocks.every((block, idx) => slot.blocks[slot.blocks.length - incomingBlocks.length + idx] === block);
  const mergedBlocks = tailAlreadyIdentical
    ? slot.blocks
    : mergeSubagentUpdates(mergeBlocksPreservingHistory(slot.blocks, incomingBlocks), detail.subagent_updates);
  const blocksChanged = mergedBlocks !== slot.blocks;
  const nextSummary = mergeSummary(slot.summary, detail.summary);
  const nextTotalBlocks = detail.total_blocks ?? Math.max(slot.totalBlocks, mergedBlocks.length);
  let nextHasMoreBlocks = Boolean(detail.has_more_blocks ?? slot.hasMoreBlocks);
  let nextBeforeCursor = slot.beforeCursor;
  if (mergedBlocks.length >= nextTotalBlocks) {
    nextBeforeCursor = null;
    nextHasMoreBlocks = false;
  } else if (detail.before_cursor) {
    if (!previousBefore || historyCursorIndex(detail.before_cursor) < historyCursorIndex(previousBefore)) {
      nextBeforeCursor = stableHistoryCursor(detail) ?? null;
    }
  } else if (!mergedBlocks.length) {
    nextBeforeCursor = null;
    nextHasMoreBlocks = false;
  }
  const changed = blocksChanged
    || slot.timeline !== nextTimeline
    || slot.subagents !== detail.subagents
    || slot.summary !== nextSummary
    || slot.totalBlocks !== nextTotalBlocks
    || slot.hasMoreBlocks !== nextHasMoreBlocks
    || slot.beforeCursor !== nextBeforeCursor;

  slot.timeline = nextTimeline;
  slot.history = detail.history;
  slot.summary = nextSummary;
  slot.subagents = detail.subagents;
  slot.subagentUpdates = detail.subagent_updates;
  slot.blocks = mergedBlocks;
  slot.totalBlocks = nextTotalBlocks;
  slot.hasMoreBlocks = nextHasMoreBlocks;
  slot.beforeCursor = nextBeforeCursor;
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
  if (page.thread_id !== threadId || (slot.history && page.history && slot.history.epoch !== page.history.epoch)) {
    return slot;
  }
  if (expectedCursor && slot.beforeCursor && slot.beforeCursor !== expectedCursor) {
    return slot;
  }
  const nextBlocks = mergeSubagentUpdates(mergeMessageBlocks(slot.blocks, page.blocks, "prepend"), slot.subagentUpdates);
  const changed = nextBlocks !== slot.blocks;
  slot.blocks = nextBlocks;
  slot.totalBlocks = page.total_blocks ?? Math.max(slot.totalBlocks, nextBlocks.length);
  slot.hasMoreBlocks = Boolean(page.has_more_blocks);
  slot.beforeCursor = stableHistoryCursor(page) ?? null;
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
  const nextSummary = mergeSummary(slot.summary, summary);
  slot.fetchedAt = Date.now();
  if (nextSummary !== slot.summary) {
    slot.summary = nextSummary;
    slot.visibleUpdateRevision += 1;
  }
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
  next.title = mergeThreadSummaryTitle(current.title, incoming.title);
  if (!isVisibleLastEventKind(incoming.last_event_kind) && isVisibleLastEventKind(current.last_event_kind)) {
    next.last_event_kind = current.last_event_kind;
  }
  const keys = Object.keys(next) as Array<keyof ThreadSummary>;
  if (keys.length === Object.keys(current).length && keys.every(key => Object.is(current[key], next[key]))) {
    return current;
  }
  return next;
}

export function mergeThreadSummaryTitle(current?: string | null, incoming?: string | null): string {
  const currentTitle = current?.trim() ?? "";
  const incomingTitle = incoming?.trim() ?? "";
  if (!incomingTitle) return currentTitle;
  if (isUsableThreadTitle(currentTitle) && isNoisyThreadTitle(incomingTitle)) return currentTitle;
  if (isNoisyThreadTitle(currentTitle) && isUsableThreadTitle(incomingTitle)) return incomingTitle;
  if (isNoisyThreadTitle(incomingTitle)) return currentTitle;
  return incomingTitle;
}

export function isNoisyThreadTitle(title?: string | null): boolean {
  const value = title?.trim() ?? "";
  if (!value) return true;
  const normalized = value.replace(/\s+/g, " ").trim();
  const lower = normalized.toLowerCase();
  if (["未命名线程", "untitled thread", "untitled", "读取中", "loading", "loading..."].includes(lower)) return true;
  if (/<\/?proposed_plan>/i.test(value)) return true;
  if (looksLikePlanBody(value)) return true;
  if (looksLikeAssistantBodyTitle(normalized)) return true;
  return normalized.length > 80;
}

function isUsableThreadTitle(title?: string | null): boolean {
  return !isNoisyThreadTitle(title);
}

function looksLikePlanBody(value: string): boolean {
  const lines = value.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  if (lines.length < 2) return false;
  const planLines = lines.filter((line) => /^(\d+[\.)、]|[-*•])\s+/.test(line));
  return planLines.length >= Math.min(2, lines.length);
}

function looksLikeAssistantBodyTitle(value: string): boolean {
  if (value.length < 40) return false;
  if (!/[。！？.!?]/.test(value)) return false;
  return /(?:我会|我将|接下来|先.+然后|然后.+最后|最后.+运行|确认不会再)/.test(value);
}

function isVisibleLastEventKind(value?: string | null): boolean {
  const event = value?.trim();
  return Boolean(event && !event.startsWith("app-server.") && !event.startsWith("panel."));
}

export function mergeBlocksPreservingHistory<T extends { id: string }>(current: T[], incoming: T[]): T[] {
  if (!current.length) return incoming;
  if (!incoming.length) return current;
  const incomingIds = new Set(incoming.map((block) => block.id));
  const firstOverlap = current.findIndex((block) => incomingIds.has(block.id));
  const previousById = new Map(current.map((block) => [block.id, block]));
  // The refreshed window owns its native order, including a creation anchor
  // that moves when its later native start event arrives. Keep loaded history
  // before that window and newer realtime messages after it.
  const next = firstOverlap < 0 ? [...current] : current.slice(0, firstOverlap);
  const beforeAnchor = new Map<string, T[]>();
  let pending: T[] = [];
  if (firstOverlap >= 0) {
    for (const block of current.slice(firstOverlap)) {
      if (incomingIds.has(block.id)) {
        if (pending.length) beforeAnchor.set(block.id, pending);
        pending = [];
      } else {
        pending.push(block);
      }
    }
  }
  for (const block of incoming) {
    next.push(...(beforeAnchor.get(block.id) ?? []));
    const previous = previousById.get(block.id);
    next.push(previous && messageBlocksEqual(previous, block) ? previous : block);
  }
  next.push(...pending);
  return next.length === current.length && next.every((block, index) => block === current[index]) ? current : next;
}

export function mergeMessageBlocks(current: MessageBlock[], incoming: MessageBlock[], mode: "append" | "prepend" = "append"): MessageBlock[] {
  if (!incoming.length) return current;
  const values = new Map(current.map(block => [block.id, block]));
  const added: string[] = [];
  let changed = false;
  const ordered = mode === "prepend" ? [...incoming].reverse() : incoming;
  for (const block of ordered) {
    const previous = values.get(block.id);
    if (previous && messageBlocksEqual(previous, block)) continue;
    if (!previous) added.push(block.id);
    values.set(block.id, block);
    changed = true;
  }
  if (!changed) return current;
  const existing = current.map(block => values.get(block.id)!);
  const additions = added.map(id => values.get(id)!);
  return mode === "prepend" ? [...additions.reverse(), ...existing] : [...existing, ...additions];
}

export function upsertMessageBlock(current: MessageBlock[], next: MessageBlock): MessageBlock[] {
  const existingIndex = current.findIndex((block) => block.id === next.id);
  if (existingIndex === -1) return [...current, next];
  const existing = current[existingIndex];
  if (messageBlocksEqual(existing, next)) return current;
  const updated = [...current];
  updated[existingIndex] = next;
  return updated;
}

function messageBlocksEqual(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (!left || !right || typeof left !== "object" || typeof right !== "object") return false;
  if (Array.isArray(left) !== Array.isArray(right)) return false;
  const a = left as Record<string, unknown>, b = right as Record<string, unknown>;
  const keys = Object.keys(a);
  return keys.length === Object.keys(b).length && keys.every(key => Object.prototype.hasOwnProperty.call(b, key) && messageBlocksEqual(a[key], b[key]));
}

// Refresh loaded historical activities without inserting off-page messages or moving anchors.
export function mergeSubagentUpdates(blocks: MessageBlock[], updates: ThreadDetail["subagent_updates"]): MessageBlock[] {
  if (!updates || !Object.keys(updates).length) return blocks;
  let changed = false;
  const result = blocks.map(block => {
    const update = updates[block.id];
    if (!block.subagent || !update) return block;
    const next = { ...block, subagent: { ...block.subagent, ...update, delegation: block.subagent.delegation } };
    if (messageBlocksEqual(block, next)) return block;
    changed = true;
    return next;
  });
  return changed ? result : blocks;
}
