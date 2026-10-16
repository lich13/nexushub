import { renderToStaticMarkup } from "react-dom/server";
import { isValidElement, type ReactElement, type ReactNode } from "react";
import { beforeEach, describe, expect, test, vi } from "vitest";
import type { DesktopPendingRequest, DesktopQuestion, DesktopReview } from "../../lib/api/desktop";
import { DesktopRequests } from "./DesktopRequests";

const hooks = vi.hoisted(() => ({
  index: 0,
  dirty: false,
  cells: new Map<number, unknown>()
}));

vi.mock("react", async original => ({
  ...await original<typeof import("react")>(),
  useState: (initial: unknown) => {
    const index = hooks.index++;
    if (!hooks.cells.has(index)) hooks.cells.set(index, typeof initial === "function" ? (initial as () => unknown)() : initial);
    return [hooks.cells.get(index), (value: unknown) => {
      const next = typeof value === "function" ? (value as (previous: unknown) => unknown)(hooks.cells.get(index)) : value;
      if (!Object.is(next, hooks.cells.get(index))) { hooks.cells.set(index, next); hooks.dirty = true; }
    }];
  }
}));

function request(overrides: Partial<DesktopPendingRequest> = {}): DesktopPendingRequest {
  return {
    key: "request-fixture", kind: "question", title: "示例请求", body: "", questions: [], formSchema: null,
    canAccept: true, ...overrides
  };
}

function review(overrides: Partial<DesktopReview> = {}): DesktopReview {
  return { command: null, cwd: null, reason: null, changes: [], permissions: null, extra: null, truncated: false, ...overrides };
}

function question(id: string, secret = false): DesktopQuestion {
  return {
    id, header: `示例标题 ${id}`, question: `示例问题 ${id}`, secret,
    options: [{ label: "示例选项", description: "" }]
  };
}

function render(requests: DesktopPendingRequest[]) {
  hooks.index = 0;
  hooks.dirty = false;
  hooks.cells.clear();
  return renderToStaticMarkup(<DesktopRequests requests={requests} busy={false} act={vi.fn(async () => undefined)} open={vi.fn(async () => undefined)} />);
}

type Element = ReactElement<Record<string, any>>;
type Action = Parameters<typeof DesktopRequests>[0]["act"];

function elements(node: ReactNode): Element[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!isValidElement<Record<string, any>>(node)) return [];
  if (typeof node.type === "function") {
    const component = node.type as (props: Record<string, any>) => ReactNode;
    return elements(component(node.props));
  }
  if (typeof node.type !== "string") return elements(node.props.children);
  return [node, ...elements(node.props.children)];
}

function textContent(node: ReactNode): string {
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(textContent).join("");
  if (!isValidElement<Record<string, any>>(node)) return "";
  return textContent(node.props.children);
}

function mount(requests: DesktopPendingRequest[], act: Action = vi.fn(async () => undefined)) {
  const open = vi.fn(async () => undefined);
  let tree: Element[] = [];
  const render = () => {
    let remaining = 10;
    do {
      hooks.index = 0;
      hooks.dirty = false;
      tree = elements(DesktopRequests({ requests, busy: false, act, open }));
      if (!hooks.dirty) return;
    } while (--remaining > 0);
    throw new Error("Fixture render did not settle");
  };
  const findAll = (predicate: (element: Element) => boolean) => tree.filter(predicate);
  render();
  return { render, findAll, act };
}

beforeEach(() => {
  hooks.index = 0;
  hooks.dirty = false;
  hooks.cells.clear();
});

