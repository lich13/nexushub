import { useEffect, useRef, useState, type RefObject } from "react";
import { locateTimelineTarget } from "./SessionSearch";
import { activeUserTimelineId, type TimelineEntry } from "../../lib/domain/timelineViewModel";

export function TimelineRail({ entries, streamRef, onSelect, loadOlder, loading }: {
  entries: TimelineEntry[]; streamRef: RefObject<HTMLDivElement>; onSelect?: (id: string) => void;
  loadOlder?: () => void; loading?: boolean;
}) {
  const [activeId, setActiveId] = useState<string | null>(entries[0]?.id ?? null);
  const marks = useRef<HTMLDivElement>(null);
  const [start, setStart] = useState(0);
  const [height, setHeight] = useState(600);
  const [preview, setPreview] = useState<{ entry: TimelineEntry; top: number } | null>(null);
  const step = 12, count = Math.max(100, Math.ceil(height / 12) + 20);
  useEffect(() => { setPreview(null); }, [entries]);
  useEffect(() => {
    const root = marks.current;
    if (!root) return;
    const measure = () => setHeight(root.clientHeight);
    const resize = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(measure);
    resize?.observe(root); measure();
    return () => resize?.disconnect();
  }, [entries.length > 0, Boolean(loadOlder)]);
  useEffect(() => {
    const root = streamRef.current;
    if (!root || !entries.length) { setActiveId(null); return; }
    const entryIds = new Set(entries.map(entry => entry.id));
    let frame = 0;
    let dirty = true;
    let anchors: { id: string; top: number }[] = [];
    const observed = new Set<Element>();
    const update = () => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        if (dirty) {
          for (const element of observed) if (element.parentElement !== root) { resize?.unobserve(element); observed.delete(element); }
          for (const element of Array.from(root.children)) if (!observed.has(element)) { resize?.observe(element); observed.add(element); }
          const origin = root.getBoundingClientRect().top - root.scrollTop;
          const seen = new Set<string>();
          anchors = Array.from(root.querySelectorAll<HTMLElement>("[data-timeline-id], [data-timeline-aliases]"))
            .flatMap(element => {
              const candidates = [element.dataset.timelineId, ...(element.dataset.timelineAliases?.split(/\s+/) ?? [])];
              const id = candidates
                .map(raw => {
                  if (!raw) return undefined;
                  if (entryIds.has(raw)) return raw;
                  const colon = raw.indexOf(":");
                  const stripped = colon >= 0 ? raw.slice(colon + 1) : "";
                  return stripped && entryIds.has(stripped) ? stripped : undefined;
                })
                .find((value): value is string => Boolean(value));
              if (!id || seen.has(id)) return [];
              seen.add(id);
              return [{ id, top: element.getBoundingClientRect().top - origin }];
            });
          dirty = false;
        }
        setActiveId(anchors.length ? activeUserTimelineId(anchors, root.scrollHeight - root.scrollTop - root.clientHeight <= 2
          ? Number.POSITIVE_INFINITY : root.scrollTop + root.clientHeight * 0.2) : entries[entries.length - 1].id);
      });
    };
    const invalidate = () => { dirty = true; update(); };
    root.addEventListener("scroll", update, { passive: true });
    root.addEventListener("toggle", invalidate, true);
    const resize = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(invalidate);
    resize?.observe(root);
    const mutations = typeof MutationObserver === "undefined" ? null : new MutationObserver(invalidate);
    mutations?.observe(root, { childList: true, characterData: true, subtree: true });
    update();
    return () => { cancelAnimationFrame(frame); root.removeEventListener("scroll", update); root.removeEventListener("toggle", invalidate, true); resize?.disconnect(); mutations?.disconnect(); };
  }, [entries, streamRef]);
  useEffect(() => {
    const index = entries.findIndex(entry => entry.id === activeId), root = marks.current;
    if (index < 0 || !root || root.matches(":hover") || root.contains(document.activeElement)) return;
    const top = index * step;
    if (top < root.scrollTop || top + step > root.scrollTop + root.clientHeight) {
      root.scrollTop = Math.max(0, top - root.clientHeight / 2);
      setStart(Math.max(0, Math.floor(root.scrollTop / step) - 10));
    }
  }, [activeId, entries]);
  const first = Math.min(start, Math.max(0, entries.length - 1));
  const last = Math.min(entries.length, first + count);
  const activeVisible = entries.slice(first, last).some(entry => entry.id === activeId);
  const focus = (index: number) => {
    const target = Math.max(0, Math.min(entries.length - 1, index));
    if (!marks.current) return;
    marks.current.scrollTop = Math.max(0, target * step - marks.current.clientHeight / 2);
    setStart(Math.max(0, Math.floor(marks.current.scrollTop / step) - 10));
    requestAnimationFrame(() => marks.current?.querySelector<HTMLButtonElement>(`[data-entry-index="${target}"]`)?.focus({ preventScroll: true }));
  };
  const show = (entry: TimelineEntry, button: HTMLButtonElement) => {
    const root = marks.current;
    if (root) setPreview({ entry, top: Math.max(12, Math.min(root.clientHeight - 12, button.getBoundingClientRect().top - root.getBoundingClientRect().top + step / 2)) });
  };
  if (!entries.length && !loadOlder) return null;
  return <nav className="timeline-rail" aria-label="线程时间线">
    {loadOlder && <button className="timeline-rail-older" disabled={loading} aria-label="加载更早的时间线记录" onClick={loadOlder}>↑</button>}
    <div ref={marks} className="timeline-rail-marks" onScroll={event => { setStart(Math.max(0, Math.floor(event.currentTarget.scrollTop / step) - 10)); setPreview(null); }} onMouseLeave={() => setPreview(null)}>
    <div style={{ height: first * step }} aria-hidden="true" />
    {entries.slice(first, last).map((entry, offset) => <button key={entry.id} type="button" data-entry-index={first + offset} className={`timeline-rail-item ${entry.id === activeId ? "active" : ""}`} aria-label={`时间线：${entry.title}`} aria-posinset={first + offset + 1} aria-setsize={entries.length}
      tabIndex={entry.id === activeId || (!activeVisible && offset === 0) ? 0 : -1}
      onFocus={event => show(entry, event.currentTarget)} onBlur={() => setPreview(null)} onMouseEnter={event => show(entry, event.currentTarget)}
      onKeyDown={event => { const index = first + offset; const target = event.key === "Home" ? 0 : event.key === "End" ? entries.length - 1 : event.key === "ArrowUp" ? index - 1 : event.key === "ArrowDown" ? index + 1 : null; if (target !== null) { event.preventDefault(); focus(target); } }}
      onClick={() => { setActiveId(entry.id); if (onSelect) onSelect(entry.id); else if (streamRef.current) locateTimelineTarget(entry.id, streamRef.current); }}>
      <span className="timeline-rail-mark" aria-hidden="true" />
    </button>)}
    <div style={{ height: (entries.length - last) * step }} aria-hidden="true" />
    </div>
    {preview && <span className="timeline-rail-preview" aria-hidden="true" style={{ top: preview.top }}><strong>{preview.entry.title}</strong>{preview.entry.preview && <span>{preview.entry.preview}</span>}</span>}
  </nav>;
}
