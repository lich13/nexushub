import { visibleMarkdown } from "../../lib/domain/visibleMarkdown";
import { Check, Copy } from "lucide-react";
import { useEffect, useRef, useState } from "react";

export function CopyReplyButton({ text, label = "复制回复", copiedLabel = "已复制回复" }: { text: string; label?: string; copiedLabel?: string }) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");
  const timer = useRef<ReturnType<typeof setTimeout>>();
  const attempt = useRef(0);
  useEffect(() => () => { clearTimeout(timer.current); attempt.current++; }, []);
  text = visibleMarkdown(text);
  if (!text.trim()) return null;
  return <div className="reply-actions">
    <button type="button" className="icon-button copy-reply" aria-label={state === "copied" ? copiedLabel : label} title={state === "copied" ? "已复制" : label} onClick={async () => {
      const current = ++attempt.current;
      clearTimeout(timer.current);
      try {
        await navigator.clipboard.writeText(text);
        if (current !== attempt.current) return;
        setState("copied");
        timer.current = setTimeout(() => setState("idle"), 2000);
      } catch {
        if (current === attempt.current) setState("failed");
      }
    }}>{state === "copied" ? <Check size={17} /> : <Copy size={17} />}</button>
    <span aria-live="polite" className={state === "failed" ? "reply-copy-error" : "visually-hidden"}>{state === "failed" ? "复制失败，请重试或手动选择文本复制。" : ""}</span>
  </div>;
}
