import { useMutation, useQuery, useQueryClient, type QueryClient, type QueryKey } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  archiveThread,
  getThread,
  listThreads,
  renameThread,
  restoreThread,
  type ThreadDetailOptions
} from "../api";
import type {
  MessageBlock,
  ThreadDetail,
  ThreadStatus,
  ThreadSummary,
} from "../../types";
import type { RuntimeCapabilityMatrix } from "../domain/capabilities";
import {
  archivedSelectedThreadCleanupView,
  clearLocalThreadTitleOverride,
  selectedThreadDetailView,
  setLocalThreadTitleOverride,
  threadSelectionView,
  type SelectedThread
} from "../domain/codexViewModel";
import {
  mergeMessageBlocks,
  threadDetailFromMessageSlot,
  upsertMessageBlock,
  useThreadMessageStoreController,
  type ThreadMessageSlot,
  type ThreadMessageStoreController
} from "../threadMessageStore";
import { preservePreviousQueryData } from "./shared";

export const threadQueryKeys = {
  threads: (status?: string, q?: string) => status === undefined && q === undefined ? ["threads"] as const : ["threads", status, q] as const,
  thread: (threadId: string | null) => ["thread", threadId] as const,
  threadBlocks: (threadId: string) => ["thread-blocks", threadId] as const,
  plugins: ["plugins"] as const,
  jobs: ["jobs"] as const
};

type ThreadMessageStoreClear = {
  clear: (threadId: string) => void;
};

type ArchivedThreadCleanupCacheActions = {
  clearArchivedThreadClientState: (messageStore: ThreadMessageStoreClear, threadId: string) => void;
};

type ThreadRefetchType = "active" | "all" | "inactive" | "none";

type QueryCacheSnapshotEntry = {
  queryKey: QueryKey;
  existed: boolean;
  data: unknown;
};

export type ThreadCacheSnapshot = {
  entries: QueryCacheSnapshotEntry[];
};

function nonEmptyString(value: unknown): boolean {
  return typeof value === "string" && value.trim().length > 0;
}

function fieldContainsSubagent(value: unknown): boolean {
  return typeof value === "string" && value.toLowerCase().includes("subagent");
}

function sourceValueContainsSubagent(value: unknown): boolean {
  if (typeof value === "string") return value.toLowerCase().includes("subagent");
  if (Array.isArray(value)) return value.some(sourceValueContainsSubagent);
  if (typeof value === "object" && value) {
    return Object.entries(value).some(([key, item]) => key.toLowerCase().includes("subagent") || sourceValueContainsSubagent(item));
  }
  return false;
}

function isVisibleMainThreadForCache(thread: Partial<ThreadSummary>): boolean {
  if (thread.status === "Archived" || thread.archived_at) return false;
  if (nonEmptyString(thread.parentThreadId ?? thread.parent_thread_id)) return false;
  if (nonEmptyString(thread.agentPath ?? thread.agent_path)) return false;
  if (nonEmptyString(thread.agentNickname ?? thread.agent_nickname)) return false;
  if (nonEmptyString(thread.agentRole ?? thread.agent_role)) return false;
  if (fieldContainsSubagent(thread.threadSource ?? thread.thread_source)) return false;
  if (fieldContainsSubagent(thread.sourceKind ?? thread.source_kind)) return false;
  if (sourceValueContainsSubagent(thread.source)) return false;
  return true;
}

function isThreadListItemRunningForCache(thread: Partial<ThreadSummary>): boolean {
  return thread.status === "Running" || (thread.status === "Recent" && Boolean(thread.active_job_id));
}

function threadMatchesListFilterForCache(thread: Partial<ThreadSummary>, status = "all", q = ""): boolean {
  if (!isVisibleMainThreadForCache(thread)) return false;
  if (status !== "all") {
    if (status === "running" && !isThreadListItemRunningForCache(thread)) return false;
    if (status === "reply-needed" && thread.status !== "ReplyNeeded") return false;
    if (status === "recoverable" && thread.status !== "Recoverable") return false;
    if (!["running", "reply-needed", "recoverable"].includes(status) && thread.status !== status) return false;
  }
  const needle = q.trim().toLowerCase();
  if (!needle) return true;
  return [
    thread.id,
    thread.title,
    thread.latest_message
  ].some((value) => String(value ?? "").toLowerCase().includes(needle));
}

