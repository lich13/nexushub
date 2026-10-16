import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { DesktopAction, DesktopView } from "../lib/api/desktop";
import { actOnDesktopThread, attachDesktopThread, openDesktopThread } from "../lib/query/desktop";
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
    invalidateThreadAndThreads: vi.fn()
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
type Attachment = { threadId: string; receive: (view: DesktopView) => void; pending: Deferred<Attached> };

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

  vi.mocked(attachDesktopThread).mockReset().mockImplementation((threadId, receive) => {
    const pending = deferred<Attached>();
    lifecycle.attachments.push({ threadId, receive, pending });
    return pending.promise;
  });
  vi.mocked(actOnDesktopThread).mockReset().mockResolvedValue(desktopView("thread-fixture"));
  vi.mocked(openDesktopThread).mockReset().mockResolvedValue({ ok: true });
});

afterEach(async () => {
  unmount();
  await flushMicrotasks();
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
    expect(disconnected.busy).toBe(false);
    expect(connected.detach).toHaveBeenCalledOnce();

    connected.attachment.receive(desktopView("thread-fixture", "session-thread-fixture", 80));
    actionResult.resolve(desktopView("thread-fixture", "session-thread-fixture", 90));
    await expect(pendingAction).rejects.toThrow("所选线程已变化，旧操作结果已丢弃");

    const afterLateResponses = render("thread-fixture");
    expect(afterLateResponses.view).toBeNull();
    expect(afterLateResponses.error).toBe("");
  });

  test("thread changes discard a late attach and keep the new thread snapshot", async () => {
    let desktop = render("thread-old");
    desktop.connect();
    render("thread-old");
    const oldAttachment = attachmentList()[0];

    desktop = render("thread-new");
    expect(desktop.view).toBeNull();
    expect(desktop.busy).toBe(false);
    desktop.connect();
    render("thread-new");
    const newAttachment = attachmentList()[1];
    const newView = desktopView("thread-new", "session-new", 3);
    const newDetach = resolveAttachment(newAttachment, newView);
    await flushMicrotasks();
    expect(render("thread-new").view).toEqual(newView);

    const oldDetach = resolveAttachment(oldAttachment, desktopView("thread-old", "session-old", 99));
    await flushMicrotasks();
    oldAttachment.receive(desktopView("thread-old", "session-old", 100));

    expect(oldDetach).toHaveBeenCalledOnce();
    expect(render("thread-new").view).toEqual(newView);
    expect(newDetach).not.toHaveBeenCalled();
  });

  test("hiding detaches the UI subscription and becoming visible attaches again", async () => {
    const connected = await connect("thread-visible");
    const documentFixture = lifecycle.documentFixture as { hidden: boolean };
    const notifyVisibility = () => {
      if (!lifecycle.visibilityListener) throw new Error("visibility listener fixture is missing");
      lifecycle.visibilityListener();
    };

    documentFixture.hidden = true;
    notifyVisibility();
    const hidden = render("thread-visible");
    expect(hidden.view).toBeNull();
    expect(hidden.busy).toBe(false);
    expect(connected.detach).toHaveBeenCalledOnce();
    expect(attachmentList()).toHaveLength(1);

    documentFixture.hidden = false;
    notifyVisibility();
    render("thread-visible");
    expect(attachmentList()).toHaveLength(2);
    const resumedAttachment = attachmentList()[1];
    const resumedView = desktopView("thread-visible", "session-resumed", 2);
    resolveAttachment(resumedAttachment, resumedView);
    await flushMicrotasks();
    expect(render("thread-visible").view).toEqual(resumedView);
  });
});
