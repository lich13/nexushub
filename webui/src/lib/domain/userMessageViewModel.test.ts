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
  const adjacentInstructions = "# AGENTS.md instructions\n\n<INSTRUCTIONS>\n# Rules\nKeep checks.\n# Notes\n保留格式🙂\n</INSTRUCTIONS>";
  test("keeps all native headings inside one disclosure and ordinary tail outside", () => {
    const result = userMessageSegments(instructions + "\n\nActual request");
    expect(result.map(s => s.kind)).toEqual(["instructions", "text"]);
    expect(result[0]).toMatchObject({ text: instructions, lines: 8, bytes: new TextEncoder().encode(instructions).length });
    expect(result[1]).toMatchObject({ text: "\n\nActual request" });
  });
  test.each([
    "<environment_context>fixture</environment_context>\n\nActual request",
    "Actual request on the closing line"
  ])("keeps a same-line tail outside the complete instruction view model: %s", tail => {
    const result = userMessageSegments(adjacentInstructions + tail);
    expect(result.map(segment => segment.kind)).toEqual(["instructions", "text"]);
    expect(result[0]).toMatchObject({ text: adjacentInstructions, lines: 8, bytes: 102 });
    expect(result[1]).toMatchObject({ text: tail });
    expect(userMessageSegments(adjacentInstructions + tail + "\nContinue.")[0].id).toBe(result[0].id);
  });
  test("preserves multiple native envelopes and a reply adjacent to an instruction close", () => {
    const tail = "<environment_context>fixture</environment_context>\n\nVisible request";
    const source = `${adjacentInstructions}${envelope([reply])}\n\n${instructions}${tail}`;
    const result = userMessageSegments(source);
    expect(result.map(segment => segment.kind)).toEqual(["instructions", "reply", "instructions", "text"]);
    expect(result[0]).toMatchObject({ text: adjacentInstructions, lines: 8, bytes: 102 });
    expect(result[1]).toMatchObject({ question: reply.question, answer: reply.answer });
    expect(result[2]).toMatchObject({ text: instructions, lines: 8 });
    expect(result[3]).toMatchObject({ text: tail });
    expect(new Set(result.map(segment => segment.id)).size).toBe(4);
  });
  test("keeps instruction samples literal and preserves adjacent question order", () => {
    const result = userMessageSegments(`${instructions}\n\n${envelope([reply])}\n\n${instructions}`);
    expect(result.map(s => s.kind)).toEqual(["instructions", "reply", "instructions"]);
    expect(new Set(result.map(s => s.id)).size).toBe(3);
    for (const source of [`\`\`\`md\n${instructions}\n\`\`\``, instructions.split("\n").map(line => `> ${line}`).join("\n"), "Please read AGENTS.md."])
      expect(userMessageSegments(source).every(s => s.kind === "text")).toBe(true);
  });
  test.each([
    ["fenced", "```xml\n</INSTRUCTIONS>\n```"],
    ["inline", "`example\n</INSTRUCTIONS>\nend`"],
    ["quoted", "> </INSTRUCTIONS>"]
  ])("ignores a %s closing-tag example before a same-line tail", (_name, example) => {
    const wrapped = `# AGENTS.md instructions\n<INSTRUCTIONS>\n\n${example}\n\n# More rules\nPreserve\n</INSTRUCTIONS>`;
    const tail = "<environment_context>fixture</environment_context>\n\nRequest";
    const result = userMessageSegments(wrapped + tail);
    expect(result.map(s => s.kind)).toEqual(["instructions", "text"]);
    expect(result[0]).toMatchObject({ text: wrapped });
    expect(result[1]).toMatchObject({ text: tail });
  });
});
