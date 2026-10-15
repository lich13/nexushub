export function TimelineHistoryStatus({ navigation }: { navigation: {
  active: boolean; busy: boolean; error: string | null; hasNewer: boolean; latest: () => void; newer: () => void;
} }) {
  return <>
    {navigation.busy && <div className="muted-row" role="status">正在定位历史…</div>}
    {navigation.error && <div className="form-error" role="alert">{navigation.error}</div>}
    {navigation.active && <div className="timeline-history-actions">
      {navigation.hasNewer && <button className="file-path-label" disabled={navigation.busy} onClick={navigation.newer}>加载较新记录</button>}
      <button className="file-path-label" onClick={navigation.latest}>返回最新</button>
    </div>}
  </>;
}
