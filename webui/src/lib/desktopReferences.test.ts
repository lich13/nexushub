import { describe, expect, test } from "vitest";
import type { DesktopChosenItem } from "./api/desktop";
import {
  editReferenceDraft,
  inlineReferenceQuery,
  insertDesktopReference,
  removeDesktopReference,
  toDesktopReferenceSpans,
  type ReferenceDraft
} from "./desktopReferences";

function chosen(handle: string, name: string, kind = "skill"): DesktopChosenItem {
  return { handle, name, kind };
}

function draft(
  text = "",
  references: DesktopChosenItem[] = [],
  spans: ReferenceDraft["spans"] = []
): ReferenceDraft {
  return { text, references, spans };
}

function span(text: string, token: string, handle: string): ReferenceDraft["spans"][number] {
  const start = text.indexOf(token);
  if (start < 0) throw new Error(`Missing fixture token: ${token}`);
  return { handle, start, end: start + token.length };
}

function thrownMessage(operation: () => unknown): string {
  let error: unknown;
  try {
    operation();
  } catch (caught) {
    error = caught;
  }
  expect(error).toBeInstanceOf(Error);
  return (error as Error).message;
}

describe("desktop reference drafts", () => {
  test("inserts a skill into the selected query and leaves a trailing space for typing", () => {
    const item = chosen("skill-diagram-fixture", "diagram");
    const initial = draft("请运行 $dia");
    const start = initial.text.indexOf("$dia");
    const result = insertDesktopReference(initial, item, start, initial.text.length);

    expect(result.draft.text).toBe("请运行 $diagram ");
    expect(result.draft.references).toEqual([item]);
    expect(result.draft.spans).toEqual([span(result.draft.text, "$diagram", item.handle)]);
    expect(result.caret).toBe(result.draft.text.length);
  });

  test("adds separators around a reference when inserted between ordinary text", () => {
    const item = chosen("app-review-fixture", "review", "app");
    const initial = draft("pleasecheck");
    const result = insertDesktopReference(initial, item, 6, 6);

    expect(result.draft.text).toBe("please @review check");
    expect(result.draft.text[result.draft.spans[0].end]).toBe(" ");
    expect(result.draft.spans).toEqual([span(result.draft.text, "@review", item.handle)]);
  });

  test("keeps same-name references distinct by handle and deduplicates repeated handles", () => {
    const first = chosen("skill-review-a", "review");
    const sameName = chosen("skill-review-b", "review");

    const firstInsert = insertDesktopReference(draft(), first, 0, 0);
    const secondInsert = insertDesktopReference(firstInsert.draft, sameName, firstInsert.caret, firstInsert.caret);

    expect(secondInsert.draft.text).toBe("$review $review ");
    expect(secondInsert.draft.references.map(item => item.handle)).toEqual([first.handle, sameName.handle]);
    expect(secondInsert.draft.spans.map(item => item.handle)).toEqual([first.handle, sameName.handle]);

    const repeatedInsert = insertDesktopReference(secondInsert.draft, first, secondInsert.caret, secondInsert.caret);
    expect(repeatedInsert.draft.references.map(item => item.handle)).toEqual([first.handle, sameName.handle]);
    expect(repeatedInsert.draft.spans.map(item => item.handle)).toEqual([first.handle, sameName.handle, first.handle]);
  });

  test("shifts untouched UTF-16 spans after text is inserted before them", () => {
    const item = chosen("plugin-helper-fixture", "helper", "plugin");
    const originalText = "开头 @helper 结尾";
    const original = draft(originalText, [item], [span(originalText, "@helper", item.handle)]);
    const editedText = `😀 ${originalText}`;

    const edited = editReferenceDraft(original, editedText);

    expect(edited.text).toBe(editedText);
    expect(edited.spans).toEqual([span(editedText, "@helper", item.handle)]);
    expect(edited.spans[0].start - original.spans[0].start).toBe(3);
    expect(edited.references).toEqual([item]);
  });

  test("unbinds only the overwritten occurrence while another span keeps its chip", () => {
    const item = chosen("skill-check-fixture", "check");
    const originalText = "$check and $check";
    const original = draft(originalText, [item], [
      span(originalText, "$check", item.handle),
      { handle: item.handle, start: originalText.lastIndexOf("$check"), end: originalText.length }
    ]);
    const editedText = originalText.replace("$check", "$changed");

    const edited = editReferenceDraft(original, editedText);

    expect(edited.text).toBe(editedText);
    expect(edited.spans).toHaveLength(1);
    expect(edited.text.slice(edited.spans[0].start, edited.spans[0].end)).toBe("$check");
    expect(edited.references).toEqual([item]);
  });

  test("removes every crossed binding and does not restore it after a text undo", () => {
    const first = chosen("skill-first-fixture", "first");
    const second = chosen("app-second-fixture", "second", "app");
    const originalText = "$first between @second";
    const original = draft(originalText, [first, second], [
      span(originalText, "$first", first.handle),
      span(originalText, "@second", second.handle)
    ]);
    const pastedText = "plain replacement";

    const pasted = editReferenceDraft(original, pastedText);
    const undone = editReferenceDraft(pasted, originalText);

    expect(pasted.spans).toEqual([]);
    expect(pasted.references).toEqual([]);
    expect(undone.text).toBe(originalText);
    expect(undone.spans).toEqual([]);
    expect(undone.references).toEqual([]);
  });

  test("preserves a file chip when editing removes its text binding", () => {
    const file = chosen("file-report-fixture", "report.txt", "file");
    const originalText = "参考 @report.txt";
    const original = draft(originalText, [file], [span(originalText, "@report.txt", file.handle)]);

    const edited = editReferenceDraft(original, "参考附件内容");

    expect(edited.spans).toEqual([]);
    expect(edited.references).toEqual([file]);
  });

  test("removes all spans for one handle while preserving other references", () => {
    const repeated = chosen("skill-repeat-fixture", "repeat");
    const kept = chosen("plugin-kept-fixture", "kept", "plugin");
    const text = "$repeat @kept $repeat";
    const initial = draft(text, [repeated, kept], [
      span(text, "$repeat", repeated.handle),
      span(text, "@kept", kept.handle),
      { handle: repeated.handle, start: text.lastIndexOf("$repeat"), end: text.length }
    ]);

    const removed = removeDesktopReference(initial, repeated.handle);

    expect(removed.text).not.toContain("$repeat");
    expect(removed.text).toContain("@kept");
    expect(removed.references.map(item => item.handle)).toEqual([kept.handle]);
    expect(removed.spans).toHaveLength(1);
    expect(removed.spans[0].handle).toBe(kept.handle);
    expect(removed.text.slice(removed.spans[0].start, removed.spans[0].end)).toBe("@kept");
  });

  test("converts Chinese, emoji and multiline UTF-16 ranges to UTF-8 byte offsets", () => {
    const skill = chosen("skill-unicode-fixture", "技能");
    const plugin = chosen("plugin-unicode-fixture", "工具", "plugin");
    const text = "猫😀\n$技能 @工具";
    const initial = draft(text, [skill, plugin], [
      span(text, "$技能", skill.handle),
      span(text, "@工具", plugin.handle)
    ]);

    expect(toDesktopReferenceSpans(initial)).toEqual([
      { handle: skill.handle, start: 8, end: 15 },
      { handle: plugin.handle, start: 16, end: 23 }
    ]);
  });

  test("returns only opaque handle and byte ranges in the external payload", () => {
    const itemWithPrivateMetadata = {
      ...chosen("plugin-payload-fixture", "helper", "plugin"),
      path: "example.invalid/private-fixture"
    };
    const text = "@helper";
    const initial = draft(text, [itemWithPrivateMetadata], [span(text, "@helper", itemWithPrivateMetadata.handle)]);
    const payload = toDesktopReferenceSpans(initial);

    expect(payload).toEqual([{ handle: itemWithPrivateMetadata.handle, start: 0, end: 7 }]);
    expect(JSON.stringify(payload)).not.toContain(itemWithPrivateMetadata.path);
    expect(Object.keys(payload[0]).sort()).toEqual(["end", "handle", "start"]);
  });

  test("allows 32 references and rejects a new distinct reference with a stable fixed error", () => {
    const files = Array.from({ length: 31 }, (_, index) => chosen(`file-${index}-fixture`, `file-${index}.txt`, "file"));
    const lastAllowed = chosen("skill-last-reference-fixture", "last");
    const full = insertDesktopReference(draft("", files), lastAllowed, 0, 0).draft;

    expect(full.references).toHaveLength(32);
    expect(full.spans).toHaveLength(1);

    const overflowItem = chosen("skill-overflow-reference-fixture", "overflow");
    const overflow = () => insertDesktopReference(full, overflowItem, full.text.length, full.text.length);
    const firstError = thrownMessage(overflow);
    const secondError = thrownMessage(overflow);

    expect(firstError).toBe(secondError);
  });

  test("allows 32 spans for one handle and rejects a 33rd span with a stable fixed error", () => {
    const item = chosen("skill-many-spans-fixture", "many");
    const token = "$many";
    const text = Array.from({ length: 31 }, () => token).join(" ");
    const spans = Array.from({ length: 31 }, (_, index) => {
      const start = index * (token.length + 1);
      return { handle: item.handle, start, end: start + token.length };
    });
    const thirtyOne = draft(text, [item], spans);
    const thirtyTwo = insertDesktopReference(thirtyOne, item, text.length, text.length).draft;

    expect(thirtyTwo.spans).toHaveLength(32);
    expect(thirtyTwo.references).toEqual([item]);

    const overflow = () => insertDesktopReference(thirtyTwo, item, thirtyTwo.text.length, thirtyTwo.text.length);
    const firstError = thrownMessage(overflow);
    const secondError = thrownMessage(overflow);

    expect(firstError).toBe(secondError);
  });
});

describe("inline desktop reference queries", () => {
  test.each([
    { text: "$diagram", kind: "skills", query: "diagram" },
    { text: "please @review-bot", kind: "mentions", query: "review-bot" },
    { text: "第一行\n  $绘图", kind: "skills", query: "绘图" },
    { text: "命令\t@助手", kind: "mentions", query: "助手" }
  ])("matches a $ or @ token at a line or whitespace boundary: $text", ({ text, kind, query }) => {
    const start = text.lastIndexOf(kind === "skills" ? "$" : "@");
    expect(inlineReferenceQuery(text, text.length, text.length)).toEqual({
      kind,
      query,
      start,
      end: text.length
    });
  });

  test.each([
    "person@example.invalid",
    "https://example.invalid/@plugin",
    "folder/$skill",
    "price$10"
  ])("does not match an email or ordinary address: %s", text => {
    expect(inlineReferenceQuery(text, text.length, text.length)).toBeNull();
  });

  test("does not open for a non-collapsed selection", () => {
    const text = "try $diagram";
    expect(inlineReferenceQuery(text, 4, text.length)).toBeNull();
  });
});
