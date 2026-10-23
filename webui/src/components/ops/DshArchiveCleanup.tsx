import { useState } from "react";
import { Archive, Database, Trash2 } from "lucide-react";
import { OPS_PANEL_TITLES } from "../../lib/domain/runtimeViewModel";
import { Metric, Panel } from "../common/Panel";
import { useDshArchiveActions } from "../../lib/query/ops";
import type { DshArchiveDeletePlan, DshArchiveDeleteResult } from "../../types";

export function DshArchiveCleanup({ supported }: { supported: boolean }) {
  const [plan, setPlan] = useState<DshArchiveDeletePlan | null>(null);
  const [result, setResult] = useState<DshArchiveDeleteResult | null>(null);
  const [armed, setArmed] = useState(false);
  const [page, setPage] = useState(0);
  const actions = useDshArchiveActions({
    supported,
    onPreview: next => { setPlan(next); setResult(null); setArmed(false); setPage(0); },
    onExecute: next => { setResult(next); setPlan(null); setArmed(false); setPage(0); }
  });
  const busy = actions.preview.isPending || actions.execute.isPending;
  const error = actions.execute.error ?? actions.preview.error;
  const rows = result?.items ?? plan?.candidates ?? [];
  const pages = Math.max(1, Math.ceil(rows.length / 100));
  return <Panel title={OPS_PANEL_TITLES.dshArchiveCleanup} icon={<Archive size={18} />}>
    {!supported ? <div className="muted-row">当前机器不支持 DSH 归档清理，请更新 NexusHub。</div> : <>
      <div className="archive-plan">
        <Metric label="归档线程" value={plan ? String(plan.archivedThreads) : "未预览"} />
        <Metric label="含子线程" value={plan ? String(plan.candidates.length) : "未预览"} />
      </div>
      {plan && !rows.length && <div className="muted-row">{plan.available ? "没有可清理的 DSH 归档线程。" : "当前机器未发现 DSH 归档数据。"}</div>}
      {plan?.issues.map(issue => <div role="alert" className="form-error" key={issue}>{issue}</div>)}
      {result && <div role="status">已清理 {result.deletedThreads} 个线程。{result.items.some(item => item.status !== "deleted") && "未清理的项目见下方；重新预览后可重试。"}</div>}
      <ul className="hidden-cleanup-candidates">{rows.slice(page * 100, (page + 1) * 100).map(row => <li key={row.id}>
        <code>{row.id}</code>
        {"parentId" in row && row.parentId && <span>子线程</span>}
        {"status" in row && <span>{row.status === "deleted" ? "已清理" : row.status === "skipped" ? "已跳过" : "清理失败"}</span>}
        {row.reason && <span>{row.reason}</span>}
        {"residuals" in row && row.residuals?.map(path => <code className="delete-path" key={path}>{path}</code>)}
      </li>)}</ul>
      {pages > 1 && <div className="button-row">
        <button className="secondary-button" disabled={page === 0} onClick={() => setPage(page - 1)}>上一页</button>
        <span>{page + 1} / {pages}</span>
        <button className="secondary-button" disabled={page + 1 >= pages} onClick={() => setPage(page + 1)}>下一页</button>
      </div>}
      {error && <div role="alert" className="form-error cleanup-error">{error.message}</div>}
      {armed && <div className="muted-row">请先退出当前机器的 DSH。将清理本次预览中的 {plan?.candidates.length ?? 0} 个线程及其专属数据。</div>}
      <div className="button-row ops-action-row cleanup-actions">
        <button className="secondary-button" disabled={busy} onClick={() => { actions.execute.reset(); setArmed(false); actions.preview.mutate(); }}><Database size={17} />Dry-run</button>
        {!armed ? <button className="danger-button soft" disabled={busy || !plan?.candidates.length} onClick={() => setArmed(true)}><Trash2 size={17} />清理归档</button> : <>
          <button className="danger-button" disabled={busy} onClick={() => actions.execute.mutate({ confirmed: true, targets: (plan?.candidates ?? []).map(({ id, archiveId }) => ({ id, archiveId })) })}><Trash2 size={17} />确认清理归档</button>
          <button className="secondary-button" disabled={busy} onClick={() => setArmed(false)}>取消</button>
        </>}
      </div>
    </>}
  </Panel>;
}
