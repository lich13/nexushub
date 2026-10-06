import { MoreHorizontal, Pencil } from "lucide-react";
import { ContextTaskMenu, ThreadMenuItems, type ThreadMenuAction } from "./TaskMenu";
import { useEffect, useRef, useState, type MouseEvent, type ReactNode } from "react";

export function RenameableSession({ title, className, disabled, selecting, selected = false, renameBlockReason, onSelect, onRename, menuActions, children }: {
  title: string;
  className: string;
  disabled?: boolean;
  selecting: boolean;
  selected?: boolean;
  renameBlockReason?: string | null;
  onSelect: () => void;
  onRename: (title: string) => Promise<unknown>;
  menuActions?: ThreadMenuAction[];
  children: ReactNode;
}) {
  const [context, setContext] = useState<{ x: number; y: number } | null>(null);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(title);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const button = useRef<HTMLButtonElement>(null);
  const root = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const busy = useRef(false);
  const cancelled = useRef(false);
  const clickTimer = useRef<ReturnType<typeof setTimeout>>();
  const outsideFlushTimer = useRef<ReturnType<typeof setTimeout>>();
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; clearTimeout(clickTimer.current); clearTimeout(outsideFlushTimer.current); };
  }, []);
  const flushPendingSelection = () => {
    if (!clickTimer.current) return;
    clearTimeout(clickTimer.current);
    clickTimer.current = undefined;
    if (!cancelled.current && mounted.current && !selecting && !disabled && !selected) onSelect();
  };
  useEffect(() => {
    const flushOutsideClick = (event: PointerEvent) => {
      if (clickTimer.current && !root.current?.contains(event.target as Node)) {
        clearTimeout(outsideFlushTimer.current);
        outsideFlushTimer.current = setTimeout(() => {
          outsideFlushTimer.current = undefined;
          flushPendingSelection();
        }, 0);
      }
    };
    document.addEventListener("pointerdown", flushOutsideClick, true);
    return () => {
      document.removeEventListener("pointerdown", flushOutsideClick, true);
      clearTimeout(outsideFlushTimer.current);
    };
  });
  useEffect(() => {
    if (selecting || disabled) {
      cancelled.current = true;
      setEditing(false);
      setContext(null);
      setError("");
    }
  }, [selecting, disabled]);
  useEffect(() => { if (editing) { input.current?.focus(); input.current?.select(); } }, [editing]);
  const finish = (restoreFocus: boolean) => {
    cancelled.current = true;
    setEditing(false);
    setError("");
    if (restoreFocus) requestAnimationFrame(() => button.current?.focus());
  };
  const begin = () => {
    if (disabled || selecting || busy.current) return;
    if (renameBlockReason) { setError(renameBlockReason); return; }
    cancelled.current = false;
    setError("");
    setDraft(title);
    setEditing(true);
  };
  const save = async (restoreFocus: boolean) => {
    if (cancelled.current || busy.current || selecting || disabled) return;
    const next = draft.trim();
    if (!next) { setError("名称不能为空"); input.current?.focus(); return; }
    if (renameBlockReason) { setError(renameBlockReason); return; }
    if (next === title) { finish(restoreFocus); return; }
    busy.current = true;
    setSaving(true);
    setError("");
    try {
      await onRename(next);
      if (mounted.current) finish(restoreFocus);
    } catch (reason) {
      if (mounted.current) {
        setError(reason instanceof Error ? reason.message : "改名失败，请重试");
        requestAnimationFrame(() => input.current?.focus());
      }
    } finally {
      busy.current = false;
      if (mounted.current) setSaving(false);
    }
  };
  const handleClick = (event: MouseEvent<HTMLButtonElement>) => {
    if (selecting || event.detail === 0) {
      onSelect();
      return;
    }
    cancelled.current = false;
    clearTimeout(clickTimer.current);
    clickTimer.current = setTimeout(() => {
      clickTimer.current = undefined;
      if (!cancelled.current && mounted.current) onSelect();
    }, 220);
  };
  const cancelPendingClick = () => {
    clearTimeout(clickTimer.current); clickTimer.current = undefined;
    clearTimeout(outsideFlushTimer.current); outsideFlushTimer.current = undefined;
  };
  const openMenu = (x: number, y: number) => {
    cancelPendingClick();
    if (!selecting && !disabled) setContext({ x, y });
  };
  const actions: ThreadMenuAction[] = [
    { id: "rename", label: "改名", icon: <Pencil size={15} />, disabled: Boolean(renameBlockReason) || disabled,
      reason: renameBlockReason, run: begin },
    ...(menuActions ?? []),
  ].map(action => ({ ...action, run: () => { button.current?.focus({ preventScroll: true }); action.run(); } }));
  return <div ref={root} className="renameable-session">
    {editing ? <div className={`${className} session-rename`}>
      <input ref={input} aria-label="线程名称" maxLength={200} value={draft} disabled={saving} aria-invalid={Boolean(error)}
        onChange={event => { setDraft(event.target.value); setError(""); }}
        onBlur={() => { void save(false); }}
        onKeyDown={event => {
          if (event.nativeEvent.isComposing) return;
          if (event.key === "Enter") { event.preventDefault(); void save(true); }
          if (event.key === "Escape") { event.preventDefault(); cancelled.current = true; finish(true); }
        }} />
      <small>{saving ? "正在保存…" : "Enter 保存 · Esc 取消"}</small>
    </div> : <button ref={button} type="button" className={className} disabled={disabled} title={title}
      onClick={handleClick}
      onContextMenu={event => { event.preventDefault(); openMenu(event.clientX, event.clientY); }}
      onDoubleClick={event => { event.preventDefault(); cancelPendingClick(); begin(); }}
      onKeyDown={event => {
        if (event.key === "F2") { event.preventDefault(); begin(); }
        if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
          event.preventDefault();
          const rect = event.currentTarget.getBoundingClientRect();
          openMenu(rect.left + 24, rect.top + Math.min(rect.height, 36));
        }
      }}>
      {children}
    </button>}
    {!editing && !selecting && !disabled && <div className="session-row-menu" onPointerDown={cancelPendingClick}>
      <button type="button" className="icon-button" title="线程操作" aria-label="线程操作" aria-expanded={Boolean(context)} onClick={event => { const rect = event.currentTarget.getBoundingClientRect(); openMenu(rect.left, rect.bottom + 4); }}><MoreHorizontal size={18} /></button>
    </div>}
    {context && <ContextTaskMenu x={context.x} y={context.y} onClose={() => setContext(null)} returnFocus={button.current}>
      <ThreadMenuItems actions={actions} />
    </ContextTaskMenu>}
    {error && <div className="session-rename-error" role="alert">{error}</div>}
  </div>;
}
