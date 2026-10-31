export function preservePreviousQueryData<T>(previous: T | undefined): T | undefined {
  return previous;
}

export function visiblePollInterval(ms: number | false): () => number | false {
  return () =>
    typeof document !== "undefined" && document.visibilityState !== "visible"
      ? false
      : ms;
}
