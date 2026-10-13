import { useEffect, useState, type RefObject } from "react";
import { locateTimelineTarget } from "./SessionSearch";
import { activeUserTimelineId, type TimelineEntry } from "../../lib/domain/timelineViewModel";

export function TimelineRail({ entries, streamRef }: { entries: TimelineEntry[]; streamRef: RefObject<HTMLDivElement> }) {
  const [activeId, setActiveId] = useState<string | null>(entries[0]?.id ?? null);
  useEffect(() => {
    const root = streamRef.current;
    if (!root || !entries.length) { setActiveId(null); return; }
    const entryIds = new Set(entries.map(entry => entry.id));
    const elements = Array.from(root.querySelectorAll<HTMLElement>("[data-timeline-id]"))
      .filter(element => entryIds.has(element.dataset.timelineId ?? ""));
    let frame = 0;
    let dirty = true;
    let anchors: { id: string; top: number }[] = [];
    const update = () => {
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        if (dirty) {
          const origin = root.getBoundingClientRect().top - root.scrollTop;
          anchors = elements.map(element => ({ id: element.dataset.timelineId!, top: element.getBoundingClientRect().top - origin }));
          dirty = false;
        }
        setActiveId(activeUserTimelineId(anchors, root.scrollHeight - root.scrollTop - root.clientHeight <= 2
          ? Number.POSITIVE_INFINITY : root.scrollTop + root.clientHeight * 0.2));
      });
    };
    const invalidate = () => { dirty = true; update(); };
    root.addEventListener("scroll", update, { passive: true });
    root.addEventListener("toggle", invalidate, true);
    const resize = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(invalidate);
    resize?.observe(root);
    Array.from(root.children).forEach(element => resize?.observe(element));
    const mutations = typeof MutationObserver === "undefined" ? null : new MutationObserver(invalidate);
    mutations?.observe(root, { childList: true, characterData: true, subtree: true });
    update();
    return () => { cancelAnimationFrame(frame); root.removeEventListener("scroll", update); root.removeEventListener("toggle", invalidate, true); resize?.disconnect(); mutations?.disconnect(); };
  }, [entries, streamRef]);
  if (!entries.length) return null;
  return <nav className="timeline-rail" aria-label="线程时间线">
    {entries.map(entry => <button key={entry.id} type="button" className={`timeline-rail-item ${entry.id === activeId ? "active" : ""}`} title={entry.preview ? `${entry.title}：${entry.preview}` : entry.title} aria-label={`时间线：${entry.title}`} onFocus={() => setActiveId(entry.id)} onClick={() => { setActiveId(entry.id); locateTimelineTarget(entry.id); }}>
      <span className="timeline-rail-mark" aria-hidden="true" />
      <span className="timeline-rail-preview" aria-hidden="true" data-title={entry.title} data-preview={entry.preview} />
    </button>)}
  </nav>;
}
