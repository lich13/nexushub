import { isValidElement, type ReactElement, type ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { DesktopView } from "../../lib/api/desktop";
import { DesktopComposer } from "./DesktopComposer";

const hooks = vi.hoisted(() => ({
  index: 0,
  cells: new Map<number, unknown>(),
  desktop: null as any,
  catalog: null as any,
  catalogCalls: [] as unknown[][]
}));

vi.mock("react", async importOriginal => ({
  ...await importOriginal<typeof import("react")>(),
  useEffect: () => undefined,
  useState: (initial: unknown) => {
    const index = hooks.index++;
    if (!hooks.cells.has(index)) hooks.cells.set(index, typeof initial === "function" ? (initial as () => unknown)() : initial);
    return [hooks.cells.get(index), (value: unknown) => {
      const next = typeof value === "function" ? (value as (current: unknown) => unknown)(hooks.cells.get(index)) : value;
      hooks.cells.set(index, next);
    }];
  },
  useRef: (initial: unknown) => {
    const index = hooks.index++;
    if (!hooks.cells.has(index)) hooks.cells.set(index, { current: initial });
    return hooks.cells.get(index);
  }
}));

vi.mock("../../hooks/useCodexDesktop", () => ({ useCodexDesktop: () => hooks.desktop }));
vi.mock("../../hooks/useDesktopReferenceCatalog", () => ({
  useDesktopReferenceCatalog: (sessionId: string | undefined, kind: string, query: string) => {
    hooks.catalogCalls.push([sessionId, kind, query]);
    return hooks.catalog;
  }
}));
vi.mock("../../lib/query/desktop", () => ({
  chooseDesktopItem: vi.fn(async () => ({ item: null })),
  createDesktopThread: vi.fn(async () => ({ threadId: "thread-fixture" })),
  desktopCatalog: vi.fn(async () => ({ items: [], nextCursor: null }))
}));

type Element = ReactElement<Record<string, any>>;

function view(): DesktopView {
  return {
    sessionId: "session-fixture", threadId: "thread-fixture", revision: 1,
    connected: true, writable: true, runtime: "codex", activeTurnId: null,
    model: null, reasoningEffort: null, mode: null, requests: [], queue: [], error: null
  };
}

function elements(node: ReactNode): Element[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!isValidElement<Record<string, any>>(node)) return [];
  return [node, ...elements(node.props.children)];
}

function render() {
  hooks.index = 0;
  return DesktopComposer({ threadId: "thread-fixture" }) as unknown as ReactElement;
}

function required(tree: ReactNode, predicate: (element: Element) => boolean): Element {
  const element = elements(tree).find(predicate);
  if (!element) throw new Error("Expected desktop composer fixture element is missing");
  return element;
}

function textarea(tree: ReactNode) {
  return required(tree, element => element.type === "textarea");
}

function typeText(text: string) {
  const tree = render();
  textarea(tree).props.onChange({ target: { value: text, selectionStart: text.length, selectionEnd: text.length } });
  return render();
}

function keyEvent(key: string, composing = false) {
  return {
    key, nativeEvent: { isComposing: composing }, shiftKey: false,
    metaKey: false, ctrlKey: false, preventDefault: vi.fn()
  };
}

async function flushMicrotasks() {
  for (let index = 0; index < 8; index += 1) await Promise.resolve();
}

beforeEach(() => {
  hooks.index = 0;
  hooks.cells.clear();
  hooks.catalogCalls = [];
  hooks.desktop = {
    view: view(), busy: false, error: "",
    act: vi.fn(async () => undefined),
    open: vi.fn(async () => undefined), connect: vi.fn()
  };
  hooks.catalog = {
    items: [
      { handle: "skill-alpha-fixture", kind: "skill", name: "alpha", description: "", reasoningEfforts: [], defaultReasoningEffort: null },
      { handle: "skill-diagram-fixture", kind: "skill", name: "diagram", description: "", reasoningEfforts: [], defaultReasoningEffort: null }
    ],
    busy: false, errors: [], hasMore: false, more: vi.fn(), retry: vi.fn()
  };
  let id = 0;
  vi.stubGlobal("crypto", { randomUUID: () => `message-fixture-${++id}` });
});

afterEach(() => { vi.unstubAllGlobals(); });

