import { MoreHorizontal } from "lucide-react";
import { useEffect, useImperativeHandle, useRef, useState, type KeyboardEvent, type ReactNode, type Ref } from "react";

export function TaskMenu({ label = "任务操作", children, triggerRef }: { label?: string; children: ReactNode; triggerRef?: Ref<HTMLElement> }) {
  const menu = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  useImperativeHandle(triggerRef, () => trigger.current as HTMLButtonElement, []);
  const close = (restoreFocus: boolean) => {
    if (!open) return;
    setOpen(false);
    if (restoreFocus) requestAnimationFrame(() => trigger.current?.focus());
  };
  useEffect(() => {
    const outside = (event: Event) => {
      if (!menu.current?.contains(event.target as Node)) close(false);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open]);
  const focusMenuItem = (direction: "next" | "previous") => {
    const buttons = Array.from(menu.current?.querySelectorAll<HTMLButtonElement>(".task-menu-items button:not(:disabled)") ?? []);
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const next = index < 0
      ? (direction === "next" ? 0 : buttons.length - 1)
      : (index + (direction === "next" ? 1 : buttons.length - 1)) % buttons.length;
    buttons[next]?.focus();
  };
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape") { event.preventDefault(); close(true); return; }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (!open) setOpen(true);
      requestAnimationFrame(() => focusMenuItem(event.key === "ArrowDown" ? "next" : "previous"));
    }
  };
  return <div ref={menu} className="task-menu" onKeyDown={handleKeyDown} onBlur={(event) => {
    if (event.relatedTarget && !event.currentTarget.contains(event.relatedTarget)) close(false);
  }}>
    <button ref={trigger} type="button" className="icon-button" title={label} aria-label={label} aria-expanded={open} onClick={() => setOpen(value => !value)}><MoreHorizontal size={18} /></button>
    {open && <div className="task-menu-items" onClick={(event) => {
      if ((event.target as Element).closest("button")) close(true);
    }}>{children}</div>}
  </div>;
}
