import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
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

export const threadQueryKeys = {
  threads: (status?: string, q?: string) => status === undefined && q === undefined ? ["threads"] as const : ["threads", status, q] as const,
  thread: (threadId: string | null) => ["thread", threadId] as const,
  threadBlocks: (threadId: string) => ["thread-blocks", threadId] as const,
  followUps: (threadId: string) => ["thread-followups", threadId] as const,
  plugins: ["plugins"] as const,
  goal: (threadId: string) => ["thread-goal", threadId] as const,
  jobs: ["jobs"] as const
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
    placeholderData: keepPreviousData
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
    placeholderData: keepPreviousData
  });
}

export function usePluginsQuery() {
  return useQuery({
    queryKey: threadQueryKeys.plugins,
    queryFn: listPlugins,
    staleTime: 30000,
    placeholderData: keepPreviousData
  });
}

export function useFollowUpsQuery(threadId: string, running: boolean) {
  return useQuery({
    queryKey: threadQueryKeys.followUps(threadId),
    queryFn: () => listFollowUps(threadId),
    refetchInterval: running ? 3000 : 8000,
    placeholderData: keepPreviousData
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
  onCancelFollowUpSuccess: (result: { threadId: string }) => void;
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
    cancelFollowUp: useMutation({
      mutationFn: async ({ threadId, followUpId }: { threadId: string; followUpId: string }) => {
        await cancelFollowUp(threadId, followUpId, csrfToken);
        return { threadId };
      },
      onSuccess: input.onCancelFollowUpSuccess,
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

export function useThreadGoalQuery(threadId: string) {
  return useQuery({
    queryKey: threadQueryKeys.goal(threadId),
    queryFn: () => getCodexGoal(threadId),
    enabled: Boolean(threadId),
    staleTime: 5000,
    refetchInterval: 15000,
    placeholderData: keepPreviousData
  });
}

export function useThreadGoalActions(input: {
  threadId: string;
  csrfToken?: string | null;
  saveInput: () => CodexGoalSaveInput;
  onSuccess: (goal: CodexGoal, message: string) => void;
  onError: (error: Error) => void;
}) {
  return {
    save: useMutation({
      mutationFn: () => saveCodexGoal(input.threadId, input.saveInput(), input.csrfToken),
      onSuccess: (goal) => input.onSuccess(goal, "Goal 已保存"),
      onError: input.onError
    }),
    clear: useMutation({
      mutationFn: () => clearCodexGoal(input.threadId, input.csrfToken),
      onSuccess: (goal) => input.onSuccess(goal, "Goal 已清除"),
      onError: input.onError
    }),
    pause: useMutation({
      mutationFn: () => pauseCodexGoal(input.threadId, input.csrfToken),
      onSuccess: (goal) => input.onSuccess(goal, "Goal 已暂停"),
      onError: input.onError
    }),
    resume: useMutation({
      mutationFn: () => resumeCodexGoal(input.threadId, input.csrfToken),
      onSuccess: (goal) => input.onSuccess(goal, "Goal 已恢复"),
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
  return useMutation({
    mutationFn: ({ message }: { message: string }) => createThread(input.payload(message), input.csrfToken),
    onSuccess: input.onSuccess,
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

function keepPreviousData<T>(previous: T | undefined): T | undefined {
  return previous;
}
