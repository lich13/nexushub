import { callCommand } from "./transport";
export type SessionProvider = "codex" | "grok" | "pi";
export type SessionOperation = "archive" | "restore" | "delete";
export type SessionBatchRequest = { provider: SessionProvider; operation: SessionOperation; sessionKeys: string[] };
export type SessionBatchItem = { sessionKey: string; id: string; title: string; paths: string[]; bytes: number; allowed: boolean; reason: string | null; fingerprint: string | null };
export type SessionBatchPreview = { provider: SessionProvider; operation: SessionOperation; items: SessionBatchItem[] };
export type SessionBatchExecuteRequest = { provider: SessionProvider; operation: SessionOperation; items: Array<{ sessionKey: string; fingerprint: string }>; confirmed: boolean };
export type SessionBatchResult = { items: Array<{ sessionKey: string; status: "succeeded" | "blocked" | "failed"; message: string | null }> };
export function previewSessionBatch(request: SessionBatchRequest, csrfToken?: string | null): Promise<SessionBatchPreview> { return callCommand("sessions.bulkPreview", { request, csrfToken }); }
export function executeSessionBatch(request: SessionBatchExecuteRequest, csrfToken?: string | null): Promise<SessionBatchResult> { return callCommand("sessions.bulkExecute", { request, csrfToken }); }