function mergeThreadSummaryTitleForCache(current?: string | null, incoming?: string | null): string {
  const next = incoming?.trim();
  if (next && !isPlaceholderThreadTitleForCache(next)) return next;
  return current?.trim() || "未命名线程";
}

function isPlaceholderThreadTitleForCache(title: string): boolean {
  const normalized = title.trim().toLowerCase();
  return !normalized
    || normalized === "未命名线程"
    || normalized === "untitled"
    || normalized === "new thread";
}

function mergeIncomingThreadSummaryForCache<T extends Partial<ThreadSummary>>(current: T, incoming: Partial<ThreadSummary>): T & Partial<ThreadSummary> {
  const next = { ...current, ...incoming };
  next.title = mergeThreadSummaryTitleForCache(current.title, incoming.title);
  return next;
}

export function mergeThreadSummaryIntoListCache(
  rows: ThreadSummary[] | undefined,
  incoming: ThreadSummary,
  status = "all",
  q = ""
): ThreadSummary[] | undefined {
  if (!rows) return rows;
  const existing = rows.find((thread) => thread.id === incoming.id);
  const merged = existing ? mergeIncomingThreadSummaryForCache(existing, incoming) as ThreadSummary : incoming;
  const matches = threadMatchesListFilterForCache(merged, status, q);
  if (!matches) {
    return existing ? rows.filter((thread) => thread.id !== incoming.id) : rows;
  }
  if (existing) {
    return rows.map((thread) => thread.id === incoming.id ? merged : thread);
  }
  return [merged, ...rows];
}

export function removeThreadFromListCaches(qc: QueryClient, threadId: string): void {
  for (const query of qc.getQueryCache().findAll({ queryKey: threadQueryKeys.threads() })) {
    qc.setQueryData<ThreadSummary[]>(query.queryKey, (rows) =>
      rows ? rows.filter((thread) => thread.id !== threadId) : rows
    );
  }
}

export function clearArchivedThreadClientState(
  qc: QueryClient,
  messageStore: ThreadMessageStoreClear,
  threadId: string
): void {
  removeThreadFromListCaches(qc, threadId);
  qc.removeQueries({ queryKey: threadQueryKeys.thread(threadId), exact: true });
  messageStore.clear(threadId);
}

function snapshotQueryCache(qc: QueryClient, queryKey: QueryKey): QueryCacheSnapshotEntry {
  return {
    queryKey,
    existed: Boolean(qc.getQueryCache().find({ queryKey, exact: true })),
    data: qc.getQueryData(queryKey)
  };
}

function snapshotThreadCaches(qc: QueryClient, threadId: string): ThreadCacheSnapshot {
  const entries = qc.getQueryCache().findAll({ queryKey: threadQueryKeys.threads() }).map((query) => snapshotQueryCache(qc, query.queryKey));
  entries.push(snapshotQueryCache(qc, threadQueryKeys.thread(threadId)));
  return { entries };
}

function restoreQueryCacheSnapshot(qc: QueryClient, snapshot?: ThreadCacheSnapshot | null): void {
  if (!snapshot) return;
  for (const entry of snapshot.entries) {
    if (entry.existed) {
      qc.setQueryData(entry.queryKey, entry.data);
    } else {
      qc.removeQueries({ queryKey: entry.queryKey, exact: true });
    }
  }
}

function updateSavedThreadTitleCaches(qc: QueryClient, threadId: string, title: string) {
  for (const query of qc.getQueryCache().findAll({ queryKey: threadQueryKeys.threads() })) {
    qc.setQueryData<ThreadSummary[]>(query.queryKey, (rows) =>
      rows ? rows.map((thread) => thread.id === threadId ? { ...thread, title } : thread) : rows
    );
  }
  qc.setQueryData<ThreadDetail>(threadQueryKeys.thread(threadId), (current) =>
    current ? { ...current, summary: { ...current.summary, title } } : current
  );
}

