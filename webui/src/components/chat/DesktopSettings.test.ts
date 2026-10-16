import { isValidElement, type ReactElement, type ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { DesktopCatalog, DesktopCatalogItem, DesktopView } from "../../lib/api/desktop";
import { desktopCatalog } from "../../lib/query/desktop";
import { DesktopComposer } from "./DesktopComposer";

const hooks = vi.hoisted(() => ({
  scope: "composer",
  index: 0,
  cells: new Map<string, unknown[]>(),
  refs: new Map<string, unknown[]>(),
  effects: new Map<string, any[]>(),
  pendingEffects: [] as { scope: string; index: number; create: () => void | (() => void); deps?: readonly unknown[] }[],
  desktop: null as any,
  catalogCalls: [] as { request: unknown; pending: any }[]
}));

vi.mock("react", async importOriginal => {
  const actual = await importOriginal<typeof import("react")>();
  return {
    ...actual,
    useState: (initial: unknown) => {
      const scope = hooks.scope;
      const index = hooks.index++;
      let cells = hooks.cells.get(scope);
      if (!cells) {
        cells = [];
        hooks.cells.set(scope, cells);
      }
      if (!(index in cells)) cells[index] = typeof initial === "function" ? (initial as () => unknown)() : initial;
      return [cells[index], (next: unknown) => {
        const value = typeof next === "function" ? (next as (current: unknown) => unknown)(cells![index]) : next;
        cells![index] = value;
      }];
    },
    useRef: (initial: unknown) => {
      const scope = hooks.scope;
      const index = hooks.index++;
      let refs = hooks.refs.get(scope);
      if (!refs) {
        refs = [];
        hooks.refs.set(scope, refs);
      }
      if (!(index in refs)) refs[index] = { current: initial };
      return refs[index];
    },
    useEffect: (create: () => void | (() => void), deps?: readonly unknown[]) => {
      const scope = hooks.scope;
      const index = hooks.index++;
      const previous = hooks.effects.get(scope)?.[index];
      const changed = !previous || !deps || !previous.deps || deps.length !== previous.deps.length
        || deps.some((value, item) => !Object.is(value, previous.deps[item]));
      if (changed) hooks.pendingEffects.push({ scope, index, create, deps });
    }
  };
});

vi.mock("../../hooks/useCodexDesktop", () => ({ useCodexDesktop: () => hooks.desktop }));
vi.mock("../../hooks/useDesktopReferenceCatalog", () => ({
  useDesktopReferenceCatalog: () => ({ items: [], busy: false, errors: [], hasMore: false, more: () => undefined, retry: () => undefined })
}));
vi.mock("../../lib/query/desktop", () => ({
  chooseDesktopItem: vi.fn(),
  createDesktopThread: vi.fn(),
  desktopCatalog: vi.fn()
}));
vi.mock("./DesktopRequests", () => ({ DesktopRequests: () => null }));
vi.mock("../common/ConfirmDialog", () => ({ ConfirmDialog: () => null }));

type Element = ReactElement<Record<string, any>>;
type Deferred<T> = { promise: Promise<T>; resolve: (value: T) => void; reject: (reason?: unknown) => void };
type CatalogCall = { request: { sessionId: string; projectHandle: null; kind: string; query: string; cursor: null }; pending: Deferred<DesktopCatalog> };

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((accept, fail) => { resolve = accept; reject = fail; });
  return { promise, resolve, reject };
}

function view(sessionId: string, model: string | null): DesktopView {
  return {
    sessionId, threadId: "thread-fixture", revision: 1, connected: true, writable: true,
    runtime: "codex", activeTurnId: null, model, reasoningEffort: "high", mode: "default",
    requests: [], queue: [], error: null
  };
}

function setDesktopView(sessionId: string, model: string | null) {
  hooks.desktop = {
    view: view(sessionId, model), busy: false, error: "",
    act: vi.fn(async () => undefined), open: vi.fn(async () => undefined), connect: vi.fn()
  };
}

