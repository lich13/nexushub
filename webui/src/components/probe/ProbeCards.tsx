import { CheckCircle2, Cloud, TerminalSquare } from "lucide-react";
import { useRef } from "react";
import { Metric } from "../common/Panel";
import { probeEventCard, probeNumberInputDraftValue, type ProbeSettingsDraft } from "../../lib/probeUi";
import { codexHomeStatusValue, logBytesDraftToMb, mbDraftToLogBytes, probeDiscoveryWarningsText, probeRunningCountValue, probeStateLabel } from "../../lib/domain/runtimeViewModel";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";
import type { ProbeEvent, ProbeSettings, ProbeStatus } from "../../types";

export type ProbeSaveStatus = { tone: "success" | "error"; message: string } | null;

export function ProbeBarkCard({
  draft,
  setDraft,
  configuredDeviceKey,
  saveStatus,
  saving,
  testing,
  testStatus,
  onSave,
  onTest
}: {
  draft: ProbeSettingsDraft;
  setDraft: (draft: ProbeSettingsDraft) => void;
  configuredDeviceKey: boolean;
  saveStatus: ProbeSaveStatus;
  saving: boolean;
  testing: boolean;
  testStatus: string | null;
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
      {testStatus && <div role="status" className="muted-row">{testStatus}</div>}
      {saveStatus && <div className={saveStatus.tone === "success" ? "form-success" : "form-error"}>{saveStatus.message}</div>}
    </fieldset>
  );
}

export function ProbeRuntimeSettingsCard({
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
        <label className="toggle-row"><span>Claude Code 通知</span><input type="checkbox" checked={draft.notifications.notify_claude} onChange={(event) => setNotifications({ notify_claude: event.target.checked })} /></label>
        <label className="toggle-row"><span>Claude Code 完成</span><input type="checkbox" checked={draft.notifications.notify_claude_completion} onChange={(event) => setNotifications({ notify_claude_completion: event.target.checked })} /></label>
        <label className="toggle-row"><span>Claude Code 失败</span><input type="checkbox" checked={draft.notifications.notify_claude_failure} onChange={(event) => setNotifications({ notify_claude_failure: event.target.checked })} /></label>
        <label className="toggle-row"><span>Claude Code 回复通知</span><input type="checkbox" checked={draft.notifications.notify_claude_reply_needed} onChange={(event) => setNotifications({ notify_claude_reply_needed: event.target.checked })} /></label>
        <label className="toggle-row"><span title="包含 Plan、进行中的同步或异步提问，以及最终回复中明确等待确认或反馈的请求">Codex 回复通知</span><input type="checkbox" checked={draft.notifications.notify_reply_needed} onChange={(event) => setNotifications({ notify_reply_needed: event.target.checked })} /></label>
        <label className="toggle-row"><span>Codex 异常通知</span><input type="checkbox" checked={draft.notifications.notify_recoverable} onChange={(event) => setNotifications({ notify_recoverable: event.target.checked })} /></label>
        <label className="toggle-row"><span>管理 Codex Hook</span><input type="checkbox" checked={draft.hooks.manage_stop_hook} onChange={(event) => setHooks({ manage_stop_hook: event.target.checked })} /></label>
      </div>
      {status?.provider_notifications?.map((provider) => <div key={provider.provider} className="muted-row">{provider.provider.toUpperCase()} · {provider.enabled ? "监控中" : "已停用"} · {provider.streams} 个会话 · {provider.read_errors} 个读取异常 · {provider.failed_deliveries} 次投递失败</div>)}
      {errors.length > 0 && <div className="form-error">{errors[0]}</div>}
      {saveStatus && <div className={saveStatus.tone === "success" ? "form-success" : "form-error"}>{saveStatus.message}</div>}
      <button className="primary-button" disabled={saving || errors.length > 0} onClick={onSave}><CheckCircle2 size={17} />保存设置</button>
    </fieldset>
  );
}


export function ProbeHookCard({
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

export function ProbeEventsCard({
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
