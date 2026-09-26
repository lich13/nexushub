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