export function applyOptimisticThreadTitle(qc: QueryClient, threadId: string, title: string): ThreadCacheSnapshot {
  const snapshot = snapshotThreadCaches(qc, threadId);
  const nextTitle = title.trim();
  if (nextTitle) {
    updateSavedThreadTitleCaches(qc, threadId, nextTitle);
  }
  return snapshot;
}

export function rollbackOptimisticThreadTitle(qc: QueryClient, snapshot?: ThreadCacheSnapshot | null): void {
  restoreQueryCacheSnapshot(qc, snapshot);
}

export function applyOptimisticThreadArchive(
  qc: QueryClient,
  messageStore: ThreadMessageStoreClear,
  threadId: string
): ThreadCacheSnapshot {
  const snapshot = snapshotThreadCaches(qc, threadId);
  clearArchivedThreadClientState(qc, messageStore, threadId);
  return snapshot;
}

export function rollbackOptimisticThreadArchive(qc: QueryClient, snapshot?: ThreadCacheSnapshot | null): void {
  restoreQueryCacheSnapshot(qc, snapshot);
}

export function cachedThreadSummary(qc: QueryClient, threadId: string): ThreadSummary | null {
  const detail = qc.getQueryData<ThreadDetail>(threadQueryKeys.thread(threadId));
  if (detail?.summary.id === threadId) return detail.summary;
  for (const query of qc.getQueryCache().findAll({ queryKey: threadQueryKeys.threads() })) {
    const rows = qc.getQueryData<ThreadSummary[]>(query.queryKey);
    const match = rows?.find((thread) => thread.id === threadId);
    if (match) return match;
  }
  return null;
}

export function updateThreadListCaches(qc: QueryClient, incoming: ThreadSummary) {
  for (const query of qc.getQueryCache().findAll({ queryKey: threadQueryKeys.threads() })) {
    const { status, q } = threadListFilterFromQueryKey(query.queryKey);
    qc.setQueryData<ThreadSummary[]>(query.queryKey, (rows) =>
      mergeThreadSummaryIntoListCache(rows, incoming, status, q)
    );
  }
}

export function applyOptimisticThreadRestore(qc: QueryClient, threadId: string): ThreadCacheSnapshot {
  const snapshot = snapshotThreadCaches(qc, threadId);
  const cached = cachedThreadSummary(qc, threadId);
  if (!cached) return snapshot;
  const restored: ThreadSummary = {
    ...cached,
    status: "Recent",
    archived_at: null
  };
  updateThreadListCaches(qc, restored);
  qc.setQueryData<ThreadDetail>(threadQueryKeys.thread(threadId), (current) => {
    if (!current) return current;
    return {
      ...current,
      summary: {
        ...mergeIncomingThreadSummaryForCache(current.summary, restored),
        status: "Recent",
        archived_at: null
      } as ThreadSummary
    };
  });
  return snapshot;
}

export function rollbackOptimisticThreadRestore(qc: QueryClient, snapshot?: ThreadCacheSnapshot | null): void {
  restoreQueryCacheSnapshot(qc, snapshot);
}

function threadListFilterFromQueryKey(queryKey: QueryKey): { status: string; q: string } {
  const [, statusOrFilter = "all", qValue = ""] = queryKey as [unknown, unknown?, unknown?];
  if (typeof statusOrFilter === "object" && statusOrFilter) {
    const filter = statusOrFilter as { status?: unknown; q?: unknown };
    return {
      status: typeof filter.status === "string" ? filter.status : "all",
      q: typeof filter.q === "string" ? filter.q : ""
    };
  }
  return {
    status: typeof statusOrFilter === "string" ? statusOrFilter : "all",
    q: typeof qValue === "string" ? qValue : ""
  };
}

