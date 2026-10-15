import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";
import { MessageBlockView } from "./MessageStream";
import { DisclosureRevealContext, DisclosureScope } from "../common/ActivityDetails";
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

test("answered question results keep the original question and options beside escaped answers", () => {
  const block: MessageBlock = {
    id: "answered-question-fixture", role: "assistant", kind: "request_user_input_result", text: "",
    questions: [{
      id: "answered-question-fixture-q1",
      header: "Fixture header <hidden> & literal",
      question: "Fixture question <query> & exact",
      options: [{ label: "Fixture option <alpha>", description: "Fixture description <detail> & exact" }]
    }],
    answers: [{ question_id: "answered-question-fixture-q1", answers: ["Fixture answer <selected> & exact"] }]
  };
  const html = renderToStaticMarkup(<MessageBlockView block={block} />);

  for (const text of [
    "Fixture question &lt;query&gt; &amp; exact",
    "Fixture option &lt;alpha&gt;",
    "Fixture description &lt;detail&gt; &amp; exact",
    "Fixture answer &lt;selected&gt; &amp; exact"
  ]) expect(html).toContain(text);
  expect(html.match(/data-timeline-id=/g) ?? []).toHaveLength(1);
  expect(html).toContain('data-timeline-id="answered-question-fixture"');
  expect(html).not.toMatch(/<(button|input|textarea|select|form)\b/);
});

test("answered question results keep answers when the original questions are absent", () => {
  const html = renderToStaticMarkup(<MessageBlockView block={{
    id: "answer-only-fixture", role: "assistant", kind: "request_user_input_result", text: "",
    questions: [], answers: [{ question_id: "missing-question-fixture", answers: ["Fixture answer without prompt"] }]
  }} />);
  expect(html).toContain("Fixture answer without prompt");
  expect(html).toContain('data-timeline-id="answer-only-fixture"');
});

test("question result without answers keeps its fallback text", () => {
  const html = renderToStaticMarkup(<MessageBlockView block={{
    id: "unanswered-result-fixture", role: "assistant", kind: "request_user_input_result", text: "",
    questions: []
  }} />);
  expect(html).toContain("Questions answered");
  expect(html).toContain('data-timeline-id="unanswered-result-fixture"');
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

test("a search position reveals AGENTS context on its native user message only", () => {
  const targetContext = [
    "# AGENTS.md instructions",
    "<INSTRUCTIONS>",
    "<oai-mem-citation>synthetic memory field</oai-mem-citation>",
    '<external_codex_apps_open_page>{"page_id":"fixture-page"}</external_codex_apps_open_page>',
    "`<oai-mem-citation>literal code example</oai-mem-citation>`",
    "`<external_codex_apps_open_page>{\"page_id\":null}</external_codex_apps_open_page>`",
    "</INSTRUCTIONS>"
  ].join("\n");
  const target: MessageBlock = {
    id: "search-hit-native-fixture", role: "user", kind: "message", questions: [],
    text: targetContext + "\n\nVisible target request"
  };
  const unrelated: MessageBlock = {
    id: "unrelated-native-fixture", role: "user", kind: "message", questions: [],
    text: "# AGENTS.md instructions\n<INSTRUCTIONS>\nUnrelated AGENTS fixture.\n</INSTRUCTIONS>\n\nVisible unrelated request"
  };
  const html = renderToStaticMarkup(
    <DisclosureScope.Provider value="codex:search-thread-fixture">
      <DisclosureRevealContext.Provider value="search-hit-native-fixture">
        <>
          <MessageBlockView block={target} />
          <MessageBlockView block={unrelated} />
        </>
      </DisclosureRevealContext.Provider>
    </DisclosureScope.Provider>
  );
  const disclosures = Array.from(html.matchAll(/<details\b[^>]*>/g), match => match[0]);

  expect(disclosures).toHaveLength(2);
  expect(disclosures[0]).toMatch(/\bopen(?:=|>)/);
  expect(disclosures[1]).not.toMatch(/\bopen(?:=|>)/);
  expect(html).toContain('data-timeline-id="search-hit-native-fixture"');
  expect(html).toContain("literal code example");
  expect(html.match(/page_id/g) ?? []).toHaveLength(1);
  expect(html).not.toContain("synthetic memory field");
  expect(html).not.toContain("fixture-page");
  expect(html).toContain("Visible target request");
  expect(html).not.toContain("Unrelated AGENTS fixture.");
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
