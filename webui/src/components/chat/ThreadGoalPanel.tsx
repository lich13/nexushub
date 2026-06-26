import { CheckCircle2, ClipboardCheck, Play, Square, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Metric, Panel } from "../common/Panel";
import {
  formatGoalTimestamp,
  goalControlState,
  goalStatusLabel,
  goalStatusTone
} from "../../lib/domain/runtimeViewModel";
import { useThreadGoalActions, useThreadGoalQuery } from "../../lib/query/threads";
import type { CodexGoal, CodexGoalSaveInput } from "../../types";

export function ThreadGoalPanel({ threadId, csrfToken, onFeedback }: {
  threadId: string;
  csrfToken?: string | null;
  onFeedback: (message: string | null) => void;
}) {
  const goal = useThreadGoalQuery(threadId);
  const [objective, setObjective] = useState("");
  const [tokenBudget, setTokenBudget] = useState("");
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const currentGoal = goal.data;

  useEffect(() => {
    if (!currentGoal || dirty) return;
    setObjective(currentGoal.objective ?? "");
    setTokenBudget(currentGoal.token_budget === null || currentGoal.token_budget === undefined ? "" : String(currentGoal.token_budget));
  }, [currentGoal, dirty]);

  useEffect(() => {
    setDirty(false);
    setError(null);
  }, [threadId]);

  const afterGoalSuccess = useCallback((next: CodexGoal, message: string) => {
    setDirty(false);
    setObjective(next.objective ?? "");
    setTokenBudget(next.token_budget === null || next.token_budget === undefined ? "" : String(next.token_budget));
    setError(null);
    onFeedback(message);
  }, [onFeedback]);

  const onGoalError = useCallback((err: Error) => {
    setError(err.message);
    onFeedback(err.message);
  }, [onFeedback]);

  const goalActions = useThreadGoalActions({
    threadId,
    csrfToken,
    saveInput: () => goalSaveInput(objective, tokenBudget),
    onSuccess: afterGoalSuccess,
    onError: onGoalError
  });
  const saveGoalMutation = goalActions.save;
  const clearGoalMutation = goalActions.clear;
  const pauseGoalMutation = goalActions.pause;
  const resumeGoalMutation = goalActions.resume;

  const busy = saveGoalMutation.isPending || clearGoalMutation.isPending || pauseGoalMutation.isPending || resumeGoalMutation.isPending;
  const controls = goalControlState(currentGoal, { busy, objective, tokenBudget });
  const unavailable = currentGoal?.available === false;

  return (
    <Panel title="Goal" icon={<ClipboardCheck size={18} />}>
      <div className="settings-meta-grid">
        <Metric label="状态" value={goalStatusLabel(currentGoal, goal.isLoading)} tone={goalStatusTone(currentGoal)} />
        <Metric label="预算" value={currentGoal?.token_budget === null || currentGoal?.token_budget === undefined ? "无" : String(currentGoal.token_budget)} />
        {currentGoal?.completed_at ? <Metric label="完成时间" value={formatGoalTimestamp(currentGoal.completed_at)} /> : null}
        {currentGoal?.blocked_reason ? <Metric label="阻塞原因" value={currentGoal.blocked_reason} tone="danger" /> : null}
      </div>
      <label className="field-label">目标<input value={objective} onChange={(event) => {
        setDirty(true);
        setObjective(event.target.value);
      }} placeholder={goal.isLoading ? "正在读取 Goal" : "输入当前线程目标"} /></label>
      <label className="field-label">Token budget<input type="number" min={1} value={tokenBudget} onChange={(event) => {
        setDirty(true);
        setTokenBudget(event.target.value);
      }} placeholder="可选" /></label>
      <div className="button-row">
        <button className="primary-button" disabled={controls.saveDisabled || unavailable} onClick={() => saveGoalMutation.mutate()}><CheckCircle2 size={17} />保存</button>
        <button className="secondary-button" disabled={controls.clearDisabled || unavailable} onClick={() => clearGoalMutation.mutate()}><Trash2 size={17} />清除</button>
        <button className="secondary-button" disabled={controls.pauseDisabled || unavailable} onClick={() => pauseGoalMutation.mutate()}><Square size={17} />暂停</button>
        <button className="secondary-button" disabled={controls.resumeDisabled || unavailable} onClick={() => resumeGoalMutation.mutate()}><Play size={17} />恢复</button>
      </div>
      {error && <div className="form-error">{error}</div>}
      {unavailable && <div className="muted-row">Goal 接口未接入</div>}
    </Panel>
  );
}

function goalSaveInput(objective: string, tokenBudget: string): CodexGoalSaveInput {
  return {
    objective: objective.trim(),
    token_budget: tokenBudget.trim() ? Number.isFinite(Number(tokenBudget.trim())) && Number(tokenBudget.trim()) > 0 ? Math.floor(Number(tokenBudget.trim())) : null : null
  };
}
