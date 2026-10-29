import { useCallback, useEffect, useMemo, useRef, useState, type MutableRefObject, type RefObject } from "react";
import {
  applyThreadTitleOverrideToDetail,
  applyThreadTitleOverrides,
  filterVisibleThreadSummaries,
  threadDetailRefetchInterval,
  type SelectedThread
} from "../lib/domain/codexViewModel";
import {
  threadDetailFromSlot,
  useArchivedSelectedThreadCleanup,
  useHydrateThreadMessageStore,
  useSelectedThreadState,
  useThreadCacheActions,
  useThreadDetailHydration,
  useThreadDetailQuery,
  useThreadMessageStoreController,
  useThreadsQuery,
  type ThreadMessageStoreController
} from "../lib/query/threads";

export function useConversationController(input: {
  archivedOpen: boolean;
  q: string;
  setMobileThreadsOpen: (open: boolean) => void;
}) {
  const threadCache = useThreadCacheActions();
  const messageStore = useThreadMessageStoreController();
  const threads = useThreadsQuery({
    archived: false,
    q: input.q,
    select: applyThreadTitleOverrides
  });
  const archivedThreads = useThreadsQuery({
    archived: true,
    q: input.q,
    enabled: input.archivedOpen,
    select: applyThreadTitleOverrides
  });
  const includeArchived = input.archivedOpen;
  const combinedThreads = useMemo(
    () => includeArchived ? [...(threads.data ?? []), ...(archivedThreads.data ?? [])] : (threads.data ?? []),
    [archivedThreads.data, includeArchived, threads.data]
  );
  const activeThreads = useMemo(
    () => filterVisibleThreadSummaries(threads.data ?? [], false),
    [threads.data]
  );
  const visibleArchivedThreads = useMemo(
    () => includeArchived
      ? filterVisibleThreadSummaries(archivedThreads.data ?? [], true).filter(
          (thread) => thread.status === "Archived" || Boolean(thread.archived_at)
        )
      : [],
    [archivedThreads.data, includeArchived]
  );
  const selection = useSelectedThreadState(combinedThreads, includeArchived);
  const {
    selectedId,
    selectThread: setSelectedId,
    visibleThreads,
    resolvedSelected,
    selectedThreadSummary,
    nextThreadAfterRemoval
  } = selection;
  const detail = useThreadDetailQuery({
    threadId: resolvedSelected,
    selectedThreadSummary,
    snapshot: () => resolvedSelected ? threadDetailFromSlot(resolvedSelected, messageStore.getSlot(resolvedSelected), selectedThreadSummary) : undefined,
    select: applyThreadTitleOverrideToDetail,
    refetchInterval: threadDetailRefetchInterval
  });
  const { rawSelectedDetail, selectedDetail } = useThreadDetailHydration({
    threadId: resolvedSelected,
    detail: detail.data,
    includeArchived
  });

  useArchivedSelectedThreadCleanup({
    threadId: resolvedSelected,
    selectedId,
    rawSelectedDetail,
    visibleThreads,
    messageStore,
    threadCache,
    onSelect: setSelectedId,
    includeArchived
  });

  useEffect(() => {
    if (!resolvedSelected || !selectedThreadSummary) return;
    threadCache.mergeThreadDetailSummary(resolvedSelected, selectedThreadSummary);
  }, [threadCache, resolvedSelected, selectedThreadSummary]);

  useHydrateThreadMessageStore({
    threadId: resolvedSelected,
    selectedThreadSummary,
    selectedDetail,
    messageStore
  });

  const selectThread = useCallback((id: SelectedThread) => {
    setSelectedId(id);
    input.setMobileThreadsOpen(false);
  }, [input, setSelectedId]);

  return {
    threadCache,
    messageStore,
    threads,
    archivedThreads,
    visibleThreads: activeThreads,
    visibleArchivedThreads,
    resolvedSelected,
    selectedThreadSummary,
    selectedDetail,
    detailLoading: detail.isLoading,
    nextThreadAfterRemoval,
    selectThread
  };
}

export function useConversationRealtimeController(input: {
  threadId: string;
  messageStore: ThreadMessageStoreController;
  messageStreamRef: RefObject<HTMLDivElement>;
  shouldFollowMessagesRef: MutableRefObject<boolean>;
  shouldAutoFollowMessageStream: (snapshot: { scrollTop: number; clientHeight: number; scrollHeight: number }) => boolean;
}) {
  const threadCache = useThreadCacheActions();
  const [explicitBottomFollowRevision, setExplicitBottomFollowRevision] = useState(0);
  const previousThreadIdRef = useRef(input.threadId);

  const updateMessageFollowState = useCallback(() => {
    input.shouldFollowMessagesRef.current = input.messageStreamRef.current
      ? input.shouldAutoFollowMessageStream(input.messageStreamRef.current)
      : true;
  }, [input]);

  const followNextMessageUpdate = useCallback(() => {
    input.shouldFollowMessagesRef.current = true;
    setExplicitBottomFollowRevision((revision) => revision + 1);
  }, [input]);


  return {
    explicitBottomFollowRevision,
    previousThreadIdRef,
    updateMessageFollowState,
    followNextMessageUpdate
  };
}

export { threadDetailFromSlot };
