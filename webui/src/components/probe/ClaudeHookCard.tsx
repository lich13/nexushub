import { useEffect, useRef, useState } from "react";
import { useClaudeHooks } from "../../lib/query/probe";
import { ConfirmDialog } from "../common/ConfirmDialog";
import type { ClaudeHooksAction } from "../../types";

export function ClaudeHookCard() {
  const [confirm, setConfirm] = useState<ClaudeHooksAction | null>(null);
  const installButton = useRef<HTMLButtonElement>(null);
  const { local, revision, status, mutation } = useClaudeHooks(() => setConfirm(null));
  useEffect(() => setConfirm(null), [revision]);
  if (!local) return <div className="muted-row">Claude hooks 仅可在本机设置；远端继续使用轮询通知。</div>;
  return <>
    <p>{status.data?.installed ? "本机 Claude hooks 已安装。" : status.data?.managedCount ? "本机 Claude hooks 需要修复。" : "本机 Claude hooks 未安装。"} 完成、问题、授权及失败提醒沿用 Bark 和 Claude 通知开关。</p>
    <div className="button-row">
      <button ref={installButton} disabled={status.isLoading || status.isError || mutation.isPending || status.data?.installed} onClick={() => setConfirm("install")}>{status.data?.managedCount ? "修复 Claude hooks" : "安装 Claude hooks"}</button>
      <button disabled={!status.data?.managedCount || mutation.isPending} onClick={() => setConfirm("remove")}>移除 Claude hooks</button>
    </div>
    {(status.error || mutation.error) && <p role="alert">{(status.error || mutation.error)?.message}</p>}
    {confirm && <ConfirmDialog labelledBy="claude-hooks-confirm" busy={mutation.isPending} returnFocus={installButton} onCancel={() => setConfirm(null)}>
      <h3 id="claude-hooks-confirm">{confirm === "install" ? "安装本机 Claude hooks" : "移除本机 Claude hooks"}</h3>
      <p>{confirm === "install" ? "将更新本机 Claude 设置，保留已有 hooks。完成、问题、授权及失败时可发送 Bark 通知；同一回合每类提醒一次。完成后不再单独提醒空闲。" : "将移除 NexusHub 管理的 Claude hooks，保留其他 hooks，并继续使用轮询通知。"}</p>
      <div className="button-row"><button disabled={mutation.isPending} onClick={() => setConfirm(null)}>取消</button><button disabled={mutation.isPending} onClick={() => mutation.mutate(confirm)}>确认{confirm === "install" ? "安装" : "移除"}</button></div>
    </ConfirmDialog>}
  </>;
}
