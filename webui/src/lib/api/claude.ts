import type { ClaudeDeletePreview, ClaudeDeleteRequest, ClaudeDeleteResult, ClaudeSessionDetail, ClaudeSessionSummary } from "../../types";
import { callCommand } from "./transport";

export function listClaudeSessions(query?: { q?: string; limit?: number }): Promise<ClaudeSessionSummary[]> { return callCommand("claude.list", query); }
export function getClaudeSession(sessionKey: string, before?: string): Promise<ClaudeSessionDetail> { return callCommand("claude.detail", { sessionKey, limit: 160, before }); }
export function renameClaudeSession(sessionKey: string, title: string): Promise<ClaudeSessionSummary> { return callCommand("claude.rename", { sessionKey, title }); }
export function previewClaudeSessionDelete(sessionKey: string): Promise<ClaudeDeletePreview> { return callCommand("claude.deletePreview", { sessionKey }); }
export function deleteClaudeSession(request: ClaudeDeleteRequest): Promise<ClaudeDeleteResult> { return callCommand("claude.deleteExecute", { request }); }
