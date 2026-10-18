import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { DesktopAction, DesktopView } from "../lib/api/desktop";
import { DesktopStatus } from "../components/chat/DesktopStatus";
import { actOnDesktopThread, attachDesktopThread, openDesktopThread } from "../lib/query/desktop";
import { requestDesktopOpenIntent } from "../lib/desktopIntent";
import { useCodexDesktop } from "./useCodexDesktop";

const lifecycle = vi.hoisted(() => ({
  index: 0,
  dirty: false,
  states: [] as unknown[],
  refs: [] as { current: unknown }[],
  effects: [] as any[],
  pendingEffects: [] as any[],
  connection: { target: "local" as "local" | "remote", revision: 1 },
  machineCallbacks: new Set<() => void | Promise<void>>(),
  visibilityListener: null as (() => void) | null,
  documentFixture: null as any,
  attachments: [] as any[],
  cache: {
    invalidateThread: vi.fn(),
    invalidateThreads: vi.fn(),
    invalidateThreadAndThreads: vi.fn(),
    refreshDesktopThread: vi.fn(async (_threadId: string): Promise<void> => undefined)
  }
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
      const changed = !previous || !deps || !previous.deps || deps.length !== previous.deps.length || deps.some((value, item) => !Object.is(value, previous.deps[item]));
      if (changed) lifecycle.pendingEffects.push({ index, create, deps });
    },
    useCallback: (callback: unknown) => {
      lifecycle.index++;
      return callback;
    }
  };
});

vi.mock("../lib/query/desktop", () => ({
  actOnDesktopThread: vi.fn(),
  attachDesktopThread: vi.fn(),
  openDesktopThread: vi.fn()
}));

vi.mock("../lib/query/connection", () => ({
  beforeMachineChange: (callback: () => void | Promise<void>) => {
    lifecycle.machineCallbacks.add(callback);
    return () => lifecycle.machineCallbacks.delete(callback);
  },
  connectionSnapshot: () => lifecycle.connection,
  useConnection: () => lifecycle.connection
}));

vi.mock("../lib/query/threads", () => ({ useThreadCacheActions: () => lifecycle.cache }));

type Attached = NonNullable<Awaited<ReturnType<typeof attachDesktopThread>>>;
type Deferred<T> = { promise: Promise<T>; resolve: (value: T) => void; reject: (reason?: unknown) => void };
type Attachment = { threadId: string; receive: (view: DesktopView) => void; confirmOwner: boolean; pending: Deferred<Attached> };

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((accept, fail) => { resolve = accept; reject = fail; });
  return { promise, resolve, reject };
}

function desktopView(threadId: string, sessionId = `session-${threadId}`, revision = 1): DesktopView {
  return {
    sessionId, threadId, revision, connected: true, writable: true, runtime: "codex",
    activeTurnId: null, model: null, reasoningEffort: null, mode: null,
    requests: [], queue: [], error: null
  };
}

function attachmentList(): Attachment[] { return lifecycle.attachments as Attachment[]; }

function resolveAttachment(attachment: Attachment, initial: DesktopView) {
  const detach = vi.fn(async () => undefined);
  attachment.pending.resolve({ initial, detach });
  return detach;
}

