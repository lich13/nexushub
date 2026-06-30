import {
  CheckCircle2,
  Cloud,
  Database,
  GitFork,
  MessageSquare,
  Play,
  RefreshCw,
  SlidersHorizontal,
  TerminalSquare,
  TriangleAlert
} from "lucide-react";
import { ReactNode, useEffect, useRef, useState } from "react";
import { Metric, Panel } from "../common/Panel";
import { JobList } from "../jobs/JobList";
import {
  buildProbeSettingsDraft,
  buildProbeSettingsPayload,
  probeEventCard,
  probeNumberInputDraftValue,
  probeSections,
  probeSettingsValidation,
  PROBE_NAV_LABEL,
  type ProbeSectionId,
  type ProbeSettingsDraft
} from "../../lib/probeUi";
import { useProbeActions, useProbeQueries } from "../../lib/query/probe";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";
import {
  codexHomeStatusValue,
  isProbeSettings,
  logBytesDraftToMb,
  logsDbPathStatusValue,
  mbDraftToLogBytes,
  probeDiscoveryWarningsText,
  probeJobActionLabel,
  probeLogDbNumber,
  probeLogDbSize,
  probeLogDbString,
  probeLogsDbTone,
  probeRunningCountValue,
  probeSettingsAfterBarkSave,
  probeStateLabel,
  probeWorkspaceView
} from "../../lib/domain/runtimeViewModel";
import {
  threadListItemPreviewText,
  threadListItemStatusText,
  threadListItemText
} from "../../lib/domain/codexViewModel";
import type {
  ProbeEvent,
  ProbeLogsDbStatus,
  ProbeSettings,
  ProbeStatus,
  ThreadSummary
} from "../../types";

type ProbeSaveStatus = { tone: "success" | "error"; message: string } | null;

