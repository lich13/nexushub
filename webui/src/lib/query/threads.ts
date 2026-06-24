import { useMutation, useQuery, useQueryClient, type QueryClient, type QueryKey } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  acceptPlan,
  answerApproval,
  answerElicitation,
  archiveThread,
  cancelFollowUp,
  clearCodexGoal,
  createThread,
  deleteUpload,
  forkThread,
  getCodexGoal,
  getThread,
  getThreadBlocks,
  listFollowUps,
  listPlugins,
  listThreads,
  pauseCodexGoal,
  renameThread,
  restoreThread,
  resumeCodexGoal,
  revisePlan,
  saveCodexGoal,
  sendMessage,
  stopThread,
  steerThread,
  subscribeThreadEvents,
  uploadFiles,
  type ThreadDetailOptions,
  type ThreadSendPayload
} from "../api";
import type {
  BridgeActionResult,
  CodexGoal,
  CodexGoalSaveInput,
  FollowUpQueueState,
  MessageBlock,
  ThreadBlockPage,
  ThreadDetail,
  ThreadStatus,
  ThreadSummary,
  UploadOutcome
} from "../../types";
import type { RuntimeCapabilityMatrix } from "../domain/capabilities";
import {
  actionMessage,
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
  followUps: (threadId: string) => ["thread-followups", threadId] as const,
  plugins: ["plugins"] as const,
  goal: (threadId: string) => ["thread-goal", threadId] as const,
  jobs: ["jobs"] as const
};

type ThreadMessageStoreClear = {
  clear: (threadId: string) => void;
};

type ArchivedThreadCleanupCacheActions = {
  clearArchivedThreadClientState: (messageStore: ThreadMessageStoreClear, threadId: string) => void;
};

type ThreadRefetchType = "active" | "all" | "inactive" | "none";

type ThreadRealtimeMessageStore = {
  isActive: (threadId: string) => boolean;
  applyRealtimeBlocks: (threadId: string, blocks: MessageBlock[]) => void;
  applySummary: (threadId: string, summary: ThreadSummary) => void;
  setFeedback: (threadId: string, message: string | null) => void;
};

type ThreadRealtimeCacheActions = {
  updateThreadListCaches: (summary: ThreadSummary) => void;
  invalidateThreads: (refetchType?: ThreadRefetchType) => void;
  invalidateThread: (threadId: string, refetchType?: ThreadRefetchType) => void;
};

type ThreadRealtimeSubscribe = (
  threadId: string,
  handlers: {
    onBlocks?: (blocks: MessageBlock[], threadId: string) => void;
    onSummary?: (summary: ThreadSummary, threadId: string) => void;
    onError?: (message: string, threadId: string) => void;
  }
) => () => void;

export type ThreadRealtimeSubscriptionInput = {
  threadId: string;
  messageStore: ThreadRealtimeMessageStore;
  threadCache: ThreadRealtimeCacheActions;
  applyThreadTitleOverride?: (summary: ThreadSummary) => ThreadSummary;
  onBeforeActiveBlocks?: () => void;
  subscribe?: ThreadRealtimeSubscribe;
};

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

function identityThreadSummary(summary: ThreadSummary): ThreadSummary {
  return summary;
}

export function connectThreadRealtimeSubscription(input: ThreadRealtimeSubscriptionInput): () => void {
  const transformSummary = input.applyThreadTitleOverride ?? identityThreadSummary;
  const subscribe = input.subscribe ?? subscribeThreadEvents;
  return subscribe(input.threadId, {
    onBlocks: (incomingBlocks, eventThreadId) => {
      if (input.messageStore.isActive(eventThreadId)) {
        input.onBeforeActiveBlocks?.();
      }
      input.messageStore.applyRealtimeBlocks(eventThreadId, incomingBlocks);
    },
    onSummary: (next, eventThreadId) => {
      const stableSummary = transformSummary(next);
      input.messageStore.applySummary(eventThreadId, stableSummary);
      input.threadCache.updateThreadListCaches(stableSummary);
      input.threadCache.invalidateThreads();
    },
    onError: (message, eventThreadId) => {
      input.messageStore.setFeedback(eventThreadId, message);
      input.threadCache.invalidateThread(eventThreadId, "all");
      input.threadCache.invalidateThreads("all");
    }
  });
}

