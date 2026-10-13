import { Check, Copy } from "lucide-react";
import { memo, useMemo, useState, type ComponentProps } from "react";
import Markdown, { defaultUrlTransform, type Options } from "react-markdown";
import remarkGfm from "remark-gfm";
import { instructionSegments, visibleMarkdown } from "../../lib/domain/visibleMarkdown";
import { ActivityDetails } from "./ActivityDetails";
import { fileLinkTarget } from "../../lib/domain/fileLinks";
import { FilePathLink } from "./FilePathLink";

type Element = { tagName?: string; children?: Element[] };

type RemarkPlugin = Extract<NonNullable<Options["remarkPlugins"]>[number], (...args: never[]) => unknown>;
type MarkdownParser = NonNullable<ThisParameterType<RemarkPlugin>["parser"]>;
const parsedMarkdown = new Map<string, { json: string; bytes: number }>();
let parsedMarkdownBytes = 0;
const parsedMarkdownLimit = 8 * 1024 * 1024;

// Reuse parsing across thread remounts, but give each transform its own tree.
// Serialized syntax contains no React owner, component state, or path context.
const reuseMarkdownParse: RemarkPlugin = function () {
  const parse = this.parser!;
  this.parser = (text, file) => {
    const cached = parsedMarkdown.get(text);
    if (cached) {
      parsedMarkdown.delete(text);
      parsedMarkdown.set(text, cached);
      return JSON.parse(cached.json) as ReturnType<MarkdownParser>;
    }
    const tree = parse(text, file);
    if (text.length * 2 > parsedMarkdownLimit) return tree;
    const json = JSON.stringify(tree);
    const bytes = (text.length + json.length) * 2;
    if (bytes <= parsedMarkdownLimit) {
      while (parsedMarkdown.size >= 512 || parsedMarkdownBytes + bytes > parsedMarkdownLimit) {
        const key = parsedMarkdown.keys().next().value;
        if (key === undefined) break;
        parsedMarkdownBytes -= parsedMarkdown.get(key)!.bytes;
        parsedMarkdown.delete(key);
      }
      parsedMarkdown.set(text, { json, bytes });
      parsedMarkdownBytes += bytes;
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
  return <Markdown remarkPlugins={[remarkGfm, reuseMarkdownParse]} rehypePlugins={[omitMemoryElements]} urlTransform={(url, key, node) => key === "href" && node.tagName === "a" && fileLinkTarget(url) ? url : defaultUrlTransform(url)} components={{
    pre: ({ node: _node, ...props }) => <CodeBlock {...props} />,
    a: ({ node: _node, href, children, ...props }) => href && fileLinkTarget(href)
      ? <FilePathLink href={href}>{children}</FilePathLink>
      : <a {...props} href={href} target={href?.startsWith("#") ? undefined : "_blank"} rel="noopener noreferrer">{children}</a>
  }}>{text}</Markdown>;
});

export const MarkdownContent = memo(function MarkdownContent({ text, activityId = "message", foldInstructions = false }: { text: string; activityId?: string; foldInstructions?: boolean }) {
  const cleaned = useMemo(() => visibleMarkdown(text), [text]);
  const segments = useMemo(() => foldInstructions ? instructionSegments(cleaned) : null, [cleaned, foldInstructions]);
  return <div className="markdown-content">{segments ? segments.map(segment => segment.kind === "instructions"
    ? <ActivityDetails key={segment.id} stateKey={`${activityId}:${segment.id}`} className="instruction-file execution-command" initiallyOpen={false} summary={<><span className="tool-title">AGENTS.md</span><small>{segment.lines} 行 · {segment.bytes} 字节</small></>}>
      {() => <div className="instruction-file-body"><MarkdownBody text={segment.text} /></div>}
    </ActivityDetails>
    : <MarkdownBody key={segment.id} text={segment.text} />) : <MarkdownBody text={cleaned} />}</div>;
});
