import { createElement } from "react";
import { renderToString } from "react-dom/server";
import { QueryClient, useQuery, useQueryClient, type QueryObserverOptions } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { ThreadDetail, ThreadStatus } from "../../types";
import { getThread } from "../api";
import { threadDetailRefetchInterval } from "../domain/codexViewModel";
import { threadQueryKeys, useThreadCacheActions, useThreadDetailQuery } from "./threads";

vi.mock("@tanstack/react-query", async original => ({
  ...await original<typeof import("@tanstack/react-query")>(),
  useQuery: vi.fn(),
  useQueryClient: vi.fn()
}));
vi.mock("../api", async original => ({ ...await original<typeof import("../api")>(), getThread: vi.fn() }));

let client: QueryClient;
const readAt = 10000;

function detail(id: string, status: ThreadStatus = "Recent"): ThreadDetail {
  return {
    summary: { id, title: "Fixture thread", status, message_count: 1 },
    blocks: [{ id: `${id}-message`, kind: "message", role: "assistant", text: "Fixture reply", questions: [] }],
    messages: [], raw_event_count: 1, total_blocks: 1, has_more_blocks: false, before_cursor: null
  };
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(readAt);
  client = new QueryClient({ defaultOptions: { queries: { retry: false, gcTime: Infinity } } });
  vi.mocked(useQueryClient).mockReturnValue(client);
  vi.mocked(getThread).mockImplementation(async id => detail(id));
});

afterEach(() => { client.clear(); vi.clearAllMocks(); vi.useRealTimers(); });

function queryOptions(id: string) {
  function Probe() {
    useThreadDetailQuery({ threadId: id, refetchInterval: threadDetailRefetchInterval });
    return null;
  }
  renderToString(createElement(Probe));
  return vi.mocked(useQuery).mock.calls.at(-1)![0] as QueryObserverOptions<ThreadDetail>;
}

function cacheActions() {
  let actions!: ReturnType<typeof useThreadCacheActions>;
  function Probe() { actions = useThreadCacheActions(); return null; }
  renderToString(createElement(Probe));
  return actions;
}

test.each<[ThreadStatus, number]>([["Recent", 5000], ["Running", 2000]])(
  "%s thread switches reuse the current polling window and refresh after it expires", async (status, interval) => {
    vi.mocked(getThread).mockImplementation(async id => detail(id, status));
    const first = queryOptions("thread-a"), second = queryOptions("thread-b");
    const cached = await client.fetchQuery(first);
    await client.fetchQuery(second);
    vi.setSystemTime(readAt + interval - 1);
    expect(await client.fetchQuery(first)).toBe(cached);
    expect(getThread).toHaveBeenCalledTimes(2);

    vi.setSystemTime(readAt + interval);
    await client.fetchQuery(first);
    expect(getThread).toHaveBeenCalledTimes(3);
    expect(getThread).toHaveBeenLastCalledWith("thread-a", { before: undefined });
  }
);

test("list summary updates do not renew transcript freshness", async () => {
  const options = queryOptions("thread-a");
  await client.fetchQuery(options);
  vi.setSystemTime(readAt + 4000);
  cacheActions().mergeThreadDetailSummary("thread-a", { title: "Updated fixture title" });
  expect(client.getQueryData<ThreadDetail>(threadQueryKeys.thread("thread-a"))?.summary.title).toBe("Updated fixture title");
  expect(client.getQueryState(threadQueryKeys.thread("thread-a"))?.dataUpdatedAt).toBe(readAt);

  vi.setSystemTime(readAt + 5000);
  await client.fetchQuery(options);
  expect(getThread).toHaveBeenCalledTimes(2);
});

test("explicit invalidation still refreshes a recently cached transcript", async () => {
  const options = queryOptions("thread-a");
  await client.fetchQuery(options);
  await client.invalidateQueries({ queryKey: threadQueryKeys.thread("thread-a"), refetchType: "none" });
  await client.fetchQuery(options);
  expect(getThread).toHaveBeenCalledTimes(2);
});