function catalogItem(handle: string, name: string): DesktopCatalogItem {
  return {
    handle, kind: "model", name, description: "fixture model",
    reasoningEfforts: ["low", "high"], defaultReasoningEffort: "low"
  };
}

function catalogCalls(): CatalogCall[] {
  return hooks.catalogCalls as CatalogCall[];
}

function elements(node: ReactNode): Element[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!isValidElement<Record<string, any>>(node)) return [];
  return [node, ...elements(node.props.children)];
}

function required(tree: ReactNode, predicate: (element: Element) => boolean): Element {
  const element = elements(tree).find(predicate);
  if (!element) throw new Error("Expected desktop settings fixture element is missing");
  return element;
}

function select(tree: ReactNode, label: string): Element {
  return required(tree, element => element.type === "select" && element.props["aria-label"] === label);
}

function invokeSettings(): ReactNode {
  hooks.scope = "composer";
  hooks.index = 0;
  const composer = DesktopComposer({ threadId: "thread-fixture" }) as unknown as ReactElement;
  const element = required(composer, item => typeof item.type === "function" && item.type.name === "DesktopSettings");
  hooks.scope = "settings";
  hooks.index = 0;
  return (element.type as (props: Record<string, unknown>) => ReactNode)(element.props);
}

function renderSettings(): ReactNode {
  let tree = invokeSettings();
  for (let pass = 0; pass < 6; pass += 1) {
    const pending = hooks.pendingEffects.splice(0);
    if (!pending.length) return tree;
    for (const effect of pending) {
      const effects = hooks.effects.get(effect.scope) ?? [];
      effects[effect.index]?.cleanup?.();
      hooks.scope = effect.scope;
      const cleanup = effect.create();
      effects[effect.index] = {
        deps: effect.deps ? [...effect.deps] : undefined,
        cleanup: typeof cleanup === "function" ? cleanup : undefined
      };
      hooks.effects.set(effect.scope, effects);
    }
    tree = invokeSettings();
  }
  throw new Error("desktop settings fixture did not settle");
}

function expectLoadingControlsDisabled(tree: ReactNode) {
  expect(select(tree, "模型").props.disabled).toBe(true);
  expect(select(tree, "协作模式").props.disabled).toBe(true);
  const effort = elements(tree).find(element => element.type === "select" && element.props["aria-label"] === "思考强度");
  if (effort) expect(effort.props.disabled).toBe(true);
}

function expectNoModelOption(tree: ReactNode, handle: string) {
  expect(elements(tree).some(element => element.type === "option" && element.props.value === handle)).toBe(false);
}

function retryButton(tree: ReactNode): Element {
  return required(tree, element => element.type === "button" && element.props.children === "重试模型目录");
}

async function flushMicrotasks() {
  for (let index = 0; index < 8; index += 1) await Promise.resolve();
}

function cleanup() {
  const effects = hooks.effects.get("settings") ?? [];
  for (const effect of [...effects].reverse()) effect?.cleanup?.();
  hooks.effects.clear();
  hooks.pendingEffects = [];
}

beforeEach(() => {
  hooks.scope = "composer";
  hooks.index = 0;
  hooks.cells.clear();
  hooks.refs.clear();
  hooks.effects.clear();
  hooks.pendingEffects = [];
  hooks.catalogCalls = [];
  setDesktopView("session-fixture", "model-fixture");
  vi.mocked(desktopCatalog).mockReset().mockImplementation(request => {
    const pending = deferred<DesktopCatalog>();
    hooks.catalogCalls.push({ request, pending });
    return pending.promise;
  });
});

afterEach(() => cleanup());

