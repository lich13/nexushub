import type { MessageBlock, ThreadBlockPage, ThreadDetail, ThreadSummary } from "../../types";
import { callCommand, openThreadEventStream } from "./transport";
import { USE_DEMO } from "./shared";
import { demoThreadBlockPage, demoThreadDetail, demoThreads } from "./demo";

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

export async function archiveThread(threadId: string, csrfToken?: string | null) {
  return callCommand("threads.archive", { threadId, csrfToken });
}

export async function restoreThread(threadId: string, csrfToken?: string | null) {
  return callCommand("threads.restore", { threadId, csrfToken });
}

export async function renameThread(threadId: string, name: string, csrfToken?: string | null) {
  return callCommand("threads.rename", { threadId, name, csrfToken });
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
