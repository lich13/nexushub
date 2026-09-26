import { Check, Copy } from "lucide-react";
import { useEffect, useRef, useState } from "react";

export function CopyReplyButton({ text }: { text: string }) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");
  const timer = useRef<ReturnType<typeof setTimeout>>();
  const attempt = useRef(0);
  useEffect(() => () => { clearTimeout(timer.current); attempt.current++; }, []);
  if (!text.trim()) return null;
  return <div className="reply-actions">
    <button type="button" className="icon-button copy-reply" aria-label={state === "copied" ? "已复制回复" : "复制回复"} title={state === "copied" ? "已复制" : "复制回复"} onClick={async () => {
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
