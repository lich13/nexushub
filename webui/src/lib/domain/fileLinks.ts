import { fromMarkdown } from "mdast-util-from-markdown";

export type FileLinkTarget = { path: string; resolved: boolean };

function normalizePath(path: string): string {
  const parts: string[] = [];
  for (const part of path.split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") parts.pop();
    else parts.push(part);
  }
  return `/${parts.join("/")}`;
}

/** Parse source URLs, never the DOM href (which adds the browser's origin). */
export function fileLinkTarget(href: string, cwd?: string | null): FileLinkTarget | null {
  let source = href.trim();
  if (!source || /[\u0000-\u001f\u007f]/.test(source) || source.startsWith("#") || source.startsWith("//")) return null;
  if (/^file:/i.test(source)) {
    try {
      const url = new URL(source);
      if (url.hostname && url.hostname !== "localhost" || url.username || url.password || url.port || url.search) return null;
      source = url.pathname + url.hash;
    } catch { return null; }
  } else if (/^[a-z][a-z0-9+.-]*:/i.test(source)) {
    return null;
  } else if (!/^(?:\/|\.{1,2}\/|~\/)/.test(source) && !/^[^?#]+(?:\/|\.[\p{L}\d_-]+(?:[:#]|$))/u.test(source)) {
    return null;
  }
  source = source.replace(/#L\d+(?:C\d+)?(?:-L?C?\d+(?:C\d+)?)?$/i, "").replace(/:\d+(?::\d+)?$/, "");
  try { source = decodeURIComponent(source); } catch { /* Literal percent signs are valid filenames. */ }
  if (!source || /[\u0000-\u001f\u007f]/.test(source)) return null;
  if (source.startsWith("/")) return { path: normalizePath(source), resolved: true };
  if (!source.startsWith("~/") && cwd?.startsWith("/") && !/[\u0000-\u001f\u007f]/.test(cwd)) {
    return { path: normalizePath(`${cwd}/${source}`), resolved: true };
  }
  return { path: source, resolved: false };
}

type Node = {
  type: string; value?: string; url?: string; identifier?: string;
  position?: { start: { offset?: number }; end: { offset?: number } };
  children?: Node[];
};
export type FileLinkSpan = { start: number; end: number; href: string; label: string };

/** Preserve plain tool output and literal code; enhance only real Markdown links. */
export function fileLinkSpans(text: string): FileLinkSpan[] {
  const root = fromMarkdown(text) as Node;
  const definitions = new Map<string, string>();
  const spans: FileLinkSpan[] = [];
  const label = (node: Node): string => node.value ?? node.children?.map(label).join("") ?? "";
  function collect(node: Node) {
    if (node.type === "definition" && node.identifier && node.url) definitions.set(node.identifier, node.url);
    node.children?.forEach(collect);
  }
  function visit(node: Node) {
    if (node.type === "code" || node.type === "inlineCode") return;
    const href = node.type === "link" ? node.url : node.type === "linkReference" ? definitions.get(node.identifier ?? "") : undefined;
    const start = node.position?.start.offset, end = node.position?.end.offset;
    if (href && fileLinkTarget(href) && start !== undefined && end !== undefined) {
      spans.push({ start, end, href, label: label(node) });
      return;
    }
    node.children?.forEach(visit);
  }
  collect(root); visit(root);
  return spans;
}
