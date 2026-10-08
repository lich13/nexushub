import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test, vi } from "vitest";
import type { SubagentActivity, SubagentCollection } from "../../types";
import { SubagentSummary } from "./SubagentSummary";

function collection(statuses: SubagentActivity["status"][], overrides: Partial<SubagentCollection> = {}): SubagentCollection {
  const counts: SubagentCollection["counts"] = { creating: 0, running: 0, completed: 0, failed: 0, interrupted: 0, unknown: 0 };
  const agents = statuses.map((status, index) => {
    counts[status] += 1;
    return { agentId: `child-fixture-${index}`, name: `直属示例 ${index}`, status, available: true };
  });
  return { agents, counts, complete: true, ...overrides };
}

describe("direct subagent summary", () => {
  test("all current states remain distinct and expose the list entry", () => {
    const html = renderToStaticMarkup(<SubagentSummary
      collection={collection(["running", "running", "creating", "completed", "failed", "interrupted", "unknown"])}
      onOpen={vi.fn()}
    />);
    const text = html.replace(/<[^>]+>/g, "");
    expect(text).toContain("2 个运行中");
    for (const label of ["1 正在创建", "1 已完成", "1 失败", "1 已中断", "1 状态未知"]) expect(text).toContain(label);
    expect(html).toContain('aria-label="查看直属子智能体"');
    expect(html).toContain("thread-running-indicator");
  });

  test.each([undefined, null])("a missing collection (%s) prompts an upgrade without inventing zero counts", value => {
    const html = renderToStaticMarkup(<SubagentSummary collection={value} onOpen={vi.fn()} />);
    expect(html).toContain("更新当前机器服务");
    expect(html).toContain('role="status"');
    expect(html).not.toContain("<button");
    expect(html).not.toMatch(/0|暂无子智能体/);
  });

  test("a confirmed empty collection does not add a summary control", () => {
    const html = renderToStaticMarkup(<SubagentSummary collection={collection([])} onOpen={vi.fn()} />);
    expect(html).toBe("");
  });

  test("partial counts retain their warning even when no child was readable", () => {
    const html = renderToStaticMarkup(<SubagentSummary
      collection={collection([], { complete: false, warning: "示例直属记录暂不可读" })} onOpen={vi.fn()}
    />);
    expect(html).toContain("示例直属记录暂不可读");
    expect(html).toContain('role="status"');
    expect(html).not.toContain("更新当前机器服务");
  });

  test("settled children do not display an active spinner or expose delegation content", () => {
    const data = collection(["completed", "failed"]);
    data.agents[0].delegation = "仅供详情显示的示例委派内容。";
    const html = renderToStaticMarkup(<SubagentSummary collection={data} onOpen={vi.fn()} />);
    expect(html).toContain("1 已完成");
    expect(html).toContain("1 失败");
    expect(html).not.toContain("thread-running-indicator");
    expect(html).not.toContain(data.agents[0].delegation);
  });
});