export function ProbeWorkspace({ csrfToken, capabilities }: { csrfToken?: string | null; capabilities: RuntimeCapabilityMatrix }) {
  const { status, settings, logsDbStatus, events, jobs } = useProbeQueries();
  const [draft, setDraft] = useState<ProbeSettingsDraft | null>(null);
  const [saveStatus, setSaveStatus] = useState<ProbeSaveStatus>(null);
  const [actionStatus, setActionStatus] = useState<ProbeSaveStatus>(null);
  const [logsDbExecuteArmed, setLogsDbExecuteArmed] = useState(false);
  const [activeSection, setActiveSection] = useState<ProbeSectionId>("overview");
  const data = status.data?.data;
  const available = status.data?.available ?? false;
  const currentSettings = settings.data?.data;
  const settingsErrors = draft ? probeSettingsValidation(draft) : [];
  const logsDb = logsDbStatus.data?.data;
  const recentEvents = events.data?.data?.events ?? [];
  const probeView = probeWorkspaceView({
    data,
    available,
    currentSettings,
    logsDb,
    recentEventCount: recentEvents.length,
    jobs: jobs.data,
    loading: status.isLoading,
    fetching: status.isFetching,
    error: status.isError,
    draftDeviceKeyConfigured: draft?.notifications.device_key_configured
  });
  const probeActions = useProbeActions({
    csrfToken,
    capabilities,
    savePayload: (submittedDeviceKey) => {
      if (!draft) throw new Error("探针设置尚未载入");
      const errors = probeSettingsValidation(draft);
      if (errors.length) throw new Error(errors[0]);
      return buildProbeSettingsPayload(draft, currentSettings, submittedDeviceKey);
    },
    onJobSuccess: (action) => {
      setActionStatus({ tone: "success", message: `${probeJobActionLabel(action)} 已加入 Job History` });
      if (action === "logs-db-dry-run") setLogsDbExecuteArmed(true);
      if (action === "logs-db-execute") setLogsDbExecuteArmed(false);
    },
    onJobError: (err, action) => {
      setActionStatus({ tone: "error", message: `${probeJobActionLabel(action)} 失败: ${err.message}` });
    },
    onSaveSuccess: (saved, submittedDeviceKey) => {
      const nextSettings = probeSettingsAfterBarkSave(saved, submittedDeviceKey ?? draft?.notifications.device_key);
      if (!isProbeSettings(nextSettings)) {
        setSaveStatus({ tone: "error", message: "保存响应结构异常，已保留当前输入" });
        return;
      }
      setSaveStatus({ tone: "success", message: "设置已保存" });
      setDraft(buildProbeSettingsDraft(nextSettings));
    },
    onSaveError: (err) => {
      setSaveStatus({ tone: "error", message: err.message });
    }
  });
  const probeJobMutation = probeActions.job;
  const saveMutation = probeActions.save;
  const pendingProbeAction = probeJobMutation.isPending ? probeJobMutation.variables : null;

  useEffect(() => {
    if (!currentSettings || draft) return;
    setDraft(buildProbeSettingsDraft(currentSettings));
  }, [currentSettings, draft]);

  const overviewSection = (
    <>
      <section className="probe-core-metrics" aria-label="探针核心指标">
        <Metric label="Codex APP" value={probeView.availability.metric} tone={probeView.statusTone} />
        <Metric label="运行中" value={probeRunningCountValue(data)} tone={Number(probeRunningCountValue(data)) > 0 ? "success" : undefined} />
        <Metric label="需回复" value={String(data?.reply_needed_count ?? 0)} tone={(data?.reply_needed_count ?? 0) > 0 ? "warning" : undefined} />
        <Metric label="异常数" value={String(data?.recoverable_count ?? 0)} tone={(data?.recoverable_count ?? 0) > 0 ? "danger" : undefined} />
        <Metric label="Bark" value={probeView.barkConfigured ? "已配置" : "未配置"} tone={probeView.barkConfigured ? "success" : "warning"} />
        <Metric label="Hook 事件" value={String(data?.recent_event_count ?? recentEvents.length)} tone={(data?.recent_event_count ?? recentEvents.length) > 0 ? "success" : undefined} />
        <Metric label="日志库" value={probeStateLabel(probeView.logsDbStatusText)} tone={probeView.logsDbTone} />
        {capabilities.codexStatePaths && <Metric label="Codex Home" value={codexHomeStatusValue(data ?? currentSettings?.codex)} wide />}
        <Metric label="刷新" value={probeView.snapshotText} tone={probeView.snapshotTone} />
      </section>
      <section className="probe-control-grid" aria-label="探针线程状态">
        <ProbeThreadBucket title="需回复" icon={<MessageSquare size={18} />} threads={probeView.probeThreads.replyNeeded} emptyText="当前没有待回复线程" />
        <ProbeThreadBucket title="异常/可恢复" icon={<TriangleAlert size={18} />} threads={probeView.probeThreads.recoverable} emptyText="当前没有可恢复异常" />
        <ProbeThreadBucket title="运行中" icon={<Play size={18} />} threads={probeView.probeThreads.running} emptyText="当前没有运行线程" />
      </section>
    </>
  );
  const activeSectionContent = (() => {
    switch (activeSection) {
      case "reply-needed":
        return <ProbeThreadBucket title="需回复" icon={<MessageSquare size={18} />} threads={probeView.probeThreads.replyNeeded} emptyText="当前没有待回复线程" />;
      case "recoverable":
        return <ProbeThreadBucket title="异常/可恢复" icon={<TriangleAlert size={18} />} threads={probeView.probeThreads.recoverable} emptyText="当前没有可恢复异常" />;
      case "running":
        return <ProbeThreadBucket title="运行中" icon={<Play size={18} />} threads={probeView.probeThreads.running} emptyText="当前没有运行线程" />;
      case "hook":
        return (
          <Panel title="Hook" icon={<GitFork size={18} />}>
            <ProbeHookCard
              status={data}
              draft={draft}
              busy={probeJobMutation.isPending}
              onInstall={() => probeJobMutation.mutate("hooks-install")}
            />
          </Panel>
        );
      case "bark":
        return (
          <Panel title="Bark" icon={<Cloud size={18} />}>
            {draft ? (
              <ProbeBarkCard
                draft={draft}
                setDraft={setDraft}
                configuredDeviceKey={probeView.barkConfigured}
                saveStatus={saveStatus}
                saving={saveMutation.isPending}
                testing={pendingProbeAction === "bark-test"}
                onSave={(deviceKey) => saveMutation.mutate(deviceKey)}
                onTest={() => probeJobMutation.mutate("bark-test")}
              />
            ) : (
              <div className="muted-row">{settings.isLoading ? "正在读取 Bark 设置" : "Bark 设置不可用"}</div>
            )}
          </Panel>
        );
      case "logs-db":
        return (
          <Panel title="Codex 日志库维护" icon={<Database size={18} />}>
            <ProbeLogsDbCard
              logsDb={logsDb}
              busy={probeJobMutation.isPending || !capabilities.probeLogMaintenance}
              executeArmed={logsDbExecuteArmed}
              onDryRun={() => capabilities.probeLogMaintenance && probeJobMutation.mutate("logs-db-dry-run")}
              onArmExecute={() => capabilities.probeLogMaintenance && setLogsDbExecuteArmed(true)}
              onCancelExecute={() => setLogsDbExecuteArmed(false)}
              onExecute={() => capabilities.probeLogMaintenance && probeJobMutation.mutate("logs-db-execute")}
            />
          </Panel>
        );
      case "events":
        return (
          <Panel title="最近事件" icon={<TerminalSquare size={18} />} className="wide-panel">
            <ProbeEventsCard events={recentEvents} available={events.data?.available ?? false} loading={events.isLoading} />
          </Panel>
        );
      case "settings":
        return (
          <>
            <Panel title="设置" icon={<SlidersHorizontal size={18} />} className="wide-panel">
              {actionStatus && <div className={actionStatus.tone === "success" ? "form-success" : "form-error"}>{actionStatus.message}</div>}
              {draft ? (
                <ProbeRuntimeSettingsCard
                  draft={draft}
                  setDraft={setDraft}
                  errors={settingsErrors}
                  saveStatus={saveStatus}
                  saving={saveMutation.isPending}
                  status={data}
                  settings={currentSettings}
                  logsDb={logsDb}
                  configuredDeviceKey={probeView.barkConfigured}
                  capabilities={capabilities}
                  onSave={() => saveMutation.mutate(undefined)}
                />
              ) : (
                <div className="muted-row">{settings.isLoading ? "正在读取设置" : "设置不可用"}</div>
              )}
            </Panel>
            <Panel title="Probe Job History" icon={<TerminalSquare size={18} />} className="wide-panel">
              <JobList jobs={probeView.probeJobs} capabilities={capabilities} />
            </Panel>
          </>
        );
      case "overview":
      default:
        return overviewSection;
    }
  })();

  return (
    <div className="probe-layout">
      <div className="probe-header">
        <div>
          <span>{PROBE_NAV_LABEL}</span>
          <h1>探针</h1>
        </div>
        <div className="button-row">
          <button className="secondary-button" onClick={probeActions.refresh}><RefreshCw size={17} />刷新</button>
          <button className="secondary-button" onClick={() => probeJobMutation.mutate("bark-test")} disabled={!probeView.barkConfigured || probeJobMutation.isPending}><Cloud size={17} />测试 Bark</button>
        </div>
      </div>

      <section className={`probe-status-banner tone-${probeView.statusTone}`}>
        <div>
          <strong>{probeView.availability.headline}</strong>
          <span>{probeView.serviceText} · {data?.host_label ?? currentSettings?.codex?.host_label ?? "未知主机"}</span>
        </div>
        <span>{probeStateLabel(data?.hook_status)} · {probeStateLabel(probeView.logsDbStatusText)}</span>
      </section>

      <div className="segmented" aria-label="Probe sections">
        {probeSections.map((section) => (
          <button
            key={section.id}
            className={activeSection === section.id ? "active" : ""}
            onClick={() => setActiveSection(section.id)}
            type="button"
          >
            {section.label}
          </button>
        ))}
      </div>

      {activeSectionContent}

      {probeView.availability.tone === "danger" && (
        <Panel title="端点" icon={<TriangleAlert size={18} />} className="wide-panel">
          <div className="muted-row">探针端点不可用</div>
        </Panel>
      )}
    </div>
  );
}

