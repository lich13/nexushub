import { useCallback, useLayoutEffect, useRef, useState, type UIEvent, type WheelEvent, type TouchEvent, type KeyboardEvent } from "react";
import { createHistoryScrollCoordinator, type HistoryScrollOptions } from "./historyScrollCoordinator";

function innerScroll(target: EventTarget, root: HTMLElement): boolean {
  let element = typeof HTMLElement !== "undefined" && target instanceof HTMLElement ? target : null;
  while (element && element !== root) {
    if (element.scrollHeight > element.clientHeight && element.scrollTop > 0) return true;
    element = element.parentElement;
  }
  return false;
}

/** Shares one coordinator between scroll loading, search and bottom following. */
export function useHistoryScroll(options: HistoryScrollOptions) {
  const [, redraw] = useState(0);
  const controller = useRef<ReturnType<typeof createHistoryScrollCoordinator>>();
  if (!controller.current) controller.current = createHistoryScrollCoordinator(() => redraw(value => value + 1));
  const history = controller.current;
  history.update(options);
  const touchY = useRef<number | null>(null);
  useLayoutEffect(() => () => history.dispose(), [history]);
  useLayoutEffect(() => { history.onRendered(); });

  const onScroll = useCallback((event: UIEvent<HTMLDivElement>) => history.onScroll(event.currentTarget), [history]);
  const onWheel = useCallback((event: WheelEvent<HTMLDivElement>) => { if (event.deltaY < 0 && !innerScroll(event.target, event.currentTarget)) history.upwardIntent(); }, [history]);
  const onTouchStart = useCallback((event: TouchEvent<HTMLDivElement>) => { touchY.current = event.touches[0]?.clientY ?? null; }, []);
  const onTouchMove = useCallback((event: TouchEvent<HTMLDivElement>) => {
    const y = event.touches[0]?.clientY;
    if (y !== undefined && touchY.current !== null && y > touchY.current && !innerScroll(event.target, event.currentTarget)) history.upwardIntent();
    touchY.current = y ?? null;
  }, [history]);
  const onKeyDown = useCallback((event: KeyboardEvent<HTMLDivElement>) => {
    if ((event.target as HTMLElement).closest("input,textarea,[contenteditable=true]")) return;
    if (["ArrowUp", "PageUp", "Home"].includes(event.key)) history.upwardIntent();
  }, [history]);
  const retry = useCallback(() => {
    options.streamRef.current?.focus({ preventScroll: true });
    history.retry();
  }, [history, options.streamRef]);
  return { onScroll, onWheel, onTouchStart, onTouchMove, onKeyDown, capture: history.capture, follow: history.follow,
    loadOlder: history.loadOlder, retry, loading: history.loading, error: history.error };
}
