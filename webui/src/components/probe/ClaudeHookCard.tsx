import { useEffect, useRef, useState } from "react";
import { useClaudeHooks, type HookProviderId } from "../../lib/query/probe";
import { AgentBrandIcon, type AgentBrandId } from "../common/AgentBrandIcon";
import { ConfirmDialog } from "../common/ConfirmDialog";
import type { ClaudeHooksAction } from "../../types";
import { TerminalSquare, Trash2 } from "lucide-react";
import { Metric } from "../common/Panel";

const HOOK_PROVIDER_META: Record<
  HookProviderId,
  {
    brand: AgentBrandId;
    title: string;
    metricLabel: string;
    hostPath: string;
    subtitle: string;
  }
> = {
  claude: {
    brand: "claude",
    title: "Claude Code",
    metricLabel: "Claude Code Hooks",
    hostPath: "~/.claude/settings.json",
    subtitle: "Stop · Notification · StopFailure 实时推送",
  },
  grok: {
    brand: "grok",
    title: "Grok Build",
    metricLabel: "Grok Build Hooks",
    hostPath: "~/.grok/hooks/nexushub.json",
    subtitle: "毫秒级完成与待回复回调 + 原生会话轮询兜底",
  },
  antigravity: {
    brand: "antigravity",
    title: "Antigravity",
    metricLabel: "Antigravity Hooks",
    hostPath: "~/.gemini/config/hooks.json",
    subtitle: "事件实时唤醒 + 本机 Language Server 直连兜底",
  },
};

export function ClaudeHookCard({ provider = "claude" }: { provider?: HookProviderId }) {
  const meta = HOOK_PROVIDER_META[provider];
  const [confirm, setConfirm] = useState<ClaudeHooksAction | null>(null);
  const installButton = useRef<HTMLButtonElement>(null);
  const removeButton = useRef<HTMLButtonElement>(null);
  const { local, revision, status, mutation } = useClaudeHooks(() => setConfirm(null), provider);
  useEffect(() => setConfirm(null), [revision]);
  const dialogId = `${provider}-hooks-confirm`;

  if (!local) {
    return (
      <div className="probe-card-stack probe-host-card">
        <div className="probe-host-card-head">
          <span className="agent-brand-badge" data-agent={meta.brand} aria-hidden="true">
            <AgentBrandIcon agent={meta.brand} size={16} />
          </span>
          <div className="probe-host-card-copy">
            <strong>{meta.title}</strong>
            <span>{meta.subtitle}</span>
          </div>
          <code className="probe-host-path">{meta.hostPath}</code>
        </div>
        <div className="settings-meta-grid">
          <Metric label={meta.metricLabel} value="仅限本机" />
          <Metric label="宿主模式" value="远端只读轮询" />
        </div>
      </div>
    );
  }

  const installed = status.data?.installed;
  const managedCount = status.data?.managedCount ?? 0;
  const busy = status.isLoading || status.isError || mutation.isPending;

  return (
    <>
      <div className="probe-card-stack probe-host-card">
        <div className="probe-host-card-head">
          <span className="agent-brand-badge" data-agent={meta.brand} aria-hidden="true">
            <AgentBrandIcon agent={meta.brand} size={16} />
          </span>
          <div className="probe-host-card-copy">
            <strong>{meta.title}</strong>
            <span>{meta.subtitle}</span>
          </div>
          <code className="probe-host-path">{meta.hostPath}</code>
        </div>
        <div className="settings-meta-grid">
          <Metric
            label={meta.metricLabel}
            value={
              status.isLoading
                ? "读取中"
                : status.isError
                  ? "读取失败"
                  : installed
                    ? "已安装"
                    : managedCount
                      ? "需要修复"
                      : "未安装"
            }
            tone={installed ? "success" : "warning"}
          />
          <Metric
            label="实际命令"
            value={status.data ? `${managedCount} 条` : "—"}
            tone={installed ? "success" : "warning"}
          />
        </div>
        <div className="button-row">
          <button
            className="secondary-button"
            ref={installButton}
            disabled={busy || installed}
            onClick={() => setConfirm("install")}
          >
            <TerminalSquare size={17} />
            {installed ? "已安装" : managedCount ? "修复 Hook" : "安装 Hook"}
          </button>
          <button
            className="secondary-button"
            ref={removeButton}
            disabled={!managedCount || busy}
            onClick={() => setConfirm("remove")}
          >
            <Trash2 size={17} />移除 Hook
          </button>
        </div>
        {(status.error || mutation.error) && (
          <div className="form-error" role="alert">
            {(status.error || mutation.error)?.message}
          </div>
        )}
      </div>
      {confirm && (
        <ConfirmDialog
          labelledBy={dialogId}
          busy={mutation.isPending}
          returnFocus={confirm === "install" ? installButton : removeButton}
          onCancel={() => setConfirm(null)}
        >
          <h3 id={dialogId}>
            {confirm === "install" ? `安装本机 ${meta.title} hooks` : `移除本机 ${meta.title} hooks`}
          </h3>
          <p>
            {confirm === "install"
              ? `向 ${meta.hostPath} 安装通知 Hook，保留其他已有 Hook 配置。`
              : `从 ${meta.hostPath} 移除托管通知 Hook，保留其他已有 Hook 配置。`}
          </p>
          <div className="button-row">
            <button
              className="secondary-button"
              disabled={mutation.isPending}
              onClick={() => setConfirm(null)}
            >
              取消
            </button>
            <button
              className={confirm === "install" ? "primary-button" : "danger-button"}
              disabled={mutation.isPending}
              onClick={() => mutation.mutate(confirm)}
            >
              确认{confirm === "install" ? "安装" : "移除"}
            </button>
          </div>
        </ConfirmDialog>
      )}
    </>
  );
}

export function DshHostCard() {
  return (
    <div className="probe-card-stack probe-host-card">
      <div className="probe-host-card-head">
        <span className="agent-brand-badge" data-agent="dsh" aria-hidden="true">
          <AgentBrandIcon agent="dsh" size={16} />
        </span>
        <div className="probe-host-card-copy">
          <strong>DeepSeek Harness</strong>
          <span>原生 .lock 进程锁与 .zstd 增量帧直连，零外部 Hook 依赖</span>
        </div>
        <code className="probe-host-path">~/.dsh/sessions</code>
      </div>
      <div className="settings-meta-grid">
        <Metric label="DSH Host" value="原生直连" tone="success" />
        <Metric label="状态引擎" value=".lock + turn/end 精确结算" tone="success" />
      </div>
      <div className="button-row">
        <button className="secondary-button" disabled>
          <TerminalSquare size={17} />免安装原生托管
        </button>
      </div>
    </div>
  );
}