function ProbeThreadBucket({
  title,
  icon,
  threads,
  emptyText
}: {
  title: string;
  icon: ReactNode;
  threads: ThreadSummary[];
  emptyText: string;
}) {
  return (
    <Panel title={title} icon={icon}>
      <div className="preview-list compact">
        {threads.map((thread) => (
          <article className="preview-item" key={`${thread.status}-${thread.id}`}>
            <div>
              <strong>{threadListItemText(thread)}</strong>
              <span>{threadListItemPreviewText(thread) || thread.id}</span>
            </div>
            <small>{threadListItemStatusText(thread)} · {thread.updated_at ?? thread.id}</small>
          </article>
        ))}
        {threads.length === 0 && <div className="muted-row">{emptyText}</div>}
      </div>
    </Panel>
  );
}

function ProbeBarkCard({
  draft,
  setDraft,
  configuredDeviceKey,
  saveStatus,
  saving,
  testing,
  onSave,
  onTest
}: {
  draft: ProbeSettingsDraft;
  setDraft: (draft: ProbeSettingsDraft) => void;
  configuredDeviceKey: boolean;
  saveStatus: ProbeSaveStatus;
  saving: boolean;
  testing: boolean;
  onSave: (deviceKey?: string) => void;
  onTest: () => void;
}) {
  const deviceKeyInputRef = useRef<HTMLInputElement>(null);
  const setNotifications = (patch: Partial<ProbeSettingsDraft["notifications"]>) => setDraft({ ...draft, notifications: { ...draft.notifications, ...patch } });
  const handleSave = () => onSave(deviceKeyInputRef.current?.value ?? draft.notifications.device_key);
  return (
    <div className="probe-card-stack">
      <Metric label="配置状态" value={configuredDeviceKey ? "已配置" : "未配置"} tone={configuredDeviceKey ? "success" : "warning"} />
      <label className="field-label">
        Device Key
        <input
          ref={deviceKeyInputRef}
          type="password"
          value={draft.notifications.device_key}
          placeholder={configuredDeviceKey ? "已配置，留空保持不变" : "粘贴 Bark Device Key"}
          onChange={(event) => setNotifications({ device_key: event.target.value })}
        />
      </label>
      <div className="button-row">
        <button className="primary-button" disabled={saving} onClick={handleSave}><CheckCircle2 size={17} />保存</button>
        <button className="secondary-button" disabled={!configuredDeviceKey || testing} onClick={onTest}><Cloud size={17} />测试推送</button>
      </div>
      {saveStatus && <div className={saveStatus.tone === "success" ? "form-success" : "form-error"}>{saveStatus.message}</div>}
    </div>
  );
}

