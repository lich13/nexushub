import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type RefObject } from "react";
import type { ClaudeSessionDetail, GrokSessionDetail, HistoryMetadata, ProviderSessionDetail, ThreadDetail } from "../types";
import { locateTimelineTarget } from "../components/common/SessionSearch";
import { historicalChanges } from "./query/usePagedHistory";
import { stableHistoryCursor } from "./domain/historyCursor";

export type TimelinePage<T> = { items: T[]; before?: string | null; after?: string | null; hasMore: boolean; history?: HistoryMetadata | null };
export const codexTimelinePage = (page: ThreadDetail): TimelinePage<ThreadDetail["blocks"][number]> => ({ items: page.blocks, before: stableHistoryCursor(page), after: page.after_cursor, hasMore: Boolean(page.has_more_blocks), history: page.history });
export const claudeTimelinePage = (page: ClaudeSessionDetail): TimelinePage<ClaudeSessionDetail["events"][number]> => ({ items: page.events, before: page.beforeCursor, after: page.afterCursor, hasMore: page.hasMore, history: page.history });
export const grokTimelinePage = (page: GrokSessionDetail): TimelinePage<GrokSessionDetail["events"][number]> => ({ items: page.events, before: page.beforeCursor, after: page.afterCursor, hasMore: Boolean(page.hasMore), history: page.history });
export const providerTimelinePage = (page: ProviderSessionDetail): TimelinePage<ProviderSessionDetail["events"][number]> => ({ items: page.events, before: page.beforeCursor, after: page.afterCursor, hasMore: page.hasMore, history: page.history });

type Window<T> = { scope: string; generation: number; pages: TimelinePage<T>[]; history?: HistoryMetadata | null };
type Options<T> = {
  scope: string; streamRef: RefObject<HTMLDivElement>; history?: HistoryMetadata | null;
  latestId?: string; id: (item: T) => string; read: (cursor: string) => Promise<TimelinePage<T>>;
  onNavigate: () => void; onLocated: (follow?: boolean) => void;
};

