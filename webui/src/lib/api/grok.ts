import type { GrokSessionDetail, GrokSessionSummary, GrokDeletePreview, GrokDeleteRequest, GrokDeleteResult } from "../../types";
import { callCommand, currentRuntimeContext } from "./transport";

export function listGrokSessions(query?: { q?: string; limit?: number }): Promise<GrokSessionSummary[]> {
  return callCommand<GrokSessionSummary[]>("grok.list", query);
}

export function renameGrokSession(id: string, title: string, csrfToken?: string | null): Promise<GrokSessionSummary> {
  return callCommand("grok.rename", { id, title, csrfToken });
}

export function previewGrokSessionDelete(id: string, csrfToken?: string | null): Promise<GrokDeletePreview> {
  return callCommand("grok.deletePreview", { id, csrfToken });
}

export function deleteGrokSession(request: GrokDeleteRequest, csrfToken?: string | null): Promise<GrokDeleteResult> {
  return callCommand("grok.deleteExecute", currentRuntimeContext().kind === "desktop" ? { request } : { ...request, csrfToken });
}

export function getGrokSession(id: string): Promise<GrokSessionDetail> {
  return callCommand<GrokSessionDetail>("grok.detail", { id });
}
