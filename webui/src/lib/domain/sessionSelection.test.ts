import { describe, expect, test } from "vitest";
import { addSessionSelection, clickSessionSelection, emptySessionSelection, syncSessionSelection, type SessionClick, type SessionSelectionState } from "./sessionSelection";
import { claudeResumeCommand } from "./claudeSession";

const visible = ["a", "b", "c", "d", "e"];
const press = (state: SessionSelectionState, key: string, mods: Partial<SessionClick> = {}, order = visible) => clickSessionSelection(state, order, key, { detail: 1, metaKey: false, ctrlKey: false, shiftKey: false, ...mods });
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
  test("Command enters selection, ordinary clicks toggle, Ctrl keeps the context target independent", () => {
    let state = press(emptySessionSelection("local"), "c", { metaKey: true });
    expect(state.selecting).toBe(true);
    state = press(state, "a");
    state = press(state, "c");
    expect(state.keys).toEqual(["a"]);
    expect(press(state, "d", { ctrlKey: true, metaKey: true })).toBe(state);
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
  test("overflow preserves selected items, caps at 100 and explains the limit", () => {
    const state = addSessionSelection(press(emptySessionSelection("local"), "kept", { metaKey: true }), Array.from({ length: 105 }, (_, i) => String(i)));
    expect(state.keys).toHaveLength(100);
    expect(state.keys[0]).toBe("kept");
    expect(state.notice).toContain("100");
    expect(press(state, "kept").keys).toHaveLength(99);
  });
});
test("Claude resume commands quote paths and IDs as literal shell arguments", () => {
  expect(claudeResumeCommand("/fixture/a'b $(printf ignored)", "id'one")).toBe("cd -- '/fixture/a'\\''b $(printf ignored)' && claude --resume 'id'\\''one'");
});
