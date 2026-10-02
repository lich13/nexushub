import { useEffect, useState, type RefObject } from "react";
import { locateTimelineTarget } from "./SessionSearch";

export type TimelineEntry = {
  id: string;
  title: string;
  preview?: string;
  status?: string;
};

export function TimelineRail({ entries, streamRef }: { entries: TimelineEntry[]; streamRef: RefObject<HTMLDivElement> }) {
  const [activeId, setActiveId] = useState<string | null>(entries[0]?.id ?? null);
  useEffect(() => {
    if (!entries.length) { setActiveId(null); return; }
    if (!entries.some(entry => entry.id === activeId)) setActiveId(entries[0].id);
    const root = streamRef.current;
    if (!root || typeof IntersectionObserver === "undefined") return;
    const observer = new IntersectionObserver((observations) => {
      const visible = observations.filter(item => item.isIntersecting).sort((a, b) => a.boundingClientRect.top - b.boundingClientRect.top)[0];
      const id = visible?.target instanceof HTMLElement ? visible.target.dataset.timelineId : undefined;
      if (id) setActiveId(id);
    }, { root, rootMargin: "-18% 0px -68% 0px", threshold: [0, 1] });
    const entryIds = new Set(entries.map(entry => entry.id));
    const elements = Array.from(root.querySelectorAll<HTMLElement>("[data-timeline-id]"))
      .filter(element => entryIds.has(element.dataset.timelineId ?? ""));
    elements.forEach(element => observer.observe(element));
    return () => observer.disconnect();
  }, [activeId, entries, streamRef]);
  if (!entries.length) return null;
  return <nav className="timeline-rail" aria-label="线程时间线">
    {entries.map(entry => <button key={entry.id} type="button" className={`timeline-rail-item ${entry.id === activeId ? "active" : ""}`} title={entry.preview ? `${entry.title}：${entry.preview}` : entry.title} aria-label={`时间线：${entry.title}`} onFocus={() => setActiveId(entry.id)} onClick={() => { setActiveId(entry.id); locateTimelineTarget(entry.id); }}>
      <span className="timeline-rail-mark" aria-hidden="true" />
      <span className="timeline-rail-preview" aria-hidden="true" data-title={entry.title} data-preview={[entry.status, entry.preview].filter(Boolean).join(" · ") || undefined} />
    </button>)}
  </nav>;
}
