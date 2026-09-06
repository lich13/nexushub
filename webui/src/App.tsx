import {
  Bot,
  Cloud,
  HardDrive,
  LogOut,
  Menu,
  MessageSquare,
  PanelLeftClose,
  PanelLeftOpen,
  ShieldCheck,
  TriangleAlert
} from "lucide-react";
import { Component, ErrorInfo, ReactNode, useState } from "react";
import { WebAuthGate } from "./components/auth/WebAuthGate";
import { ChatWorkspace } from "./components/chat/ChatWorkspace";
import { OpsWorkspace } from "./components/ops/OpsWorkspace";
import { ProbeWorkspace } from "./components/probe/ProbeWorkspace";
import { SecurityWorkspace } from "./components/security/SecurityWorkspace";
import { PROBE_NAV_LABEL } from "./lib/probeUi";
import { desktopRuntimeSessionUser, logoutRuntime } from "./lib/query/auth";
import { GrokWorkspace } from "./components/grok/GrokWorkspace";
import {
  useBootstrapRuntimeCapabilities,
  useRuntimeCapabilities,
  useSystemStatusQuery,
  type RuntimeCapabilityMatrix
} from "./lib/query/system";
import {
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
  applyThreadTitleOverride,
  cleanThreadPreviewText,
  conversationTitleText,
  extractPlanText,
  filterVisibleThreadSummaries,
  isThreadListItemRunning,
  isThreadRunning,
  lastEventKindText,
  mergeIncomingThreadSummary,
  mergeThreadDetailSummaryFromList,
  modelSupportsServiceTier,
  nextVisibleThreadIdAfterRemoval,
  optionalUnavailableMessage,
  renderConversationHeaderHtml,
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
export { preservePreviousQueryData } from "./lib/query/shared";

export const navigationItems: Array<{ id: View; label: string; icon: ReactNode }> = [
  { id: "codex", label: "Codex", icon: <MessageSquare /> },
  { id: "grok", label: "Grok Build", icon: <Bot /> },
  { id: "probe", label: PROBE_NAV_LABEL, icon: <TriangleAlert /> },
  { id: "ops", label: "设置", icon: <HardDrive /> }
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
  const [settingsSection, setSettingsSection] = useState<"system" | "maintenance" | "security">("system");
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
            {view === "grok" && <GrokWorkspace csrfToken={session?.csrf_token} />}
            {view === "probe" && session && <ProbeWorkspace csrfToken={session.csrf_token} capabilities={capabilities} />}
            {view === "ops" && session && <div className="settings-workspace">
              <header className="settings-header"><h1>设置</h1><div className="settings-tabs" role="tablist">
                <button role="tab" aria-selected={settingsSection === "system"} onClick={() => setSettingsSection("system")}>系统与更新</button>
                <button role="tab" aria-selected={settingsSection === "maintenance"} onClick={() => setSettingsSection("maintenance")}>维护</button>
                {capabilities.securitySettings && <button role="tab" aria-selected={settingsSection === "security"} onClick={() => setSettingsSection("security")}>账户与安全</button>}
              </div></header>
              {settingsSection === "security" && capabilities.securitySettings ? <SecurityWorkspace csrfToken={session.csrf_token} username={session.username} /> : <OpsWorkspace section={settingsSection === "maintenance" ? "maintenance" : "system"} csrfToken={session.csrf_token} capabilities={capabilities} />}
              {settingsSection === "maintenance" && <ProbeWorkspace maintenance csrfToken={session.csrf_token} capabilities={capabilities} />}
            </div>}
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
