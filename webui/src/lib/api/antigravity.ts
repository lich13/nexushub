import type {
  ProviderDetailRequest,
  ProviderListRequest,
  ProviderRenameRequest,
  ProviderSessionDetail,
  ProviderSessionRequest,
  ProviderSessionSummary
} from "../../types";
import { callCommand } from "./transport";

export function listAntigravitySessions(request: ProviderListRequest = {}): Promise<ProviderSessionSummary[]> {
  return callCommand("antigravity.list", { request });
}
export function getAntigravitySession(request: ProviderDetailRequest): Promise<ProviderSessionDetail> {
  return callCommand("antigravity.detail", { request });
}
export function renameAntigravitySession(request: ProviderRenameRequest): Promise<ProviderSessionSummary> {
  return callCommand("antigravity.rename", { request });
}
export function archiveAntigravitySession(request: ProviderSessionRequest): Promise<ProviderSessionSummary> {
  return callCommand("antigravity.archive", { request });
}
export function restoreAntigravitySession(request: ProviderSessionRequest): Promise<ProviderSessionSummary> {
  return callCommand("antigravity.restore", { request });
}