describe("Codex desktop pending requests", () => {
  test("prototype-like question IDs render blank answers and keep secret answers masked", () => {
    const html = render([request({ questions: [question("__proto__"), question("toString"), question("secret-fixture", true)] })]);

    expect(html).toContain('name="request-fixture:__proto__"');
    expect(html).toContain('name="request-fixture:toString"');
    expect(html).toContain('<input type="password" autoComplete="off" maxLength="4096" value=""');
    expect(html).not.toMatch(/<input[^>]*\bchecked(?:=|\s|>)/);
    expect(html).toMatch(/<button disabled="">提交回答<\/button>/);
  });

  test("prototype-like form property names render empty controls instead of inherited defaults", () => {
    const properties = JSON.parse('{"__proto__":{"type":"string","title":"原型字段"},"toString":{"type":"string","title":"字符串字段"},"constructor":{"type":"boolean","title":"布尔字段"}}') as Record<string, unknown>;
    const html = render([request({
      kind: "elicitation",
      formSchema: { properties, required: ["__proto__", "toString", "constructor"] }
    })]);

    expect(html).toContain("原型字段");
    expect(html).toContain("字符串字段");
    expect(html).toContain("布尔字段");
    expect(html).toMatch(/type="text"[^>]*value=""/);
    expect(html).toContain('<select required=""><option value="" selected="">请选择</option>');
    expect(html).toContain('required=""');
    expect(html).not.toContain("[object Object]");
  });

  test.each([
    ["不支持批准的命令", "command", false],
    ["不支持批准的问题", "question", false],
    ["未知请求类型", "future_request", true]
  ])("%s has no approval entry point", (_label, kind, canAccept) => {
    const html = render([request({ kind, canAccept })]);

    expect(html).not.toContain("允许本次");
    expect(html).not.toContain("提交回答");
    expect(html).not.toContain("<form");
  });

  test.each([
    ["旧服务未声明", "command", undefined, false],
    ["支持会话级命令审批", "command", ["acceptForSession"], true],
    ["支持会话级文件审批", "file", ["acceptForSession"], true],
    ["权限审批仍仅限本回合", "permissions", ["acceptForSession"], false]
  ])("会话级审批按钮受后端能力门控：%s", (_label, kind, supportedDecisions, shown) => {
    const html = render([request({ kind, canAccept: true, supportedDecisions, review: review() })]);

    expect(html.includes("允许本次会话")).toBe(shown);
  });

  test("structured command review takes precedence over the legacy body", () => {
    const html = render([request({
      kind: "command",
      body: "legacy-body-sentinel",
      review: review({ command: "fixture command", cwd: "/fixture/workspace", reason: "Synthetic review reason" })
    })]);

    expect(html).toContain("fixture command");
    expect(html).toContain("/fixture/workspace");
    expect(html).toContain("Synthetic review reason");
    expect(html).not.toContain("legacy-body-sentinel");
  });

  test("file changes show separate folded rows and mount each diff only after expansion", () => {
    const firstDiff = "--- before\n+++ fixture first\n";
    const secondDiff = "+++ fixture second\n+synthetic content\n";
    const view = mount([request({
      kind: "file",
      review: review({ changes: [
        { path: "fixture-first.txt", kind: "update", diff: firstDiff, truncated: false },
        { path: "fixture-second.txt", kind: "create", diff: secondDiff, truncated: false }
      ] })
    })]);

    const disclosures = view.findAll(element => element.type === "details");
    expect(disclosures).toHaveLength(2);
    expect(disclosures.map(textContent)).toEqual(expect.arrayContaining([expect.stringContaining("fixture-first.txt"), expect.stringContaining("fixture-second.txt")]));
    expect(view.findAll(element => element.type === "pre" && textContent(element).includes(firstDiff))).toHaveLength(0);
    expect(view.findAll(element => element.type === "pre" && textContent(element).includes(secondDiff))).toHaveLength(0);

    const first = disclosures.find(element => textContent(element).includes("fixture-first.txt"));
    expect(first).toBeDefined();
    first?.props.onToggle?.({ currentTarget: { open: true } });
    view.render();

    expect(view.findAll(element => element.type === "pre" && textContent(element).includes(firstDiff))).toHaveLength(1);
    expect(view.findAll(element => element.type === "pre" && textContent(element).includes(secondDiff))).toHaveLength(0);
  });

  test("unknown review fields stay available inside a folded disclosure", () => {
    const view = mount([request({
      kind: "command",
      review: review({ command: "fixture command", extra: "{\"futureField\":\"fixture extra value\"}" })
    })]);
    const extra = view.findAll(element => element.type === "details" && textContent(element).includes("其他请求信息"))[0];

    expect(extra).toBeDefined();
    expect(view.findAll(element => element.type === "pre" && textContent(element).includes("fixture extra value"))).toHaveLength(0);
    extra?.props.onToggle?.({ currentTarget: { open: true } });
    view.render();
    expect(view.findAll(element => element.type === "pre" && textContent(element).includes("fixture extra value"))).toHaveLength(1);
  });

  test("legacy service bodies remain the fallback when no structured review is supplied", () => {
    const html = render([request({ kind: "command", body: "{\"command\":\"fixture legacy command\"}" })]);

    expect(html).toContain('<pre>{&quot;command&quot;:&quot;fixture legacy command&quot;}</pre>');
  });

  test("truncated reviews remain viewable without an approval control", () => {
    const html = render([request({
      kind: "file",
      canAccept: false,
      review: review({
        changes: [{ path: "fixture-large.txt", kind: "update", diff: "fixture diff prefix", truncated: true }],
        truncated: true
      })
    })]);

    expect(html).toContain("fixture-large.txt · update · 已截断");
    expect(html).toContain("内容超出显示上限");
    expect(html).not.toContain("允许本次</button>");
  });

  test("approval actions keep the request key bound to the displayed review", () => {
    const act = vi.fn(async () => undefined);
    const view = mount([request({
      key: "fixture-file-request-key",
      kind: "file",
      review: review({ changes: [{ path: "fixture-change.txt", kind: "update", diff: "fixture diff", truncated: false }] })
    })], act);
    const allow = view.findAll(element => element.type === "button" && textContent(element) === "允许本次")[0];

    expect(allow).toBeDefined();
    allow?.props.onClick();
    expect(act).toHaveBeenCalledWith({ type: "decide", requestKey: "fixture-file-request-key", allow: true });
  });
});
