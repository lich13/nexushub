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

test("plans remain readable without submission controls or pending copy", () => {
  const block: MessageBlock = {
    id: "plan", role: "assistant", kind: "plan", text: "<proposed_plan>## Verified plan\nOne step</proposed_plan>", questions: []
  };
  const html = renderToStaticMarkup(<MessageBlockView block={block} />);
  expect(html).toContain("Verified plan");
  expect(html).toContain("One step");
  expect(html).not.toMatch(/<(button|input|textarea|form)\b/);
});
