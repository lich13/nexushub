import { MoreHorizontal } from "lucide-react";
import { useEffect, useRef, type ReactNode, type Ref } from "react";

export function TaskMenu({ label = "任务操作", children, triggerRef }: { label?: string; children: ReactNode; triggerRef?: Ref<HTMLElement> }) {
  const menu = useRef<HTMLDetailsElement>(null);
  const close = (restoreFocus: boolean) => {
    if (!menu.current?.open) return;
    menu.current.open = false;
    if (restoreFocus) menu.current.querySelector("summary")?.focus();
  };
  useEffect(() => {
    const outside = (event: PointerEvent) => {
      if (!menu.current?.contains(event.target as Node)) close(false);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, []);
  return <details ref={menu} className="task-menu" onKeyDown={(event) => {
    if (event.key === "Escape") { event.preventDefault(); close(true); }
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      if (menu.current) menu.current.open = true;
      const buttons = Array.from(menu.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []);
      const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
      const next = index < 0
        ? (event.key === "ArrowDown" ? 0 : buttons.length - 1)
        : (index + (event.key === "ArrowDown" ? 1 : buttons.length - 1)) % buttons.length;
      buttons[next]?.focus();
    }
  }} onBlur={(event) => {
    if (event.relatedTarget && !event.currentTarget.contains(event.relatedTarget)) close(false);
  }}>
    <summary ref={triggerRef} className="icon-button" title={label} aria-label={label}><MoreHorizontal size={18} /></summary>
    <div className="task-menu-items" onClick={(event) => {
      if ((event.target as Element).closest("button")) close(true);
    }}>{children}</div>
  </details>;
}
