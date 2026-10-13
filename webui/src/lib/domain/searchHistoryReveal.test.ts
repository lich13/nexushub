import { describe, expect, it } from "vitest";
import type { MessageBlock } from "../../types";
import { visibleConversationBlocksForHistory } from "./conversationViewModel";

function message(id: string): MessageBlock {
  return { id, role: "assistant", kind: "message", text: `Fixture ${id}`, questions: [] };
}

describe("search results in compact conversation history", () => {
  it("keeps one older result in place while folding other old messages and retaining the newest 60", () => {
    const blocks = Array.from({ length: 67 }, (_, index) => message(`chat-${String(index).padStart(3, "0")}`));
    const targetId = "chat-000";

    const visible = visibleConversationBlocksForHistory(blocks, false, undefined, undefined, targetId);
    const ids = visible.map(block => block.id);

    expect(ids[0]).toBe(targetId);
    expect(ids[1]).toBe("chat-history-collapsed");
    expect(ids).not.toContain("chat-003");
    expect(ids.slice(-60)).toEqual(blocks.slice(-60).map(block => block.id));
    expect(ids.indexOf(targetId)).toBeLessThan(ids.indexOf("chat-history-collapsed"));
  });
});
