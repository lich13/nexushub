import { QueryClient } from "@tanstack/react-query";
import { expect, test, vi } from "vitest";
import type { ClaudeSessionDetail, ClaudeSessionSummary } from "../../types";

const state = vi.hoisted(() => ({ client: undefined as QueryClient | undefined, rename: vi.fn() }));
vi.mock("../api/claude", async original => ({ ...await original<typeof import("../api/claude")>(), renameClaudeSession: state.rename }));
vi.mock("@tanstack/react-query", async original => ({
  ...await original<typeof import("@tanstack/react-query")>(),
  useQueryClient: () => state.client,
  useMutation: (options: { mutationFn: (value: unknown) => Promise<unknown>; onSuccess?: (value: unknown) => void }) => ({
    mutateAsync: async (value: unknown) => { const result = await options.mutationFn(value); options.onSuccess?.(result); return result; }
  })
}));

test("rename immediately uses the read-back summary in loaded history and list caches", async () => {
  const { useClaudeActions } = await import("./claude");
  const client = state.client = new QueryClient();
  const summary: ClaudeSessionSummary = { id: "fixture", sessionKey: "fixture-key", title: "Old", cwd: "/fixture", path: "/fixture/session.jsonl", messageCount: 2, status: "recent", formatVersion: "fixture", canRename: true, canDelete: true };
  const page: ClaudeSessionDetail = { summary, events: [], totalEvents: 2, hasMore: false };
  client.setQueryData(["claude_code", "session", summary.sessionKey], { pages: [page, page], pageParams: [undefined, "older"] });
  client.setQueryData(["claude_code", "sessions", ""], [summary, { ...summary, sessionKey: "other" }]);
  const renamed = { ...summary, title: "Read back title" };
  state.rename.mockResolvedValueOnce(renamed);
  await useClaudeActions().rename.mutateAsync({ sessionKey: summary.sessionKey, title: "Requested title" });
  expect(client.getQueryData(["claude_code", "session", summary.sessionKey])).toEqual({ pages: [{ ...page, summary: renamed }, { ...page, summary: renamed }], pageParams: [undefined, "older"] });
  expect(client.getQueryData(["claude_code", "sessions", ""])).toEqual([renamed, { ...summary, sessionKey: "other" }]);
  expect(client.getQueryState(["claude_code", "sessions", ""])?.isInvalidated).toBe(true);
  client.clear();
});
