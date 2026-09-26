import { Check, Copy, Download } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { downloadPlanMarkdown, planFilename } from "../../lib/domain/planExport";

export function PlanActions({ markdown, fallbackTitle }: { markdown: string; fallbackTitle: string }) {
  const [state, setState] = useState<"idle" | "copied" | "copy-failed" | "downloaded" | "download-failed">("idle");
  const timer = useRef<ReturnType<typeof setTimeout>>();
  const attempt = useRef(0);
  useEffect(() => () => { clearTimeout(timer.current); attempt.current++; }, []);
  if (!markdown.trim()) return null;
  const feedback = (value: typeof state) => {
    clearTimeout(timer.current);
    setState(value);
    if (value === "copied" || value === "downloaded") timer.current = setTimeout(() => setState("idle"), 2000);
  };
  return <div className="plan-actions">
    <button className="icon-button plan-action" type="button" aria-label={state === "copied" ? "已复制计划" : "复制计划"} title={state === "copied" ? "已复制" : "复制计划"} onClick={async () => {
      const current = ++attempt.current;
      try {
        await navigator.clipboard.writeText(markdown);
        if (current === attempt.current) feedback("copied");
      } catch {
        if (current === attempt.current) feedback("copy-failed");
      }
    }}>{state === "copied" ? <Check size={17} /> : <Copy size={17} />}</button>
    <button className="icon-button plan-action" type="button" aria-label="下载计划 Markdown" title="下载计划 Markdown" onClick={() => {
      try {
        downloadPlanMarkdown(markdown, planFilename(markdown, fallbackTitle));
        feedback("downloaded");
      } catch { feedback("download-failed"); }
    }}><Download size={17} /></button>
    <span role="status" aria-live="polite" className={state.endsWith("failed") ? "plan-action-error" : "visually-hidden"}>{state === "copied" ? "已复制" : state === "copy-failed" ? "复制失败，请重试或手动选择文本复制。" : state === "downloaded" ? "已开始下载" : state === "download-failed" ? "下载失败，请重试。" : ""}</span>
  </div>;
}