describe("desktop composer reference interactions", () => {
  test.each([
    { text: "$dia", kind: "skills", query: "dia" },
    { text: "please @rev", kind: "mentions", query: "rev" }
  ])("opens and dismisses the $ / @ suggestions for $text", ({ text, kind, query }) => {
    const tree = typeText(text);
    expect(textarea(tree).props["aria-expanded"]).toBe(true);
    expect(hooks.catalogCalls.at(-1)).toEqual(["session-fixture", kind, query]);

    const escape = keyEvent("Escape");
    textarea(tree).props.onKeyDown(escape);
    expect(escape.preventDefault).toHaveBeenCalledOnce();
    expect(textarea(render()).props["aria-expanded"]).toBe(false);
  });

  test("opens and closes the reference picker from the toolbar", () => {
    let tree = render();
    required(tree, element => element.type === "button" && element.props.children === "引用").props.onClick();
    tree = render();
    expect(required(tree, element => element.type === "div" && element.props.className === "desktop-reference-picker")).toBeTruthy();
    expect(hooks.catalogCalls.at(-1)).toEqual(["session-fixture", "skills", ""]);

    required(tree, element => element.type === "button" && element.props.title === "关闭引用").props.onClick();
    tree = render();
    expect(elements(tree).some(element => element.type === "div" && element.props.className === "desktop-reference-picker")).toBe(false);
  });

  test("supports arrow and Enter selection while leaving IME Enter untouched", () => {
    let tree = typeText("$dia");
    const composing = keyEvent("Enter", true);
    textarea(tree).props.onKeyDown(composing);
    expect(composing.preventDefault).not.toHaveBeenCalled();
    expect(hooks.desktop.act).not.toHaveBeenCalled();
    expect(textarea(render()).props.value).toBe("$dia");

    tree = render();
    const down = keyEvent("ArrowDown");
    textarea(tree).props.onKeyDown(down);
    expect(down.preventDefault).toHaveBeenCalledOnce();
    tree = render();
    expect(textarea(tree).props["aria-activedescendant"]).toBe("desktop-reference-option-1");

    const enter = keyEvent("Enter");
    textarea(tree).props.onKeyDown(enter);
    expect(enter.preventDefault).toHaveBeenCalledOnce();
    tree = render();
    expect(textarea(tree).props.value).toBe("$diagram ");
    expect(textarea(tree).props["aria-expanded"]).toBe(false);
    expect(elements(tree).some(element => element.type === "div" && element.props.className === "desktop-reference-chips")).toBe(true);
  });

  test("renders a reference chip and removes its text binding when dismissed", () => {
    let tree = typeText("$dia");
    textarea(tree).props.onKeyDown(keyEvent("ArrowDown"));
    tree = render();
    textarea(tree).props.onKeyDown(keyEvent("Enter"));
    tree = render();
    expect(required(tree, element => element.type === "button" && element.props["aria-label"] === "移除 diagram")).toBeTruthy();
    expect(textarea(tree).props.value).toBe("$diagram ");

    required(tree, element => element.type === "button" && element.props["aria-label"] === "移除 diagram").props.onClick();
    tree = render();
    expect(elements(tree).some(element => element.type === "div" && element.props.className === "desktop-reference-chips")).toBe(false);
    expect(textarea(tree).props.value).toBe(" ");
  });

  test("sends only opaque handles and UTF-8 reference spans, without catalog path metadata", async () => {
    hooks.catalog.items = [{
      handle: "skill-handle-fixture", kind: "skill", name: "工具", description: "", reasoningEfforts: [], defaultReasoningEffort: null,
      path: "example.invalid/private-fixture"
    }];
    let tree = typeText("猫😀 $工");
    textarea(tree).props.onKeyDown(keyEvent("Enter"));
    tree = render();
    expect(textarea(tree).props.value).toBe("猫😀 $工具 ");

    required(tree, element => element.type === "form").props.onSubmit({ preventDefault: vi.fn() });
    await flushMicrotasks();

    const [action] = hooks.desktop.act.mock.calls[0];
    expect(action).toEqual({
      type: "send", messageId: "message-fixture-1", text: "猫😀 $工具 ",
      references: ["skill-handle-fixture"],
      referenceSpans: [{ handle: "skill-handle-fixture", start: 8, end: 15 }]
    });
    expect(Object.keys(action).sort()).toEqual(["messageId", "referenceSpans", "references", "text", "type"]);
    expect(JSON.stringify(action)).not.toContain("example.invalid/private-fixture");
  });
});
