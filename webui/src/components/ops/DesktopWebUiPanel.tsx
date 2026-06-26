import { CheckCircle2, Globe2, KeyRound, Play, Square } from "lucide-react";
import { useEffect, useState } from "react";
import { Metric, Panel } from "../common/Panel";
import { useDesktopWebUiActions, useDesktopWebUiQueries } from "../../lib/query/desktopWebui";
import { OPS_PANEL_TITLES, secondsToDays } from "../../lib/domain/runtimeViewModel";
import { defaultSessionTtlDays } from "../../lib/domain/codexViewModel";
import type { DesktopWebUiSettingsPatch } from "../../types";

export function DesktopWebUiPanel({ enabled }: { enabled: boolean }) {
  const queries = useDesktopWebUiQueries(enabled);
  const actions = useDesktopWebUiActions();
  const settings = queries.settings.data;
  const status = queries.status.data;
  const [draft, setDraft] = useState<DesktopWebUiSettingsPatch | null>(null);
  const [password, setPassword] = useState("");
  const [feedback, setFeedback] = useState<string | null>(null);
  const effective = draft ?? settings;
  const ttlDays = secondsToDays(effective?.sessionTtlSeconds ?? defaultSessionTtlDays * 86400);
  const passwordReady = Boolean((effective?.username ?? "").trim()) && password.length >= 12;
  const busy = actions.saveSettings.isPending || actions.resetPassword.isPending || actions.start.isPending || actions.stop.isPending;
  const canStart = Boolean(effective?.enabled && settings?.passwordConfigured && !status?.running);

  useEffect(() => {
    if (!settings) return;
    setDraft({
      enabled: settings.enabled,
      listen: settings.listen,
      username: settings.username,
      sessionTtlSeconds: settings.sessionTtlSeconds,
      cookieSecure: settings.cookieSecure,
      publicBaseUrl: settings.publicBaseUrl
    });
  }, [settings]);

  if (!enabled) return null;

  const updateDraft = (patch: Partial<DesktopWebUiSettingsPatch>) => {
    setDraft((current) => ({
      enabled: current?.enabled ?? settings?.enabled ?? false,
      listen: current?.listen ?? settings?.listen ?? "0.0.0.0:15743",
      username: current?.username ?? settings?.username ?? "admin",
      sessionTtlSeconds: current?.sessionTtlSeconds ?? settings?.sessionTtlSeconds ?? 86400,
      cookieSecure: current?.cookieSecure ?? settings?.cookieSecure ?? false,
      publicBaseUrl: current?.publicBaseUrl ?? settings?.publicBaseUrl ?? null,
      ...patch
    }));
  };

  const saveSettings = () => {
    if (!effective) return;
    actions.saveSettings.mutate(effective, {
      onSuccess: () => setFeedback("WebUI 服务设置已保存"),
      onError: (error) => setFeedback(error instanceof Error ? error.message : String(error))
    });
  };

  const resetPassword = () => {
    if (!effective) return;
    actions.resetPassword.mutate({ username: effective.username, password }, {
      onSuccess: () => {
        setPassword("");
        setFeedback("WebUI 服务密码已更新");
      },
      onError: (error) => setFeedback(error instanceof Error ? error.message : String(error))
    });
  };

  return (
    <Panel title={OPS_PANEL_TITLES.desktopWebui} icon={<Globe2 size={18} />} className="wide-panel">
      <div className="settings-meta-grid">
        <Metric label="Status" value={status?.running ? "running" : "stopped"} tone={status?.running ? "success" : "warning"} />
        <Metric label="Enabled" value={effective?.enabled ? "enabled" : "disabled"} tone={effective?.enabled ? "success" : "warning"} />
        <Metric label="Password" value={settings?.passwordConfigured ? "configured" : "required"} tone={settings?.passwordConfigured ? "success" : "warning"} />
        <Metric label="Listen" value={effective?.listen ?? "读取中"} />
        <Metric label="URL" value={status?.url ?? "读取中"} wide />
        <Metric label="PID" value={status?.pid ? String(status.pid) : "none"} />
      </div>
      {status?.message && <div className="muted-row">{status.message}</div>}
      <div className="form-grid compact-three">
        <label className="field-label">Listen<input value={effective?.listen ?? ""} onChange={(event) => updateDraft({ listen: event.target.value })} /></label>
        <label className="field-label">Username<input value={effective?.username ?? ""} onChange={(event) => updateDraft({ username: event.target.value })} /></label>
        <label className="field-label">Session TTL days<input type="number" min={1} value={ttlDays} onChange={(event) => updateDraft({ sessionTtlSeconds: Math.max(1, Number(event.target.value) || defaultSessionTtlDays) * 86400 })} /></label>
        <label className="field-label">Public base URL<input value={effective?.publicBaseUrl ?? ""} onChange={(event) => updateDraft({ publicBaseUrl: event.target.value || null })} /></label>
        <label className="field-label">New password<input type="password" value={password} onChange={(event) => setPassword(event.target.value)} /></label>
      </div>
      <div className="probe-toggle-grid">
        <label className="toggle-row"><span>启用 WebUI 服务</span><input type="checkbox" checked={Boolean(effective?.enabled)} onChange={(event) => updateDraft({ enabled: event.target.checked })} /></label>
        <label className="toggle-row"><span>Secure cookie</span><input type="checkbox" checked={Boolean(effective?.cookieSecure)} onChange={(event) => updateDraft({ cookieSecure: event.target.checked })} /></label>
      </div>
      {feedback && <div className={feedback.includes("已") ? "form-success" : "form-error"}>{feedback}</div>}
      <div className="button-row ops-action-row">
        <button className="primary-button" disabled={!effective || busy} onClick={saveSettings}><CheckCircle2 size={17} />保存 WebUI 服务</button>
        <button className="secondary-button" disabled={!passwordReady || busy} onClick={resetPassword}><KeyRound size={17} />重置 WebUI 密码</button>
        <button className="secondary-button" disabled={!canStart || busy} onClick={() => actions.start.mutate()}><Play size={17} />启动 WebUI</button>
        <button className="secondary-button" disabled={!status?.running || busy} onClick={() => actions.stop.mutate()}><Square size={17} />停止 WebUI</button>
      </div>
    </Panel>
  );
}
