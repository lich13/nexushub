import { describe, expect, it } from "vitest";
import { activeUserTimelineId, userTimelineEntries } from "./timelineViewModel";

describe("user instruction timeline", () => {
  it.each(["codex", "claude_code", "grok"] as const)("%s keeps user anchors while excluding assistant, plan and tool items", provider => {
    const userKind = provider === "grok" ? "user_message_chunk" : "user_message";
    const user = { id: "u1", role: "user", kind: userKind, text: "Fixture request" };
    const entries = userTimelineEntries(provider, [
      user, { id: "a1", role: "assistant", kind: "assistant_message", text: "Reply" },
      { id: "tool", role: "tool", kind: "tool_result", text: "Output" },
      { id: "plan", role: "assistant", kind: "plan", text: "Plan" },
      { ...user, id: "u2", text: "Next request" }
    ]);
    expect(entries.map(entry => entry.id)).toEqual(["u1", "u2"]);
  });
  it("deduplicates content chunks by native user identity but keeps repeat messages", () => {
    const userMessage = { id: "message", text: "Request", attachments: [] };
    expect(userTimelineEntries("grok", [
      { id: "chunk1", kind: "user_message_chunk", userMessage },
      { id: "chunk2", kind: "user_message_chunk", userMessage },
      { id: "chunk3", kind: "user_message_chunk", userMessage: { ...userMessage, id: "next" } }
    ]).map(entry => entry.id)).toEqual(["chunk1", "chunk3"]);
  });
  it("previews answers and strips metadata and injected instructions", () => {
    const entries = userTimelineEntries("codex", [
      { id: "instructions", role: "user", text: "# AGENTS.md instructions\n<INSTRUCTIONS>\n# Fixture\nRead this.\n</INSTRUCTIONS>" },
      { id: "reply", role: "user", text: '<send_user_message_question_reply>\n[{"questionItemId":"fixture","question":"Ready?","answer":"Ready"}]\n</send_user_message_question_reply>' },
      { id: "request", role: "user", text: "Request<oai-mem-citation>private-metadata</oai-mem-citation>" }
    ]);
    expect(entries).toEqual([{ id: "reply", title: "Ready", preview: undefined }, { id: "request", title: "Request", preview: undefined }]);
  });
  it("tracks the preceding instruction in long replies and on backward scroll", () => {
    const anchors = [{ id: "first", top: -900 }, { id: "second", top: 500 }, { id: "third", top: 1500 }];
    expect(activeUserTimelineId(anchors, 100)).toBe("first");
    expect(activeUserTimelineId(anchors, 700)).toBe("second");
    expect(activeUserTimelineId(anchors, 100)).toBe("first");
    expect(activeUserTimelineId([], 0)).toBeNull();
    expect(activeUserTimelineId([{ id: "first", top: 200 }], 0)).toBe("first");
  });
});
