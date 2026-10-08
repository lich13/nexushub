import { describe, expect, test } from "vitest";
import type { SubagentActivity, ThreadDetail } from "../../types";
import { threadDetailRefetchInterval } from "./codexViewModel";

function detail(status: SubagentActivity["status"]): ThreadDetail {
  return {
    summary: { id: "root-fixture", title: "父线程示例", status: "Recent", message_count: 0 },
    blocks: [], messages: [], raw_event_count: 0,
    subagents: {
      agents: [{ agentId: "child-fixture", name: "直属示例", status, available: true }],
      counts: { creating: 0, running: 0, completed: 0, failed: 0, interrupted: 0, unknown: 0, [status]: 1 },
      complete: true
    }
  };
}

describe("parent detail polling with direct child activity", () => {
  test.each(["creating", "running"] as const)("an idle parent refreshes every two seconds while a direct child is %s", status => {
    expect(threadDetailRefetchInterval(detail(status))).toBe(2000);
  });

  test.each(["completed", "failed", "interrupted", "unknown"] as const)("a %s direct child does not keep an idle parent at the active interval", status => {
    expect(threadDetailRefetchInterval(detail(status))).toBe(5000);
  });

  test("historical started events do not override a confirmed settled collection", () => {
    const data = detail("completed");
    data.blocks = [{
      id: "old-start", role: "tool", kind: "subagent_activity", questions: [],
      subagent: { agentId: "child-fixture", name: "直属示例", status: "running", available: true, eventKind: "started", eventId: "start-fixture" }
    }];
    expect(threadDetailRefetchInterval(data)).toBe(5000);
  });

  test("a running parent keeps the active interval after all direct children have settled", () => {
    const data = detail("completed");
    data.summary.status = "Running";
    expect(threadDetailRefetchInterval(data)).toBe(2000);
  });

  test("legacy detail without the collection and initial summary fallbacks remain supported", () => {
    const data = detail("completed");
    delete data.subagents;
    expect(threadDetailRefetchInterval(data)).toBe(5000);
    expect(threadDetailRefetchInterval(undefined, { status: "Running" })).toBe(2000);
    expect(threadDetailRefetchInterval(undefined, { status: "Recent" })).toBe(5000);
  });
});
