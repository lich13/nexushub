import { isValidElement, type ComponentProps, type ReactElement, type ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { locateTimelineTarget } from "./SessionSearch";
import { TimelineRail } from "./TimelineRail";
import type { TimelineEntry } from "../../lib/domain/timelineViewModel";

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
  const effect = (callback: () => void | (() => void), deps: readonly unknown[] | undefined) => {
    const index = hooks.cursor++;
    const cell = hooks.cells[index];
    if (!cell || changed(cell.deps, deps)) {
      hooks.cells[index] = { kind: "effect", deps, cleanup: cell?.cleanup };
      hooks.pending.push({ index, callback });
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
    useEffect: effect
  };
});

vi.mock("./SessionSearch", () => ({ locateTimelineTarget: vi.fn(() => false) }));

type Element = ReactElement<Record<string, any>>;
type Props = ComponentProps<typeof TimelineRail>;
type Anchor = { dataset: { timelineId?: string; timelineAliases?: string }; top: number };

function elements(node: ReactNode): Element[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!isValidElement<Record<string, any>>(node)) return [];
  return [node, ...elements(node.props.children as ReactNode)];
}

class FakeStream {
  scrollTop = 0;
  scrollHeight = 2000;
  clientHeight = 200;
  children: unknown[] = [];
  anchors: Anchor[] = [];
  listeners = new Map<string, Set<(...args: any[]) => void>>();
  querySelectorAll = vi.fn((_selector: string) => this.anchors.map(anchor => ({
    dataset: anchor.dataset,
    getBoundingClientRect: () => ({ top: anchor.top - this.scrollTop, bottom: anchor.top - this.scrollTop + 10, height: 10 })
  })));
  getBoundingClientRect() { return { top: 0, bottom: this.clientHeight, height: this.clientHeight }; }
  addEventListener(name: string, callback: (...args: any[]) => void) {
    const handlers = this.listeners.get(name) ?? new Set();
    handlers.add(callback);
    this.listeners.set(name, handlers);
  }
  removeEventListener(name: string, callback: (...args: any[]) => void) { this.listeners.get(name)?.delete(callback); }
}

class FakeMarks {
  clientHeight = 60;
  clientWidth = 20;
  scrollHeight = 60000;
  scrollTop = 0;
  focusedIndex: number | null = null;
  currentButtons: Element[] = [];
  getBoundingClientRect() { return { top: 100, bottom: 100 + this.clientHeight, height: this.clientHeight, left: 0, right: this.clientWidth, width: this.clientWidth }; }
  matches(_selector: string) { return false; }
  contains(_target: unknown) { return false; }
  querySelector(selector: string) {
    const index = Number(selector.match(/data-entry-index="(\d+)"/)?.[1]);
    const found = this.currentButtons.find(button => button.props["data-entry-index"] === index);
    return found ? { focus: (_options?: FocusOptions) => { this.focusedIndex = index; } } : null;
  }
}

let mutationObservers: Array<{ callback: (...args: any[]) => void; trigger: () => void }> = [];
let resizeObservers: Array<{ callback: (...args: any[]) => void }> = [];

function stubs() {
  hooks.cursor = 0;
  hooks.cells.length = 0;
  hooks.pending.length = 0;
  hooks.dirty = false;
  hooks.frames.clear();
  hooks.nextFrame = 1;
  mutationObservers = [];
  resizeObservers = [];
  vi.stubGlobal("document", { activeElement: null });
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    const id = hooks.nextFrame++;
    hooks.frames.set(id, callback);
    return id;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => { hooks.frames.delete(id); });
  vi.stubGlobal("ResizeObserver", class {
    constructor(callback: (...args: any[]) => void) { resizeObservers.push({ callback }); }
    observe(_target: unknown) {}
    unobserve(_target: unknown) {}
    disconnect() {}
  });
  vi.stubGlobal("MutationObserver", class {
    callback: (...args: any[]) => void;
    constructor(callback: (...args: any[]) => void) {
      this.callback = callback;
      mutationObservers.push({ callback, trigger: () => callback([], this) });
    }
    observe(_target: unknown, _options?: unknown) {}
    disconnect() {}
  });
}

