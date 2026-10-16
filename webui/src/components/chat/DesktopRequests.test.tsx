import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test, vi } from "vitest";
import type { DesktopPendingRequest, DesktopQuestion } from "../../lib/api/desktop";
import { DesktopRequests } from "./DesktopRequests";

function request(overrides: Partial<DesktopPendingRequest> = {}): DesktopPendingRequest {
  return {
    key: "request-fixture", kind: "question", title: "示例请求", body: "", questions: [], formSchema: null,
    canAccept: true, ...overrides
  };
}

function question(id: string, secret = false): DesktopQuestion {
  return {
    id, header: `示例标题 ${id}`, question: `示例问题 ${id}`, secret,
    options: [{ label: "示例选项", description: "" }]
  };
}

function render(requests: DesktopPendingRequest[]) {
  return renderToStaticMarkup(<DesktopRequests requests={requests} busy={false} act={vi.fn(async () => undefined)} open={vi.fn(async () => undefined)} />);
}

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
    const html = render([request({ kind, canAccept: true, supportedDecisions })]);

    expect(html.includes("允许本次会话")).toBe(shown);
  });
});
