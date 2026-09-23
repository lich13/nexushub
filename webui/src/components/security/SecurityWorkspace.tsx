import { CheckCircle2, KeyRound, Lock, ShieldCheck } from "lucide-react";
import { useState } from "react";
import { Metric, Panel } from "../common/Panel";
import {
  cleanHostValue,
  hostnameFromPublicEndpoint,
  normalizeTurnstileAction,
  secondsToDays
} from "../../lib/domain/runtimeViewModel";
import { defaultSessionTtlDays } from "../../lib/domain/codexViewModel";
import { useSecurityActions, useSecurityQuery } from "../../lib/query/security";
import { useSystemStatusQuery } from "../../lib/query/system";
import type { SecuritySettings } from "../../types";

export function SecurityWorkspace({ csrfToken, username }: { csrfToken?: string | null; username: string }) {
  const security = useSecurityQuery();
  const systemStatus = useSystemStatusQuery();
  const [draft, setDraft] = useState<Partial<SecuritySettings> & { turnstile_secret_key?: string }>({});
  const [passwordForm, setPasswordForm] = useState({ current: "", next: "", confirm: "" });
  const [passwordFeedback, setPasswordFeedback] = useState<string | null>(null);
  const securityActions = useSecurityActions({
    csrfToken,
    draft,
    passwordForm,
    onSaveSuccess: () => setDraft({}),
    onPasswordSuccess: () => {
      setPasswordFeedback("密码已更新");
      setPasswordForm({ current: "", next: "", confirm: "" });
    },
    onPasswordError: (err) => setPasswordFeedback(err.message)
  });
  const mutation = securityActions.save;
  const passwordMutation = securityActions.password;
  const merged = { ...security.data, ...draft } as SecuritySettings & { turnstile_secret_key?: string };
  const ttlDays = secondsToDays(merged.session_ttl_seconds ?? defaultSessionTtlDays * 86400);
  const defaultExpectedHostname = hostnameFromPublicEndpoint(systemStatus.data?.public_endpoint);
  const expectedHostname = cleanHostValue(merged.turnstile_expected_hostname) ?? defaultExpectedHostname;
  const expectedAction = normalizeTurnstileAction(merged.turnstile_expected_action);
  const passwordReady = passwordForm.current && passwordForm.next.length >= 12 && passwordForm.next === passwordForm.confirm;
  return (
    <div className="security-layout">
      <Panel title="Turnstile" icon={<ShieldCheck size={18} />}>
        <div className="settings-meta-grid">
          <Metric label="Secret" value={security.data?.turnstile_secret_configured ? "configured" : "not configured"} tone={security.data?.turnstile_secret_configured ? "success" : "warning"} />
          <Metric label="Mode" value={merged.turnstile_required ? "fail-closed" : "enabled"} />
          <Metric label="Expected hostname" value={expectedHostname ?? "未配置"} />
          <Metric label="Expected action" value={expectedAction} />
        </div>
        <label className="toggle-row">
          <span>启用 Turnstile</span>
          <input type="checkbox" checked={Boolean(merged.turnstile_enabled)} onChange={(event) => setDraft({ ...draft, turnstile_enabled: event.target.checked })} />
        </label>
        <label className="toggle-row">
          <span>未启用时拒绝登录</span>
          <input type="checkbox" checked={Boolean(merged.turnstile_required)} onChange={(event) => setDraft({ ...draft, turnstile_required: event.target.checked })} />
        </label>
        <label className="field-label">Site Key<input value={merged.turnstile_site_key ?? ""} onChange={(event) => setDraft({ ...draft, turnstile_site_key: event.target.value })} /></label>
        <label className="field-label">Expected hostname<input value={merged.turnstile_expected_hostname ?? ""} placeholder={defaultExpectedHostname ?? "未配置"} onChange={(event) => setDraft({ ...draft, turnstile_expected_hostname: event.target.value })} /></label>
        <label className="field-label">Expected action<input value={expectedAction} onChange={(event) => setDraft({ ...draft, turnstile_expected_action: event.target.value })} /></label>
        <label className="field-label">Secret Key<input type="password" placeholder={security.data?.turnstile_secret_configured ? "已配置，留空保留" : "未配置"} onChange={(event) => setDraft({ ...draft, turnstile_secret_key: event.target.value })} /></label>
        <button className="primary-button" onClick={() => mutation.mutate()}><ShieldCheck size={17} />保存 Turnstile</button>
      </Panel>
      <Panel title="登录设置" icon={<KeyRound size={18} />}>
        <Metric label="管理员" value={username} />
        <Metric label="Session TTL" value={`${ttlDays} 天`} />
        <label className="field-label">Session TTL days<input type="number" min={1} value={ttlDays} onChange={(event) => setDraft({ ...draft, session_ttl_seconds: Math.max(1, Number(event.target.value) || defaultSessionTtlDays) * 86400 })} /></label>
        <button className="secondary-button" onClick={() => mutation.mutate()}><CheckCircle2 size={17} />保存会话设置</button>
      </Panel>
      <Panel title="修改密码" icon={<Lock size={18} />} className="wide-panel">
        <div className="form-grid three">
          <label className="field-label">当前密码<input type="password" value={passwordForm.current} onChange={(event) => setPasswordForm({ ...passwordForm, current: event.target.value })} /></label>
          <label className="field-label">新密码<input type="password" value={passwordForm.next} onChange={(event) => setPasswordForm({ ...passwordForm, next: event.target.value })} /></label>
          <label className="field-label">确认新密码<input type="password" value={passwordForm.confirm} onChange={(event) => setPasswordForm({ ...passwordForm, confirm: event.target.value })} /></label>
        </div>
        {passwordFeedback && <div className={passwordFeedback.includes("已更新") ? "form-success" : "form-error"}>{passwordFeedback}</div>}
        <button className="primary-button" disabled={!passwordReady || passwordMutation.isPending} onClick={() => passwordMutation.mutate()}><KeyRound size={17} />修改密码</button>
      </Panel>
    </div>
  );
}


