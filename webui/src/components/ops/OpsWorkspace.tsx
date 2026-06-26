import {
  Archive,
  CheckCircle2,
  Database,
  HardDrive,
  Play,
  RefreshCw,
  TerminalSquare,
  Trash2
} from "lucide-react";
import { useState } from "react";
import { JobList } from "../jobs/JobList";
import { Metric, Panel } from "../common/Panel";
import { useOpsActions, useOpsQueries } from "../../lib/query/ops";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";
import {
  OPS_PANEL_TITLES,
  archivePlanAfterExecute,
  canStartHiddenThreadDelete,
  hiddenRolloutDeleteResultText,
  opsWorkspaceView
} from "../../lib/domain/runtimeViewModel";
import type {
  ArchiveDeletePlan,
  HiddenThreadDeletePlan,
  HiddenThreadDeleteResult,
  UpdateStatus
} from "../../types";

export function OpsWorkspace({ csrfToken, capabilities }: { csrfToken?: string | null; capabilities: RuntimeCapabilityMatrix }) {
  const { status, update, jobs } = useOpsQueries();
  const [plan, setPlan] = useState<ArchiveDeletePlan | null>(null);
  const [hiddenPlan, setHiddenPlan] = useState<HiddenThreadDeletePlan | null>(null);
  const [hiddenDeleteResult, setHiddenDeleteResult] = useState<HiddenThreadDeleteResult | null>(null);
  const [deleteArmed, setDeleteArmed] = useState(false);
  const [hiddenDeleteArmed, setHiddenDeleteArmed] = useState(false);
  const opsActions = useOpsActions({
    csrfToken,
    capabilities,
    onArchiveDryRun: (nextPlan) => {
      setPlan(nextPlan);
      setDeleteArmed(false);
    },
    onArchiveExecute: (result) => {
      setDeleteArmed(false);
      setPlan((current) => archivePlanAfterExecute(current, result));
    },
    onHiddenDryRun: (nextPlan) => {
      setHiddenPlan(nextPlan);
      setHiddenDeleteResult(null);
      setHiddenDeleteArmed(false);
    },
    onHiddenExecute: (result) => {
      setHiddenDeleteArmed(false);
      setHiddenDeleteResult(result);
      setHiddenPlan((current) => current ? { ...current, hidden_threads: result.hidden_threads, hidden_ids: [], hidden_source_counts: {} } : current);
    }
  });
  const jobMutation = opsActions.updateJob;
  const dryRun = opsActions.archiveDryRun;
  const executeDelete = opsActions.archiveExecute;
  const hiddenDryRun = opsActions.hiddenDryRun;
  const executeHiddenDelete = opsActions.hiddenExecute;
  const opsView = opsWorkspaceView({
    status: status.data,
    update: update.data,
    hiddenPlan,
    archivePlan: plan,
    archiveDryRunPending: dryRun.isPending,
    archiveDeleteArmed: deleteArmed,
    archiveExecutePending: executeDelete.isPending,
    hiddenDryRunPending: hiddenDryRun.isPending,
    hiddenDeleteArmed,
    hiddenExecutePending: executeHiddenDelete.isPending,
    capabilities
  });

  return (
    <div className="ops-grid">
      <Panel title={OPS_PANEL_TITLES.system} icon={<HardDrive size={18} />} className="wide-panel ops-status-panel">
        <div className="ops-status-overview">
          {opsView.systemMetrics.map((metric) => (
            <Metric key={metric.label} label={metric.label} value={metric.value} tone={metric.tone} wide={metric.wide} />
          ))}
        </div>
      </Panel>
      <Panel title={OPS_PANEL_TITLES.updates} icon={<RefreshCw size={18} />}>
        <UpdateMetrics status={update.data} />
        <div className="button-row ops-action-row">
          {opsView.updateActions.map((action) => {
            const className = action.tone === "primary" ? "primary-button" : action.tone === "danger" ? "danger-button soft" : "secondary-button";
            const icon = action.action === "check" ? <CheckCircle2 size={17} /> : action.action === "install" ? <Play size={17} /> : <Trash2 size={17} />;
            return (
              <button key={action.action} className={className} disabled={jobMutation.isPending || action.disabled} onClick={() => jobMutation.mutate({ action: action.action })}>
                {icon}{action.label}
              </button>
            );
          })}
        </div>
      </Panel>
      {capabilities.threadCleanup && <Panel title={OPS_PANEL_TITLES.archivedCleanup} icon={<Archive size={18} />}>
        <div className="cleanup-panel-head">
          <span>删除 archived 线程与 rollout</span>
          <span className={`status-chip ${opsView.archivedCleanupStage.tone ? `tone-${opsView.archivedCleanupStage.tone}` : "tone-muted"}`}>{opsView.archivedCleanupStage.label}</span>
        </div>
        <div className="archive-plan">
          <Metric label="active" value={plan ? String(plan.active_threads) : "dry-run 未执行"} />
          <Metric label="archived" value={String(plan?.archived_threads ?? 0)} tone={(plan?.archived_threads ?? 0) > 0 ? "warning" : undefined} />
          <Metric label="integrity" value={plan?.integrity ?? status.data?.state_db_integrity ?? "unknown"} tone={(plan?.integrity ?? status.data?.state_db_integrity) === "ok" ? "success" : "danger"} />
          <Metric label="session index" value={plan ? String(plan.session_index_lines) : "dry-run 未执行"} />
          <Metric label="rollout 文件" value={plan ? String(plan.rollout_files) : "dry-run 未执行"} />
        </div>
        <div className="button-row ops-action-row cleanup-actions">
          <button className="secondary-button" disabled={dryRun.isPending || executeDelete.isPending} onClick={() => dryRun.mutate()}><Database size={17} />Dry-run</button>
          {!deleteArmed ? (
            <button className="danger-button soft" disabled={(plan?.archived_threads ?? 0) === 0 || dryRun.isPending || executeDelete.isPending} onClick={() => setDeleteArmed(true)}><Trash2 size={17} />清理归档</button>
          ) : (
            <>
              <button className="danger-button" onClick={() => executeDelete.mutate({ expectedCount: plan?.archived_threads ?? 0 })} disabled={executeDelete.isPending}><Trash2 size={17} />确认清理归档</button>
              <button className="secondary-button" onClick={() => setDeleteArmed(false)} disabled={executeDelete.isPending}>取消</button>
            </>
          )}
        </div>
      </Panel>}
      {capabilities.threadCleanup && <Panel title={OPS_PANEL_TITLES.hiddenCleanup} icon={<Database size={18} />}>
        <div className="cleanup-panel-head">
          <span>删除 non-archived subagent/internal</span>
          <span className={`status-chip ${opsView.hiddenCleanupStage.tone ? `tone-${opsView.hiddenCleanupStage.tone}` : "tone-muted"}`}>{opsView.hiddenCleanupStage.label}</span>
        </div>
        <div className="archive-plan">
          <Metric label="visible" value={hiddenPlan ? String(opsView.hiddenStats.visible) : "dry-run 未执行"} />
          <Metric label="hidden" value={String(opsView.hiddenStats.hidden)} tone={opsView.hiddenStats.hidden > 0 ? "warning" : undefined} />
          <Metric label="sources" value={opsView.hiddenStats.sourceCounts} />
          <Metric label="integrity" value={opsView.hiddenStats.integrity} tone={opsView.hiddenStats.integrity === "ok" ? "success" : "danger"} />
          <Metric label="rollout 删除结果" value={hiddenRolloutDeleteResultText(hiddenDeleteResult)} tone={hiddenDeleteResult ? "success" : undefined} />
        </div>
        <div className="button-row ops-action-row cleanup-actions">
          <button className="secondary-button" disabled={hiddenDryRun.isPending || executeHiddenDelete.isPending} onClick={() => hiddenDryRun.mutate()}><Database size={17} />扫描隐藏线程</button>
          {!hiddenDeleteArmed ? (
            <button className="danger-button soft" disabled={!canStartHiddenThreadDelete(hiddenPlan) || hiddenDryRun.isPending || executeHiddenDelete.isPending} onClick={() => setHiddenDeleteArmed(true)}><Trash2 size={17} />清理隐藏线程</button>
          ) : (
            <>
              <button className="danger-button" onClick={() => executeHiddenDelete.mutate({ expectedCount: opsView.hiddenStats.hidden })} disabled={executeHiddenDelete.isPending}><Trash2 size={17} />确认清理隐藏</button>
              <button className="secondary-button" onClick={() => setHiddenDeleteArmed(false)} disabled={executeHiddenDelete.isPending}>取消</button>
            </>
          )}
        </div>
      </Panel>}
      <Panel title={OPS_PANEL_TITLES.jobs} icon={<TerminalSquare size={18} />} className="wide-panel">
        <JobList jobs={jobs.data ?? []} capabilities={capabilities} />
      </Panel>
    </div>
  );
}

function UpdateMetrics({ status }: { status?: UpdateStatus }) {
  return (
    <div className="version-grid">
      <Metric label="Current" value={status?.current_version ?? "读取中"} />
      <Metric
        label="Latest"
        value={status?.latest_version ?? "unknown"}
        tone={status?.update_available ? "warning" : "success"}
      />
      <Metric label="Update" value={status?.update_available ? "available" : status?.state ?? "current"} tone={status?.update_available ? "warning" : "success"} />
    </div>
  );
}
