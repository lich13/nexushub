import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient, type InfiniteData, type QueryKey } from "@tanstack/react-query";
import type { HistoryMetadata } from "../../types";

type HistoryData<Page> = InfiniteData<Page>;
export type HistoryAdapter<Page> = {
  ids: (page: Page) => string[];
  history: (page: Page) => HistoryMetadata | null | undefined;
  total: (page: Page) => number | undefined;
  cursor: (page: Page) => string | undefined;
  cursorAfter: (page: Page, index: number) => string | undefined;
  invalidate?: (page: Page) => Page;
  merge: (previous: unknown, incoming: unknown) => HistoryData<Page>;
};

export function historicalChanges(previous: HistoryMetadata | null | undefined, incoming: HistoryMetadata | null | undefined): Set<string> | null {
  if (!previous || !incoming || previous.epoch !== incoming.epoch || incoming.reset) return null;
  if (previous.revision === incoming.revision) return new Set();
  const index = incoming.changes?.findIndex(change => change.revision === previous.revision) ?? -1;
  if (index < 0) return null;
  const changes = incoming.changes!.slice(index + 1);
  if (changes.some(change => change.reset)) return null;
  return new Set(changes.flatMap(change => change.changedIds));
}

function checkSignal(signal: AbortSignal) {
  if (signal.aborted) throw new DOMException("请求已取消", "AbortError");
}

// One poll fetches the latest window. Only changed, already visible rows need
// historical reads; a missed revision falls back to a safe bounded-page refresh.
export async function refreshHistory<Page>(previous: HistoryData<Page> | undefined, head: Page, read: (before?: string) => Promise<Page>, adapter: HistoryAdapter<Page>, signal: AbortSignal): Promise<HistoryData<Page>> {
  const single = (page: Page): HistoryData<Page> => ({ pages: [page], pageParams: [undefined] });
  const oldHead = previous?.pages[0];
  if (!previous || !oldHead) return single(head);
  const oldVersion = adapter.history(oldHead), version = adapter.history(head);
  const oldTotal = adapter.total(oldHead), total = adapter.total(head);
  const priorRevision = version?.changes?.findIndex(change => change.revision === oldVersion?.revision) ?? -1;
  const rewritten = Boolean(version?.reset || (priorRevision >= 0 && version?.changes?.slice(priorRevision + 1).some(change => change.reset)));
  if (rewritten || (oldVersion && version && oldVersion.epoch !== version.epoch) || (oldTotal !== undefined && total !== undefined && total < oldTotal)) return single(adapter.invalidate?.(head) ?? head);

  const changed = historicalChanges(oldVersion, version);
  const known = new Set(previous.pages.flatMap(adapter.ids));
  let combined = head;
  let bridge = head;
  const visited = new Set<string>();
  // Fast appends may move the head by more than one window between polls.
  while (known.size && adapter.ids(bridge).length && !adapter.ids(bridge).some(id => known.has(id))) {
    const cursor = adapter.cursor(bridge);
    if (!cursor || visited.has(cursor)) break;
    visited.add(cursor);
    bridge = await read(cursor); checkSignal(signal);
    const bridgeVersion = adapter.history(bridge);
    if (version && bridgeVersion && version.epoch !== bridgeVersion.epoch) throw new Error("历史来源已变化，请重试");
    combined = adapter.merge(single(bridge), single(combined)).pages[0];
  }
  let next = adapter.merge(previous, { pages: [combined, ...previous.pages.slice(1)], pageParams: previous.pageParams });
  const headIds = new Set(adapter.ids(combined));
  const refreshed = new Set(headIds);
  for (let index = 0; index < previous.pages.length; index++) {
    const page = previous.pages[index];
    const ids = adapter.ids(page);
    const pending = ids.filter(id => !refreshed.has(id) && (!changed || changed.has(id)));
    while (pending.length) {
      const id = pending[pending.length - 1];
      const position = ids.indexOf(id);
      const saved = previous.pageParams[index];
      const cursor = adapter.cursorAfter(page, position) ?? (typeof saved === "string" ? saved : undefined);
      const update = await read(cursor); checkSignal(signal);
      const updateVersion = adapter.history(update);
      if (version && updateVersion && updateVersion.epoch !== version.epoch) throw new Error("历史来源已变化，请重试");
      const updateIds = adapter.ids(update);
      // Native removals invalidate the old window; don't keep a stale row or
      // loop forever when an older service cannot resolve the retained cursor.
      if (!updateIds.includes(id)) return single(adapter.invalidate?.(head) ?? head);
      updateIds.forEach(item => refreshed.add(item));
      next = { ...next, pages: next.pages.map((oldPage, pageIndex) => pageIndex === index ? adapter.merge(single(oldPage), single(update)).pages[0] : oldPage) };
      while (pending.length && refreshed.has(pending[pending.length - 1])) pending.pop();
    }
  }
  // Historical pages carry their own older summary; the newest snapshot stays
  // authoritative even when an old tool result was refreshed in the head cache.
  next = adapter.merge(next, { ...next, pages: [combined, ...next.pages.slice(1)] });
  return next;
}