/** Jump without downloading or mounting the gap between the latest and selected turn. */
export function useTimelineWindow<T>(options: Options<T>) {
  const current = useRef(options); current.current = options;
  const sequence = useRef(0);
  const mounted = useRef(true);
  const [window, setWindow] = useState<Window<T> | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [target, setTarget] = useState<{ scope: string; id: string } | null>(null);
  const request = useRef(false);
  const pageFlight = useRef(false);
  const active = window?.scope === options.scope && (!window.history || !options.history || window.history.epoch === options.history.epoch) ? window : null;
  const activeRef = useRef(active); activeRef.current = active;
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; sequence.current++; }; }, []);
  useLayoutEffect(() => {
    sequence.current++; request.current = false; setBusy(false); setError(null); setTarget(null); setWindow(null);
  }, [options.scope, options.history?.epoch]);

  const run = useCallback(async (work: (valid: () => boolean) => Promise<void>, replace = false) => {
    if (request.current && !replace) return;
    const token = ++sequence.current, scope = current.current.scope, epoch = current.current.history?.epoch;
    const valid = () => mounted.current && token === sequence.current && scope === current.current.scope && epoch === current.current.history?.epoch;
    request.current = true; setBusy(true); setError(null);
    try { await work(valid); }
    catch (reason) { if (valid()) setError(reason instanceof Error ? reason.message : "历史读取失败，请重试"); }
    finally { if (valid()) { request.current = false; setBusy(false); } }
  }, []);

  const select = useCallback((id: string) => {
    current.current.onNavigate();
    if (current.current.streamRef.current && locateTimelineTarget(id, current.current.streamRef.current, "auto")) {
      if (!pageFlight.current) sequence.current++;
      request.current = false; setBusy(false); setError(null); setTarget(null);
      current.current.onLocated(); return;
    }
    void run(async valid => {
      const page = await current.current.read(`j:${id}`);
      if (!valid()) return;
      if (!page.items.some(item => current.current.id(item) === id)) throw new Error("历史位置已变化，请刷新会话");
      if (page.history && current.current.history && page.history.epoch !== current.current.history.epoch) throw new Error("会话来源已变化，请刷新后重试");
      setWindow({ scope: current.current.scope, generation: sequence.current, pages: [page], history: page.history });
      setTarget({ scope: current.current.scope, id });
    }, true);
  }, [run]);

  useLayoutEffect(() => {
    if (!target || target.scope !== options.scope) return;
    const frame = requestAnimationFrame(() => {
      const root = current.current.streamRef.current;
      if (root && locateTimelineTarget(target.id, root, "auto")) { current.current.onLocated(); setTarget(null); }
    });
    return () => cancelAnimationFrame(frame);
  }, [target, active, options.scope]);

  const load = useCallback(async (cursor: string, newer: boolean, isCurrent: () => boolean = () => true) => {
    const previous = activeRef.current;
    if (!previous) throw new Error("历史窗口已关闭");
    const token = sequence.current;
    pageFlight.current = true;
    let page: TimelinePage<T>;
    try { page = await current.current.read(cursor); } finally { pageFlight.current = false; }
    if (!mounted.current || token !== sequence.current || !isCurrent() || activeRef.current !== previous) return { cursor: page.before, hasMore: page.hasMore };
    if (previous.history && page.history && previous.history.epoch !== page.history.epoch) throw new Error("会话来源已变化，请重新定位");
    setWindow({ ...previous, pages: newer ? [...previous.pages, page] : [page, ...previous.pages] });
    return { cursor: page.before, hasMore: page.hasMore };
  }, []);

  const newer = useCallback(() => {
    const pages = activeRef.current?.pages;
    const last = pages?.[pages.length - 1];
    const lastId = last?.items.length ? current.current.id(last.items[last.items.length - 1]) : undefined;
    const cursor = last?.after ?? (lastId && current.current.latestId && lastId !== current.current.latestId ? `n:${lastId}` : undefined);
    if (cursor) void run(valid => load(cursor, true, valid).then(() => {}));
  }, [run, load]);
  const latest = useCallback(() => {
    sequence.current++; request.current = false; setBusy(false); setError(null); setTarget(null); setWindow(null);
    const token = sequence.current, scope = current.current.scope;
    requestAnimationFrame(() => {
      if (!mounted.current || token !== sequence.current || scope !== current.current.scope) return;
      const root = current.current.streamRef.current;
      if (root) root.scrollTop = root.scrollHeight;
      current.current.onLocated(true);
    });
  }, []);

  // Reuse the latest poll's change journal. Static windows make no extra requests.
  useEffect(() => {
    const previous = activeRef.current, incoming = options.history;
    if (!previous || !incoming || incoming.revision === previous.history?.revision || busy || request.current || pageFlight.current || error) return;
    const previousRevision = incoming.changes?.findIndex(change => change.revision === previous.history?.revision) ?? -1;
    if (incoming.reset || (previousRevision >= 0 && incoming.changes?.slice(previousRevision + 1).some(change => change.reset))) { latest(); setError("历史已更新，请重新定位"); return; }
    const changed = historicalChanges(previous.history, incoming);
    void run(async valid => {
      const pages: TimelinePage<T>[] = [];
      for (const page of previous.pages) {
        if (!valid() || activeRef.current !== previous) return;
        const affected = !changed || page.items.some(item => changed.has(current.current.id(item)));
        pages.push(affected && page.items.length ? await current.current.read(`w:${current.current.id(page.items[0])}`) : page);
      }
      if (valid() && activeRef.current === previous) setWindow({ ...previous, pages, history: incoming });
    });
  }, [options.history, active, busy, error, run, latest]);

  const items = useMemo(() => active ? [...new Map(active.pages.flatMap(page => page.items).map(item => [current.current.id(item), item])).values()] : undefined, [active]);
  const lastPage = active?.pages[active.pages.length - 1];
  const lastId = lastPage?.items.length ? options.id(lastPage.items[lastPage.items.length - 1]) : undefined;
  return { items, active: Boolean(active), generation: active?.generation ?? 0, before: active?.pages[0]?.before,
    hasMore: active?.pages[0]?.hasMore ?? false, hasNewer: Boolean(lastPage?.after || (lastId && options.latestId && lastId !== options.latestId)), busy, error,
    select, latest, newer, loadOlder: (cursor: string, isCurrent: () => boolean) => load(cursor, false, isCurrent) };
}
