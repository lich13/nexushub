import type { PiDeletePreview, PiDeleteRequest, PiDeleteResult, PiSessionDetail, PiSessionSummary } from "../../types";
import { callCommand, currentRuntimeContext } from "./transport";

export function listPiSessions(query?: { q?: string; limit?: number }): Promise<PiSessionSummary[]> { return callCommand("pi.list", query); }
export function getPiSession(sessionKey: string): Promise<PiSessionDetail> { return callCommand("pi.detail", { sessionKey }); }
export function renamePiSession(sessionKey: string, title: string, csrfToken?: string | null): Promise<PiSessionSummary> { return callCommand("pi.rename", { sessionKey, title, csrfToken }); }
export function previewPiSessionDelete(sessionKey: string, csrfToken?: string | null): Promise<PiDeletePreview> { return callCommand("pi.deletePreview", { sessionKey, csrfToken }); }
export function deletePiSession(request: PiDeleteRequest, csrfToken?: string | null): Promise<PiDeleteResult> { return callCommand("pi.deleteExecute", currentRuntimeContext().kind === "desktop" ? { request } : { ...request, csrfToken }); }