function render(threadId: string): ReturnType<typeof useCodexDesktop> {
  let result!: ReturnType<typeof useCodexDesktop>;
  for (let pass = 0; pass < 8; pass += 1) {
    lifecycle.dirty = false;
    lifecycle.index = 0;
    result = useCodexDesktop(threadId);
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
  throw new Error("desktop hook fixture did not settle");
}

function statusMarkup(view: DesktopView | null): string {
  return renderToStaticMarkup(createElement(DesktopStatus, { view }));
}

function unmount() {
  for (const effect of [...lifecycle.effects].reverse()) effect?.cleanup?.();
  lifecycle.effects = [];
  lifecycle.pendingEffects = [];
}

async function flushMicrotasks() {
  for (let index = 0; index < 8; index += 1) await Promise.resolve();
}

async function connect(threadId: string) {
  let desktop = render(threadId);
  desktop.connect();
  render(threadId);
  const attachment = attachmentList()[attachmentList().length - 1];
  const detach = resolveAttachment(attachment, desktopView(threadId));
  await flushMicrotasks();
  desktop = render(threadId);
  return { desktop, attachment, detach };
}

beforeEach(() => {
  lifecycle.index = 0;
  lifecycle.dirty = false;
  lifecycle.states = [];
  lifecycle.refs = [];
  lifecycle.effects = [];
  lifecycle.pendingEffects = [];
  lifecycle.connection = { target: "local", revision: 1 };
  lifecycle.machineCallbacks.clear();
  lifecycle.visibilityListener = null;
  lifecycle.attachments = [];
  lifecycle.cache.invalidateThread.mockReset();
  lifecycle.cache.invalidateThreads.mockReset();
  lifecycle.cache.invalidateThreadAndThreads.mockReset();
  lifecycle.cache.refreshDesktopThread.mockReset().mockResolvedValue(undefined);

  const documentFixture = {
    hidden: false,
    addEventListener: vi.fn((type: string, listener: () => void) => {
      if (type === "visibilitychange") lifecycle.visibilityListener = listener;
    }),
    removeEventListener: vi.fn((type: string, listener: () => void) => {
      if (type === "visibilitychange" && lifecycle.visibilityListener === listener) lifecycle.visibilityListener = null;
    })
  };
  lifecycle.documentFixture = documentFixture;
  vi.stubGlobal("document", documentFixture);

  vi.mocked(attachDesktopThread).mockReset().mockImplementation((threadId, receive, confirmOwner = true) => {
    const pending = deferred<Attached>();
    lifecycle.attachments.push({ threadId, receive, confirmOwner, pending });
    return pending.promise;
  });
  vi.mocked(actOnDesktopThread).mockReset().mockResolvedValue(desktopView("thread-fixture"));
  vi.mocked(openDesktopThread).mockReset().mockResolvedValue({ ok: true });
});

afterEach(async () => {
  unmount();
  await flushMicrotasks();
  vi.clearAllTimers();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("Codex desktop hook lifecycle", () => {
  test("a failed machine switch clears the selection and drops late action and stream responses", async () => {
    const connected = await connect("thread-fixture");
    const actionResult = deferred<DesktopView>();
    vi.mocked(actOnDesktopThread).mockReturnValueOnce(actionResult.promise);
    const pendingAction = connected.desktop.act({ type: "stop", turnId: "turn-fixture" });
    const [beforeChange] = lifecycle.machineCallbacks;
    expect(beforeChange).toBeTypeOf("function");

    await beforeChange();
    const disconnected = render("thread-fixture");
    expect(lifecycle.connection).toEqual({ target: "local", revision: 1 });
    expect(disconnected.view).toBeNull();
    expect(statusMarkup(disconnected.view)).toBe("");
    expect(disconnected.busy).toBe(false);
    expect(connected.detach).toHaveBeenCalledOnce();

    connected.attachment.receive(desktopView("thread-fixture", "session-thread-fixture", 80));
    actionResult.resolve(desktopView("thread-fixture", "session-thread-fixture", 90));
    await expect(pendingAction).rejects.toThrow("所选线程已变化，旧操作结果已丢弃");

    const afterLateResponses = render("thread-fixture");
    expect(afterLateResponses.view).toBeNull();
    expect(statusMarkup(afterLateResponses.view)).toBe("");
    expect(afterLateResponses.error).toBe("");
  });

  test("thread changes discard a late attach and keep the new thread snapshot", async () => {
    let desktop = render("thread-old");
    desktop.connect();
    render("thread-old");
    const oldAttachment = attachmentList()[0];
    expect(oldAttachment.confirmOwner).toBe(true);

    desktop = render("thread-new");
    expect(desktop.view).toBeNull();
    expect(statusMarkup(desktop.view)).toBe("");
    expect(desktop.busy).toBe(false);
    expect(attachmentList()).toHaveLength(1);
    desktop.connect();
    render("thread-new");
    const newAttachment = attachmentList()[1];
    expect(newAttachment.confirmOwner).toBe(true);
    const newView = desktopView("thread-new", "session-new", 3);
    const newDetach = resolveAttachment(newAttachment, newView);
    await flushMicrotasks();
    expect(render("thread-new").view).toEqual(newView);
    expect(statusMarkup(render("thread-new").view)).toContain("空闲");

    const staleOldView = { ...desktopView("thread-old", "session-old", 99), runtime: "active", activeTurnId: "stale-turn" };
    const oldDetach = resolveAttachment(oldAttachment, staleOldView);
    await flushMicrotasks();
    oldAttachment.receive({ ...staleOldView, revision: 100 });

    expect(oldDetach).toHaveBeenCalledOnce();
    expect(render("thread-new").view).toEqual(newView);
    expect(statusMarkup(render("thread-new").view)).toContain("空闲");
    expect(statusMarkup(render("thread-new").view)).not.toContain("执行中");
    expect(newDetach).not.toHaveBeenCalled();
  });

  test("hiding detaches the UI subscription and becoming visible attaches again", async () => {
    vi.useFakeTimers();
    const connected = await connect("thread-visible");
    expect(attachmentList()[0].confirmOwner).toBe(true);
    const documentFixture = lifecycle.documentFixture as { hidden: boolean };
    const notifyVisibility = () => {
      if (!lifecycle.visibilityListener) throw new Error("visibility listener fixture is missing");
      lifecycle.visibilityListener();
    };

    documentFixture.hidden = true;
    notifyVisibility();
    const hidden = render("thread-visible");
    expect(hidden.view).toBeNull();
    expect(statusMarkup(hidden.view)).toBe("");
    expect(hidden.busy).toBe(false);
    expect(connected.detach).toHaveBeenCalledOnce();
    expect(attachmentList()).toHaveLength(1);
    connected.attachment.receive({ ...desktopView("thread-visible", "session-thread-visible", 90), runtime: "active", activeTurnId: "stale-turn" });
    expect(render("thread-visible").view).toBeNull();
    expect(statusMarkup(render("thread-visible").view)).toBe("");
    await vi.advanceTimersByTimeAsync(1_000);
    expect(lifecycle.cache.refreshDesktopThread).not.toHaveBeenCalled();

    documentFixture.hidden = false;
    notifyVisibility();
    render("thread-visible");
    expect(attachmentList()).toHaveLength(2);
    const resumedAttachment = attachmentList()[1];
    expect(resumedAttachment.confirmOwner).toBe(false);
    const resumedView = desktopView("thread-visible", "session-resumed", 2);
    resolveAttachment(resumedAttachment, resumedView);
    await flushMicrotasks();
    expect(render("thread-visible").view).toEqual(resumedView);
    expect(statusMarkup(render("thread-visible").view)).toContain("空闲");
  });

  test("a new session drops late stream updates from the previous session", async () => {
    vi.useFakeTimers();
    const connected = await connect("thread-session");
    await vi.advanceTimersByTimeAsync(350);
    expect(lifecycle.cache.refreshDesktopThread).toHaveBeenCalledTimes(1);
    const current = render("thread-session").view;
    expect(statusMarkup(current)).toContain("空闲");

    connected.attachment.receive({ ...desktopView("thread-session", "session-replaced", 90), runtime: "active", activeTurnId: "stale-turn" });
    expect(render("thread-session").view).toEqual(current);
    expect(statusMarkup(render("thread-session").view)).toContain("空闲");
    expect(statusMarkup(render("thread-session").view)).not.toContain("执行中");
    await vi.advanceTimersByTimeAsync(1_000);
    expect(lifecycle.cache.refreshDesktopThread).toHaveBeenCalledTimes(1);
  });

  test("a read-only owner stays unconfirmed on hide and show until an explicit connect", async () => {
    let desktop = render("thread-readonly");
    desktop.connect();
    render("thread-readonly");
    const first = attachmentList()[0];
    expect(first.confirmOwner).toBe(true);
    const readonlyView = { ...desktopView("thread-readonly"), writable: false };
    const firstDetach = resolveAttachment(first, readonlyView);
    await flushMicrotasks();
    expect(render("thread-readonly").view?.writable).toBe(false);

    const documentFixture = lifecycle.documentFixture as { hidden: boolean };
    documentFixture.hidden = true;
    lifecycle.visibilityListener?.();
    render("thread-readonly");
    expect(firstDetach).toHaveBeenCalledOnce();

    documentFixture.hidden = false;
    lifecycle.visibilityListener?.();
    render("thread-readonly");
    const resumed = attachmentList()[1];
    expect(resumed.confirmOwner).toBe(false);
    resolveAttachment(resumed, { ...readonlyView, sessionId: "session-readonly-resumed" });
    await flushMicrotasks();

    desktop = render("thread-readonly");
    desktop.connect();
    render("thread-readonly");
    expect(attachmentList()[2].confirmOwner).toBe(true);
  });

  test("a successful Open intent automatically attaches with owner confirmation", async () => {
    const result = deferred<{ ok: boolean }>();
    vi.mocked(openDesktopThread).mockReturnValueOnce(result.promise);
    const desktop = render("thread-open");
    const opening = desktop.open();

    expect(openDesktopThread).toHaveBeenCalledExactlyOnceWith("thread-open");
    result.resolve({ ok: true });
    await opening;
    render("thread-open");

    expect(attachmentList()).toHaveLength(1);
    expect(attachmentList()[0].confirmOwner).toBe(true);
  });

  test("a late Open result after changing threads does not start an attach", async () => {
    const result = deferred<{ ok: boolean }>();
    vi.mocked(openDesktopThread).mockReturnValueOnce(result.promise);
    const opening = render("thread-open-old").open();
    render("thread-open-new");

    result.resolve({ ok: true });
    await opening;
    render("thread-open-new");

    expect(attachmentList()).toHaveLength(0);
  });

  test("a create intent is consumed once and attaches with owner confirmation", async () => {
    requestDesktopOpenIntent("thread-created");
    render("thread-created");

    expect(attachmentList()).toHaveLength(1);
    expect(attachmentList()[0].confirmOwner).toBe(true);
    expect(attachmentList()[0].threadId).toBe("thread-created");
  });

  test("projects a synthetic dispatch event in the status row within one second", async () => {
    vi.useFakeTimers();
    const connected = await connect("thread-projection");
    const dispatching = {
      ...desktopView("thread-projection"),
      queue: [{ id: "message-fixture", status: "dispatching", kind: "send", preview: "fixture", createdAt: 1 }]
    };

    connected.attachment.receive(dispatching);
    await vi.advanceTimersByTimeAsync(999);

    const projection = render("thread-projection").view;
    expect(projection).toEqual(dispatching);
    expect(statusMarkup(projection)).toContain("正在提交到 Codex…");
  });

  test("connection metadata accepts a new session epoch and drops old session and action results", async () => {
    const connected = await connect("thread-epoch");
    connected.attachment.receive(desktopView("thread-epoch", "session-thread-epoch", 8));
    expect(render("thread-epoch").view?.revision).toBe(8);
    const pendingResult = deferred<DesktopView>();
    vi.mocked(actOnDesktopThread).mockReturnValueOnce(pendingResult.promise);
    const pendingAction = render("thread-epoch").act({ type: "stop", turnId: "turn-fixture" });
    const epoch = {
      ...desktopView("thread-epoch", "session-next", 1),
      connectionPhase: "connecting"
    };

    connected.attachment.receive(epoch);
    expect(render("thread-epoch").view).toEqual(epoch);
    connected.attachment.receive({ ...desktopView("thread-epoch", "session-thread-epoch", 99), runtime: "active", activeTurnId: "stale-turn" });
    expect(render("thread-epoch").view).toEqual(epoch);

    pendingResult.resolve(desktopView("thread-epoch", "session-thread-epoch", 100));
    await expect(pendingAction).rejects.toThrow("所选线程已变化，旧操作结果已丢弃");
    expect(render("thread-epoch").view).toEqual(epoch);
  });

  test("approval and completion events during a slow detail read get one trailing refresh", async () => {
    vi.useFakeTimers();
    const slow = deferred<void>();
    lifecycle.cache.refreshDesktopThread.mockReturnValueOnce(slow.promise);
    const connected = await connect("thread-refresh");
    await vi.advanceTimersByTimeAsync(350);
    expect(lifecycle.cache.refreshDesktopThread).toHaveBeenCalledTimes(1);

    connected.attachment.receive({
      ...desktopView("thread-refresh", "session-thread-refresh", 2),
      requests: [{
        key: "request-fixture", kind: "command", title: "Fixture request", body: "",
        questions: [], formSchema: null, canAccept: true
      }]
    });
    const completed = { ...desktopView("thread-refresh", "session-thread-refresh", 3), runtime: "idle" };
    connected.attachment.receive(completed);
    expect(render("thread-refresh").view).toEqual(completed);

    await vi.advanceTimersByTimeAsync(5_000);
    expect(lifecycle.cache.refreshDesktopThread).toHaveBeenCalledTimes(1);
    slow.resolve(undefined);
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(349);
    expect(lifecycle.cache.refreshDesktopThread).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(lifecycle.cache.refreshDesktopThread).toHaveBeenCalledTimes(2);
    expect(lifecycle.cache.refreshDesktopThread).toHaveBeenLastCalledWith("thread-refresh");
  });

  test("a later successful operation or recovery clears a previous operation error", async () => {
    const connected = await connect("thread-error");
    const action = { type: "stop" as const, turnId: "turn-fixture" };
    vi.mocked(actOnDesktopThread).mockRejectedValueOnce(new Error("Codex 未接受此操作"));
    await expect(connected.desktop.act(action)).rejects.toThrow("Codex 未接受此操作");
    expect(render("thread-error").error).toBe("Codex 未接受此操作");
    vi.mocked(actOnDesktopThread).mockResolvedValueOnce(desktopView("thread-error", "session-thread-error", 2));
    await render("thread-error").act(action);
    expect(render("thread-error").error).toBe("");
    vi.mocked(actOnDesktopThread).mockRejectedValueOnce(new Error("操作结果未确认"));
    await expect(render("thread-error").act(action)).rejects.toThrow("操作结果未确认");
    connected.attachment.receive({ ...desktopView("thread-error", "recovered-fixture", 1), connectionPhase: "connected" });
    expect(render("thread-error").error).toBe("");
  });

  test("clearActionError clears only a rejected action and preserves view errors and unknown queue entries", async () => {
    const connected = await connect("thread-clear-action-error");
    const viewError = "fixture view error";
    connected.attachment.receive({
      ...desktopView("thread-clear-action-error"),
      error: viewError,
      queue: [{ id: "unknown-fixture", status: "outcome_unknown", kind: "queue", preview: "fixture unknown", createdAt: 1 }]
    });
    let desktop = render("thread-clear-action-error");
    expect(desktop.error).toBe(viewError);
    expect(desktop.view?.queue[0]?.status).toBe("outcome_unknown");

    const action = { type: "stop" as const, turnId: "turn-fixture" };
    vi.mocked(actOnDesktopThread).mockRejectedValueOnce(new Error("fixture action error"));
    await expect(desktop.act(action)).rejects.toThrow("fixture action error");
    desktop = render("thread-clear-action-error");
    expect(desktop.error).toBe("fixture action error");

    desktop.clearActionError();
    desktop = render("thread-clear-action-error");
    expect(desktop.error).toBe(viewError);
    expect(desktop.view?.queue[0]?.status).toBe("outcome_unknown");
  });

  test("clearActionError does not clear a connection failure", async () => {
    let desktop = render("thread-connect-error");
    desktop.connect();
    render("thread-connect-error");
    const attachment = attachmentList()[0];
    attachment.pending.reject(new Error("fixture connect error"));
    await flushMicrotasks();

    desktop = render("thread-connect-error");
    expect(desktop.error).toBe("fixture connect error");
    desktop.clearActionError();
    expect(render("thread-connect-error").error).toBe("fixture connect error");
  });

  test("clearActionError does not clear an Open failure", async () => {
    vi.mocked(openDesktopThread).mockRejectedValueOnce(new Error("fixture open error"));
    const desktop = render("thread-open-error");
    await desktop.open();

    expect(render("thread-open-error").error).toBe("fixture open error");
    desktop.clearActionError();
    expect(render("thread-open-error").error).toBe("fixture open error");
  });

  test("manual actions request a coalesced thread detail refresh", async () => {
    vi.useFakeTimers();
    const connected = await connect("thread-action");
    await vi.advanceTimersByTimeAsync(350);
    lifecycle.cache.refreshDesktopThread.mockClear();
    vi.mocked(actOnDesktopThread).mockResolvedValueOnce(desktopView("thread-action", "session-thread-action", 2));

    await connected.desktop.act({ type: "stop", turnId: "turn-fixture" });
    expect(lifecycle.cache.refreshDesktopThread).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(349);
    expect(lifecycle.cache.refreshDesktopThread).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(lifecycle.cache.refreshDesktopThread).toHaveBeenCalledOnce();
    expect(lifecycle.cache.refreshDesktopThread).toHaveBeenCalledWith("thread-action");
  });
});