function mount(props: Props, stream: FakeStream) {
  const marks = new FakeMarks();
  let currentProps = props;
  let tree: ReactNode;
  const commit = () => {
    const pending = [...hooks.pending];
    hooks.pending.length = 0;
    for (const item of pending) {
      const cell = hooks.cells[item.index];
      cell.cleanup?.();
      cell.cleanup = item.callback();
    }
  };
  const render = (nextProps?: Props) => {
    if (nextProps) currentProps = nextProps;
    let attempts = 20;
    do {
      hooks.cursor = 0;
      hooks.dirty = false;
      tree = TimelineRail(currentProps);
      const all = elements(tree);
      marks.currentButtons = all.filter(element => element.type === "button" && element.props.className?.includes("timeline-rail-item"));
      for (const element of all) {
        if (typeof element.type !== "string" || element.props.className !== "timeline-rail-marks") continue;
        const ref = (element as unknown as { ref?: { current: unknown } }).ref ?? element.props.ref;
        if (ref) ref.current = marks;
      }
      commit();
      if (--attempts === 0 && hooks.dirty) throw new Error("Timeline rail fixture did not settle");
    } while (hooks.dirty);
    return elements(tree);
  };
  const flushFrames = () => {
    let attempts = 20;
    while (hooks.frames.size && attempts--) {
      const batch = [...hooks.frames.entries()];
      hooks.frames.clear();
      for (const [, callback] of batch) callback(0);
    }
    if (hooks.frames.size) throw new Error("Timeline rail animation frames did not settle");
  };
  const button = (index: number) => {
    const found = marks.currentButtons.find(element => element.props["data-entry-index"] === index);
    if (!found) throw new Error(`Timeline mark ${index} is outside the rendered window`);
    return found;
  };
  render();
  return { marks, render, flushFrames, button, elements: () => elements(tree) };
}

function props(entries: TimelineEntry[], stream: FakeStream, overrides: Partial<Props> = {}): Props {
  return { entries, streamRef: { current: stream as unknown as HTMLDivElement }, ...overrides };
}

beforeEach(stubs);
afterEach(() => vi.unstubAllGlobals());

describe("TimelineRail", () => {
  test("uses a focus preview without adding duplicate browser title tooltips", () => {
    const stream = new FakeStream();
    const entry = { id: "entry-fixture", title: "Fixture title", preview: "Fixture preview" };
    const view = mount(props([entry], stream), stream);
    view.flushFrames();
    view.render();

    const mark = view.button(0);
    expect(mark.props.title).toBeUndefined();
    mark.props.onMouseEnter({ currentTarget: { getBoundingClientRect: () => ({ top: 900 }) } });
    const rendered = view.render();
    const preview = rendered.find(element => element.props.className === "timeline-rail-preview");

    expect(preview?.props["aria-hidden"]).toBe("true");
    expect(preview?.props.style.top).toBe(48);
    expect(rendered.every(element => element.props.title === undefined)).toBe(true);
  });

  test("keeps a 5,000-entry rail bounded while arrow keys and Home/End reach every item", () => {
    const stream = new FakeStream();
    const entries = Array.from({ length: 5000 }, (_, index) => ({ id: `entry-${index}`, title: `Entry ${index}` }));
    stream.anchors = [{ dataset: { timelineId: "entry-0" }, top: 0 }];
    const view = mount(props(entries, stream), stream);
    view.flushFrames();
    view.render();

    expect(view.marks.currentButtons).toHaveLength(100);
    expect(view.button(0).props["aria-setsize"]).toBe(5000);
    const visited = [0];
    for (let index = 1; index < entries.length; index++) {
      const event = { key: "ArrowDown", preventDefault: vi.fn() };
      view.button(index - 1).props.onKeyDown(event);
      view.render();
      view.flushFrames();
      expect(event.preventDefault).toHaveBeenCalledOnce();
      expect(view.marks.focusedIndex).toBe(index);
      visited.push(view.marks.focusedIndex!);
    }
    expect(visited).toEqual(entries.map((_entry, index) => index));

    view.button(4999).props.onKeyDown({ key: "Home", preventDefault: vi.fn() });
    view.render();
    view.flushFrames();
    expect(view.marks.focusedIndex).toBe(0);
    view.button(0).props.onKeyDown({ key: "End", preventDefault: vi.fn() });
    view.render();
    view.flushFrames();
    expect(view.marks.focusedIndex).toBe(4999);
    view.button(4999).props.onKeyDown({ key: "ArrowUp", preventDefault: vi.fn() });
    view.render();
    view.flushFrames();
    expect(view.marks.focusedIndex).toBe(4998);
  });

  test("routes a click through the current stream scope for duplicate global IDs", () => {
    const stream = new FakeStream();
    const view = mount(props([{ id: "shared-id", title: "Scoped item" }], stream), stream);
    view.button(0).props.onClick();

    expect(locateTimelineTarget).toHaveBeenCalledWith("shared-id", stream as unknown as HTMLDivElement);
  });

  test("recomputes timeline anchors after stream DOM mutations", () => {
    const stream = new FakeStream();
    stream.anchors = [
      { dataset: { timelineId: "first" }, top: 0 },
      { dataset: { timelineId: "second" }, top: 100 }
    ];
    const entries = [{ id: "first", title: "First" }, { id: "second", title: "Second" }];
    const view = mount(props(entries, stream), stream);
    view.flushFrames();
    view.render();
    expect(view.button(0).props.className).toContain("active");
    const reads = stream.querySelectorAll.mock.calls.length;

    stream.anchors = [
      { dataset: { timelineId: "second" }, top: 0 },
      { dataset: { timelineId: "first" }, top: 100 }
    ];
    mutationObservers[mutationObservers.length - 1]?.trigger();
    view.flushFrames();
    view.render();

    expect(stream.querySelectorAll.mock.calls.length).toBeGreaterThan(reads);
    expect(view.button(1).props.className).toContain("active");
  });
});
