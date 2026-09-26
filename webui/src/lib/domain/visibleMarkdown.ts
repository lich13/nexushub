import { fromMarkdown } from "mdast-util-from-markdown";

type Range = { start: number; end: number };
type MarkdownNode = { type: string; children?: MarkdownNode[]; position?: { start: { offset?: number }; end: { offset?: number } } };

function codeRanges(text: string, inline = true): Range[] {
  const ranges: Range[] = [];
  function visit(node: MarkdownNode) {
    if (node.type === "code" || (inline && node.type === "inlineCode")) {
      const start = node.position?.start.offset;
      const end = node.position?.end.offset;
      if (start !== undefined && end !== undefined) ranges.push({ start, end });
    } else node.children?.forEach(visit);
  }
  visit(fromMarkdown(text));
  return ranges;
}

// Keep source slices, rather than serializing Markdown and changing its formatting.
export function memoryMetadataRanges(text: string): Range[] {
  if (!/oai-mem-citation|citation_entries|rollout_ids|memory_citation/i.test(text)) return [];
  try {
    const value = JSON.parse(text);
    if (value && typeof value === "object" && Object.keys(value).length === 1 && Object.prototype.hasOwnProperty.call(value, "memory_citation")) return [{ start: 0, end: text.length }];
  } catch { /* Ordinary Markdown is not a JSON metadata envelope. */ }
  const code = codeRanges(text);
  const protectedAt = (offset: number) => code.some(range => range.start <= offset && offset < range.end);
  const ranges: Range[] = [];
  const stack: Array<{ name: string; start: number }> = [];
  const tags = /(?:<|&lt;)(\/?)(oai-mem-citation|citation_entries|rollout_ids)\s*(?:>|&gt;)/gi;
  for (const match of text.matchAll(tags)) {
    const start = match.index!;
    if (protectedAt(start) && !stack.length) continue;
    const name = match[2].toLowerCase();
    if (!match[1]) stack.push({ name, start });
    else {
      const index = stack.map(item => item.name).lastIndexOf(name);
      if (index < 0) ranges.push({ start, end: start + match[0].length });
      else {
        ranges.push({ start: stack[index].start, end: start + match[0].length });
        stack.splice(index);
      }
    }
  }
  // A streaming/incomplete metadata block must not flash its internal fields.
  for (const item of stack) ranges.push({ start: item.start, end: text.length });

  // Only standalone fields qualify. Prose and literal code examples are untouched.
  const fields = /^[ \t]*(?:["']memory_citation["']|memory_citation)[ \t]*[:=][ \t]*/gim;
  for (const match of text.matchAll(fields)) {
    const start = match.index!;
    if (protectedAt(start + match[0].search(/\S/)) || ranges.some(range => range.start <= start && start < range.end)) continue;
    let end = start + match[0].length;
    let depth = 0;
    let quote = "";
    let escaped = false;
    for (; end < text.length; end++) {
      const char = text[end];
      if (escaped) { escaped = false; continue; }
      if (quote) {
        if (char === "\\") escaped = true;
        else if (char === quote) quote = "";
      } else if ((char === '"' || char === "'") && (depth > 0 || end === start + match[0].length)) quote = char;
      else if (char === "{" || char === "[") depth++;
      else if ((char === "}" || char === "]") && depth > 0) depth--;
      else if (char === "\n" && depth === 0) break;
    }
    ranges.push({ start, end });
  }
  return ranges.sort((a, b) => a.start - b.start || b.end - a.end);
}

export function visibleMarkdown(text: string): string {
  const ranges = memoryMetadataRanges(text);
  if (!ranges.length) return text;
  let result = "";
  let cursor = 0;
  for (const range of ranges) {
    if (range.start > cursor) result += text.slice(cursor, range.start);
    cursor = Math.max(cursor, range.end);
  }
  return (result + text.slice(cursor)).trim();
}

export type InstructionSegment = {
  kind: "markdown" | "instructions";
  id: string;
  text: string;
  lines: number;
  bytes: number;
};

export function isInstructionFileActivity(...values: Array<string | null | undefined>): boolean {
  return values.some(value => value && /(?:^|[\s/\\`"'(=:])agents\.md(?=$|[\s`"'),:;\]}>])/i.test(value));
}

// File labels and native instruction headers qualify; merely mentioning the file does not.
function instructionLabel(line: string): boolean {
  const label = line.trim().replace(/^#{1,6}\s+/, "").replace(/\s+#+$/, "").replace(/^[`*]+|[`*:]+$/g, "");
  return /^(?:[a-z]:[/\\]|[/\\]|\.{1,2}[/\\]|~[/\\])(?:[^<>\n]*[/\\])?agents\.md$/i.test(label)
    || /^(?:[\w.-]+[/\\])+agents\.md$/i.test(label)
    || /^agents\.md(?:\s+instructions(?:\s+for\s+.+)?)?$/i.test(label);
}

export function instructionSegments(text: string): InstructionSegment[] {
  if (!/agents\.md/i.test(text)) return [{ kind: "markdown", id: "text", text, lines: text.split(/\r?\n/).length, bytes: new TextEncoder().encode(text).length }];
  const code = codeRanges(text, false);
  const lines = Array.from(text.matchAll(/[^\n]*(?:\n|$)/g)).filter(match => match[0]);
  const markers = lines.filter(line => !code.some(range => range.start <= line.index! && line.index! < range.end) && instructionLabel(line[0]));
  if (!markers.length) return [{ kind: "markdown", id: "text", text, lines: text.split(/\r?\n/).length, bytes: new TextEncoder().encode(text).length }];
  const result: InstructionSegment[] = [];
  const add = (kind: InstructionSegment["kind"], start: number, end: number) => {
    const value = text.slice(start, end);
    if (!value.trim()) return;
    result.push({ kind, id: `${kind}:${start}`, text: value, lines: value.trimEnd().split(/\r?\n/).length, bytes: new TextEncoder().encode(value).length });
  };
  let cursor = 0;
  for (const marker of markers) {
    const start = marker.index!;
    if (start < cursor) continue;
    add("markdown", cursor, start);
    const depth = marker[0].match(/^ {0,3}(#{1,6})\s/)?.[1].length ?? 0;
    const next = lines.find(line => line.index! > start && !code.some(range => range.start <= line.index! && line.index! < range.end)
      && ((line[0].match(/^ {0,3}(#{1,6})\s/)?.[1].length ?? 7) <= depth || instructionLabel(line[0])));
    let end = next?.index ?? text.length;
    const close = text.indexOf("</INSTRUCTIONS>", start);
    if (close >= 0 && (close < end || /^\s*<INSTRUCTIONS>/i.test(text.slice(start + marker[0].length)))) end = close + "</INSTRUCTIONS>".length;
    add("instructions", start, end);
    cursor = end;
  }
  add("markdown", cursor, text.length);
  return result;
}