export function useThreadCacheActions() {
  const qc = useQueryClient();
  return useMemo(() => ({
    clearArchivedThreadClientState: (messageStore: ThreadMessageStoreClear, threadId: string) =>
      clearArchivedThreadClientState(qc, messageStore, threadId),
    mergeThreadDetailSummary: (threadId: string, incoming: Partial<ThreadSummary>) =>
      qc.setQueryData<ThreadDetail>(threadQueryKeys.thread(threadId), (current) => (
        current ? { ...current, summary: mergeIncomingThreadSummaryForCache(current.summary, incoming) as ThreadSummary } : current
      )),
    updateThreadListCaches: (incoming: ThreadSummary) => updateThreadListCaches(qc, incoming),
    invalidateThreads: (refetchType?: "active" | "all" | "inactive" | "none") =>
      qc.invalidateQueries({ queryKey: threadQueryKeys.threads(), ...(refetchType ? { refetchType } : {}) }),
    invalidateThread: (threadId: string, refetchType?: "active" | "all" | "inactive" | "none") =>
      qc.invalidateQueries({ queryKey: threadQueryKeys.thread(threadId), ...(refetchType ? { refetchType } : {}) }),
    invalidateThreadAndThreads: (threadId: string, refetchType?: "active" | "all" | "inactive" | "none") => {
      qc.invalidateQueries({ queryKey: threadQueryKeys.thread(threadId), ...(refetchType ? { refetchType } : {}) });
      qc.invalidateQueries({ queryKey: threadQueryKeys.threads(), ...(refetchType ? { refetchType } : {}) });
    },
    invalidateJobs: () => qc.invalidateQueries({ queryKey: threadQueryKeys.jobs }),
    cancelThreadsAndThread: (threadId: string) => Promise.all([
      qc.cancelQueries({ queryKey: threadQueryKeys.threads() }),
      qc.cancelQueries({ queryKey: threadQueryKeys.thread(threadId) })
    ]),
    applyOptimisticThreadTitle: (threadId: string, title: string) => applyOptimisticThreadTitle(qc, threadId, title),
    rollbackOptimisticThreadTitle: (snapshot?: ThreadCacheSnapshot | null) => rollbackOptimisticThreadTitle(qc, snapshot),
    applyOptimisticThreadArchive: (messageStore: ThreadMessageStoreClear, threadId: string) => applyOptimisticThreadArchive(qc, messageStore, threadId),
    rollbackOptimisticThreadArchive: (snapshot?: ThreadCacheSnapshot | null) => rollbackOptimisticThreadArchive(qc, snapshot),
    applyOptimisticThreadRestore: (threadId: string) => applyOptimisticThreadRestore(qc, threadId),
    rollbackOptimisticThreadRestore: (snapshot?: ThreadCacheSnapshot | null) => rollbackOptimisticThreadRestore(qc, snapshot),
    cachedThreadSummary: (threadId: string) => cachedThreadSummary(qc, threadId)
  }), [qc]);
}

export function useSelectedThreadState(threads: ThreadSummary[] = [], includeArchived = false) {
  const [selectedId, setSelectedId] = useState<SelectedThread>(null);
  const selection = useMemo(
    () => threadSelectionView({ threads, selectedId, includeArchived }),
    [threads, selectedId, includeArchived]
  );
  const selectThread = useCallback((id: SelectedThread) => {
    setSelectedId(id);
  }, []);
  const selectAfterRemoval = useCallback((removedThreadId: string) => {
    setSelectedId(threadSelectionView({ threads, selectedId: removedThreadId }).nextThreadAfterRemoval);
  }, [threads]);

  return {
    selectedId,
    setSelectedId,
    selectThread,
    selectAfterRemoval,
    ...selection
  };
}

export function useThreadDetailHydration(input: {
  threadId: string | null;
  detail?: ThreadDetail | null;
  includeArchived?: boolean;
}) {
  return useMemo(
    () => selectedThreadDetailView(input),
    [input.detail, input.threadId, input.includeArchived]
  );
}

