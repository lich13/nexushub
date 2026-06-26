import {
  Bot,
  CheckCircle2,
  Cloud,
  ClipboardCheck,
  Database,
  Files,
  HardDrive,
  KeyRound,
  LogOut,
  Menu,
  MessageSquare,
  PanelLeftClose,
  PanelLeftOpen,
  ShieldCheck,
  TerminalSquare,
  TriangleAlert
} from "lucide-react";
import { Component, ErrorInfo, ReactNode, useState } from "react";
import { WebAuthGate } from "./components/auth/WebAuthGate";
import { ChatWorkspace } from "./components/chat/ChatWorkspace";
import { Metric, Panel } from "./components/common/Panel";
import { OpsWorkspace } from "./components/ops/OpsWorkspace";
import { ProbeWorkspace } from "./components/probe/ProbeWorkspace";
import { SecurityWorkspace } from "./components/security/SecurityWorkspace";
import { PROBE_NAV_LABEL } from "./lib/probeUi";
import { desktopRuntimeSessionUser, logoutRuntime } from "./lib/query/auth";
import { useClaudeQueries } from "./lib/query/claude";
import {
  useBootstrapRuntimeCapabilities,
  useRuntimeCapabilities,
  useSystemStatusQuery,
  type RuntimeCapabilityMatrix
} from "./lib/query/system";
import {
  pathText,
  type RuntimeCapabilityInput
} from "./lib/domain/runtimeViewModel";
import {
  navigationLabelsForRuntime as navigationLabelsForRuntimeDomain,
  shouldUseSavedSessionForRuntime,
  visibleNavigationItems,
  type View
} from "./lib/domain/codexViewModel";
import { clearSession, loadSession, saveSession } from "./lib/session";
import type {
  AgentProviderInfo,
  ClaudeOverview,
  SessionUser,
} from "./types";

export { probeEventCard } from "./lib/probeUi";
export { runtimeCapabilitiesForRuntime } from "./lib/query/system";
export { mergeMessageBlocks, mergeThreadSummaryIntoListCache, upsertMessageBlock } from "./lib/query/threads";
export { statusTabs } from "./components/chat/ChatWorkspace";
export * from "./components/chat/Conversation";
export {
  approvalActionMode,
  canStartHiddenThreadDelete,
  canStartUpdateInstall,
  canShowForkAction,
  codexHomeStatusValue,
  desktopRuntimeVisibleCopy,
  failureCategoryLabel,
  formatGoalTimestamp,
  goalControlState,
  goalStatusLabel,
  goalStatusTone,
  logsDbPathStatusValue,
  archivePlanAfterExecute,
  hiddenRolloutDeleteResultText,
  opsWorkspaceView,
  jobFailureAnalysisView,
  jobOutputView,
  pathText,
  probeAvailabilityView,
  probeDiscoveryWarningsText,
  probeEventSummary,
  probeJobActionLabel,
  probeLogDbNumber,
  probeLogDbSize,
  probeLogDbString,
  probeLogsDbTone,
  probeRunningCountValue,
  probeSettingsAfterBarkSave,
  probeSnapshotStatusText,
  probeStateLabel,
  probeStatusThreads,
  probeThreadsByStatus,
  probeWorkspaceView,
  opsUpdateActionView,
  resolvedSelectedThreadId,
  shouldAutoScrollProbeFeed,
  opsWorkspacePanelTitles,
  opsWorkspaceVisibleCopy,
  threadInspectorActionState
} from "./lib/domain/runtimeViewModel";
export {
  actionMessage,
  applyPermissionPreset,
  applyThreadTitleOverride,
  buildPayload,
  cleanThreadPreviewText,
  conversationTitleText,
  defaultRunConfig,
  extractPlanText,
  filterVisibleThreadSummaries,
  isThreadListItemRunning,
  isThreadRunning,
  lastEventKindText,
  mergeIncomingThreadSummary,
  mergeRunConfigFromDefaults,
  mergeThreadDetailSummaryFromList,
  modelSupportsServiceTier,
  nextVisibleThreadIdAfterRemoval,
  optionalUnavailableMessage,
  renderConversationHeaderHtml,
  runConfigAfterSuccessfulSend,
  runConfigWithSupportedServiceTier,
  setLocalThreadTitleOverride,
  clearLocalThreadTitleOverride,
  shouldHydrateThreadDetail,
  shouldShowLogoutForRuntime,
  shouldUseSavedSessionForRuntime,
  threadDetailRefetchInterval,
  threadListItemPreviewText,
  threadListItemStatusText,
  threadListItemText,
  threadMatchesListFilter,
  threadSettingsMetricLabels
} from "./lib/domain/codexViewModel";
export { slashCommandAction, slashCommandExecutionPlan, slashCommands, slashCommandsForRuntime } from "./lib/domain/slashCommands";
export { preservePreviousQueryData } from "./lib/query/shared";
export {
  activeComposerMenuKind,
  applyPluginMentionSelection,
  applySlashCommandSelection,
  composerActionLabel,
  composerActionMode,
  composerActionTitle,
  composerFileInputAcceptValue,
  composerMenuKeyAction,
  composerSubmitDraftValue,
  composerUploadIds,
  exactSlashCommandFromDraft,
  formatFileSize,
  pluginMentionSuggestions,
  readyComposerUploads,
  renderPluginMentionMenuHtml,
  renderSlashCommandMenuHtml,
  nextSlashCommandSelection,
  slashCommandForComposerSubmit,
  slashCommandKeyAction,
  slashCommandSuggestions,
  uploadKindLabel,
  uploadStatusText
} from "./components/composer/ComposerControls";

