import { isValidElement, type ReactElement, type ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { DesktopView } from "../lib/api/desktop";
import { DesktopComposer } from "../components/chat/DesktopComposer";

const hooks = vi.hoisted(() => ({
  index: 0,
  cells: new Map<number, unknown>(),
  desktop: null as any
}));

vi.mock("react", async original => ({
  ...await original<typeof import("react")>(),
  useState: (initial: unknown) => {
    const index = hooks.index++;
    if (!hooks.cells.has(index)) hooks.cells.set(index, typeof initial === "function" ? initial() : initial);
    return [hooks.cells.get(index), (value: unknown) => {
      const next = typeof value === "function" ? value(hooks.cells.get(index)) : value;
      hooks.cells.set(index, next);
    }];
  },
  useRef: (initial: unknown) => {
    const index = hooks.index++;
    if (!hooks.cells.has(index)) hooks.cells.set(index, { current: initial });
    return hooks.cells.get(index);
  }
}));

vi.mock("../hooks/useCodexDesktop", () => ({ useCodexDesktop: () => hooks.desktop }));
vi.mock("../lib/api/desktop", () => ({
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

async function flushSend() {
  await Promise.resolve();
}

beforeEach(() => {
  hooks.index = 0;
  hooks.cells.clear();
  hooks.desktop = {
    view: view(), busy: false, error: "",
    act: vi.fn(async () => { throw new Error("fixture transport failure"); }),
    open: vi.fn(async () => undefined), connect: vi.fn()
  };
  let id = 0;
  vi.stubGlobal("crypto", { randomUUID: () => `message-fixture-${++id}` });
});
afterEach(() => { vi.unstubAllGlobals(); });

describe("Codex desktop action retry identity", () => {
  test("reuses a message ID for an identical retry and changes it with the payload", async () => {
    let tree = render();
    required(tree, element => element.type === "textarea").props.onChange({ target: { value: "fixture request" } });

    tree = render();
    required(tree, element => element.type === "form").props.onSubmit({ preventDefault: vi.fn() });
    await flushSend();

    tree = render();
    required(tree, element => element.type === "form").props.onSubmit({ preventDefault: vi.fn() });
    await flushSend();

    tree = render();
    required(tree, element => element.type === "textarea").props.onChange({ target: { value: "changed request" } });
    tree = render();
    required(tree, element => element.type === "form").props.onSubmit({ preventDefault: vi.fn() });
    await flushSend();

    const actions = hooks.desktop.act.mock.calls.map(([action]: [{ type: string; messageId: string; text: string }]) => action);
    expect(actions.map(action => action.messageId)).toEqual([
      "message-fixture-1", "message-fixture-1", "message-fixture-2"
    ]);
    expect(actions.map(action => action.text)).toEqual([
      "fixture request", "fixture request", "changed request"
    ]);
  });
});