function ProbeRuntimeSettingsCard({
  draft,
  setDraft,
  errors,
  saveStatus,
  saving,
  status,
  settings,
  logsDb,
  configuredDeviceKey,
  capabilities,
  onSave
}: {
  draft: ProbeSettingsDraft;
  setDraft: (draft: ProbeSettingsDraft) => void;
  errors: string[];
  saveStatus: ProbeSaveStatus;
  saving: boolean;
  status?: ProbeStatus;
  settings?: ProbeSettings;
  logsDb?: ProbeLogsDbStatus;
  configuredDeviceKey: boolean;
  capabilities: RuntimeCapabilityMatrix;
  onSave: () => void;
}) {
  const setCodex = (patch: Partial<ProbeSettingsDraft["codex"]>) => setDraft({ ...draft, codex: { ...draft.codex, ...patch } });
  const setProbe = (patch: Partial<ProbeSettingsDraft["probe"]>) => setDraft({ ...draft, probe: { ...draft.probe, ...patch } });
  const setHooks = (patch: Partial<ProbeSettingsDraft["hooks"]>) => setDraft({ ...draft, hooks: { ...draft.hooks, ...patch } });
  const setNotifications = (patch: Partial<ProbeSettingsDraft["notifications"]>) => setDraft({ ...draft, notifications: { ...draft.notifications, ...patch } });
  const setObservability = (patch: Partial<ProbeSettingsDraft["observability"]>) => setDraft({ ...draft, observability: { ...draft.observability, ...patch } });
  const setLogsDb = (patch: Partial<ProbeSettingsDraft["logs_db"]>) => setDraft({ ...draft, logs_db: { ...draft.logs_db, ...patch } });
  return (
    <div className="probe-card-stack">
      <div className="settings-meta-grid">
        <Metric label="通知" value={draft.notifications.enabled ? "已启用" : "已停用"} tone={draft.notifications.enabled ? "success" : "warning"} />
        <Metric label="Device Key" value={configuredDeviceKey ? "已配置" : "未配置"} tone={configuredDeviceKey ? "success" : "warning"} />
        <Metric label="Hook" value={probeStateLabel(status?.hook_status)} tone={status?.hook_status === "managed" ? "success" : "warning"} />
        <Metric label="Logs DB" value={probeStateLabel(logsDb?.logs_db_status ?? logsDb?.status)} tone={probeLogsDbTone(logsDb?.logs_db_status ?? logsDb?.status)} />
        {capabilities.codexStatePaths && <Metric label="Codex Home" value={codexHomeStatusValue(status ?? settings?.codex)} wide />}
        <Metric label="Logs DB Path" value={logsDbPathStatusValue(logsDb ?? settings?.logs_db)} wide />
        <Metric label="Discovery" value={probeDiscoveryWarningsText(status?.discovery_warnings ?? settings?.codex?.discovery_warnings ?? settings?.discovery_warnings ?? logsDb?.discovery_warnings)} wide />
      </div>
      <div className="form-grid compact-three">
        {capabilities.codexStatePaths && <label className="field-label">Codex Home<input value={draft.codex.home} placeholder="auto" onChange={(event) => setCodex({ home: event.target.value })} /></label>}
        <label className="field-label">主机标签<input value={draft.codex.host_label} onChange={(event) => setCodex({ host_label: event.target.value })} /></label>
        <label className="field-label">轮询秒数<input type="number" min={5} max={3600} value={draft.probe.poll_seconds} onChange={(event) => setProbe({ poll_seconds: probeNumberInputDraftValue(event.target.value) })} /></label>
        <label className="field-label">最近事件数<input type="number" min={1} max={500} value={draft.probe.recent_limit} onChange={(event) => setProbe({ recent_limit: probeNumberInputDraftValue(event.target.value) })} /></label>
        <label className="field-label">Hook 事件行数<input type="number" min={1} max={5000} value={draft.observability.hook_event_max_lines} onChange={(event) => setObservability({ hook_event_max_lines: probeNumberInputDraftValue(event.target.value) })} /></label>
        <label className="field-label">冷却行数<input type="number" min={1} max={5000} value={draft.observability.hook_cooldown_max_lines} onChange={(event) => setObservability({ hook_cooldown_max_lines: probeNumberInputDraftValue(event.target.value) })} /></label>
        <label className="field-label">日志上限 MB<input type="number" min={1} max={10} value={logBytesDraftToMb(draft.observability.log_max_bytes)} onChange={(event) => setObservability({ log_max_bytes: mbDraftToLogBytes(event.target.value) })} /></label>
        <label className="field-label">Logs 保留天数<input type="number" min={1} max={3650} value={draft.logs_db.retention_days} onChange={(event) => setLogsDb({ retention_days: probeNumberInputDraftValue(event.target.value) })} /></label>
        <label className="field-label">维护间隔小时<input type="number" min={1} max={8760} value={draft.logs_db.maintenance_interval_hours} onChange={(event) => setLogsDb({ maintenance_interval_hours: probeNumberInputDraftValue(event.target.value) })} /></label>
        <label className="field-label">最大删除行数<input type="number" min={1} max={1000000} value={draft.logs_db.max_delete_rows_per_run} onChange={(event) => setLogsDb({ max_delete_rows_per_run: probeNumberInputDraftValue(event.target.value) })} /></label>
      </div>
      <div className="probe-toggle-grid">
        <label className="toggle-row"><span>启用 Probe</span><input type="checkbox" checked={draft.probe.enabled} onChange={(event) => setProbe({ enabled: event.target.checked })} /></label>
        <label className="toggle-row"><span>启用 Bark</span><input type="checkbox" checked={draft.notifications.enabled} onChange={(event) => setNotifications({ enabled: event.target.checked })} /></label>
        <label className="toggle-row"><span>回复通知</span><input type="checkbox" checked={draft.notifications.notify_reply_needed} onChange={(event) => setNotifications({ notify_reply_needed: event.target.checked })} /></label>
        <label className="toggle-row"><span>异常通知</span><input type="checkbox" checked={draft.notifications.notify_recoverable} onChange={(event) => setNotifications({ notify_recoverable: event.target.checked })} /></label>
        <label className="toggle-row"><span>管理 Stop Hook</span><input type="checkbox" checked={draft.hooks.manage_stop_hook} onChange={(event) => setHooks({ manage_stop_hook: event.target.checked })} /></label>
        <label className="toggle-row"><span>启用 Logs DB</span><input type="checkbox" checked={draft.logs_db.enabled} onChange={(event) => setLogsDb({ enabled: event.target.checked })} /></label>
        <label className="toggle-row"><span>退出后维护</span><input type="checkbox" checked={draft.logs_db.maintain_on_codex_exit} onChange={(event) => setLogsDb({ maintain_on_codex_exit: event.target.checked })} /></label>
      </div>
      {errors.length > 0 && <div className="form-error">{errors[0]}</div>}
      {saveStatus && <div className={saveStatus.tone === "success" ? "form-success" : "form-error"}>{saveStatus.message}</div>}
      <button className="primary-button" disabled={saving || errors.length > 0} onClick={onSave}><CheckCircle2 size={17} />保存设置</button>
    </div>
  );
}