export const navigationItems: Array<{ id: View; label: string; icon: ReactNode }> = [
  { id: "codex", label: "Codex", icon: <MessageSquare /> },
  { id: "claude", label: "Claude Code", icon: <Bot /> },
  { id: "probe", label: PROBE_NAV_LABEL, icon: <TriangleAlert /> },
  { id: "ops", label: "运维", icon: <HardDrive /> },
  { id: "security", label: "安全", icon: <ShieldCheck /> }
];

function navigationItemsForCapabilities(capabilities: RuntimeCapabilityMatrix) {
  return visibleNavigationItems(navigationItems, capabilities);
}

export function navigationLabelsForRuntime(desktop?: RuntimeCapabilityInput): string[] {
  return navigationLabelsForRuntimeDomain(navigationItems, desktop);
}

export function initialSessionForRuntime(desktop?: RuntimeCapabilityInput): SessionUser | null {
  return shouldUseSavedSessionForRuntime(desktop) ? loadSession() : desktopRuntimeSessionUser();
}

export default function App() {
  const bootstrapCapabilities = useBootstrapRuntimeCapabilities();
  const [session, setSession] = useState<SessionUser | null>(() => initialSessionForRuntime(bootstrapCapabilities));
  const systemStatus = useSystemStatusQuery({ enabled: Boolean(session), refetchInterval: 8000 });
  const capabilities = useRuntimeCapabilities(systemStatus.data, bootstrapCapabilities);
  const [view, setView] = useState<View>("codex");
  const [mobileThreadsOpen, setMobileThreadsOpen] = useState(false);
  const [navCollapsed, setNavCollapsed] = useState(() => localStorage.getItem("nexushub.nav-collapsed") === "1");

  const toggleNavCollapsed = () => {
    setNavCollapsed((current) => {
      const next = !current;
      localStorage.setItem("nexushub.nav-collapsed", next ? "1" : "0");
      return next;
    });
  };

  return (
    <WebAuthGate
      session={session}
      webAuth={capabilities.webAuth}
      onLogin={(user) => {
        saveSession(user);
        setSession(user);
      }}
    >
      <div className={`app-shell ${navCollapsed ? "nav-collapsed" : ""}`}>
        {navCollapsed ? (
          <button className="nav-restore" onClick={toggleNavCollapsed} title="展开导航"><PanelLeftOpen size={18} /></button>
        ) : (
          <SideNav view={view} setView={setView} capabilities={capabilities} onCollapse={toggleNavCollapsed} onLogout={async () => {
            if (capabilities.logout && session) {
              await logoutRuntime(session.csrf_token);
              clearSession();
              setSession(null);
            }
          }} />
        )}
        <main className="main-workspace">
          <MobileTopBar onOpenThreads={() => setMobileThreadsOpen(true)} view={view} setView={setView} capabilities={capabilities} />
          <WorkspaceErrorBoundary resetKey={view}>
            {view === "codex" && session && (
              <ChatWorkspace
                csrfToken={session.csrf_token}
                mobileThreadsOpen={mobileThreadsOpen}
                setMobileThreadsOpen={setMobileThreadsOpen}
                setView={setView}
                capabilities={capabilities}
              />
            )}
            {view === "claude" && <ClaudeWorkspace />}
            {view === "probe" && session && <ProbeWorkspace csrfToken={session.csrf_token} capabilities={capabilities} />}
            {view === "ops" && session && <OpsWorkspace csrfToken={session.csrf_token} capabilities={capabilities} />}
            {capabilities.securitySettings && view === "security" && session && <SecurityWorkspace csrfToken={session.csrf_token} username={session.username} />}
          </WorkspaceErrorBoundary>
        </main>
      </div>
    </WebAuthGate>
  );
}

