import { visibleMarkdown } from "./visibleMarkdown";
import { savePlanMarkdown, type PlanSaveRequest, type PlanSaveResult } from "../api/plans";

export function planFilename(markdown: string, fallbackTitle: string): string {
  markdown = visibleMarkdown(markdown);
  fallbackTitle = visibleMarkdown(fallbackTitle);
  const heading = markdown.match(/^ {0,3}#\s+(.+?)\s*#*\s*$/m)?.[1];
  const firstLine = markdown.split(/\r?\n/).map(line => line.trim()).find(Boolean);
  const rawTitle = (heading || firstLine || fallbackTitle || "计划").replace(/^#{1,6}\s+/, "");
  const title = rawTitle
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/<[^>]*>/g, "")
    .replace(/[*_`~]+/g, "")
    .replace(/[\x00-\x1f\x7f<>:"/\\|?*]+/g, " ")
    .replace(/\s+/g, " ")
    .replace(/^[.\s]+|[.\s]+$/g, "");
  let stem = "";
  for (const character of title.replace(/\.md$/i, "")) {
    if (new TextEncoder().encode(stem + character).length > 200) break;
    stem += character;
  }
  stem = stem.trim().replace(/[.\s]+$/g, "") || "计划";
  return `${/^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/i.test(stem) ? `_${stem}` : stem}.md`;
}

export function downloadPlanMarkdown(markdown: string, filename: string): Promise<PlanSaveResult> {
  const request: PlanSaveRequest = { filename, markdown: visibleMarkdown(markdown) };
  return savePlanMarkdown(request);
}
