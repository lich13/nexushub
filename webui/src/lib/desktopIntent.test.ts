import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";

const machine = vi.hoisted(() => ({ revision: 1 }));

vi.mock("./query/connection", () => ({
  connectionSnapshot: () => ({ target: "local", revision: machine.revision })
}));

import { consumeDesktopOpenIntent, requestDesktopOpenIntent } from "./desktopIntent";

beforeEach(() => {
  machine.revision = 1;
});

afterEach(() => {
  vi.useRealTimers();
});

describe("desktop thread creation intent", () => {
  test("is thread-scoped and can be consumed only once", () => {
    requestDesktopOpenIntent("thread-created");

    expect(consumeDesktopOpenIntent("thread-other")).toBe(false);
    expect(consumeDesktopOpenIntent("thread-created")).toBe(true);
    expect(consumeDesktopOpenIntent("thread-created")).toBe(false);
  });

  test("expires after twenty seconds", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-17T00:00:00Z"));
    requestDesktopOpenIntent("thread-created");

    vi.advanceTimersByTime(20_001);

    expect(consumeDesktopOpenIntent("thread-created")).toBe(false);
  });

  test("is invalidated by a machine revision change", () => {
    requestDesktopOpenIntent("thread-created");
    machine.revision += 1;

    expect(consumeDesktopOpenIntent("thread-created")).toBe(false);
    expect(consumeDesktopOpenIntent("thread-created")).toBe(false);
  });
});
