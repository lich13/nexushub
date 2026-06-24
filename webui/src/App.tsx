import {
  Archive,
  Bot,
  CheckCircle2,
  ChevronRight,
  Cloud,
  ClipboardCheck,
  Copy,
  Database,
  Edit3,
  Files,
  GitFork,
  HardDrive,
  KeyRound,
  Lock,
  LogOut,
  Menu,
  MessageSquare,
  PanelLeftClose,
  PanelLeftOpen,
  Play,
  Plus,
  RefreshCw,
  Search,
  Send,
  ShieldCheck,
  SlidersHorizontal,
  Square,
  TerminalSquare,
  Trash2,
  TriangleAlert,
  Undo2,
  X
} from "lucide-react";
import { ChangeEvent, Component, ErrorInfo, FormEvent, ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
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
} from "./lib/probeUi";
import { desktopRuntimeSessionUser, logoutRuntime, useLoginMutation, usePublicSettingsQuery } from "./lib/query/auth";
import { useClaudeQueries } from "./lib/query/claude";
import { useCodexConfigQuery, useCodexModelQuery, useCodexPermissionProfilesQuery } from "./lib/query/codex";
import { useOpsActions, useOpsQueries } from "./lib/query/ops";
import { useProbeActions, useProbeQueries } from "./lib/query/probe";
import { useSecurityActions, useSecurityQuery } from "./lib/query/security";
import {
  useBootstrapRuntimeCapabilities,
  useRuntimeCapabilities,
  useSystemStatusQuery,
  type RuntimeCapabilityMatrix
} from "./lib/query/system";
import {
  OPS_PANEL_TITLES,
  approvalActionMode,
  canStartHiddenThreadDelete,
  canStartUpdateInstall,
  canShowForkAction,
  capabilitiesForInput,
  failureCategoryLabel,
  formatGoalTimestamp,
  goalControlState,
  goalStatusLabel,
  goalStatusTone,
  jobFailureAnalysisView,
  jobOutputView,
  opsUpdateActionView,
  resolvedSelectedThreadId,
  threadInspectorActionState,
  type RuntimeCapabilityInput
} from "./lib/domain/runtimeViewModel";
import {
  actionMessage,
  applyPermissionPreset,
  applyThreadTitleOverride,
  applyThreadTitleOverrideToDetail,
  applyThreadTitleOverrides,
  buildPayload,
  cleanThreadPreviewText,
  codexLocalCopy,
  conversationTitleText,
  defaultRunConfig,
  defaultSessionTtlDays,
  extractPlanText,
  filterVisibleThreadSummaries,
  isThreadListItemRunning,
  isThreadRunning,
  lastEventKindText,
  makeRunConfig,
  mergeIncomingThreadSummary,
  mergeRunConfigFromDefaults,
  mergeThreadDetailSummaryFromList,
  modelSupportsServiceTier,
  navigationLabelsForRuntime as navigationLabelsForRuntimeDomain,
  nextVisibleThreadIdAfterRemoval,
  optionalUnavailableMessage,
  reasoningOptions,
  renderConversationHeaderHtml,
  runConfigAfterSuccessfulSend,
  runConfigWithSupportedServiceTier,
  secondsPerDay,
  setLocalThreadTitleOverride,
  clearLocalThreadTitleOverride,
  shouldHydrateThreadDetail,
  shouldShowLogoutForRuntime,
  shouldUseSavedSessionForRuntime,
  sourceCountsText,
  threadDetailRefetchInterval,
  threadListItemPreviewText,
  threadListItemStatusText,
  threadListItemText,
  threadMatchesListFilter,
  threadSettingsMetricLabels,
  threadStatusLabel,
  visibleNavigationItems,
  type PermissionPresetId,
  type RunConfig,
  type SelectedThread,
  type View
} from "./lib/domain/codexViewModel";
import {
  slashCommandAction,
  slashCommands,
  slashCommandsForRuntime,
  type SlashCommand
} from "./lib/domain/slashCommands";
import {
  type ThreadCacheSnapshot,
  threadDetailFromSlot,
  useArchivedSelectedThreadCleanup,
  useThreadCacheActions,
  useCreateThreadMutation,
  useFollowUpsQuery,
  useHydrateThreadMessageStore,
  usePluginsQuery,
  useSelectedThreadState,
  useThreadActionMutations,
  useThreadBlockPageMutation,
  useThreadDetailHydration,
  useThreadDetailQuery,
  useThreadGoalActions,
  useThreadGoalQuery,
  useThreadMessageStoreController,
  useThreadRealtimeSubscription,
  useThreadsQuery,
  useUploadActions,
  type ThreadMessageSlot,
  type ThreadMessageStoreController
} from "./lib/query/threads";
import { clearSession, loadSession, saveSession } from "./lib/session";
import type {
  ArchiveDeletePlan,
  ArchiveDeleteResult,
  AgentProviderInfo,
  BridgeActionResult,
  ClaudeOverview,
  CodexGoal,
  CodexGoalSaveInput,
  CodexModel,
  FollowUpQueueItem,
  HiddenThreadDeletePlan,
  HiddenThreadDeleteResult,
  JobRecord,
  MessageBlock,
  PendingElicitation,
  PermissionProfile,
  PlatformOverview,
  ProbeEvent,
  ProbeJobAction,
  PluginInfo,
  ProbeLogsDbStatus,
  ProbeStatus,
  ProbeSettings,
  SecuritySettings,
  SessionUser,
  SystemStatus,
  ThreadBlockPage,
  UpdateStatus,
  ThreadDetail,
  ThreadStatus,
  ThreadSummary,
  UploadRecord
} from "./types";

