import { Archive, Copy, Edit3, Files, GitFork, SlidersHorizontal, TerminalSquare, Undo2 } from "lucide-react";
import { useCallback } from "react";
import { Metric, Panel } from "../common/Panel";
import { threadCopyId, threadResumeCommand, threadRolloutPath } from "../../lib/domain/conversationViewModel";
import type { ThreadSummary } from "../../types";
import { ThreadGoalPanel } from "./ThreadGoalPanel";

export function ThreadInspectorPanels({
  summary,
  csrfToken,
  renameValue,
  setRenameValue,
  setRenameDirty,
  renamePending,
  onRename,
  archivePending,
  onArchive,
  forkPending,
  onFork,
  showFork,
  showArchive,
  onFeedback
}: {
  summary: ThreadSummary;
  csrfToken?: string | null;
  renameValue: string;
  setRenameValue: (value: string) => void;
  setRenameDirty: (dirty: boolean) => void;
  renamePending: boolean;
  onRename: () => void;
  archivePending: boolean;
  onArchive: () => void;
  forkPending: boolean;
  onFork: () => void;
  showFork: boolean;
  showArchive: boolean;
  onFeedback: (message: string | null) => void;
}) {
  const copyText = useCallback((text: string | null, message: string) => {
    if (!text) return;
    navigator.clipboard?.writeText(text);
    onFeedback(message);
  }, [onFeedback]);
  const copyId = threadCopyId(summary.id);
  const rolloutPath = threadRolloutPath(summary.rollout_path);
  const resumeCommand = threadResumeCommand(summary.id);

  return (
    <>
      <Panel title="名称与归档" icon={<SlidersHorizontal size={18} />}>
        <label className="field-label">线程标题<input value={renameValue} onChange={(event) => {
          setRenameDirty(true);
          setRenameValue(event.target.value);
        }} /></label>
        <div className="button-row">
          <button className="secondary-button" onClick={onRename} disabled={!renameValue.trim() || renamePending}><Edit3 size={17} />重命名</button>
          <button className={summary.status === "Archived" ? "secondary-button" : "danger-button soft"} onClick={onArchive} disabled={archivePending || !showArchive}>
            {summary.status === "Archived" ? <Undo2 size={17} /> : <Archive size={17} />}
            {summary.status === "Archived" ? "恢复" : "归档"}
          </button>
        </div>
        {showFork && (
          <button className="secondary-button full-width-action" onClick={onFork} disabled={forkPending}>
            <GitFork size={17} />Fork
          </button>
        )}
      </Panel>

      <ThreadGoalPanel threadId={summary.id} csrfToken={csrfToken} onFeedback={onFeedback} />

      <Panel title="复制与路径" icon={<Files size={18} />}>
        <Metric label="线程 ID" value={copyId || "无"} wide />
        <Metric label="会话文件" value={rolloutPath || "无会话文件"} wide />
        <div className="copy-row">
          <button className="secondary-button" onClick={() => copyText(copyId, "已复制线程 ID")} disabled={!copyId}>
            <Copy size={17} />复制 ID
          </button>
          <button className="secondary-button" onClick={() => copyText(rolloutPath, "已复制文件路径")} disabled={!rolloutPath}>
            <Copy size={17} />复制文件路径
          </button>
          <button className="secondary-button" onClick={() => copyText(resumeCommand, "已复制 resume 命令")} disabled={!resumeCommand}>
            <TerminalSquare size={17} />复制 codex resume+ID
          </button>
        </div>
      </Panel>
    </>
  );
}
