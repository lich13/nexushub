import { RENDER_CACHE_BUDGET } from "../../lib/domain/resourceBudgets";
import { Check, Copy } from "lucide-react";
import { memo, useContext, useMemo, useState, type ComponentProps, type ReactElement } from "react";
import Markdown, { defaultUrlTransform, type Options } from "react-markdown";
import remarkGfm from "remark-gfm";
import { instructionSegments, visibleMarkdown } from "../../lib/domain/visibleMarkdown";
import { ActivityDetails, DisclosureRevealContext } from "./ActivityDetails";
import { matchesTimelinePosition } from "../../lib/domain/timelineViewModel";
import { fileLinkTarget } from "../../lib/domain/fileLinks";
import { FilePathLink } from "./FilePathLink";

type Element = { tagName?: string; children?: Element[] };

type RemarkPlugin = Extract<NonNullable<Options["remarkPlugins"]>[number], (...args: never[]) => unknown>;
type MarkdownParser = NonNullable<ThisParameterType<RemarkPlugin>["parser"]>;
const compiledMarkdown = new Map<string, { json: string; bytes: number; element?: ReactElement }>();
let compiledMarkdownBytes = 0;
const compiledMarkdownLimit = RENDER_CACHE_BUDGET.markdownBytes;

// Cache the complete, context-free syntax pipeline across thread remounts.
// react-markdown's URL handling and React components still run per render on
// a fresh tree; no React owner, component state, or resolved path is retained.
const reuseMarkdownParse: RemarkPlugin = function () {
  const parse = this.parser!;
  this.parser = (text, file) => compiledMarkdown.has(text)
    ? { type: "root", children: [] } as ReturnType<MarkdownParser>
    : parse(text, file);
};

// On a hit the ordinary transforms receive an empty root, then this final
// transform restores an independent copy of the complete compiled tree.
const reuseMarkdownTree: RemarkPlugin = function () {
  return (tree, file) => {
    const text = typeof file.value === "string" ? file.value : null;
    if (text === null) return tree;
    const cached = compiledMarkdown.get(text);
    if (cached) {
      compiledMarkdown.delete(text);
      compiledMarkdown.set(text, cached);
      return JSON.parse(cached.json) as typeof tree;
    }
    if (text.length * 2 > compiledMarkdownLimit) return tree;
    const json = JSON.stringify(tree);
    const bytes = (text.length + json.length) * 2;
    if (bytes <= compiledMarkdownLimit) {
      while (compiledMarkdown.size >= RENDER_CACHE_BUDGET.entries || compiledMarkdownBytes + bytes > compiledMarkdownLimit) {
        const key = compiledMarkdown.keys().next().value;
        if (key === undefined) break;
        compiledMarkdownBytes -= compiledMarkdown.get(key)!.bytes;
        compiledMarkdown.delete(key);
      }
      compiledMarkdown.set(text, { json, bytes });
      compiledMarkdownBytes += bytes;
    }
    return tree;
  };
};

function omitMemoryElements() {
  return (tree: Element) => {
    function visit(node: Element) {
      if (node.tagName === "code" || node.tagName === "pre") return;
      node.children = node.children?.filter(child => !/^(oai-mem-citation|citation_entries|rollout_ids|memory_citation)$/i.test(child.tagName ?? ""));
      node.children?.forEach(visit);
    }
    visit(tree);
  };
}

function CodeBlock({ children, ...props }: ComponentProps<"pre">) {
  const [copied, setCopied] = useState(false);
  return <div className="markdown-code"><button className="icon-button" title={copied ? "已复制代码" : "复制代码"} onClick={async (event) => {
    const text = event.currentTarget.parentElement?.querySelector("pre")?.textContent;
    if (!text) return;
    try { await navigator.clipboard.writeText(text); setCopied(true); }
    catch { setCopied(false); }
  }}>{copied ? <Check size={14} /> : <Copy size={14} />}</button><pre {...props}>{children}</pre></div>;
}

const MarkdownBody = memo(function MarkdownBody({ text }: { text: string }) {
  const cached = compiledMarkdown.get(text);
  if (cached?.element) {
    compiledMarkdown.delete(text);
    compiledMarkdown.set(text, cached);
    return cached.element;
  }
  const element = Markdown({
    remarkPlugins: [remarkGfm, reuseMarkdownParse],
    rehypePlugins: [omitMemoryElements, reuseMarkdownTree],
    urlTransform: (url, key, node) => key === "href" && node.tagName === "a" && fileLinkTarget(url) ? url : defaultUrlTransform(url),
    components: {
      pre: ({ node: _node, ...props }) => <CodeBlock {...props} />,
      a: ({ node: _node, href, children, ...props }) => href && fileLinkTarget(href)
        ? <FilePathLink href={href}>{children}</FilePathLink>
        : <a {...props} href={href} target={href?.startsWith("#") ? undefined : "_blank"} rel="noopener noreferrer">{children}</a>
    },
    children: text
  });
  const entry = compiledMarkdown.get(text);
  if (entry) entry.element = element;
  return element;
});

export const MarkdownContent = memo(function MarkdownContent({ text, activityId = "message", timelineAliases = [], foldInstructions = false }: { text: string; activityId?: string; timelineAliases?: string[]; foldInstructions?: boolean }) {
  const cleaned = useMemo(() => visibleMarkdown(text), [text]);
  const segments = useMemo(() => foldInstructions ? instructionSegments(cleaned) : null, [cleaned, foldInstructions]);
  const positionKey = useContext(DisclosureRevealContext);
  const reveal = matchesTimelinePosition(positionKey, activityId, timelineAliases);
  return <div className="markdown-content">{segments ? segments.map(segment => segment.kind === "instructions"
    ? <ActivityDetails key={segment.id} stateKey={`${activityId}:${segment.id}`} className="instruction-file execution-command" initiallyOpen={false} reveal={reveal} summary={<><span className="tool-title">AGENTS.md</span><small>{segment.lines} 行 · {segment.bytes} 字节</small></>}>
      {() => <div className="instruction-file-body"><MarkdownBody text={segment.text} /></div>}
    </ActivityDetails>
    : <MarkdownBody key={segment.id} text={segment.text} />) : <MarkdownBody text={cleaned} />}</div>;
});
