import type { SessionSearchRequest, SessionSearchResponse } from "../../types";
import { callCommand } from "./transport";

export function searchSessions(request: SessionSearchRequest): Promise<SessionSearchResponse> {
  return callCommand<SessionSearchResponse>("sessions.search", { request });
}
