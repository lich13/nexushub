import { isValidElement, type ReactElement, type ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { chooseDesktopItem, createDesktopThread } from "../../lib/query/desktop";
import { requestDesktopOpenIntent } from "../../lib/desktopIntent";
import { NewDesktopThread } from "./DesktopComposer";

const hooks = vi.hoisted(() => ({ index: 0, cells: new Map<number, unknown>() }));

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

vi.mock("../../lib/query/desktop", () => ({
  chooseDesktopItem: vi.fn(),
  createDesktopThread: vi.fn()
}));
vi.mock("../../lib/desktopIntent", () => ({ requestDesktopOpenIntent: vi.fn() }));

type Element = ReactElement<Record<string, any>>;

function elements(node: ReactNode): Element[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!isValidElement<Record<string, any>>(node)) return [];
  return [node, ...elements(node.props.children)];
}

function render(onCreated: (id: string) => void | Promise<void>) {
  hooks.index = 0;
  return NewDesktopThread({ onCreated });
}

function button(tree: ReactNode): Element {
  const found = elements(tree).find(element => element.type === "button");
  if (!found) throw new Error("New thread button fixture is missing");
  return found;
}

async function flushMicrotasks() {
  for (let index = 0; index < 8; index += 1) await Promise.resolve();
}

beforeEach(() => {
  hooks.index = 0;
  hooks.cells.clear();
  vi.mocked(chooseDesktopItem).mockReset().mockResolvedValue({ item: { handle: "project-fixture", name: "Fixture", kind: "project" } });
  vi.mocked(createDesktopThread).mockReset().mockResolvedValue({ threadId: "thread-created" });
  vi.mocked(requestDesktopOpenIntent).mockReset();
});

afterEach(() => vi.clearAllMocks());

describe("new desktop thread creation", () => {
  test("reuses a created thread when page navigation fails and is retried", async () => {
    const onCreated = vi.fn().mockRejectedValueOnce(new Error("fixture navigation failure")).mockResolvedValueOnce(undefined);

    button(render(onCreated)).props.onClick();
    await flushMicrotasks();
    expect(createDesktopThread).toHaveBeenCalledExactlyOnceWith("project-fixture");
    expect(onCreated).toHaveBeenCalledExactlyOnceWith("thread-created");

    button(render(onCreated)).props.onClick();
    await flushMicrotasks();

    expect(chooseDesktopItem).toHaveBeenCalledOnce();
    expect(createDesktopThread).toHaveBeenCalledExactlyOnceWith("project-fixture");
    expect(onCreated).toHaveBeenCalledTimes(2);
    expect(requestDesktopOpenIntent).toHaveBeenCalledTimes(2);
    expect(requestDesktopOpenIntent).toHaveBeenLastCalledWith("thread-created");
  });

  test("does not blindly retry an outcome-unknown create", async () => {
    vi.mocked(createDesktopThread).mockRejectedValueOnce(new Error("结果未确认"));
    const onCreated = vi.fn();

    button(render(onCreated)).props.onClick();
    await flushMicrotasks();
    const tree = render(onCreated);

    expect(createDesktopThread).toHaveBeenCalledOnce();
    expect(elements(tree).some(element => element.type === "button" && element.props.disabled)).toBe(true);
    expect(elements(tree).some(element => element.props.role === "alert" && String(element.props.children).includes("结果未确认"))).toBe(true);
    expect(onCreated).not.toHaveBeenCalled();
    expect(requestDesktopOpenIntent).not.toHaveBeenCalled();
  });
});
