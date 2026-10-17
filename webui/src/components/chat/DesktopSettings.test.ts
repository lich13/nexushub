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
vi.mock("../../hooks/useDesktopAttachments", () => ({
  useDesktopAttachments: () => ({ paste: vi.fn(), release: vi.fn() })
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
type CatalogCall = { request: { sessionId: string; projectHandle: null; kind: string; query: string; cursor: null; forceRefetch?: boolean }; pending: Deferred<DesktopCatalog> };

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((accept, fail) => { resolve = accept; reject = fail; });
  return { promise, resolve, reject };
}

function view(sessionId: string, model: string | null, reasoningEffort: string | null = "high", mode: "default" | "plan" | null = "default"): DesktopView {
  return {
    sessionId, threadId: "thread-fixture", revision: 1, connected: true, writable: true,
    runtime: "codex", activeTurnId: null, model, reasoningEffort, mode,
    requests: [], queue: [], error: null
  };
}

function setDesktopView(sessionId: string, model: string | null, reasoningEffort: string | null = "high", mode: "default" | "plan" | null = "default") {
  hooks.desktop = {
    view: view(sessionId, model, reasoningEffort, mode), busy: false, error: "",
    act: vi.fn(async () => undefined), open: vi.fn(async () => undefined), connect: vi.fn()
  };
}

function catalogItem(handle: string, name: string, kind = "model", reasoningEfforts = ["low", "high"]): DesktopCatalogItem {
  return {
    handle, kind, name, description: `fixture ${kind}`,
    reasoningEfforts, defaultReasoningEffort: reasoningEfforts[0] ?? null
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

function renderedText(node: ReactNode): string {
  if (Array.isArray(node)) return node.map(renderedText).join("");
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (isValidElement<Record<string, any>>(node)) return renderedText(node.props.children);
  return "";
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

function retryButton(tree: ReactNode, catalogName: "模型" | "模式" = "模型"): Element {
  return required(tree, element => element.type === "button" && element.props.children === `重试${catalogName}目录`);
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

describe("desktop settings catalogs", () => {
  test("keeps model and mode catalogs independent across failure and retry", async () => {
    let tree = renderSettings();
    expect(catalogCalls()).toHaveLength(2);
    expect(catalogCalls().map(call => call.request)).toEqual([
      { sessionId: "session-fixture", projectHandle: null, kind: "models", query: "", cursor: null },
      { sessionId: "session-fixture", projectHandle: null, kind: "modes", query: "", cursor: null }
    ]);
    expectLoadingControlsDisabled(tree);
    expect(elements(tree).some(element => element.props.role === "alert")).toBe(false);

    catalogCalls()[0].pending.reject(new Error("模型列表暂不可用"));
    await flushMicrotasks();
    tree = renderSettings();
    expect(elements(tree).filter(element => element.props.role === "alert").map(element => renderedText(element.props.children))).toContain("模型：模型列表暂不可用");
    expect(retryButton(tree)).toBeTruthy();

    const defaultMode = catalogItem("default", "默认", "mode", []);
    const planMode = catalogItem("plan", "Plan", "mode", []);
    catalogCalls()[1].pending.resolve({ items: [defaultMode, planMode], nextCursor: null });
    await flushMicrotasks();
    tree = renderSettings();
    expect(elements(tree).filter(element => element.props.role === "alert").map(element => renderedText(element.props.children))).toContain("模型：模型列表暂不可用");

    retryButton(tree).props.onClick();
    tree = renderSettings();
    expect(catalogCalls()).toHaveLength(3);
    expect(catalogCalls()[2].request).toEqual({ sessionId: "session-fixture", projectHandle: null, kind: "models", query: "", cursor: null, forceRefetch: true });
    expectLoadingControlsDisabled(tree);
    expect(elements(tree).some(element => element.props.role === "alert")).toBe(false);

    const recovered = catalogItem("model-fixture", "Fixture Model");
    catalogCalls()[2].pending.resolve({ items: [recovered], nextCursor: null });
    await flushMicrotasks();
    tree = renderSettings();
    expect(select(tree, "模型").props).toMatchObject({ value: recovered.handle, disabled: false });
    expect(select(tree, "思考强度").props).toMatchObject({ value: "high", disabled: false });
    expect(select(tree, "协作模式").props).toMatchObject({ value: "default", disabled: false });
    expect(elements(tree).some(element => element.type === "option" && element.props.value === "plan")).toBe(true);
    expect(elements(tree).some(element => element.props.role === "alert")).toBe(false);
  });

  test("keeps the model selected when the mode catalog fails and force-refetches only modes", async () => {
    let tree = renderSettings();
    const model = catalogItem("model-fixture", "Fixture Model");
    catalogCalls()[0].pending.resolve({ items: [model], nextCursor: null });
    catalogCalls()[1].pending.reject(new Error("模式列表暂不可用"));
    await flushMicrotasks();
    tree = renderSettings();

    expect(select(tree, "模型").props).toMatchObject({ value: model.handle, disabled: false });
    expect(select(tree, "协作模式").props.disabled).toBe(true);
    expect(elements(tree).filter(element => element.props.role === "alert").map(element => renderedText(element.props.children))).toContain("模式：模式列表暂不可用");

    retryButton(tree, "模式").props.onClick();
    tree = renderSettings();
    expect(catalogCalls()).toHaveLength(3);
    expect(catalogCalls()[2].request).toEqual({ sessionId: "session-fixture", projectHandle: null, kind: "modes", query: "", cursor: null, forceRefetch: true });
    expect(select(tree, "模型").props.value).toBe(model.handle);
    expect(select(tree, "模型").props.disabled).toBe(false);

    const plan = catalogItem("plan", "Plan", "mode", []);
    catalogCalls()[2].pending.resolve({ items: [catalogItem("default", "默认", "mode", []), plan], nextCursor: null });
    await flushMicrotasks();
    tree = renderSettings();
    expect(select(tree, "协作模式").props.disabled).toBe(false);
    expect(elements(tree).some(element => element.props.role === "alert")).toBe(false);
  });

  test("offers a catalog-supported max effort and preserves or clears it on mode changes", async () => {
    setDesktopView("session-fixture", "model-fixture", "high", "default");
    let tree = renderSettings();
    const model = catalogItem("model-fixture", "Fixture Model", "model", ["low", "high", "max", "ultra", "custom"]);
    catalogCalls()[0].pending.resolve({ items: [model], nextCursor: null });
    catalogCalls()[1].pending.resolve({ items: [catalogItem("default", "默认", "mode", []), catalogItem("plan", "Plan", "mode", [])], nextCursor: null });
    await flushMicrotasks();
    tree = renderSettings();

    const effort = select(tree, "思考强度");
    expect(elements(effort).some(element => element.type === "option" && element.props.value === "max")).toBe(true);
    expect(elements(effort).some(element => element.type === "option" && element.props.value === "ultra")).toBe(true);
    expect(elements(effort).some(element => element.type === "option" && element.props.value === "custom")).toBe(true);
    effort.props.onChange({ target: { value: "max" } });
    expect(hooks.desktop.act).toHaveBeenCalledWith({ type: "settings", model: "model-fixture", reasoningEffort: "max", mode: "default" });

    setDesktopView("session-fixture", "model-fixture", "max", "default");
    tree = renderSettings();
    select(tree, "协作模式").props.onChange({ target: { value: "plan" } });
    expect(hooks.desktop.act).toHaveBeenCalledWith({ type: "settings", model: "model-fixture", reasoningEffort: "max", mode: "plan" });

    setDesktopView("session-fixture", "model-fixture", "xhigh", "default");
    tree = renderSettings();
    select(tree, "协作模式").props.onChange({ target: { value: "plan" } });
    expect(hooks.desktop.act).toHaveBeenCalledWith({ type: "settings", model: "model-fixture", reasoningEffort: null, mode: "plan" });
    tree = renderSettings();
    expect(elements(tree).some(element => element.props.role === "status" && element.props.children === "当前思考强度不可用，将继承原生默认")).toBe(true);
  });

  test("shows a failed setting action and keeps the controlled view until the next view arrives", async () => {
    let tree = renderSettings();
    catalogCalls()[0].pending.resolve({ items: [catalogItem("model-fixture", "Fixture Model")], nextCursor: null });
    catalogCalls()[1].pending.resolve({ items: [catalogItem("default", "默认", "mode", []), catalogItem("plan", "Plan", "mode", [])], nextCursor: null });
    await flushMicrotasks();
    tree = renderSettings();
    hooks.desktop.act = vi.fn(async () => { throw new Error("设置未应用"); });
    tree = renderSettings();

    select(tree, "协作模式").props.onChange({ target: { value: "plan" } });
    await flushMicrotasks();
    tree = renderSettings();
    expect(hooks.desktop.act).toHaveBeenCalledWith({ type: "settings", model: "model-fixture", reasoningEffort: "high", mode: "plan" });

    expect(elements(tree).filter(element => element.props.role === "alert").map(element => renderedText(element.props.children))).toContain("设置未应用");
    expect(select(tree, "模型").props.value).toBe("model-fixture");
    expect(select(tree, "思考强度").props.value).toBe("high");
    expect(select(tree, "协作模式").props.value).toBe("default");
  });

  test("clears a setting error after success and on a session change", async () => {
    let tree = renderSettings();
    catalogCalls()[0].pending.resolve({ items: [catalogItem("model-fixture", "Fixture Model")], nextCursor: null });
    catalogCalls()[1].pending.resolve({ items: [catalogItem("default", "默认", "mode", []), catalogItem("plan", "Plan", "mode", [])], nextCursor: null });
    await flushMicrotasks();
    tree = renderSettings();
    hooks.desktop.act = vi.fn(async () => { throw new Error("设置未应用"); });
    tree = renderSettings();
    select(tree, "协作模式").props.onChange({ target: { value: "plan" } });
    await flushMicrotasks();
    tree = renderSettings();
    expect(elements(tree).some(element => element.props.role === "alert" && renderedText(element.props.children) === "设置未应用")).toBe(true);

    hooks.desktop.act = vi.fn(async () => undefined);
    tree = renderSettings();
    select(tree, "模型").props.onChange({ target: { value: "model-fixture" } });
    await flushMicrotasks();
    tree = renderSettings();
    expect(elements(tree).some(element => element.props.role === "alert" && renderedText(element.props.children) === "设置未应用")).toBe(false);

    hooks.desktop.act = vi.fn(async () => { throw new Error("再次失败"); });
    tree = renderSettings();
    select(tree, "协作模式").props.onChange({ target: { value: "plan" } });
    await flushMicrotasks();
    tree = renderSettings();
    expect(elements(tree).some(element => element.props.role === "alert" && renderedText(element.props.children) === "再次失败")).toBe(true);

    setDesktopView("session-next", "model-next");
    tree = renderSettings();
    expect(elements(tree).some(element => element.props.role === "alert" && renderedText(element.props.children) === "再次失败")).toBe(false);
    expect(catalogCalls().slice(-2).map(call => call.request.kind)).toEqual(["models", "modes"]);
  });

  test("discards late catalog results and errors after session changes", async () => {
    let tree = renderSettings();
    const first = catalogCalls()[0];

    setDesktopView("session-next", "model-next");
    tree = renderSettings();
    const second = catalogCalls()[2];
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
    const third = catalogCalls()[4];
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
    catalogCalls()[1].pending.resolve({ items: [catalogItem("default", "默认", "mode", [])], nextCursor: null });
    await flushMicrotasks();
    tree = renderSettings();
    expect(select(tree, "模型").props.value).toBe(previous.handle);

    setDesktopView("session-new", "model-new");
    tree = renderSettings();
    expectLoadingControlsDisabled(tree);
    expectNoModelOption(tree, previous.handle);

    catalogCalls()[2].pending.reject(new Error("模型列表暂不可用"));
    await flushMicrotasks();
    tree = renderSettings();
    expect(retryButton(tree)).toBeTruthy();
    expectNoModelOption(tree, previous.handle);

    retryButton(tree).props.onClick();
    tree = renderSettings();
    expect(catalogCalls()).toHaveLength(5);
    expect(catalogCalls()[4].request.sessionId).toBe("session-new");
    expectLoadingControlsDisabled(tree);
    expectNoModelOption(tree, previous.handle);

    const next = catalogItem("model-new", "New Model");
    catalogCalls()[4].pending.resolve({ items: [next], nextCursor: null });
    await flushMicrotasks();
    tree = renderSettings();
    expect(select(tree, "模型").props.value).toBe(next.handle);
    expectNoModelOption(tree, previous.handle);
  });
});
