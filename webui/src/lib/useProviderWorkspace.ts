import { useCallback, useEffect, useState, type RefObject } from "react";
import type { SessionSearchResult } from "../types";
import { locateTimelineTarget } from "../components/common/SessionSearch";

export function useNarrowViewport(breakpoint = "(max-width: 767px)"): boolean {
  const [narrow, setNarrow] = useState(
    () => typeof window !== "undefined" && window.matchMedia(breakpoint).matches,
  );
  useEffect(() => {
    const media = window.matchMedia(breakpoint);
    const update = () => setNarrow(media.matches);
    media.addEventListener("change", update);
    update();
    return () => media.removeEventListener("change", update);
  }, [breakpoint]);
  return narrow;
}

export function useTimelineNavigator(input: {
  streamRef: RefObject<HTMLElement | null>;
  followRef: { current: boolean };
  capture: () => void;
  hasResolvedTimeline: boolean;
  selectWindowTarget: (id: string) => void;
}) {
  const { streamRef, followRef, capture, hasResolvedTimeline, selectWindowTarget } = input;
  return useCallback(
    (id: string) => {
      followRef.current = false;
      if (streamRef.current && locateTimelineTarget(id, streamRef.current, "auto")) {
        capture();
        return;
      }
      if (hasResolvedTimeline) {
        selectWindowTarget(id);
      }
    },
    [capture, followRef, hasResolvedTimeline, selectWindowTarget, streamRef],
  );
}

export function useProviderSearchLocator(input: {
  pendingSearch: SessionSearchResult | null;
  selectedSessionKey: string | null | undefined;
  isLoading: boolean;
  isFetching: boolean;
  hasNextPage: boolean;
  navigationActive: boolean;
  navigationLatest: () => void;
  historyLoading: boolean;
  historyError: unknown;
  historyLoadOlder: (reason?: "scroll" | "search") => Promise<unknown>;
  followRef: { current: boolean };
  capture: () => void;
  streamRef: RefObject<HTMLElement | null>;
  setFeedback: (value: string) => void;
  setPendingSearch: (value: SessionSearchResult | null) => void;
  events: unknown[];
}) {
  const {
    pendingSearch,
    selectedSessionKey,
    isLoading,
    isFetching,
    hasNextPage,
    navigationActive,
    navigationLatest,
    historyLoading,
    historyError,
    historyLoadOlder,
    followRef,
    capture,
    streamRef,
    setFeedback,
    setPendingSearch,
    events,
  } = input;

  useEffect(() => {
    if (
      !pendingSearch ||
      pendingSearch.sessionKey !== selectedSessionKey ||
      isLoading ||
      isFetching ||
      historyLoading
    ) {
      return;
    }
    if (navigationActive) {
      navigationLatest();
      return;
    }
    followRef.current = false;
    if (
      locateTimelineTarget(
        pendingSearch.positionKey,
        streamRef.current?.closest(".provider-detail") ?? undefined,
        "auto",
      )
    ) {
      capture();
      setFeedback("");
      setPendingSearch(null);
    } else if (hasNextPage) {
      if (!historyError) void historyLoadOlder("search");
    } else {
      setFeedback("结果所在历史未加载，请刷新后重试");
      setPendingSearch(null);
    }
  }, [
    capture,
    events,
    followRef,
    hasNextPage,
    historyError,
    historyLoadOlder,
    historyLoading,
    isFetching,
    isLoading,
    navigationActive,
    navigationLatest,
    pendingSearch,
    selectedSessionKey,
    setFeedback,
    setPendingSearch,
    streamRef,
  ]);
}