export { runtimeCapabilitiesForRuntime } from "./lib/query/system";
export { mergeThreadSummaryIntoListCache } from "./lib/query/threads";
export {
  approvalActionMode,
  canStartHiddenThreadDelete,
  canStartUpdateInstall,
  canShowForkAction,
  desktopRuntimeVisibleCopy,
  failureCategoryLabel,
  formatGoalTimestamp,
  goalControlState,
  goalStatusLabel,
  goalStatusTone,
  jobFailureAnalysisView,
  jobOutputView,
  opsUpdateActionView,
  resolvedSelectedThreadId,
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
export { slashCommandAction, slashCommands, slashCommandsForRuntime } from "./lib/domain/slashCommands";
export { preservePreviousQueryData } from "./lib/query/shared";

type MessageScrollSnapshot = {
  scrollTop: number;
  clientHeight: number;
  scrollHeight: number;
};

type MessageBlockState = {
  blocks: MessageBlock[];
  totalBlocks: number;
  hasMoreBlocks: boolean;
  beforeCursor: string | null;
  visibleUpdateRevision: number;
  bottomFollowRevision: number;
};

type ComposerUpload = UploadRecord & {
  local_status?: "uploading" | "ready" | "error";
  local_error?: string | null;
};

export const statusTabs = [
  { id: "all", label: "全部" },
  { id: "running", label: "运行中" },
  { id: "reply-needed", label: "待回复" },
  { id: "recoverable", label: "异常" }
];

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

const permissionPresets: Array<{ id: PermissionPresetId; label: string; description: string; icon: ReactNode }> = [
  { id: "ask", label: "请求批准", description: "编辑外部文件和使用互联网时始终询问", icon: <Lock size={17} /> },
  { id: "auto", label: "替我审批", description: "仅对检测到的风险操作请求批准", icon: <ShieldCheck size={17} /> },
  { id: "full", label: "完全访问权限", description: "可不受限制地访问互联网和文件", icon: <CheckCircle2 size={17} /> },
  { id: "custom", label: "自定义 (config.toml)", description: "使用 config.toml 中定义的权限", icon: <SlidersHorizontal size={17} /> }
];

type PluginMentionCandidate = {
  id: string;
  label: string;
  description: string;
  unavailableReason?: string | null;
  plugin?: PluginInfo;
};

type TurnstileWidgetId = string;

declare global {
  interface Window {
    turnstile?: {
      render: (container: HTMLElement, options: {
        sitekey: string;
        action?: string;
        theme?: "dark" | "light" | "auto";
        callback?: (token: string) => void;
        "expired-callback"?: () => void;
        "error-callback"?: () => void;
      }) => TurnstileWidgetId;
      reset: (widgetId?: TurnstileWidgetId) => void;
      remove?: (widgetId: TurnstileWidgetId) => void;
    };
  }
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

  if (!session && capabilities.webAuth) {
    return <LoginScreen onLogin={(user) => {
      saveSession(user);
      setSession(user);
    }} />;
  }
  if (!session) return null;

  return (
    <div className={`app-shell ${navCollapsed ? "nav-collapsed" : ""}`}>
      {navCollapsed ? (
        <button className="nav-restore" onClick={toggleNavCollapsed} title="展开导航"><PanelLeftOpen size={18} /></button>
      ) : (
        <SideNav view={view} setView={setView} capabilities={capabilities} onCollapse={toggleNavCollapsed} onLogout={async () => {
          if (capabilities.logout) {
            await logoutRuntime(session.csrf_token);
            clearSession();
            setSession(null);
          }
        }} />
      )}
      <main className="main-workspace">
        <MobileTopBar onOpenThreads={() => setMobileThreadsOpen(true)} view={view} setView={setView} capabilities={capabilities} />
        <WorkspaceErrorBoundary resetKey={view}>
          {view === "codex" && (
            <ChatWorkspace
              csrfToken={session.csrf_token}
              mobileThreadsOpen={mobileThreadsOpen}
              setMobileThreadsOpen={setMobileThreadsOpen}
              setView={setView}
              capabilities={capabilities}
            />
          )}
          {view === "claude" && <ClaudeWorkspace />}
          {view === "probe" && <ProbeWorkspace csrfToken={session.csrf_token} capabilities={capabilities} />}
          {view === "ops" && <OpsWorkspace csrfToken={session.csrf_token} capabilities={capabilities} />}
          {capabilities.securitySettings && view === "security" && <SecurityWorkspace csrfToken={session.csrf_token} username={session.username} />}
        </WorkspaceErrorBoundary>
      </main>
    </div>
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

function LoginScreen({ onLogin }: { onLogin: (user: SessionUser) => void }) {
  const [username, setUsername] = useState("admin");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [turnstileToken, setTurnstileToken] = useState("");
  const [turnstileStatus, setTurnstileStatus] = useState<"idle" | "loading" | "ready" | "verified" | "error">("idle");
  const widgetRef = useRef<TurnstileWidgetId | null>(null);
  const turnstileRef = useRef<HTMLDivElement | null>(null);
  const publicSettings = usePublicSettingsQuery();
  const turnstileEnabled = Boolean(publicSettings.data?.turnstile_enabled && publicSettings.data.turnstile_site_key);
  const turnstileRequired = Boolean(publicSettings.data?.turnstile_required);
  const turnstileAction = publicSettings.data?.turnstile_action || "login";

  useEffect(() => {
    if (!turnstileEnabled || !publicSettings.data?.turnstile_site_key || !turnstileRef.current) {
      setTurnstileStatus("idle");
      setTurnstileToken("");
      return;
    }

    let cancelled = false;
    setTurnstileToken("");
    setTurnstileStatus("loading");
    ensureTurnstileScript()
      .then(() => {
        if (cancelled || !turnstileRef.current || !window.turnstile) return;
        if (widgetRef.current && window.turnstile.remove) {
          window.turnstile.remove(widgetRef.current);
          widgetRef.current = null;
        }
        turnstileRef.current.innerHTML = "";
        widgetRef.current = window.turnstile.render(turnstileRef.current, {
          sitekey: publicSettings.data.turnstile_site_key,
          action: turnstileAction,
          theme: "dark",
          callback: (token) => {
            setTurnstileToken(token);
            setTurnstileStatus("verified");
          },
          "expired-callback": () => {
            setTurnstileToken("");
            setTurnstileStatus("ready");
          },
          "error-callback": () => {
            setTurnstileToken("");
            setTurnstileStatus("error");
          }
        });
        setTurnstileStatus("ready");
      })
      .catch(() => {
        if (!cancelled) setTurnstileStatus("error");
      });

    return () => {
      cancelled = true;
      if (widgetRef.current && window.turnstile?.remove) {
        window.turnstile.remove(widgetRef.current);
        widgetRef.current = null;
      }
    };
  }, [turnstileAction, turnstileEnabled, publicSettings.data?.turnstile_site_key]);

  const resetTurnstile = () => {
    if (widgetRef.current && window.turnstile) {
      window.turnstile.reset(widgetRef.current);
      setTurnstileToken("");
      setTurnstileStatus("ready");
    }
  };

  const mutation = useLoginMutation(onLogin);

  return (
    <div className="login-shell">
      <form className="login-panel" onSubmit={(event) => {
        event.preventDefault();
        setError(null);
        if (turnstileEnabled && !turnstileToken.trim()) {
          setError("请先完成 Turnstile 验证");
          return;
        }
        mutation.mutate(
          { username, password, turnstileToken },
          {
            onError: (err) => {
              setError(err.message);
              resetTurnstile();
            }
          }
        );
      }}>
        <div className="brand-mark"><Cloud size={24} /></div>
        <h1>NexusHub</h1>
        <p>{codexLocalCopy.loginSubtitle}</p>
        <label>
          <span>管理员</span>
          <input value={username} onChange={(event) => setUsername(event.target.value)} autoComplete="username" />
        </label>
        <label>
          <span>密码</span>
          <input type="password" value={password} onChange={(event) => setPassword(event.target.value)} autoComplete="current-password" />
        </label>
        {turnstileEnabled && (
          <div className="turnstile-box">
            <div ref={turnstileRef} />
            <span>
              {turnstileRequired ? "Turnstile 强制验证" : "Turnstile 登录验证"}
              {turnstileStatus === "verified" ? "：已完成" : turnstileStatus === "loading" ? "：加载中" : turnstileStatus === "error" ? "：加载失败" : ""}
            </span>
          </div>
        )}
        {error && <div className="inline-error">{error}</div>}
        <button className="primary-button" disabled={mutation.isPending}>
          <Lock size={18} />
          登录
        </button>
      </form>
    </div>
  );
}

function ensureTurnstileScript(): Promise<void> {
  if (window.turnstile) return Promise.resolve();
  return new Promise((resolve, reject) => {
    const existing = document.getElementById("cloudflare-turnstile-script") as HTMLScriptElement | null;
    if (existing) {
      existing.addEventListener("load", () => resolve(), { once: true });
      existing.addEventListener("error", () => reject(new Error("Turnstile script failed")), { once: true });
      return;
    }
    const script = document.createElement("script");
    script.id = "cloudflare-turnstile-script";
    script.src = "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";
    script.async = true;
    script.defer = true;
    script.onload = () => resolve();
    script.onerror = () => reject(new Error("Turnstile script failed"));
    document.head.appendChild(script);
  });
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

function ChatWorkspace({ csrfToken, mobileThreadsOpen, setMobileThreadsOpen, setView, capabilities }: {
  csrfToken?: string | null;
  mobileThreadsOpen: boolean;
  setMobileThreadsOpen: (open: boolean) => void;
  setView: (view: View) => void;
  capabilities: RuntimeCapabilityMatrix;
}) {
  const threadCache = useThreadCacheActions();
  const [status, setStatus] = useState("all");
  const [q, setQ] = useState("");
  const messageStore = useThreadMessageStoreController();
  const threads = useThreadsQuery({
    status,
    q,
    select: applyThreadTitleOverrides
  });
  const selection = useSelectedThreadState(threads.data ?? []);
  const { selectedId, selectThread: setSelectedId, visibleThreads, resolvedSelected, selectedThreadSummary, nextThreadAfterRemoval } = selection;
  const detail = useThreadDetailQuery({
    threadId: resolvedSelected,
    selectedThreadSummary,
    select: applyThreadTitleOverrideToDetail,
    refetchInterval: threadDetailRefetchInterval
  });
  const { rawSelectedDetail, selectedDetail } = useThreadDetailHydration({
    threadId: resolvedSelected,
    detail: detail.data
  });
  useArchivedSelectedThreadCleanup({
    threadId: resolvedSelected,
    selectedId,
    rawSelectedDetail,
    visibleThreads,
    messageStore,
    threadCache,
    onSelect: setSelectedId
  });

  useEffect(() => {
    if (!resolvedSelected || !selectedThreadSummary) return;
    threadCache.mergeThreadDetailSummary(resolvedSelected, selectedThreadSummary);
  }, [threadCache, resolvedSelected, selectedThreadSummary]);
  useHydrateThreadMessageStore({
    threadId: resolvedSelected,
    selectedThreadSummary,
    selectedDetail,
    messageStore
  });

  const selectThread = (id: SelectedThread) => {
    setSelectedId(id);
    setMobileThreadsOpen(false);
  };

  const list = (
    <ThreadList
      status={status}
      q={q}
      setQ={setQ}
      setStatus={setStatus}
      threads={visibleThreads}
      selectedId={resolvedSelected}
      onSelect={selectThread}
      onNew={() => selectThread("__new")}
      onRefresh={() => threadCache.invalidateThreads()}
      loading={threads.isLoading}
    />
  );

  return (
    <div className="chat-layout">
      <aside className="thread-column desktop-only">{list}</aside>
      {mobileThreadsOpen && (
        <div className="drawer-backdrop" onClick={() => setMobileThreadsOpen(false)}>
          <aside className="thread-drawer" onClick={(event) => event.stopPropagation()}>
            <button className="icon-button drawer-close" onClick={() => setMobileThreadsOpen(false)} title="关闭"><X size={18} /></button>
            {list}
          </aside>
        </div>
      )}
      <section className="conversation-column">
        {resolvedSelected && (selectedDetail || messageStore.getSlot(resolvedSelected).summary) ? (
          <Conversation
            threadId={resolvedSelected}
            detail={selectedDetail ?? threadDetailFromSlot(resolvedSelected, messageStore.getSlot(resolvedSelected), selectedThreadSummary)}
            slot={messageStore.getSlot(resolvedSelected)}
            messageStore={messageStore}
            csrfToken={csrfToken}
            onSelect={(id) => selectThread(id)}
            onPanelSelect={setView}
            nextThreadAfterArchive={nextThreadAfterRemoval}
            capabilities={capabilities}
          />
        ) : (
          <EmptyConversation
            loading={Boolean(resolvedSelected && detail.isLoading)}
            csrfToken={csrfToken}
            onCreated={(id) => selectThread(id)}
            capabilities={capabilities}
          />
        )}
      </section>
    </div>
  );
}

function ThreadList({ status, q, setQ, setStatus, threads, selectedId, onSelect, onNew, onRefresh, loading }: {
  status: string;
  q: string;
  setQ: (value: string) => void;
  setStatus: (value: string) => void;
  threads: ThreadSummary[];
  selectedId: string | null;
  onSelect: (id: SelectedThread) => void;
  onNew: () => void;
  onRefresh: () => void;
  loading: boolean;
}) {
  return (
    <div className="thread-list">
      <div className="section-title thread-title-row">
        <div>
          <span>{codexLocalCopy.threadListEyebrow}</span>
          <strong>线程</strong>
        </div>
        <div className="thread-title-actions">
          <button className="icon-button compact" onClick={onRefresh} title="刷新线程"><RefreshCw size={16} /></button>
          <button className="icon-button compact primary-icon" onClick={onNew} title="新建线程"><Plus size={16} /></button>
        </div>
      </div>
      <label className="search-box">
        <Search size={16} />
        <input value={q} onChange={(event) => setQ(event.target.value)} placeholder="搜索标题或 ID" />
      </label>
      <div className="segmented">
        {statusTabs.map((tab) => (
          <button key={tab.id} className={status === tab.id ? "active" : ""} onClick={() => setStatus(tab.id)}>{tab.label}</button>
        ))}
      </div>
      <div className="thread-scroll">
        {loading && <div className="muted-row">正在读取 Codex 状态...</div>}
        {threads.map((thread) => {
          const title = threadListItemText(thread);
          const preview = threadListItemPreviewText(thread);
          const running = isThreadListItemRunning(thread);
          return (
            <button key={thread.id} className={`thread-item ${selectedId === thread.id ? "selected" : ""}${running ? " running" : ""}`} onClick={() => onSelect(thread.id)} title={title}>
              <span className="thread-item-content">
                <span className="thread-item-title">{title}</span>
                <span className="thread-item-meta">
                  {running ? (
                    <span className="thread-running-indicator" aria-label="运行中" title="运行中">
                      <span className="thread-running-spinner" aria-hidden="true" />
                    </span>
                  ) : (
                    <span className={`thread-item-status ${thread.status}`}>{threadListItemStatusText(thread)}</span>
                  )}
                  {preview && <span className="thread-item-preview">{preview}</span>}
                </span>
              </span>
            </button>
          );
        })}
        {!loading && threads.length === 0 && <div className="muted-row">没有匹配线程</div>}
      </div>
    </div>
  );
}

function useCodexRunOptions() {
  const models = useCodexModelQuery();
  const profiles = useCodexPermissionProfilesQuery();
  const config = useCodexConfigQuery();
  return {
    models: models.data?.available ? models.data.data ?? [] : [],
    profiles: profiles.data?.available ? profiles.data.data ?? [] : [],
    config: config.data?.available ? config.data.data : undefined,
    unavailable: {
      models: models.data && !models.data.available,
      profiles: profiles.data && !profiles.data.available,
      config: config.data && !config.data.available
    }
  };
}

export function shouldAutoFollowMessageStream(snapshot: MessageScrollSnapshot, threshold = 96): boolean {
  return snapshot.scrollHeight - snapshot.scrollTop - snapshot.clientHeight <= threshold;
}

export function composerSubmitDraftValue(stateValue: string, domValue?: string | null): string {
  return typeof domValue === "string" ? domValue : stateValue;
}

function initialMessageBlockState(detail: ThreadDetail): MessageBlockState {
  const blocks = detail.blocks.length ? detail.blocks : legacyBlocks(detail);
  return {
    blocks,
    totalBlocks: detail.total_blocks ?? blocks.length,
    hasMoreBlocks: Boolean(detail.has_more_blocks),
    beforeCursor: detail.before_cursor ?? null,
    visibleUpdateRevision: 0,
    bottomFollowRevision: 0
  };
}

function mergeIncomingMessageBlockState(current: MessageBlockState, detail: ThreadDetail): MessageBlockState {
  const incomingBlocks = detail.blocks.length ? detail.blocks : legacyBlocks(detail);
  const nextBlocks = mergeMessageBlocks(current.blocks, incomingBlocks);
  return {
    blocks: nextBlocks,
    totalBlocks: detail.total_blocks ?? Math.max(current.totalBlocks, nextBlocks.length),
    hasMoreBlocks: Boolean(detail.has_more_blocks ?? current.hasMoreBlocks),
    beforeCursor: detail.before_cursor ?? current.beforeCursor,
    visibleUpdateRevision: nextBlocks === current.blocks ? current.visibleUpdateRevision : current.visibleUpdateRevision + 1,
    bottomFollowRevision: nextBlocks === current.blocks ? current.bottomFollowRevision : current.bottomFollowRevision + 1
  };
}

export function upsertMessageBlock(current: MessageBlock[], next: MessageBlock): MessageBlock[] {
  const existingIndex = current.findIndex((block) => block.id === next.id);
  if (existingIndex === -1) return [...current, next];

  const existing = current[existingIndex];
  if (existing === next || messageBlocksEqual(existing, next)) return current;

  const updated = [...current];
  updated[existingIndex] = next;
  return updated;
}

export function mergeMessageBlocks(current: MessageBlock[], incoming: MessageBlock[], mode: "append" | "prepend" = "append"): MessageBlock[] {
  if (!incoming.length) return current;
  let changed = false;
  let next = current;
  const ordered = mode === "prepend" ? [...incoming].reverse() : incoming;
  for (const block of ordered) {
    if (mode === "prepend" && !next.some((item) => item.id === block.id)) {
      next = [block, ...next];
      changed = true;
      continue;
    }
    const updated = upsertMessageBlock(next, block);
    if (updated !== next) {
      next = updated;
      changed = true;
    }
  }
  return changed ? next : current;
}

function messageBlocksEqual(left: MessageBlock, right: MessageBlock): boolean {
  try {
    return JSON.stringify(left) === JSON.stringify(right);
  } catch {
    return false;
  }
}

export type ComposerActionMode = "send" | "stop" | "followup" | "disabled";

export function isRunningToolBlock(block: MessageBlock): boolean {
  if (!isToolBlock(block)) return false;
  const status = block.status?.trim();
  return Boolean(status && ["pending", "running", "in_progress", "inProgress", "active"].includes(status));
}

export function composerActionMode(running: boolean, draft: string, canStop: boolean, attachmentCount = 0): ComposerActionMode {
  const hasContent = draft.trim().length > 0 || attachmentCount > 0;
  if (running && hasContent) return "followup";
  if (running) return canStop ? "stop" : "disabled";
  return hasContent ? "send" : "disabled";
}

export function composerActionLabel(mode: ComposerActionMode): string {
  if (mode === "stop") return "停止";
  if (mode === "followup") return "跟进";
  if (mode === "send") return "发送";
  return "发送";
}

export function composerActionTitle(mode: ComposerActionMode): string {
  if (mode === "stop") return "停止当前运行中的 turn";
  if (mode === "followup") return "跟进当前 turn；不可用时自动加入跟进队列";
  if (mode === "send") return "发送新 turn";
  return "输入消息后发送";
}

export function readyComposerUploads(uploads: ComposerUpload[]): ComposerUpload[] {
  return uploads.filter((upload) => upload.status === "ready" && upload.local_status !== "error");
}

export function composerUploadIds(uploads: ComposerUpload[]): string[] {
  return readyComposerUploads(uploads).map((upload) => upload.id).filter(Boolean);
}

export function formatFileSize(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "unknown";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KiB", "MiB", "GiB"];
  let value = bytes / 1024;
  for (const unit of units) {
    if (value < 1024 || unit === units[units.length - 1]) {
      return `${value.toFixed(value >= 10 ? 0 : 1)} ${unit}`;
    }
    value /= 1024;
  }
  return `${bytes} B`;
}

export function uploadKindLabel(kind?: string | null): string {
  if (kind === "markdown") return "Markdown";
  if (kind === "spreadsheet") return "表格";
  if (kind === "document") return "文档";
  if (kind === "pdf") return "PDF";
  if (kind === "image") return "图片";
  if (kind === "file") return "文件";
  return "文本";
}

export function uploadStatusText(upload: Pick<ComposerUpload, "status" | "local_status" | "error_preview" | "local_error">): string {
  if (upload.local_status === "uploading" || upload.status === "uploading") return "上传中";
  if (upload.local_status === "error" || upload.status === "error") return upload.local_error || upload.error_preview || "上传失败";
  return "已就绪";
}

export function composerFileInputAcceptValue(): string | undefined {
  return undefined;
}

export type InternalReferenceSegment = {
  type: "text" | "internal_reference";
  text: string;
  copyText?: string;
  kind?: "path" | "thread" | "turn" | "job";
};

const internalReferencePattern = /((?:\/(?:Users|Volumes|home|root|tmp|var|opt|srv|etc|run|private)\/[^\s,，。；;）)]+)|\b(?:thread|turn|job)[\s:=#-]+[A-Za-z0-9._:-]{3,})/gi;

export function segmentInternalReferences(text: string): InternalReferenceSegment[] {
  const segments: InternalReferenceSegment[] = [];
  let lastIndex = 0;
  for (const match of text.matchAll(internalReferencePattern)) {
    const value = match[0];
    const index = match.index ?? 0;
    if (index > lastIndex) {
      segments.push({ type: "text", text: text.slice(lastIndex, index) });
    }
    segments.push({
      type: "internal_reference",
      text: value,
      copyText: value,
      kind: internalReferenceKind(value)
    });
    lastIndex = index + value.length;
  }
  if (lastIndex < text.length) {
    segments.push({ type: "text", text: text.slice(lastIndex) });
  }
  return segments.length ? segments : [{ type: "text", text }];
}

function internalReferenceKind(value: string): InternalReferenceSegment["kind"] {
  const lower = value.toLowerCase();
  if (lower.startsWith("/")) return "path";
  if (lower.startsWith("thread")) return "thread";
  if (lower.startsWith("turn")) return "turn";
  return "job";
}

type SlashQuery = {
  start: number;
  end: number;
  value: string;
};

type TriggerQuery = SlashQuery & {
  trigger: "/" | "@";
};

function isInsideSimpleCodeContext(before: string): boolean {
  const backticks = (before.match(/`/g) ?? []).length;
  if (backticks % 2 === 1) return true;
  const singleQuotes = (before.match(/'/g) ?? []).length;
  const doubleQuotes = (before.match(/"/g) ?? []).length;
  return singleQuotes % 2 === 1 || doubleQuotes % 2 === 1;
}

function activeTriggerQuery(draft: string, cursor: number, trigger: "/" | "@"): TriggerQuery | null {
  const safeCursor = Math.max(0, Math.min(cursor, draft.length));
  const before = draft.slice(0, safeCursor);
  const start = before.lastIndexOf(trigger);
  if (start < 0) return null;
  if (start > 0 && !/\s/.test(before[start - 1])) return null;
  const value = before.slice(start);
  if (!value.startsWith(trigger) || value.includes("\n")) return null;
  if (trigger === "@" && isInsideSimpleCodeContext(before)) return null;
  return { start, end: safeCursor, value, trigger };
}

function activeSlashQuery(draft: string, cursor: number): SlashQuery | null {
  return activeTriggerQuery(draft, cursor, "/");
}

function activePluginMentionQuery(draft: string, cursor: number): SlashQuery | null {
  return activeTriggerQuery(draft, cursor, "@");
}

function nearestActiveComposerQuery(draft: string, cursor: number): TriggerQuery | null {
  const slash = activeTriggerQuery(draft, cursor, "/");
  const plugin = activeTriggerQuery(draft, cursor, "@");
  if (slash && plugin) return slash.start > plugin.start ? slash : plugin;
  return slash ?? plugin;
}

export function slashCommandSuggestions(draft: string, cursor: number, hasThread = true, desktop?: RuntimeCapabilityInput): SlashCommand[] {
  const query = activeSlashQuery(draft, cursor)?.value.toLowerCase();
  if (!query) return [];
  return slashCommandsForRuntime(desktop)
    .filter((item) => item.command.toLowerCase().startsWith(query));
}

export function applySlashCommandSelection(draft: string, cursor: number, command: string): { value: string; cursor: number } {
  const query = activeSlashQuery(draft, cursor);
  const insertion = `${command} `;
  if (!query) {
    const value = `${draft.slice(0, cursor)}${insertion}${draft.slice(cursor)}`;
    return { value, cursor: cursor + insertion.length };
  }
  const value = `${draft.slice(0, query.start)}${insertion}${draft.slice(query.end)}`;
  return { value, cursor: query.start + insertion.length };
}

export function pluginMentionSuggestions(
  draft: string,
  cursor: number,
  plugins: PluginInfo[] | null | undefined = [],
  unavailable = false
): PluginMentionCandidate[] {
  const query = activePluginMentionQuery(draft, cursor);
  if (!query) return [];
  const needle = query.value.slice(1).trim().toLowerCase();
  const rows = plugins ?? [];
  if (unavailable || rows.length === 0) {
    return [{
      id: "__plugins_unavailable__",
      label: "插件列表不可用",
      description: "当前无法读取插件列表",
      unavailableReason: "请稍后刷新，或在插件/Provider 页面查看可用能力。"
    }];
  }
  return rows
    .filter((plugin) => {
      if (!needle) return true;
      return plugin.id.toLowerCase().includes(needle) || plugin.label.toLowerCase().includes(needle);
    })
    .map((plugin) => ({
      id: plugin.id,
      label: plugin.label,
      description: plugin.description || plugin.kind || "插件能力",
      unavailableReason: plugin.unavailable_reason || (plugin.status === "planned" ? "当前能力尚未启用" : null),
      plugin
    }));
}

export function applyPluginMentionSelection(
  draft: string,
  cursor: number,
  plugin: Pick<PluginInfo, "id" | "label" | "invocation_template">
): { value: string; cursor: number } {
  const query = activePluginMentionQuery(draft, cursor);
  const label = (plugin.invocation_template || plugin.label || plugin.id).trim();
  const insertion = label.startsWith("@") ? `${label} ` : `@${label} `;
  if (!query) {
    const value = `${draft.slice(0, cursor)}${insertion}${draft.slice(cursor)}`;
    return { value, cursor: cursor + insertion.length };
  }
  const value = `${draft.slice(0, query.start)}${insertion}${draft.slice(query.end)}`;
  return { value, cursor: query.start + insertion.length };
}

export function exactSlashCommandFromDraft(draft: string, desktop?: RuntimeCapabilityInput): string | null {
  const command = draft.trim().replace(/\s+/g, " ");
  return slashCommandsForRuntime(desktop).some((item) => item.command === command) ? command : null;
}

export function slashCommandForComposerSubmit(draft: string, desktop?: RuntimeCapabilityInput): string | null {
  return exactSlashCommandFromDraft(draft, desktop);
}

export function activeComposerMenuKind(draft: string, cursor: number, plugins?: PluginInfo[] | null): "slash" | "plugin" | null {
  void plugins;
  const query = nearestActiveComposerQuery(draft, cursor);
  if (query?.trigger === "/") return "slash";
  if (query?.trigger === "@") return "plugin";
  return null;
}

export function nextSlashCommandSelection(current: number, total: number, key: string): number {
  if (key === "ArrowDown") return moveActionSelection(current, total, 1);
  if (key === "ArrowUp") return moveActionSelection(current, total, -1);
  return current;
}

export function composerMenuKeyAction({
  key,
  shiftKey = false,
  composing = false,
  menuSelectionArmed = false,
  selected,
  suggestions
}: {
  key: string;
  shiftKey?: boolean;
  composing?: boolean;
  menuSelectionArmed?: boolean;
  selected: number;
  suggestions: Array<{ command?: string; id?: string }>;
}): { action: "move"; selected: number } | { action: "insert"; index: number } | { action: "dismiss" } | { action: "none" } {
  if (composing) return { action: "none" };
  if (key === "ArrowDown" || key === "ArrowUp") {
    return { action: "move", selected: nextSlashCommandSelection(selected, suggestions.length, key) };
  }
  if (key === "Escape") return { action: "dismiss" };
  if (key === "Tab" && suggestions.length > 0) {
    return { action: "insert", index: Math.min(Math.max(selected, 0), suggestions.length - 1) };
  }
  if (key === "Enter" && !shiftKey && menuSelectionArmed && suggestions.length > 0) {
    return { action: "insert", index: Math.min(Math.max(selected, 0), suggestions.length - 1) };
  }
  return { action: "none" };
}

export function slashCommandKeyAction({
  key,
  shiftKey = false,
  selected,
  suggestions
}: {
  key: string;
  shiftKey?: boolean;
  selected: number;
  suggestions: Array<{ command: string }>;
}): { action: "move"; selected: number } | { action: "insert"; command: string } | { action: "dismiss" } | { action: "none" } {
  if (key === "ArrowDown" || key === "ArrowUp") {
    return { action: "move", selected: nextSlashCommandSelection(selected, suggestions.length, key) };
  }
  if (key === "Escape") return { action: "dismiss" };
  if (key === "Enter" && !shiftKey && suggestions.length > 0) {
    return { action: "insert", command: suggestions[selected]?.command ?? suggestions[0].command };
  }
  return { action: "none" };
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function renderSlashCommandMenuHtml(
  draft: string,
  cursor: number,
  hasThread = true,
  selected = 0,
  desktop?: RuntimeCapabilityInput,
): string {
  const suggestions = slashCommandSuggestions(draft, cursor, hasThread, desktop);
  if (suggestions.length === 0) return "";
  const options = suggestions.map((item, index) => {
    const className = index === selected ? "slash-option selected" : "slash-option";
    const threadBadge = item.requiresThread && !hasThread ? '<em class="slash-thread-note">需要已有线程</em>' : "";
    return [
      `<button type="button" class="${className}" role="option" aria-selected="${index === selected}">`,
      `<strong>${escapeHtml(item.command)}</strong>`,
      '<span class="slash-option-copy">',
      `<span>${escapeHtml(item.description)}</span>`,
      `<small>用法 ${escapeHtml(item.usageHint)}</small>`,
      threadBadge,
      "</span>",
      "</button>"
    ].join("");
  }).join("");
  return `<div class="slash-menu" role="listbox" aria-label="Slash 命令">${options}</div>`;
}

export function renderPluginMentionMenuHtml(
  draft: string,
  cursor: number,
  plugins: PluginInfo[] | null | undefined = [],
  unavailable = false,
  selected = 0
): string {
  const suggestions = pluginMentionSuggestions(draft, cursor, plugins, unavailable);
  if (suggestions.length === 0) return "";
  const options = suggestions.map((item, index) => {
    const className = index === selected ? "slash-option selected" : "slash-option";
    const reason = item.unavailableReason ? `<em>${escapeHtml(item.unavailableReason)}</em>` : "";
    return [
      `<button type="button" class="${className}" role="option" aria-selected="${index === selected}">`,
      `<strong>@${escapeHtml(item.label)}</strong>`,
      '<span class="slash-option-copy">',
      `<span>${escapeHtml(item.description)}</span>`,
      reason,
      "</span>",
      "</button>"
    ].join("");
  }).join("");
  return `<div class="slash-menu" role="listbox" aria-label="@ 插件">${options}</div>`;
}

export function planModeButtonState(nextMessagePlan: boolean, threadStatus?: string, hasPendingPlan = false, hasPendingQuestion = false): { pressed: boolean; label: string; statusText: string } {
  if (threadStatus === "ReplyNeeded" && hasPendingPlan) {
    return { pressed: nextMessagePlan, label: "Plan Mode", statusText: "当前线程正在等待计划确认" };
  }
  if (threadStatus === "ReplyNeeded" && hasPendingQuestion) {
    return { pressed: nextMessagePlan, label: "Plan Mode", statusText: "当前线程正在等待问题回复" };
  }
  return {
    pressed: nextMessagePlan,
    label: "Plan Mode",
    statusText: nextMessagePlan ? "下一条消息将使用 Plan Mode" : "下一条消息将直接发送"
  };
}

export function latestAssistantCopyText(blocks: MessageBlock[]): string | null {
  const latest = [...blocks].reverse().find((block) =>
    block.role === "assistant" && shouldRenderConversationMessage(block)
  );
  const text = latest ? messageBlockText(latest).trim() : "";
  return text || null;
}

export function nextRenameDraftValue(input: {
  previousThreadId: string;
  threadId: string;
  currentDraft: string;
  incomingTitle: string;
  dirty: boolean;
}): string {
  if (input.previousThreadId !== input.threadId) return input.incomingTitle;
  if (input.dirty) return input.currentDraft;
  const merged = mergeIncomingThreadSummary(
    { id: input.threadId, title: input.currentDraft },
    { id: input.threadId, title: input.incomingTitle }
  );
  return merged.title ?? input.currentDraft;
}

export function mergeSavedThreadTitle(threads: ThreadSummary[], threadId: string, title: string): ThreadSummary[] {
  return threads.map((thread) => thread.id === threadId ? { ...thread, title } : thread);
}

export function threadInspectorPanelTitles(): string[] {
  return ["名称与归档", "Goal", "复制与路径"];
}

export function threadResumeCommand(threadId?: string | null): string | null {
  const id = threadId?.trim();
  return id ? `codex resume ${id}` : null;
}

export function threadCopyId(threadId?: string | null): string | null {
  return threadId?.trim() || null;
}

export function threadRolloutPath(rolloutPath?: string | null): string | null {
  return rolloutPath?.trim() || null;
}

export function probeStatusThreads(status?: Pick<ProbeStatus, "running_threads" | "reply_needed_threads" | "recoverable_threads"> | null): ThreadSummary[] {
  return [
    ...(status?.running_threads ?? []),
    ...(status?.reply_needed_threads ?? []),
    ...(status?.recoverable_threads ?? [])
  ];
}

export function probeThreadsByStatus(status?: Pick<ProbeStatus, "running_threads" | "reply_needed_threads" | "recoverable_threads"> | null): {
  running: ThreadSummary[];
  replyNeeded: ThreadSummary[];
  recoverable: ThreadSummary[];
} {
  return {
    running: status?.running_threads ?? [],
    replyNeeded: status?.reply_needed_threads ?? [],
    recoverable: status?.recoverable_threads ?? []
  };
}

export function probeRunningCountValue(status?: Pick<ProbeStatus, "running_count" | "running_threads"> | null): string {
  const backendCount = typeof status?.running_count === "number" ? Math.max(0, status.running_count) : 0;
  const threadCount = status?.running_threads?.length ?? 0;
  return String(backendCount > 0 ? backendCount : threadCount);
}

export function probeSettingsAfterBarkSave<T extends { notifications: { device_key_configured?: boolean } }>(
  saved: T,
  submittedDeviceKey?: string | null,
): T {
  if (!submittedDeviceKey?.trim()) return saved;
  return {
    ...saved,
    notifications: {
      ...saved.notifications,
      device_key_configured: true
    }
  };
}

function isProbeJob(job: JobRecord): boolean {
  return job.kind.startsWith("probe_")
    || job.kind.startsWith("probe-")
    || job.title.includes("探针")
    || job.title.includes("Probe");
}

function probeJobActionLabel(action: ProbeJobAction | undefined): string {
  switch (action) {
    case "bark-test":
      return "Bark 测试";
    case "hooks-install":
      return "Hook 安装";
    case "logs-db-dry-run":
      return "日志库 dry-run";
    case "logs-db-execute":
      return "日志库维护";
    default:
      return "Probe job";
  }
}

function useComposerAttachments(csrfToken?: string | null, setFeedback?: (message: string | null) => void) {
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [uploads, setUploads] = useState<ComposerUpload[]>([]);
  const [uploadInProgress, setUploadInProgress] = useState(false);
  const [removingUploadId, setRemovingUploadId] = useState<string | null>(null);
  const uploadActions = useUploadActions({ csrfToken });
  const readyUploads = useMemo(() => readyComposerUploads(uploads), [uploads]);

  const openPicker = () => {
    if (!uploadInProgress) inputRef.current?.click();
  };

  const onFileInputChange = async (event: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.target.files ?? []);
    event.target.value = "";
    if (files.length === 0) return;

    const pending = files.map((file, index): ComposerUpload => ({
      id: `uploading-${Date.now()}-${index}-${Math.random().toString(36).slice(2)}`,
      name: file.name || `file-${index + 1}`,
      mime: file.type || "application/octet-stream",
      size: file.size,
      sha256: "",
      kind: "text",
      status: "uploading",
      local_status: "uploading"
    }));
    const pendingIds = new Set(pending.map((item) => item.id));
    setUploads((current) => [...current, ...pending]);
    setUploadInProgress(true);
    setFeedback?.("正在上传附件...");

    try {
      const outcome = await uploadActions.upload(files);
      setUploads((current) => [
        ...current.filter((item) => !pendingIds.has(item.id)),
        ...outcome.files.map((file) => ({ ...file, local_status: "ready" as const }))
      ]);
      setFeedback?.(`已上传 ${outcome.files.length} 个附件`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setUploads((current) => current.map((item) => pendingIds.has(item.id)
        ? {
          ...item,
          status: "error",
          local_status: "error",
          local_error: message,
          error_preview: message
        }
        : item));
      setFeedback?.(message);
    } finally {
      setUploadInProgress(false);
    }
  };

  const removeUpload = async (upload: ComposerUpload) => {
    setUploads((current) => current.filter((item) => item.id !== upload.id));
    if (upload.status !== "ready") return;
    setRemovingUploadId(upload.id);
    try {
      await uploadActions.delete(upload.id);
    } catch (error) {
      setFeedback?.(error instanceof Error ? error.message : String(error));
    } finally {
      setRemovingUploadId(null);
    }
  };

  const clearUploads = () => setUploads([]);

  return {
    inputRef,
    uploads,
    readyUploads,
    uploadInProgress,
    removingUploadId,
    openPicker,
    onFileInputChange,
    removeUpload,
    clearUploads
  };
}

function ComposerAttachmentList({
  uploads,
  removingUploadId,
  onRemove
}: {
  uploads: ComposerUpload[];
  removingUploadId?: string | null;
  onRemove: (upload: ComposerUpload) => void;
}) {
  if (uploads.length === 0) return null;
  return (
    <div className="attachment-list" aria-label="已选择附件">
      {uploads.map((upload) => {
        const errored = upload.local_status === "error" || upload.status === "error";
        const uploading = upload.local_status === "uploading" || upload.status === "uploading";
        return (
          <div key={upload.id} className={errored ? "attachment-chip error" : uploading ? "attachment-chip uploading" : "attachment-chip"}>
            <div className="attachment-copy">
              <strong title={upload.name}>{upload.name}</strong>
              <small>{uploadKindLabel(upload.kind)} · {formatFileSize(upload.size)} · {uploadStatusText(upload)}</small>
            </div>
            <button
              type="button"
              className="icon-button compact attachment-remove"
              onClick={() => onRemove(upload)}
              disabled={removingUploadId === upload.id}
              title="移除附件"
            >
              <X size={14} />
            </button>
          </div>
        );
      })}
    </div>
  );
}

function SlashCommandTextarea({
  inputRef,
  value,
  onChange,
  placeholder,
  hasThread,
  plugins,
  pluginsUnavailable = false,
  capabilities = capabilitiesForInput(),
  onSlashCommand,
  onSubmitShortcut,
  disabled = false
}: {
  inputRef?: (node: HTMLTextAreaElement | null) => void;
  value: string;
  onChange: (value: string) => void;
  placeholder: string;
  hasThread: boolean;
  plugins?: PluginInfo[] | null;
  pluginsUnavailable?: boolean;
  capabilities?: RuntimeCapabilityMatrix;
  onSlashCommand?: (command: string) => void;
  onSubmitShortcut?: (value?: string | null) => void;
  disabled?: boolean;
}) {
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const [cursor, setCursor] = useState(0);
  const [selected, setSelected] = useState(0);
  const [menuSelectionArmed, setMenuSelectionArmed] = useState(false);
  const [dismissedSignature, setDismissedSignature] = useState<string | null>(null);
  const signature = `${value}:${cursor}`;
  const menuKind = activeComposerMenuKind(value, cursor, plugins);
  const slashSuggestions = menuKind === "slash" ? slashCommandSuggestions(value, cursor, hasThread, capabilities) : [];
  const pluginSuggestions = menuKind === "plugin" ? pluginMentionSuggestions(value, cursor, plugins, pluginsUnavailable) : [];
  const suggestions = dismissedSignature === signature ? [] : menuKind === "plugin" ? pluginSuggestions : slashSuggestions;
  const open = suggestions.length > 0;
  const ariaLabel = menuKind === "plugin" ? "@ 插件" : "Slash 命令";
  const updateCursor = (target: HTMLTextAreaElement) => setCursor(target.selectionStart ?? target.value.length);
  const insertCommand = (command: string) => {
    const next = applySlashCommandSelection(value, cursor, command);
    onChange(next.value);
    setCursor(next.cursor);
    setSelected(0);
    setMenuSelectionArmed(false);
    requestAnimationFrame(() => {
      const textarea = textareaRef.current;
      if (!textarea) return;
      textarea.focus();
      textarea.setSelectionRange(next.cursor, next.cursor);
    });
  };
  const insertPlugin = (candidate: PluginMentionCandidate) => {
    if (candidate.id === "__plugins_unavailable__") return;
    const plugin = candidate.plugin ?? { id: candidate.id, label: candidate.label, status: "ready", kind: "builtin" };
    const next = applyPluginMentionSelection(value, cursor, plugin);
    onChange(next.value);
    setCursor(next.cursor);
    setSelected(0);
    setMenuSelectionArmed(false);
    requestAnimationFrame(() => {
      const textarea = textareaRef.current;
      if (!textarea) return;
      textarea.focus();
      textarea.setSelectionRange(next.cursor, next.cursor);
    });
  };
  const maybeRunExactSlashCommand = (currentValue = value) => {
    if (!onSlashCommand) return false;
    const command = exactSlashCommandFromDraft(currentValue, capabilities);
    if (!command) return false;
    onSlashCommand(command);
    return true;
  };
  const selectedSlashMatchesExactDraft = (command: string, currentValue = value) => exactSlashCommandFromDraft(currentValue, capabilities) === command;

  useEffect(() => {
    if (selected >= suggestions.length) {
      setSelected(0);
      setMenuSelectionArmed(false);
    }
  }, [selected, suggestions.length]);

  return (
    <div className="slash-composer">
      {open && (
        <div className="slash-menu" role="listbox" aria-label={ariaLabel}>
          {suggestions.map((item, index) => (
            <button
              key={menuKind === "plugin" ? (item as PluginMentionCandidate).id : (item as SlashCommand).command}
              type="button"
              className={index === selected ? "slash-option selected" : "slash-option"}
              onMouseDown={(event) => {
                event.preventDefault();
                if (menuKind === "plugin") {
                  insertPlugin(item as PluginMentionCandidate);
                } else {
                  insertCommand((item as SlashCommand).command);
                }
              }}
              role="option"
              aria-selected={index === selected}
            >
              <strong>{menuKind === "plugin" ? `@${(item as PluginMentionCandidate).label}` : (item as SlashCommand).command}</strong>
              <span className="slash-option-copy">
                <span>{item.description}</span>
                {menuKind === "plugin" ? null : <small>用法 {(item as SlashCommand).usageHint}</small>}
                {menuKind === "plugin" && (item as PluginMentionCandidate).unavailableReason ? <em>{(item as PluginMentionCandidate).unavailableReason}</em> : null}
                {menuKind !== "plugin" && (item as SlashCommand).requiresThread && !hasThread ? <em>需要已有线程</em> : null}
              </span>
            </button>
          ))}
        </div>
      )}
      <textarea
        ref={(node) => {
          textareaRef.current = node;
          inputRef?.(node);
        }}
        value={value}
        disabled={disabled}
        onChange={(event) => {
          onChange(event.target.value);
          setDismissedSignature(null);
          setMenuSelectionArmed(false);
          updateCursor(event.target);
        }}
        onClick={(event) => {
          setDismissedSignature(null);
          setMenuSelectionArmed(false);
          updateCursor(event.currentTarget);
        }}
        onKeyUp={(event) => {
          if (event.key !== "Escape") setDismissedSignature(null);
          updateCursor(event.currentTarget);
        }}
        onKeyDown={(event) => {
          if (!open) {
            if (event.nativeEvent.isComposing) return;
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              if (!maybeRunExactSlashCommand(event.currentTarget.value)) {
                onSubmitShortcut?.(event.currentTarget.value);
              }
            }
            return;
          }
          const action = composerMenuKeyAction({
            key: event.key,
            shiftKey: event.shiftKey,
            composing: event.nativeEvent.isComposing,
            menuSelectionArmed,
            selected,
            suggestions
          });
          if (action.action === "move") {
            event.preventDefault();
            setSelected(action.selected);
            setMenuSelectionArmed(true);
          } else if (action.action === "dismiss") {
            event.preventDefault();
            setSelected(0);
            setMenuSelectionArmed(false);
            setDismissedSignature(signature);
          } else if (action.action === "insert") {
            event.preventDefault();
            const item = suggestions[action.index];
            if (!item) return;
            if (menuKind === "plugin") {
              insertPlugin(item as PluginMentionCandidate);
            } else {
              const command = (item as SlashCommand).command;
              if (selectedSlashMatchesExactDraft(command, event.currentTarget.value) && maybeRunExactSlashCommand(event.currentTarget.value)) {
                return;
              }
              insertCommand(command);
            }
          } else if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
            event.preventDefault();
            if (!maybeRunExactSlashCommand(event.currentTarget.value)) {
              onSubmitShortcut?.(event.currentTarget.value);
            }
          }
        }}
        placeholder={placeholder}
      />
    </div>
  );
}

type CurrentActionKind = "plan" | "question";
type CurrentActionQuestion = PendingElicitation["questions"][number];
type PlanActionSubmission = { action: "accept" } | { action: "revise"; instructions: string } | { action: "keep_plan" };

export function currentActionKey(plan: MessageBlock | null | undefined, pending: PendingElicitation | null | undefined): string | null {
  if (plan) {
    return `plan:${plan.turn_id ?? "turn"}:${plan.item_id ?? plan.call_id ?? plan.id}`;
  }
  if (pending) {
    return `question:${pending.turn_id ?? "turn"}:${pending.item_id ?? pending.questions[0]?.id ?? "request"}`;
  }
  return null;
}

export function shouldShowCurrentActionCard(actionKey: string | null | undefined, hiddenActionKey: string | null | undefined): boolean {
  return Boolean(actionKey && actionKey !== hiddenActionKey);
}

export function selectionFromDigitKey(key: string, total: number): number | null {
  if (!/^[1-9]$/.test(key)) return null;
  const index = Number(key) - 1;
  return index >= 0 && index < total ? index : null;
}

export function moveActionSelection(current: number, total: number, delta: number): number {
  if (total <= 0) return 0;
  return (current + delta + total) % total;
}

export function currentPlanActionOptions(): { label: string; description: string }[] {
  return [
    { label: "接受计划", description: "按聊天记录里的 Proposed Plan 继续执行" },
    { label: "修改计划", description: "补充修改要求后重新生成计划" },
    { label: "保持计划模式", description: "不提交回复，继续让本线程使用 Plan Mode" }
  ];
}

export function planActionSubmission(selected: number, revision: string): PlanActionSubmission | null {
  if (selected === 0) return { action: "accept" };
  if (selected === 1 && revision.trim()) return { action: "revise", instructions: revision.trim() };
  if (selected === 2) return { action: "keep_plan" };
  return null;
}

export function questionAnswersReady(questions: CurrentActionQuestion[], answers: Record<string, string | string[] | undefined>): boolean {
  return questions.every((question) => {
    const value = answers[question.id];
    if (Array.isArray(value)) return value.some((item) => item.trim().length > 0);
    return typeof value === "string" && value.trim().length > 0;
  });
}

function combinedQuestionAnswers(
  questions: CurrentActionQuestion[],
  answers: Record<string, string | string[] | undefined>,
  notes: Record<string, string>
): Record<string, string[]> {
  return Object.fromEntries(questions.map((question) => {
    const answer = answers[question.id];
    const selected = Array.isArray(answer) ? answer : answer ? [answer] : [];
    const note = notes[question.id]?.trim();
    return [question.id, note ? [...selected, note] : selected];
  }));
}

export function questionAnswerPayload(questions: CurrentActionQuestion[], answers: Record<string, string | string[] | undefined>): Record<string, string[]> {
  return Object.fromEntries(questions.map((question) => {
    const value = answers[question.id];
    return [question.id, Array.isArray(value) ? value : value ? [value] : []];
  }));
}

export function renderCurrentActionCardSnapshot(input: {
  kind: CurrentActionKind;
  questions?: CurrentActionQuestion[];
}): { buttons: string[]; supplementalInput: boolean } {
  if (input.kind === "plan") {
    return {
      buttons: currentPlanActionOptions().map((option) => option.label),
      supplementalInput: false
    };
  }
  return {
    buttons: (input.questions ?? []).flatMap((question) => question.options.map((option) => option.label)),
    supplementalInput: true
  };
}

export function hiddenThreadDeleteStats(plan: HiddenThreadDeletePlan | null, status?: Pick<SystemStatus, "hidden_thread_count" | "state_db_integrity">): { hidden: number; visible: number; sourceCounts: string; integrity: string } {
  const hidden = plan?.hidden_threads ?? status?.hidden_thread_count ?? 0;
  return {
    hidden,
    visible: plan?.visible_threads ?? 0,
    sourceCounts: sourceCountsText(plan?.hidden_source_counts),
    integrity: plan?.integrity ?? status?.state_db_integrity ?? "未知"
  };
}

export function archivePlanAfterExecute(current: ArchiveDeletePlan | null, result: Pick<ArchiveDeleteResult, "after_total_threads" | "after_active_threads" | "after_archived_threads" | "after_integrity">): ArchiveDeletePlan | null {
  if (!current) return current;
  return {
    ...current,
    total_threads: result.after_total_threads,
    active_threads: result.after_active_threads,
    archived_threads: result.after_archived_threads,
    archived_ids: [],
    integrity: result.after_integrity
  };
}

function cleanupStageLabel(input: { hasPlan: boolean; dryRunPending: boolean; armed: boolean; executePending: boolean; executableCount: number }): { label: string; tone?: "success" | "warning" | "danger" } {
  if (input.executePending) return { label: "执行中", tone: "warning" };
  if (input.armed) return { label: "等待确认", tone: "danger" };
  if (input.dryRunPending) return { label: "扫描中", tone: "warning" };
  if (!input.hasPlan) return { label: "待 dry-run" };
  if (input.executableCount > 0) return { label: "可清理", tone: "warning" };
  return { label: "无可清理", tone: "success" };
}

function hiddenRolloutDeleteResultText(result?: Pick<HiddenThreadDeleteResult, "deleted_rollout_files"> | null): string {
  if (!result) return "等待执行";
  return String(result.deleted_rollout_files ?? 0);
}

function currentActionKindFromBlocks(
  blocks: MessageBlock[],
  plan: MessageBlock | null | undefined,
  pending: PendingElicitation | null | undefined
): CurrentActionKind | null {
  if (!plan && !pending) return null;
  if (plan && !pending) return "plan";
  if (!plan && pending) return "question";
  const planIndex = blocks.findIndex((block) => isActionablePlanBlock(block, plan));
  const questionIndex = blocks.findIndex((block) => isActionableQuestionBlock(block, pending));
  if (planIndex === -1 && questionIndex === -1) return plan ? "plan" : "question";
  if (questionIndex === -1) return "plan";
  if (planIndex === -1) return "question";
  return questionIndex >= planIndex ? "question" : "plan";
}

function Conversation({ threadId, detail, slot, messageStore, csrfToken, onSelect, onPanelSelect, nextThreadAfterArchive, capabilities }: {
  threadId: string;
  detail: ThreadDetail;
  slot: ThreadMessageSlot;
  messageStore: ThreadMessageStoreController;
  csrfToken?: string | null;
  onSelect: (id: SelectedThread) => void;
  onPanelSelect: (view: View) => void;
  nextThreadAfterArchive: string | null;
  capabilities: RuntimeCapabilityMatrix;
}) {
  const threadCache = useThreadCacheActions();
  const messageStreamRef = useRef<HTMLDivElement | null>(null);
  const messageEndRef = useRef<HTMLDivElement | null>(null);
  const composerTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const shouldFollowMessagesRef = useRef(true);
  const previousThreadIdRef = useRef(threadId);
  const [explicitBottomFollowRevision, setExplicitBottomFollowRevision] = useState(0);
  const [draft, setDraft] = useState("");
  const runOptions = useCodexRunOptions();
  const pluginsQuery = usePluginsQuery();
  const [runConfig, setRunConfig] = useState<RunConfig>(() => makeRunConfig(undefined, detail.summary));
  const [renameValue, setRenameValue] = useState(detail.summary.title);
  const [renameDirty, setRenameDirty] = useState(false);
  const [toolsOpen, setToolsOpen] = useState(false);
  const updateMessageFollowState = useCallback(() => {
    shouldFollowMessagesRef.current = messageStreamRef.current
      ? shouldAutoFollowMessageStream(messageStreamRef.current)
      : true;
  }, []);
  const followNextMessageUpdate = useCallback(() => {
    shouldFollowMessagesRef.current = true;
    setExplicitBottomFollowRevision((revision) => revision + 1);
  }, []);
  const attachComposerTextarea = useCallback((node: HTMLTextAreaElement | null) => {
    composerTextareaRef.current = node;
  }, []);
  const setActiveFeedback = useCallback((message: string | null) => {
    messageStore.setFeedback(threadId, message);
  }, [messageStore, threadId]);
  const attachments = useComposerAttachments(csrfToken, setActiveFeedback);
  const fallbackBlockState = initialMessageBlockState(detail);
  const summary = slot.summary ?? detail.summary;
  const blocks = slot.blocks.length ? slot.blocks : fallbackBlockState.blocks;
  const messageBlockState: MessageBlockState = {
    blocks,
    totalBlocks: slot.totalBlocks || fallbackBlockState.totalBlocks,
    hasMoreBlocks: slot.hasMoreBlocks || fallbackBlockState.hasMoreBlocks,
    beforeCursor: slot.beforeCursor ?? fallbackBlockState.beforeCursor,
    visibleUpdateRevision: slot.visibleUpdateRevision,
    bottomFollowRevision: slot.bottomFollowRevision || fallbackBlockState.bottomFollowRevision
  };
  const lastResult = slot.lastResult;
  const feedback = slot.feedback;
  const showAllHistory = slot.showAllHistory;
  const hiddenActionKey = slot.hiddenActionKey;
  const inspectorActions = threadInspectorActionState(capabilities);

  useEffect(() => {
    const defaults = makeRunConfig(runOptions.config, detail.summary);
    setRunConfig((current) => mergeRunConfigFromDefaults(current, defaults));
  }, [detail.summary, runOptions.config]);

  useEffect(() => {
    const sameThread = previousThreadIdRef.current === threadId;
    if (!sameThread) shouldFollowMessagesRef.current = true;
    setRenameValue((current) => {
      return nextRenameDraftValue({
        previousThreadId: previousThreadIdRef.current,
        threadId,
        currentDraft: current,
        incomingTitle: detail.summary.title,
        dirty: renameDirty
      });
    });
    if (!sameThread) {
      setRenameDirty(false);
      setDraft("");
      attachments.clearUploads();
      messageStore.setFeedback(threadId, null);
    }
    previousThreadIdRef.current = threadId;
  }, [detail.summary.title, messageStore, renameDirty, threadId]);

  useThreadRealtimeSubscription({
    threadId,
    messageStore,
    threadCache,
    applyThreadTitleOverride,
    onBeforeActiveBlocks: updateMessageFollowState
  });

  const pending = useMemo(() => currentPendingElicitation(summary.pending_elicitation, summary.active_turn_id) ?? pendingFromBlocks(blocks, summary.status, summary.active_turn_id), [summary.pending_elicitation, summary.status, summary.active_turn_id, blocks]);
  const planBlock = useMemo(() => latestActionBlock(blocks, summary.status, summary.active_turn_id, isPlanBlock), [blocks, summary.status, summary.active_turn_id]);
  const approvalBlock = useMemo(() => latestActionBlock(blocks, summary.status, summary.active_turn_id, isApprovalBlock), [blocks, summary.status, summary.active_turn_id]);
  const currentActionKind = useMemo(() => currentActionKindFromBlocks(blocks, planBlock, pending), [blocks, planBlock, pending]);
  const currentActionPlan = currentActionKind === "plan" ? planBlock : null;
  const currentActionPending = currentActionKind === "question" ? pending : null;
  const currentActionId = currentActionKey(currentActionPlan, currentActionPending);
  const showCurrentActionCard = shouldShowCurrentActionCard(currentActionId, hiddenActionKey);
  const conversationSourceBlocks = useMemo(() => blocksWithCurrentPending(blocks, pending), [blocks, pending]);
  const visibleConversationBlocks = useMemo(() => (
    prioritizeCurrentActionBlocks(
      visibleConversationBlocksForHistory(conversationSourceBlocks, showAllHistory, planBlock, pending),
      planBlock,
      pending
    )
  ), [conversationSourceBlocks, showAllHistory, planBlock, pending]);

  useEffect(() => {
    if (!shouldFollowMessagesRef.current) return;
    requestAnimationFrame(() => {
      messageEndRef.current?.scrollIntoView({ block: "end" });
    });
  }, [messageBlockState.bottomFollowRevision, threadId, explicitBottomFollowRevision]);

  useEffect(() => {
    if (!currentActionId) messageStore.setHiddenActionKey(threadId, null);
  }, [currentActionId, messageStore, threadId]);

  const running = isThreadRunning(summary, blocks, lastResult);
  const canStop = running || Boolean(summary.active_turn_id || summary.active_job_id || lastResult?.turn_id || lastResult?.job_id);
  const actionMode = composerActionMode(running, draft, canStop, attachments.readyUploads.length);
  const followUps = useFollowUpsQuery(summary.id, running);
  const followUpItems = followUps.data?.items ?? [];
  const payloadRunConfig = useMemo(
    () => runConfigWithSupportedServiceTier(runConfig, runOptions.models),
    [runConfig, runOptions.models]
  );
  const loadEarlierMutation = useThreadBlockPageMutation({
    onBeforeLoad: (requestThreadId) => {
      const beforeHeight = messageStreamRef.current?.scrollHeight ?? 0;
      messageStore.setLoadingEarlier(requestThreadId, true);
      return beforeHeight;
    },
    onSuccess: ({ threadId: loadedThreadId, cursor, page, beforeHeight }) => {
      messageStore.applyBlockPage(loadedThreadId, page, cursor);
      requestAnimationFrame(() => {
        if (!messageStore.isActive(loadedThreadId)) return;
        const stream = messageStreamRef.current;
        if (!stream) return;
        stream.scrollTop += Math.max(0, stream.scrollHeight - beforeHeight);
      });
    },
    onError: (err, variables) => {
      const failedThreadId = variables?.threadId ?? summary.id;
      messageStore.setLoadingEarlier(failedThreadId, false, err.message);
      messageStore.setFeedback(failedThreadId, err.message);
    }
  });

  const threadActions = useThreadActionMutations({
    csrfToken,
    capabilities,
    buildPayload,
    onSendSuccess: ({ threadId: resultThreadId, result }) => {
      messageStore.setLastResult(resultThreadId, result);
      if (result.job_id || result.turn_id) {
        messageStore.patchSummary(resultThreadId, (current) => ({
          ...current,
          status: "Running",
          active_turn_id: result.turn_id ?? current.active_turn_id,
          active_job_id: result.job_id ?? current.active_job_id
        }));
      }
      if (messageStore.isActive(resultThreadId)) {
        setDraft("");
        attachments.clearUploads();
        setRunConfig((current) => runConfigAfterSuccessfulSend(current));
      }
      messageStore.setFeedback(resultThreadId, actionMessage(result));
      threadCache.invalidateJobs();
      threadCache.invalidateThreads();
      threadCache.invalidateThread(resultThreadId);
    },
    onStopSuccess: ({ threadId: stoppedThreadId }) => {
      messageStore.setFeedback(stoppedThreadId, "停止请求已发送");
      threadCache.invalidateThreads();
      threadCache.invalidateThread(stoppedThreadId);
    },
    onSteerSuccess: ({ threadId: resultThreadId, result }) => {
      messageStore.setLastResult(resultThreadId, result);
      if (messageStore.isActive(resultThreadId)) {
        setDraft("");
        attachments.clearUploads();
        setRunConfig((current) => runConfigAfterSuccessfulSend(current));
      }
      messageStore.setFeedback(resultThreadId, actionMessage(result));
      threadCache.invalidateFollowUps(resultThreadId);
      threadCache.invalidateThreads();
      threadCache.invalidateThread(resultThreadId);
    },
    onFollowUpCancelSuccess: ({ threadId: cancelledThreadId }) => {
      messageStore.setFeedback(cancelledThreadId, "跟进已取消");
      threadCache.invalidateFollowUps(cancelledThreadId);
    },
    onArchiveMutate: async (variables) => {
      await threadCache.cancelThreadsAndThread(variables.threadId);
      const wasArchived = variables.status === "Archived";
      const snapshot = wasArchived
        ? threadCache.applyOptimisticThreadRestore(variables.threadId)
        : threadCache.applyOptimisticThreadArchive(messageStore, variables.threadId);
      if (!wasArchived) {
        onSelect(nextThreadAfterArchive);
      }
      return { snapshot, wasArchived };
    },
    onArchiveSuccess: ({ threadId: archivedThreadId, wasArchived }) => {
      messageStore.setFeedback(archivedThreadId, wasArchived ? "恢复请求已提交" : "归档请求已提交");
    },
    onArchiveError: (err, variables, context) => {
      const archiveContext = context as { snapshot?: ThreadCacheSnapshot; wasArchived?: boolean } | undefined;
      if (archiveContext?.wasArchived) {
        threadCache.rollbackOptimisticThreadRestore(archiveContext.snapshot);
      } else {
        threadCache.rollbackOptimisticThreadArchive(archiveContext?.snapshot);
        if (variables?.threadId) {
          onSelect(variables.threadId);
        }
      }
      messageStore.setFeedback(variables?.threadId ?? summary.id, err.message);
    },
    onArchiveSettled: (variables) => {
      threadCache.invalidateThreads();
      if (variables?.threadId) {
        threadCache.invalidateThread(variables.threadId);
      }
    },
    onRenameMutate: async (variables) => {
      const title = variables.title.trim();
      await threadCache.cancelThreadsAndThread(variables.threadId);
      const snapshot = threadCache.applyOptimisticThreadTitle(variables.threadId, title);
      if (title) {
        setLocalThreadTitleOverride(variables.threadId, title);
        setRenameValue(title);
        setRenameDirty(false);
        messageStore.patchSummary(variables.threadId, { title });
      }
      return { snapshot };
    },
    onRenameSuccess: ({ threadId: renamedThreadId, title }) => {
      messageStore.setFeedback(renamedThreadId, "线程名称已更新");
      if (title) {
        setLocalThreadTitleOverride(renamedThreadId, title);
        threadCache.applyOptimisticThreadTitle(renamedThreadId, title);
      }
    },
    onRenameError: (err, variables, context) => {
      const renameContext = context as { snapshot?: ThreadCacheSnapshot } | undefined;
      if (variables?.threadId) {
        clearLocalThreadTitleOverride(variables.threadId);
      }
      threadCache.rollbackOptimisticThreadTitle(renameContext?.snapshot);
      const restoredTitle = threadCache.cachedThreadSummary(variables?.threadId ?? summary.id)?.title ?? detail.summary.title;
      if (variables?.threadId === summary.id && restoredTitle) {
        setRenameValue(restoredTitle);
        setRenameDirty(false);
        messageStore.patchSummary(variables.threadId, { title: restoredTitle });
      }
      messageStore.setFeedback(variables?.threadId ?? summary.id, err.message);
    },
    onRenameSettled: (variables) => {
      threadCache.invalidateThreads();
      if (variables?.threadId) {
        threadCache.invalidateThread(variables.threadId);
      }
    },
    onForkSuccess: ({ threadId: forkedThreadId, result }) => {
      messageStore.setLastResult(forkedThreadId, result);
      messageStore.setFeedback(forkedThreadId, actionMessage(result));
      if (result.thread_id) onSelect(result.thread_id);
      threadCache.invalidateThreads();
    },
    onBridgeActionSuccess: ({ threadId: actionThreadId, result }) => {
      messageStore.setLastResult(actionThreadId, result);
      messageStore.setFeedback(actionThreadId, actionMessage(result));
      threadCache.invalidateThreads();
      threadCache.invalidateThread(actionThreadId);
    },
    onActionError: (err, variables) => messageStore.setFeedback(variables?.threadId ?? summary.id, err.message)
  });

  const sendMutation = threadActions.send;
  const stopMutation = threadActions.stop;
  const steerMutation = threadActions.steer;
  const followUpCancelMutation = threadActions.followUpCancel;
  const archiveMutation = threadActions.archive;
  const renameMutation = threadActions.rename;
  const forkMutation = threadActions.fork;
  const answerMutation = threadActions.answer;
  const planAcceptMutation = threadActions.planAccept;
  const planReviseMutation = threadActions.planRevise;
  const approvalMutation = threadActions.approval;
  const executeSlashCommand = useCallback((command: string) => {
    const action = slashCommandAction(command, Boolean(threadId), capabilities);
    switch (action.kind) {
      case "toggle_plan_mode":
        setDraft("");
        setRunConfig((current) => ({
          ...current,
          collaborationMode: current.collaborationMode === "plan" ? "" : "plan"
        }));
        messageStore.setFeedback(threadId, action.message ?? "Plan Mode 已切换");
        break;
      case "open_plugins":
        setDraft("");
        onPanelSelect("claude");
        messageStore.setFeedback(threadId, action.message ?? "已打开插件/Provider 面板");
        break;
      case "open_status":
        setDraft("");
        onPanelSelect("codex");
        messageStore.setFeedback(threadId, action.message ?? "已打开线程状态");
        break;
      case "open_new_thread":
        setDraft("");
        onSelect("__new");
        break;
      case "open_resume":
        setDraft("");
        onPanelSelect("codex");
        messageStore.setFeedback(threadId, "请在线程列表选择要恢复的会话");
        break;
      case "open_thread_settings":
        setDraft("");
        messageStore.setFeedback(threadId, "线程设置已在右侧面板显示");
        break;
      case "archive_thread":
        setDraft("");
        if (!inspectorActions.showArchive) {
          messageStore.setFeedback(threadId, "当前运行时不支持归档操作");
          break;
        }
        archiveMutation.mutate({ threadId: summary.id, status: summary.status });
        break;
      case "fork_thread":
        setDraft("");
        if (!inspectorActions.showFork) {
          messageStore.setFeedback(threadId, "macOS App 当前不支持 Fork 操作");
          break;
        }
        forkMutation.mutate({ threadId: summary.id });
        break;
      case "stop_thread":
        setDraft("");
        stopMutation.mutate({
          threadId: summary.id,
          turnId: lastResult?.turn_id ?? summary.active_turn_id,
          jobId: lastResult?.job_id ?? summary.active_job_id
        });
        break;
      case "copy_latest":
        setDraft("");
        {
          const text = latestAssistantCopyText(blocks);
          if (text) {
            navigator.clipboard?.writeText(text);
            messageStore.setFeedback(threadId, "已复制最新回复");
          } else {
            messageStore.setFeedback(threadId, "没有可复制的最新回复");
          }
        }
        break;
      case "toggle_fast":
        setDraft("");
        if (!modelSupportsServiceTier(runOptions.models, runConfig.model, "priority")) {
          messageStore.setFeedback(threadId, "当前模型不支持 Fast service tier");
          break;
        }
        {
          const next = runConfig.serviceTier === "priority" ? "" : "priority";
          setRunConfig({ ...runConfig, serviceTier: next });
          messageStore.setFeedback(threadId, next === "priority" ? "Fast 已开启" : "Fast 已关闭");
        }
        break;
      case "insert_template":
        setDraft(`${action.command} `);
        messageStore.setFeedback(threadId, action.message);
        break;
      case "focus_control":
      case "requires_thread":
      case "unavailable":
      case "unknown":
      default:
        setDraft("");
        messageStore.setFeedback(threadId, action.message ?? "已执行");
        break;
    }
  }, [archiveMutation, blocks, csrfToken, forkMutation, inspectorActions.showArchive, inspectorActions.showFork, lastResult?.job_id, lastResult?.turn_id, messageStore, onPanelSelect, onSelect, runConfig, runOptions.models, stopMutation, summary.id, summary.status, summary.active_job_id, summary.active_turn_id, threadId]);

  const loadEarlierPending = slot.loadingEarlier;
  const sendPending = sendMutation.isPending && sendMutation.variables?.threadId === summary.id;
  const stopPending = stopMutation.isPending && stopMutation.variables?.threadId === summary.id;
  const steerPending = steerMutation.isPending && steerMutation.variables?.threadId === summary.id;
  const followUpCancelPending = followUpCancelMutation.isPending && followUpCancelMutation.variables?.threadId === summary.id;
  const forkPending = forkMutation.isPending && forkMutation.variables?.threadId === summary.id;
  const renamePending = renameMutation.isPending && renameMutation.variables?.threadId === summary.id;
  const archivePending = archiveMutation.isPending && archiveMutation.variables?.threadId === summary.id;
  const answerPending = answerMutation.isPending && answerMutation.variables?.threadId === summary.id;
  const planAcceptPending = planAcceptMutation.isPending && planAcceptMutation.variables?.threadId === summary.id;
  const planRevisePending = planReviseMutation.isPending && planReviseMutation.variables?.threadId === summary.id;
  const approvalPending = approvalMutation.isPending && approvalMutation.variables?.threadId === summary.id;

  const submitComposer = useCallback((domValue?: string | null) => {
    if (attachments.uploadInProgress) return;
    const currentDraft = composerSubmitDraftValue(draft, domValue ?? composerTextareaRef.current?.value);
    if (currentDraft !== draft) setDraft(currentDraft);
    const exactSlash = slashCommandForComposerSubmit(currentDraft, capabilities);
    if (exactSlash) {
      executeSlashCommand(exactSlash);
      return;
    }
    if (actionMode === "send" && !sendPending) {
      followNextMessageUpdate();
      sendMutation.mutate({
        threadId: summary.id,
        message: currentDraft,
        config: payloadRunConfig,
        uploads: [...attachments.readyUploads]
      });
    } else if (actionMode === "followup" && !steerPending) {
      followNextMessageUpdate();
      steerMutation.mutate({
        threadId: summary.id,
        message: currentDraft,
        config: payloadRunConfig,
        uploads: [...attachments.readyUploads]
      });
    } else if (actionMode === "stop" && !stopPending) {
      stopMutation.mutate({
        threadId: summary.id,
        turnId: lastResult?.turn_id ?? summary.active_turn_id,
        jobId: lastResult?.job_id ?? summary.active_job_id
      });
    }
  }, [actionMode, attachments, capabilities, draft, executeSlashCommand, followNextMessageUpdate, lastResult?.job_id, lastResult?.turn_id, payloadRunConfig, sendMutation, sendPending, steerMutation, steerPending, stopMutation, stopPending, summary.active_job_id, summary.active_turn_id, summary.id]);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    submitComposer();
  };
  const conversationTitle = conversationTitleText(summary);
  const actionBusy = sendPending || stopPending || steerPending || attachments.uploadInProgress;
  const actionLabel = composerActionLabel(actionMode);
  const actionTitle = composerActionTitle(actionMode);
  const inspectorPanels = (
    <ThreadInspectorPanels
      summary={summary}
      csrfToken={csrfToken}
      renameValue={renameValue}
      setRenameValue={setRenameValue}
      setRenameDirty={setRenameDirty}
      renamePending={renamePending}
      onRename={() => renameMutation.mutate({ threadId: summary.id, title: renameValue })}
      archivePending={archivePending}
      onArchive={() => archiveMutation.mutate({ threadId: summary.id, status: summary.status })}
      forkPending={forkPending}
      onFork={() => forkMutation.mutate({ threadId: summary.id })}
      showFork={inspectorActions.showFork}
      showArchive={inspectorActions.showArchive}
      onFeedback={setActiveFeedback}
    />
  );

  return (
    <div className="conversation-shell">
      <div className="conversation-main">
        <header className="conversation-header">
          <div className="conversation-title-copy">
            <h2 className="conversation-title" title={conversationTitle}>{conversationTitle}</h2>
          </div>
          <div className="header-actions">
            <StatusChip status={summary.status} />
            <button className="secondary-button thread-tools-button" onClick={() => setToolsOpen(true)} type="button">
              <SlidersHorizontal size={17} />线程工具
            </button>
            <button
              className="icon-button"
              disabled={!canStop || stopPending}
              onClick={() => stopMutation.mutate({
                threadId: summary.id,
                turnId: lastResult?.turn_id ?? summary.active_turn_id,
                jobId: lastResult?.job_id ?? summary.active_job_id
              })}
              title="停止当前 turn"
            >
              <Square size={17} />
            </button>
          </div>
        </header>

        {toolsOpen && (
          <div className="thread-tools-backdrop" onClick={() => setToolsOpen(false)}>
            <aside className="thread-tools-drawer" onClick={(event) => event.stopPropagation()}>
              <div className="drawer-title-row">
                <strong>线程工具</strong>
                <button className="icon-button compact" onClick={() => setToolsOpen(false)} title="关闭线程工具" type="button"><X size={16} /></button>
              </div>
              {inspectorPanels}
            </aside>
          </div>
        )}

        {summary.status === "ReplyNeeded" && (
          <div className="reply-banner">
            <TriangleAlert size={18} />
            <span>{pending ? "Plan Mode 正在等待选择。" : "Plan Mode 正在等待确认。"}</span>
          </div>
        )}
        {feedback && <div className="feedback-banner">{feedback}</div>}

        {approvalBlock && (
          <div className="action-stack">
            {inspectorActions.approvalMode === "unsupported" ? (
              <UnsupportedApprovalCard block={approvalBlock} />
            ) : (
              <ApprovalCard
                key={`approval-${approvalBlock.id}`}
                block={approvalBlock}
                onDecision={(decision) => {
                  followNextMessageUpdate();
                  approvalMutation.mutate({ threadId: summary.id, block: approvalBlock, decision });
                }}
                pending={approvalPending}
              />
            )}
          </div>
        )}

        <div className="message-stream" ref={messageStreamRef} onScroll={updateMessageFollowState}>
          {messageBlockState.hasMoreBlocks && (
            <button
              className="load-earlier-button"
              disabled={slot.loadingEarlier || !messageBlockState.beforeCursor}
              onClick={() => {
                if (!messageBlockState.beforeCursor) return;
                loadEarlierMutation.mutate({ threadId: summary.id, cursor: messageBlockState.beforeCursor });
              }}
              type="button"
            >
              {slot.loadingEarlier ? "正在加载..." : "加载更早消息"}
            </button>
          )}
          {visibleConversationBlocks.map((block) => (
            <MessageBlockView
              key={block.id}
              block={block}
              activePlan={isActionablePlanBlock(block, planBlock)}
              planPending={planAcceptPending || planRevisePending}
              activeQuestion={isActionableQuestionBlock(block, pending)}
              questionPending={answerPending}
              onShowHistory={() => messageStore.setHistoryExpanded(threadId, true)}
              historyExpanded={showAllHistory}
            />
          ))}
          {visibleConversationBlocks.length === 0 && !approvalBlock && !planBlock && !pending && <div className="muted-row">没有可展示的 rollout 消息。</div>}
          <div ref={messageEndRef} aria-hidden="true" />
        </div>

        {showCurrentActionCard && (currentActionPlan || currentActionPending) && (
          <CurrentActionCard
            plan={currentActionPlan}
            pending={currentActionPending}
            onAcceptPlan={(block) => {
              followNextMessageUpdate();
              planAcceptMutation.mutate({ threadId: summary.id, block });
            }}
            onRevisePlan={(block, instructions) => {
              followNextMessageUpdate();
              planReviseMutation.mutate({ threadId: summary.id, block, instructions });
            }}
            planPending={planAcceptPending || planRevisePending}
            onSubmitQuestion={(answers) => {
              followNextMessageUpdate();
              answerMutation.mutate({ threadId: summary.id, answers });
            }}
            questionPending={answerPending}
            onDismiss={() => messageStore.setHiddenActionKey(threadId, currentActionId)}
          />
        )}

        <form className="composer" onSubmit={submit}>
          <input
            ref={attachments.inputRef}
            className="visually-hidden"
            type="file"
            multiple
            onChange={attachments.onFileInputChange}
          />
          <SlashCommandTextarea
            inputRef={attachComposerTextarea}
            value={draft}
            onChange={setDraft}
            placeholder={summary.status === "ReplyNeeded" ? "输入选择编号、确认语句或补充要求" : "发送给 Codex"}
            hasThread
            plugins={pluginsQuery.data ?? []}
            pluginsUnavailable={pluginsQuery.isError}
            capabilities={capabilities}
            onSlashCommand={executeSlashCommand}
            onSubmitShortcut={submitComposer}
          />
          <ComposerAttachmentList
            uploads={attachments.uploads}
            removingUploadId={attachments.removingUploadId}
            onRemove={attachments.removeUpload}
          />
          <RunConfigControls
            config={runConfig}
            setConfig={setRunConfig}
            models={runOptions.models}
            profiles={runOptions.profiles}
            unavailable={runOptions.unavailable}
            onPickFiles={attachments.openPicker}
            uploadInProgress={attachments.uploadInProgress}
            threadStatus={summary.status}
            hasPendingPlan={Boolean(planBlock)}
            hasPendingQuestion={Boolean(pending)}
          />
          {followUpItems.length > 0 && (
            <FollowUpQueue
              items={followUpItems}
              onCancel={(item) => followUpCancelMutation.mutate({ threadId: summary.id, followUpId: item.id })}
              cancelling={followUpCancelPending}
            />
          )}
          <div className="composer-actions">
            <span>{feedback || (lastResult ? actionMessage(lastResult) : "")}</span>
            <button className="primary-button composer-action-button" disabled={actionMode === "disabled" || actionBusy} title={actionTitle}>
              {actionMode === "stop" ? <Square size={17} /> : actionMode === "followup" ? <MessageSquare size={17} /> : <Send size={17} />}
              {actionLabel}
            </button>
          </div>
        </form>
      </div>

      <aside className="conversation-inspector">
        {inspectorPanels}
      </aside>
    </div>
  );
}

function ThreadInspectorPanels({
  summary,
  csrfToken,
  renameValue,
  setRenameValue,
  setRenameDirty,
  renamePending,
  onRename,
  archivePending,
  onArchive,
  forkPending,
  onFork,
  showFork,
  showArchive,
  onFeedback
}: {
  summary: ThreadSummary;
  csrfToken?: string | null;
  renameValue: string;
  setRenameValue: (value: string) => void;
  setRenameDirty: (dirty: boolean) => void;
  renamePending: boolean;
  onRename: () => void;
  archivePending: boolean;
  onArchive: () => void;
  forkPending: boolean;
  onFork: () => void;
  showFork: boolean;
  showArchive: boolean;
  onFeedback: (message: string | null) => void;
}) {
  const copyText = useCallback((text: string | null, message: string) => {
    if (!text) return;
    navigator.clipboard?.writeText(text);
    onFeedback(message);
  }, [onFeedback]);
  const copyId = threadCopyId(summary.id);
  const rolloutPath = threadRolloutPath(summary.rollout_path);
  const resumeCommand = threadResumeCommand(summary.id);

  return (
    <>
      <Panel title="名称与归档" icon={<SlidersHorizontal size={18} />}>
        <label className="field-label">线程标题<input value={renameValue} onChange={(event) => {
          setRenameDirty(true);
          setRenameValue(event.target.value);
        }} /></label>
        <div className="button-row">
          <button className="secondary-button" onClick={onRename} disabled={!renameValue.trim() || renamePending}><Edit3 size={17} />重命名</button>
          <button className={summary.status === "Archived" ? "secondary-button" : "danger-button soft"} onClick={onArchive} disabled={archivePending || !showArchive}>
            {summary.status === "Archived" ? <Undo2 size={17} /> : <Archive size={17} />}
            {summary.status === "Archived" ? "恢复" : "归档"}
          </button>
        </div>
        {showFork && (
          <button className="secondary-button full-width-action" onClick={onFork} disabled={forkPending}>
            <GitFork size={17} />Fork
          </button>
        )}
      </Panel>

      <ThreadGoalPanel threadId={summary.id} csrfToken={csrfToken} onFeedback={onFeedback} />

      <Panel title="复制与路径" icon={<Files size={18} />}>
        <Metric label="线程 ID" value={copyId || "无"} wide />
        <Metric label="会话文件" value={rolloutPath || "无会话文件"} wide />
        <div className="copy-row">
          <button className="secondary-button" onClick={() => copyText(copyId, "已复制线程 ID")} disabled={!copyId}>
            <Copy size={17} />复制 ID
          </button>
          <button className="secondary-button" onClick={() => copyText(rolloutPath, "已复制文件路径")} disabled={!rolloutPath}>
            <Copy size={17} />复制文件路径
          </button>
          <button className="secondary-button" onClick={() => copyText(resumeCommand, "已复制 resume 命令")} disabled={!resumeCommand}>
            <TerminalSquare size={17} />复制 codex resume+ID
          </button>
        </div>
      </Panel>
    </>
  );
}

function ThreadGoalPanel({ threadId, csrfToken, onFeedback }: {
  threadId: string;
  csrfToken?: string | null;
  onFeedback: (message: string | null) => void;
}) {
  const goal = useThreadGoalQuery(threadId);
  const [objective, setObjective] = useState("");
  const [tokenBudget, setTokenBudget] = useState("");
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const currentGoal = goal.data;

  useEffect(() => {
    if (!currentGoal || dirty) return;
    setObjective(currentGoal.objective ?? "");
    setTokenBudget(currentGoal.token_budget === null || currentGoal.token_budget === undefined ? "" : String(currentGoal.token_budget));
  }, [currentGoal, dirty]);

  useEffect(() => {
    setDirty(false);
    setError(null);
  }, [threadId]);

  const afterGoalSuccess = useCallback((next: CodexGoal, message: string) => {
    setDirty(false);
    setObjective(next.objective ?? "");
    setTokenBudget(next.token_budget === null || next.token_budget === undefined ? "" : String(next.token_budget));
    setError(null);
    onFeedback(message);
  }, [onFeedback]);

  const onGoalError = useCallback((err: Error) => {
    setError(err.message);
    onFeedback(err.message);
  }, [onFeedback]);

  const goalActions = useThreadGoalActions({
    threadId,
    csrfToken,
    saveInput: () => goalSaveInput(objective, tokenBudget),
    onSuccess: afterGoalSuccess,
    onError: onGoalError
  });
  const saveGoalMutation = goalActions.save;
  const clearGoalMutation = goalActions.clear;
  const pauseGoalMutation = goalActions.pause;
  const resumeGoalMutation = goalActions.resume;

  const busy = saveGoalMutation.isPending || clearGoalMutation.isPending || pauseGoalMutation.isPending || resumeGoalMutation.isPending;
  const controls = goalControlState(currentGoal, { busy, objective, tokenBudget });
  const unavailable = currentGoal?.available === false;

  return (
    <Panel title="Goal" icon={<ClipboardCheck size={18} />}>
      <div className="settings-meta-grid">
        <Metric label="状态" value={goalStatusLabel(currentGoal, goal.isLoading)} tone={goalStatusTone(currentGoal)} />
        <Metric label="预算" value={currentGoal?.token_budget === null || currentGoal?.token_budget === undefined ? "无" : String(currentGoal.token_budget)} />
        {currentGoal?.completed_at ? <Metric label="完成时间" value={formatGoalTimestamp(currentGoal.completed_at)} /> : null}
        {currentGoal?.blocked_reason ? <Metric label="阻塞原因" value={currentGoal.blocked_reason} tone="danger" /> : null}
      </div>
      <label className="field-label">目标<input value={objective} onChange={(event) => {
        setDirty(true);
        setObjective(event.target.value);
      }} placeholder={goal.isLoading ? "正在读取 Goal" : "输入当前线程目标"} /></label>
      <label className="field-label">Token budget<input type="number" min={1} value={tokenBudget} onChange={(event) => {
        setDirty(true);
        setTokenBudget(event.target.value);
      }} placeholder="可选" /></label>
      <div className="button-row">
        <button className="primary-button" disabled={controls.saveDisabled || unavailable} onClick={() => saveGoalMutation.mutate()}><CheckCircle2 size={17} />保存</button>
        <button className="secondary-button" disabled={controls.clearDisabled || unavailable} onClick={() => clearGoalMutation.mutate()}><Trash2 size={17} />清除</button>
        <button className="secondary-button" disabled={controls.pauseDisabled || unavailable} onClick={() => pauseGoalMutation.mutate()}><Square size={17} />暂停</button>
        <button className="secondary-button" disabled={controls.resumeDisabled || unavailable} onClick={() => resumeGoalMutation.mutate()}><Play size={17} />恢复</button>
      </div>
      {error && <div className="form-error">{error}</div>}
      {unavailable && <div className="muted-row">Goal 接口未接入</div>}
    </Panel>
  );
}

function goalSaveInput(objective: string, tokenBudget: string): CodexGoalSaveInput {
  return {
    objective: objective.trim(),
    token_budget: tokenBudget.trim() ? Number.isFinite(Number(tokenBudget.trim())) && Number(tokenBudget.trim()) > 0 ? Math.floor(Number(tokenBudget.trim())) : null : null
  };
}

function RunConfigControls({ config, setConfig, models, unavailable, onPickFiles, uploadInProgress = false, threadStatus, hasPendingPlan = false, hasPendingQuestion = false }: {
  config: RunConfig;
  setConfig: (config: RunConfig) => void;
  models: CodexModel[];
  profiles: PermissionProfile[];
  unavailable: { models?: boolean; profiles?: boolean; config?: boolean };
  onPickFiles?: () => void;
  uploadInProgress?: boolean;
  threadStatus?: ThreadStatus | string;
  hasPendingPlan?: boolean;
  hasPendingQuestion?: boolean;
}) {
  const modelList = models.some((item) => item.id === config.model)
    ? models
    : config.model
      ? [{ id: config.model, label: config.model }, ...models]
      : models;
  const activePreset = permissionPresets.find((item) => item.id === config.permissionPreset) ?? permissionPresets[2];
  const supportsFast = modelSupportsServiceTier(modelList, config.model, "priority");
  const serviceTier = supportsFast ? config.serviceTier : "";
  const planButton = planModeButtonState(config.collaborationMode === "plan", threadStatus, hasPendingPlan, hasPendingQuestion);
  return (
    <div className="composer-config">
      <div className="composer-toolbar">
        <button
          type="button"
          className="composer-chip icon-only"
          title={uploadInProgress ? "附件上传中" : "上传本地文件"}
          onClick={onPickFiles}
          disabled={!onPickFiles || uploadInProgress}
        >
          <Plus size={15} />
        </button>
        {supportsFast && (
          <button
            type="button"
            className={serviceTier === "priority" ? "composer-chip active" : "composer-chip"}
            onClick={() => setConfig({ ...config, serviceTier: serviceTier === "priority" ? "" : "priority" })}
            title="使用 Codex priority service tier"
          >
            <RefreshCw size={15} />Fast
          </button>
        )}
        <button
          type="button"
          className={planButton.pressed ? "composer-chip active" : "composer-chip"}
          aria-pressed={planButton.pressed}
          title={planButton.statusText}
          onClick={() => setConfig({ ...config, collaborationMode: config.collaborationMode === "plan" ? "" : "plan" })}
        >
          <ClipboardCheck size={15} />{planButton.label}
        </button>
        <span className="composer-chip muted">{planButton.statusText}</span>
        <label className="permission-menu-trigger">
          <ShieldCheck size={15} />
          <select value={config.permissionPreset} onChange={(event) => setConfig(applyPermissionPreset(config, event.target.value as PermissionPresetId))}>
            {permissionPresets.map((preset) => <option key={preset.id} value={preset.id}>{preset.label}</option>)}
          </select>
        </label>
      </div>
      <div className="composer-grid main-config">
        <label>
          <span>模型</span>
          {modelList.length > 0 ? (
            <select value={config.model} onChange={(event) => {
              const model = event.target.value;
              setConfig({
                ...config,
                model,
                serviceTier: modelSupportsServiceTier(modelList, model, "priority") ? config.serviceTier : ""
              });
            }}>
              {modelList.map((item) => <option key={item.id} value={item.id}>{item.label ?? item.id}</option>)}
            </select>
          ) : (
            <input value={config.model} onChange={(event) => setConfig({ ...config, model: event.target.value })} placeholder={unavailable.models ? "模型接口不可用" : "model"} />
          )}
        </label>
        <label>
          <span>Reasoning</span>
          <select value={config.reasoning} onChange={(event) => setConfig({ ...config, reasoning: event.target.value })}>
            {reasoningOptions.map((value) => <option key={value || "default"} value={value}>{value || "default"}</option>)}
          </select>
        </label>
      </div>
      <div className="permission-summary">
        <div className="permission-summary-icon">{activePreset.icon}</div>
        <div>
          <strong>{activePreset.label}</strong>
          <span>{activePreset.description}</span>
        </div>
      </div>
      {unavailable.config && <div className="config-note">Codex 默认配置接口不可用，使用当前表单值发送。</div>}
    </div>
  );
}

function FollowUpQueue({ items, onCancel, cancelling }: { items: FollowUpQueueItem[]; onCancel: (item: FollowUpQueueItem) => void; cancelling: boolean }) {
  const visible = items.filter((item) => item.status !== "submitted" || item.submitted_at);
  if (!visible.length) return null;
  return (
    <div className="follow-up-queue">
      {visible.slice(0, 4).map((item) => (
        <div className="follow-up-item" key={item.id}>
          <div className="follow-up-copy">
            <span>{followUpStatusLabel(item.status)}</span>
            <strong>{followUpMessagePreview(item)}</strong>
          </div>
          {item.status === "pending" && (
            <button type="button" className="icon-button compact" disabled={cancelling} onClick={() => onCancel(item)} title="取消跟进">
              <X size={15} />
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

export function followUpStatusLabel(status?: string | null): string {
  if (status === "pending") return "待跟进";
  if (status === "submitting") return "提交中";
  if (status === "submitted") return "已提交";
  if (status === "cancelled") return "已取消";
  if (status === "error") return "失败";
  return status || "未知";
}

export function followUpMessagePreview(item: Pick<FollowUpQueueItem, "message" | "error" | "status">): string {
  const source = item.status === "error" && item.error ? item.error : item.message;
  const compact = source.replace(/\s+/g, " ").trim();
  if (compact.length <= 120) return compact || "空跟进";
  return `${compact.slice(0, 120)}...`;
}

function EmptyConversation({ loading, csrfToken, onCreated, capabilities }: {
  loading: boolean;
  csrfToken?: string | null;
  onCreated: (id: string) => void;
  capabilities: RuntimeCapabilityMatrix;
}) {
  const composerTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const [draft, setDraft] = useState("");
  const runOptions = useCodexRunOptions();
  const pluginsQuery = usePluginsQuery();
  const [runConfig, setRunConfig] = useState<RunConfig>(() => makeRunConfig());
  const [result, setResult] = useState<BridgeActionResult | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const attachments = useComposerAttachments(csrfToken, setFeedback);
  useEffect(() => {
    if (runOptions.config) {
      const defaults = makeRunConfig(runOptions.config);
      setRunConfig((current) => mergeRunConfigFromDefaults(current, defaults));
    }
  }, [runOptions.config]);
  const payloadRunConfig = useMemo(
    () => runConfigWithSupportedServiceTier(runConfig, runOptions.models),
    [runConfig, runOptions.models]
  );
  const mutation = useCreateThreadMutation({
    csrfToken,
    payload: (message) => buildPayload(message, payloadRunConfig, attachments.readyUploads),
    onSuccess: (next) => {
      setResult(next);
      setDraft("");
      attachments.clearUploads();
      setRunConfig((current) => runConfigAfterSuccessfulSend(current));
      if (next.thread_id) onCreated(next.thread_id);
    },
    onError: (err: Error) => setFeedback(err.message)
  });
  const executeSlashCommand = (command: string) => {
    const action = slashCommandAction(command, false, capabilities);
    setDraft("");
    if (action.kind === "toggle_plan_mode") {
      setRunConfig((current) => ({
        ...current,
        collaborationMode: current.collaborationMode === "plan" ? "" : "plan"
      }));
      setFeedback("Plan Mode 已切换");
      return;
    }
    if (action.kind === "open_new_thread") {
      setFeedback("已经在新线程输入框");
      return;
    }
    setFeedback(action.message ?? "该命令需要已有线程");
  };
  const submitComposer = (domValue?: string | null) => {
    const currentDraft = composerSubmitDraftValue(draft, domValue ?? composerTextareaRef.current?.value);
    if (currentDraft !== draft) setDraft(currentDraft);
    const exactSlash = slashCommandForComposerSubmit(currentDraft, capabilities);
    if (exactSlash) {
      executeSlashCommand(exactSlash);
      return;
    }
    if (!attachments.uploadInProgress && (currentDraft.trim() || attachments.readyUploads.length)) {
      mutation.mutate({ message: currentDraft });
    }
  };
  if (loading) {
    return <div className="empty-state"><Bot size={32} /><strong>正在读取线程</strong></div>;
  }
  return (
    <div className="new-thread-state">
      <Bot size={34} />
      <strong>新建 Codex 线程</strong>
      <span>通过受控 Codex job 启动，并在任务历史中记录。</span>
      <form className="composer new-composer" onSubmit={(event) => {
        event.preventDefault();
        submitComposer();
      }}>
        <input
          ref={attachments.inputRef}
          className="visually-hidden"
          type="file"
          multiple
          onChange={attachments.onFileInputChange}
        />
        <SlashCommandTextarea
          inputRef={(node) => {
            composerTextareaRef.current = node;
          }}
          value={draft}
          onChange={setDraft}
          placeholder="输入第一条消息"
          hasThread={false}
          plugins={pluginsQuery.data ?? []}
          pluginsUnavailable={pluginsQuery.isError}
          capabilities={capabilities}
          onSlashCommand={executeSlashCommand}
          onSubmitShortcut={submitComposer}
        />
        <ComposerAttachmentList
          uploads={attachments.uploads}
          removingUploadId={attachments.removingUploadId}
          onRemove={attachments.removeUpload}
        />
        <RunConfigControls
          config={runConfig}
          setConfig={setRunConfig}
          models={runOptions.models}
          profiles={runOptions.profiles}
          unavailable={runOptions.unavailable}
          onPickFiles={attachments.openPicker}
          uploadInProgress={attachments.uploadInProgress}
          threadStatus="Recent"
        />
        <div className="composer-actions">
          <span>{feedback ?? (result ? actionMessage(result) : "新线程会在列表中自动出现")}</span>
          <button className="primary-button" disabled={(!draft.trim() && !attachments.readyUploads.length) || attachments.uploadInProgress || mutation.isPending}><Play size={17} />启动</button>
        </div>
      </form>
    </div>
  );
}

function MessageBlockView({
  block,
  activePlan = false,
  planPending = false,
  activeQuestion = false,
  questionPending = false,
  onShowHistory,
  historyExpanded = false
}: {
  block: MessageBlock;
  activePlan?: boolean;
  planPending?: boolean;
  activeQuestion?: boolean;
  questionPending?: boolean;
  onShowHistory?: () => void;
  historyExpanded?: boolean;
}) {
  if (isHistoryCollapsedBlock(block)) {
    return <HistoryCollapseCell block={block} onShowHistory={onShowHistory} expanded={historyExpanded} />;
  }
  if (isPlanBlock(block)) {
    return (
      <ProposedPlanCell
        block={block}
        active={activePlan}
        pending={planPending}
      />
    );
  }
  if (isQuestionBlock(block)) {
    if (activeQuestion) return <QuestionCell block={block} pendingSubmit={questionPending} />;
    return <QuestionResultCell block={block} />;
  }
  if (isQuestionResultBlock(block)) {
    return <QuestionResultCell block={block} />;
  }
  if (isToolBlock(block)) {
    return <ToolBlockView block={block} />;
  }
  if (!shouldRenderConversationMessage(block)) {
    return null;
  }
  const presentation = conversationMessagePresentation(block);
  return (
    <article className={presentation.rowClassName}>
      <div className="chat-meta">
        <span>{roleLabel(block.role)}</span>
        <small>{blockKindLabel(block.kind)}{block.created_at ? ` · ${formatTime(block.created_at)}` : ""}</small>
      </div>
      <div className={presentation.bodyClassName}>
        <MessageContent text={messageBlockText(block)} />
      </div>
    </article>
  );
}

function ToolBlockView({ block }: { block: MessageBlock }) {
  const [open, setOpen] = useState(false);
  const summary = toolBlockSummary(block);
  return (
    <details
      className={`tool-card ${isRunningToolBlock(block) ? "running" : ""}`}
      onToggle={(event) => setOpen((event.currentTarget as HTMLDetailsElement).open)}
    >
      <summary>
        <span className="tool-title">{toolBlockTitle(block)}</span>
        <small>{toolBlockStatus(block)}</small>
        <ChevronRight size={16} />
      </summary>
      {summary && <div className="tool-summary">{summary}</div>}
      {open && <pre>{toolBlockDetailText(block)}</pre>}
    </details>
  );
}

function MessageContent({ text }: { text: string }) {
  const [copied, setCopied] = useState<string | null>(null);
  const segments = useMemo(() => segmentInternalReferences(text), [text]);
  return (
    <>
      {segments.map((segment, index) => {
        if (segment.type === "text") {
          return <span key={`text-${index}`}>{segment.text}</span>;
        }
        return (
          <button
            key={`ref-${index}-${segment.text}`}
            type="button"
            className="internal-reference"
            title="复制内部引用"
            onClick={async () => {
              const copyText = segment.copyText ?? segment.text;
              await navigator.clipboard?.writeText(copyText);
              setCopied(copyText);
              window.setTimeout(() => setCopied((current) => current === copyText ? null : current), 1600);
            }}
          >
            {segment.text}
            {copied === (segment.copyText ?? segment.text) && <small>已复制</small>}
          </button>
        );
      })}
    </>
  );
}

function HistoryCollapseCell({ block, onShowHistory, expanded }: { block: MessageBlock; onShowHistory?: () => void; expanded: boolean }) {
  const kind = historyCollapseKind(block);
  const label = firstDisplayLine(block.summary) ?? firstDisplayLine(block.text) ?? (kind === "tool" ? "历史工具活动已折叠" : kind === "action" ? "历史计划和问题已折叠" : "较早消息已折叠");
  const eyebrow = kind === "tool" ? "Tool activity" : kind === "action" ? "Plan & questions" : "Earlier messages";
  return (
    <article className="history-collapse-cell">
      <div>
        <span>{eyebrow}</span>
        <strong>{label}</strong>
      </div>
      {onShowHistory && (
        <button className="secondary-button" disabled={expanded} onClick={onShowHistory} type="button">
          {expanded ? "已显示全部" : "显示全部历史"}
        </button>
      )}
    </article>
  );
}

function ProposedPlanCell({ block, active, pending }: { block: MessageBlock; active: boolean; pending: boolean }) {
  return (
    <article className={active ? "plan-cell active" : "plan-cell"}>
      <div className="message-meta">
        <span>Proposed Plan</span>
        <small>{block.plan_status || block.status || block.turn_id || block.item_id || block.kind}</small>
      </div>
      <div className="plan-body">{extractPlanText(block.text || "")}</div>
      {active && pending && <div className="action-inline-status">正在提交计划操作...</div>}
    </article>
  );
}

function QuestionResultCell({ block }: { block: MessageBlock }) {
  const answers = block.answers ?? [];
  return (
    <article className="question-result-cell">
      <div className="message-meta">
        <span>Questions</span>
        <small>{block.status || "completed"}</small>
      </div>
      {answers.length > 0 ? (
        <div className="answered-list">
          {answers.map((answer) => (
            <div className="answered-row" key={answer.question_id}>
              <span>{answer.question_id}</span>
              <strong>{answer.answers.length ? answer.answers.join(", ") : "未回答"}</strong>
              {answer.note && <small>{answer.note}</small>}
            </div>
          ))}
        </div>
      ) : (
        <p>Questions answered</p>
      )}
    </article>
  );
}

function QuestionCell({ block, pendingSubmit }: { block: MessageBlock; pendingSubmit: boolean }) {
  return (
    <article className="question-cell active-choice">
      <div className="message-meta">
        <span>Questions</span>
        <small>{block.turn_id || block.item_id || block.call_id || "request_user_input"}</small>
      </div>
      {block.questions.map((question) => (
        <div key={question.id} className="question-block">
          <strong>{question.question}</strong>
          <div className="choice-grid">
            {question.options.map((option, index) => (
              <button
                key={`${question.id}-${option.label}`}
                className="choice-option"
                disabled
                type="button"
              >
                <span>{index + 1}</span>
                <strong>{option.label}</strong>
                {option.description && <small>{option.description}</small>}
              </button>
            ))}
          </div>
        </div>
      ))}
      {pendingSubmit && <div className="action-inline-status">正在提交选择...</div>}
    </article>
  );
}

function CurrentActionCard({
  plan,
  pending,
  onAcceptPlan,
  onRevisePlan,
  planPending,
  onSubmitQuestion,
  questionPending,
  onDismiss
}: {
  plan?: MessageBlock | null;
  pending?: PendingElicitation | null;
  onAcceptPlan: (block: MessageBlock) => void;
  onRevisePlan: (block: MessageBlock, instructions: string) => void;
  planPending: boolean;
  onSubmitQuestion: (answers: Record<string, string[]>) => void;
  questionPending: boolean;
  onDismiss: () => void;
}) {
  const isPlan = Boolean(plan);
  const busy = isPlan ? planPending : questionPending;
  const questions = pending?.questions ?? [];
  const questionSignature = questions.map((question) => `${question.id}:${question.options.map((option) => option.label).join("|")}`).join(";");
  const [selected, setSelected] = useState(0);
  const [revision, setRevision] = useState("");
  const [questionAnswers, setQuestionAnswers] = useState<Record<string, string | string[] | undefined>>({});
  const [questionNotes, setQuestionNotes] = useState<Record<string, string>>({});
  const options = isPlan ? currentPlanActionOptions() : questions[0]?.options ?? [];
  const selectedPlanRequiresRevision = isPlan && selected === 1;
  const ready = isPlan
    ? Boolean(plan && planActionSubmission(selected, revision))
    : questionAnswersReady(questions, combinedQuestionAnswers(questions, questionAnswers, questionNotes));

  function submitAction() {
    if (busy || !ready) return;
    if (plan) {
      const submission = planActionSubmission(selected, revision);
      if (!submission) return;
      if (submission.action === "accept") {
        onAcceptPlan(plan);
      } else if (submission.action === "revise") {
        onRevisePlan(plan, submission.instructions);
      } else {
        onDismiss();
      }
      return;
    }
    if (pending) onSubmitQuestion(questionAnswerPayload(questions, combinedQuestionAnswers(questions, questionAnswers, questionNotes)));
  }

  useEffect(() => {
    setSelected(0);
    setRevision("");
    setQuestionAnswers((current) => {
      const initial: Record<string, string | string[] | undefined> = {};
      for (const question of questions) {
        initial[question.id] = current[question.id] ?? question.options[0]?.label;
      }
      return initial;
    });
    setQuestionNotes({});
  }, [plan?.id, pending?.turn_id, pending?.item_id, questionSignature]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const editable = target?.closest("input, textarea, select, [contenteditable='true']");
      if (event.key === "Escape") {
        event.preventDefault();
        onDismiss();
        return;
      }
      if (!editable && (event.key === "ArrowDown" || event.key === "ArrowUp")) {
        event.preventDefault();
        setSelected((current) => moveActionSelection(current, options.length, event.key === "ArrowDown" ? 1 : -1));
        return;
      }
      if (!editable) {
        const digitSelection = selectionFromDigitKey(event.key, options.length);
        if (digitSelection !== null) {
          event.preventDefault();
          setSelected(digitSelection);
          if (!isPlan && questions[0]?.options[digitSelection]) {
            setQuestionAnswers((current) => ({ ...current, [questions[0].id]: questions[0].options[digitSelection].label }));
          }
          return;
        }
      }
      if (event.key === "Enter" && !event.shiftKey) {
        event.preventDefault();
        submitAction();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [busy, isPlan, onDismiss, options.length, questions, ready, revision, selected, questionAnswers, questionNotes]);

  const chooseQuestionOption = (questionId: string, label: string, index: number) => {
    setSelected(index);
    setQuestionAnswers((current) => ({ ...current, [questionId]: label }));
  };

  return (
    <section className="current-action-card" aria-live="polite">
      <div className="current-action-header">
        <div>
          <span>{isPlan ? "Plan Mode" : "Questions"}</span>
          <strong>{isPlan ? "实施此计划?" : questions[0]?.question ?? "Codex 正在等待选择"}</strong>
        </div>
        <small>↑↓ 选择 · 1-9 快选</small>
      </div>
      <div className="current-action-options">
        {isPlan ? options.map((option, index) => (
          <button
            type="button"
            key={option.label}
            className={selected === index ? "current-action-option selected" : "current-action-option"}
            onClick={() => setSelected(index)}
          >
            <span>{index + 1}</span>
            <div>
              <strong>{option.label}</strong>
              <small>{option.description}</small>
            </div>
          </button>
        )) : questions.map((question) => (
          <div className="current-action-question" key={question.id}>
            {questions.length > 1 && <strong>{question.question}</strong>}
            {question.options.map((option, index) => (
              <button
                type="button"
                key={`${question.id}-${option.label}`}
                className={questionAnswers[question.id] === option.label ? "current-action-option selected" : "current-action-option"}
                onClick={() => chooseQuestionOption(question.id, option.label, index)}
              >
                <span>{index + 1}</span>
                <div>
                  <strong>{option.label}</strong>
                  {option.description && <small>{option.description}</small>}
                </div>
              </button>
            ))}
            <textarea
              className="current-action-textarea"
              value={questionNotes[question.id] ?? ""}
              onChange={(event) => setQuestionNotes((current) => ({ ...current, [question.id]: event.target.value }))}
              placeholder="补充输入"
            />
          </div>
        ))}
      </div>
      {selectedPlanRequiresRevision && (
        <textarea
          className="current-action-textarea"
          value={revision}
          onChange={(event) => setRevision(event.target.value)}
          placeholder="告诉 Codex 需要怎样调整计划"
        />
      )}
      <div className="current-action-footer">
        <button className="secondary-button ghost" type="button" onClick={onDismiss}>
          忽略 <kbd>ESC</kbd>
        </button>
        <button className="primary-button" type="button" disabled={!ready || busy} onClick={submitAction}>
          提交 <kbd>↵</kbd>
        </button>
      </div>
    </section>
  );
}

function ApprovalCard({ block, onDecision, pending }: { block: MessageBlock; onDecision: (decision: string) => void; pending: boolean }) {
  return (
    <article className="approval-card action-request">
      <div className="message-meta">
        <span>审批请求</span>
        <small>{block.call_id || block.item_id || block.turn_id || block.kind}</small>
      </div>
      <pre>{block.text || formatPayload(block.payload) || "Codex 正在等待权限审批。"}</pre>
      <div className="button-row">
        <button className="primary-button" disabled={pending} onClick={() => onDecision("accept")}>
          <ClipboardCheck size={17} />批准
        </button>
        <button className="danger-button soft" disabled={pending} onClick={() => onDecision("decline")}>
          <X size={17} />拒绝
        </button>
      </div>
    </article>
  );
}

function UnsupportedApprovalCard({ block }: { block: MessageBlock }) {
  return (
    <article className="approval-card action-request">
      <div className="message-meta">
        <span>审批请求</span>
        <small>{block.call_id || block.item_id || block.turn_id || block.kind}</small>
      </div>
      <pre>{block.text || formatPayload(block.payload) || "Codex 正在等待权限审批。"}</pre>
      <div className="muted-row">macOS App 当前不支持在此面板处理权限审批，请在 Codex 原生会话中处理。</div>
    </article>
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

function ProbeWorkspace({ csrfToken, capabilities }: { csrfToken?: string | null; capabilities: RuntimeCapabilityMatrix }) {
  const { status, settings, logsDbStatus, events, jobs } = useProbeQueries();
  const [draft, setDraft] = useState<ProbeSettingsDraft | null>(null);
  const [saveStatus, setSaveStatus] = useState<{ tone: "success" | "error"; message: string } | null>(null);
  const [actionStatus, setActionStatus] = useState<{ tone: "success" | "error"; message: string } | null>(null);
  const [logsDbExecuteArmed, setLogsDbExecuteArmed] = useState(false);
  const [activeSection, setActiveSection] = useState<ProbeSectionId>("overview");
  const data = status.data?.data;
  const available = status.data?.available ?? false;
  const currentSettings = settings.data?.data;
  const settingsErrors = draft ? probeSettingsValidation(draft) : [];
  const logsDb = logsDbStatus.data?.data;
  const recentEvents = events.data?.data?.events ?? [];
  const logsDbStatusText = logsDb?.logs_db_status ?? logsDb?.status ?? data?.logs_db_status;
  const logsDbTone = probeLogsDbTone(logsDbStatusText);
  const barkConfigured = Boolean(currentSettings?.notifications?.device_key_configured || draft?.notifications.device_key_configured);
  const probeThreads = probeThreadsByStatus(data);
  const probeEnabled = data?.enabled ?? currentSettings?.probe?.enabled ?? false;
  const serviceText = data ? `${data.service_kind}:${data.service_name}` : "未知";
  const availability = probeAvailabilityView({
    available,
    probeEnabled,
    loading: status.isLoading,
    fetching: status.isFetching,
    hasData: Boolean(data),
    error: status.isError
  });
  const statusTone = availability.tone;
  const snapshotText = probeSnapshotStatusText(data, status.isFetching);
  const snapshotTone = data?.is_refreshing || status.isFetching ? "warning" : "success";
  const probeJobs = (jobs.data ?? []).filter(isProbeJob).slice(0, 6);
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
        <Metric label="Codex APP" value={availability.metric} tone={statusTone} />
        <Metric label="运行中" value={probeRunningCountValue(data)} tone={Number(probeRunningCountValue(data)) > 0 ? "success" : undefined} />
        <Metric label="需回复" value={String(data?.reply_needed_count ?? 0)} tone={(data?.reply_needed_count ?? 0) > 0 ? "warning" : undefined} />
        <Metric label="异常数" value={String(data?.recoverable_count ?? 0)} tone={(data?.recoverable_count ?? 0) > 0 ? "danger" : undefined} />
        <Metric label="Bark" value={barkConfigured ? "已配置" : "未配置"} tone={barkConfigured ? "success" : "warning"} />
        <Metric label="Hook 事件" value={String(data?.recent_event_count ?? recentEvents.length)} tone={(data?.recent_event_count ?? recentEvents.length) > 0 ? "success" : undefined} />
        <Metric label="日志库" value={probeStateLabel(logsDbStatusText)} tone={logsDbTone} />
        {capabilities.codexStatePaths && <Metric label="Codex Home" value={codexHomeStatusValue(data ?? currentSettings?.codex)} wide />}
        <Metric label="刷新" value={snapshotText} tone={snapshotTone} />
      </section>
      <section className="probe-control-grid" aria-label="探针线程状态">
        <ProbeThreadBucket title="需回复" icon={<MessageSquare size={18} />} threads={probeThreads.replyNeeded} emptyText="当前没有待回复线程" />
        <ProbeThreadBucket title="异常/可恢复" icon={<TriangleAlert size={18} />} threads={probeThreads.recoverable} emptyText="当前没有可恢复异常" />
        <ProbeThreadBucket title="运行中" icon={<Play size={18} />} threads={probeThreads.running} emptyText="当前没有运行线程" />
      </section>
    </>
  );
  const activeSectionContent = (() => {
    switch (activeSection) {
      case "reply-needed":
        return <ProbeThreadBucket title="需回复" icon={<MessageSquare size={18} />} threads={probeThreads.replyNeeded} emptyText="当前没有待回复线程" />;
      case "recoverable":
        return <ProbeThreadBucket title="异常/可恢复" icon={<TriangleAlert size={18} />} threads={probeThreads.recoverable} emptyText="当前没有可恢复异常" />;
      case "running":
        return <ProbeThreadBucket title="运行中" icon={<Play size={18} />} threads={probeThreads.running} emptyText="当前没有运行线程" />;
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
                configuredDeviceKey={barkConfigured}
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
                  configuredDeviceKey={barkConfigured}
                  capabilities={capabilities}
                  onSave={() => saveMutation.mutate(undefined)}
                />
              ) : (
                <div className="muted-row">{settings.isLoading ? "正在读取设置" : "设置不可用"}</div>
              )}
            </Panel>
            <Panel title="Probe Job History" icon={<TerminalSquare size={18} />} className="wide-panel">
              <JobList jobs={probeJobs} capabilities={capabilities} />
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
          <button className="secondary-button" onClick={() => probeJobMutation.mutate("bark-test")} disabled={!barkConfigured || probeJobMutation.isPending}><Cloud size={17} />测试 Bark</button>
        </div>
      </div>

      <section className={`probe-status-banner tone-${statusTone}`}>
        <div>
          <strong>{availability.headline}</strong>
          <span>{serviceText} · {data?.host_label ?? currentSettings?.codex?.host_label ?? "未知主机"}</span>
        </div>
        <span>{probeStateLabel(data?.hook_status)} · {probeStateLabel(logsDbStatusText)}</span>
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

      {availability.tone === "danger" && (
        <Panel title="端点" icon={<TriangleAlert size={18} />} className="wide-panel">
          <div className="muted-row">探针端点不可用</div>
        </Panel>
      )}
    </div>
  );
}

function OpsWorkspace({ csrfToken, capabilities }: { csrfToken?: string | null; capabilities: RuntimeCapabilityMatrix }) {
  const { status, update, jobs } = useOpsQueries();
  const [plan, setPlan] = useState<ArchiveDeletePlan | null>(null);
  const [hiddenPlan, setHiddenPlan] = useState<HiddenThreadDeletePlan | null>(null);
  const [hiddenDeleteResult, setHiddenDeleteResult] = useState<HiddenThreadDeleteResult | null>(null);
  const [deleteArmed, setDeleteArmed] = useState(false);
  const [hiddenDeleteArmed, setHiddenDeleteArmed] = useState(false);
  const opsActions = useOpsActions({
    csrfToken,
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
      setHiddenPlan((current) => current ? { ...current, hidden_threads: result.hidden_threads, hidden_ids: [], hidden_source_counts: {} } : current);
    }
  });
  const jobMutation = opsActions.updateJob;
  const dryRun = opsActions.archiveDryRun;
  const executeDelete = opsActions.archiveExecute;
  const hiddenDryRun = opsActions.hiddenDryRun;
  const executeHiddenDelete = opsActions.hiddenExecute;
  const publicEndpoint = cleanHostValue(status.data?.public_endpoint);
  const hostname = cleanHostValue(status.data?.hostname) ?? "读取中";
  const hiddenStats = hiddenThreadDeleteStats(hiddenPlan, status.data);
  const archivedCleanupStage = cleanupStageLabel({
    hasPlan: Boolean(plan),
    dryRunPending: dryRun.isPending,
    armed: deleteArmed,
    executePending: executeDelete.isPending,
    executableCount: plan?.archived_threads ?? 0
  });
  const hiddenCleanupStage = cleanupStageLabel({
    hasPlan: Boolean(hiddenPlan),
    dryRunPending: hiddenDryRun.isPending,
    armed: hiddenDeleteArmed,
    executePending: executeHiddenDelete.isPending,
    executableCount: hiddenStats.hidden
  });
  const updateActions = opsUpdateActionView(update.data, capabilities);

  return (
    <div className="ops-grid">
      <Panel title={OPS_PANEL_TITLES.system} icon={<HardDrive size={18} />} className="wide-panel ops-status-panel">
        <div className="ops-status-overview">
          <Metric label="Hostname" value={hostname} />
          {capabilities.publicEndpointStatus && <Metric label="Public endpoint" value={publicEndpoint ?? "未配置"} tone={publicEndpoint ? "success" : "warning"} />}
          {capabilities.codexStatePaths && <Metric label="state DB" value={status.data?.state_db_integrity ?? "unknown"} tone={status.data?.state_db_integrity === "ok" ? "success" : "warning"} />}
          {capabilities.codexStatePaths && <Metric label="Codex Home" value={codexHomeStatusValue(status.data)} wide />}
          {capabilities.codexStatePaths && <Metric label="State DB" value={status.data?.state_db ?? "unknown"} wide />}
          <Metric label="Hidden threads" value={String(status.data?.hidden_thread_count ?? 0)} tone={(status.data?.hidden_thread_count ?? 0) > 0 ? "warning" : undefined} />
          <Metric label="Sources" value={sourceCountsText(status.data?.thread_source_counts)} />
        </div>
      </Panel>
      <Panel title={OPS_PANEL_TITLES.updates} icon={<RefreshCw size={18} />}>
        <UpdateMetrics status={update.data} />
        <div className="button-row ops-action-row">
          {updateActions.map((action) => {
            const className = action.tone === "primary" ? "primary-button" : action.tone === "danger" ? "danger-button soft" : "secondary-button";
            const icon = action.action === "check" ? <CheckCircle2 size={17} /> : action.action === "install" ? <Play size={17} /> : <Trash2 size={17} />;
            return (
              <button key={action.action} className={className} disabled={jobMutation.isPending || action.disabled} onClick={() => jobMutation.mutate({ action: action.action })}>
                {icon}{action.label}
              </button>
            );
          })}
        </div>
      </Panel>
      {capabilities.threadCleanup && <Panel title={OPS_PANEL_TITLES.archivedCleanup} icon={<Archive size={18} />}>
        <div className="cleanup-panel-head">
          <span>删除 archived 线程与 rollout</span>
          <span className={`status-chip ${archivedCleanupStage.tone ? `tone-${archivedCleanupStage.tone}` : "tone-muted"}`}>{archivedCleanupStage.label}</span>
        </div>
        <div className="archive-plan">
          <Metric label="active" value={plan ? String(plan.active_threads) : "dry-run 未执行"} />
          <Metric label="archived" value={String(plan?.archived_threads ?? 0)} tone={(plan?.archived_threads ?? 0) > 0 ? "warning" : undefined} />
          <Metric label="integrity" value={plan?.integrity ?? status.data?.state_db_integrity ?? "unknown"} tone={(plan?.integrity ?? status.data?.state_db_integrity) === "ok" ? "success" : "danger"} />
          <Metric label="session index" value={plan ? String(plan.session_index_lines) : "dry-run 未执行"} />
          <Metric label="rollout 文件" value={plan ? String(plan.rollout_files) : "dry-run 未执行"} />
        </div>
        <div className="button-row ops-action-row cleanup-actions">
          <button className="secondary-button" disabled={dryRun.isPending || executeDelete.isPending} onClick={() => dryRun.mutate()}><Database size={17} />Dry-run</button>
          {!deleteArmed ? (
            <button className="danger-button soft" disabled={(plan?.archived_threads ?? 0) === 0 || dryRun.isPending || executeDelete.isPending} onClick={() => setDeleteArmed(true)}><Trash2 size={17} />清理归档</button>
          ) : (
            <>
              <button className="danger-button" onClick={() => executeDelete.mutate({ expectedCount: plan?.archived_threads ?? 0 })} disabled={executeDelete.isPending}><Trash2 size={17} />确认清理归档</button>
              <button className="secondary-button" onClick={() => setDeleteArmed(false)} disabled={executeDelete.isPending}>取消</button>
            </>
          )}
        </div>
      </Panel>}
      {capabilities.threadCleanup && <Panel title={OPS_PANEL_TITLES.hiddenCleanup} icon={<Database size={18} />}>
        <div className="cleanup-panel-head">
          <span>删除 non-archived subagent/internal</span>
          <span className={`status-chip ${hiddenCleanupStage.tone ? `tone-${hiddenCleanupStage.tone}` : "tone-muted"}`}>{hiddenCleanupStage.label}</span>
        </div>
        <div className="archive-plan">
          <Metric label="visible" value={hiddenPlan ? String(hiddenStats.visible) : "dry-run 未执行"} />
          <Metric label="hidden" value={String(hiddenStats.hidden)} tone={hiddenStats.hidden > 0 ? "warning" : undefined} />
          <Metric label="sources" value={hiddenStats.sourceCounts} />
          <Metric label="integrity" value={hiddenStats.integrity} tone={hiddenStats.integrity === "ok" ? "success" : "danger"} />
          <Metric label="rollout 删除结果" value={hiddenRolloutDeleteResultText(hiddenDeleteResult)} tone={hiddenDeleteResult ? "success" : undefined} />
        </div>
        <div className="button-row ops-action-row cleanup-actions">
          <button className="secondary-button" disabled={hiddenDryRun.isPending || executeHiddenDelete.isPending} onClick={() => hiddenDryRun.mutate()}><Database size={17} />扫描隐藏线程</button>
          {!hiddenDeleteArmed ? (
            <button className="danger-button soft" disabled={!canStartHiddenThreadDelete(hiddenPlan) || hiddenDryRun.isPending || executeHiddenDelete.isPending} onClick={() => setHiddenDeleteArmed(true)}><Trash2 size={17} />清理隐藏线程</button>
          ) : (
            <>
              <button className="danger-button" onClick={() => executeHiddenDelete.mutate({ expectedCount: hiddenStats.hidden })} disabled={executeHiddenDelete.isPending}><Trash2 size={17} />确认清理隐藏</button>
              <button className="secondary-button" onClick={() => setHiddenDeleteArmed(false)} disabled={executeHiddenDelete.isPending}>取消</button>
            </>
          )}
        </div>
      </Panel>}
      <Panel title={OPS_PANEL_TITLES.jobs} icon={<TerminalSquare size={18} />} className="wide-panel">
        <JobList jobs={jobs.data ?? []} capabilities={capabilities} />
      </Panel>
    </div>
  );
}

function UpdateMetrics({ status }: { status?: UpdateStatus }) {
  return (
    <div className="version-grid">
      <Metric label="Current" value={status?.current_version ?? "读取中"} />
      <Metric
        label="Latest"
        value={status?.latest_version ?? "unknown"}
        tone={status?.update_available ? "warning" : "success"}
      />
      <Metric label="Update" value={status?.update_available ? "available" : status?.state ?? "current"} tone={status?.update_available ? "warning" : "success"} />
    </div>
  );
}

function SecurityWorkspace({ csrfToken, username }: { csrfToken?: string | null; username: string }) {
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
  const ttlDays = secondsToDays(merged.session_ttl_seconds ?? defaultSessionTtlDays * secondsPerDay);
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
        <label className="field-label">Session TTL days<input type="number" min={1} value={ttlDays} onChange={(event) => setDraft({ ...draft, session_ttl_seconds: Math.max(1, Number(event.target.value) || defaultSessionTtlDays) * secondsPerDay })} /></label>
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

function Panel({ title, icon, children, className = "" }: { title: string; icon: ReactNode; children: ReactNode; className?: string }) {
  return <section className={`panel ${className}`}><header>{icon}<strong>{title}</strong></header>{children}</section>;
}

function Metric({ label, value, tone, wide = false }: { label: string; value: string; tone?: "success" | "warning" | "danger"; wide?: boolean }) {
  return <div className={wide ? "metric metric-wide" : "metric"}><span>{label}</span><strong className={tone ? `tone-${tone}` : ""}>{value}</strong></div>;
}

type ProbeSaveStatus = { tone: "success" | "error"; message: string } | null;

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
  const managed = status?.hook_status === "managed";
  const configured = managed || draft?.hooks.manage_stop_hook === true;
  return (
    <div className="probe-card-stack">
      <Metric label="Stop Hook" value={probeStateLabel(status?.hook_status)} tone={managed ? "success" : "warning"} />
      <Metric label="管理开关" value={configured ? "已开启" : "已关闭"} tone={configured ? "success" : "warning"} />
      <Metric label="动作" value="固定 Hook 安装 job" />
      <button className="secondary-button" disabled={busy} onClick={onInstall}><TerminalSquare size={17} />安装 Hook</button>
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

export { probeEventCard };

export function probeEventSummary(event: ProbeEvent): string {
  const thread = event.thread_id ? `线程 ${event.thread_id}` : "无线程";
  const fields = [
    event.payload?.session_id ? "session" : "",
    event.payload?.transcript_path ? "transcript" : "",
    event.payload?.last_assistant_message ? "assistant" : ""
  ].filter(Boolean);
  return [thread, fields.length ? fields.join(" · ") : "payload 已脱敏"].join(" · ");
}

export function shouldAutoScrollProbeFeed(
  current: MessageScrollSnapshot,
  _previous?: MessageScrollSnapshot | null
): boolean {
  return current.scrollHeight - current.scrollTop - current.clientHeight <= 32;
}

function providerById(providers: AgentProviderInfo[] | undefined, id: string): AgentProviderInfo | undefined {
  return providers?.find((provider) => provider.id === id);
}

function capabilityText(provider?: Pick<AgentProviderInfo, "capabilities"> | null): string {
  const capabilities = provider?.capabilities ?? [];
  return capabilities.length ? capabilities.join(", ") : "none";
}

type CodexHomePathFields = {
  home?: string | null;
  codex_home?: string | null;
  configured_codex_home?: string | null;
  resolved_codex_home?: string | null;
  codex_home_source?: string | null;
};

export function codexHomeStatusValue(status?: CodexHomePathFields | null): string {
  return pathWithSource(
    firstStringValue(status, ["resolved_codex_home", "codex_home", "home", "configured_codex_home"]),
    firstStringValue(status, ["codex_home_source"])
  );
}

export function logsDbPathStatusValue(logsDb?: ProbeLogsDbStatus | ProbeSettings["logs_db"] | null): string {
  return pathWithSource(
    firstStringValue(logsDb, ["resolved_logs_db_path", "resolved_path", "path", "logs_db_path"]),
    firstStringValue(logsDb, ["logs_db_source", "source"])
  );
}

export function probeDiscoveryWarningsText(warnings?: string[] | null): string {
  return warnings?.length ? warnings.join(", ") : "无";
}

function pathText(value?: string | null): string {
  return value && value.trim() ? value : "未知";
}

function pathWithSource(value?: string | null, source?: string | null): string {
  const path = pathText(value);
  const cleanedSource = source?.trim();
  return path !== "未知" && cleanedSource ? `${path} · ${cleanedSource}` : path;
}

function firstStringValue(source: unknown, keys: string[]): string | null {
  if (!source || typeof source !== "object") return null;
  const record = source as Record<string, unknown>;
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string" && value.trim()) return value.trim();
  }
  return null;
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

function probeFlavorLabel(flavor?: string | null): string {
  if (flavor === "builtin") return "内置";
  if (flavor === "server") return "服务";
  return flavor ?? "未知";
}

function probeStateLabel(value?: string | null): string {
  if (!value) return "未知";
  const labels: Record<string, string> = {
    managed: "已管理",
    disabled: "已停用",
    configured: "已配置",
    not_configured: "未配置",
    maintenance_ready: "可维护",
    ready: "就绪",
    ok: "正常",
    builtin: "内置"
  };
  return labels[value] ?? value;
}

function probeLogsDbTone(value?: string | null): "success" | "warning" | "danger" {
  if (value === "ok" || value === "maintenance_ready") return "success";
  if (value === "disabled") return "warning";
  return value ? "danger" : "warning";
}

function isProbeSettings(value: unknown): value is ProbeSettings {
  return Boolean(value && typeof value === "object" && "codex" in value && "probe" in value && "notifications" in value && "logs_db" in value);
}

function logBytesDraftToMb(value: ProbeSettingsDraft["observability"]["log_max_bytes"]): number | "" {
  if (value === "") return "";
  return Math.max(1, Math.round(value / (1024 * 1024)));
}

function mbDraftToLogBytes(value: string): number | "" {
  const parsed = probeNumberInputDraftValue(value);
  return parsed === "" ? "" : parsed * 1024 * 1024;
}

function probeLogDbNumber(logsDb: ProbeLogsDbStatus | undefined, keys: string[]): string {
  const value = probeLogDbValue(logsDb, keys);
  return typeof value === "number" && Number.isFinite(value) ? String(value) : "未知";
}

function probeLogDbString(logsDb: ProbeLogsDbStatus | undefined, keys: string[]): string {
  const value = probeLogDbValue(logsDb, keys);
  if (typeof value === "string" && value.trim()) return value;
  if (typeof value === "number" && Number.isFinite(value)) return String(value);
  if (typeof value === "boolean") return value ? "true" : "false";
  return "未知";
}

function probeLogDbSize(logsDb: ProbeLogsDbStatus | undefined, keys: string[]): string {
  const value = probeLogDbValue(logsDb, keys);
  return typeof value === "number" && Number.isFinite(value) ? formatFileSize(value) : "未知";
}

function probeLogDbValue(logsDb: ProbeLogsDbStatus | undefined, keys: string[]): unknown {
  if (!logsDb) return undefined;
  for (const key of keys) {
    const value = logsDb[key];
    if (value !== undefined && value !== null && value !== "") return value;
  }
  return undefined;
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

function JobList({ jobs, capabilities }: { jobs: JobRecord[]; capabilities: RuntimeCapabilityMatrix }) {
  return (
    <div className="job-list">
      {jobs.map((job) => {
        const analysis = job.failure_analysis ? jobFailureAnalysisView(job.failure_analysis, capabilities) : null;
        return (
          <details key={job.id} className="job-item">
            <summary>
              <span>{job.title}</span>
              <StatusDot status={job.status} />
              <ChevronRight size={16} />
            </summary>
            <div className="job-meta">
              <span>{job.kind}</span>
              {job.thread_id && <span>{job.thread_id}</span>}
              {job.turn_id && <span>{job.turn_id}</span>}
            </div>
            {analysis && (
              <div className="job-analysis">
                <strong>{analysis.label}</strong>
                <p>{analysis.explanation}</p>
                <ul>
                  {analysis.suggestions.map((suggestion) => <li key={suggestion}>{suggestion}</li>)}
                </ul>
              </div>
            )}
            <pre>{jobOutputView(job.output || job.error || "no output", capabilities)}</pre>
          </details>
        );
      })}
      {jobs.length === 0 && <div className="muted-row">暂无后台 job</div>}
    </div>
  );
}

function StatusDot({ status }: { status: string }) {
  return <span className={`status-dot ${status}`} />;
}

function StatusChip({ status }: { status: ThreadStatus }) {
  return <span className={`status-chip ${status}`}>{threadStatusLabel(status)}</span>;
}

function legacyBlocks(detail: ThreadDetail): MessageBlock[] {
  return detail.messages.map((message, index) => ({
    id: `legacy-${index}`,
    role: message.role,
    kind: message.kind,
    text: message.text,
    created_at: message.created_at,
    questions: []
  }));
}

export function pendingFromBlocks(blocks: MessageBlock[], status: ThreadStatus, activeTurnId: string | null | undefined): PendingElicitation | null {
  void status;
  if (!activeTurnId) return null;
  const block = [...blocks].reverse().find((item) => item.turn_id === activeTurnId && isQuestionBlock(item) && !isResolvedActionBlock(item));
  if (!block) return null;
  return {
    turn_id: block.turn_id,
    item_id: block.item_id ?? block.call_id,
    questions: block.questions
  };
}

export function latestActionBlock(blocks: MessageBlock[], status: ThreadStatus, activeTurnId: string | null | undefined, predicate: (block: MessageBlock) => boolean): MessageBlock | null {
  const reversed = [...blocks].reverse();
  if (activeTurnId) {
    const active = reversed.find((block) => block.turn_id === activeTurnId && predicate(block) && !isResolvedActionBlock(block));
    if (active) return active;
    return null;
  }
  if (status !== "ReplyNeeded") return null;
  if (!reversed.some((block) => predicate(block) && isPlanBlock(block))) return null;

  for (const block of reversed) {
    if (predicate(block) && isPlanBlock(block) && !isResolvedActionBlock(block)) return block;
    if (isExternalProgressAfterPlan(block)) return null;
  }
  return null;
}

export function currentPendingElicitation(pending: PendingElicitation | null | undefined, activeTurnId: string | null | undefined): PendingElicitation | null {
  if (!pending || !activeTurnId) return null;
  if (pending.turn_id !== activeTurnId) return null;
  return pending;
}

export function isPlanBlock(block: MessageBlock): boolean {
  const kind = normalizedBlockKind(block);
  const displayKind = normalizedDisplayKind(block);
  return displayKind === "plan" || kind.includes("plan") || Boolean(block.text?.includes("<proposed_plan>"));
}

export function isApprovalBlock(block: MessageBlock): boolean {
  const kind = normalizedBlockKind(block);
  const displayKind = normalizedDisplayKind(block);
  return displayKind === "approval" || kind.includes("requestapproval") || kind.includes("approval") || kind.includes("permissions/request");
}

export function isQuestionBlock(block: MessageBlock): boolean {
  const kind = normalizedBlockKind(block);
  const displayKind = normalizedDisplayKind(block);
  return displayKind === "question"
    || (kind === "request_user_input" || kind === "requestuserinput")
    || kind.includes("request_user_input")
    || kind.includes("requestuserinput")
    || (block.questions?.length ?? 0) > 0;
}

export function isQuestionResultBlock(block: MessageBlock): boolean {
  const kind = normalizedBlockKind(block);
  const displayKind = normalizedDisplayKind(block);
  return displayKind === "question_result"
    || kind === "request_user_input_result"
    || (block.answers?.length ?? 0) > 0;
}

export function isToolBlock(block: MessageBlock): boolean {
  const kind = normalizedBlockKind(block);
  return block.role === "tool"
    || kind.includes("tool")
    || kind.includes("command")
    || kind.includes("function_call")
    || kind.includes("web_search");
}

export function shouldRenderConversationMessage(block: MessageBlock): boolean {
  const role = block.role || "";
  const kind = normalizedBlockKind(block);
  if (!["assistant", "user"].includes(role)) return false;
  if ((block.questions?.length ?? 0) > 0) return false;
  if (kind === "request_user_input" || kind === "requestuserinput") return false;
  if (isToolBlock(block) || isPlanBlock(block) || isApprovalBlock(block)) return false;
  if (isInternalContextText(block.text)) return false;
  return !["reasoning", "agent_reasoning", "session_meta"].includes(kind);
}

export function shouldRenderConversationBlock(block: MessageBlock): boolean {
  if (isApprovalBlock(block)) return false;
  if (isPlanBlock(block) || isQuestionBlock(block) || isQuestionResultBlock(block)) return true;
  return isToolBlock(block) || shouldRenderConversationMessage(block);
}

export function shouldRenderActionStackBlock(block: MessageBlock): boolean {
  return isApprovalBlock(block) && !isResolvedActionBlock(block);
}

export function isResolvedActionBlock(block: MessageBlock): boolean {
  if (block.resolved === true) return true;
  const status = (block.plan_status ?? block.status ?? "").toLowerCase();
  return ["completed", "complete", "succeeded", "success", "done", "approved", "declined", "rejected", "cancelled", "canceled", "failed"].includes(status);
}

export function isActionablePlanBlock(block: MessageBlock, current: MessageBlock | null | undefined): boolean {
  return Boolean(current && isPlanBlock(block) && !isResolvedActionBlock(block) && sameActionBlock(block, current));
}

export function isActionableQuestionBlock(block: MessageBlock, current: PendingElicitation | MessageBlock | null | undefined): boolean {
  if (!current || !isQuestionBlock(block) || isResolvedActionBlock(block)) return false;
  const currentTurnId = current.turn_id ?? null;
  if (currentTurnId && block.turn_id !== currentTurnId) return false;
  const currentItemId = "id" in current ? current.item_id ?? current.call_id ?? current.id : current.item_id ?? null;
  if (!currentItemId) return Boolean(currentTurnId);
  return [block.item_id, block.call_id, block.id].some((value) => value === currentItemId);
}

export function questionAnswerLabels(block: MessageBlock, questionId: string): string[] {
  return block.answers?.find((answer) => answer.question_id === questionId)?.answers ?? [];
}

export function blocksWithCurrentPending(blocks: MessageBlock[], pending: PendingElicitation | null): MessageBlock[] {
  if (!pending || !pending.questions.length) return blocks;
  const existing = blocks.some((block) => {
    if (!isQuestionBlock(block)) return false;
    if (pending.item_id && (block.item_id === pending.item_id || block.call_id === pending.item_id)) return true;
    if (pending.turn_id && block.turn_id === pending.turn_id) return true;
    return false;
  });
  if (existing) return blocks;
  return [
    ...blocks,
    {
      id: `pending-question-${pending.turn_id ?? pending.item_id ?? "current"}`,
      role: "assistant",
      kind: "request_user_input",
      display_kind: "question",
      status: "pending",
      resolved: false,
      turn_id: pending.turn_id,
      item_id: pending.item_id,
      questions: pending.questions
    }
  ];
}

function sameActionBlock(left: MessageBlock, right: MessageBlock): boolean {
  const leftIds = [left.id, left.item_id, left.call_id].filter(Boolean);
  const rightIds = [right.id, right.item_id, right.call_id].filter(Boolean);
  const sameId = leftIds.some((leftId) => rightIds.includes(leftId));
  if (!sameId) return false;
  if (left.turn_id && right.turn_id) return left.turn_id === right.turn_id;
  return true;
}

function isExternalProgressAfterPlan(block: MessageBlock): boolean {
  if (isPlanBlock(block) || isQuestionBlock(block) || isQuestionResultBlock(block) || isApprovalBlock(block)) return false;
  if (isHistoryCollapsedBlock(block)) return false;
  if (isToolBlock(block)) return true;
  return shouldRenderConversationMessage(block);
}

function normalizedBlockKind(block: Pick<MessageBlock, "kind">): string {
  return block.kind.toLowerCase();
}

function normalizedDisplayKind(block: Pick<MessageBlock, "display_kind">): string {
  return block.display_kind?.toLowerCase() ?? "";
}

export type ConversationMessagePresentation = {
  kind: "user" | "assistant";
  rowClassName: string;
  bodyClassName: string;
};

export function conversationMessagePresentation(block: Pick<MessageBlock, "role">): ConversationMessagePresentation {
  const kind = block.role === "user" ? "user" : "assistant";
  return {
    kind,
    rowClassName: `chat-row ${kind}`,
    bodyClassName: kind === "user" ? "chat-bubble" : "assistant-message-body"
  };
}

export function visibleConversationBlocksForHistory(
  blocks: MessageBlock[],
  showAllHistory: boolean,
  currentPlan?: MessageBlock | null,
  currentQuestion?: PendingElicitation | MessageBlock | null
): MessageBlock[] {
  const renderable = blocks.filter(shouldRenderConversationBlock);
  if (showAllHistory) return renderable;
  return compactConversationBlocks(renderable, 4, 60, 3, currentPlan, currentQuestion);
}

export function prioritizeCurrentActionBlocks(
  blocks: MessageBlock[],
  currentPlan: MessageBlock | null | undefined,
  currentQuestion: PendingElicitation | MessageBlock | null | undefined
): MessageBlock[] {
  const promoted: MessageBlock[] = [];
  const rest: MessageBlock[] = [];
  for (const block of blocks) {
    if (isActionablePlanBlock(block, currentPlan) || isActionableQuestionBlock(block, currentQuestion)) {
      promoted.push(block);
    } else {
      rest.push(block);
    }
  }
  return promoted.length ? [...rest, ...promoted] : blocks;
}

export function compactConversationBlocks(
  blocks: MessageBlock[],
  maxCompletedTools = 4,
  maxChatMessages = 60,
  maxActionBlocks = 3,
  currentPlan?: MessageBlock | null,
  currentQuestion?: PendingElicitation | MessageBlock | null
): MessageBlock[] {
  const hasToolHistoryCollapse = blocks.some((block) => historyCollapseKind(block) === "tool");
  const completedToolIndexes = hasToolHistoryCollapse
    ? []
    : blocks
      .map((block, index) => ({ block, index }))
      .filter(({ block }) => isToolBlock(block) && !isHistoryCollapsedBlock(block) && !isRunningToolBlock(block))
      .map(({ index }) => index);
  const toolCompacted = hasToolHistoryCollapse
    ? blocks
    : compactIndexedBlocks(
      blocks,
      completedToolIndexes,
      maxCompletedTools,
      "completed-tool-history-collapsed",
      "tool_history_collapsed",
      "tool_history",
      "个历史工具调用已折叠"
    );

  if (toolCompacted.some((block) => historyCollapseKind(block) === "action")) {
    return toolCompacted;
  }

  const actionIndexes = toolCompacted
    .map((block, index) => ({ block, index }))
    .filter(({ block }) => {
      if (isHistoryCollapsedBlock(block)) return false;
      if (isActionablePlanBlock(block, currentPlan)) return false;
      if (isActionableQuestionBlock(block, currentQuestion)) return false;
      return isPlanBlock(block) || isQuestionBlock(block) || isQuestionResultBlock(block);
    })
    .map(({ index }) => index);
  const actionCompacted = compactIndexedBlocks(
    toolCompacted,
    actionIndexes,
    maxActionBlocks,
    "action-history-collapsed",
    "action_history_collapsed",
    "action_history",
    "条历史计划/问题已折叠"
  );

  if (actionCompacted.some((block) => historyCollapseKind(block) === "chat")) {
    return actionCompacted;
  }

  const chatIndexes = actionCompacted
    .map((block, index) => ({ block, index }))
    .filter(({ block }) => shouldRenderConversationMessage(block))
    .map(({ index }) => index);
  return compactIndexedBlocks(
    actionCompacted,
    chatIndexes,
    maxChatMessages,
    "chat-history-collapsed",
    "chat_history_collapsed",
    "chat_history",
    "条历史对话已折叠"
  );
}

function compactIndexedBlocks(
  blocks: MessageBlock[],
  indexes: number[],
  maxVisible: number,
  id: string,
  kind: string,
  toolName: string,
  label: string
): MessageBlock[] {
  if (indexes.length <= maxVisible) return blocks;
  const keep = new Set(indexes.slice(-maxVisible));
  const hide = new Set(indexes.slice(0, -maxVisible));
  const hidden = indexes.length - keep.size;
  const collapsed: MessageBlock = {
    id,
    role: "tool",
    kind,
    status: "completed",
    text: `${hidden} ${label}`,
    summary: `${hidden} ${label}`,
    tool_name: toolName,
    truncated: false,
    questions: []
  };
  const compacted: MessageBlock[] = [];
  let inserted = false;
  for (const [index, block] of blocks.entries()) {
    if (hide.has(index) && !keep.has(index)) {
      if (!inserted) {
        compacted.push(collapsed);
        inserted = true;
      }
      continue;
    }
    compacted.push(block);
  }
  return compacted;
}

function isHistoryCollapsedBlock(block: MessageBlock): boolean {
  return historyCollapseKind(block) !== null;
}

function historyCollapseKind(block: MessageBlock): "chat" | "tool" | "action" | null {
  const kind = block.kind.toLowerCase();
  if (kind === "chat_history_collapsed") return "chat";
  if (kind === "tool_history_collapsed") return "tool";
  if (kind === "action_history_collapsed") return "action";
  return null;
}

function isInternalContextText(value?: string | null): boolean {
  const text = value?.trimStart().toLowerCase();
  if (!text) return false;
  return [
    "<environment_context>",
    "<permissions instructions>",
    "<app-context>",
    "<collaboration_mode>",
    "<skills_instructions>",
    "<plugins_instructions>",
    "<subagent_notification>",
    "<subagent_context>",
    "<codex_internal_context",
    "<goal_context>",
    "<additional_context>",
    "<user_instructions>",
    "<turn_aborted>",
    "<user_shell_command>",
    "<legacy_unified_exec_process_limit_warning>",
    "<legacy_apply_patch_exec_command_warning>",
    "<legacy_model_mismatch_warning>",
    "========= memory_summary begins ========="
  ].some((prefix) => text.startsWith(prefix));
}

export function toolBlockTitle(block: MessageBlock): string {
  if (isHistoryCollapsedBlock(block)) {
    return firstDisplayLine(block.summary)
      ?? firstDisplayLine(block.text)
      ?? block.tool_name?.trim()
      ?? block.kind
      ?? "tool";
  }
  return block.tool_name?.trim() || block.kind || "tool";
}

export function toolBlockStatus(block: MessageBlock): string {
  return block.status?.trim() || block.call_id || "completed";
}

export function toolBlockSummary(block: MessageBlock): string | null {
  return firstDisplayLine(block.summary)
    ?? firstDisplayLine(block.text)
    ?? firstDisplayLine(block.input)
    ?? null;
}

export function toolBlockDetailText(block: MessageBlock): string {
  const sections: string[] = [];
  const input = block.input?.trim();
  const output = (block.text?.trim() || formatPayload(block.payload).trim());
  if (input) sections.push(`Input\n${input}`);
  if (output) sections.push(`${input ? "Output\n" : ""}${output}`);
  if (block.truncated) sections.push("[output truncated]");
  return sections.join("\n\n") || "No output";
}

export function messageBlockText(block: MessageBlock): string {
  return block.text?.trim() || formatPayload(block.payload) || "";
}

function firstDisplayLine(value?: string | null): string | null {
  const line = value?.split(/\r?\n/).map((item) => item.trim()).find(Boolean);
  return line || null;
}

export function probeSnapshotStatusText(status?: Pick<ProbeStatus, "snapshot_age_seconds" | "is_refreshing" | "snapshot_status"> | null, fetching = false): string {
  const age = typeof status?.snapshot_age_seconds === "number" ? Math.max(0, Math.round(status.snapshot_age_seconds)) : null;
  const prefix = status?.is_refreshing || fetching ? "后台刷新" : "已同步";
  if (age === null) return prefix;
  if (age < 60) return `${prefix} ${age}s`;
  const minutes = Math.floor(age / 60);
  return `${prefix} ${minutes}m`;
}

export function probeAvailabilityView(input: {
  available?: boolean;
  probeEnabled?: boolean;
  loading?: boolean;
  fetching?: boolean;
  hasData?: boolean;
  error?: boolean;
}): { headline: string; metric: string; tone: "success" | "warning" | "danger" } {
  if (!input.hasData && input.error) {
    return {
      headline: "Probe 快照读取失败",
      metric: "读取失败",
      tone: "danger"
    };
  }
  if (!input.hasData && (input.loading || input.fetching)) {
    return {
      headline: "正在读取 Probe 快照",
      metric: "读取中",
      tone: "warning"
    };
  }
  if (input.available) {
    return input.probeEnabled
      ? { headline: "Probe 正在接管云机观测", metric: "运行中", tone: "success" }
      : { headline: "Probe 已停用", metric: "停用", tone: "warning" };
  }
  return {
    headline: "Probe 端点不可用",
    metric: "不可用",
    tone: "danger"
  };
}

function cleanHostValue(value?: string | null): string | null {
  const cleaned = value?.trim();
  const legacyAlias = ["tencent", "wanka"].join("-");
  if (!cleaned || cleaned === legacyAlias) return null;
  return cleaned;
}

function hostnameFromPublicEndpoint(value?: string | null): string | null {
  const endpoint = cleanHostValue(value);
  if (!endpoint) return null;
  try {
    return new URL(endpoint).hostname || null;
  } catch {
    return endpoint.replace(/^\/+/, "").split("/")[0]?.split(":")[0] || null;
  }
}

function secondsToDays(seconds: number): number {
  return Math.max(1, Math.round(seconds / secondsPerDay));
}

function normalizeTurnstileAction(value?: string | null): string {
  const action = value?.trim();
  return action || "login";
}

function blockKindLabel(kind: string): string {
  if (kind.includes("agentMessage")) return "assistant";
  if (kind.includes("userMessage")) return "user";
  if (kind.includes("function")) return "tool";
  return kind;
}

function roleLabel(role: string): string {
  if (role === "assistant") return "Codex";
  if (role === "user") return "User";
  if (role === "tool") return "Tool";
  return role || "System";
}

function formatPayload(payload: unknown): string {
  if (!payload) return "";
  if (typeof payload === "string") return payload;
  return JSON.stringify(payload, null, 2);
}

function formatTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}
