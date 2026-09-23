import { useRef } from "react";
import type { SessionSelection, SessionOperation } from "../../lib/query/sessions";
import { ConfirmDialog } from "./ConfirmDialog";

const labels: Record<SessionOperation, string> = { archive: "归档", restore: "恢复", delete: "删除" };
export function SessionCheckbox({ batch, sessionKey, title }: { batch: SessionSelection; sessionKey: string; title: string }) {
  return <input className="session-checkbox" type="checkbox" aria-label={`选择 ${title}`} checked={batch.selection.includes(sessionKey)} disabled={batch.busy || (batch.selection.length >= 100 && !batch.selection.includes(sessionKey))} onChange={() => batch.toggle(sessionKey)} />;
}
export function SessionBatchControls({ batch, operations }: { batch: SessionSelection; operations: SessionOperation[] }) {
  const trigger = useRef<HTMLElement | null>(null);
  const allowed = batch.preview?.items.filter(i => i.allowed).length ?? 0;
  return <div className="session-batch-controls">
    {!batch.selecting ? <button className="secondary-button" onClick={batch.begin}>多选线程</button> : <>
      <div className="button-row"><span role="status">已选 {batch.selection.length} / 100</span><button disabled={batch.busy} onClick={batch.selectAll}>全选当前结果</button><button disabled={batch.busy} onClick={batch.clear}>清空</button><button disabled={batch.busy} onClick={batch.cancel}>取消多选</button></div>
      <div className="button-row">{operations.map(operation => <button key={operation} className={operation === "delete" ? "danger-button" : "secondary-button"} disabled={!batch.selection.length || batch.busy} onClick={(event) => { trigger.current = event.currentTarget; batch.execute.reset(); batch.prepare.mutate(operation); }}>{labels[operation]}所选线程</button>)}</div>
    </>}
    {batch.prepare.error && <p role="alert" className="form-error">{batch.prepare.error.message}</p>}
    {batch.execute.error && <p role="alert" className="form-error">{batch.execute.error.message}。请刷新并重新预览。</p>}
    {batch.result && <div role="status"><p>已完成 {batch.result.items.filter(i => i.status === "succeeded").length} 项，未完成 {batch.result.items.filter(i => i.status !== "succeeded").length} 项</p>{batch.result.items.filter(i => i.status !== "succeeded").map(item => <p className="form-error" key={item.sessionKey}>{item.sessionKey}：{item.message}</p>)}</div>}
    {batch.preview && <ConfirmDialog labelledBy="batch-confirm-title" busy={batch.execute.isPending} onCancel={batch.dismissPreview} returnFocus={trigger}>
      <h2 id="batch-confirm-title">{labels[batch.preview.operation]}所选线程</h2>
      <p>可执行 {allowed} 项，受保护 {batch.preview.items.length - allowed} 项。{batch.preview.operation === "delete" ? "仅删除下列会话数据，工作目录和配置保留。" : "确认后更新所选线程的归档状态。"}</p>
      <ul className="batch-preview-items">{batch.preview.items.map(item => <li key={item.sessionKey}><strong>{item.title}</strong><small>{item.id}</small>{item.paths.map(path => <code className="delete-path" key={path}>{path}</code>)}<span>{item.bytes.toLocaleString()} 字节</span>{!item.allowed && <span className="form-error">{item.reason}</span>}</li>)}</ul>
      {batch.execute.error && <p role="alert" className="form-error">{batch.execute.error.message}</p>}
      <div className="button-row"><button disabled={batch.execute.isPending} onClick={batch.dismissPreview}>取消</button><button className={batch.preview.operation === "delete" ? "danger-button" : "secondary-button"} disabled={!allowed || batch.execute.isPending} onClick={() => batch.execute.mutate()}>{batch.execute.isPending ? "正在处理…" : `确认${labels[batch.preview.operation]} ${allowed} 项`}</button></div>
    </ConfirmDialog>}
  </div>;
}
