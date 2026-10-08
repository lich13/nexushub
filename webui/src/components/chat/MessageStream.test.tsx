import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";
import { MessageBlockView } from "./MessageStream";
import type { MessageBlock } from "../../types";

test("questions retain their content and options without reply controls", () => {
  const block: MessageBlock = {
    id: "question", role: "assistant", kind: "request_user_input", text: "",
    questions: [{ id: "q1", question: "Choose scope", options: [{ label: "Small", description: "One module" }, { label: "Full" }] }]
  };
  const html = renderToStaticMarkup(<MessageBlockView block={block} />);
  for (const text of ["Choose scope", "Small", "One module", "Full"]) expect(html).toContain(text);
  expect(html).not.toMatch(/<(button|input|textarea|form)\b/);
});

test("plans remain readable with copy and download but no submission controls", () => {
  const block: MessageBlock = {
    id: "plan", role: "assistant", kind: "plan", text: "<proposed_plan>## Verified plan\nOne step</proposed_plan>", questions: []
  };
  const html = renderToStaticMarkup(<MessageBlockView block={block} />);
  expect(html).toContain("Verified plan");
  expect(html).toContain("One step");
  expect(html).toContain('aria-label="复制计划"');
  expect(html).toContain('aria-label="下载计划 Markdown"');
  expect(html).not.toMatch(/<(input|textarea|form)\b/);
});

test("empty plans do not expose copy or download", () => {
  const block: MessageBlock = {
    id: "empty-plan", role: "assistant", kind: "plan", text: "<proposed_plan> </proposed_plan>", questions: []
  };
  expect(renderToStaticMarkup(<MessageBlockView block={block} />)).not.toContain("plan-actions");
});

test("assistant replies keep their Markdown and copy action without duplicate metadata labels", () => {
  const html = renderToStaticMarkup(<MessageBlockView block={{
    id: "assistant-fixture", role: "assistant", kind: "message", questions: [],
    created_at: "2026-01-01T00:00:00Z",
    text: "## 示例回复\n\n保留 **正文**<oai-mem-citation>fixture metadata</oai-mem-citation>"
  }} />);
  expect(html).toContain("<h2>示例回复</h2>");
  expect(html).toContain("<strong>正文</strong>");
  expect(html).toContain('aria-label="复制回复"');
  expect(html).toContain('data-timeline-id="assistant-fixture"');
  expect(html).not.toContain("chat-meta");
  expect(html).not.toContain("fixture metadata");
});

test("metadata-only assistant content does not expose an empty copy action", () => {
  const html = renderToStaticMarkup(<MessageBlockView block={{
    id: "metadata-fixture", role: "assistant", kind: "message", questions: [],
    text: "<oai-mem-citation>fixture metadata</oai-mem-citation>"
  }} />);
  expect(html).not.toContain('aria-label="复制回复"');
  expect(html).not.toContain("fixture metadata");
});

test("subagent activity takes precedence over the native tool wrapper", () => {
  const html = renderToStaticMarkup(<MessageBlockView block={{
    id: "delegation-fixture", role: "tool", kind: "function_call", tool_name: "spawn_agent",
    text: "native delegation payload", questions: [],
    subagent: { agentId: "child-fixture", name: "示例子智能体", status: "running", available: true }
  }} onOpenSubagent={() => undefined} subagentsSupported={false} />);
  expect(html).toContain("subagent-activity");
  expect(html).toContain("示例子智能体");
  expect(html).toContain("请更新当前机器服务以查看子智能体");
  expect(html).not.toContain("native delegation payload");
  expect(html).not.toContain("tool-cell");
});
