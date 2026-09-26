export function planFilename(markdown: string, fallbackTitle: string): string {
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

export function downloadPlanMarkdown(markdown: string, filename: string): void {
  const url = URL.createObjectURL(new Blob([markdown], { type: "text/markdown;charset=utf-8" }));
  try {
    const link = document.createElement("a");
    link.href = url;
    link.download = filename;
    link.style.display = "none";
    document.body.append(link);
    try { link.click(); } finally { link.remove(); }
  } finally {
    // WebKit may still be opening the download after the synthetic click returns.
    window.setTimeout(() => URL.revokeObjectURL(url), 60_000);
  }
}
