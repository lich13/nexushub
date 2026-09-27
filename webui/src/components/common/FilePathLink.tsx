import { Check, Copy } from "lucide-react";
import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { fileLinkSpans, fileLinkTarget } from "../../lib/domain/fileLinks";
import { visibleMarkdown } from "../../lib/domain/visibleMarkdown";
import { localPathRuntime } from "../../lib/query/filePaths";

export const MarkdownPathScope = createContext<string | null | undefined>(undefined);

export function FilePathLink({ href, children }: { href: string; children: ReactNode }) {
  const cwd = useContext(MarkdownPathScope);
  const target = fileLinkTarget(href, cwd);
  const desktop = localPathRuntime.isDesktop();
  const [feedback, setFeedback] = useState("");
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!feedback || failed) return;
    const timer = window.setTimeout(() => setFeedback(""), 2000);
    return () => window.clearTimeout(timer);
  }, [feedback, failed]);
  if (!target) return <span>{children}</span>;
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(target.path);
      setFailed(false); setFeedback(target.resolved ? "已复制路径" : "已复制原路径；缺少工作目录，无法定位。");
    } catch {
      setFailed(true); setFeedback("复制路径失败，请检查剪贴板权限后重试。");
    }
  };
  const reveal = async () => {
    if (!target.resolved) {
      setFailed(true); setFeedback("无法确定完整路径；可以复制原路径。"); return;
    }
    try {
      await localPathRuntime.reveal(target.path);
      setFailed(false); setFeedback("");
    } catch {
      setFailed(true); setFeedback("无法在 Finder 中显示，请检查路径是否存在及访问权限。");
    }
  };
  return <span className="markdown-file-link">
    <button type="button" className="file-path-label" title={`${desktop ? "在 Finder 中显示" : "复制路径"}：${target.path}`} onClick={desktop ? reveal : copy}>{children}</button>
    <button type="button" className="icon-button file-path-copy" aria-label="复制文件路径" title="复制文件路径" onClick={copy}>{feedback.startsWith("已复制") ? <Check size={14} /> : <Copy size={14} />}</button>
    {feedback && <span className={`file-path-feedback ${failed ? "form-error" : target.resolved ? "sr-only" : ""}`} role={failed ? "alert" : "status"}>{feedback}</span>}
  </span>;
}

export function ToolOutput({ text }: { text: string }) {
  const cleaned = useMemo(() => visibleMarkdown(text), [text]);
  const spans = useMemo(() => fileLinkSpans(cleaned), [cleaned]);
  const content: ReactNode[] = [];
  let offset = 0;
  for (const span of spans) {
    content.push(cleaned.slice(offset, span.start));
    content.push(<FilePathLink key={span.start} href={span.href}>{span.label}</FilePathLink>);
    offset = span.end;
  }
  content.push(cleaned.slice(offset));
  return <pre>{content}</pre>;
}