export function useThreadRealtimeSubscription(input: ThreadRealtimeSubscriptionInput): void {
  const {
    threadId,
    messageStore,
    threadCache,
    applyThreadTitleOverride,
    onBeforeActiveBlocks,
    subscribe
  } = input;

  useEffect(() => connectThreadRealtimeSubscription({
    threadId,
    messageStore,
    threadCache,
    applyThreadTitleOverride,
    onBeforeActiveBlocks,
    subscribe
  }), [threadId, messageStore, threadCache, applyThreadTitleOverride, onBeforeActiveBlocks, subscribe]);
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
    invalidateFollowUps: (threadId: string) => qc.invalidateQueries({ queryKey: threadQueryKeys.followUps(threadId) }),
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

export function useSelectedThreadState(threads: ThreadSummary[] = []) {
  const [selectedId, setSelectedId] = useState<SelectedThread>(null);
  const selection = useMemo(
    () => threadSelectionView({ threads, selectedId }),
    [threads, selectedId]
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
}) {
  return useMemo(
    () => selectedThreadDetailView(input),
    [input.detail, input.threadId]
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
}) {
  const {
    threadId,
    selectedId,
    rawSelectedDetail,
    visibleThreads,
    messageStore,
    threadCache,
    onSelect
  } = input;

  useEffect(() => {
    const cleanup = archivedSelectedThreadCleanupView({
      threadId,
      selectedId,
      detail: rawSelectedDetail,
      visibleThreads
    });
    if (!threadId || !cleanup.shouldClearClientState) return;
    threadCache.clearArchivedThreadClientState(messageStore, threadId);
    if (cleanup.nextSelectedId !== selectedId) {
      onSelect(cleanup.nextSelectedId);
    }
  }, [messageStore, onSelect, rawSelectedDetail, selectedId, threadCache, threadId, visibleThreads]);
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

export function usePluginsQuery() {
  return useQuery({
    queryKey: threadQueryKeys.plugins,
    queryFn: listPlugins,
    staleTime: 30000,
    placeholderData: preservePreviousQueryData
  });
}

export function useFollowUpsQuery(threadId: string, running: boolean) {
  return useQuery({
    queryKey: threadQueryKeys.followUps(threadId),
    queryFn: () => listFollowUps(threadId),
    refetchInterval: running ? 3000 : 8000,
    placeholderData: preservePreviousQueryData
  });
}

export function useThreadBlockPageMutation(input: {
  onBeforeLoad: (threadId: string) => number;
  onSuccess: (result: { threadId: string; cursor: string; page: ThreadBlockPage; beforeHeight: number }) => void;
  onError: (error: Error, variables?: { threadId: string; cursor: string }) => void;
}) {
  return useMutation({
    mutationFn: async ({ threadId, cursor }: { threadId: string; cursor: string }) => {
      if (!cursor) throw new Error("没有更早的消息");
      const beforeHeight = input.onBeforeLoad(threadId);
      const page = await getThreadBlocks(threadId, { limit: 120, before: cursor });
      return { threadId, cursor, page, beforeHeight };
    },
    onSuccess: input.onSuccess,
    onError: input.onError
  });
}

export function useThreadActionMutations(input: {
  csrfToken?: string | null;
  capabilities: RuntimeCapabilityMatrix;
  buildPayload: (message: string, config: any, uploads: Pick<{ id: string }, "id">[]) => ThreadSendPayload;
  onSendSuccess: (result: { threadId: string; result: BridgeActionResult }) => void;
  onSteerSuccess: (result: { threadId: string; result: BridgeActionResult }) => void;
  onStopSuccess: (result: { threadId: string }) => void;
  onFollowUpCancelSuccess: (result: { threadId: string }) => void;
  onArchiveSuccess: (result: { threadId: string; wasArchived: boolean }) => void;
  onArchiveMutate: (variables: { threadId: string; status: ThreadStatus }) => Promise<unknown> | unknown;
  onArchiveError: (error: Error, variables?: { threadId: string; status: ThreadStatus }, context?: unknown) => void;
  onArchiveSettled: (variables?: { threadId: string; status: ThreadStatus }) => void;
  onRenameMutate: (variables: { threadId: string; title: string }) => Promise<unknown> | unknown;
  onRenameSuccess: (result: { threadId: string; title: string }) => void;
  onRenameError: (error: Error, variables?: { threadId: string; title: string }, context?: unknown) => void;
  onRenameSettled: (variables?: { threadId: string; title: string }) => void;
  onForkSuccess: (result: { threadId: string; result: BridgeActionResult }) => void;
  onBridgeActionSuccess: (result: { threadId: string; result: BridgeActionResult }) => void;
  onActionError: (error: Error, variables?: { threadId?: string }) => void;
}) {
  const csrfToken = input.csrfToken;

  return {
    send: useMutation({
      mutationFn: async ({ threadId, message, config, uploads }: { threadId: string; message: string; config: any; uploads: Pick<{ id: string }, "id">[] }) => ({
        threadId,
        result: await sendMessage(threadId, input.buildPayload(message, config, uploads), csrfToken)
      }),
      onSuccess: input.onSendSuccess,
      onError: input.onActionError
    }),
    steer: useMutation({
      mutationFn: async ({ threadId, message, config, uploads }: { threadId: string; message: string; config: any; uploads: Pick<{ id: string }, "id">[] }) => ({
        threadId,
        result: await steerThread(threadId, input.buildPayload(message, config, uploads), csrfToken)
      }),
      onSuccess: input.onSteerSuccess,
      onError: input.onActionError
    }),
    stop: useMutation({
      mutationFn: async ({ threadId, turnId, jobId }: { threadId: string; turnId?: string | null; jobId?: string | null }) => {
        await stopThread(threadId, { turn_id: turnId, job_id: jobId }, csrfToken);
        return { threadId };
      },
      onSuccess: input.onStopSuccess,
      onError: input.onActionError
    }),
    followUpCancel: useMutation({
      mutationFn: async ({ threadId, followUpId }: { threadId: string; followUpId: string }) => {
        await cancelFollowUp(threadId, followUpId, csrfToken);
        return { threadId };
      },
      onSuccess: input.onFollowUpCancelSuccess,
      onError: input.onActionError
    }),
    archive: useMutation({
      mutationFn: async ({ threadId, status }: { threadId: string; status: ThreadStatus }) => {
        if (!input.capabilities.threadArchiveActions) {
          throw new Error("当前运行时不支持归档操作");
        }
        const wasArchived = status === "Archived";
        if (wasArchived) {
          await restoreThread(threadId, csrfToken);
        } else {
          await archiveThread(threadId, csrfToken);
        }
        return { threadId, wasArchived };
      },
      onMutate: input.onArchiveMutate,
      onSuccess: input.onArchiveSuccess,
      onError: input.onArchiveError,
      onSettled: (_data, _error, variables) => input.onArchiveSettled(variables)
    }),
    rename: useMutation({
      mutationFn: async ({ threadId, title: requestedTitle }: { threadId: string; title: string }) => {
        const title = requestedTitle.trim();
        await renameThread(threadId, requestedTitle, csrfToken);
        return { threadId, title };
      },
      onMutate: input.onRenameMutate,
      onSuccess: input.onRenameSuccess,
      onError: input.onRenameError,
      onSettled: (_data, _error, variables) => input.onRenameSettled(variables)
    }),
    fork: useMutation({
      mutationFn: async ({ threadId }: { threadId: string }) => ({
        threadId,
        result: await forkThread(threadId, csrfToken)
      }),
      onSuccess: input.onForkSuccess,
      onError: input.onActionError
    }),
    answer: useMutation({
      mutationFn: async ({ threadId, answers }: { threadId: string; answers: Record<string, string[]> }) => ({
        threadId,
        result: await answerElicitation(threadId, answers, csrfToken)
      }),
      onSuccess: input.onBridgeActionSuccess,
      onError: input.onActionError
    }),
    planAccept: useMutation({
      mutationFn: async ({ threadId, block }: { threadId: string; block: MessageBlock }) => ({
        threadId,
        result: await acceptPlan(threadId, { turn_id: block.turn_id, item_id: block.item_id }, csrfToken)
      }),
      onSuccess: input.onBridgeActionSuccess,
      onError: input.onActionError
    }),
    planRevise: useMutation({
      mutationFn: async ({ threadId, block, instructions }: { threadId: string; block: MessageBlock; instructions: string }) => ({
        threadId,
        result: await revisePlan(threadId, { turn_id: block.turn_id, item_id: block.item_id, instructions }, csrfToken)
      }),
      onSuccess: input.onBridgeActionSuccess,
      onError: input.onActionError
    }),
    approval: useMutation({
      mutationFn: async ({ threadId, block, decision }: { threadId: string; block: MessageBlock; decision: string }) => ({
        threadId,
        result: await answerApproval(threadId, { turn_id: block.turn_id, item_id: block.item_id ?? block.call_id, decision }, csrfToken)
      }),
      onSuccess: input.onBridgeActionSuccess,
      onError: input.onActionError
    })
  };
}

export function useThreadConversationActions(input: {
  csrfToken?: string | null;
  capabilities: RuntimeCapabilityMatrix;
  messageStore: ThreadMessageStoreController;
  buildPayload: (message: string, config: any, uploads: Pick<{ id: string }, "id">[]) => ThreadSendPayload;
  activeThreadId: string;
  fallbackRenameTitle: string;
  nextThreadAfterArchive: SelectedThread;
  onActiveMessageAccepted: () => void;
  onArchiveSelectionChange: (threadId: SelectedThread) => void;
  onRenameDraftCommitted: (title: string) => void;
  onRenameDraftRestored: (title: string) => void;
  onForkedThread: (threadId: string) => void;
}) {
  const threadCache = useThreadCacheActions();
  const {
    messageStore,
    activeThreadId
  } = input;

  return useThreadActionMutations({
    csrfToken: input.csrfToken,
    capabilities: input.capabilities,
    buildPayload: input.buildPayload,
    onSendSuccess: ({ threadId: resultThreadId, result }) => {
      messageStore.setLastResult(resultThreadId, result);
      if (result.job_id || result.turn_id) {
        messageStore.patchSummary(resultThreadId, (current) => ({
          ...current,
          status: "Running",
          active_turn_id: result.turn_id ?? current.active_turn_id,
          active_job_id: result.job_id ?? current.active_job_id
        }));
      }
      if (messageStore.isActive(resultThreadId)) {
        input.onActiveMessageAccepted();
      }
      messageStore.setFeedback(resultThreadId, actionMessage(result));
      threadCache.invalidateJobs();
      threadCache.invalidateThreads();
      threadCache.invalidateThread(resultThreadId);
    },
    onStopSuccess: ({ threadId: stoppedThreadId }) => {
      messageStore.setFeedback(stoppedThreadId, "停止请求已发送");
      threadCache.invalidateThreads();
      threadCache.invalidateThread(stoppedThreadId);
    },
    onSteerSuccess: ({ threadId: resultThreadId, result }) => {
      messageStore.setLastResult(resultThreadId, result);
      if (messageStore.isActive(resultThreadId)) {
        input.onActiveMessageAccepted();
      }
      messageStore.setFeedback(resultThreadId, actionMessage(result));
      threadCache.invalidateFollowUps(resultThreadId);
      threadCache.invalidateThreads();
      threadCache.invalidateThread(resultThreadId);
    },
    onFollowUpCancelSuccess: ({ threadId: cancelledThreadId }) => {
      messageStore.setFeedback(cancelledThreadId, "跟进已取消");
      threadCache.invalidateFollowUps(cancelledThreadId);
    },
    onArchiveMutate: async (variables) => {
      await threadCache.cancelThreadsAndThread(variables.threadId);
      const wasArchived = variables.status === "Archived";
      const snapshot = wasArchived
        ? threadCache.applyOptimisticThreadRestore(variables.threadId)
        : threadCache.applyOptimisticThreadArchive(messageStore, variables.threadId);
      if (!wasArchived) {
        input.onArchiveSelectionChange(input.nextThreadAfterArchive);
      }
      return { snapshot, wasArchived };
    },
    onArchiveSuccess: ({ threadId: archivedThreadId, wasArchived }) => {
      messageStore.setFeedback(archivedThreadId, wasArchived ? "恢复请求已提交" : "归档请求已提交");
    },
    onArchiveError: (err, variables, context) => {
      const archiveContext = context as { snapshot?: ThreadCacheSnapshot; wasArchived?: boolean } | undefined;
      if (archiveContext?.wasArchived) {
        threadCache.rollbackOptimisticThreadRestore(archiveContext.snapshot);
      } else {
        threadCache.rollbackOptimisticThreadArchive(archiveContext?.snapshot);
        if (variables?.threadId) {
          input.onArchiveSelectionChange(variables.threadId);
        }
      }
      messageStore.setFeedback(variables?.threadId ?? activeThreadId, err.message);
    },
    onArchiveSettled: (variables) => {
      threadCache.invalidateThreads();
      if (variables?.threadId) {
        threadCache.invalidateThread(variables.threadId);
      }
    },
    onRenameMutate: async (variables) => {
      const title = variables.title.trim();
      await threadCache.cancelThreadsAndThread(variables.threadId);
      const snapshot = threadCache.applyOptimisticThreadTitle(variables.threadId, title);
      if (title) {
        setLocalThreadTitleOverride(variables.threadId, title);
        input.onRenameDraftCommitted(title);
        messageStore.patchSummary(variables.threadId, { title });
      }
      return { snapshot };
    },
    onRenameSuccess: ({ threadId: renamedThreadId, title }) => {
      messageStore.setFeedback(renamedThreadId, "线程名称已更新");
      if (title) {
        setLocalThreadTitleOverride(renamedThreadId, title);
        threadCache.applyOptimisticThreadTitle(renamedThreadId, title);
      }
    },
    onRenameError: (err, variables, context) => {
      const renameContext = context as { snapshot?: ThreadCacheSnapshot } | undefined;
      if (variables?.threadId) {
        clearLocalThreadTitleOverride(variables.threadId);
      }
      threadCache.rollbackOptimisticThreadTitle(renameContext?.snapshot);
      const failedThreadId = variables?.threadId ?? activeThreadId;
      const restoredTitle = threadCache.cachedThreadSummary(failedThreadId)?.title ?? input.fallbackRenameTitle;
      if (variables?.threadId === activeThreadId && restoredTitle) {
        input.onRenameDraftRestored(restoredTitle);
        messageStore.patchSummary(variables.threadId, { title: restoredTitle });
      }
      messageStore.setFeedback(failedThreadId, err.message);
    },
    onRenameSettled: (variables) => {
      threadCache.invalidateThreads();
      if (variables?.threadId) {
        threadCache.invalidateThread(variables.threadId);
      }
    },
    onForkSuccess: ({ threadId: forkedThreadId, result }) => {
      messageStore.setLastResult(forkedThreadId, result);
      messageStore.setFeedback(forkedThreadId, actionMessage(result));
      if (result.thread_id) input.onForkedThread(result.thread_id);
      threadCache.invalidateThreads();
    },
    onBridgeActionSuccess: ({ threadId: actionThreadId, result }) => {
      messageStore.setLastResult(actionThreadId, result);
      messageStore.setFeedback(actionThreadId, actionMessage(result));
      threadCache.invalidateThreads();
      threadCache.invalidateThread(actionThreadId);
    },
    onActionError: (err, variables) => messageStore.setFeedback(variables?.threadId ?? activeThreadId, err.message)
  });
}

export function useThreadGoalQuery(threadId: string) {
  return useQuery({
    queryKey: threadQueryKeys.goal(threadId),
    queryFn: () => getCodexGoal(threadId),
    enabled: Boolean(threadId),
    staleTime: 5000,
    refetchInterval: 15000,
    placeholderData: preservePreviousQueryData
  });
}

export function useThreadGoalActions(input: {
  threadId: string;
  csrfToken?: string | null;
  saveInput: () => CodexGoalSaveInput;
  onSuccess: (goal: CodexGoal, message: string) => void;
  onError: (error: Error) => void;
}) {
  const qc = useQueryClient();
  const handleSuccess = (goal: CodexGoal, message: string) => {
    qc.setQueryData<CodexGoal>(threadQueryKeys.goal(input.threadId), goal);
    input.onSuccess(goal, message);
    qc.invalidateQueries({ queryKey: threadQueryKeys.goal(input.threadId) });
  };
  return {
    save: useMutation({
      mutationFn: () => saveCodexGoal(input.threadId, input.saveInput(), input.csrfToken),
      onSuccess: (goal) => handleSuccess(goal, "Goal 已保存"),
      onError: input.onError
    }),
    clear: useMutation({
      mutationFn: () => clearCodexGoal(input.threadId, input.csrfToken),
      onSuccess: (goal) => handleSuccess(goal, "Goal 已清除"),
      onError: input.onError
    }),
    pause: useMutation({
      mutationFn: () => pauseCodexGoal(input.threadId, input.csrfToken),
      onSuccess: (goal) => handleSuccess(goal, "Goal 已暂停"),
      onError: input.onError
    }),
    resume: useMutation({
      mutationFn: () => resumeCodexGoal(input.threadId, input.csrfToken),
      onSuccess: (goal) => handleSuccess(goal, "Goal 已恢复"),
      onError: input.onError
    })
  };
}

export function useCreateThreadMutation(input: {
  csrfToken?: string | null;
  payload: (message: string) => ThreadSendPayload;
  onSuccess: (result: BridgeActionResult) => void;
  onError: (error: Error) => void;
}) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ message }: { message: string }) => createThread(input.payload(message), input.csrfToken),
    onSuccess: (result) => {
      input.onSuccess(result);
      qc.invalidateQueries({ queryKey: threadQueryKeys.threads() });
      qc.invalidateQueries({ queryKey: threadQueryKeys.jobs });
    },
    onError: input.onError
  });
}

export function useUploadActions(input: {
  csrfToken?: string | null;
  onUploaded?: (outcome: UploadOutcome) => void;
  onDeleted?: (id: string) => void;
}) {
  return {
    upload: (files: File[]) => uploadFiles(files, input.csrfToken).then((outcome) => {
      input.onUploaded?.(outcome);
      return outcome;
    }),
    delete: (id: string) => deleteUpload(id, input.csrfToken).then((result) => {
      input.onDeleted?.(id);
      return result;
    })
  };
}

export { subscribeThreadEvents };
export type { FollowUpQueueState, ThreadDetailOptions, ThreadSendPayload };
