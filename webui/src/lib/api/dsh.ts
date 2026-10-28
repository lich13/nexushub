import type {
  ProviderDetailRequest,
  ProviderListRequest,
  ProviderRenameRequest,
  ProviderSessionDetail,
  ProviderSessionRequest,
  ProviderSessionSummary
} from "../../types";
import { callCommand } from "./transport";

export function listDshSessions(request: ProviderListRequest = {}): Promise<ProviderSessionSummary[]> {
  return callCommand("dsh.list", { request });
}
export function getDshSession(request: ProviderDetailRequest): Promise<ProviderSessionDetail> {
  return callCommand("dsh.detail", { request });
}
export function renameDshSession(request: ProviderRenameRequest): Promise<ProviderSessionSummary> {
  return callCommand("dsh.rename", { request });
}
export function archiveDshSession(request: ProviderSessionRequest): Promise<ProviderSessionSummary> {
  return callCommand("dsh.archive", { request });
}
export function restoreDshSession(request: ProviderSessionRequest): Promise<ProviderSessionSummary> {
  return callCommand("dsh.restore", { request });
}
