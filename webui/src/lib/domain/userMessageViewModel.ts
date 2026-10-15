import { literalMarkdownRanges, visibleMarkdown } from "./visibleMarkdown";
import { projectUserMessage } from "./userContext";
import type { UserContextBlock } from "../../types";

export type UserMessageSegment =
  | { kind: "text"; id: string; text: string }
  | { kind: "reply"; id: string; question: string; answer: string };

type QuestionReply = { questionItemId: string; question: string; answer: string };
function isReply(value: unknown): value is QuestionReply {
  if (!value || typeof value !== "object") return false;
  const item = value as Record<string, unknown>;
  return typeof item.questionItemId === "string" && !!item.questionItemId.trim()
    && typeof item.question === "string" && typeof item.answer === "string";
}

// Presentation only: incomplete/unknown envelopes and literal examples retain
// their exact source text. Ordinary user Markdown is never rendered as HTML.
export function userMessageSegments(text: string, contextBlocks?: UserContextBlock[]): UserMessageSegment[] {
  const result: UserMessageSegment[] = [];
  const occurrences = new Map<string, number>();
  const uniqueId = (key: string) => {
    const occurrence = occurrences.get(key) ?? 0;
    occurrences.set(key, occurrence + 1);
    return `${key}:${occurrence}`;
  };
  const source = projectUserMessage(text, contextBlocks).text;
  const protectedRanges = source.includes("<send_user_message_question_reply>") ? literalMarkdownRanges(source) : [];
  const envelopes = /^[ \t]{0,3}<send_user_message_question_reply>([\s\S]*?)<\/send_user_message_question_reply>[ \t]*(?=\r?$)/gm;
  let cursor = 0;
  const addText = (end: number) => {
    const value = visibleMarkdown(source.slice(cursor, end));
    if (value.trim()) result.push({ kind: "text", id: `text:${cursor}`, text: value });
  };
  for (const match of source.matchAll(envelopes)) {
    const start = match.index!;
    if (protectedRanges.some(range => range.start <= start && start < range.end)) continue;
    let replies: unknown;
    try { replies = JSON.parse(match[1]); } catch { continue; }
    if (!Array.isArray(replies) || !replies.length || !replies.every(isReply)) continue;
    addText(start);
    for (const reply of replies) result.push({
      kind: "reply", id: uniqueId(`reply:${reply.questionItemId}`),
      question: visibleMarkdown(reply.question), answer: visibleMarkdown(reply.answer)
    });
    cursor = start + match[0].length;
  }
  addText(source.length);
  return result;
}