class WorkspaceErrorBoundary extends Component<
  { children: ReactNode; resetKey: string },
  { error: Error | null }
> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("NexusHub workspace render failed", error, info.componentStack);
  }

  componentDidUpdate(previous: { resetKey: string }) {
    if (previous.resetKey !== this.props.resetKey && this.state.error) {
      this.setState({ error: null });
    }
  }

  render() {
    if (this.state.error) {
      return (
        <section className="panel wide-panel">
          <header><TriangleAlert size={18} /><strong>视图载入失败</strong></header>
          <div className="form-error">{this.state.error.message || "未知错误"}</div>
        </section>
      );
    }
    return this.props.children;
  }
}

function SideNav({ view, setView, capabilities, onCollapse, onLogout }: {
  view: View;
  setView: (view: View) => void;
  capabilities: RuntimeCapabilityMatrix;
  onCollapse: () => void;
  onLogout: () => void;
}) {
  const items = navigationItemsForCapabilities(capabilities);
  return (
    <aside className="side-nav">
      <div className="nav-brand">
        <div className="brand-mark"><Cloud size={22} /></div>
        <div>
          <strong>NexusHub</strong>
          <span>Agent Ops</span>
        </div>
        <button className="icon-button nav-collapse-button" onClick={onCollapse} title="隐藏导航"><PanelLeftClose size={17} /></button>
      </div>
      <nav>
        {items.map((item) => (
          <NavButton key={item.id} icon={item.icon} active={view === item.id} onClick={() => setView(item.id)}>
            {item.label}
          </NavButton>
        ))}
      </nav>
      {capabilities.logout && (
        <button className="ghost-button nav-logout" onClick={onLogout}><LogOut size={17} />退出</button>
      )}
    </aside>
  );
}

function NavButton({ icon, active, onClick, children }: { icon: ReactNode; active: boolean; onClick: () => void; children: ReactNode }) {
  return <button className={`nav-button ${active ? "active" : ""}`} onClick={onClick}>{icon}{children}</button>;
}

function MobileTopBar({ onOpenThreads, view, setView, capabilities }: {
  onOpenThreads: () => void;
  view: View;
  setView: (view: View) => void;
  capabilities: RuntimeCapabilityMatrix;
}) {
  const items = navigationItemsForCapabilities(capabilities);
  const current = items.find((item) => item.id === view);
  return (
    <>
      <div className="mobile-topbar">
        <button className="icon-button" onClick={onOpenThreads} disabled={view !== "codex"} title={view === "codex" ? "打开线程" : "线程列表仅用于 Codex"}>
          <Menu size={20} />
        </button>
        <span>{current?.label ?? "NexusHub"}</span>
        <div className="topbar-dot" />
      </div>
      <div className="mobile-tabs">
        {items.map((item) => (
          <button key={item.id} className={view === item.id ? "active" : ""} onClick={() => setView(item.id)}>
            {item.icon}
            {item.label}
          </button>
        ))}
      </div>
    </>
  );
}

