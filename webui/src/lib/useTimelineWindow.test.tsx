import type { RefObject } from "react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { locateTimelineTarget } from "../components/common/SessionSearch";
import type { HistoryMetadata } from "../types";
import { useTimelineWindow, type TimelinePage } from "./useTimelineWindow";

const hooks = vi.hoisted(() => ({
  cursor: 0,
  cells: [] as any[],
  pending: [] as any[],
  dirty: false,
  frames: new Map<number, FrameRequestCallback>(),
  nextFrame: 1
}));

vi.mock("react", async original => {
  const actual = await original<typeof import("react")>();
  const changed = (previous: readonly unknown[] | undefined, next: readonly unknown[] | undefined) =>
    !previous || !next || previous.length !== next.length || next.some((item, index) => !Object.is(item, previous[index]));
  const memo = <T,>(factory: () => T, deps?: readonly unknown[]): T => {
    const index = hooks.cursor++;
    const cell = hooks.cells[index];
    if (!cell || changed(cell.deps, deps)) hooks.cells[index] = { kind: "memo", value: factory(), deps };
    return hooks.cells[index].value as T;
  };
  const effect = (callback: () => void | (() => void), deps: readonly unknown[] | undefined, layout: boolean) => {
    const index = hooks.cursor++;
    const cell = hooks.cells[index];
    if (!cell || changed(cell.deps, deps)) {
      hooks.cells[index] = { kind: "effect", deps, cleanup: cell?.cleanup };
      hooks.pending.push({ index, callback, layout });
    }
  };
  return {
    ...actual,
    useState: (initial: unknown) => {
      const index = hooks.cursor++;
      if (!hooks.cells[index]) hooks.cells[index] = { kind: "state", value: typeof initial === "function" ? (initial as () => unknown)() : initial };
      return [hooks.cells[index].value, (next: unknown) => {
        const current = hooks.cells[index].value;
        const value = typeof next === "function" ? (next as (value: unknown) => unknown)(current) : next;
        if (!Object.is(current, value)) { hooks.cells[index].value = value; hooks.dirty = true; }
      }];
    },
    useRef: (initial: unknown) => {
      const index = hooks.cursor++;
      if (!hooks.cells[index]) hooks.cells[index] = { kind: "ref", value: { current: initial } };
      return hooks.cells[index].value;
    },
    useMemo: memo,
    useCallback: (callback: unknown, deps?: readonly unknown[]) => memo(() => callback, deps),
    useEffect: (callback: () => void | (() => void), deps?: readonly unknown[]) => effect(callback, deps, false),
    useLayoutEffect: (callback: () => void | (() => void), deps?: readonly unknown[]) => effect(callback, deps, true)
  };
});

vi.mock("../components/common/SessionSearch", () => ({ locateTimelineTarget: vi.fn(() => false) }));

type Item = { id: string; value?: string };
type HistoryOptions = {
  scope: string;
  streamRef: RefObject<HTMLDivElement>;
  history?: HistoryMetadata | null;
  latestId?: string;
  id: (item: Item) => string;
  read: (cursor: string) => Promise<TimelinePage<Item>>;
  onNavigate: ReturnType<typeof vi.fn>;
  onLocated: ReturnType<typeof vi.fn>;
};

function version(revision: string, changes: HistoryMetadata["changes"] = [{ revision, changedIds: [] }], epoch = "source-fixture"): HistoryMetadata {
  return { epoch, revision, changes };
}

function page(items: Item[], overrides: Partial<TimelinePage<Item>> = {}): TimelinePage<Item> {
  return { items, before: null, after: null, hasMore: false, ...overrides };
}

function baseOptions(read: HistoryOptions["read"], overrides: Partial<HistoryOptions> = {}): HistoryOptions {
  const root = { scrollTop: 0, scrollHeight: 800, querySelectorAll: () => [] } as unknown as HTMLDivElement;
  return {
    scope: "machine-fixture:thread-fixture:codex",
    streamRef: { current: root },
    history: version("revision-1"),
    id: item => item.id,
    read,
    onNavigate: vi.fn(),
    onLocated: vi.fn(),
    ...overrides
  };
}

