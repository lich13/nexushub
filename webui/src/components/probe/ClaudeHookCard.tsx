import { useEffect, useRef, useState } from "react";
import { useClaudeHooks } from "../../lib/query/probe";
import { ConfirmDialog } from "../common/ConfirmDialog";
import type { ClaudeHooksAction } from "../../types";
import { TerminalSquare, Trash2 } from "lucide-react";
import { Metric } from "../common/Panel";

export function ClaudeHookCard() {
  const [confirm, setConfirm] = useState<ClaudeHooksAction | null>(null);
  const installButton = useRef<HTMLButtonElement>(null);
  const removeButton = useRef<HTMLButtonElement>(null);
  const { local, revision, status, mutation } = useClaudeHooks(() => setConfirm(null));
  useEffect(() => setConfirm(null), [revision]);
  if (!local) return <div className="probe-card-stack"><Metric label="Claude Code Hooks" value="仅限本机" /></div>;
  const installed = status.data?.installed;
  const managedCount = status.data?.managedCount ?? 0;
  const busy = status.isLoading || status.isError || mutation.isPending;
  return <>
    <div className="probe-card-stack">
      <Metric label="Claude Code Hooks" value={status.isLoading ? "读取中" : status.isError ? "读取失败" : installed ? "已安装" : managedCount ? "需要修复" : "未安装"} tone={installed ? "success" : "warning"} />
      <Metric label="实际命令" value={status.data ? `${managedCount} 条` : "—"} tone={installed ? "success" : "warning"} />
      <div className="button-row">
        <button className="secondary-button" ref={installButton} disabled={busy || installed} onClick={() => setConfirm("install")}><TerminalSquare size={17} />{installed ? "已安装" : managedCount ? "修复 Hook" : "安装 Hook"}</button>
        <button className="secondary-button" ref={removeButton} disabled={!managedCount || busy} onClick={() => setConfirm("remove")}><Trash2 size={17} />移除 Hook</button>
      </div>
      {(status.error || mutation.error) && <div className="form-error" role="alert">{(status.error || mutation.error)?.message}</div>}
    </div>
    {confirm && <ConfirmDialog labelledBy="claude-hooks-confirm" busy={mutation.isPending} returnFocus={confirm === "install" ? installButton : removeButton} onCancel={() => setConfirm(null)}>
      <h3 id="claude-hooks-confirm">{confirm === "install" ? "安装本机 Claude hooks" : "移除本机 Claude hooks"}</h3>
      <p>{confirm === "install" ? "安装 NexusHub 通知 Hook，保留其他 Hook。" : "移除 NexusHub 通知 Hook，保留其他 Hook。"}</p>
      <div className="button-row"><button className="secondary-button" disabled={mutation.isPending} onClick={() => setConfirm(null)}>取消</button><button className={confirm === "install" ? "primary-button" : "danger-button"} disabled={mutation.isPending} onClick={() => mutation.mutate(confirm)}>确认{confirm === "install" ? "安装" : "移除"}</button></div>
    </ConfirmDialog>}
  </>;
}
