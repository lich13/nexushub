import type { GrokSessionDetail, GrokSessionSummary, GrokDeletePreview, GrokDeleteRequest, GrokDeleteResult } from "../../types";
import { callCommand } from "./transport";

export function listGrokSessions(query?: { q?: string; limit?: number }): Promise<GrokSessionSummary[]> {
  return callCommand<GrokSessionSummary[]>("grok.list", query);
}

export function renameGrokSession(id: string, title: string): Promise<GrokSessionSummary> {
  return callCommand("grok.rename", { id, title });
}

export function previewGrokSessionDelete(id: string): Promise<GrokDeletePreview> {
  return callCommand("grok.deletePreview", { id });
}

export function deleteGrokSession(request: GrokDeleteRequest): Promise<GrokDeleteResult> {
  return callCommand("grok.deleteExecute", { request });
}

export function getGrokSession(id: string): Promise<GrokSessionDetail> {
  return callCommand<GrokSessionDetail>("grok.detail", { id });
}
