import type {
  BridgeActionResult,
  CodexGoal,
  CodexGoalSaveInput,
  FollowUpQueueItem,
  FollowUpQueueState,
  MessageBlock,
  ThreadBlockPage,
  ThreadDetail,
  ThreadSummary,
  UploadOutcome
} from "../../types";
import {
  callCommand,
  openThreadEventStream,
  uploadFilesTransport
} from "./transport";
import { USE_DEMO } from "./shared";
import {
  demoBridgeActionResult,
  demoClearedCodexGoal,
  demoCodexGoal,
  demoCreatedThreadResult,
  demoDeletedUpload,
  demoEnqueuedFollowUp,
  demoFollowUps,
  demoOk,
  demoPausedCodexGoal,
  demoResumedCodexGoal,
  demoSavedCodexGoal,
  demoThreadBlockPage,
  demoThreadDetail,
  demoThreads,
  demoUploadOutcome
} from "./demo";

export async function listThreads(status: string, q: string): Promise<ThreadSummary[]> {
  if (USE_DEMO) return demoThreads(status, q);
  return callCommand<ThreadSummary[]>("threads.list", { status, q, limit: 120 });
}

export type ThreadDetailOptions = {
  limit?: number;
  before?: string | null;
  full?: boolean;
};

export async function getThread(id: string, options: ThreadDetailOptions = {}): Promise<ThreadDetail> {
  if (USE_DEMO) {
    return demoThreadDetail(id);
  }
  return callCommand<ThreadDetail>("threads.detail", { id, options });
}

export async function getThreadBlocks(id: string, options: Pick<ThreadDetailOptions, "limit" | "before"> = {}): Promise<ThreadBlockPage> {
  if (USE_DEMO) {
    return demoThreadBlockPage(id);
  }
  return callCommand<ThreadBlockPage>("threads.blocks", { id, options });
}

export type ThreadSendPayload = {
  message: string;
  attachments?: string[];
  model?: string | null;
  service_tier?: string | null;
  reasoning_effort?: string | null;
  cwd?: string | null;
  permission_profile?: string | null;
  approval_policy?: string | null;
  sandbox_mode?: string | null;
  network_access?: boolean | null;
  collaboration_mode?: string | null;
};

export async function uploadFiles(files: File[], csrfToken?: string | null): Promise<UploadOutcome> {
  if (USE_DEMO) {
    return demoUploadOutcome(files);
  }
  return uploadFilesTransport<UploadOutcome>(files, csrfToken);
}

export async function deleteUpload(id: string, csrfToken?: string | null): Promise<{ ok: boolean; deleted: boolean }> {
  if (USE_DEMO) return demoDeletedUpload();
  return callCommand<{ ok: boolean; deleted: boolean }>("uploads.delete", { id, csrfToken });
}

export async function createThread(payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return demoCreatedThreadResult();
  return callCommand<BridgeActionResult>("threads.create", { payload, csrfToken });
}

export async function sendMessage(threadId: string, payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return demoBridgeActionResult(threadId);
  return callCommand<BridgeActionResult>("threads.send", { threadId, payload, csrfToken });
}

export async function steerThread(threadId: string, payload: ThreadSendPayload, csrfToken?: string | null): Promise<BridgeActionResult> {
  if (USE_DEMO) return demoBridgeActionResult(threadId);
  return callCommand<BridgeActionResult>("threads.steer", { threadId, payload, csrfToken });
}

export async function listFollowUps(threadId: string): Promise<FollowUpQueueState> {
  if (USE_DEMO) return demoFollowUps();
  const result = await callCommand<FollowUpQueueState | FollowUpQueueItem[]>("threads.followups.list", { threadId });
  return Array.isArray(result) ? { items: result } : result;
}

export async function enqueueFollowUp(threadId: string, payload: ThreadSendPayload, csrfToken?: string | null): Promise<FollowUpQueueItem> {
  if (USE_DEMO) {
    return demoEnqueuedFollowUp(threadId, payload);
  }
  return callCommand<FollowUpQueueItem>("threads.followups.enqueue", { threadId, payload, csrfToken });
}

export async function cancelFollowUp(threadId: string, followUpId: string, csrfToken?: string | null): Promise<{ ok: boolean }> {
  if (USE_DEMO) return demoOk();
  return callCommand<{ ok: boolean }>("threads.followups.cancel", { threadId, followUpId, csrfToken });
}

export async function stopThread(threadId: string, payload: { turn_id?: string | null; job_id?: string | null }, csrfToken?: string | null) {
  if (USE_DEMO) return demoOk();
  return callCommand("threads.stop", { threadId, payload, csrfToken });
}

