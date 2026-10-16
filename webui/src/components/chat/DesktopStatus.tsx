import type { DesktopView } from "../../lib/query/desktop";
import { RunningIndicator } from "../common/RunningIndicator";

/** Only the current follower snapshot supplies status; no parallel turn state. */
export function DesktopStatus({ view }: { view: DesktopView | null }) {
  if (!view?.connected || !view.writable) return null;
  const waiting = view.requests.length > 0 || view.runtime === "waiting";
  const active = view.runtime === "active" || Boolean(view.activeTurnId);
  const dispatching = view.queue.some(item => item.status === "dispatching");
  const queued = view.queue.some(item => item.status === "queued");
  const unknown = view.queue.some(item => item.status === "outcome_unknown");
  const label = waiting ? "等待你处理" : active ? "执行中" : dispatching ? "正在发送" : queued ? "队列中" : unknown ? "发送结果待核对" : view.queue.length ? "队列已暂停" : "空闲";
  return <div className="desktop-collaboration-status" role="status">
    {!waiting && (active || dispatching) && <RunningIndicator />}
    <span>{label}</span>{view.queue.length > 0 && <span className="muted-text"> · {view.queue.length} 条待处理</span>}
  </div>;
}
