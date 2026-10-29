import { describe, expect, test } from "vitest";
import { compareSessionMessageTime, addSessionSelection, clickSessionSelection, emptySessionSelection, syncSessionSelection, type SessionClick, type SessionSelectionState } from "./sessionSelection";
import { claudeResumeCommand } from "./claudeSession";

const visible = ["a", "b", "c", "d", "e"];
const press = (
  state: SessionSelectionState,
  key: string,
  mods: Partial<SessionClick> = {},
  order = visible,
  activeKey?: string | null,
) =>
  clickSessionSelection(
    state,
    order,
    key,
    { detail: 1, metaKey: false, ctrlKey: false, shiftKey: false, ...mods },
    activeKey,
  );
describe("session selection", () => {
  test("ordinary selection anchors a later additive Shift range and repeated ranges keep the anchor", () => {
    let state = press(emptySessionSelection("local"), "b");
    expect(state.selecting).toBe(false);
    state = press(state, "d", { shiftKey: true });
    expect(state.keys).toEqual(["b", "c", "d"]);
    state = press(state, "a", { shiftKey: true });
    expect(state.keys).toEqual(["b", "c", "d", "a"]);
    expect(state.anchor).toBe("b");
  });
  test("Finder Command-click toggles selection, promotes active session, plain click exits multi-select, and Ctrl stays independent", () => {
    let state = press(emptySessionSelection("local"), "c", { metaKey: true }, visible, "a");
    expect(state.selecting).toBe(true);
    expect(state.keys).toEqual(["a", "c"]);
    state = press(state, "e", { metaKey: true }, visible, "a");
    expect(state.keys).toEqual(["a", "c", "e"]);
    state = press(state, "c", { metaKey: true }, visible, "a");
    expect(state.keys).toEqual(["a", "e"]);
    expect(press(state, "d", { ctrlKey: true, metaKey: true })).toBe(state);
    state = press(state, "d");
    expect(state.selecting).toBe(false);
    expect(state.keys).toEqual([]);
    expect(state.anchor).toBe("d");
  });
  test("Shift-click falls back to activeKey when no explicit anchor was set yet", () => {
    const state = press(emptySessionSelection("local"), "d", { shiftKey: true }, visible, "b");
    expect(state.selecting).toBe(true);
    expect(state.keys).toEqual(["b", "c", "d"]);
    expect(state.anchor).toBe("b");
  });
  test("keyboard clicks ignore modifiers and Shift without a visible anchor selects only its target", () => {
    expect(press(emptySessionSelection("local"), "b", { detail: 0, metaKey: true, shiftKey: true, ctrlKey: true }).selecting).toBe(false);
    const state = press(emptySessionSelection("local"), "d", { shiftKey: true });
    expect(state.keys).toEqual(["d"]);
    expect(state.anchor).toBe("d");
  });
  test("polling removes missing selections, follows visible order and never selects incoming rows", () => {
    let state = press(emptySessionSelection("local"), "b", { metaKey: true });
    const reordered = ["e", "b", "d", "c", "a", "new"];
    state = syncSessionSelection(state, "local", reordered);
    expect(state.keys).toEqual(["b"]);
    state = press(state, "c", { shiftKey: true }, reordered);
    expect(state.keys).toEqual(["b", "d", "c"]);
    state = syncSessionSelection(state, "local", ["e", "c", "new"]);
    expect(state.anchor).toBeNull();
    expect(press(state, "e", { shiftKey: true }).keys).toEqual(["c", "e"]);
  });
  test("all scopes and exit reset the anchor, selection and limit notice", () => {
    const state = press(emptySessionSelection("local"), "b", { metaKey: true });
    for (const scope of ["remote", "other-provider", "other-filter"]) expect(syncSessionSelection(state, scope, visible)).toEqual(emptySessionSelection(scope));
    expect(emptySessionSelection(state.scope).anchor).toBeNull();
  });
  test("selection preserves all explicit items beyond one hundred and Command-click toggles an item off", () => {
    const state = addSessionSelection(press(emptySessionSelection("local"), "kept", { metaKey: true }), Array.from({ length: 105 }, (_, i) => String(i)));
    expect(state.keys).toHaveLength(106);
    expect(state.keys[0]).toBe("kept");
    expect(state.notice).toBeNull();
    expect(press(state, "kept", { metaKey: true }).keys).toHaveLength(105);
  });
});
test("Claude resume commands quote paths and IDs as literal shell arguments", () => {
  expect(claudeResumeCommand("/fixture/a'b $(printf ignored)", "id'one")).toBe("cd -- '/fixture/a'\\''b $(printf ignored)' && claude --resume 'id'\\''one'");
});

test("provider lists sort known conversation times newest first with stable unknowns", () => {
  const rows = [
    { id: "first", sessionKey: "z", updatedAt: "2026-10-01T11:00:00+08:00" },
    { id: "unknown-b", updatedAt: "invalid" },
    { id: "second", sessionKey: "a", updatedAt: "2026-10-01T04:00:00Z" },
    { id: "unknown-a", updatedAt: null }
  ];
  expect(rows.sort(compareSessionMessageTime).map(row => row.id)).toEqual(["second", "first", "unknown-b", "unknown-a"]);
});