function mount(options: HistoryOptions) {
  let currentOptions = options;
  let result!: ReturnType<typeof useTimelineWindow<Item>>;
  const commit = () => {
    const pending = [...hooks.pending];
    hooks.pending.length = 0;
    pending.sort((left, right) => Number(right.layout) - Number(left.layout));
    for (const item of pending) {
      const cell = hooks.cells[item.index];
      cell.cleanup?.();
      cell.cleanup = item.callback();
    }
  };
  const render = (nextOptions?: HistoryOptions) => {
    if (nextOptions) currentOptions = nextOptions;
    let attempts = 20;
    do {
      hooks.cursor = 0;
      hooks.dirty = false;
      result = useTimelineWindow<Item>(currentOptions);
      commit();
      if (--attempts === 0 && hooks.dirty) throw new Error("Timeline hook fixture did not settle");
    } while (hooks.dirty);
    return result;
  };
  const flushFrames = () => {
    let attempts = 20;
    while (hooks.frames.size && attempts--) {
      const batch = [...hooks.frames.entries()];
      hooks.frames.clear();
      for (const [, callback] of batch) callback(0);
    }
    if (hooks.frames.size) throw new Error("Timeline animation frames did not settle");
  };
  render();
  return { get result() { return result; }, render, flushFrames };
}

function resetHooks() {
  hooks.cursor = 0;
  hooks.cells.length = 0;
  hooks.pending.length = 0;
  hooks.dirty = false;
  hooks.frames.clear();
  hooks.nextFrame = 1;
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    const id = hooks.nextFrame++;
    hooks.frames.set(id, callback);
    return id;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => { hooks.frames.delete(id); });
}

async function settle(view: ReturnType<typeof mount>) {
  for (let index = 0; index < 6; index++) await Promise.resolve();
  view.render();
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(resolvePromise => { resolve = resolvePromise; });
  return { promise, resolve };
}

beforeEach(resetHooks);
afterEach(() => vi.unstubAllGlobals());