function ProbeLogsDbCard({
  logsDb,
  busy,
  onDryRun,
  executeArmed,
  onArmExecute,
  onCancelExecute,
  onExecute
}: {
  logsDb?: ProbeLogsDbStatus;
  busy?: boolean;
  onDryRun?: () => void;
  executeArmed?: boolean;
  onArmExecute?: () => void;
  onCancelExecute?: () => void;
  onExecute?: () => void;
}) {
  const status = logsDb?.logs_db_status ?? logsDb?.status;
  return (
    <div className="probe-card-stack">
      <Metric label="状态" value={probeStateLabel(status)} tone={probeLogsDbTone(status)} />
      <Metric label="数据库路径" value={logsDbPathStatusValue(logsDb)} wide />
      <Metric label="旧行数" value={probeLogDbNumber(logsDb, ["old_rows", "pending_cleanup_rows", "stale_rows", "would_delete_probe_events"])} />
      <Metric label="保留行数" value={probeLogDbNumber(logsDb, ["retained_rows", "retained_row_count", "total_rows", "row_count", "event_count"])} />
      <Metric label="DB 大小" value={probeLogDbSize(logsDb, ["database_size", "db_size_bytes", "database_size_bytes", "size_bytes"])} />
      <Metric label="WAL 大小" value={probeLogDbSize(logsDb, ["wal_size", "wal_size_bytes", "wal_bytes"])} />
      <Metric label="SHM 大小" value={probeLogDbSize(logsDb, ["shm_size", "shm_size_bytes", "shm_bytes"])} />
      <Metric label="上次维护" value={probeLogDbString(logsDb, ["last_run_at", "last_maintain_at", "last_maintenance_at", "last_maintain"])} />
      <Metric label="下次维护" value={probeLogDbString(logsDb, ["next_run_at", "next_maintain_at", "next_maintenance_at"])} />
      <Metric label="最近结果" value={probeLogDbString(logsDb, ["recent_result", "last_result", "last_maintain_result", "skip_reason"])} />
      {(onDryRun || onExecute) && (
        <div className="button-row">
          {onDryRun && <button className="secondary-button" disabled={busy} onClick={onDryRun}><Database size={17} />Dry-run</button>}
          {onExecute && !executeArmed && <button className="secondary-button" disabled={busy} onClick={onArmExecute}><Play size={17} />准备执行</button>}
          {onExecute && executeArmed && <button className="primary-button" disabled={busy} onClick={onExecute}><Play size={17} />确认执行</button>}
          {executeArmed && onCancelExecute && <button className="secondary-button" disabled={busy} onClick={onCancelExecute}>取消</button>}
        </div>
      )}
    </div>
  );
}

