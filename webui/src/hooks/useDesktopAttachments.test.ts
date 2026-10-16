import type { ClipboardEvent, RefObject } from "react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { DesktopAttachmentEvent, DesktopChosenItem } from "../lib/api/desktop";
import { bindDesktopAttachments } from "../lib/query/desktop";
import { useDesktopAttachments } from "./useDesktopAttachments";

const lifecycle = vi.hoisted(() => ({
  index: 0,
  refs: [] as Array<{ current: unknown }>,
  effects: [] as any[],
  pendingEffects: [] as any[]
}));

vi.mock("react", async importOriginal => {
  const actual = await importOriginal<typeof import("react")>();
  return {
    ...actual,
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

vi.mock("../lib/query/desktop", () => ({ bindDesktopAttachments: vi.fn() }));

type Binding = Awaited<ReturnType<typeof bindDesktopAttachments>>;
type Listener = () => void;
type Rect = { left: number; top: number; right: number; bottom: number };

let observers: FakeResizeObserver[];
let windowListeners: Map<string, Set<Listener>>;
let documentListeners: Map<string, Set<Listener>>;
let fakeWindow: {
  innerWidth: number;
  innerHeight: number;
  addEventListener: ReturnType<typeof vi.fn>;
  removeEventListener: ReturnType<typeof vi.fn>;
};
let fakeDocument: {
  hidden: boolean;
  addEventListener: ReturnType<typeof vi.fn>;
  removeEventListener: ReturnType<typeof vi.fn>;
};

class FakeResizeObserver {
  observe = vi.fn();
  disconnect = vi.fn();

  constructor(private readonly callback: () => void) { observers.push(this); }

  trigger() { this.callback(); }
}

function installBrowserGlobals() {
  observers = [];
  windowListeners = new Map();
  documentListeners = new Map();
  const add = (listeners: Map<string, Set<Listener>>, type: string, listener: Listener) => {
    const values = listeners.get(type) ?? new Set<Listener>();
    values.add(listener);
    listeners.set(type, values);
  };
  const remove = (listeners: Map<string, Set<Listener>>, type: string, listener: Listener) => {
    listeners.get(type)?.delete(listener);
  };
  fakeWindow = {
    innerWidth: 1000,
    innerHeight: 800,
    addEventListener: vi.fn((type: string, listener: Listener) => add(windowListeners, type, listener)),
    removeEventListener: vi.fn((type: string, listener: Listener) => remove(windowListeners, type, listener))
  };
  fakeDocument = {
    hidden: false,
    addEventListener: vi.fn((type: string, listener: Listener) => add(documentListeners, type, listener)),
    removeEventListener: vi.fn((type: string, listener: Listener) => remove(documentListeners, type, listener))
  };
  vi.stubGlobal("window", fakeWindow);
  vi.stubGlobal("document", fakeDocument);
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
}

function fire(listeners: Map<string, Set<Listener>>, type: string) {
  for (const listener of [...(listeners.get(type) ?? [])]) listener();
}

function createRegion(initial: Rect) {
  const rect = { current: initial };
  const region = {
    current: { getBoundingClientRect: () => rect.current }
  } as unknown as RefObject<HTMLElement>;
  return { region, setRect: (next: Rect) => { rect.current = next; } };
}

function createBinding(): Binding {
  return {
    update: vi.fn(async () => undefined),
    paste: vi.fn(async () => undefined),
    release: vi.fn(async () => undefined),
    close: vi.fn(async () => undefined)
  } as unknown as Binding;
}

function render(
  sessionId: string | undefined,
  region: RefObject<HTMLElement>,
  handles: string[],
  receive: (items: DesktopChosenItem[]) => void,
  fail: (message: string) => void
) {
  lifecycle.index = 0;
  const result = useDesktopAttachments(sessionId, region, handles, receive, fail);
  for (const effect of lifecycle.pendingEffects.splice(0)) {
    lifecycle.effects[effect.index]?.cleanup?.();
    const cleanup = effect.create();
    lifecycle.effects[effect.index] = {
      deps: effect.deps ? [...effect.deps] : undefined,
      cleanup: typeof cleanup === "function" ? cleanup : undefined
    };
  }
  return result;
}

function unmount() {
  for (const effect of [...lifecycle.effects].reverse()) effect?.cleanup?.();
  lifecycle.effects = [];
  lifecycle.pendingEffects = [];
}

async function flushMicrotasks() {
  for (let index = 0; index < 12; index += 1) await Promise.resolve();
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((accept, fail) => { resolve = accept; reject = fail; });
  return { promise, resolve, reject };
}

function attachmentEvent(overrides: Partial<DesktopAttachmentEvent> = {}): DesktopAttachmentEvent {
  return {
    bindingId: "binding-fixture",
    items: [{ handle: "file-handle-fixture", name: "image.png", kind: "file" }],
    error: null,
    ...overrides
  };
}

function clipboardEvent(items: Array<{ kind: string; type: string }>) {
  return {
    clipboardData: { items },
    preventDefault: vi.fn()
  } as unknown as ClipboardEvent<HTMLTextAreaElement>;
}

beforeEach(() => {
  lifecycle.index = 0;
  lifecycle.refs = [];
  lifecycle.effects = [];
  lifecycle.pendingEffects = [];
  vi.mocked(bindDesktopAttachments).mockReset();
  installBrowserGlobals();
});

afterEach(async () => {
  unmount();
  await flushMicrotasks();
  vi.unstubAllGlobals();
});

describe("useDesktopAttachments", () => {
  test("binds clipped form geometry and refreshes native bounds on resize and handle changes", async () => {
    const binding = createBinding();
    vi.mocked(bindDesktopAttachments).mockResolvedValue(binding);
    const initialRect = { left: -10, top: 10, right: 1100, bottom: 900 };
    const { region, setRect } = createRegion(initialRect);
    const receive = vi.fn();
    const fail = vi.fn();
    const handles = ["file-handle-fixture"];

    render("session-fixture", region, handles, receive, fail);
    await flushMicrotasks();

    const initialBounds = { x: 0, y: 10, width: 1000, height: 790 };
    expect(bindDesktopAttachments).toHaveBeenCalledWith("session-fixture", initialBounds, handles, expect.any(Function));
    expect(observers).toHaveLength(1);
    expect(observers[0].observe).toHaveBeenCalledExactlyOnceWith(region.current);
    expect(fakeWindow.addEventListener).toHaveBeenCalledWith("resize", expect.any(Function));
    expect(fakeWindow.addEventListener).toHaveBeenCalledWith("scroll", expect.any(Function), true);
    expect(binding.update).toHaveBeenCalledWith(initialBounds, handles);

    const nextRect = { left: 50, top: 40, right: 500, bottom: 300 };
    const nextBounds = { x: 50, y: 40, width: 450, height: 260 };
    setRect(nextRect);
    binding.update.mockClear();
    observers[0].trigger();
    await flushMicrotasks();
    expect(binding.update).toHaveBeenCalledWith(nextBounds, handles);

    binding.update.mockClear();
    fire(windowListeners, "resize");
    await flushMicrotasks();
    expect(binding.update).toHaveBeenCalledWith(nextBounds, handles);

    binding.update.mockClear();
    const nextHandles = ["file-handle-fixture", "project-handle-fixture"];
    render("session-fixture", region, nextHandles, receive, fail);
    await flushMicrotasks();
    expect(binding.update).toHaveBeenCalledWith(nextBounds, nextHandles);
  });

  test("intercepts only native image MIME paste and leaves text and HTML to the browser", async () => {
    const binding = createBinding();
    vi.mocked(bindDesktopAttachments).mockResolvedValue(binding);
    const { region } = createRegion({ left: 0, top: 0, right: 300, bottom: 120 });
    const receive = vi.fn();
    const fail = vi.fn();
    const attachments = render("session-fixture", region, [], receive, fail);
    await flushMicrotasks();

    const text = clipboardEvent([{ kind: "string", type: "text/plain" }]);
    attachments.paste(text);
    const html = clipboardEvent([{ kind: "string", type: "text/html" }]);
    attachments.paste(html);
    expect(text.preventDefault).not.toHaveBeenCalled();
    expect(html.preventDefault).not.toHaveBeenCalled();
    expect(binding.paste).not.toHaveBeenCalled();

    const image = clipboardEvent([{ kind: "file", type: "image/png" }]);
    attachments.paste(image);
    await flushMicrotasks();
    expect(image.preventDefault).toHaveBeenCalledOnce();
    expect(binding.paste).toHaveBeenCalledExactlyOnceWith();
  });

  test("closes on hide and unmount, and drops callbacks from both bindings", async () => {
    const hiddenBinding = createBinding();
    const visibleBinding = createBinding();
    vi.mocked(bindDesktopAttachments).mockResolvedValueOnce(hiddenBinding).mockResolvedValueOnce(visibleBinding);
    const { region } = createRegion({ left: 10, top: 20, right: 400, bottom: 180 });
    const receive = vi.fn();
    const fail = vi.fn();
    render("session-fixture", region, [], receive, fail);
    await flushMicrotasks();
    const hiddenCallback = vi.mocked(bindDesktopAttachments).mock.calls[0][3];

    fakeDocument.hidden = true;
    fire(documentListeners, "visibilitychange");
    await flushMicrotasks();
    expect(hiddenBinding.close).toHaveBeenCalledOnce();
    hiddenCallback(attachmentEvent());
    hiddenCallback(attachmentEvent({ error: "stale hidden error", items: [] }));
    expect(receive).not.toHaveBeenCalled();
    expect(fail).not.toHaveBeenCalled();

    fakeDocument.hidden = false;
    fire(documentListeners, "visibilitychange");
    await flushMicrotasks();
    expect(bindDesktopAttachments).toHaveBeenCalledTimes(2);
    const visibleCallback = vi.mocked(bindDesktopAttachments).mock.calls[1][3];
    hiddenCallback(attachmentEvent());
    visibleCallback(attachmentEvent());
    expect(receive).toHaveBeenCalledExactlyOnceWith(attachmentEvent().items);

    unmount();
    await flushMicrotasks();
    expect(visibleBinding.close).toHaveBeenCalledOnce();
    expect(observers[0].disconnect).toHaveBeenCalledOnce();
    expect(fakeWindow.removeEventListener).toHaveBeenCalledWith("resize", expect.any(Function));
    expect(fakeWindow.removeEventListener).toHaveBeenCalledWith("scroll", expect.any(Function), true);
    expect(fakeDocument.removeEventListener).toHaveBeenCalledWith("visibilitychange", expect.any(Function));
    visibleCallback(attachmentEvent({ error: "stale unmounted error", items: [] }));
    expect(fail).not.toHaveBeenCalled();
  });

  test("closes a binding that resolves after its session is replaced", async () => {
    const delayed = deferred<Binding>();
    const staleBinding = createBinding();
    const currentBinding = createBinding();
    vi.mocked(bindDesktopAttachments).mockReturnValueOnce(delayed.promise).mockResolvedValueOnce(currentBinding);
    const { region } = createRegion({ left: 0, top: 0, right: 240, bottom: 100 });
    const staleReceive = vi.fn();
    const currentReceive = vi.fn();
    const staleFail = vi.fn();
    const currentFail = vi.fn();

    render("old-session-fixture", region, [], staleReceive, staleFail);
    const staleCallback = vi.mocked(bindDesktopAttachments).mock.calls[0][3];
    render("new-session-fixture", region, [], currentReceive, currentFail);
    await flushMicrotasks();
    delayed.resolve(staleBinding);
    await flushMicrotasks();

    expect(bindDesktopAttachments).toHaveBeenNthCalledWith(2, "new-session-fixture", expect.any(Object), [], expect.any(Function));
    expect(staleBinding.close).toHaveBeenCalledOnce();
    staleCallback(attachmentEvent());
    expect(staleReceive).not.toHaveBeenCalled();
    expect(currentReceive).not.toHaveBeenCalled();
    expect(staleFail).not.toHaveBeenCalled();
    expect(currentFail).not.toHaveBeenCalled();
  });

  test("suppresses an old session bind error after a new session starts", async () => {
    const delayed = deferred<Binding>();
    const currentBinding = createBinding();
    vi.mocked(bindDesktopAttachments).mockReturnValueOnce(delayed.promise).mockResolvedValueOnce(currentBinding);
    const { region } = createRegion({ left: 0, top: 0, right: 240, bottom: 100 });
    const oldFail = vi.fn();
    const newFail = vi.fn();

    render("old-session-fixture", region, [], vi.fn(), oldFail);
    render("new-session-fixture", region, [], vi.fn(), newFail);
    await flushMicrotasks();
    delayed.reject(new Error("old session fixture failed"));
    await flushMicrotasks();

    expect(oldFail).not.toHaveBeenCalled();
    expect(newFail).not.toHaveBeenCalled();
  });
});