function ClaudeWorkspace() {
  const { providers, overview, platform } = useClaudeQueries();
  const provider = providerById(providers.data, "claude_code") ?? providerById(providers.data, "claude-code");
  const data = overview.data?.data;
  const available = overview.data?.available ?? false;
  const projectCount = data?.projects.length ?? 0;
  const sessionCount = totalClaudeSessions(data);
  const settingsSummary = claudeSettingsSummary(data?.settings_preview, data?.mcp);
  const installation = data?.installation;
  const cacheStatus = data?.cache_status;
  const healthText = installation?.health_hints?.length ? installation.health_hints.join(", ") : "ok";

  return (
    <div className="ops-grid">
      <Panel title="Claude Code" icon={<Bot size={18} />}>
        <Metric label="Name" value={provider?.label ?? "Claude Code"} />
        <Metric label="Mode" value="read-only workbench" tone="success" />
        <Metric label="Status" value={provider?.status ?? (available ? "preview" : "unavailable")} tone={provider?.status === "ready" || available ? "success" : "warning"} />
        <Metric label="Capabilities" value={capabilityText(provider)} />
      </Panel>
      <Panel title="Claude Home" icon={<HardDrive size={18} />}>
        <Metric label="Home" value={pathText(installation?.claude_home ?? data?.home)} />
        <Metric label="Settings" value={(installation?.settings_exists ?? data?.settings_exists) ? "present" : available ? "missing" : "unavailable"} tone={(installation?.settings_exists ?? data?.settings_exists) ? "success" : "warning"} />
        <Metric label="Projects" value={String(projectCount)} />
        <Metric label="Sessions" value={String(sessionCount)} />
      </Panel>
      <Panel title="Runtime Status" icon={<TerminalSquare size={18} />}>
        <Metric label="Platform" value={platform.data?.kind ?? "unknown"} />
        <Metric label="Service" value={platform.data ? `${platform.data.service_kind}:${platform.data.service_name}` : "unknown"} />
        <Metric label="Boundary" value={provider?.safety ?? "no launch/resume/send/stop"} />
        <Metric label="Refresh" value={overview.isFetching ? "refreshing" : "idle"} tone={overview.isFetching ? "warning" : undefined} />
      </Panel>
      <Panel title="MCP Summary" icon={<ClipboardCheck size={18} />}>
        <Metric label="MCP config" value={settingsSummary.mcp} tone={settingsSummary.mcp === "not detected" ? "warning" : "success"} />
        <Metric label="Permissions" value={settingsSummary.permissions} />
        <Metric label="Config files" value={String(data?.mcp?.config_files?.length ?? 0)} />
        <Metric label="Settings source" value={data?.settings_exists ? "settings preview" : "not loaded"} />
      </Panel>
      <Panel title="Install Health" icon={<CheckCircle2 size={18} />}>
        <Metric label="Version" value={installation?.version_hint ?? "unknown"} tone={installation?.version_hint ? "success" : "warning"} />
        <Metric label="Executable" value={installation?.executable_candidates?.[0] ?? "not found"} tone={installation?.executable_candidates?.length ? "success" : "warning"} />
        <Metric label="User config" value={installation?.user_config_exists ? "found" : "not found"} />
        <Metric label="Health" value={healthText} tone={installation?.health_hints?.length ? "warning" : "success"} />
      </Panel>
      <Panel title="Cache and Logs" icon={<Database size={18} />}>
        <Metric label="Cache" value={cacheStatus?.cache_exists ? "found" : "missing"} tone={cacheStatus?.cache_exists ? "success" : "warning"} />
        <Metric label="Cache files" value={String(cacheStatus?.cache_file_count ?? 0)} />
        <Metric label="Logs" value={cacheStatus?.log_exists ? "found" : "missing"} tone={cacheStatus?.log_exists ? "success" : "warning"} />
        <Metric label="Log files" value={String(cacheStatus?.log_file_count ?? 0)} />
      </Panel>
      <Panel title="Projects" icon={<Files size={18} />} className="wide-panel">
        <div className="preview-list">
          {(data?.projects ?? []).slice(0, 12).map((project) => (
            <article className="preview-item" key={project.id}>
              <div>
                <strong>{project.display_name}</strong>
                <span>{project.id}</span>
              </div>
              <small>{project.session_count} sessions</small>
            </article>
          ))}
          {available && projectCount === 0 && <div className="muted-row">未发现 Claude Code 项目</div>}
          {!available && <div className="muted-row">Claude Code preview endpoint unavailable</div>}
        </div>
      </Panel>
      <Panel title="Session Detail" icon={<MessageSquare size={18} />} className="wide-panel">
        <div className="preview-list compact">
          {recentClaudeSessions(data).map((session) => (
            <article className="preview-item" key={session.key}>
              <div>
                <strong>{session.title || session.id}</strong>
                <span>{session.project}</span>
              </div>
              <small>{session.message_count} messages · {session.updated_at ?? "unknown"}</small>
            </article>
          ))}
          {available && sessionCount === 0 && <div className="muted-row">暂无会话</div>}
        </div>
      </Panel>
      {data?.settings_preview !== undefined && (
        <Panel title="Settings Preview" icon={<KeyRound size={18} />} className="wide-panel">
          <pre className="config-preview">{formatPayload(data.settings_preview)}</pre>
        </Panel>
      )}
    </div>
  );
}