describe("desktop settings model catalog", () => {
  test("shows a fixed timeout error and recovers after manual retry", async () => {
    let tree = renderSettings();
    expect(catalogCalls()).toHaveLength(1);
    expect(catalogCalls()[0].request).toEqual({ sessionId: "session-fixture", projectHandle: null, kind: "models", query: "", cursor: null });
    expectLoadingControlsDisabled(tree);
    expect(elements(tree).some(element => element.props.role === "alert")).toBe(false);

    catalogCalls()[0].pending.reject(new Error("模型列表暂不可用"));
    await flushMicrotasks();
    tree = renderSettings();
    expect(required(tree, element => element.props.role === "alert").props.children).toBe("模型列表暂不可用");
    expect(retryButton(tree)).toBeTruthy();

    retryButton(tree).props.onClick();
    tree = renderSettings();
    expect(catalogCalls()).toHaveLength(2);
    expect(catalogCalls()[1].request).toEqual(catalogCalls()[0].request);
    expectLoadingControlsDisabled(tree);
    expect(elements(tree).some(element => element.props.role === "alert")).toBe(false);

    const recovered = catalogItem("model-fixture", "Fixture Model");
    catalogCalls()[1].pending.resolve({ items: [recovered], nextCursor: null });
    await flushMicrotasks();
    tree = renderSettings();
    expect(select(tree, "模型").props).toMatchObject({ value: recovered.handle, disabled: false });
    expect(select(tree, "思考强度").props).toMatchObject({ value: "high", disabled: false });
    expect(select(tree, "协作模式").props).toMatchObject({ value: "default", disabled: false });
    expect(elements(tree).some(element => element.props.role === "alert")).toBe(false);
  });

  test("discards late catalog results and errors after session changes", async () => {
    let tree = renderSettings();
    const first = catalogCalls()[0];

    setDesktopView("session-next", "model-next");
    tree = renderSettings();
    const second = catalogCalls()[1];
    expect(second.request.sessionId).toBe("session-next");
    expectLoadingControlsDisabled(tree);

    first.pending.resolve({ items: [catalogItem("model-old", "Old Model")], nextCursor: null });
    await flushMicrotasks();
    tree = renderSettings();
    expectLoadingControlsDisabled(tree);
    expectNoModelOption(tree, "model-old");
    expect(elements(tree).some(element => element.props.role === "alert")).toBe(false);

    setDesktopView("session-current", "model-current");
    tree = renderSettings();
    const third = catalogCalls()[2];
    expect(third.request.sessionId).toBe("session-current");
    expectLoadingControlsDisabled(tree);

    second.pending.reject(new Error("旧会话目录错误"));
    await flushMicrotasks();
    tree = renderSettings();
    expectLoadingControlsDisabled(tree);
    expectNoModelOption(tree, "model-old");
    expect(elements(tree).some(element => element.props.role === "alert")).toBe(false);

    const current = catalogItem("model-current", "Current Model");
    third.pending.resolve({ items: [current], nextCursor: null });
    await flushMicrotasks();
    tree = renderSettings();
    expect(select(tree, "模型").props.value).toBe(current.handle);
    expectNoModelOption(tree, "model-old");
  });

  test("clears the previous session model while loading and after a retry", async () => {
    setDesktopView("session-fixture", "model-previous");
    let tree = renderSettings();
    const previous = catalogItem("model-previous", "Previous Model");
    catalogCalls()[0].pending.resolve({ items: [previous], nextCursor: null });
    await flushMicrotasks();
    tree = renderSettings();
    expect(select(tree, "模型").props.value).toBe(previous.handle);

    setDesktopView("session-new", "model-new");
    tree = renderSettings();
    expectLoadingControlsDisabled(tree);
    expectNoModelOption(tree, previous.handle);

    catalogCalls()[1].pending.reject(new Error("模型列表暂不可用"));
    await flushMicrotasks();
    tree = renderSettings();
    expect(retryButton(tree)).toBeTruthy();
    expectNoModelOption(tree, previous.handle);

    retryButton(tree).props.onClick();
    tree = renderSettings();
    expect(catalogCalls()).toHaveLength(3);
    expect(catalogCalls()[2].request.sessionId).toBe("session-new");
    expectLoadingControlsDisabled(tree);
    expectNoModelOption(tree, previous.handle);

    const next = catalogItem("model-new", "New Model");
    catalogCalls()[2].pending.resolve({ items: [next], nextCursor: null });
    await flushMicrotasks();
    tree = renderSettings();
    expect(select(tree, "模型").props.value).toBe(next.handle);
    expectNoModelOption(tree, previous.handle);
  });
});
