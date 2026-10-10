export function HistoryLoading({ loading, error, retry }: { loading: boolean; error: string | null; retry: () => void }) {
  if (error) return <div className="history-loading" role="status">加载失败，<button type="button" className="file-path-label" onClick={retry}>重试</button></div>;
  return loading ? <div className="history-loading" role="status">正在加载较早消息…</div> : null;
}