function providerById(providers: AgentProviderInfo[] | undefined, id: string): AgentProviderInfo | undefined {
  return providers?.find((provider) => provider.id === id);
}

function capabilityText(provider?: Pick<AgentProviderInfo, "capabilities"> | null): string {
  const capabilities = provider?.capabilities ?? [];
  return capabilities.length ? capabilities.join(", ") : "none";
}

function totalClaudeSessions(overview?: ClaudeOverview): number {
  return overview?.projects.reduce((total, project) => total + project.session_count, 0) ?? 0;
}

function claudeSettingsSummary(settings: unknown, mcpSummary?: ClaudeOverview["mcp"]): { mcp: string; permissions: string } {
  const raw = settings && typeof settings === "object" ? settings as Record<string, unknown> : {};
  const mcp = raw.mcpServers ?? raw.mcp_servers ?? raw.mcp;
  const permissions = raw.permissions;
  const serverCount = mcpSummary?.server_count;
  return {
    mcp: typeof serverCount === "number" ? `${serverCount} servers` : mcp && typeof mcp === "object" ? `${Object.keys(mcp as Record<string, unknown>).length} servers` : "not detected",
    permissions: permissions && typeof permissions === "object" ? "configured" : "unknown"
  };
}

function recentClaudeSessions(overview?: ClaudeOverview): Array<{
  key: string;
  project: string;
  id: string;
  title?: string | null;
  updated_at?: string | null;
  message_count: number;
}> {
  if (overview?.recent_sessions?.length) {
    return overview.recent_sessions.slice(0, 12).map((session) => ({
      key: `${session.project_id}:${session.id}`,
      project: session.project_display_name,
      id: session.id,
      title: session.title,
      updated_at: session.updated_at,
      message_count: session.message_count
    }));
  }
  return (overview?.projects ?? [])
    .flatMap((project) => project.sessions.map((session) => ({
      key: `${project.id}:${session.id}`,
      project: project.display_name,
      ...session
    })))
    .sort((a, b) => (b.updated_at ?? "").localeCompare(a.updated_at ?? ""))
    .slice(0, 12);
}


function formatPayload(payload: unknown): string {
  if (!payload) return "";
  if (typeof payload === "string") return payload;
  return JSON.stringify(payload, null, 2);
}
