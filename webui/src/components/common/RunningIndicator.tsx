export function RunningIndicator() {
  return (
    <span className="thread-running-indicator" aria-label="运行中" title="运行中">
      <span className="thread-running-spinner" aria-hidden="true" />
    </span>
  );
}
