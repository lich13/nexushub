import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";
import type { DesktopPendingRequest, DesktopQueuedMessage, DesktopView } from "../../lib/api/desktop";
import { DesktopStatus } from "./DesktopStatus";

function desktopView(overrides: Partial<DesktopView> = {}): DesktopView {
  return {
    sessionId: "session-fixture",
    threadId: "thread-fixture",
    revision: 1,
    connected: true,
    writable: true,
    runtime: "codex",
    activeTurnId: null,
    model: null,
    reasoningEffort: null,
    mode: null,
    requests: [],
    queue: [],
    error: null,
    ...overrides
  };
}

function queued(status: string, index: number): DesktopQueuedMessage {
  return { id: `message-${index}`, status, kind: "queue", preview: "fixture", createdAt: index };
}

function request(): DesktopPendingRequest {
  return {
    key: "request-fixture",
    kind: "command",
    title: "Fixture request",
    body: "",
    questions: [],
    formSchema: null,
    canAccept: true
  };
}

function markup(view: DesktopView | null): string {
  return renderToStaticMarkup(<DesktopStatus view={view} />);
}

describe("desktop collaboration status", () => {
  test.each([
    ["no snapshot", null],
    ["disconnected snapshot", desktopView({ connected: false })],
    ["read-only snapshot", desktopView({ writable: false })]
  ])("does not render for a %s", (_description, view) => {
    expect(markup(view)).toBe("");
  });

  test("shows an idle row for the current writable snapshot", () => {
    const html = markup(desktopView());

    expect(html).toContain('class="desktop-collaboration-status" role="status"');
    expect(html).toContain("空闲");
    expect(html).not.toContain("运行中");
  });

  test("pending requests and runtime waiting outrank active work and queued submissions", () => {
    const html = markup(desktopView({
      runtime: "waiting",
      activeTurnId: "turn-fixture",
      requests: [request()],
      queue: [queued("dispatching", 1), queued("queued", 2), queued("outcome_unknown", 3), queued("held", 4)]
    }));

    expect(html).toContain("等待你处理");
    expect(html).toContain("4 条待处理");
    expect(html).not.toContain("运行中");
  });

  test("active turns outrank dispatching, queued, and uncertain submissions", () => {
    const html = markup(desktopView({
      activeTurnId: "turn-fixture",
      queue: [queued("dispatching", 1), queued("queued", 2), queued("outcome_unknown", 3)]
    }));

    expect(html).toContain("执行中");
    expect(html).toContain('aria-label="运行中"');
    expect(html).toContain("3 条待处理");
  });

  test("dispatching outranks queued and uncertain submissions", () => {
    const html = markup(desktopView({
      queue: [queued("queued", 1), queued("outcome_unknown", 2), queued("dispatching", 3)]
    }));

    expect(html).toContain("正在提交到 Codex…");
    expect(html).toContain('aria-label="运行中"');
  });

  test("queued submissions outrank uncertain and held submissions", () => {
    const html = markup(desktopView({ queue: [queued("outcome_unknown", 1), queued("held", 2), queued("queued", 3)] }));

    expect(html).toContain("队列中");
    expect(html).not.toContain("运行中");
  });

  test("outcome-unknown and held submissions keep distinct safe labels", () => {
    expect(markup(desktopView({ queue: [queued("outcome_unknown", 1)] }))).toContain("发送结果待核对");
    expect(markup(desktopView({ queue: [queued("held", 1)] }))).toContain("队列已暂停");
  });

  test("keeps the queue count in the same status row", () => {
    const html = markup(desktopView({ queue: [queued("queued", 1), queued("queued", 2)] }));

    expect(html.match(/class="desktop-collaboration-status"/g)).toHaveLength(1);
    expect(html).toContain('class="muted-text"> · 2 条待处理</span>');
  });
});
