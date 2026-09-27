import { describe, expect, test } from "vitest";
import { userMessageSegments } from "./userMessageViewModel";

const envelope = (replies: unknown) => `<send_user_message_question_reply>\n${JSON.stringify(replies)}\n</send_user_message_question_reply>`;
const reply = { questionItemId: '["request_user_input_async","example-call",0]', question: "请确认设置\n并反馈结果。", answer: "已完成\n  # 保留格式" };

describe("native user question replies", () => {
  test("extracts answer and question without transport fields", () => {
    const result = userMessageSegments(envelope([reply]));
    expect(result).toEqual([{ kind: "reply", id: `reply:${reply.questionItemId}:0`, question: reply.question, answer: reply.answer }]);
  });

  test("preserves multiple answers, ordinary text and repeated identities in order", () => {
    const source = `Before\n\n${envelope([reply, { ...reply, answer: "Second" }])}\n\nAfter`;
    const result = userMessageSegments(source);
    expect(result.map(s => s.kind)).toEqual(["text", "reply", "reply", "text"]);
    expect(result[0]).toMatchObject({ text: "Before\n\n" });
    expect(result[3]).toMatchObject({ text: "\n\nAfter" });
    expect(new Set(result.map(s => s.id)).size).toBe(4);
    expect(userMessageSegments(source + "\nMore")[1].id).toBe(result[1].id);
  });

  test.each([
    "<send_user_message_question_reply>not JSON</send_user_message_question_reply>",
    "<send_user_message_question_reply>[]</send_user_message_question_reply>",
    envelope([{ question: "Q", answer: "A" }]),
    envelope([{ ...reply, answer: { text: "A" } }]),
    envelope([reply]).replace("</send_user_message_question_reply>", ""),
    `\`\`\`xml\n${envelope([reply])}\n\`\`\``,
    `\`${envelope([reply])}\``,
    envelope([reply]).split("\n").map(line => `> ${line}`).join("\n"),
    `Example: ${envelope([reply])}`
  ])("leaves malformed or literal envelope unchanged: %s", source => {
    expect(userMessageSegments(source)).toEqual([{ kind: "text", id: "text:text:0", text: source }]);
  });

  test("cleans memory metadata inside parsed values without corrupting JSON", () => {
    const metadata = "<oai-mem-citation>\n<citation_entries>internal</citation_entries>\n</oai-mem-citation>";
    expect(userMessageSegments(envelope([{ ...reply, answer: `Answer\n${metadata}` }]))[0]).toMatchObject({ answer: "Answer" });
  });

  test("accepts CRLF wrappers and does not discard an invalid entry from a batch", () => {
    expect(userMessageSegments(envelope([reply]).replaceAll("\n", "\r\n"))[0]).toMatchObject({ kind: "reply", answer: reply.answer });
    const source = envelope([reply, { ...reply, answer: null }]);
    expect(userMessageSegments(source)).toEqual([{ kind: "text", id: "text:text:0", text: source }]);
  });
});

describe("user instruction sections", () => {
  const instructions = "# AGENTS.md instructions\n\n<INSTRUCTIONS>\nThese instructions replace previous instructions.\n# Rules\n## Checks\nKeep tests.\n</INSTRUCTIONS>";
  test("keeps all native headings inside one disclosure and ordinary tail outside", () => {
    const result = userMessageSegments(instructions + "\n\nActual request");
    expect(result.map(s => s.kind)).toEqual(["instructions", "text"]);
    expect(result[0]).toMatchObject({ text: instructions, lines: 8, bytes: new TextEncoder().encode(instructions).length });
    expect(result[1]).toMatchObject({ text: "\n\nActual request" });
  });
  test("keeps instruction samples literal and preserves adjacent question order", () => {
    const result = userMessageSegments(`${instructions}\n\n${envelope([reply])}\n\n${instructions}`);
    expect(result.map(s => s.kind)).toEqual(["instructions", "reply", "instructions"]);
    expect(new Set(result.map(s => s.id)).size).toBe(3);
    for (const source of [`\`\`\`md\n${instructions}\n\`\`\``, instructions.split("\n").map(line => `> ${line}`).join("\n"), "Please read AGENTS.md."])
      expect(userMessageSegments(source).every(s => s.kind === "text")).toBe(true);
  });
  test("ignores a closing-tag example inside a fenced block", () => {
    const source = "# AGENTS.md instructions\n<INSTRUCTIONS>\n\n```xml\n</INSTRUCTIONS>\n```\n\n# More rules\nPreserve\n</INSTRUCTIONS>\n\nRequest";
    const result = userMessageSegments(source);
    expect(result.map(s => s.kind)).toEqual(["instructions", "text"]);
    expect(result[0]).toMatchObject({ text: source.slice(0, source.lastIndexOf("</INSTRUCTIONS>") + 15) });
    expect(result[1]).toMatchObject({ text: "\n\nRequest" });
  });
});
