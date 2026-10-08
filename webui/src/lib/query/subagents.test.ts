import { useInfiniteQuery } from "@tanstack/react-query";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import type { SubagentDetailResponse } from "../../types";
import { getSubagentDetail } from "../api/threads";
import { machineScope } from "./connection";
import { useSubagentDetail } from "./subagents";

vi.mock("@tanstack/react-query", () => ({ useInfiniteQuery: vi.fn() }));
vi.mock("../api/threads", () => ({ getSubagentDetail: vi.fn() }));
vi.mock("./connection", () => ({ machineScope: vi.fn(() => "local:0") }));

type QueryOptions = {
  queryKey: unknown[];
  enabled: boolean;
  initialPageParam: string | null;
  queryFn: (context: { pageParam: string | null; signal: AbortSignal }) => Promise<SubagentDetailResponse>;
  getNextPageParam: (page: SubagentDetailResponse) => string | null | undefined;
  refetchInterval: () => number | false;
  retry: boolean;
};

function options(rootThreadId = "root-fixture", agentId: string | null | undefined = "child-fixture"): QueryOptions {
  useSubagentDetail(rootThreadId, agentId);
  const calls = vi.mocked(useInfiniteQuery).mock.calls;
  return calls[calls.length - 1][0] as unknown as QueryOptions;
}

function response(): SubagentDetailResponse {
  return {
    rootThreadId: "root-fixture", parentThreadId: "parent-fixture",
    agent: { agentId: "child-fixture", name: "示例子智能体", available: true, status: "running" },
    detail: {
      summary: { id: "child-fixture", title: "示例子智能体", status: "Running", message_count: 1 },
      blocks: [], messages: [], raw_event_count: 1, has_more_blocks: false
    }
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(machineScope).mockReturnValue("local:0");
  vi.mocked(getSubagentDetail).mockReset();
});
afterEach(() => vi.unstubAllGlobals());

describe("subagent detail queries", () => {
  test("first and older-page requests retain the original root and selected child", async () => {
    const data = response();
    vi.mocked(getSubagentDetail).mockResolvedValue(data);
    const query = options();
    const signal = new AbortController().signal;
    expect(query.initialPageParam).toBeNull();
    expect(await query.queryFn({ pageParam: null, signal })).toBe(data);
    expect(await query.queryFn({ pageParam: "block:120", signal })).toBe(data);
    expect(getSubagentDetail).toHaveBeenNthCalledWith(1, { rootThreadId: "root-fixture", agentId: "child-fixture", before: null, limit: 120 });
    expect(getSubagentDetail).toHaveBeenNthCalledWith(2, { rootThreadId: "root-fixture", agentId: "child-fixture", before: "block:120", limit: 120 });
  });

  test("query caches distinguish machine revision, root thread, and nested child", () => {
    const initial = options().queryKey;
    expect(options("other-root").queryKey).not.toEqual(initial);
    expect(options("root-fixture", "nested-fixture").queryKey).not.toEqual(initial);
    vi.mocked(machineScope).mockReturnValue("remote:1");
    expect(options().queryKey).not.toEqual(initial);
    const remote = options().queryKey;
    vi.mocked(machineScope).mockReturnValue("remote:2");
    expect(options().queryKey).not.toEqual(remote);
  });

  test.each([null, ""])("missing child identity %s disables fetching", agentId => {
    expect(options("root-fixture", agentId).enabled).toBe(false);
    expect(getSubagentDetail).not.toHaveBeenCalled();
  });

  test.each(["root", "child"])("a response with a mismatched %s identity is rejected", async field => {
    const data = response();
    if (field === "root") data.rootThreadId = "other-root";
    else data.agent.agentId = "other-child";
    vi.mocked(getSubagentDetail).mockResolvedValue(data);
    await expect(options().queryFn({ pageParam: null, signal: new AbortController().signal }))
      .rejects.toThrow("子智能体记录已变化，请重新打开");
  });

  test("an aborted selection discards a late native response", async () => {
    let resolve!: (data: SubagentDetailResponse) => void;
    vi.mocked(getSubagentDetail).mockImplementation(() => new Promise(done => { resolve = done; }));
    const controller = new AbortController();
    const pending = options().queryFn({ pageParam: null, signal: controller.signal });
    controller.abort();
    resolve(response());
    await expect(pending).rejects.toMatchObject({ name: "AbortError" });
  });

  test("only an advertised earlier page produces a cursor", () => {
    const query = options();
    const data = response();
    data.detail.before_cursor = "block:120";
    expect(query.getNextPageParam(data)).toBeUndefined();
    data.detail.has_more_blocks = true;
    expect(query.getNextPageParam(data)).toBe("block:120");
    data.detail.before_cursor = null;
    expect(query.getNextPageParam(data)).toBeNull();
  });

  test("native failures stay visible and do not silently retry", async () => {
    const error = new Error("示例记录暂时不可读");
    vi.mocked(getSubagentDetail).mockRejectedValue(error);
    const query = options();
    expect(query.retry).toBe(false);
    await expect(query.queryFn({ pageParam: null, signal: new AbortController().signal })).rejects.toBe(error);
    expect(getSubagentDetail).toHaveBeenCalledTimes(1);
  });

  test("background tabs stop polling", () => {
    vi.stubGlobal("document", { visibilityState: "visible" });
    const query = options();
    expect(query.refetchInterval()).toBe(5000);
    vi.stubGlobal("document", { visibilityState: "hidden" });
    expect(query.refetchInterval()).toBe(false);
  });
});
