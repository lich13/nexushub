import { isValidElement, type ReactElement, type ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { DesktopView } from "../../lib/api/desktop";
import { desktopDiagnostics } from "../../lib/query/desktop";
import { DesktopComposer } from "./DesktopComposer";

const hooks = vi.hoisted(() => ({
  index: 0,
  cells: new Map<number, unknown>(),
  desktop: null as any,
  catalog: null as any,
  catalogCalls: [] as unknown[][],
  attachments: null as any,
  attachmentSessionId: undefined as string | undefined,
  attachmentReceive: null as ((items: any[]) => void) | null,
  attachmentHandles: [] as string[]
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
vi.mock("../../hooks/useDesktopAttachments", () => ({
  useDesktopAttachments: (sessionId: string | undefined, _region: unknown, handles: string[], receive: (items: any[]) => void) => {
    hooks.attachmentSessionId = sessionId;
    hooks.attachmentHandles = handles;
    hooks.attachmentReceive = sessionId ? receive : null;
    return hooks.attachments;
  }
}));
vi.mock("../../hooks/useDesktopReferenceCatalog", () => ({
  useDesktopReferenceCatalog: (sessionId: string | undefined, kind: string, query: string) => {
    hooks.catalogCalls.push([sessionId, kind, query]);
    return hooks.catalog;
  }
}));
vi.mock("../../lib/query/desktop", () => ({
  chooseDesktopItem: vi.fn(async () => ({ item: null })),
  createDesktopThread: vi.fn(async () => ({ threadId: "thread-fixture" })),
  desktopCatalog: vi.fn(async () => ({ items: [], nextCursor: null })),
  desktopDiagnostics: vi.fn()
}));

type Element = ReactElement<Record<string, any>>;

function view(overrides: Partial<DesktopView> = {}): DesktopView {
  return {
    sessionId: "session-fixture", threadId: "thread-fixture", revision: 1,
    connected: true, writable: true, runtime: "codex", activeTurnId: null,
    model: null, reasoningEffort: null, mode: null, requests: [], queue: [], error: null,
    ...overrides
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

function attachmentItem(index: number) {
  return { handle: `file-handle-${index}-fixture`, name: `file-${index}.txt`, kind: "file" };
}

function receiveAttachments(items: ReturnType<typeof attachmentItem>[]) {
  if (!hooks.attachmentReceive) throw new Error("Desktop attachment callback fixture is missing");
  hooks.attachmentReceive(items);
}

async function flushMicrotasks() {
  for (let index = 0; index < 8; index += 1) await Promise.resolve();
}

function mockClipboard() {
  const writeText = vi.fn(async (_text: string) => undefined);
  const navigatorFixture = Object.create(globalThis.navigator ?? null);
  Object.defineProperty(navigatorFixture, "clipboard", { value: { writeText }, configurable: true });
  vi.stubGlobal("navigator", navigatorFixture);
  return writeText;
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
  hooks.attachments = { paste: vi.fn(), release: vi.fn() };
  hooks.attachmentSessionId = undefined;
  hooks.attachmentReceive = null;
  hooks.attachmentHandles = [];
  vi.mocked(desktopDiagnostics).mockReset();
  let id = 0;
  vi.stubGlobal("crypto", { randomUUID: () => `message-fixture-${++id}` });
});

afterEach(() => { vi.unstubAllGlobals(); });

describe("desktop composer reference interactions", () => {
  test("starts the WebKit clipboard write during the click before diagnostics resolve", async () => {
    let resolve!: (value: any) => void;
    vi.mocked(desktopDiagnostics).mockReturnValueOnce(new Promise(accept => { resolve = accept; }));
    const payloads: Promise<Blob>[] = [];
    vi.stubGlobal("ClipboardItem", class {
      constructor(parts: Record<string, Promise<Blob>>) { payloads.push(parts["text/plain"]); }
    });
    const write = vi.fn(async () => { await payloads[0]; });
    const navigatorFixture = Object.create(globalThis.navigator ?? null);
    Object.defineProperty(navigatorFixture, "clipboard", { value: { write }, configurable: true });
    vi.stubGlobal("navigator", navigatorFixture);
    hooks.desktop.view = null;
    required(render(), element => element.type === "button" && element.props.children === "复制诊断").props.onClick();
    expect(write).toHaveBeenCalledOnce();
    const diagnostics = { records: [], desktopBuild: "20586", bundledCliVersion: "0.0.0-fixture" };
    resolve(diagnostics);
    await flushMicrotasks();
    expect(JSON.parse(await (await payloads[0]).text())).toEqual(diagnostics);
  });

  test.each([
    ["detached", null],
    ["disconnected", view({ connected: false })],
    ["read-only", view({ writable: false })]
  ])("shows the connection row without stale status for a %s snapshot", (_description, snapshot) => {
    hooks.desktop.view = snapshot;
    const tree = render();

    expect(required(tree, element => element.type === "div" && element.props.className === "desktop-connect-row")).toBeTruthy();
    expect(elements(tree).some(element => element.props.className === "desktop-collaboration-status")).toBe(false);
    expect(required(tree, element => element.type === "button" && element.props.children === "连接")).toBeTruthy();
  });

  test("keeps unknown and paused queue entries visible without authorizing a new owner", () => {
    hooks.desktop.view = view({ writable: false, connectionPhase: "owner_confirmation_required", queue: [
      { id: "unknown-fixture", kind: "queue", status: "outcome_unknown", preview: "unknown fixture" },
      { id: "held-fixture", kind: "queue", status: "held", preview: "held fixture" }
    ] });
    const tree = render();
    expect(required(tree, element => element.type === "small" && element.props.children === "结果待核对")).toBeTruthy();
    for (const label of ["核对后移除", "取消", "继续发送队列"]) {
      expect(required(tree, element => element.type === "button" && element.props.children === label).props.disabled).toBe(true);
    }
    expect(elements(tree).some(element => element.type === "textarea")).toBe(false);
  });

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

  test("sends @ app references as opaque handles with UTF-8 spans", async () => {
    hooks.catalog.items = [{
      handle: "app-handle-fixture", kind: "app", name: "工具", description: "fixture app",
      reasoningEfforts: [], defaultReasoningEffort: null, path: "example.invalid/app-fixture"
    }];
    let tree = typeText("猫😀 @工");
    textarea(tree).props.onKeyDown(keyEvent("Enter"));
    tree = render();
    expect(textarea(tree).props.value).toBe("猫😀 @工具 ");

    required(tree, element => element.type === "form").props.onSubmit({ preventDefault: vi.fn() });
    await flushMicrotasks();

    const [action] = hooks.desktop.act.mock.calls[0];
    expect(action).toEqual({
      type: "send", messageId: "message-fixture-1", text: "猫😀 @工具 ",
      references: ["app-handle-fixture"],
      referenceSpans: [{ handle: "app-handle-fixture", start: 8, end: 15 }]
    });
    expect(JSON.stringify(action)).not.toContain("example.invalid/app-fixture");
  });

  test("copies host diagnostics while disconnected", async () => {
    hooks.desktop.view = view({ connected: false });
    const writeText = mockClipboard();
    const diagnostics = { records: [], desktopBuild: "fixture-build", bundledCliVersion: null };
    vi.mocked(desktopDiagnostics).mockResolvedValue(diagnostics);

    const tree = render();
    required(tree, element => element.type === "button" && element.props.children === "复制诊断").props.onClick();
    await flushMicrotasks();

    expect(desktopDiagnostics).toHaveBeenCalledOnce();
    expect(desktopDiagnostics).toHaveBeenCalledWith();
    expect(writeText).toHaveBeenCalledExactlyOnceWith(JSON.stringify(diagnostics, null, 2));
  });

  test("shows a safe copy error when diagnostics or clipboard access fails", async () => {
    hooks.desktop.view = view({ connected: false });
    const writeText = mockClipboard();
    vi.mocked(desktopDiagnostics).mockRejectedValue(new Error("diagnostics fixture unavailable"));

    let tree = render();
    required(tree, element => element.type === "button" && element.props.children === "复制诊断").props.onClick();
    await flushMicrotasks();
    tree = render();

    expect(writeText).not.toHaveBeenCalled();
    expect(required(tree, element => element.props.role === "alert").props.children).toBe("无法复制诊断，请重试");

    vi.mocked(desktopDiagnostics).mockResolvedValue({ records: [], desktopBuild: null, bundledCliVersion: null });
    writeText.mockRejectedValueOnce(new Error("clipboard fixture unavailable"));
    required(tree, element => element.type === "button" && element.props.children === "复制诊断").props.onClick();
    await flushMicrotasks();
    tree = render();
    expect(required(tree, element => element.props.role === "alert").props.children).toBe("无法复制诊断，请重试");
  });

  test("rejects an over-limit attachment batch atomically and releases every incoming handle", () => {
    render();
    const existing = Array.from({ length: 31 }, (_, index) => attachmentItem(index));
    receiveAttachments(existing);
    render();
    expect(hooks.attachmentHandles).toEqual(existing.map(item => item.handle));

    hooks.attachments.release.mockClear();
    const incoming = [attachmentItem(31), attachmentItem(32)];
    receiveAttachments(incoming);
    const tree = render();

    expect(hooks.attachmentHandles).toEqual(existing.map(item => item.handle));
    expect(hooks.attachments.release).toHaveBeenCalledExactlyOnceWith(incoming.map(item => item.handle));
    expect(elements(tree).some(element => element.props.role === "alert" && element.props.children === "引用数量超出限制，本次附件未添加")).toBe(true);
  });

  test("releases a removed attachment handle", () => {
    render();
    const item = attachmentItem(0);
    receiveAttachments([item]);
    const tree = render();
    required(tree, element => element.type === "button" && element.props["aria-label"] === `移除 ${item.name}`).props.onClick();
    const afterRemoval = render();

    expect(hooks.attachments.release).toHaveBeenCalledExactlyOnceWith([item.handle]);
    expect(hooks.attachmentHandles).toEqual([]);
    expect(elements(afterRemoval).some(element => element.props.className === "desktop-reference-chips")).toBe(false);
  });

  test("pauses native attachment receiving while a desktop action is busy", () => {
    render();
    expect(hooks.attachmentSessionId).toBe("session-fixture");

    hooks.desktop.busy = true;
    render();

    expect(hooks.attachmentSessionId).toBeUndefined();
    expect(hooks.attachmentReceive).toBeNull();
  });

  test("pauses native attachment receiving while choosing a file", async () => {
    const tree = render();
    expect(hooks.attachmentSessionId).toBe("session-fixture");

    required(tree, element => element.type === "button" && element.props.title === "添加文件或图片").props.onClick();
    const choosing = render();

    expect(hooks.attachmentSessionId).toBeUndefined();
    expect(hooks.attachmentReceive).toBeNull();
    expect(textarea(choosing).props.disabled).toBe(true);

    await flushMicrotasks();
    render();
    expect(hooks.attachmentSessionId).toBe("session-fixture");
  });
});
