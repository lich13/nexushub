import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";
import { UserMessage } from "./UserMessage";
import { MessageBlockView } from "../chat/MessageStream";

test("user content is literal text with line breaks and no Markdown headings", () => {
  const text = "# Heading\n  - list\n```js\nconsole.log(1)\n```";
  const html = renderToStaticMarkup(<UserMessage text={text} />);
  expect(html).toContain(text);
  expect(html).not.toMatch(/<(h1|h2|ul|pre)\b/);
  expect(html).not.toContain("chat-meta");
});

test("image-only messages have an attachment area and no empty bubble", () => {
  const html = renderToStaticMarkup(<UserMessage message={{ id: "m", text: "", attachments: [{ id: "i", name: "sample.png", kind: "image", reason: "原附件已不存在或无法读取" }] }} />);
  expect(html).toContain("消息附件");
  expect(html).toContain("sample.png");
  expect(html).not.toContain("user-message-bubble");
});

test("Codex user messages with proposed-plan examples remain user text", () => {
  const html = renderToStaticMarkup(<MessageBlockView block={{ id: "m", role: "user", kind: "message", questions: [], text: "Example: <proposed_plan># text</proposed_plan>" }} />);
  expect(html).toContain("user-message-bubble");
  expect(html).not.toContain("plan-cell");
});

test("normalized body wins over legacy envelope and attachments stay outside the bubble", () => {
  const html = renderToStaticMarkup(<MessageBlockView block={{ id: "m", role: "user", kind: "message", questions: [], text: "legacy envelope", user_message: { id: "m", text: "Request only", attachments: [{ id: "i", name: "sample.png", kind: "image", reason: "Missing" }] } }} />);
  expect(html).not.toContain("legacy envelope");
  expect(html.indexOf("user-attachments")).toBeLessThan(html.indexOf("user-message-bubble"));
  expect(html).toContain("Request only");
});

test("legacy Codex instruction messages default closed and keep following request visible", () => {
  const text = "# AGENTS.md instructions\n<INSTRUCTIONS>\n# Rules\nGuidance\n</INSTRUCTIONS>\n\nRequest";
  const html = renderToStaticMarkup(<MessageBlockView block={{ id: "legacy", role: "user", kind: "message", questions: [], text }} />);
  expect(html).toContain("user-instructions");
  expect(html).not.toMatch(/<details[^>]*\bopen=/);
  expect(html).not.toContain("Guidance");
  expect(html).toContain("Request");
});

test("native question reply renders a question and answer with an answer-only copy action", () => {
  const text = '<send_user_message_question_reply>[{"questionItemId":"example","question":"Please confirm","answer":"Confirmed"}]</send_user_message_question_reply>';
  const html = renderToStaticMarkup(<UserMessage text={text} activityId="reply" />);
  expect(html).toContain("user-question-context");
  expect(html).toContain("Please confirm");
  expect(html).toContain("Confirmed");
  expect(html).toContain('aria-label="复制回答"');
  expect(html).not.toContain("questionItemId");
  expect(html).not.toContain("send_user_message_question_reply");
});
