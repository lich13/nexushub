import type { PiDeletePreview, PiDeleteRequest, PiDeleteResult, PiSessionDetail, PiSessionSummary } from "../../types";
import { callCommand } from "./transport";

export function listPiSessions(query?: { q?: string; limit?: number }): Promise<PiSessionSummary[]> { return callCommand("pi.list", query); }
export function getPiSession(sessionKey: string): Promise<PiSessionDetail> { return callCommand("pi.detail", { sessionKey }); }
export function renamePiSession(sessionKey: string, title: string): Promise<PiSessionSummary> { return callCommand("pi.rename", { sessionKey, title }); }
export function previewPiSessionDelete(sessionKey: string): Promise<PiDeletePreview> { return callCommand("pi.deletePreview", { sessionKey }); }
export function deletePiSession(request: PiDeleteRequest): Promise<PiDeleteResult> { return callCommand("pi.deleteExecute", { request }); }
