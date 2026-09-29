import type { MessageBlock, ThreadBlockPage, ThreadDetail, ThreadSummary } from "../../types";
import { callCommand } from "./transport";
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
  const page = await callCommand<ThreadBlockPage | {
    threadId: string; blocks: MessageBlock[]; totalBlocks: number; hasMoreBlocks: boolean; beforeCursor?: string | null;
  }>("threads.blocks", { id, options });
  if ("thread_id" in page) return page;
  return {
    thread_id: page.threadId,
    blocks: page.blocks,
    total_blocks: page.totalBlocks,
    has_more_blocks: page.hasMoreBlocks,
    before_cursor: page.beforeCursor
  };
}

export async function archiveThread(threadId: string) {
  return callCommand("threads.archive", { threadId });
}

export async function restoreThread(threadId: string) {
  return callCommand("threads.restore", { threadId });
}

export async function renameThread(threadId: string, name: string) {
  return callCommand("threads.rename", { threadId, name });
}
