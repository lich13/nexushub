import { describe, expect, test, vi } from "vitest";
import { createHistoryScrollCoordinator, type HistoryScrollOptions } from "./historyScrollCoordinator";

type FakeItem = {
  dataset: { timelineId?: string; timelineAliases?: string };
  isConnected: boolean;
  parentElement: FakeItem | FakeStream | null;
  top: number;
  height: number;
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

function item(id: string, top: number, height: number, aliases?: string): FakeItem {
  const next: FakeItem = {
    dataset: { timelineId: id, timelineAliases: aliases },
    isConnected: true,
    parentElement: null,
    top,
    height,
    children: [],
    contains: target => target === next || next.children.some(child => child.contains(target)),
    getBoundingClientRect: () => {
      let parent = next.parentElement;
      let scrollTop = 0;
      while (parent) {
        if ("items" in parent) { scrollTop = parent.scrollTop; break; }
        parent = parent.parentElement;
      }
      const topInViewport = next.top - scrollTop;
      return { top: topInViewport, bottom: topInViewport + next.height, height: next.height };
    }
  };
  return next;
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
  for (const child of items) connect(next, child);
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

  test("restores a nested command anchor when a group ID changes during prepend", async () => {
    const oldGroup = item("group:old", 180, 420);
    const oldCommand = item("group:cmd-1", 230, 80);
    connect(oldGroup, oldCommand);
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
    const newCommand = item("command:cmd-1", 530, 80, "group:cmd-1");
    connect(newGroup, newCommand);
    next.items = [newGroup, newCommand];
    connect(next, newGroup);
    next.scrollHeight = 1300;
    controller.onRendered();

    // The nested command moved by 300px; anchoring the outer group would move
    // by -30px instead and visibly jump the reader.
    expect(next.scrollTop).toBe(480);
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
