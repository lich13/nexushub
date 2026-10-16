import type { DesktopChosenItem, DesktopReferenceSpan } from "./api/desktop";

// Editing uses textarea UTF-16 positions. Only the transport conversion emits
// UTF-8 offsets; neither representation contains a native path.
export type ReferenceDraft = { text: string; references: DesktopChosenItem[]; spans: DesktopReferenceSpan[] };
export const emptyReferenceDraft = (): ReferenceDraft => ({ text: "", references: [], spans: [] });
const inlineKind = (kind: string) => ["skill", "app", "plugin"].includes(kind);

function replace(draft: ReferenceDraft, start: number, end: number, value: string): ReferenceDraft {
  const removed = new Set<string>();
  const delta = value.length - (end - start);
  const spans = draft.spans.flatMap(span => {
    if (span.end <= start) return [span];
    if (span.start >= end) return [{ ...span, start: span.start + delta, end: span.end + delta }];
    removed.add(span.handle);
    return [];
  });
  return {
    text: draft.text.slice(0, start) + value + draft.text.slice(end), spans,
    references: draft.references.filter(item => !inlineKind(item.kind) || !removed.has(item.handle) || spans.some(span => span.handle === item.handle))
  };
}

export function editReferenceDraft(draft: ReferenceDraft, text: string): ReferenceDraft {
  if (draft.text === text) return draft;
  let start = 0;
  while (start < draft.text.length && start < text.length && draft.text[start] === text[start]) start++;
  let oldEnd = draft.text.length, newEnd = text.length;
  while (oldEnd > start && newEnd > start && draft.text[oldEnd - 1] === text[newEnd - 1]) { oldEnd--; newEnd--; }
  return replace(draft, start, oldEnd, text.slice(start, newEnd));
}

export function insertDesktopReference(draft: ReferenceDraft, item: DesktopChosenItem, start: number, end: number): { draft: ReferenceDraft; caret: number } {
  if (!inlineKind(item.kind) || start < 0 || end < start || end > draft.text.length) throw new Error("无法插入所选引用");
  const label = `${item.kind === "skill" ? "$" : "@"}${item.name}`;
  const leading = start > 0 && !/\s/.test(draft.text[start - 1]) ? " " : "";
  const trailing = /\s/.test(draft.text[end] ?? "") ? "" : " ";
  const next = replace(draft, start, end, leading + label + trailing);
  const references = next.references.some(value => value.handle === item.handle)
    ? next.references.map(value => value.handle === item.handle ? item : value) : [...next.references, item];
  if (references.length > 32 || next.spans.length >= 32) throw new Error("引用数量超出限制");
  const spanStart = start + leading.length;
  return {
    draft: { ...next, references, spans: [...next.spans, { handle: item.handle, start: spanStart, end: spanStart + label.length }].sort((a, b) => a.start - b.start) },
    caret: spanStart + label.length + trailing.length
  };
}

export function removeDesktopReference(draft: ReferenceDraft, handle: string): ReferenceDraft {
  let next = draft;
  for (const span of draft.spans.filter(span => span.handle === handle).sort((a, b) => b.start - a.start)) {
    next = replace(next, span.start, span.end, "");
  }
  return { ...next, references: next.references.filter(item => item.handle !== handle) };
}

export function inlineReferenceQuery(text: string, start: number, end: number) {
  if (start !== end || start < 0 || start > text.length) return null;
  const match = /(?:^|\s)([$@])([^\s$@]*)$/.exec(text.slice(0, start));
  if (!match) return null;
  const tokenStart = start - match[2].length - 1;
  let tokenEnd = start;
  while (tokenEnd < text.length && !/\s/.test(text[tokenEnd])) tokenEnd++;
  return { kind: match[1] === "$" ? "skills" as const : "mentions" as const, query: match[2], start: tokenStart, end: tokenEnd };
}

export function toDesktopReferenceSpans(draft: ReferenceDraft): DesktopReferenceSpan[] {
  const encoder = new TextEncoder();
  return draft.spans.map(span => ({
    handle: span.handle,
    start: encoder.encode(draft.text.slice(0, span.start)).length,
    end: encoder.encode(draft.text.slice(0, span.end)).length
  }));
}
