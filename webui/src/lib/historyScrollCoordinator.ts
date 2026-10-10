import type { RefObject } from "react";
import { shouldAutoFollowMessageStream } from "./domain/conversationViewModel";

type Page = { cursor?: string | null; hasMore: boolean };
type Snapshot = { element: HTMLElement | null; id?: string; offset: number; height: number; top: number };
export type HistoryScrollOptions = {
  scopeKey: string;
  streamRef: RefObject<HTMLDivElement>;
  cursor?: string | null;
  hasMore: boolean;
  ready: boolean;
  busy?: boolean;
  searchActive?: boolean;
  initialPosition?: { top: number; follow: boolean };
  loadPage: (cursor: string, isCurrent: () => boolean) => Promise<Page>;
};

function snapshot(element: HTMLDivElement): Snapshot {
  const bounds = element.getBoundingClientRect();
  let anchor: HTMLElement | null = null;
  for (const item of element.querySelectorAll<HTMLElement>("[data-timeline-id]")) {
    const rect = item.getBoundingClientRect();
    if (rect.height <= 0 || rect.bottom <= bounds.top || rect.top >= bounds.bottom) continue;
    // Prefer a visible command within an expanded group. Its outer group can
    // acquire older commands on prepend without moving the group's top edge.
    if (!anchor || anchor.contains(item)) anchor = item;
    else break;
  }
  return { element: anchor, id: anchor?.dataset.timelineId, offset: anchor ? anchor.getBoundingClientRect().top - bounds.top : 0, height: element.scrollHeight, top: element.scrollTop };
}

/** Coordinates page requests and viewport changes independently of the UI lifecycle. */
export function createHistoryScrollCoordinator(onChange: () => void = () => {}) {
  let options: HistoryScrollOptions | undefined;
  let generation = 0;
  let initialized = false;
  const follow = { current: true };
  let upward = false;
  let lastTop = 0;
  let anchor: Snapshot | null = null;
  let active: Promise<boolean> | null = null;
  let failure: string | null = null;
  let loading = false;
  const completed = new Set<string>();

  const reset = () => {
    generation += 1;
    initialized = false;
    follow.current = true;
    upward = false;
    lastTop = 0;
    anchor = null;
    active = null;
    failure = null;
    loading = false;
    completed.clear();
  };
  const update = (next: HistoryScrollOptions) => {
    if (options && options.scopeKey !== next.scopeKey) reset();
    options = next;
  };
  const capture = () => {
    const element = options?.streamRef.current;
    if (element) { anchor = snapshot(element); lastTop = element.scrollTop; }
  };
  const loadOlder = (reason: "search" | "scroll" = "scroll"): Promise<boolean> => {
    if (active) return active;
    const current = options;
    if (!current || !current.ready || current.busy || !current.hasMore || !current.cursor || failure || completed.has(current.cursor)) return Promise.resolve(false);
    const requestGeneration = generation;
    const cursor = current.cursor;
    const isCurrent = () => requestGeneration === generation && current.scopeKey === options?.scopeKey;
    follow.current = false;
    if (reason === "search") upward = false;
    capture();
    loading = true;
    // Defer the callback until the single-flight promise is installed.
    active = Promise.resolve().then(() => {
      if (!isCurrent()) throw new Error("读取目标已变化");
      return current.loadPage(cursor, isCurrent);
    }).then(page => {
      if (!isCurrent()) return false;
      if (page.hasMore && (!page.cursor || page.cursor === cursor)) throw new Error("历史游标未推进");
      completed.add(cursor);
      return true;
    }).catch(() => {
      if (isCurrent()) { failure = "加载失败"; upward = false; }
      return false;
    }).finally(() => {
      if (isCurrent()) { active = null; loading = false; onChange(); }
    });
    onChange();
    return active;
  };
  const maybeLoad = () => {
    if (initialized && upward && !options?.searchActive && options?.streamRef.current && options.streamRef.current.scrollTop <= 200) void loadOlder("scroll");
  };
  const onRendered = () => {
    if (!options) return;
    const { streamRef, ready, initialPosition } = options;
    const element = streamRef.current;
    if (!element || !ready || !element.getClientRects().length) return;
    if (!initialized) {
      follow.current = initialPosition?.follow ?? true;
      element.scrollTop = initialPosition?.top ?? element.scrollHeight;
      initialized = true;
    } else if (follow.current) {
      element.scrollTop = element.scrollHeight;
    } else if (anchor) {
      const saved = anchor;
      const target = saved.element?.isConnected && element.contains(saved.element) ? saved.element
        : saved.id ? Array.from(element.querySelectorAll<HTMLElement>("[data-timeline-id], [data-timeline-aliases]"))
          .find(item => item.dataset.timelineId === saved.id || item.dataset.timelineAliases?.split(/\s+/).includes(saved.id!)) : null;
      if (target) {
        element.scrollTop += target.getBoundingClientRect().top - element.getBoundingClientRect().top - saved.offset;
      } else if (element.scrollHeight !== saved.height) {
        element.scrollTop = saved.top + element.scrollHeight - saved.height;
      }
    }
    capture();
    maybeLoad();
  };
  const onScroll = (element: HTMLDivElement) => {
    if (!initialized) return;
    if (element.scrollTop < lastTop - 1) upward = true;
    else if (element.scrollTop > lastTop + 1) upward = false;
    follow.current = !upward && shouldAutoFollowMessageStream(element);
    capture();
    maybeLoad();
  };
  const upwardIntent = () => {
    if (initialized) { upward = true; follow.current = false; capture(); maybeLoad(); }
  };
  const retry = () => { if (options?.busy) return; failure = null; void loadOlder("scroll"); };
  return {
    update, capture, onRendered, onScroll, upwardIntent, loadOlder, retry, dispose: reset, follow,
    get loading() { return loading; },
    get error() { return failure; }
  };
}
