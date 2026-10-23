import { DshArchiveCleanup } from "./DshArchiveCleanup";
import { machineScope } from "../../lib/query/connection";
import { UpdatePanel } from "./UpdatePanel";
import {
  Archive,
  Database,
  TerminalSquare,
  Trash2
} from "lucide-react";
import { useState } from "react";
import { JobList } from "../jobs/JobList";
import { Metric, Panel } from "../common/Panel";
import { useOpsActions, useOpsQueries } from "../../lib/query/ops";
import { isTerminalJob, useStartedJob } from "../../lib/query/jobs";
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
  HiddenThreadDeleteResult
} from "../../types";

export function OpsWorkspace({ capabilities }: { capabilities: RuntimeCapabilityMatrix }) {
  const [historyOpen, setHistoryOpen] = useState(false);
  const { update, jobs } = useOpsQueries({ historyOpen });
  const [plan, setPlan] = useState<ArchiveDeletePlan | null>(null);
  const [hiddenPlan, setHiddenPlan] = useState<HiddenThreadDeletePlan | null>(null);
  const [hiddenDeleteResult, setHiddenDeleteResult] = useState<HiddenThreadDeleteResult | null>(null);
  const [deleteArmed, setDeleteArmed] = useState(false);
  const [hiddenDeleteArmed, setHiddenDeleteArmed] = useState(false);
  const [candidatePage, setCandidatePage] = useState(0);
  const [resultPage, setResultPage] = useState(0);
  const opsActions = useOpsActions({
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
      setHiddenPlan((current) => current ? { ...current, hidden_threads: result.hidden_threads, hidden_ids: [], candidates: [], hidden_source_counts: {} } : current);
    }
  });
  const jobMutation = opsActions.updateJob;
  const startedJob = useStartedJob(jobMutation.data?.job_id);
  const jobBusy = jobMutation.isPending || Boolean(jobMutation.data?.job_id && !isTerminalJob(startedJob.data?.status));
  const dryRun = opsActions.archiveDryRun;
  const executeDelete = opsActions.archiveExecute;
  const hiddenDryRun = opsActions.hiddenDryRun;
  const executeHiddenDelete = opsActions.hiddenExecute;
  const hiddenScan = hiddenPlan?.scan;
  const scanLabels = { preparing: "正在准备", scanning: "正在扫描", complete: "扫描完成", stopping: "正在停止", stopped: "已停止", failed: "扫描失败" };
  const visibleCandidates = hiddenPlan?.candidates.slice(candidatePage * 100, (candidatePage + 1) * 100) ?? [];
  const unsuccessful = hiddenDeleteResult?.items.filter(item => item.status !== "deleted") ?? [];
  const archiveCleanupError = executeDelete.error
    ? cleanupErrorMessage("归档清理失败", executeDelete.error)
    : dryRun.error
      ? cleanupErrorMessage("归档扫描失败", dryRun.error)
      : null;
  const hiddenCleanupError = executeHiddenDelete.error
    ? cleanupErrorMessage("隐藏线程清理失败", executeHiddenDelete.error)
    : hiddenDryRun.error
      ? cleanupErrorMessage("隐藏线程扫描失败", hiddenDryRun.error)
      : null;
  const opsView = opsWorkspaceView({
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
      <UpdatePanel
        capabilities={capabilities}
        status={update.data}
        loading={update.isPending}
        action={jobMutation.variables?.action}
        busy={jobBusy}
        finished={jobMutation.isSuccess && startedJob.data?.status === "succeeded"}
        job={startedJob.data}
        error={update.error?.message ?? jobMutation.error?.message ?? startedJob.error?.message}
        onAction={(action) => jobMutation.mutate({ action })}
      />
      {capabilities.threadCleanup && <Panel title={OPS_PANEL_TITLES.archivedCleanup} icon={<Archive size={18} />}>
        <div className="cleanup-panel-head">
          <span className={`status-chip ${opsView.archivedCleanupStage.tone ? `tone-${opsView.archivedCleanupStage.tone}` : "tone-muted"}`}>{opsView.archivedCleanupStage.label}</span>
        </div>
        <div className="archive-plan">
          <Metric label="active" value={plan ? String(plan.active_threads) : "dry-run 未执行"} />
          <Metric label="archived" value={String(plan?.archived_threads ?? 0)} tone={(plan?.archived_threads ?? 0) > 0 ? "warning" : undefined} />
          <Metric label="session index" value={plan ? String(plan.session_index_lines) : "dry-run 未执行"} />
          <Metric label="rollout 文件" value={plan ? String(plan.rollout_files) : "dry-run 未执行"} />
        </div>
        {plan && <ul className="hidden-cleanup-candidates">{plan.archived_ids.map(id => <li key={id}>{id}</li>)}</ul>}
        {executeDelete.data?.items?.filter(item => item.status !== "deleted").map(item => <div role="alert" key={item.id}>{item.id}：{item.reason}{item.residuals?.map(path => <code className="delete-path" key={path}>{path}</code>)}</div>)}
        {archiveCleanupError && <div className="form-error cleanup-error">{archiveCleanupError}</div>}
        <div className="button-row ops-action-row cleanup-actions">
          <button className="secondary-button" disabled={dryRun.isPending || executeDelete.isPending} onClick={() => { executeDelete.reset(); dryRun.mutate(); }}><Database size={17} />Dry-run</button>
          {!deleteArmed ? (
            <button className="danger-button soft" disabled={(plan?.archived_threads ?? 0) === 0 || dryRun.isPending || executeDelete.isPending} onClick={() => setDeleteArmed(true)}><Trash2 size={17} />清理归档</button>
          ) : (
            <>
              <button className="danger-button" onClick={() => executeDelete.mutate({ candidates: (plan?.archived_ids ?? []).map(id => ({ id })) })} disabled={executeDelete.isPending}><Trash2 size={17} />确认清理归档</button>
              <button className="secondary-button" onClick={() => setDeleteArmed(false)} disabled={executeDelete.isPending}>取消</button>
            </>
          )}
        </div>
      </Panel>}
      <DshArchiveCleanup key={machineScope()} supported={capabilities.dshArchiveCleanup === true} />
      {capabilities.threadCleanup && <Panel title={OPS_PANEL_TITLES.hiddenCleanup} icon={<Database size={18} />}>
        <div className="cleanup-panel-head">
          <span role="status" className={`status-chip ${opsView.hiddenCleanupStage.tone ? `tone-${opsView.hiddenCleanupStage.tone}` : "tone-muted"}`}>{hiddenScan ? `${scanLabels[hiddenScan.phase]} · ${hiddenScan.processed} / ${hiddenScan.total}` : opsView.hiddenCleanupStage.label}</span>
        </div>
        <div className="archive-plan">
          <Metric label="visible" value={hiddenPlan ? String(opsView.hiddenStats.visible) : "dry-run 未执行"} />
          <Metric label="hidden" value={String(opsView.hiddenStats.hidden)} tone={opsView.hiddenStats.hidden > 0 ? "warning" : undefined} />
          <Metric label="sources" value={opsView.hiddenStats.sourceCounts} />
          <Metric label="rollout 删除结果" value={hiddenRolloutDeleteResultText(hiddenDeleteResult)} tone={hiddenDeleteResult ? "success" : undefined} />
        </div>
        {hiddenPlan && <ul className="hidden-cleanup-candidates">
          {visibleCandidates.map(item => <li key={item.id}><strong>{item.title || item.id}</strong><span>{item.id}</span></li>)}
        </ul>}
        {hiddenPlan && hiddenPlan.candidates.length > 100 && <CleanupPagination page={candidatePage} count={hiddenPlan.candidates.length} onChange={setCandidatePage} label="候选" />}
        {hiddenPlan && !hiddenDryRun.isPending && hiddenPlan.candidates.length === 0 && <p role="status">无可清理项目</p>}
        {hiddenDeleteResult && <div role="status">
          <p>已删除 {hiddenDeleteResult.deleted_threads}，已跳过 {hiddenDeleteResult.skipped_threads}，失败 {hiddenDeleteResult.failed_threads}，剩余 {hiddenDeleteResult.hidden_threads}</p>
          {unsuccessful.slice(resultPage * 100, (resultPage + 1) * 100).map(item => <p key={item.id}>{item.id}：{item.reason}{item.residuals?.map(path => <code className="delete-path" key={path}>{path}</code>)}</p>)}
          {unsuccessful.length > 100 && <CleanupPagination page={resultPage} count={unsuccessful.length} onChange={setResultPage} label="清理结果" />}
        </div>}
        {hiddenCleanupError && <div className="form-error cleanup-error">{hiddenCleanupError}</div>}
        <div className="button-row ops-action-row cleanup-actions">
          <button className="secondary-button" disabled={hiddenDryRun.isPending || executeHiddenDelete.isPending} onClick={() => { executeHiddenDelete.reset(); setHiddenPlan(null); setHiddenDeleteResult(null); setHiddenDeleteArmed(false); setCandidatePage(0); setResultPage(0); hiddenDryRun.mutate(); }}><Database size={17} />扫描隐藏线程</button>
          {hiddenDryRun.isPending && capabilities.hiddenCleanupScan && <button className="secondary-button" disabled={hiddenScan?.phase === "stopping"} onClick={() => { setHiddenPlan(current => current?.scan ? { ...current, scan: { ...current.scan, phase: "stopping" } } : current); void opsActions.stopHiddenScan(); }}>停止扫描</button>}
          {!hiddenDeleteArmed ? (
            <button className="danger-button soft" disabled={!canStartHiddenThreadDelete(hiddenPlan) || hiddenDryRun.isPending || executeHiddenDelete.isPending} onClick={() => setHiddenDeleteArmed(true)}><Trash2 size={17} />清理隐藏线程</button>
          ) : (
            <>
              <button className="danger-button" onClick={() => executeHiddenDelete.mutate({ expectedCount: hiddenPlan?.candidates.length ?? 0, candidates: (hiddenPlan?.candidates ?? []).map(({ id, fingerprint }) => ({ id, fingerprint })) })} disabled={executeHiddenDelete.isPending}><Trash2 size={17} />确认清理隐藏</button>
              <button className="secondary-button" onClick={() => setHiddenDeleteArmed(false)} disabled={executeHiddenDelete.isPending}>取消</button>
            </>
          )}
        </div>
      </Panel>}
      <details className="execution-history" open={historyOpen} onToggle={(event) => setHistoryOpen(event.currentTarget.open)}><summary>执行记录</summary><Panel title={OPS_PANEL_TITLES.jobs} icon={<TerminalSquare size={18} />} className="wide-panel">
        {jobs.error && <div role="alert" className="form-error">{jobs.error.message}</div>}
        <JobList jobs={jobs.data ?? []} capabilities={capabilities} />
      </Panel></details>
    </div>
  );
}

function cleanupErrorMessage(prefix: string, error: unknown): string {
  return `${prefix}: ${error instanceof Error ? error.message : String(error)}`;
}

export function CleanupPagination({ page, count, onChange, label }: { page: number; count: number; onChange: (page: number) => void; label: string }) {
  const pages = Math.ceil(count / 100);
  return <nav className="button-row" aria-label={`${label}分页`}>
    <button className="secondary-button" disabled={page === 0} onClick={() => onChange(page - 1)}>上一页</button>
    <span>{page + 1} / {pages}</span>
    <button className="secondary-button" disabled={page + 1 >= pages} onClick={() => onChange(page + 1)}>下一页</button>
  </nav>;
}
