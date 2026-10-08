import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, expect, test, vi } from "vitest";
import type { MessageBlock, SubagentActivity, SubagentDetailResponse } from "../../types";
import { useSubagentDetail } from "../../lib/query/subagents";
import { SubagentPanel } from "./SubagentPanel";

vi.mock("../../lib/query/subagents", () => ({ useSubagentDetail: vi.fn() }));
vi.mock("../../lib/query/connection", () => ({ machineScope: () => "local:fixture" }));

const agent: SubagentActivity = {
  agentId: "child-fixture", name: "示例子智能体", status: "running", available: true,
  delegation: "仅检查示例文件。"
};
const message = (id: string, text: string): MessageBlock => ({ id, role: "assistant", kind: "message", text, questions: [] });
const page = (blocks: MessageBlock[], overrides: Partial<SubagentDetailResponse> = {}): SubagentDetailResponse => ({
  rootThreadId: "root-fixture", parentThreadId: "root-fixture", agent,
  detail: { summary: { id: "child-fixture", title: "示例子智能体", status: "Running", message_count: blocks.length }, blocks, messages: [], raw_event_count: blocks.length },
  ...overrides
});
type QueryState = ReturnType<typeof useSubagentDetail>;

function queryState(overrides: Partial<QueryState> = {}) {
  vi.mocked(useSubagentDetail).mockReturnValue({
    data: undefined, isLoading: false, error: null, hasNextPage: false,
    isFetchingNextPage: false, refetch: vi.fn(), fetchNextPage: vi.fn(),
    ...overrides
  } as QueryState);
}

function renderPanel(docked = true) {
  return renderToStaticMarkup(<SubagentPanel rootThreadId="root-fixture" initialAgent={agent} onClose={vi.fn()} docked={docked} />);
}

beforeEach(() => { vi.clearAllMocks(); queryState(); });

test("loading is announced and does not claim the child history is empty", () => {
  queryState({ isLoading: true });
  const html = renderPanel();
  expect(useSubagentDetail).toHaveBeenCalledWith("root-fixture", "child-fixture");
  expect(html).toContain('role="status"');
  expect(html).toContain("正在读取子智能体");
  expect(html).not.toContain("暂无子智能体消息");
});

test("a read error exposes retry and keeps previously loaded child content", () => {
  queryState({
    error: new Error("示例读取失败"),
    data: { pages: [page([message("cached", "已读取的示例回复")])], pageParams: [null] }
  });
  const html = renderPanel();
  expect(html).toContain('role="alert"');
  expect(html).toContain("示例读取失败");
  expect(html).toContain(">重试</button>");
  expect(html).toContain("已读取的示例回复");
});

test("an empty loaded history retains the latest child name and status", () => {
  queryState({ data: { pages: [page([], { agent: { ...agent, name: "更新后的示例名称", status: "completed" } })], pageParams: [null] } });
  const html = renderPanel();
  expect(html).toContain("更新后的示例名称");
  expect(html).toContain("已完成");
  expect(html).toContain("暂无子智能体消息");
  expect(html).not.toContain("thread-running-indicator");
});

test("older pages render chronologically and duplicate IDs use the current page content", () => {
  queryState({ data: {
    pages: [page([message("shared", "当前版本"), message("latest", "最新回复")]), page([message("oldest", "较早回复"), message("shared", "旧版本")])],
    pageParams: [null, "block:2"]
  } });
  const html = renderPanel();
  expect(html.indexOf("较早回复")).toBeLessThan(html.indexOf("当前版本"));
  expect(html.indexOf("当前版本")).toBeLessThan(html.indexOf("最新回复"));
  expect(html).not.toContain("旧版本");
  expect(html.match(/data-timeline-id="shared"/g)).toHaveLength(1);
});

test("the newest page updates a nested child whose activity only exists in older history", () => {
  const nested: SubagentActivity = { agentId: "nested-fixture", name: "较早子智能体", status: "running", available: true, delegation: "保留较早的委派内容。" };
  const latest = page([message("latest", "最新回复")]);
  latest.detail.subagent_updates = {
    "old-nested": { agentId: nested.agentId, name: nested.name, status: "completed", available: true },
    "unloaded-nested": { agentId: "unloaded-fixture", name: "尚未加载的子智能体", status: "failed", available: false }
  };
  const older = page([{
    id: "old-nested", role: "tool", kind: "function_call", questions: [], subagent: nested
  }]);
  queryState({ data: { pages: [latest, older], pageParams: [null, "block:1"] } });
  const html = renderPanel();
  expect(html).toContain('class="subagent-state completed"');
  expect(html).not.toContain('class="subagent-state running"');
  expect(html).not.toContain("尚未加载的子智能体");
  expect(html.indexOf("较早子智能体")).toBeLessThan(html.indexOf("最新回复"));
  expect(nested.status).toBe("running");
  expect(nested.delegation).toBe("保留较早的委派内容。");
});

test("a pending older-page read disables the pagination control", () => {
  queryState({ hasNextPage: true, isFetchingNextPage: true, data: { pages: [page([message("current", "当前回复")])], pageParams: [null] } });
  expect(renderPanel()).toMatch(/<button[^>]*disabled=""[^>]*>较早消息<\/button>/);
});

test("delegation starts collapsed and nested activities expose their own detail entry", () => {
  queryState({ data: { pages: [page([{
    id: "nested-activity", role: "tool", kind: "function_call", questions: [],
    subagent: { agentId: "grandchild-fixture", name: "嵌套示例", status: "unknown", available: true }
  }])], pageParams: [null] } });
  const html = renderPanel();
  expect(html).toMatch(/<details[^>]*class="subagent-delegation"[^>]*>/);
  expect(html).not.toMatch(/<details[^>]*class="subagent-delegation"[^>]*\bopen=/);
  expect(html).toContain("委派内容");
  expect(html).not.toContain("仅检查示例文件。");
  expect(html).toContain("嵌套示例");
  expect(html).toContain('aria-disabled="false"');
  expect(html).not.toContain("返回上一级子智能体");
});

test.each([true, false])("docked=%s advertises its modal state and close action", docked => {
  const html = renderPanel(docked);
  expect(html).toContain('role="dialog"');
  expect(html).toContain(`aria-modal="${!docked}"`);
  expect(html).toContain('aria-label="关闭子智能体详情"');
  expect(html.includes('class="subagent-backdrop"')).toBe(!docked);
});