describe("useTimelineWindow", () => {
  test("reads one bounded window at the selected j:ID and does not fetch the gap", async () => {
    const read = vi.fn(async (cursor: string) => page([{ id: "target" }, { id: "nearby" }], {
      before: "older-cursor", after: "newer-cursor", hasMore: true, history: version("revision-1")
    }));
    const options = baseOptions(read);
    const view = mount(options);

    view.result.select("target");
    await settle(view);

    expect(read.mock.calls.map(([cursor]) => cursor)).toEqual(["j:target"]);
    expect(view.result.items?.map(item => item.id)).toEqual(["target", "nearby"]);
    expect(view.result.active).toBe(true);
    expect(options.onNavigate).toHaveBeenCalledOnce();
  });

  test.each([
    ["machine", { scope: "machine-next:thread-fixture:codex" }],
    ["thread", { scope: "machine-fixture:thread-next:codex" }],
    ["source epoch", { history: version("revision-next", undefined, "source-next") }]
  ])("drops a late selection response after a %s switch", async (_label, change) => {
    const pending = deferred<TimelinePage<Item>>();
    const read = vi.fn(() => pending.promise);
    const view = mount(baseOptions(read));

    view.result.select("stale-target");
    view.render();
    view.render({ ...baseOptions(read), ...change });
    pending.resolve(page([{ id: "stale-target" }], { history: version("revision-1") }));
    await settle(view);

    expect(view.result.active).toBe(false);
    expect(view.result.items).toBeUndefined();
    expect(view.result.error).toBeNull();
  });

  test("a repeated selection replaces the older request already in flight", async () => {
    const first = deferred<TimelinePage<Item>>();
    const second = deferred<TimelinePage<Item>>();
    const read = vi.fn((cursor: string) => cursor === "j:first" ? first.promise : second.promise);
    const view = mount(baseOptions(read));

    view.result.select("first");
    view.render();
    view.result.select("second");
    view.render();
    second.resolve(page([{ id: "second" }], { history: version("revision-1") }));
    await settle(view);
    expect(view.result.items?.map(item => item.id)).toEqual(["second"]);

    first.resolve(page([{ id: "first" }], { history: version("revision-1") }));
    await settle(view);
    expect(view.result.items?.map(item => item.id)).toEqual(["second"]);
    expect(read.mock.calls.map(([cursor]) => cursor)).toEqual(["j:first", "j:second"]);
  });

  test("keeps both pagination directions, deduplicates IDs, and returns to the latest stream", async () => {
    const read = vi.fn(async (cursor: string) => {
      if (cursor === "j:b") return page([{ id: "b", value: "selected-b" }, { id: "c", value: "selected-c" }], {
        before: "older-cursor", hasMore: true, history: version("revision-1")
      });
      if (cursor === "older-cursor") return page([{ id: "a" }, { id: "b", value: "older-b" }], { before: "oldest-cursor", hasMore: true, history: version("revision-1") });
      if (cursor === "n:c") return page([{ id: "c", value: "newer-c" }, { id: "d" }], { history: version("revision-1") });
      throw new Error(`Unexpected fixture cursor: ${cursor}`);
    });
    const options = baseOptions(read, { latestId: "d" });
    const view = mount(options);
    view.result.select("b");
    await settle(view);
    if (!view.result.active) throw new Error(`items=${JSON.stringify(view.result.items)} error=${view.result.error} reads=${JSON.stringify(read.mock.calls)}`);
    expect(view.result.items?.map(item => item.id)).toEqual(["b", "c"]);
    expect(view.result.hasNewer).toBe(true);
    await view.result.loadOlder("older-cursor", () => true);
    view.render();
    view.result.newer();
    await settle(view);

    expect(view.result.items?.map(item => item.id)).toEqual(["a", "b", "c", "d"]);
    expect(view.result.items?.find(item => item.id === "b")?.value).toBe("selected-b");
    expect(view.result.items?.find(item => item.id === "c")?.value).toBe("newer-c");
    expect(read.mock.calls.map(([cursor]) => cursor)).toEqual(["j:b", "older-cursor", "n:c"]);
    expect(view.result.hasNewer).toBe(false);

    view.result.latest();
    view.render();
    view.flushFrames();
    expect(view.result.active).toBe(false);
    expect(view.result.items).toBeUndefined();
    expect(options.streamRef.current?.scrollTop).toBe(800);
    expect(options.onLocated).toHaveBeenCalledWith(true);
  });

  test("does not issue page reads when a static history revision has no changed IDs", async () => {
    const read = vi.fn(async () => page([{ id: "target" }, { id: "other" }], { history: version("revision-1") }));
    const view = mount(baseOptions(read));
    view.result.select("target");
    await settle(view);

    const history = version("revision-2", [
      { revision: "revision-1", changedIds: [] },
      { revision: "revision-2", changedIds: [] }
    ]);
    view.render(baseOptions(read, { history }));
    await settle(view);

    expect(read).toHaveBeenCalledTimes(1);
    expect(view.result.items?.map(item => item.id)).toEqual(["target", "other"]);
  });

  test("refreshes only the affected older page by its first ID", async () => {
    const read = vi.fn(async (cursor: string) => {
      if (cursor === "j:target") return page([{ id: "target" }], { before: "older-cursor", hasMore: true, history: version("revision-1") });
      if (cursor === "older-cursor") return page([{ id: "older-first" }, { id: "older-changed" }], { history: version("revision-1") });
      if (cursor === "w:older-first") return page([{ id: "older-first" }, { id: "older-changed", value: "refreshed" }], { history: version("revision-2") });
      throw new Error(`Unexpected fixture cursor: ${cursor}`);
    });
    const view = mount(baseOptions(read));
    view.result.select("target");
    await settle(view);
    await view.result.loadOlder("older-cursor", () => true);
    view.render();
    read.mockClear();

    const history = version("revision-2", [
      { revision: "revision-1", changedIds: [] },
      { revision: "revision-2", changedIds: ["older-changed"] }
    ]);
    view.render(baseOptions(read, { history }));
    await settle(view);

    expect(read.mock.calls.map(([cursor]) => cursor)).toEqual(["w:older-first"]);
    expect(view.result.items?.find(item => item.id === "older-changed")?.value).toBe("refreshed");
  });

  test.each([
    ["empty index", page([])],
    ["legacy fallback without the requested identity", page([{ id: "unrelated" }])]
  ])("does not fabricate a located item for an %s", async (_label, response) => {
    const read = vi.fn(async () => response);
    const options = baseOptions(read);
    const view = mount(options);
    view.result.select("missing");
    await settle(view);

    expect(view.result.active).toBe(false);
    expect(view.result.items).toBeUndefined();
    expect(view.result.error).toMatch(/历史位置已变化/);
    expect(options.onLocated).not.toHaveBeenCalled();
  });
});
