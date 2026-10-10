import { describe, expect, test, vi } from "vitest";
import { createHistoryScrollCoordinator, type HistoryScrollOptions } from "./historyScrollCoordinator";

type FakeItem = {
  dataset: { timelineId?: string; timelineAliases?: string; historyScrollContainer?: string };
  isConnected: boolean;
  parentElement: FakeItem | FakeStream | null;
  top: number;
  height: number;
  scrollTop: number;
  clientHeight: number;
  scrollHeight: number;
  children: FakeItem[];
  contains: (target: unknown) => boolean;
  getBoundingClientRect: () => { top: number; bottom: number; height: number };
};

type FakeStream = {
  scrollTop: number;
  scrollHeight: number;
  clientHeight: number;
  items: FakeItem[];
  getClientRects: () => Array<unknown>;
  getBoundingClientRect: () => { top: number; bottom: number; height: number };
  querySelectorAll: <T extends FakeItem>(selector: string) => T[];
  contains: (target: unknown) => boolean;
};

function item(id: string, top: number, height: number, aliases?: string, options: { scrollTop?: number; clientHeight?: number; scrollHeight?: number; historyScrollContainer?: string } = {}): FakeItem {
  const next: FakeItem = {
    dataset: { timelineId: id, timelineAliases: aliases },
    isConnected: true,
    parentElement: null,
    top,
    height,
    scrollTop: options.scrollTop ?? 0,
    clientHeight: options.clientHeight ?? height,
    scrollHeight: options.scrollHeight ?? height,
    children: [],
    contains: target => target === next || next.children.some(child => child.contains(target)),
    getBoundingClientRect: () => {
      let topInViewport = next.top;
      let parent = next.parentElement;
      while (parent) {
        topInViewport -= parent.scrollTop;
        if ("items" in parent) {
          topInViewport += parent.getBoundingClientRect().top;
          break;
        }
        topInViewport += parent.top;
        parent = parent.parentElement;
      }
      return { top: topInViewport, bottom: topInViewport + next.height, height: next.height };
    }
  };
  if (options.historyScrollContainer !== undefined) next.dataset.historyScrollContainer = options.historyScrollContainer;
  return next;
}

function scrollContainer(top: number, clientHeight: number, scrollHeight: number, scrollTop: number): FakeItem {
  return item("", top, clientHeight, undefined, { clientHeight, scrollHeight, scrollTop, historyScrollContainer: "commands" });
}

function stream(items: FakeItem[] = [], overrides: Partial<Pick<FakeStream, "scrollTop" | "scrollHeight" | "clientHeight">> = {}): FakeStream {
  let currentScrollTop = 0;
  let currentScrollHeight = 1000;
  const next: FakeStream = {
    get scrollTop() { return currentScrollTop; },
    set scrollTop(value: number) {
      currentScrollTop = Math.max(0, Math.min(value, Math.max(0, currentScrollHeight - next.clientHeight)));
    },
    get scrollHeight() { return currentScrollHeight; },
    set scrollHeight(value: number) {
      currentScrollHeight = value;
      next.scrollTop = currentScrollTop;
    },
    clientHeight: 400,
    items,
    getClientRects: () => [{}],
    getBoundingClientRect: () => ({ top: 0, bottom: 400, height: 400 }),
    querySelectorAll: <T extends FakeItem>(_selector: string) => next.items as T[],
    contains: target => next.items.some(item => item.contains(target))
  };
  if (overrides.clientHeight !== undefined) next.clientHeight = overrides.clientHeight;
  if (overrides.scrollHeight !== undefined) next.scrollHeight = overrides.scrollHeight;
  if (overrides.scrollTop !== undefined) next.scrollTop = overrides.scrollTop;
  for (const child of items) if (!child.parentElement) connect(next, child);
  return next;
}

function connect(parent: FakeItem | FakeStream, child: FakeItem): void {
  child.parentElement = parent;
  if ("children" in parent) parent.children.push(child);
}

function optionsFor(next: FakeStream, overrides: Partial<HistoryScrollOptions> = {}): HistoryScrollOptions {
  return {
    scopeKey: "machine-a:thread-a",
    streamRef: { current: next as unknown as HTMLDivElement },
    cursor: "cursor-a",
    hasMore: true,
    ready: true,
    searchActive: false,
    loadPage: async () => ({ cursor: "cursor-b", hasMore: false }),
    ...overrides
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => { resolve = resolvePromise; reject = rejectPromise; });
  return { promise, resolve, reject };
}

