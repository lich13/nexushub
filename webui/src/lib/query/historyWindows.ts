import { replaceEqualDeep, type InfiniteData } from "@tanstack/react-query";
import type { ClaudeSessionDetail, SubagentDetailResponse } from "../../types";
import { mergeBlocksPreservingHistory } from "../threadMessageStore";

type WindowBoundary = { hasMore?: boolean; cursor?: string | null };
type HistoryPageAdapter<Page> = {
  identity: (page: Page) => string;
  total: (page: Page) => number | undefined;
  merge: (previous: Page, incoming: Page) => Page;
};

function retainedBoundary(previous: WindowBoundary, incoming: WindowBoundary): WindowBoundary {
  // Polling moves the newest window forward. Paging must continue from the
  // earliest boundary already loaded, not from that sliding window.
  if (previous.hasMore === false) return previous;
  if (incoming.hasMore === false) return incoming;
  return { hasMore: incoming.hasMore, cursor: previous.cursor ?? incoming.cursor };
}

function retainHistoryWindows<Page>(oldData: unknown, newData: unknown, adapter: HistoryPageAdapter<Page>): InfiniteData<Page> {
  const previous = oldData as InfiniteData<Page> | undefined;
  const incoming = newData as InfiniteData<Page>;
  const oldHead = previous?.pages[0];
  const newHead = incoming.pages[0];
  if (!oldHead || !newHead || adapter.identity(oldHead) !== adapter.identity(newHead)) return incoming;
  const oldTotal = adapter.total(oldHead);
  const newTotal = adapter.total(newHead);
  // A shortened native stream invalidates the old positional cursors.
  if (oldTotal !== undefined && newTotal !== undefined && newTotal < oldTotal) return incoming;

  const pages = incoming.pages.map((page, index) => {
    const oldPage = previous.pages[index];
    return oldPage && adapter.identity(oldPage) === adapter.identity(page) ? adapter.merge(oldPage, page) : page;
  });
  // A refreshed window may already reach the beginning and return fewer pages.
  // Keep rendered messages until the thread identity changes or is invalidated.
  pages.push(...previous.pages.slice(pages.length));
  const pageParams = pages.map((_, index) => index < previous.pageParams.length ? previous.pageParams[index] : incoming.pageParams[index]);
  return replaceEqualDeep(previous, { ...incoming, pages, pageParams });
}

export function retainClaudeHistory(previous: unknown, incoming: unknown): InfiniteData<ClaudeSessionDetail> {
  return retainHistoryWindows<ClaudeSessionDetail>(previous, incoming, {
    identity: page => JSON.stringify([page.summary.sessionKey, page.summary.id]),
    total: page => page.totalEvents,
    merge: (oldPage, page) => {
      const boundary = retainedBoundary(
        { hasMore: oldPage.hasMore, cursor: oldPage.beforeCursor },
        { hasMore: page.hasMore, cursor: page.beforeCursor }
      );
      return { ...page, events: mergeBlocksPreservingHistory(oldPage.events, page.events), hasMore: Boolean(boundary.hasMore), beforeCursor: boundary.cursor };
    }
  });
}

export function retainSubagentHistory(previous: unknown, incoming: unknown): InfiniteData<SubagentDetailResponse> {
  return retainHistoryWindows<SubagentDetailResponse>(previous, incoming, {
    identity: page => JSON.stringify([page.rootThreadId, page.parentThreadId, page.agent.agentId, page.detail.summary.id]),
    total: page => page.detail.total_blocks,
    merge: (oldPage, page) => {
      const boundary = retainedBoundary(
        { hasMore: oldPage.detail.has_more_blocks, cursor: oldPage.detail.before_cursor },
        { hasMore: page.detail.has_more_blocks, cursor: page.detail.before_cursor }
      );
      return { ...page, detail: { ...page.detail, blocks: mergeBlocksPreservingHistory(oldPage.detail.blocks, page.detail.blocks), has_more_blocks: boundary.hasMore, before_cursor: boundary.cursor } };
    }
  });
}
