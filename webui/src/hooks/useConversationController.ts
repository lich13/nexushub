import { useCallback, useEffect, useRef, useState, type MutableRefObject, type RefObject } from "react";
import {
  applyThreadTitleOverride,
  applyThreadTitleOverrideToDetail,
  applyThreadTitleOverrides,
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
  useThreadRealtimeSubscription,
  useThreadsQuery,
  type ThreadMessageStoreController
} from "../lib/query/threads";
import type { ThreadSummary } from "../types";

export function useConversationController(input: {
  status: string;
  q: string;
  setMobileThreadsOpen: (open: boolean) => void;
}) {
  const threadCache = useThreadCacheActions();
  const messageStore = useThreadMessageStoreController();
  const threads = useThreadsQuery({
    status: input.status,
    q: input.q,
    select: applyThreadTitleOverrides
  });
  const selection = useSelectedThreadState(threads.data ?? []);
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
    select: applyThreadTitleOverrideToDetail,
    refetchInterval: threadDetailRefetchInterval
  });
  const { rawSelectedDetail, selectedDetail } = useThreadDetailHydration({
    threadId: resolvedSelected,
    detail: detail.data
  });

  useArchivedSelectedThreadCleanup({
    threadId: resolvedSelected,
    selectedId,
    rawSelectedDetail,
    visibleThreads,
    messageStore,
    threadCache,
    onSelect: setSelectedId
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
    visibleThreads,
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

  useThreadRealtimeSubscription({
    threadId: input.threadId,
    messageStore: input.messageStore,
    threadCache,
    applyThreadTitleOverride: applyThreadTitleOverride as (summary: ThreadSummary) => ThreadSummary,
    onBeforeActiveBlocks: updateMessageFollowState
  });

  return {
    explicitBottomFollowRevision,
    previousThreadIdRef,
    updateMessageFollowState,
    followNextMessageUpdate
  };
}

export { threadDetailFromSlot };