type Options<Page> = HistoryAdapter<Page> & {
  queryKey: QueryKey;
  enabled: boolean;
  read: (before: string | undefined, signal: AbortSignal) => Promise<Page>;
  interval: (head?: Page) => number;
};

export function usePagedHistory<Page>(options: Options<Page>) {
  const client = useQueryClient();
  const current = useRef(options); current.current = options;
  const scope = JSON.stringify(options.queryKey);
  const flight = useRef<{ scope: string; controller: AbortController; promise: Promise<{ data?: HistoryData<Page>; isError: boolean; error: Error | null }> }>();
  const [paging, setPaging] = useState<{ scope: string; busy: boolean; error: Error | null }>({ scope, busy: false, error: null });
  const busy = paging.scope === scope && paging.busy;
  const query = useQuery<HistoryData<Page>, Error>({
    queryKey: options.queryKey,
    enabled: options.enabled,
    retry: false,
    queryFn: async ({ signal }) => {
      const read = async (before?: string) => { const page = await options.read(before, signal); checkSignal(signal); return page; };
      const head = await read();
      return refreshHistory(client.getQueryData<HistoryData<Page>>(options.queryKey), head, read, options, signal);
    },
    refetchInterval: state => document.visibilityState !== "visible" || busy ? false : options.interval(state.state.data?.pages[0]),
    refetchIntervalInBackground: false
  });
  useEffect(() => () => { flight.current?.controller.abort(); }, [scope]);
  const fetchNextPage = useCallback(async (_options?: { cancelRefetch?: boolean }) => {
    if (flight.current?.scope === scope) return flight.current.promise;
    const opts = current.current;
    const controller = new AbortController();
    const task = async () => {
      setPaging({ scope, busy: true, error: null });
      await client.cancelQueries({ queryKey: opts.queryKey, exact: true });
      const prior = client.getQueryData<HistoryData<Page>>(opts.queryKey);
      const last = prior?.pages[prior.pages.length - 1];
      const cursor = last && opts.cursor(last);
      try {
        if (!cursor) return { data: prior, isError: false, error: null };
        const page = await opts.read(cursor, controller.signal); checkSignal(controller.signal);
        const data = client.setQueryData<HistoryData<Page>>(opts.queryKey, value => {
          if (!value) return value;
          const oldVersion = opts.history(value.pages[0]), version = opts.history(page);
          if (oldVersion && version && oldVersion.epoch !== version.epoch) return value;
          if (value.pageParams.includes(cursor)) return value;
          return { pages: [...value.pages, page], pageParams: [...value.pageParams, cursor] };
        });
        return { data, isError: false, error: null };
      } catch (error) {
        const failure = error instanceof Error ? error : new Error(String(error));
        if (!controller.signal.aborted) setPaging({ scope, busy: false, error: failure });
        return { data: client.getQueryData<HistoryData<Page>>(opts.queryKey), isError: true, error: failure };
      } finally {
        if (flight.current?.controller === controller) { flight.current = undefined; setPaging(value => value.scope === scope ? { ...value, busy: false } : value); }
      }
    };
    const promise = task();
    flight.current = { scope, controller, promise };
    return promise;
  }, [client, scope]);
  const tail = query.data?.pages[query.data.pages.length - 1];
  const pageError = paging.scope === scope ? paging.error : null;
  return { ...query, isFetching: query.isFetching || busy, isFetchingNextPage: busy, isFetchNextPageError: Boolean(pageError), error: pageError ?? query.error,
    hasNextPage: Boolean(tail && options.cursor(tail)), fetchNextPage };
}
