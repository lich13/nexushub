import {
  Bot,
  HardDrive,
  Menu,
  MessageSquare,
  PanelLeftClose,
  PanelLeftOpen,
  Pi as PiIcon,
  TriangleAlert
} from "lucide-react";
import { Component, ErrorInfo, ReactNode, useState } from "react";
import { MachineButton, RemoteConnectionSettings } from "./components/connection/RemoteConnection";
import { useConnection } from "./lib/query/connection";
import { ChatWorkspace } from "./components/chat/ChatWorkspace";
import { OpsWorkspace } from "./components/ops/OpsWorkspace";
import { ProbeWorkspace } from "./components/probe/ProbeWorkspace";
import { PROBE_NAV_LABEL } from "./lib/probeUi";
import { GrokWorkspace } from "./components/grok/GrokWorkspace";
import { ClaudeWorkspace } from "./components/claude/ClaudeWorkspace";
import { PiWorkspace } from "./components/pi/PiWorkspace";
import { SearchWorkspaceProvider } from "./components/common/SessionSearch";
import { useBackgroundJobs } from "./lib/query/jobs";
import {
  useBootstrapRuntimeCapabilities,
  useRuntimeCapabilities,
  useSystemCapabilitiesQuery,
  type RuntimeCapabilityMatrix
} from "./lib/query/system";
import {
  type RuntimeCapabilityInput
} from "./lib/domain/runtimeViewModel";
import {
  navigationLabelsForRuntime as navigationLabelsForRuntimeDomain,
  visibleNavigationItems,
  type View
} from "./lib/domain/codexViewModel";



export const navigationItems: Array<{ id: View; label: string; icon: ReactNode }> = [
  { id: "codex", label: "Codex", icon: <MessageSquare /> },
  { id: "claude", label: "Claude Code", icon: <Bot /> },
  { id: "grok", label: "Grok Build", icon: <Bot /> },
  { id: "pi", label: "Pi", icon: <PiIcon /> },
  { id: "probe", label: PROBE_NAV_LABEL, icon: <TriangleAlert /> },
  { id: "ops", label: "设置", icon: <HardDrive /> }
];

function navigationItemsForCapabilities(capabilities: RuntimeCapabilityMatrix) {
  return visibleNavigationItems(navigationItems, capabilities);
}

export function navigationLabelsForRuntime(desktop?: RuntimeCapabilityInput): string[] {
  return navigationLabelsForRuntimeDomain(navigationItems, desktop);
}

// Navigation preferences contain no machine data. Business views remount per revision.
let lastView: View = "codex";
let lastSettingsSection: "maintenance" | "remote" = "maintenance";

export default function App() {
  const bootstrapCapabilities = useBootstrapRuntimeCapabilities();
  const connection = useConnection();
  const systemCapabilities = useSystemCapabilitiesQuery();
  useBackgroundJobs(true);
  const capabilities = useRuntimeCapabilities(systemCapabilities.data, bootstrapCapabilities);
  const [view, setViewState] = useState<View>(lastView);
  const setView = (next: View) => { lastView = next; setViewState(next); };
  const [settingsSection, setSettingsSectionState] = useState<"maintenance" | "remote">(lastSettingsSection);
  const setSettingsSection = (next: "maintenance" | "remote") => { lastSettingsSection = next; setSettingsSectionState(next); };
  const [mobileThreadsOpen, setMobileThreadsOpen] = useState(true);
  const [navCollapsed, setNavCollapsed] = useState(() => localStorage.getItem("nexushub.nav-collapsed") === "1");

  const toggleNavCollapsed = () => {
    setNavCollapsed((current) => {
      const next = !current;
      localStorage.setItem("nexushub.nav-collapsed", next ? "1" : "0");
      return next;
    });
  };

  return <SearchWorkspaceProvider><div className={`app-shell ${navCollapsed ? "nav-collapsed" : ""}`}>
      <SideNav view={view} setView={setView} collapsed={navCollapsed} capabilities={capabilities} onCollapse={toggleNavCollapsed} onConfigure={() => { setView("ops"); setSettingsSection("remote"); }} />
        <main className="main-workspace">
          {(connection.error || systemCapabilities.error) && <div role="alert" className="form-error">{connection.error || systemCapabilities.error?.message}</div>}
          <MobileTopBar onOpenThreads={() => setMobileThreadsOpen(true)} view={view} setView={setView} capabilities={capabilities} />
          <WorkspaceErrorBoundary resetKey={view}>
            {view === "codex" && (
              <ChatWorkspace
                mobileThreadsOpen={mobileThreadsOpen}
                setMobileThreadsOpen={setMobileThreadsOpen}
                setView={setView}
                capabilities={capabilities}
              />
            )}
            {view === "grok" && <GrokWorkspace />}
            {view === "pi" && <PiWorkspace />}
            {view === "claude" && <ClaudeWorkspace />}
            {view === "probe" && <ProbeWorkspace capabilities={capabilities} />}
            {view === "ops" && <div className="settings-workspace">
              <header className="settings-header"><h1>设置</h1><div className="settings-tabs" role="tablist">
                <button role="tab" aria-selected={settingsSection === "maintenance"} onClick={() => setSettingsSection("maintenance")}>更新与维护</button>
                <button role="tab" aria-selected={settingsSection === "remote"} onClick={() => setSettingsSection("remote")}>远程连接</button>
              </div></header>
              {settingsSection === "remote" ? <RemoteConnectionSettings /> : <OpsWorkspace capabilities={capabilities} />}
            </div>}
          </WorkspaceErrorBoundary>
        </main>
      </div></SearchWorkspaceProvider>;
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

function SideNav({ view, setView, collapsed, capabilities, onCollapse, onConfigure }: {
  view: View;
  setView: (view: View) => void;
  collapsed: boolean;
  capabilities: RuntimeCapabilityMatrix;
  onCollapse: () => void;
  onConfigure: () => void;
}) {
  const items = navigationItemsForCapabilities(capabilities);
  return (
    <aside className="side-nav">
      <div className="nav-brand">
        <strong>NexusHub</strong>
        <button className="icon-button nav-collapse-button" onClick={onCollapse} title={collapsed ? "展开导航" : "折叠导航"} aria-expanded={!collapsed}>{collapsed ? <PanelLeftOpen size={17} /> : <PanelLeftClose size={17} />}</button>
      </div>
      <MachineButton collapsed={collapsed} onConfigure={onConfigure} />
      <nav>
        {items.map((item) => (
          <NavButton key={item.id} icon={item.icon} active={view === item.id} onClick={() => setView(item.id)}>
            {item.label}
          </NavButton>
        ))}
      </nav>
    </aside>
  );
}

function NavButton({ icon, active, onClick, children }: { icon: ReactNode; active: boolean; onClick: () => void; children: ReactNode }) {
  return <button className={`nav-button ${active ? "active" : ""}`} title={String(children)} aria-label={String(children)} aria-current={active ? "page" : undefined} onClick={onClick}>{icon}<span>{children}</span></button>;
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
