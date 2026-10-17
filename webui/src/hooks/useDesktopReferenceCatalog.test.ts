import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { DesktopCatalog, DesktopCatalogItem } from "../lib/api/desktop";
import { desktopCatalog } from "../lib/query/desktop";
import { useDesktopReferenceCatalog } from "./useDesktopReferenceCatalog";

const lifecycle = vi.hoisted(() => ({
  index: 0,
  dirty: false,
  states: [] as unknown[],
  refs: [] as { current: unknown }[],
  effects: [] as any[],
  pendingEffects: [] as any[]
}));

vi.mock("react", async importOriginal => {
  const actual = await importOriginal<typeof import("react")>();
  return {
    ...actual,
    useState: (initial: unknown) => {
      const index = lifecycle.index++;
      if (!(index in lifecycle.states)) lifecycle.states[index] = typeof initial === "function" ? (initial as () => unknown)() : initial;
      return [lifecycle.states[index], (next: unknown) => {
        const previous = lifecycle.states[index];
        const value = typeof next === "function" ? (next as (current: unknown) => unknown)(previous) : next;
        if (!Object.is(previous, value)) {
          lifecycle.states[index] = value;
          lifecycle.dirty = true;
        }
      }];
    },
    useRef: (initial: unknown) => {
      const index = lifecycle.index++;
      if (!lifecycle.refs[index]) lifecycle.refs[index] = { current: initial };
      return lifecycle.refs[index];
    },
    useEffect: (create: () => void | (() => void), deps?: readonly unknown[]) => {
      const index = lifecycle.index++;
      const previous = lifecycle.effects[index];
      const changed = !previous || !deps || !previous.deps || deps.length !== previous.deps.length
        || deps.some((value, item) => !Object.is(value, previous.deps[item]));
      if (changed) lifecycle.pendingEffects.push({ index, create, deps });
    }
  };
});

vi.mock("../lib/query/desktop", () => ({ desktopCatalog: vi.fn() }));

type Deferred<T> = { promise: Promise<T>; resolve: (value: T) => void; reject: (reason?: unknown) => void };

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((accept, fail) => { resolve = accept; reject = fail; });
  return { promise, resolve, reject };
}

function item(kind: string, name: string, handle = `${kind}-${name}-fixture`): DesktopCatalogItem {
  return { handle, kind, name, description: "", reasoningEfforts: [], defaultReasoningEffort: null };
}

function catalog(items: DesktopCatalogItem[], nextCursor: string | null = null): DesktopCatalog {
  return { items, nextCursor };
}

function render(sessionId: string | undefined = "session-fixture", kind = "skills", query = ""): ReturnType<typeof useDesktopReferenceCatalog> {
  let result!: ReturnType<typeof useDesktopReferenceCatalog>;
  for (let pass = 0; pass < 8; pass += 1) {
    lifecycle.dirty = false;
    lifecycle.index = 0;
    result = useDesktopReferenceCatalog(sessionId, kind, query);
    const pending = lifecycle.pendingEffects.splice(0);
    for (const effect of pending) {
      lifecycle.effects[effect.index]?.cleanup?.();
      const cleanup = effect.create();
      lifecycle.effects[effect.index] = {
        deps: effect.deps ? [...effect.deps] : undefined,
        cleanup: typeof cleanup === "function" ? cleanup : undefined
      };
    }
    if (!lifecycle.dirty) return result;
  }
  throw new Error("desktop reference catalog fixture did not settle");
}

function unmount() {
  for (const effect of [...lifecycle.effects].reverse()) effect?.cleanup?.();
  lifecycle.effects = [];
  lifecycle.pendingEffects = [];
}

async function flushMicrotasks() {
  for (let index = 0; index < 12; index += 1) await Promise.resolve();
}

beforeEach(() => {
  lifecycle.index = 0;
  lifecycle.dirty = false;
  lifecycle.states = [];
  lifecycle.refs = [];
  lifecycle.effects = [];
  lifecycle.pendingEffects = [];
  vi.useFakeTimers();
  vi.mocked(desktopCatalog).mockReset();
});

afterEach(async () => {
  unmount();
  await flushMicrotasks();
  vi.clearAllTimers();
  vi.useRealTimers();
});

