import { Check, Copy } from "lucide-react";
import { useState, type ComponentProps } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

function CodeBlock({ children, ...props }: ComponentProps<"pre">) {
  const [copied, setCopied] = useState(false);
  return <div className="markdown-code"><button className="icon-button" title={copied ? "已复制代码" : "复制代码"} onClick={async (event) => {
    const text = event.currentTarget.parentElement?.querySelector("pre")?.textContent;
    if (!text) return;
    try { await navigator.clipboard.writeText(text); setCopied(true); }
    catch { setCopied(false); }
  }}>{copied ? <Check size={14} /> : <Copy size={14} />}</button><pre {...props}>{children}</pre></div>;
}

export function MarkdownContent({ text }: { text: string }) {
  return <div className="markdown-content"><Markdown remarkPlugins={[remarkGfm]} components={{
    pre: ({ node: _node, ...props }) => <CodeBlock {...props} />,
    a: ({ node: _node, ...props }) => <a {...props} target="_blank" rel="noopener noreferrer" />
  }}>{text}</Markdown></div>;
}
