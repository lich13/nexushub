import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test, vi } from "vitest";
import type { MessageBlock, SubagentActivity } from "../../types";
import { SubagentActivityRow } from "./SubagentActivity";

const agent: SubagentActivity = {
  agentId: "child-fixture", name: "检查示例", status: "completed", available: true
};
const block = (overrides: Partial<SubagentActivity> = {}): MessageBlock => ({
  id: "activity-fixture", role: "tool", kind: "function_call", questions: [],
  subagent: { ...agent, ...overrides }
});

describe("subagent activity presentation", () => {
  test.each([
    ["running", "运行中"], ["completed", "已完成"], ["failed", "失败"],
    ["interrupted", "已中断"], ["unknown", "状态未知"]
  ] as const)("renders the %s state without inferring another outcome", (status, label) => {
    const html = renderToStaticMarkup(<SubagentActivityRow block={block({ status })} onOpen={vi.fn()} />);
    expect(html).toContain("检查示例");
    expect(html).toContain(label);
    expect(html).toContain('data-timeline-id="activity-fixture"');
    expect(html).toContain('aria-disabled="false"');
    expect(html.includes("thread-running-indicator")).toBe(status === "running");
    expect(html).not.toContain("execution-group");
  });

  test("keeps an unavailable record and its explanation readable", () => {
    const html = renderToStaticMarkup(<SubagentActivityRow block={block({
      available: false, unavailableReason: "子智能体记录已移除"
    })} onOpen={vi.fn()} />);
    expect(html).toContain('aria-disabled="true"');
    expect(html).toContain("检查示例");
    expect(html).toContain("子智能体记录已移除");
    expect(html).not.toContain("lucide-chevron-right");
  });

  test("a machine without the capability explains why detail cannot open", () => {
    const html = renderToStaticMarkup(<SubagentActivityRow block={block()} onOpen={vi.fn()} supported={false} />);
    expect(html).toContain('aria-disabled="true"');
    expect(html).toContain("请更新当前机器服务以查看子智能体");
    expect(html).not.toContain("lucide-chevron-right");
  });

  test.each([null, undefined, ""])("a missing native agent identity (%s) cannot open detail", agentId => {
    const html = renderToStaticMarkup(<SubagentActivityRow block={block({ agentId })} onOpen={vi.fn()} />);
    expect(html).toContain('aria-disabled="true"');
  });

  test("a read-only row without a detail handler remains readable", () => {
    const html = renderToStaticMarkup(<SubagentActivityRow block={block()} />);
    expect(html).toContain("检查示例");
    expect(html).toContain('aria-disabled="true"');
  });

  test("structured memory metadata is absent from the displayed agent name", () => {
    const html = renderToStaticMarkup(<SubagentActivityRow block={block({
      name: "检查示例<oai-mem-citation>fixture internal metadata</oai-mem-citation>"
    })} onOpen={vi.fn()} />);
    expect(html).toContain("检查示例");
    expect(html).not.toContain("fixture internal metadata");
    expect(html).not.toContain("oai-mem-citation");
  });
});
