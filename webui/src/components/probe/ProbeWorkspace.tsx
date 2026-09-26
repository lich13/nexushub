import {
  CheckCircle2,
  Cloud,
  GitFork,
  MessageSquare,
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
import { isTerminalJob, useStartedJob } from "../../lib/query/jobs";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";
import {
  codexHomeStatusValue,
  isProbeSettings,
  logBytesDraftToMb,
  mbDraftToLogBytes,
  probeDiscoveryWarningsText,
  probeJobActionLabel,
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
  ProbeSettings,
  ProbeStatus,
  ThreadSummary
} from "../../types";

type ProbeSaveStatus = { tone: "success" | "error"; message: string } | null;

export function ProbeWorkspace({ csrfToken, capabilities }: { csrfToken?: string | null; capabilities: RuntimeCapabilityMatrix }) {
  const [activeSection, setActiveSection] = useState<ProbeSectionId>("events");
  const [historyOpen, setHistoryOpen] = useState(false);
  const { status, settings, events, jobs } = useProbeQueries({ section: activeSection, historyOpen });
  const [draft, setDraft] = useState<ProbeSettingsDraft | null>(null);
  const submittedDraft = useRef<ProbeSettingsDraft | null>(null);
  const saveInFlight = useRef(false);
  const [saveStatus, setSaveStatus] = useState<ProbeSaveStatus>(null);
  const [actionStatus, setActionStatus] = useState<ProbeSaveStatus>(null);
  const data = status.data?.data;
  const available = status.data?.available ?? false;
  const currentSettings = settings.data?.data;
  const settingsErrors = draft ? probeSettingsValidation(draft) : [];
  const recentEvents = events.data?.data?.events ?? [];
  const probeView = probeWorkspaceView({
    data,
    available,
    currentSettings,
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
      submittedDraft.current = draft;
      return buildProbeSettingsPayload(draft, currentSettings, submittedDeviceKey);
    },
    onJobSuccess: (action) => {
      setActionStatus({ tone: "success", message: `${probeJobActionLabel(action)} 已加入 Job History` });
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
      // A completed request must not replace edits made before its callback renders.
      const savedDraft = submittedDraft.current;
      setDraft((current) => current === savedDraft ? buildProbeSettingsDraft(nextSettings) : current);
    },
    onSaveError: (err) => {
      setSaveStatus({ tone: "error", message: err.message });
    }
  });
  const probeJobMutation = probeActions.job;
  const startedJob = useStartedJob(probeJobMutation.data?.job_id);
  const jobRunning = Boolean(probeJobMutation.data?.job_id && !isTerminalJob(startedJob.data?.status));
  const jobBusy = probeJobMutation.isPending || jobRunning;
  const saveMutation = probeActions.save;
  const saveSettings = (deviceKey?: string) => {
    if (saveInFlight.current) return;
    saveInFlight.current = true;
    setSaveStatus(null);
    saveMutation.mutate(deviceKey, { onSettled: () => { saveInFlight.current = false; } });
  };
  const pendingProbeAction = jobBusy ? probeJobMutation.variables : null;

  useEffect(() => {
    if (!currentSettings || draft) return;
    setDraft(buildProbeSettingsDraft(currentSettings));
  }, [currentSettings, draft]);

  const activeSectionContent = (() => {
    switch (activeSection) {
      case "events":
        return (
          <Panel title="最近事件" icon={<TerminalSquare size={18} />} className="wide-panel">
            {events.error && <div role="alert" className="form-error">{events.error.message}</div>}
            <ProbeEventsCard events={recentEvents} available={events.data?.available ?? false} loading={events.isLoading} />
          </Panel>
        );
      case "settings":
        return (
          <>
            <Panel title="设置" icon={<SlidersHorizontal size={18} />} className="wide-panel">
              {settings.error && <div role="alert" className="form-error">{settings.error.message}</div>}
              {actionStatus && <div className={actionStatus.tone === "success" ? "form-success" : "form-error"}>{actionStatus.message}</div>}
              {startedJob.error && <div role="alert" className="form-error">{startedJob.error.message}</div>}
              {startedJob.data && <JobList jobs={[startedJob.data]} capabilities={capabilities} />}
              {draft ? (
                <ProbeRuntimeSettingsCard
                  draft={draft}
                  setDraft={setDraft}
                  errors={settingsErrors}
                  saveStatus={saveStatus}
                  saving={saveMutation.isPending}
                  status={data}
                  settings={currentSettings}
                  capabilities={capabilities}
                  onSave={() => saveSettings()}
                />
              ) : (
                <div className="muted-row">{settings.isLoading ? "正在读取设置" : "设置不可用"}</div>
              )}
            </Panel>
            <Panel title="Codex Hook" icon={<GitFork size={18} />}>
              <ProbeHookCard status={data} draft={draft} busy={jobBusy} onInstall={() => probeJobMutation.mutate("hooks-install")} />
            </Panel>
            <Panel title="Bark" icon={<Cloud size={18} />}>
              {draft && <ProbeBarkCard draft={draft} setDraft={setDraft} configuredDeviceKey={probeView.barkConfigured} saveStatus={saveStatus} saving={saveMutation.isPending} testing={pendingProbeAction === "bark-test"} onSave={saveSettings} onTest={() => probeJobMutation.mutate("bark-test")} />}
            </Panel>
            <details className="execution-history" open={historyOpen} onToggle={(event) => setHistoryOpen(event.currentTarget.open)}><summary>执行记录</summary><JobList jobs={probeView.probeJobs} capabilities={capabilities} /></details>
          </>
        );
      default:
        return null;
    }
  })();

  return (
    <div className="probe-layout">
      <div className="probe-header">
        <div>
          <h1>{PROBE_NAV_LABEL}</h1>
        </div>
        <div className="button-row">
          <button className="icon-button" title="刷新 Probe" onClick={probeActions.refresh}><RefreshCw size={17} /></button>
        </div>
      </div>

      <section className={`probe-status-banner tone-${probeView.statusTone}`}>
        <div>
          <strong>{probeView.availability.headline}</strong>
          <span>{probeView.serviceText} · {data?.host_label ?? currentSettings?.codex?.host_label ?? "未知主机"}</span>
        </div>
        <span>{probeStateLabel(data?.hook_status)}</span>
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
    <fieldset className="probe-card-stack" disabled={saving}>
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
    </fieldset>
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
  capabilities: RuntimeCapabilityMatrix;
  onSave: () => void;
}) {
  const setCodex = (patch: Partial<ProbeSettingsDraft["codex"]>) => setDraft({ ...draft, codex: { ...draft.codex, ...patch } });
  const setProbe = (patch: Partial<ProbeSettingsDraft["probe"]>) => setDraft({ ...draft, probe: { ...draft.probe, ...patch } });
  const setHooks = (patch: Partial<ProbeSettingsDraft["hooks"]>) => setDraft({ ...draft, hooks: { ...draft.hooks, ...patch } });
  const setNotifications = (patch: Partial<ProbeSettingsDraft["notifications"]>) => setDraft({ ...draft, notifications: { ...draft.notifications, ...patch } });
  const setObservability = (patch: Partial<ProbeSettingsDraft["observability"]>) => setDraft({ ...draft, observability: { ...draft.observability, ...patch } });
  return (
    <fieldset className="probe-card-stack" disabled={saving}>
      <div className="settings-meta-grid">
        <Metric label="通知" value={draft.notifications.enabled ? "已启用" : "已停用"} tone={draft.notifications.enabled ? "success" : "warning"} />
        <Metric label="Hook" value={probeStateLabel(status?.hook_status)} tone={status?.hook_status === "managed" ? "success" : "warning"} />
        <Metric label="错误监控" value={probeStateLabel(status?.error_monitor_status ?? (draft.probe.error_monitor.enabled ? "enabled" : "disabled"))} tone={draft.probe.error_monitor.enabled ? "success" : "warning"} />
        <Metric label="错误事件" value={String(status?.error_monitor_incident_count ?? 0)} />
        {capabilities.codexStatePaths && <Metric label="Codex Home" value={codexHomeStatusValue(status ?? settings?.codex)} wide />}
        <Metric label="Discovery" value={probeDiscoveryWarningsText(status?.discovery_warnings ?? settings?.codex?.discovery_warnings ?? settings?.discovery_warnings)} wide />
      </div>
      <div className="form-grid compact-three">
        {capabilities.codexStatePaths && <label className="field-label">Codex Home<input value={draft.codex.home} placeholder="auto" onChange={(event) => setCodex({ home: event.target.value })} /></label>}
        <label className="field-label">主机标签<input value={draft.codex.host_label} onChange={(event) => setCodex({ host_label: event.target.value })} /></label>
        <label className="field-label">通知事件保留天数<input type="number" min={1} max={3650} value={draft.observability.event_retention_days} onChange={(event) => setObservability({ event_retention_days: probeNumberInputDraftValue(event.target.value) })} /></label>
        <label className="field-label">轮询秒数<input type="number" min={5} max={3600} value={draft.probe.poll_seconds} onChange={(event) => setProbe({ poll_seconds: probeNumberInputDraftValue(event.target.value) })} /></label>
        <label className="field-label">最近事件数<input type="number" min={1} max={500} value={draft.probe.recent_limit} onChange={(event) => setProbe({ recent_limit: probeNumberInputDraftValue(event.target.value) })} /></label>
        <label className="field-label">通知事件保留天数<input type="number" min={1} max={3650} value={draft.observability.event_retention_days} onChange={(event) => setObservability({ event_retention_days: probeNumberInputDraftValue(event.target.value) })} /></label>
        <label className="field-label">Hook 事件行数<input type="number" min={1} max={5000} value={draft.observability.hook_event_max_lines} onChange={(event) => setObservability({ hook_event_max_lines: probeNumberInputDraftValue(event.target.value) })} /></label>
        <label className="field-label">冷却行数<input type="number" min={1} max={5000} value={draft.observability.hook_cooldown_max_lines} onChange={(event) => setObservability({ hook_cooldown_max_lines: probeNumberInputDraftValue(event.target.value) })} /></label>
        <label className="field-label">日志上限 MB<input type="number" min={1} max={10} value={logBytesDraftToMb(draft.observability.log_max_bytes)} onChange={(event) => setObservability({ log_max_bytes: mbDraftToLogBytes(event.target.value) })} /></label>
      </div>
      <div className="probe-toggle-grid">
        <label className="toggle-row"><span>启用 Probe</span><input type="checkbox" checked={draft.probe.enabled} onChange={(event) => setProbe({ enabled: event.target.checked })} /></label>
        <label className="toggle-row"><span>终止错误监控</span><input type="checkbox" checked={draft.probe.error_monitor.enabled} onChange={(event) => setProbe({ error_monitor: { ...draft.probe.error_monitor, enabled: event.target.checked } })} /></label>
        <label className="toggle-row"><span>启用 Bark</span><input type="checkbox" checked={draft.notifications.enabled} onChange={(event) => setNotifications({ enabled: event.target.checked })} /></label>
        <label className="toggle-row"><span>Codex 通知</span><input type="checkbox" checked={draft.notifications.notify_codex} onChange={(event) => setNotifications({ notify_codex: event.target.checked })} /></label>
        <label className="toggle-row"><span>Codex 完成</span><input type="checkbox" checked={draft.notifications.notify_completion} onChange={(event) => setNotifications({ notify_completion: event.target.checked })} /></label>
        <label className="toggle-row"><span>Grok 通知</span><input type="checkbox" checked={draft.notifications.notify_grok} onChange={(event) => setNotifications({ notify_grok: event.target.checked })} /></label>
        <label className="toggle-row"><span>Grok 完成</span><input type="checkbox" checked={draft.notifications.notify_grok_completion} onChange={(event) => setNotifications({ notify_grok_completion: event.target.checked })} /></label>
        <label className="toggle-row"><span>Grok 失败</span><input type="checkbox" checked={draft.notifications.notify_grok_failure} onChange={(event) => setNotifications({ notify_grok_failure: event.target.checked })} /></label>
        <label className="toggle-row"><span>Pi 通知</span><input type="checkbox" checked={draft.notifications.notify_pi} onChange={(event) => setNotifications({ notify_pi: event.target.checked })} /></label>
        <label className="toggle-row"><span>Pi 完成</span><input type="checkbox" checked={draft.notifications.notify_pi_completion} onChange={(event) => setNotifications({ notify_pi_completion: event.target.checked })} /></label>
        <label className="toggle-row" title="原生记录无法确认自动重试结束"><span>Pi 失败（暂不支持）</span><input type="checkbox" checked={false} disabled /></label>
        <label className="toggle-row"><span>Codex 回复通知</span><input type="checkbox" checked={draft.notifications.notify_reply_needed} onChange={(event) => setNotifications({ notify_reply_needed: event.target.checked })} /></label>
        <label className="toggle-row"><span>Codex 异常通知</span><input type="checkbox" checked={draft.notifications.notify_recoverable} onChange={(event) => setNotifications({ notify_recoverable: event.target.checked })} /></label>
        <label className="toggle-row"><span>管理 Codex Hook</span><input type="checkbox" checked={draft.hooks.manage_stop_hook} onChange={(event) => setHooks({ manage_stop_hook: event.target.checked })} /></label>
      </div>
      <p className="muted-row">Pi 未安装状态扩展：仅发送明确完成通知。原生记录无法证明自动重试已终止，最终失败通知暂不支持。</p>
      {status?.provider_notifications?.map((provider) => <div key={provider.provider} className="muted-row">{provider.provider.toUpperCase()} · {provider.enabled ? "监控中" : "已停用"} · {provider.streams} 个会话 · {provider.read_errors} 个读取异常 · {provider.failed_deliveries} 次投递失败</div>)}
      {errors.length > 0 && <div className="form-error">{errors[0]}</div>}
      {saveStatus && <div className={saveStatus.tone === "success" ? "form-success" : "form-error"}>{saveStatus.message}</div>}
      <button className="primary-button" disabled={saving || errors.length > 0} onClick={onSave}><CheckCircle2 size={17} />保存设置</button>
    </fieldset>
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
      <Metric label="Codex Hook" value={probeStateLabel(hookStatus)} tone={managed ? "success" : "warning"} />
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
