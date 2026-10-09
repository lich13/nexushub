import { Children, isValidElement, type ComponentProps, type MouseEvent, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, test, vi } from "vitest";
import { UserMessage } from "./UserMessage";
import { CopyReplyButton } from "./CopyReplyButton";
import { DisclosureScope } from "./ActivityDetails";
import { MessageBlockView } from "../chat/MessageStream";

const observedDisclosures = vi.hoisted(() => ({ elements: [] as ReactElement<ComponentProps<"details">>[] }));

// Observe the actual event handlers without replacing disclosure state behavior.
vi.mock("./ActivityDetails", async importOriginal => {
  const actual = await importOriginal<typeof import("./ActivityDetails")>();
  return {
    ...actual,
    ActivityDetails: (props: ComponentProps<typeof actual.ActivityDetails>) => {
      const element = actual.ActivityDetails(props);
      observedDisclosures.elements.push(element);
      return element;
    }
  };
});

const adjacentInstructions = "# AGENTS.md instructions\n\n<INSTRUCTIONS>\n# Rules\nKeep checks.\n# Notes\n保留格式🙂\n</INSTRUCTIONS>";

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

test("internal page wrappers are removed from user messages and wrapper-only bubbles", () => {
  const page = '<external_codex_apps_open_page>{"page_id":null}</external_codex_apps_open_page>';
  const mixed = renderToStaticMarkup(<MessageBlockView block={{ id: "mixed-page-fixture", role: "user", kind: "message", questions: [], text: `Continue ${page} please` }} />);
  expect(mixed).toContain("user-message-bubble");
  expect(mixed).toContain("Continue  please");
  expect(mixed).toContain('data-timeline-id="mixed-page-fixture"');
  expect(mixed).not.toContain("external_codex_apps_open_page");
  expect(mixed).not.toContain("page_id");

  const internalOnly = renderToStaticMarkup(<MessageBlockView block={{ id: "internal-page-fixture", role: "user", kind: "message", questions: [], text: page }} />);
  expect(internalOnly).toBe("");
  expect(internalOnly).not.toContain("user-message-bubble");
  expect(internalOnly).not.toContain("data-timeline-id");

  expect(renderToStaticMarkup(<CopyReplyButton text={page} />)).toBe("");
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

test("same-line metadata follows a closed disclosure with the full line and byte summary", () => {
  const text = adjacentInstructions + "<environment_context>fixture</environment_context>\n\nVisible request";
  const html = renderToStaticMarkup(<MessageBlockView block={{ id: "adjacent-instruction-fixture", role: "user", kind: "message", questions: [], text }} />);
  expect(html.match(/<details\b/g)).toHaveLength(1);
  expect(html).not.toMatch(/<details[^>]*\bopen(?:=|\s|>)/);
  expect(html).toContain("8 行 · 102 字节");
  expect(html).not.toContain("# Rules");
  expect(html).not.toContain("Keep checks.");
  expect(html).not.toContain("保留格式🙂");
  expect(html).toContain("&lt;environment_context&gt;fixture&lt;/environment_context&gt;");
  expect(html).toContain("Visible request");
});

test("a user's expanded instruction disclosure survives a refreshed native message", () => {
  const renderMessage = (request: string, activityId: string, messageId = "stable-instruction-fixture") => {
    observedDisclosures.elements = [];
    return renderToStaticMarkup(<DisclosureScope.Provider value="instruction-refresh-fixture">
      <UserMessage message={{ id: messageId, text: adjacentInstructions + request, attachments: [] }} activityId={activityId} />
    </DisclosureScope.Provider>);
  };
  const initialHtml = renderMessage("First request", "initial-activity-fixture");
  expect(initialHtml).not.toMatch(/<details[^>]*\bopen(?:=|\s|>)/);
  expect(initialHtml).not.toContain("保留格式🙂");
  expect(initialHtml).toContain("First request");
  expect(observedDisclosures.elements).toHaveLength(1);

  const summary = Children.toArray(observedDisclosures.elements[0].props.children)
    .find(child => isValidElement(child) && child.type === "summary");
  if (!isValidElement<ComponentProps<"summary">>(summary)) throw new Error("Instruction summary is missing");
  expect(summary.props.onClick).toBeTypeOf("function");
  const preventDefault = vi.fn();
  summary.props.onClick!({ target: { closest: () => null }, preventDefault } as unknown as MouseEvent<HTMLElement>);
  expect(preventDefault).toHaveBeenCalledOnce();

  const refreshedHtml = renderMessage("Updated request", "refreshed-activity-fixture");
  expect(refreshedHtml).toMatch(/<details[^>]*\bopen(?:=|\s|>)/);
  expect(refreshedHtml).toContain("8 行 · 102 字节");
  expect(refreshedHtml).toContain("Keep checks.");
  expect(refreshedHtml).toContain("保留格式🙂");
  expect(refreshedHtml).toContain("Updated request");
  expect(refreshedHtml).not.toContain("First request");

  const otherMessage = renderMessage("Other request", "refreshed-activity-fixture", "different-instruction-fixture");
  expect(otherMessage).not.toMatch(/<details[^>]*\bopen(?:=|\s|>)/);
  expect(otherMessage).not.toContain("保留格式🙂");
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
