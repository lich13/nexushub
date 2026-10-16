import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { desktopRefresh } from "./desktopRefresh";

type Deferred<T> = { promise: Promise<T>; resolve: (value: T) => void };

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(accept => { resolve = accept; });
  return { promise, resolve };
}

describe("Codex desktop detail refresh coalescer", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  test("coalesces bursts and delivers one trailing refresh after a slow read", async () => {
    const slow = deferred<void>();
    const refresh = vi.fn().mockImplementationOnce(() => slow.promise).mockResolvedValue(undefined);
    const scheduled = desktopRefresh(refresh, () => true);

    for (let index = 0; index < 64; index += 1) scheduled.request();
    expect(refresh).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(349);
    expect(refresh).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(refresh).toHaveBeenCalledOnce();

    // These represent a newly arrived approval followed by the final native
    // completion event while the original detail read is still in flight.
    for (let index = 0; index < 64; index += 1) scheduled.request();
    await vi.advanceTimersByTimeAsync(5_000);
    expect(refresh).toHaveBeenCalledOnce();

    slow.resolve(undefined);
    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(349);
    expect(refresh).toHaveBeenCalledOnce();
    await vi.advanceTimersByTimeAsync(1);
    expect(refresh).toHaveBeenCalledTimes(2);
    await vi.advanceTimersByTimeAsync(1_000);
    expect(refresh).toHaveBeenCalledTimes(2);
    scheduled.dispose();
  });

  test("the current-source guard and dispose prevent stale queued work", async () => {
    const refresh = vi.fn().mockResolvedValue(undefined);
    let current = true;
    const stale = desktopRefresh(refresh, () => current);
    stale.request();
    current = false;
    await vi.advanceTimersByTimeAsync(350);
    expect(refresh).not.toHaveBeenCalled();
    stale.dispose();

    current = true;
    const detached = desktopRefresh(refresh, () => current);
    detached.request();
    detached.dispose();
    await vi.advanceTimersByTimeAsync(1_000);
    expect(refresh).not.toHaveBeenCalled();
  });
});
