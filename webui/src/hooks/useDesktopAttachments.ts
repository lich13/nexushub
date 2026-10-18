import { useEffect, useRef, type ClipboardEvent, type RefObject } from "react";
import { bindDesktopAttachments, type DesktopChosenItem, type DesktopAttachmentBounds } from "../lib/query/desktop";

type Binding = Awaited<ReturnType<typeof bindDesktopAttachments>>;
export function useDesktopAttachments(sessionId: string | undefined, region: RefObject<HTMLElement>, handles: string[], receive: (items: DesktopChosenItem[]) => void, fail: (message: string) => void) {
  const binding = useRef<Binding | null>(null);
  const latest = useRef({ handles, receive, fail });
  latest.current = { handles, receive, fail };
  const handleKey = JSON.stringify(handles);
  const update = useRef<(() => void) | null>(null);
  const ready = useRef<(() => Promise<Binding | null>) | null>(null);
  const epoch = useRef(0);
  const pasting = useRef<{ connect: () => Promise<Binding | null>; epoch: number } | null>(null);
  useEffect(() => {
    if (!sessionId) return;
    let disposed = false;
    let generation = 0;
    let updating = false;
    let dirty = false;
    let pending: Promise<Binding | null> | null = null;
    const active = () => !disposed && !document.hidden && document.hasFocus();
    const bounds = (): DesktopAttachmentBounds | null => {
      const rect = region.current?.getBoundingClientRect();
      if (!rect) return null;
      const x = Math.max(0, rect.left), y = Math.max(0, rect.top);
      const width = Math.min(window.innerWidth, rect.right) - x, height = Math.min(window.innerHeight, rect.bottom) - y;
      return width > 0 && height > 0 ? { x, y, width, height } : null;
    };
    const error = (reason: unknown) => { if (active()) latest.current.fail(reason instanceof Error ? reason.message : "无法添加附件"); };
    const refresh = async () => {
      dirty = true;
      if (updating) return;
      updating = true;
      try {
        while (dirty && active()) {
          dirty = false;
          const current = binding.current;
          const rect = bounds();
          if (current && rect) {
            try { await current.update(rect, latest.current.handles); }
            catch (reason) { if (binding.current === current) error(reason); }
          }
        }
      }
      finally { updating = false; }
    };
    const connect = (): Promise<Binding | null> => {
      if (binding.current) return Promise.resolve(binding.current);
      if (pending) return pending;
      const captured = ++generation;
      const rect = bounds();
      if (!active() || !rect) return Promise.resolve(null);
      const task = (async () => { try {
        const next = await bindDesktopAttachments(sessionId, rect, latest.current.handles, event => {
          if (!active() || captured !== generation) return;
          if (event.error) latest.current.fail(event.error);
          else latest.current.receive(event.items);
        });
        if (!active() || captured !== generation) { await next.close(); return null; }
        binding.current = next;
        void refresh();
        return next;
      } catch (reason) { if (captured === generation) error(reason); return null; } })();
      pending = task;
      void task.finally(() => { if (pending === task) pending = null; });
      return task;
    };
    const close = () => {
      ++generation;
      ++epoch.current; pasting.current = null;
      pending = null;
      const current = binding.current; binding.current = null;
      if (current) void current.close().catch(() => {});
    };
    // A visible WKWebView may belong to an unfocused window. Passive geometry
    // work must not become a failed user paste, nor retain an old receiver.
    const visibility = () => { close(); if (active()) void connect(); };
    const focus = () => { if (!binding.current) void connect(); };
    update.current = () => { void refresh(); };
    ready.current = connect;
    const observer = new ResizeObserver(() => { void refresh(); });
    if (region.current) observer.observe(region.current);
    const geometry = () => { void refresh(); };
    window.addEventListener("resize", geometry);
    window.addEventListener("scroll", geometry, true);
    window.addEventListener("blur", close);
    window.addEventListener("focus", focus);
    document.addEventListener("visibilitychange", visibility);
    void connect();
    return () => {
      disposed = true; close(); update.current = null; ready.current = null; pasting.current = null; observer.disconnect();
      window.removeEventListener("resize", geometry); window.removeEventListener("scroll", geometry, true);
      window.removeEventListener("blur", close); window.removeEventListener("focus", focus);
      document.removeEventListener("visibilitychange", visibility);
    };
  }, [sessionId, region]);
  useEffect(() => { update.current?.(); }, [handleKey]);
  return {
    paste: (event: ClipboardEvent<HTMLTextAreaElement>) => {
      // Browser text is used only for routing/fallback, never sent to the native
      // attachment API. Rust independently resolves the system pasteboard.
      const text = event.clipboardData.getData?.("text/plain") ?? "";
      const items = Array.from(event.clipboardData.items);
      const types = Array.from(event.clipboardData.types ?? []);
      const files = items.some(item => item.kind === "file")
        || types.some(type => type === "Files" || type === "text/uri-list" || type === "public.file-url" || type.startsWith("image/"));
      const lines = text.trim().split(/\r?\n/).filter(Boolean);
      const pathText = lines.length > 0 && lines.every(line => /^(?:["']?(?:\/|~\/|file:))/.test(line.trim()));
      if (!files && text && !pathText) return;
      if (!files && !text && items.some(item => item.type === "text/html")) return;
      const connect = ready.current;
      if (!connect) {
        // A closing native picker may briefly leave no receiver. Keep text in
        // the browser's normal paste/undo path instead of consuming it.
        if (!text) { event.preventDefault(); latest.current.fail("请先连接当前线程再添加附件"); }
        return;
      }
      event.preventDefault();
      if (pasting.current?.connect === connect && pasting.current.epoch === epoch.current) return;
      const attempt = { connect, epoch: epoch.current };
      pasting.current = attempt;
      const target = event.currentTarget;
      const before = target?.value;
      const start = target?.selectionStart, end = target?.selectionEnd;
      const valid = () => ready.current === connect && epoch.current === attempt.epoch && !document.hidden && document.hasFocus();
      const fallback = () => {
        if (!text) return;
        if (!valid() || !target || target.value !== before || target.selectionStart !== start || target.selectionEnd !== end
          || document.activeElement !== target) {
          if (valid()) latest.current.fail("输入位置已变化，请重新粘贴文本");
          return;
        }
        // Native browser editing preserves selection, React input events and
        // the WKWebView undo stack. Never overwrite a newer draft with a late paste.
        try {
          if (!document.execCommand?.("insertText", false, text)) latest.current.fail("无法插入剪贴板文本，请重试粘贴");
        } catch { latest.current.fail("无法插入剪贴板文本，请重试粘贴"); }
      };
      void (async () => {
        let timer: ReturnType<typeof setTimeout> | undefined;
        try {
          const captured = await Promise.race([connect(), new Promise<null>(resolve => { timer = setTimeout(() => resolve(null), 5000); })]);
          if (timer) clearTimeout(timer);
          if (!valid()) return;
          if (!captured || binding.current !== captured) throw new Error("附件接收区尚未就绪，请重试粘贴");
          const result = await captured.paste();
          if (!valid() || binding.current !== captured) return;
          if (result?.pasteDisposition === "text") fallback();
          else if (result?.pasteDisposition === "unsupported") {
            if (text) fallback(); else latest.current.fail("剪贴板没有可添加的文件或图片");
          }
          // Older local adapters omit pasteDisposition and deliver via channel.
        } catch (reason) {
          if (valid()) { fallback(); latest.current.fail(reason instanceof Error ? reason.message : "无法粘贴附件"); }
        } finally {
          if (timer) clearTimeout(timer);
          if (pasting.current === attempt) pasting.current = null;
        }
      })();
    },
    release: (handles: string[]) => {
      if (binding.current) void binding.current.release(handles).catch(() => {});
    }
  };
}
