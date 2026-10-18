import { isValidElement, type ReactElement, type ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { DesktopView } from "../../lib/api/desktop";
import { chooseDesktopItem, desktopDiagnostics } from "../../lib/query/desktop";
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

function renderedText(node: ReactNode): string {
  if (Array.isArray(node)) return node.map(renderedText).join("");
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (isValidElement<Record<string, any>>(node)) return renderedText(node.props.children);
  return "";
}

function render(threadId = "thread-fixture") {
  hooks.index = 0;
  return DesktopComposer({ threadId }) as unknown as ReactElement;
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
    open: vi.fn(async () => undefined), connect: vi.fn(), clearActionError: vi.fn(() => {
      hooks.desktop.error = "";
    })
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
  vi.mocked(chooseDesktopItem).mockReset().mockResolvedValue({ item: null });
  vi.mocked(desktopDiagnostics).mockReset();
  let id = 0;
  vi.stubGlobal("crypto", { randomUUID: () => `message-fixture-${++id}` });
});

afterEach(() => { vi.unstubAllGlobals(); });

describe("desktop composer reference interactions", () => {
  test("requests native diagnostic copying and shows pending and success states without browser clipboard access", async () => {
    let resolve!: (value: Awaited<ReturnType<typeof desktopDiagnostics>>) => void;
    vi.mocked(desktopDiagnostics).mockReturnValueOnce(new Promise(accept => { resolve = accept; }));
    const writeText = mockClipboard();
    hooks.desktop.view = null;
    const copyButton = required(render(), element => element.type === "button" && element.props["aria-label"] === "复制诊断");
    copyButton.props.onClick();
    expect(desktopDiagnostics).toHaveBeenCalledExactlyOnceWith(true);
    expect(required(render(), element => element.type === "button" && element.props["aria-label"] === "复制诊断").props.disabled).toBe(true);
    expect(writeText).not.toHaveBeenCalled();

    const diagnostics = { records: [], desktopBuild: "fixture-build", bundledCliVersion: "0.0.0-fixture" };
    resolve(diagnostics);
    await flushMicrotasks();
    const completed = render();
    expect(required(completed, element => element.props.role === "status").props.children).toBe("诊断已复制");
    expect(required(completed, element => element.type === "button" && element.props["aria-label"] === "复制诊断").props.disabled).toBe(false);
    expect(writeText).not.toHaveBeenCalled();
  });

  test.each([
    { identity: "thread", outcome: "success" },
    { identity: "session", outcome: "success" },
    { identity: "thread", outcome: "error" },
    { identity: "session", outcome: "error" }
  ] as const)("ignores late diagnostic $outcome after the $identity changes", async ({ identity, outcome }) => {
    let resolve!: (value: Awaited<ReturnType<typeof desktopDiagnostics>>) => void;
    let reject!: (reason?: unknown) => void;
    vi.mocked(desktopDiagnostics).mockReturnValueOnce(new Promise((accept, fail) => {
      resolve = accept;
      reject = fail;
    }));

    required(render(), element => element.type === "button" && element.props["aria-label"] === "复制诊断").props.onClick();
    if (identity === "thread") {
      render("thread-next");
    } else {
      hooks.desktop.view = view({ sessionId: "session-next" });
      render();
    }

    if (outcome === "success") {
      resolve({ records: [], desktopBuild: "fixture-build", bundledCliVersion: null });
    } else {
      reject(new Error("late diagnostics fixture"));
    }
    await flushMicrotasks();

    const tree = render(identity === "thread" ? "thread-next" : "thread-fixture");
    expect(elements(tree).some(element => element.props.role === "status" || element.props.role === "alert")).toBe(false);
    expect(desktopDiagnostics).toHaveBeenCalledExactlyOnceWith(true);
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

  test.each(["20586", "20770", "fixture-build"]) ("uses the desktop protocol without a build warning for build %s", desktopBuild => {
    hooks.desktop.view = view({ desktopBuild });
    const tree = render();

    expect(elements(tree).some(element => renderedText(element.props.children).includes("尚未验证"))).toBe(false);
    expect(elements(tree).some(element => renderedText(element.props.children).includes("对照 build 20586"))).toBe(false);
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

  test("keeps a failed draft and clears its action error when a reference is selected", async () => {
    hooks.desktop.view = view({ queue: [
      { id: "unknown-fixture", kind: "queue", status: "outcome_unknown", preview: "unknown fixture" }
    ] });
    hooks.desktop.act = vi.fn(async () => {
      hooks.desktop.error = "fixture action failure";
      throw new Error("fixture action failure");
    });
    hooks.catalog.items = [{
      handle: "skill-alpha-fixture", kind: "skill", name: "alpha", description: "",
      reasoningEfforts: [], defaultReasoningEffort: null
    }];

    let tree = render();
    textarea(tree).props.onChange({ target: { value: "fixture request", selectionStart: 15, selectionEnd: 15 } });
    tree = render();
    required(tree, element => element.type === "form").props.onSubmit({ preventDefault: vi.fn() });
    await flushMicrotasks();
    tree = render();

    expect(textarea(tree).props.value).toBe("fixture request");
    expect(elements(tree).some(element => element.props.role === "alert" && renderedText(element.props.children).includes("fixture action failure"))).toBe(true);
    expect(required(tree, element => element.type === "small" && element.props.children === "结果待核对")).toBeTruthy();

    tree = typeText("$a");
    textarea(tree).props.onKeyDown(keyEvent("ArrowDown"));
    tree = render();
    textarea(tree).props.onKeyDown(keyEvent("Enter"));
    tree = render();

    expect(hooks.desktop.clearActionError).toHaveBeenCalledOnce();
    expect(elements(tree).some(element => element.props.role === "alert" && renderedText(element.props.children).includes("fixture action failure"))).toBe(false);
    expect(required(tree, element => element.type === "small" && element.props.children === "结果待核对")).toBeTruthy();
  });

  test("keeps a failed draft and clears its action error when an attachment is removed", async () => {
    hooks.desktop.view = view({ queue: [
      { id: "unknown-fixture", kind: "queue", status: "outcome_unknown", preview: "unknown fixture" }
    ] });
    hooks.desktop.act = vi.fn(async () => {
      hooks.desktop.error = "fixture action failure";
      throw new Error("fixture action failure");
    });

    render();
    const item = attachmentItem(0);
    receiveAttachments([item]);
    render();
    hooks.desktop.clearActionError.mockClear();

    let tree = render();
    textarea(tree).props.onChange({ target: { value: "fixture request", selectionStart: 15, selectionEnd: 15 } });
    tree = render();
    required(tree, element => element.type === "form").props.onSubmit({ preventDefault: vi.fn() });
    await flushMicrotasks();
    tree = render();
    expect(textarea(tree).props.value).toBe("fixture request");
    expect(required(tree, element => element.type === "button" && element.props["aria-label"] === `移除 ${item.name}`)).toBeTruthy();
    expect(required(tree, element => element.type === "small" && element.props.children === "结果待核对")).toBeTruthy();

    required(tree, element => element.type === "button" && element.props["aria-label"] === `移除 ${item.name}`).props.onClick();
    tree = render();

    expect(hooks.desktop.clearActionError).toHaveBeenCalledOnce();
    expect(elements(tree).some(element => element.props.role === "alert" && renderedText(element.props.children).includes("fixture action failure"))).toBe(false);
    expect(elements(tree).some(element => element.props.className === "desktop-reference-chips")).toBe(false);
    expect(required(tree, element => element.type === "small" && element.props.children === "结果待核对")).toBeTruthy();
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

  test("offers file, skill, plugin and app, and Codex goal actions from the add menu", async () => {
    let tree = render();
    const summary = required(tree, element => element.type === "summary" && element.props["aria-label"] === "添加引用或附件");
    expect(summary.props.title).toBe("添加引用或附件");
    const menu = required(tree, element => element.type === "details" && element.props.className === "desktop-add-menu");
    expect(menu.props.open).toBeUndefined();
    const labels = elements(menu).filter(element => element.type === "button").map(element => renderedText(element.props.children));
    expect(labels).toEqual(["文件或图片", "技能$", "插件与应用@", "目标在 Codex 中管理"]);

    required(menu, element => element.type === "button" && renderedText(element.props.children).includes("文件或图片")).props.onClick();
    expect(chooseDesktopItem).toHaveBeenCalledExactlyOnceWith("file", "session-fixture");
    await flushMicrotasks();

    tree = render();
    const skillsMenu = required(tree, element => element.type === "details" && element.props.className === "desktop-add-menu");
    required(skillsMenu, element => element.type === "button" && renderedText(element.props.children).includes("技能")).props.onClick();
    tree = render();
    expect(hooks.catalogCalls.at(-1)).toEqual(["session-fixture", "skills", ""]);
    expect(required(tree, element => element.type === "div" && element.props.className === "desktop-reference-picker")).toBeTruthy();

    const mentionsMenu = required(tree, element => element.type === "details" && element.props.className === "desktop-add-menu");
    required(mentionsMenu, element => element.type === "button" && renderedText(element.props.children).includes("插件与应用")).props.onClick();
    tree = render();
    expect(hooks.catalogCalls.at(-1)).toEqual(["session-fixture", "mentions", ""]);

    const goalMenu = required(tree, element => element.type === "details" && element.props.className === "desktop-add-menu");
    required(goalMenu, element => element.type === "button" && renderedText(element.props.children).includes("目标")).props.onClick();
    expect(hooks.desktop.open).toHaveBeenCalledOnce();
  });

  test("keeps the add menu open for null or internal blur targets", () => {
    const menu = required(render(), element => element.type === "details" && element.props.className === "desktop-add-menu");
    const inside = {};
    const outside = {};
    const currentTarget = { open: true, contains: vi.fn((target: unknown) => target === inside) };

    menu.props.onBlur({ currentTarget, relatedTarget: null });
    expect(currentTarget.open).toBe(true);
    expect(currentTarget.contains).not.toHaveBeenCalled();

    menu.props.onBlur({ currentTarget, relatedTarget: inside });
    expect(currentTarget.open).toBe(true);
    menu.props.onBlur({ currentTarget, relatedTarget: outside });
    expect(currentTarget.open).toBe(false);
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

  test("reports completion when the native desktop copies diagnostics while disconnected", async () => {
    hooks.desktop.view = view({ connected: false });
    const writeText = mockClipboard();
    const diagnostics = { records: [], desktopBuild: "fixture-build", bundledCliVersion: null };
    vi.mocked(desktopDiagnostics).mockResolvedValue(diagnostics);

    const tree = render();
    required(tree, element => element.type === "button" && element.props["aria-label"] === "复制诊断").props.onClick();
    await flushMicrotasks();
    const completed = render();

    expect(desktopDiagnostics).toHaveBeenCalledOnce();
    expect(desktopDiagnostics).toHaveBeenCalledWith(true);
    expect(writeText).not.toHaveBeenCalled();
    expect(required(completed, element => element.props.role === "status").props.children).toBe("诊断已复制");
  });

  test("shows a safe copy error when the native diagnostics command fails", async () => {
    hooks.desktop.view = view({ connected: false });
    const writeText = mockClipboard();
    vi.mocked(desktopDiagnostics).mockRejectedValue(new Error("diagnostics fixture unavailable"));

    let tree = render();
    required(tree, element => element.type === "button" && element.props["aria-label"] === "复制诊断").props.onClick();
    await flushMicrotasks();
    tree = render();

    expect(writeText).not.toHaveBeenCalled();
    expect(required(tree, element => element.props.role === "alert").props.children).toBe("无法复制诊断，请重试");

    vi.mocked(desktopDiagnostics).mockResolvedValue({ records: [], desktopBuild: null, bundledCliVersion: null });
    required(tree, element => element.type === "button" && element.props["aria-label"] === "复制诊断").props.onClick();
    await flushMicrotasks();
    tree = render();
    expect(desktopDiagnostics).toHaveBeenCalledTimes(2);
    expect(desktopDiagnostics).toHaveBeenNthCalledWith(1, true);
    expect(desktopDiagnostics).toHaveBeenNthCalledWith(2, true);
    expect(writeText).not.toHaveBeenCalled();
    expect(required(tree, element => element.props.role === "status").props.children).toBe("诊断已复制");
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

    required(tree, element => element.type === "button" && renderedText(element.props.children).includes("文件或图片")).props.onClick();
    const choosing = render();

    expect(hooks.attachmentSessionId).toBeUndefined();
    expect(hooks.attachmentReceive).toBeNull();
    expect(textarea(choosing).props.disabled).toBe(true);

    await flushMicrotasks();
    render();
    expect(hooks.attachmentSessionId).toBe("session-fixture");
  });
});
