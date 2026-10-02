import type { UserMessageContent } from "../../types";
import { userMessageSegments } from "./userMessageViewModel";

export type TimelineEntry = { id: string; title: string; preview?: string };
type TimelineSource = {
  id: string;
  role?: string | null;
  kind?: string;
  text?: string | null;
  userMessage?: UserMessageContent | null;
};

/** Anchors remain on every rendered block; the rail represents human input only. */
export function userTimelineEntries(provider: "codex" | "claude_code" | "grok" | "pi", sources: readonly TimelineSource[]): TimelineEntry[] {
  const seen = new Set<string>();
  const entries: TimelineEntry[] = [];
  for (const source of sources) {
    const isUser = provider === "codex" ? source.role?.trim().toLowerCase() === "user"
      : source.kind === (provider === "grok" ? "user_message_chunk" : "user_message");
    if (!isUser) continue;
    const identity = source.userMessage?.id || source.id;
    if (seen.has(identity)) continue;
    seen.add(identity);
    const text = userMessageSegments(source.userMessage?.text ?? source.text ?? "")
      .flatMap(segment => segment.kind === "text" ? [segment.text] : segment.kind === "reply" ? [segment.answer] : [])
      .join("\n").trim();
    const attachments = source.userMessage?.attachments ?? [];
    // Injected instructions have their own disclosure, not a human-input marker.
    if (!text && !attachments.length) continue;
    const compact = text.replace(/\s+/g, " ");
    entries.push({ id: source.id, title: compact.slice(0, 80) || "附件", preview: compact.length > 80 ? compact.slice(80, 260) : undefined });
  }
  return entries;
}

/** Keep the preceding instruction active throughout its assistant/tool replies. */
export function activeUserTimelineId(anchors: readonly { id: string; top: number }[], viewportTop: number): string | null {
  let active = anchors[0]?.id ?? null;
  for (const anchor of anchors) {
    if (anchor.top > viewportTop) break;
    active = anchor.id;
  }
  return active;
}