export function useArchivedSelectedThreadCleanup(input: {
  threadId: string | null;
  selectedId: SelectedThread;
  rawSelectedDetail?: ThreadDetail | null;
  visibleThreads: ThreadSummary[];
  messageStore: ThreadMessageStoreClear;
  threadCache: ArchivedThreadCleanupCacheActions;
  onSelect: (threadId: SelectedThread) => void;
  includeArchived?: boolean;
}) {
  const {
    threadId,
    selectedId,
    rawSelectedDetail,
    visibleThreads,
    messageStore,
    threadCache,
    onSelect,
    includeArchived
  } = input;

  useEffect(() => {
    const cleanup = archivedSelectedThreadCleanupView({
      threadId,
      selectedId,
      detail: rawSelectedDetail,
      visibleThreads
    });
    if (includeArchived || !threadId || !cleanup.shouldClearClientState) return;
    threadCache.clearArchivedThreadClientState(messageStore, threadId);
    if (cleanup.nextSelectedId !== selectedId) {
      onSelect(cleanup.nextSelectedId);
    }
  }, [messageStore, onSelect, rawSelectedDetail, selectedId, threadCache, threadId, visibleThreads, includeArchived]);
}

export function useHydrateThreadMessageStore(input: {
  threadId: string | null;
  selectedThreadSummary?: ThreadSummary | null;
  selectedDetail?: ThreadDetail | null;
  messageStore: Pick<ThreadMessageStoreController, "setActive" | "getSlot" | "applySummary" | "applyDetail">;
}) {
  const {
    threadId,
    selectedThreadSummary,
    selectedDetail,
    messageStore
  } = input;

  useEffect(() => {
    messageStore.setActive(threadId);
  }, [messageStore, threadId]);

  useEffect(() => {
    if (!threadId || !selectedThreadSummary) return;
    const slot = messageStore.getSlot(threadId);
    if (!slot.summary) {
      messageStore.applySummary(threadId, selectedThreadSummary);
    }
  }, [messageStore, threadId, selectedThreadSummary]);

  useEffect(() => {
    if (!threadId || !selectedDetail) return;
    messageStore.applyDetail(threadId, selectedDetail);
  }, [messageStore, threadId, selectedDetail]);
}

export {
  mergeMessageBlocks,
  threadDetailFromMessageSlot,
  threadDetailFromMessageSlot as threadDetailFromSlot,
  upsertMessageBlock,
  useThreadMessageStoreController
};

export type {
  ThreadMessageSlot,
  ThreadMessageStoreController
};

export function useThreadsQuery(input: {
  status: string;
  q: string;
  select?: (threads: ThreadSummary[]) => ThreadSummary[];
}) {
  return useQuery({
    queryKey: threadQueryKeys.threads(input.status, input.q),
    queryFn: async () => {
      const threads = await listThreads(input.status, input.q);
      return input.select ? input.select(threads) : threads;
    },
    refetchInterval: 5000,
    staleTime: 3000,
    placeholderData: preservePreviousQueryData
  });
}

export function useThreadDetailQuery(input: {
  threadId: string | null;
  selectedThreadSummary?: ThreadSummary | null;
  select?: (detail: ThreadDetail) => ThreadDetail;
  refetchInterval: (detail: ThreadDetail | undefined, summary?: ThreadSummary | null) => number | false;
}) {
  return useQuery({
    queryKey: threadQueryKeys.thread(input.threadId),
    queryFn: async () => {
      const detail = await getThread(input.threadId!);
      return input.select ? input.select(detail) : detail;
    },
    enabled: Boolean(input.threadId),
    refetchInterval: (query) => input.refetchInterval(query.state.data as ThreadDetail | undefined, input.selectedThreadSummary),
    placeholderData: preservePreviousQueryData
  });
}

export function useReadOnlyThreadActions(input: { onSuccess?: () => void }) {
  const client = useQueryClient();
  return useMutation({
    mutationFn: async (action: { kind: "rename" | "archive" | "restore"; id: string; title?: string }) => {
      if (action.kind === "rename") return renameThread(action.id, action.title ?? "");
      if (action.kind === "archive") return archiveThread(action.id);
      return restoreThread(action.id);
    },
    onSuccess: (_result, action) => {
      void client.invalidateQueries({ queryKey: threadQueryKeys.threads() });
      void client.invalidateQueries({ queryKey: threadQueryKeys.thread(action.id) });
      input.onSuccess?.();
    }
  });
}