export async function archiveThread(threadId: string, csrfToken?: string | null) {
  return callCommand("threads.archive", { threadId, csrfToken });
}

export async function restoreThread(threadId: string, csrfToken?: string | null) {
  return callCommand("threads.restore", { threadId, csrfToken });
}

export async function renameThread(threadId: string, name: string, csrfToken?: string | null) {
  return callCommand("threads.rename", { threadId, name, csrfToken });
}

export async function forkThread(threadId: string, csrfToken?: string | null): Promise<BridgeActionResult> {
  return callCommand<BridgeActionResult>("threads.fork", { threadId, csrfToken });
}

export async function answerElicitation(threadId: string, answers: Record<string, string[]>, csrfToken?: string | null): Promise<BridgeActionResult> {
  return callCommand<BridgeActionResult>("threads.elicitation.answer", { threadId, answers, csrfToken });
}

export async function acceptPlan(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  return callCommand<BridgeActionResult>("threads.plan.accept", { threadId, payload, csrfToken });
}

export async function revisePlan(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null; instructions: string },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  return callCommand<BridgeActionResult>("threads.plan.revise", { threadId, payload, csrfToken });
}

export async function answerApproval(
  threadId: string,
  payload: { turn_id?: string | null; item_id?: string | null; request_id?: string | null; decision: string },
  csrfToken?: string | null
): Promise<BridgeActionResult> {
  return callCommand<BridgeActionResult>("threads.approval.answer", { threadId, payload, csrfToken });
}

export async function getCodexGoal(threadId: string): Promise<CodexGoal> {
  if (USE_DEMO) return demoCodexGoal(threadId);
  const result = await callCommand<CodexGoal | { goal?: CodexGoal | null }>("threads.goal.get", { threadId });
  return result && typeof result === "object" && "goal" in result
    ? result.goal ?? demoCodexGoal(threadId)
    : result as CodexGoal;
}

export async function saveCodexGoal(threadId: string, goal: CodexGoalSaveInput, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return demoSavedCodexGoal(threadId, goal);
  }
  return callCommand<CodexGoal>("threads.goal.save", {
    threadId,
    objective: goal.objective,
    tokenBudget: goal.token_budget ?? null,
    csrfToken
  });
}

export async function clearCodexGoal(threadId: string, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return demoClearedCodexGoal(threadId);
  }
  return callCommand<CodexGoal>("threads.goal.clear", { threadId, csrfToken });
}

export async function pauseCodexGoal(threadId: string, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return demoPausedCodexGoal(threadId);
  }
  return callCommand<CodexGoal>("threads.goal.pause", { threadId, csrfToken });
}

export async function resumeCodexGoal(threadId: string, csrfToken?: string | null): Promise<CodexGoal> {
  if (USE_DEMO) {
    return demoResumedCodexGoal(threadId);
  }
  return callCommand<CodexGoal>("threads.goal.resume", { threadId, csrfToken });
}

export function subscribeThreadEvents(
  threadId: string,
  handlers: { onBlock?: (block: MessageBlock, threadId: string) => void; onBlocks?: (blocks: MessageBlock[], threadId: string) => void; onSummary?: (summary: ThreadSummary, threadId: string) => void; onError?: (message: string, threadId: string) => void }
): () => void {
  if (USE_DEMO) return () => {};
  const source = openThreadEventStream(threadId);
  if (source.unavailable) return () => {};
  let pendingBlocks: MessageBlock[] = [];
  let flushTimer: ReturnType<typeof setTimeout> | null = null;
  const flushBlocks = () => {
    flushTimer = null;
    if (!pendingBlocks.length) return;
    const blocks = pendingBlocks;
    pendingBlocks = [];
    handlers.onBlocks?.(blocks, threadId);
  };
  source.addEventListener("block", (event) => {
    const block = JSON.parse((event as MessageEvent).data) as MessageBlock;
    handlers.onBlock?.(block, threadId);
    if (handlers.onBlocks) {
      pendingBlocks.push(block);
      if (!flushTimer) flushTimer = setTimeout(flushBlocks, 100);
    }
  });
  source.addEventListener("summary", (event) => handlers.onSummary?.(JSON.parse((event as MessageEvent).data), threadId));
  source.addEventListener("error", (event) => {
    const data = (event as MessageEvent).data;
    handlers.onError?.(data ? String(data) : "stream disconnected", threadId);
  });
  return () => {
    if (flushTimer) {
      clearTimeout(flushTimer);
      flushBlocks();
    }
    source.close();
  };
}
