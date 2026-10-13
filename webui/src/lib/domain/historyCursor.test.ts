import { describe, expect, test } from "vitest";
import type { MessageBlock } from "../../types";
import { historyCursorIndex, stableHistoryCursor } from "./historyCursor";

const blocks: MessageBlock[] = [
  { id: "native:call:fixture", role: "tool", kind: "function_call", questions: [] },
  { id: "native:tail:fixture", role: "assistant", kind: "message", questions: [] }
];
const history = { epoch: "source-fixture", revision: "revision-1" };

describe("stable history cursors", () => {
  test("older services keep the original positional cursor", () => {
    expect(stableHistoryCursor({ blocks, before_cursor: "b:42" })).toBe("b:42");
    expect(stableHistoryCursor({ blocks, history: null, before_cursor: "b:42" })).toBe("b:42");
  });

  test("verified history adds the complete opaque ID of the first block", () => {
    expect(stableHistoryCursor({ blocks, history, before_cursor: "b:42" })).toBe("b:42:native:call:fixture");
    expect(stableHistoryCursor({ blocks, history, before_cursor: "b:42:native:call:fixture" })).toBe("b:42:native:call:fixture");
  });

  test("an index change preserves the stable anchor identity", () => {
    expect(stableHistoryCursor({ blocks, history, before_cursor: "b:41" })).toBe("b:41:native:call:fixture");
    expect(stableHistoryCursor({ blocks, history, before_cursor: "b:45" })).toBe("b:45:native:call:fixture");
  });

  test("an empty page or exhausted history cannot invent a new anchor", () => {
    expect(stableHistoryCursor({ blocks: [], history, before_cursor: "b:42" })).toBe("b:42");
    expect(stableHistoryCursor({ blocks, history, before_cursor: null })).toBeNull();
    expect(stableHistoryCursor({ blocks, history })).toBeUndefined();
  });

  test.each(["b:42", "b:42:native:call:fixture"])("%s keeps its numeric hint for earliest-page comparison", cursor => {
    expect(historyCursorIndex(cursor)).toBe(42);
  });

  test.each([undefined, null, "invalid", "b:invalid:fixture"])("missing or invalid hint %s cannot outrank a known earlier page", cursor => {
    expect(historyCursorIndex(cursor)).toBe(Infinity);
  });
});