function ProbeHookCard({
  status,
  draft,
  busy,
  onInstall
}: {
  status?: ProbeStatus | null;
  draft?: ProbeSettingsDraft | null;
  busy?: boolean;
  onInstall: () => void;
}) {
  const hookStatus = status?.hook_status;
  const managed = hookStatus === "managed";
  const needsRepair = hookStatus === "stale" || hookStatus === "missing";
  const configured = managed || draft?.hooks.manage_stop_hook === true;
  const actualCommandCount = status?.actual_commands?.length ?? 0;
  const staleCommandCount = status?.stale_command_count ?? 0;
  const emptyGroupCount = status?.empty_group_count ?? 0;
  const actualSummary = `${actualCommandCount} 条${staleCommandCount > 0 ? ` · 旧 ${staleCommandCount}` : ""}${emptyGroupCount > 0 ? ` · 空 ${emptyGroupCount}` : ""}`;
  return (
    <div className="probe-card-stack">
      <Metric label="Stop Hook" value={probeStateLabel(hookStatus)} tone={managed ? "success" : "warning"} />
      <Metric label="管理开关" value={configured ? "已开启" : "已关闭"} tone={configured ? "success" : "warning"} />
      <Metric label="实际命令" value={actualSummary} tone={needsRepair ? "warning" : "success"} />
      <Metric label="动作" value={needsRepair ? "重新安装 Hook" : "固定 Hook 安装 job"} />
      <button className="secondary-button" disabled={busy} onClick={onInstall}><TerminalSquare size={17} />{needsRepair ? "重新安装 Hook" : "安装 Hook"}</button>
    </div>
  );
}

