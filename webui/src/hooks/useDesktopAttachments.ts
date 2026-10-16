import { useEffect, useRef, type ClipboardEvent, type RefObject } from "react";
import { bindDesktopAttachments, type DesktopChosenItem, type DesktopAttachmentBounds } from "../lib/query/desktop";

type Binding = Awaited<ReturnType<typeof bindDesktopAttachments>>;
export function useDesktopAttachments(sessionId: string | undefined, region: RefObject<HTMLElement>, handles: string[], receive: (items: DesktopChosenItem[]) => void, fail: (message: string) => void) {
  const binding = useRef<Binding | null>(null);
  const latest = useRef({ handles, receive, fail });
  latest.current = { handles, receive, fail };
  const update = useRef<(() => void) | null>(null);
  useEffect(() => {
    if (!sessionId) return;
    let disposed = false;
    let generation = 0;
    let updating = false;
    let dirty = false;
    const bounds = (): DesktopAttachmentBounds | null => {
      const rect = region.current?.getBoundingClientRect();
      if (!rect) return null;
      const x = Math.max(0, rect.left), y = Math.max(0, rect.top);
      const width = Math.min(window.innerWidth, rect.right) - x, height = Math.min(window.innerHeight, rect.bottom) - y;
      return width > 0 && height > 0 ? { x, y, width, height } : null;
    };
    const error = (reason: unknown) => { if (!disposed && !document.hidden) latest.current.fail(reason instanceof Error ? reason.message : "无法添加附件"); };
    const refresh = async () => {
      dirty = true;
      if (updating) return;
      updating = true;
      try {
        while (dirty && !disposed && !document.hidden) {
          dirty = false;
          const current = binding.current;
          const rect = bounds();
          if (current && rect) await current.update(rect, latest.current.handles);
        }
      } catch (reason) { error(reason); }
      finally { updating = false; }
    };
    const connect = async () => {
      const captured = ++generation;
      const rect = bounds();
      if (disposed || document.hidden || !rect) return;
      try {
        const next = await bindDesktopAttachments(sessionId, rect, latest.current.handles, event => {
          if (disposed || document.hidden || captured !== generation) return;
          if (event.error) latest.current.fail(event.error);
          else latest.current.receive(event.items);
        });
        if (disposed || document.hidden || captured !== generation) { await next.close(); return; }
        binding.current = next;
        void refresh();
      } catch (reason) { if (captured === generation) error(reason); }
    };
    const close = () => {
      ++generation;
      const current = binding.current; binding.current = null;
      if (current) void current.close().catch(() => {});
    };
    const visibility = () => { close(); if (!document.hidden) void connect(); };
    update.current = () => { void refresh(); };
    const observer = new ResizeObserver(() => { void refresh(); });
    if (region.current) observer.observe(region.current);
    const geometry = () => { void refresh(); };
    window.addEventListener("resize", geometry);
    window.addEventListener("scroll", geometry, true);
    document.addEventListener("visibilitychange", visibility);
    void connect();
    return () => {
      disposed = true; close(); update.current = null; observer.disconnect();
      window.removeEventListener("resize", geometry); window.removeEventListener("scroll", geometry, true);
      document.removeEventListener("visibilitychange", visibility);
    };
  }, [sessionId, region]);
  useEffect(() => { update.current?.(); }, [handles]);
  return {
    paste: (event: ClipboardEvent<HTMLTextAreaElement>) => {
      // Inspect only clipboard MIME metadata; native bytes/paths stay in Rust.
      if (!Array.from(event.clipboardData.items).some(item => item.kind === "file" && item.type.startsWith("image/"))) return;
      event.preventDefault();
      if (!binding.current) { latest.current.fail("附件接收区尚未就绪，请重试粘贴"); return; }
      const captured = binding.current;
      void captured.paste().catch(reason => {
        if (binding.current === captured && !document.hidden) latest.current.fail(reason instanceof Error ? reason.message : "无法粘贴图片");
      });
    },
    release: (handles: string[]) => {
      if (binding.current) void binding.current.release(handles).catch(() => {});
    }
  };
}