describe("useDesktopReferenceCatalog", () => {
  test("debounces searches for 250ms and cancels the previous query timer", async () => {
    vi.mocked(desktopCatalog).mockResolvedValue(catalog([item("skill", "diagram")]));

    render("session-fixture", "skills", "old");
    await vi.advanceTimersByTimeAsync(200);
    render("session-fixture", "skills", "diagram");
    await vi.advanceTimersByTimeAsync(249);
    expect(desktopCatalog).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    await flushMicrotasks();

    expect(desktopCatalog).toHaveBeenCalledOnce();
    expect(desktopCatalog).toHaveBeenCalledWith({
      sessionId: "session-fixture", projectHandle: null, kind: "skills", query: "diagram", cursor: null
    });
  });

  test("fetches app and plugin mentions in series and keeps a successful source after a partial failure", async () => {
    const apps = deferred<DesktopCatalog>();
    const plugins = deferred<DesktopCatalog>();
    vi.mocked(desktopCatalog).mockReturnValueOnce(apps.promise).mockReturnValueOnce(plugins.promise);

    render("session-fixture", "mentions", "review");
    await vi.advanceTimersByTimeAsync(250);
    expect(desktopCatalog).toHaveBeenCalledOnce();
    expect(desktopCatalog).toHaveBeenNthCalledWith(1, {
      sessionId: "session-fixture", projectHandle: null, kind: "apps", query: "review", cursor: null
    });

    apps.reject(new Error("apps fixture unavailable"));
    await flushMicrotasks();
    expect(desktopCatalog).toHaveBeenCalledTimes(2);
    expect(desktopCatalog).toHaveBeenNthCalledWith(2, {
      sessionId: "session-fixture", projectHandle: null, kind: "plugins", query: "review", cursor: null
    });
    const partial = render("session-fixture", "mentions", "review");
    expect(partial.errors).toEqual([{ kind: "apps", message: "apps fixture unavailable" }]);
    expect(partial.busy).toBe(true);

    const plugin = item("plugin", "review-helper");
    plugins.resolve(catalog([plugin]));
    await flushMicrotasks();
    const complete = render("session-fixture", "mentions", "review");
    expect(complete.items).toEqual([plugin]);
    expect(complete.errors).toEqual([{ kind: "apps", message: "apps fixture unavailable" }]);
    expect(complete.busy).toBe(false);
  });

  test("manual retry starts a fresh first-page request and clears the error on success", async () => {
    const skill = item("skill", "diagram");
    vi.mocked(desktopCatalog)
      .mockRejectedValueOnce(new Error("catalog fixture unavailable"))
      .mockResolvedValueOnce(catalog([skill]));

    render("session-fixture", "skills", "dia");
    await vi.advanceTimersByTimeAsync(250);
    await flushMicrotasks();
    const failed = render("session-fixture", "skills", "dia");
    expect(failed.errors).toEqual([{ kind: "skills", message: "catalog fixture unavailable" }]);
    expect(desktopCatalog).toHaveBeenCalledOnce();

    failed.retry();
    render("session-fixture", "skills", "dia");
    expect(desktopCatalog).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(250);
    await flushMicrotasks();

    const retried = render("session-fixture", "skills", "dia");
    expect(desktopCatalog).toHaveBeenCalledTimes(2);
    expect(desktopCatalog).toHaveBeenLastCalledWith({
      sessionId: "session-fixture", projectHandle: null, kind: "skills", query: "dia", cursor: null, forceRefetch: true
    });
    expect(retried.items).toEqual([skill]);
    expect(retried.errors).toEqual([]);
  });

  test("retry refreshes only the failed source and later searches use the ordinary cache", async () => {
    const plugin = item("plugin", "ready");
    const app = item("app", "ready");
    vi.mocked(desktopCatalog).mockRejectedValueOnce(new Error("app fixture failed"))
      .mockResolvedValueOnce(catalog([plugin])).mockResolvedValueOnce(catalog([app]))
      .mockResolvedValue(catalog([]));
    render("session-fixture", "mentions", "ready");
    await vi.advanceTimersByTimeAsync(250);
    await flushMicrotasks();
    const failed = render("session-fixture", "mentions", "ready");
    failed.retry();
    expect(render("session-fixture", "mentions", "ready").items).toEqual([plugin]);
    await flushMicrotasks();
    expect(desktopCatalog).toHaveBeenCalledTimes(3);
    expect(desktopCatalog).toHaveBeenLastCalledWith({ sessionId: "session-fixture", projectHandle: null, kind: "apps", query: "ready", cursor: null, forceRefetch: true });
    expect(render("session-fixture", "mentions", "ready").items).toEqual([app, plugin]);
    render("session-fixture", "mentions", "changed");
    await vi.advanceTimersByTimeAsync(250);
    await flushMicrotasks();
    expect(vi.mocked(desktopCatalog).mock.calls.slice(3).every(([request]) => request.forceRefetch === undefined)).toBe(true);
  });

  test("retry after an expired cursor requests the first page again", async () => {
    const oldPage = item("skill", "old-page");
    const newPage = item("skill", "fresh-page");
    vi.mocked(desktopCatalog)
      .mockResolvedValueOnce(catalog([oldPage], "stale-cursor-fixture"))
      .mockRejectedValueOnce(new Error("cursor fixture expired"))
      .mockResolvedValueOnce(catalog([newPage]));

    render("session-fixture", "skills", "page");
    await vi.advanceTimersByTimeAsync(250);
    await flushMicrotasks();
    const firstPage = render("session-fixture", "skills", "page");
    expect(firstPage.items).toEqual([oldPage]);
    expect(firstPage.hasMore).toBe(true);

    firstPage.more();
    await flushMicrotasks();
    const failedCursor = render("session-fixture", "skills", "page");
    expect(failedCursor.errors).toEqual([{ kind: "skills", message: "cursor fixture expired" }]);
    expect(failedCursor.items).toEqual([oldPage]);

    failedCursor.retry();
    render("session-fixture", "skills", "page");
    await vi.advanceTimersByTimeAsync(250);
    await flushMicrotasks();

    const retried = render("session-fixture", "skills", "page");
    expect(desktopCatalog).toHaveBeenNthCalledWith(3, {
      sessionId: "session-fixture", projectHandle: null, kind: "skills", query: "page", cursor: null, forceRefetch: true
    });
    expect(retried.items).toEqual([newPage]);
    expect(retried.errors).toEqual([]);
  });

  test("limits windows to 100 and replaces consumed source snapshots with the next cursor page", async () => {
    const apps = Array.from({ length: 100 }, (_, index) => item("app", `app-${index}`));
    const plugins = Array.from({ length: 50 }, (_, index) => item("plugin", `plugin-${index}`));
    const nextPlugins = Array.from({ length: 20 }, (_, index) => item("plugin", `next-plugin-${index}`));
    vi.mocked(desktopCatalog)
      .mockResolvedValueOnce(catalog(apps))
      .mockResolvedValueOnce(catalog(plugins, "plugin-next-fixture"))
      .mockResolvedValueOnce(catalog(nextPlugins));

    render("session-fixture", "mentions", "all");
    await vi.advanceTimersByTimeAsync(250);
    await flushMicrotasks();
    const firstWindow = render("session-fixture", "mentions", "all");
    expect(firstWindow.items).toHaveLength(100);
    expect(firstWindow.items).toEqual(apps);
    expect(firstWindow.hasMore).toBe(true);

    firstWindow.more();
    const localPluginWindow = render("session-fixture", "mentions", "all");
    expect(localPluginWindow.items).toHaveLength(50);
    expect(localPluginWindow.items).toEqual(plugins);
    expect(desktopCatalog).toHaveBeenCalledTimes(2);

    localPluginWindow.more();
    await flushMicrotasks();
    const nextCursorWindow = render("session-fixture", "mentions", "all");
    expect(desktopCatalog).toHaveBeenNthCalledWith(3, {
      sessionId: "session-fixture", projectHandle: null, kind: "plugins", query: "all", cursor: "plugin-next-fixture"
    });
    expect(nextCursorWindow.items).toEqual(nextPlugins);
    expect(nextCursorWindow.items).toHaveLength(20);
    expect(nextCursorWindow.items.length).toBeLessThanOrEqual(100);
    expect(nextCursorWindow.items.some(value => apps.some(previous => previous.handle === value.handle))).toBe(false);
    expect(nextCursorWindow.items.some(value => plugins.some(previous => previous.handle === value.handle))).toBe(false);
    expect(new Set(nextCursorWindow.items.map(value => value.handle)).size).toBe(20);
    expect(nextCursorWindow.hasMore).toBe(false);
  });

  test("keeps successful plugin results visible when the app source times out during paging", async () => {
    const apps = Array.from({ length: 100 }, (_, index) => item("app", `timeout-app-${index}`));
    const plugins = Array.from({ length: 50 }, (_, index) => item("plugin", `timeout-plugin-${index}`));
    const nextPlugins = Array.from({ length: 20 }, (_, index) => item("plugin", `fresh-plugin-${index}`));
    vi.mocked(desktopCatalog)
      .mockResolvedValueOnce(catalog(apps, "apps-next-fixture"))
      .mockResolvedValueOnce(catalog(plugins, "plugins-next-fixture"))
      .mockRejectedValueOnce(new Error("apps page timed out"))
      .mockResolvedValueOnce(catalog(nextPlugins));

    render("session-fixture", "mentions", "timeout");
    await vi.advanceTimersByTimeAsync(250);
    await flushMicrotasks();
    const firstWindow = render("session-fixture", "mentions", "timeout");
    expect(firstWindow.items).toHaveLength(100);
    firstWindow.more();
    const pluginWindow = render("session-fixture", "mentions", "timeout");
    expect(pluginWindow.items).toEqual(plugins);

    pluginWindow.more();
    await flushMicrotasks();
    const afterPaging = render("session-fixture", "mentions", "timeout");
    expect(desktopCatalog).toHaveBeenNthCalledWith(3, {
      sessionId: "session-fixture", projectHandle: null, kind: "apps", query: "timeout", cursor: "apps-next-fixture"
    });
    expect(desktopCatalog).toHaveBeenNthCalledWith(4, {
      sessionId: "session-fixture", projectHandle: null, kind: "plugins", query: "timeout", cursor: "plugins-next-fixture"
    });
    expect(afterPaging.errors).toEqual([{ kind: "apps", message: "apps page timed out" }]);
    expect(afterPaging.hasMore).toBe(true);

    afterPaging.more();
    const successfulPluginWindow = render("session-fixture", "mentions", "timeout");
    expect(successfulPluginWindow.items).toEqual(nextPlugins);
    expect(successfulPluginWindow.items).toHaveLength(20);
    expect(successfulPluginWindow.errors).toEqual([{ kind: "apps", message: "apps page timed out" }]);
  });

  test("a session switch cancels the old debounce timer", async () => {
    vi.mocked(desktopCatalog).mockResolvedValue(catalog([]));

    render("session-old", "skills", "fixture");
    await vi.advanceTimersByTimeAsync(200);
    render("session-new", "skills", "fixture");
    await vi.advanceTimersByTimeAsync(50);
    expect(desktopCatalog).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(199);
    expect(desktopCatalog).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    await flushMicrotasks();
    expect(desktopCatalog).toHaveBeenCalledOnce();
    expect(desktopCatalog).toHaveBeenCalledWith({
      sessionId: "session-new", projectHandle: null, kind: "skills", query: "fixture", cursor: null
    });
  });

  test("a query switch drops a late response from the previous request", async () => {
    const oldRequest = deferred<DesktopCatalog>();
    const freshItem = item("skill", "fresh-result");
    vi.mocked(desktopCatalog).mockReturnValueOnce(oldRequest.promise).mockResolvedValueOnce(catalog([freshItem]));

    render("session-fixture", "skills", "old-query");
    await vi.advanceTimersByTimeAsync(250);
    expect(desktopCatalog).toHaveBeenCalledOnce();

    render("session-fixture", "skills", "fresh-query");
    oldRequest.resolve(catalog([item("skill", "stale-result")]));
    await flushMicrotasks();
    const whileFreshDebounces = render("session-fixture", "skills", "fresh-query");
    expect(whileFreshDebounces.items).toEqual([]);
    expect(whileFreshDebounces.busy).toBe(true);

    await vi.advanceTimersByTimeAsync(250);
    await flushMicrotasks();
    const fresh = render("session-fixture", "skills", "fresh-query");
    expect(desktopCatalog).toHaveBeenCalledTimes(2);
    expect(desktopCatalog).toHaveBeenLastCalledWith({
      sessionId: "session-fixture", projectHandle: null, kind: "skills", query: "fresh-query", cursor: null
    });
    expect(fresh.items).toEqual([freshItem]);
  });
});