function ProbeEventsCard({
  events,
  available,
  loading
}: {
  events: ProbeEvent[];
  available: boolean;
  loading: boolean;
}) {
  if (!available) {
    return <div className="muted-row">{loading ? "正在读取事件" : "事件接口不可用"}</div>;
  }
  if (events.length === 0) {
    return <div className="muted-row">暂无最近 Hook 事件</div>;
  }
  return (
    <div className="preview-list compact">
      {events.map((event) => (
        <ProbeEventRow event={event} key={event.id} />
      ))}
    </div>
  );
}

function ProbeEventRow({ event }: { event: ProbeEvent }) {
  const card = probeEventCard(event);
  return (
    <article className="preview-item probe-event-card">
      <div>
        <strong>{card.title} · {card.headline}</strong>
        <span>{card.summary}</span>
      </div>
      {card.reason && <small>{card.reason}</small>}
      <div className="probe-event-detail-row">
        <span className={`status-chip tone-${card.bark.tone}`}>{card.bark.label}</span>
        <span className={`status-chip tone-${card.dedupe.tone}`}>{card.dedupe.label}</span>
        {card.details.map((detail) => (
          <span key={`${detail.label}:${detail.value}`}>{detail.label}: {detail.value}</span>
        ))}
        <span>{card.time}</span>
      </div>
    </article>
  );
}
