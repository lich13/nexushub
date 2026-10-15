import type { HistoryMetadata, MessageBlock } from "../../types";

export function historyCursorIndex(cursor?: string | null): number {
  const index = Number(cursor?.split(":", 2)[1]);
  return Number.isFinite(index) ? index : Number.POSITIVE_INFINITY;
}

// History metadata and stable cursors were introduced in 1.2.17. This is a
// capability introduction, not a minimum supported protocol-2 service version.
// New readers accept a stable anchor alongside the legacy positional hint.
// Only responses advertising history metadata opt into this capability.
export function stableHistoryCursor(page: {
  history?: HistoryMetadata | null;
  blocks: MessageBlock[];
  before_cursor?: string | null;
}): string | null | undefined {
  if (page.before_cursor?.startsWith("p:")) return page.before_cursor;
  if (!page.history || !page.before_cursor || !page.blocks[0]) return page.before_cursor;
  return `b:${historyCursorIndex(page.before_cursor)}:${page.blocks[0].id}`;
}