describe("useHistoryScroll contract", () => {
  test("initializes a ready stream at the bottom and never prefetches all history", () => {
    const next = stream([], { scrollTop: 0, scrollHeight: 1400 });
    const loadPage = vi.fn(async () => ({ cursor: "cursor-b", hasMore: true }));
    const controller = createHistoryScrollCoordinator();
    controller.update(optionsFor(next, { loadPage }));

    controller.onRendered();

    expect(next.scrollTop).toBe(1000);
    expect(loadPage).not.toHaveBeenCalled();
    expect(controller.follow.current).toBe(true);
  });

  test("shares one in-flight request between search and scroll triggers", async () => {
    const next = stream([], { scrollTop: 180 });
    const pending = deferred<{ cursor: string; hasMore: boolean }>();
    const loadPage = vi.fn(() => pending.promise);
    const controller = createHistoryScrollCoordinator();
    controller.update(optionsFor(next, { searchActive: true, loadPage }));
    controller.onRendered();
    controller.upwardIntent();

    const searchRequest = controller.loadOlder("search");
    const scrollRequest = controller.loadOlder("scroll");
    await Promise.resolve();

    expect(searchRequest).toBe(scrollRequest);
    expect(loadPage).toHaveBeenCalledTimes(1);
    pending.resolve({ cursor: "cursor-b", hasMore: true });
    await expect(searchRequest).resolves.toBe(true);
  });

  test("loads the next cursor at the 200px threshold while preserving an anchor during polling", async () => {
    const visible = item("message:anchor", 300, 80);
    const next = stream([visible], { scrollHeight: 1000 });
    const first = deferred<{ cursor: string; hasMore: boolean }>();
    const second = deferred<{ cursor: string; hasMore: boolean }>();
    const loadPage = vi.fn()
      .mockImplementationOnce(() => first.promise)
      .mockImplementationOnce(() => second.promise);
    const controller = createHistoryScrollCoordinator();
    const initial = optionsFor(next, { loadPage });
    controller.update(initial);
    controller.onRendered();

    next.scrollTop = 200;
    controller.onScroll(next);
    const firstRequest = controller.loadOlder("scroll");
    await Promise.resolve();
    expect(loadPage).toHaveBeenCalledTimes(1);

    // The reader continues upward while the first request is in flight.
    next.scrollTop = 180;
    controller.onScroll(next);
    next.scrollHeight = 1200;
    controller.onRendered();
    expect(next.scrollTop).toBe(180);
    expect(loadPage).toHaveBeenCalledTimes(1);

    first.resolve({ cursor: "cursor-b", hasMore: true });
    await expect(firstRequest).resolves.toBe(true);
    controller.update({ ...initial, cursor: "cursor-b", hasMore: true });
    controller.onRendered();
    const secondRequest = controller.loadOlder("scroll");
    await Promise.resolve();
    expect(loadPage).toHaveBeenCalledTimes(2);

    second.resolve({ cursor: "cursor-c", hasMore: false });
    await expect(secondRequest).resolves.toBe(true);
  });

  test("restores a nested command anchor when a group remounts during prepend", async () => {
    const oldGroup = item("group:old", 180, 420);
    const oldRows = scrollContainer(0, 180, 600, 220);
    const oldCommand = item("group:cmd-1", 300, 80);
    connect(oldGroup, oldRows);
    connect(oldRows, oldCommand);
    const next = stream([oldGroup, oldCommand], { scrollTop: 180, scrollHeight: 1000 });
    const pending = deferred<{ cursor: string; hasMore: boolean }>();
    const controller = createHistoryScrollCoordinator();
    controller.update(optionsFor(next, { searchActive: true, loadPage: () => pending.promise }));
    controller.onRendered();
    next.scrollTop = 180;
    controller.onScroll(next);

    const request = controller.loadOlder("search");
    await Promise.resolve();
    pending.resolve({ cursor: "cursor-b", hasMore: false });
    await expect(request).resolves.toBe(true);

    oldCommand.isConnected = false;
    oldGroup.isConnected = false;
    const newGroup = item("group:new", 150, 600);
    const newRows = scrollContainer(0, 180, 900, 0);
    const newCommand = item("command:cmd-1", 520, 80, "group:cmd-1");
    connect(newGroup, newRows);
    connect(newRows, newCommand);
    next.items = [newGroup, newCommand];
    connect(next, newGroup);
    next.scrollHeight = 1300;
    controller.onRendered();

    // The remounted group moved, and its command gained 220px of prepended
    // content. Restore the inner viewport first, then compensate the parent.
    expect(newRows.scrollTop).toBe(440);
    expect(next.scrollTop).toBe(150);
    expect(newCommand.getBoundingClientRect().top).toBe(80);
  });

  test("does not anchor a command clipped by its nested execution viewport", async () => {
    const oldGroup = item("group:stable", 150, 300);
    const oldRows = scrollContainer(0, 100, 400, 200);
    const oldCommand = item("group:cmd-clipped", 300, 80);
    connect(oldGroup, oldRows);
    connect(oldRows, oldCommand);
    const next = stream([oldGroup, oldCommand], { scrollTop: 100, scrollHeight: 1000 });
    const pending = deferred<{ cursor: string; hasMore: boolean }>();
    const controller = createHistoryScrollCoordinator();
    const initial = optionsFor(next, { searchActive: true, loadPage: () => pending.promise });
    controller.update(initial);
    controller.onRendered();
    next.scrollTop = 100;
    controller.onScroll(next);

    const request = controller.loadOlder("search");
    await Promise.resolve();
    pending.resolve({ cursor: "cursor-b", hasMore: false });
    await expect(request).resolves.toBe(true);

    oldCommand.isConnected = false;
    oldGroup.isConnected = false;
    const newGroup = item("group:stable", 180, 300);
    const newRows = scrollContainer(0, 100, 400, 0);
    const newCommand = item("group:cmd-clipped", 300, 80);
    connect(newGroup, newRows);
    connect(newRows, newCommand);
    next.items = [newGroup, newCommand];
    connect(next, newGroup);
    next.scrollHeight = 1200;
    controller.onRendered();

    // The command intersects the reading pane but lies outside the bounded
    // execution viewport. The visible group, not the clipped command, is the anchor.
    expect(next.scrollTop).toBe(130);
    expect(newGroup.getBoundingClientRect().top).toBe(50);
  });

  test("does not retry a failed page until explicitly requested", async () => {
    const next = stream([], { scrollTop: 180 });
    const loadPage = vi.fn()
      .mockRejectedValueOnce(new Error("fixture read failure"))
      .mockResolvedValueOnce({ cursor: "cursor-b", hasMore: false });
    const controller = createHistoryScrollCoordinator();
    controller.update(optionsFor(next, { searchActive: true, loadPage }));
    controller.onRendered();

    await expect(controller.loadOlder("search")).resolves.toBe(false);
    expect(controller.error).toBe("加载失败");
    controller.onRendered();
    await Promise.resolve();
    expect(loadPage).toHaveBeenCalledTimes(1);

    controller.retry();
    await Promise.resolve();
    await Promise.resolve();
    expect(loadPage).toHaveBeenCalledTimes(2);
    expect(controller.error).toBeNull();
  });

  test("stops when the server returns a stalled cursor", async () => {
    const next = stream([], { scrollTop: 180 });
    const loadPage = vi.fn(async () => ({ cursor: "cursor-a", hasMore: true }));
    const controller = createHistoryScrollCoordinator();
    controller.update(optionsFor(next, { searchActive: true, loadPage }));

    await expect(controller.loadOlder("search")).resolves.toBe(false);
    expect(loadPage).toHaveBeenCalledTimes(1);
    expect(controller.error).toBe("加载失败");
    controller.onRendered();
    await Promise.resolve();
    expect(loadPage).toHaveBeenCalledTimes(1);
  });

  test("drops a late response after machine or thread scope changes", async () => {
    const next = stream([], { scrollTop: 180 });
    const oldPage = deferred<{ cursor: string; hasMore: boolean }>();
    const oldLoad = vi.fn(() => oldPage.promise);
    const nextLoad = vi.fn(async () => ({ cursor: "cursor-c", hasMore: false }));
    const controller = createHistoryScrollCoordinator();
    const oldOptions = optionsFor(next, { loadPage: oldLoad });
    controller.update(oldOptions);
    controller.onRendered();
    const oldRequest = controller.loadOlder("search");
    await Promise.resolve();

    controller.update(optionsFor(next, {
      scopeKey: "machine-b:thread-b",
      cursor: "cursor-b",
      loadPage: nextLoad
    }));
    oldPage.resolve({ cursor: "cursor-old-next", hasMore: true });
    await expect(oldRequest).resolves.toBe(false);

    const nextRequest = controller.loadOlder("search");
    await expect(nextRequest).resolves.toBe(true);
    expect(oldLoad).toHaveBeenCalledTimes(1);
    expect(nextLoad).toHaveBeenCalledTimes(1);
    expect(controller.error).toBeNull();
  });

});
