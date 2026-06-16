import { QueryClient, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
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
  Goal,
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
import { ChangeEvent, FormEvent, ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  acceptPlan,
  answerApproval,
  answerElicitation,
  archiveThread,
  cancelFollowUp,
  changePassword,
  clearGoalMode,
  createThread,
  deleteUpload,
  dryRunArchiveDelete,
  dryRunHiddenThreadDelete,
  forkThread,
  getCodexConfig,
  getGoalMode,
  getPublicSettings,
  getProbeLogsDbStatus,
  getProbeSettings,
  getProbeStatus,
  getSecurity,
  getSystemStatus,
  getSystemVersion,
  getThread,
  getThreadBlocks,
  listFollowUps,
  listPlugins,
  listModels,
  listJobs,
  listPermissionProfiles,
  listThreads,
  login,
  logout,
  renameThread,
  revisePlan,
  resumeGoalMode,
  restoreThread,
  saveProbeSettings,
  saveSecurity,
  sendMessage,
  setGoalMode,
  startClaudeCodeJob,
  startArchiveDelete,
  startHiddenThreadDelete,
  startProbeJob,
  startUpdateJob,
  stopThread,
  steerThread,
  subscribeThreadEvents,
  uploadFiles,
  getClaudeCodeOverview,
  getPlatformOverview,
  getProbeEvents,
  listProviders,
  type ThreadSendPayload
} from "./lib/api";
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
import { clearSession, loadSession, saveSession } from "./lib/session";
import {
  applyRealtimeBlocksToThreadSlot,
  applyThreadBlockPageToSlot,
  applyThreadDetailToSlot,
  applyThreadSummaryToSlot,
  clearThreadSlot,
  createThreadMessageStoreState,
  getThreadSlot,
  setActiveThreadSlot,
  setThreadFeedback as setThreadSlotFeedback,
  setThreadHiddenActionKey,
  setThreadHistoryExpanded,
  setThreadLastResult,
  setThreadLoadingEarlier,
  type ThreadMessageSlot,
  type ThreadMessageStoreState
} from "./lib/threadMessageStore";
import type {
  ArchiveDeletePlan,
  AgentProviderInfo,
  BridgeActionResult,
  ClaudeOverview,
  CodexConfig,
  CodexModel,
  FollowUpQueueItem,
  HiddenThreadDeletePlan,
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
  SystemVersion,
  ThreadDetail,
  ThreadStatus,
  ThreadSummary,
  UploadRecord
} from "./types";

type View = "codex" | "claude" | "probe" | "ops" | "security";
type SelectedThread = string | "__new" | null;
type PermissionPresetId = "ask" | "auto" | "full" | "custom";
type RunConfig = {
  model: string;
  serviceTier: string;
  reasoning: string;
  cwd: string;
  permissionPreset: PermissionPresetId;
  permissionProfile: string;
  approvalPolicy: string;
  sandboxMode: string;
  networkAccess: boolean | null;
  collaborationMode: string;
};

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

type ThreadTitleLike = {
  title?: string | null;
  [key: string]: unknown;
};

type ThreadListItemLike = ThreadTitleLike & {
  status?: ThreadStatus | string | null;
  latest_message?: string | null;
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

const reasoningOptions = ["", "low", "medium", "high", "xhigh"];
const permissionPresets: Array<{ id: PermissionPresetId; label: string; description: string; icon: ReactNode }> = [
  { id: "ask", label: "请求批准", description: "编辑外部文件和使用互联网时始终询问", icon: <Lock size={17} /> },
  { id: "auto", label: "替我审批", description: "仅对检测到的风险操作请求批准", icon: <ShieldCheck size={17} /> },
  { id: "full", label: "完全访问权限", description: "可不受限制地访问互联网和文件", icon: <CheckCircle2 size={17} /> },
  { id: "custom", label: "自定义 (config.toml)", description: "使用 config.toml 中定义的权限", icon: <SlidersHorizontal size={17} /> }
];
const defaultCwd = "/home/ubuntu/codex-workspace";
const defaultSessionTtlDays = 365;
const secondsPerDay = 86400;

type SlashCommand = {
  command: string;
  description: string;
  usageHint: string;
  requiresThread?: boolean;
};

type PluginMentionCandidate = {
  id: string;
  label: string;
  description: string;
  unavailableReason?: string | null;
  plugin?: PluginInfo;
};

type SlashCommandAction =
  | { kind: "archive_thread" | "clear_goal" | "copy_latest" | "fork_thread" | "open_debug_config" | "open_new_thread" | "open_plugins" | "open_resume" | "open_status" | "open_thread_settings" | "resume_goal" | "stop_thread" | "toggle_fast" | "toggle_plan_mode"; command: string; message?: string }
  | { kind: "focus_control" | "insert_template" | "requires_thread" | "unavailable" | "unknown"; command: string; message: string };

type ControlledSlashActionKind = "archive_thread" | "clear_goal" | "copy_latest" | "fork_thread" | "open_debug_config" | "open_new_thread" | "open_plugins" | "open_resume" | "open_status" | "open_thread_settings" | "resume_goal" | "stop_thread" | "toggle_fast" | "toggle_plan_mode";

export const slashCommands: SlashCommand[] = [
  { command: "/permissions", description: "调整权限与审批模式", usageHint: "/permissions" },
  { command: "/ide", description: "加入 IDE 上下文", usageHint: "/ide" },
  { command: "/keymap", description: "查看或调整 TUI 快捷键", usageHint: "/keymap" },
  { command: "/vim", description: "切换 Vim 输入模式", usageHint: "/vim" },
  { command: "/sandbox-add-read-dir", description: "添加沙盒只读目录，Windows 专用", usageHint: "/sandbox-add-read-dir <path>" },
  { command: "/agent", description: "切换或查看子代理线程", usageHint: "/agent", requiresThread: true },
  { command: "/apps", description: "浏览 apps 与 connectors", usageHint: "/apps" },
  { command: "/plugins", description: "浏览插件", usageHint: "/plugins" },
  { command: "/hooks", description: "查看生命周期 hooks", usageHint: "/hooks" },
  { command: "/clear", description: "清空当前输入或开启新会话语义", usageHint: "/clear" },
  { command: "/archive", description: "归档当前会话", usageHint: "/archive", requiresThread: true },
  { command: "/compact", description: "压缩当前上下文", usageHint: "/compact", requiresThread: true },
  { command: "/copy", description: "复制最新回复", usageHint: "/copy", requiresThread: true },
  { command: "/diff", description: "查看当前 diff", usageHint: "/diff", requiresThread: true },
  { command: "/exit", description: "退出当前会话", usageHint: "/exit", requiresThread: true },
  { command: "/quit", description: "退出当前会话", usageHint: "/quit", requiresThread: true },
  { command: "/experimental", description: "查看实验功能", usageHint: "/experimental" },
  { command: "/approve", description: "批准一次自动审查拒绝后的重试", usageHint: "/approve", requiresThread: true },
  { command: "/memories", description: "查看记忆设置", usageHint: "/memories" },
  { command: "/skills", description: "浏览或使用技能", usageHint: "/skills" },
  { command: "/feedback", description: "提交反馈", usageHint: "/feedback" },
  { command: "/init", description: "生成 AGENTS.md", usageHint: "/init" },
  { command: "/logout", description: "退出登录", usageHint: "/logout" },
  { command: "/mcp", description: "查看 MCP 工具", usageHint: "/mcp" },
  { command: "/mention", description: "引用文件或目录", usageHint: "/mention <path>" },
  { command: "/model", description: "切换模型或推理等级", usageHint: "/model" },
  { command: "/fast", description: "切换 Fast 服务层", usageHint: "/fast" },
  { command: "/plan", description: "切换计划模式，可带内联提示", usageHint: "/plan [prompt]" },
  { command: "/goal", description: "查看或设置当前线程 Goal", usageHint: "/goal [objective]", requiresThread: true },
  { command: "/goal pause", description: "暂停当前线程 Goal", usageHint: "/goal pause", requiresThread: true },
  { command: "/goal resume", description: "恢复当前线程 Goal", usageHint: "/goal resume", requiresThread: true },
  { command: "/goal clear", description: "清除当前线程 Goal", usageHint: "/goal clear", requiresThread: true },
  { command: "/personality", description: "切换沟通风格", usageHint: "/personality" },
  { command: "/ps", description: "查看后台终端", usageHint: "/ps", requiresThread: true },
  { command: "/stop", description: "停止后台终端", usageHint: "/stop", requiresThread: true },
  { command: "/fork", description: "分叉当前对话", usageHint: "/fork", requiresThread: true },
  { command: "/side", description: "开启旁路对话", usageHint: "/side [prompt]", requiresThread: true },
  { command: "/btw", description: "开启旁路对话别名", usageHint: "/btw [prompt]", requiresThread: true },
  { command: "/raw", description: "切换原始滚动输出", usageHint: "/raw", requiresThread: true },
  { command: "/resume", description: "恢复历史会话", usageHint: "/resume" },
  { command: "/new", description: "新建会话", usageHint: "/new" },
  { command: "/review", description: "请求工作区 review", usageHint: "/review" },
  { command: "/status", description: "查看会话状态", usageHint: "/status" },
  { command: "/debug-config", description: "查看配置层诊断", usageHint: "/debug-config" },
  { command: "/statusline", description: "配置状态栏", usageHint: "/statusline" },
  { command: "/title", description: "配置终端标题", usageHint: "/title" },
  { command: "/theme", description: "选择语法主题", usageHint: "/theme" }
];

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
  const [session, setSession] = useState<SessionUser | null>(() => loadSession());
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

  if (!session) {
    return <LoginScreen onLogin={(user) => {
      saveSession(user);
      setSession(user);
    }} />;
  }

  return (
    <div className={`app-shell ${navCollapsed ? "nav-collapsed" : ""}`}>
      {navCollapsed ? (
        <button className="nav-restore" onClick={toggleNavCollapsed} title="展开导航"><PanelLeftOpen size={18} /></button>
      ) : (
        <SideNav view={view} setView={setView} onCollapse={toggleNavCollapsed} onLogout={async () => {
          await logout(session.csrf_token);
          clearSession();
          setSession(null);
        }} />
      )}
      <main className="main-workspace">
        <MobileTopBar onOpenThreads={() => setMobileThreadsOpen(true)} view={view} setView={setView} />
        {view === "codex" && (
          <ChatWorkspace
            csrfToken={session.csrf_token}
            mobileThreadsOpen={mobileThreadsOpen}
            setMobileThreadsOpen={setMobileThreadsOpen}
            setView={setView}
          />
        )}
        {view === "claude" && <ClaudeWorkspace />}
        {view === "probe" && <ProbeWorkspace csrfToken={session.csrf_token} />}
        {view === "ops" && <OpsWorkspace csrfToken={session.csrf_token} />}
        {view === "security" && <SecurityWorkspace csrfToken={session.csrf_token} username={session.username} />}
      </main>
    </div>
  );
}

function LoginScreen({ onLogin }: { onLogin: (user: SessionUser) => void }) {
  const [username, setUsername] = useState("admin");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [turnstileToken, setTurnstileToken] = useState("");
  const [turnstileStatus, setTurnstileStatus] = useState<"idle" | "loading" | "ready" | "verified" | "error">("idle");
  const widgetRef = useRef<TurnstileWidgetId | null>(null);
  const turnstileRef = useRef<HTMLDivElement | null>(null);
  const publicSettings = useQuery({ queryKey: ["public-settings"], queryFn: getPublicSettings });
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

  const mutation = useMutation({
    mutationFn: () => login(username, password, turnstileToken),
    onSuccess: onLogin,
    onError: (err: Error) => {
      setError(err.message);
      resetTurnstile();
    }
  });

  return (
    <div className="login-shell">
      <form className="login-panel" onSubmit={(event) => {
        event.preventDefault();
        setError(null);
        if (turnstileEnabled && !turnstileToken.trim()) {
          setError("请先完成 Turnstile 验证");
          return;
        }
        mutation.mutate();
      }}>
        <div className="brand-mark"><Cloud size={24} /></div>
        <h1>NexusHub</h1>
        <p>root app-server 专用控制台</p>
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

function SideNav({ view, setView, onCollapse, onLogout }: { view: View; setView: (view: View) => void; onCollapse: () => void; onLogout: () => void }) {
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
        {navigationItems.map((item) => (
          <NavButton key={item.id} icon={item.icon} active={view === item.id} onClick={() => setView(item.id)}>
            {item.label}
          </NavButton>
        ))}
      </nav>
      <button className="ghost-button nav-logout" onClick={onLogout}><LogOut size={17} />退出</button>
    </aside>
  );
}

function NavButton({ icon, active, onClick, children }: { icon: ReactNode; active: boolean; onClick: () => void; children: ReactNode }) {
  return <button className={`nav-button ${active ? "active" : ""}`} onClick={onClick}>{icon}{children}</button>;
}

function MobileTopBar({ onOpenThreads, view, setView }: { onOpenThreads: () => void; view: View; setView: (view: View) => void }) {
  const current = navigationItems.find((item) => item.id === view);
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
        {navigationItems.map((item) => (
          <button key={item.id} className={view === item.id ? "active" : ""} onClick={() => setView(item.id)}>
            {item.icon}
            {item.label}
          </button>
        ))}
      </div>
    </>
  );
}

function ChatWorkspace({ csrfToken, mobileThreadsOpen, setMobileThreadsOpen, setView }: {
  csrfToken?: string | null;
  mobileThreadsOpen: boolean;
  setMobileThreadsOpen: (open: boolean) => void;
  setView: (view: View) => void;
}) {
  const qc = useQueryClient();
  const [status, setStatus] = useState("all");
  const [q, setQ] = useState("");
  const [selectedId, setSelectedId] = useState<SelectedThread>(null);
  const messageStore = useThreadMessageStoreController();
  const threads = useQuery({ queryKey: ["threads", status, q], queryFn: () => listThreads(status, q), refetchInterval: 5000 });
  const visibleThreads = useMemo(() => filterVisibleThreadSummaries(threads.data ?? []), [threads.data]);
  const resolvedSelected = selectedId === "__new" ? null : selectedId ?? visibleThreads[0]?.id ?? null;
  const selectedThreadSummary = useMemo(
    () => visibleThreads.find((thread) => thread.id === resolvedSelected) ?? null,
    [visibleThreads, resolvedSelected]
  );
  const detail = useQuery({
    queryKey: ["thread", resolvedSelected],
    queryFn: () => getThread(resolvedSelected!),
    enabled: Boolean(resolvedSelected),
    refetchInterval: (query) => {
      const current = query.state.data as ThreadDetail | undefined;
      return threadDetailRefetchInterval(current, selectedThreadSummary);
    }
  });
  const selectedDetail = detail.data?.summary.id === resolvedSelected ? detail.data : null;

  useEffect(() => {
    if (!resolvedSelected || selectedDetail?.summary.status !== "Archived") return;
    removeThreadFromListCaches(qc, resolvedSelected);
    messageStore.clear(resolvedSelected);
    if (selectedId === resolvedSelected) {
      setSelectedId(null);
    }
  }, [messageStore, qc, resolvedSelected, selectedDetail, selectedId]);

  useEffect(() => {
    if (!resolvedSelected || !selectedThreadSummary) return;
    qc.setQueryData<ThreadDetail>(["thread", resolvedSelected], (current) => {
      if (!current) return current;
      return mergeThreadDetailSummaryFromList(current, selectedThreadSummary);
    });
  }, [qc, resolvedSelected, selectedThreadSummary]);

  useEffect(() => {
    messageStore.setActive(resolvedSelected);
  }, [messageStore, resolvedSelected]);

  useEffect(() => {
    if (!resolvedSelected || !selectedThreadSummary) return;
    const slot = messageStore.getSlot(resolvedSelected);
    if (!slot.summary) {
      messageStore.applySummary(resolvedSelected, selectedThreadSummary);
    }
  }, [messageStore, resolvedSelected, selectedThreadSummary]);

  useEffect(() => {
    if (!resolvedSelected || !selectedDetail) return;
    messageStore.applyDetail(resolvedSelected, selectedDetail);
  }, [messageStore, resolvedSelected, selectedDetail]);

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
      onRefresh={() => qc.invalidateQueries({ queryKey: ["threads"] })}
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
          />
        ) : (
          <EmptyConversation
            loading={Boolean(resolvedSelected && detail.isLoading)}
            csrfToken={csrfToken}
            onCreated={(id) => selectThread(id)}
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
          <span>root app-server</span>
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
  const models = useQuery({ queryKey: ["codex-models"], queryFn: listModels, staleTime: 60000 });
  const profiles = useQuery({ queryKey: ["codex-permission-profiles"], queryFn: listPermissionProfiles, staleTime: 60000 });
  const config = useQuery({ queryKey: ["codex-config"], queryFn: getCodexConfig, staleTime: 60000 });
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

function makeRunConfig(config?: CodexConfig, summary?: ThreadSummary): RunConfig {
  return {
    model: summary?.model ?? config?.model ?? "gpt-5.5",
    serviceTier: normalizeServiceTier(config?.service_tier),
    reasoning: config?.reasoning_effort ?? "xhigh",
    cwd: summary?.cwd ?? config?.cwd ?? defaultCwd,
    permissionPreset: permissionPresetFromConfig(config),
    permissionProfile: config?.permission_profile ?? "",
    approvalPolicy: config?.approval_policy ?? "never",
    sandboxMode: config?.sandbox_mode ?? "danger-full-access",
    networkAccess: config?.network_access ?? true,
    collaborationMode: ""
  };
}

function permissionPresetFromConfig(config?: CodexConfig): PermissionPresetId {
  if (!config?.approval_policy && !config?.sandbox_mode && !config?.permission_profile) return "custom";
  if (config?.approval_policy === "on-request" && config?.sandbox_mode === "workspace-write") return "ask";
  if (config?.approval_policy === "untrusted" && config?.sandbox_mode === "workspace-write") return "auto";
  if (config?.approval_policy === "never" && config?.sandbox_mode === "danger-full-access") return "full";
  return "custom";
}

export function defaultRunConfig(): RunConfig {
  return makeRunConfig();
}

export function applyPermissionPreset(config: RunConfig, preset: PermissionPresetId): RunConfig {
  if (preset === "custom") {
    return {
      ...config,
      permissionPreset: preset,
      permissionProfile: "",
      approvalPolicy: "",
      sandboxMode: "",
      networkAccess: null
    };
  }
  if (preset === "full") {
    return {
      ...config,
      permissionPreset: preset,
      permissionProfile: "",
      approvalPolicy: "never",
      sandboxMode: "danger-full-access",
      networkAccess: true
    };
  }
  return {
    ...config,
    permissionPreset: preset,
    permissionProfile: "",
    approvalPolicy: preset === "auto" ? "untrusted" : "on-request",
    sandboxMode: "workspace-write",
    networkAccess: true
  };
}

export function threadListItemText(thread: ThreadTitleLike): string {
  return thread.title?.trim() || "未命名线程";
}

export function filterVisibleThreadSummaries<T extends Partial<ThreadSummary>>(threads: T[]): T[] {
  return threads.filter(isVisibleMainThread);
}

export function isVisibleMainThread(thread: Partial<ThreadSummary>): boolean {
  if (thread.status === "Archived" || thread.archived_at) return false;
  if (nonEmptyString(thread.parentThreadId ?? thread.parent_thread_id)) return false;
  if (nonEmptyString(thread.agentPath ?? thread.agent_path)) return false;
  if (nonEmptyString(thread.agentNickname ?? thread.agent_nickname)) return false;
  if (nonEmptyString(thread.agentRole ?? thread.agent_role)) return false;
  if (fieldContainsSubagent(thread.threadSource ?? thread.thread_source)) return false;
  if (fieldContainsSubagent(thread.sourceKind ?? thread.source_kind)) return false;
  if (sourceValueContainsSubagent(thread.source)) return false;
  return !isInternalExecThread(thread);
}

export function threadListItemStatusText(thread: ThreadListItemLike): string {
  return threadStatusLabel(thread.status);
}

export function threadListItemPreviewText(thread: ThreadListItemLike): string {
  return cleanThreadPreviewText(thread.latest_message);
}

export function cleanThreadPreviewText(value?: string | null): string {
  const source = value?.trim();
  if (!source) return "";
  return extractPlanText(source).replace(/\s+/g, " ").trim();
}

export function conversationTitleText(thread: ThreadTitleLike): string {
  return thread.title?.trim() || "未命名线程";
}

function isPlaceholderThreadTitle(title?: string | null): boolean {
  const value = title?.trim() ?? "";
  return !value || value === "未命名线程" || value === "Untitled thread" || value === "Untitled";
}

export function mergeIncomingThreadSummary<T extends Partial<ThreadSummary>>(current: T, incoming: Partial<ThreadSummary>): T & Partial<ThreadSummary> {
  const next = { ...current, ...incoming };
  if (!isPlaceholderThreadTitle(current.title) && isPlaceholderThreadTitle(incoming.title)) {
    next.title = current.title;
  }
  if (!isUserVisibleLastEventKind(incoming.last_event_kind) && isUserVisibleLastEventKind(current.last_event_kind)) {
    next.last_event_kind = current.last_event_kind;
  }
  return next;
}

export function lastEventKindText(summary: Pick<ThreadSummary, "last_event_kind">): string {
  const value = summary.last_event_kind?.trim();
  if (!isUserVisibleLastEventKind(value)) return "未知";
  return value || "未知";
}

function isUserVisibleLastEventKind(value?: string | null): boolean {
  const event = value?.trim();
  return Boolean(event && !event.startsWith("app-server.") && !event.startsWith("panel."));
}

export function mergeThreadDetailSummaryFromList(detail: ThreadDetail, incoming: Partial<ThreadSummary>): ThreadDetail {
  return {
    ...detail,
    summary: mergeIncomingThreadSummary(detail.summary, incoming) as ThreadSummary
  };
}

export function threadMatchesListFilter(thread: Partial<ThreadSummary>, status = "all", q = ""): boolean {
  if (!isVisibleMainThread(thread)) return false;
  if (status !== "all") {
    if (status === "running" && !isThreadListItemRunning(thread)) return false;
    if (status === "reply-needed" && thread.status !== "ReplyNeeded") return false;
    if (status === "recoverable" && thread.status !== "Recoverable") return false;
    if (!["running", "reply-needed", "recoverable"].includes(status) && thread.status !== status) return false;
  }
  const needle = q.trim().toLowerCase();
  if (!needle) return true;
  return [
    thread.id,
    thread.title,
    thread.latest_message
  ].some((value) => String(value ?? "").toLowerCase().includes(needle));
}

export function mergeThreadSummaryIntoListCache(
  rows: ThreadSummary[] | undefined,
  incoming: ThreadSummary,
  status = "all",
  q = ""
): ThreadSummary[] | undefined {
  if (!rows) return rows;
  const existing = rows.find((thread) => thread.id === incoming.id);
  const merged = existing ? mergeIncomingThreadSummary(existing, incoming) as ThreadSummary : incoming;
  const matches = threadMatchesListFilter(merged, status, q);
  if (!matches) {
    return existing ? rows.filter((thread) => thread.id !== incoming.id) : rows;
  }
  if (existing) {
    return rows.map((thread) => thread.id === incoming.id ? merged : thread);
  }
  return [merged, ...rows];
}

export function removeThreadFromListCaches(qc: QueryClient, threadId: string): void {
  for (const query of qc.getQueryCache().findAll({ queryKey: ["threads"] })) {
    qc.setQueryData<ThreadSummary[]>(query.queryKey, (rows) =>
      rows ? rows.filter((thread) => thread.id !== threadId) : rows
    );
  }
}

function updateThreadListCaches(qc: QueryClient, incoming: ThreadSummary) {
  for (const query of qc.getQueryCache().findAll({ queryKey: ["threads"] })) {
    const [, status = "all", q = ""] = query.queryKey as [string, string?, string?];
    qc.setQueryData<ThreadSummary[]>(query.queryKey, (rows) =>
      mergeThreadSummaryIntoListCache(rows, incoming, status, q)
    );
  }
}

export function threadDetailRefetchInterval(detail?: ThreadDetail, selectedSummary?: Partial<ThreadSummary> | null): number {
  if (detail) return isThreadRunning(detail.summary, detail.blocks, null) ? 2000 : 5000;
  return selectedSummary && isThreadRunning(selectedSummary, [], null) ? 2000 : 5000;
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

export function isThreadRunning(summary: Partial<ThreadSummary>, blocks: MessageBlock[] = [], lastResult?: Partial<BridgeActionResult> | null): boolean {
  void blocks;
  void lastResult;
  if (summary.status === "Running") return true;
  if (summary.status === "Recent" && Boolean(summary.active_job_id)) return true;
  return false;
}

export function isThreadListItemRunning(thread: Partial<ThreadSummary>): boolean {
  return isThreadRunning(thread, [], null);
}

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
  kind?: "path" | "thread" | "turn" | "job" | "goal";
};

const internalReferencePattern = /((?:\/(?:Users|Volumes|home|root|tmp|var|opt|srv|etc|run|private)\/[^\s,，。；;）)]+)|\b(?:thread|turn|job|goal)[\s:=#-]+[A-Za-z0-9._:-]{3,})/gi;

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
  if (lower.startsWith("job")) return "job";
  return "goal";
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

export function slashCommandSuggestions(draft: string, cursor: number, hasThread = true): SlashCommand[] {
  const query = activeSlashQuery(draft, cursor)?.value.toLowerCase();
  if (!query) return [];
  return slashCommands
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

export function exactSlashCommandFromDraft(draft: string): string | null {
  const command = draft.trim().replace(/\s+/g, " ");
  return slashCommands.some((item) => item.command === command) ? command : null;
}

export function slashCommandForComposerSubmit(draft: string): string | null {
  return exactSlashCommandFromDraft(draft);
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

const controlledSlashActions: Record<string, ControlledSlashActionKind> = {
  "/new": "open_new_thread",
  "/resume": "open_resume",
  "/archive": "archive_thread",
  "/fork": "fork_thread",
  "/stop": "stop_thread",
  "/goal": "open_thread_settings",
  "/goal resume": "resume_goal",
  "/goal clear": "clear_goal",
  "/fast": "toggle_fast",
  "/plan": "toggle_plan_mode",
  "/status": "open_status",
  "/debug-config": "open_debug_config",
  "/copy": "copy_latest",
  "/plugins": "open_plugins",
  "/apps": "open_plugins",
  "/skills": "open_plugins"
};

const focusSlashCommands = new Set(["/model", "/permissions", "/title"]);
const templateSlashCommands = new Set(["/compact", "/diff", "/mention", "/review", "/side", "/btw", "/raw", "/init", "/approve"]);
const unavailableSlashCommands: Record<string, string> = {
  "/ide": "Web 端暂不支持注入 IDE 上下文；可在本机 Codex TUI 中使用该命令。",
  "/vim": "Web 端暂不支持 Vim 输入模式；请使用浏览器输入法或本机 TUI。",
  "/keymap": "Web 端暂不支持 TUI 快捷键设置；浏览器快捷键由系统和浏览器管理。",
  "/theme": "Web 端暂不支持 TUI 主题切换；NexusHub 使用固定设计系统。",
  "/exit": "Web 端暂不需要退出 TUI；关闭页面或切换线程即可。",
  "/quit": "Web 端暂不需要退出 TUI；关闭页面或切换线程即可。",
  "/sandbox-add-read-dir": "Web 端暂不支持动态添加沙盒只读目录；请通过 Codex 配置或受控权限预设处理。",
  "/agent": "Web 端暂不支持切换子代理控制台；可在线程列表查看主线程。",
  "/hooks": "Web 端 Hook 维护在探针页面处理。",
  "/clear": "Web 端不清空历史线程；可清空当前输入或新建线程。",
  "/experimental": "Web 端暂不暴露实验开关。",
  "/memories": "Web 端暂不管理本机记忆；请在 Codex TUI 或本地文件中查看。",
  "/feedback": "Web 端暂不接入反馈通道。",
  "/logout": "请使用左下角退出登录按钮。",
  "/mcp": "Web 端暂不直接操作 MCP；可在插件/Provider 页面查看可用能力。",
  "/personality": "Web 端暂不支持切换 Personality。请在 Codex 配置中调整。",
  "/ps": "Web 端暂不显示后台终端列表；当前固定运维任务在 Job History 中查看。",
  "/statusline": "Web 端暂不配置 TUI 状态栏。"
};

export function slashCommandAction(command: string, hasThread = true): SlashCommandAction {
  const normalized = command.trim().replace(/\s+/g, " ");
  const known = slashCommands.find((item) => item.command === normalized);
  if (!known) return { kind: "unknown", command: normalized, message: "未知 Slash 命令" };
  if (known.requiresThread && !hasThread) {
    return { kind: "requires_thread", command: normalized, message: "该命令需要已有线程，请先选择或创建线程。" };
  }
  const controlled = controlledSlashActions[normalized];
  if (controlled) return { kind: controlled, command: normalized };
  if (focusSlashCommands.has(normalized)) {
    return { kind: "focus_control", command: normalized, message: "请使用输入框下方的同名控制项调整。" };
  }
  if (templateSlashCommands.has(normalized)) {
    return { kind: "insert_template", command: normalized, message: "已插入命令模板，请补充参数后发送。" };
  }
  return {
    kind: "unavailable",
    command: normalized,
    message: unavailableSlashCommands[normalized] ?? "Web 端暂不支持该 TUI 命令；请使用现有面板或本机 Codex TUI。"
  };
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

export function renderSlashCommandMenuHtml(draft: string, cursor: number, hasThread = true, selected = 0): string {
  const suggestions = slashCommandSuggestions(draft, cursor, hasThread);
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

export function runConfigAfterSuccessfulSend<T extends { collaborationMode: string }>(config: T): T {
  if (config.collaborationMode !== "plan") return config;
  return { ...config, collaborationMode: "" };
}

export function mergeRunConfigFromDefaults<T extends { collaborationMode: string }>(current: T, defaults: T): T {
  return {
    ...defaults,
    collaborationMode: current.collaborationMode
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

export function threadSettingsMetricLabels(): string[] {
  return [];
}

export function threadResumeCommand(threadId?: string | null): string | null {
  const id = threadId?.trim();
  return id ? `codex resume ${id}` : null;
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

function updateSavedThreadTitleCaches(qc: QueryClient, threadId: string, title: string) {
  for (const query of qc.getQueryCache().findAll({ queryKey: ["threads"] })) {
    qc.setQueryData<ThreadSummary[]>(query.queryKey, (rows) =>
      rows ? mergeSavedThreadTitle(rows, threadId, title) : rows
    );
  }
  qc.setQueryData<ThreadDetail>(["thread", threadId], (current) =>
    current ? { ...current, summary: { ...current.summary, title } } : current
  );
}

function useComposerAttachments(csrfToken?: string | null, setFeedback?: (message: string | null) => void) {
  const inputRef = useRef<HTMLInputElement | null>(null);
  const [uploads, setUploads] = useState<ComposerUpload[]>([]);
  const [uploadInProgress, setUploadInProgress] = useState(false);
  const [removingUploadId, setRemovingUploadId] = useState<string | null>(null);
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
      const outcome = await uploadFiles(files, csrfToken);
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
      await deleteUpload(upload.id, csrfToken);
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
  const slashSuggestions = menuKind === "slash" ? slashCommandSuggestions(value, cursor, hasThread) : [];
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
    const command = exactSlashCommandFromDraft(currentValue);
    if (!command) return false;
    onSlashCommand(command);
    return true;
  };
  const selectedSlashMatchesExactDraft = (command: string, currentValue = value) => exactSlashCommandFromDraft(currentValue) === command;

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
type PlanActionSubmission = { action: "accept" } | { action: "revise"; instructions: string };

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
    { label: "是，实施此计划", description: "按聊天记录里的 Proposed Plan 继续执行" },
    { label: "否，请告知 Codex 如何调整", description: "补充修改要求后重新生成计划" }
  ];
}

export function planActionSubmission(selected: number, revision: string): PlanActionSubmission | null {
  if (selected === 0) return { action: "accept" };
  if (selected === 1 && revision.trim()) return { action: "revise", instructions: revision.trim() };
  return null;
}

export function questionAnswersReady(questions: CurrentActionQuestion[], answers: Record<string, string | string[] | undefined>): boolean {
  return questions.every((question) => {
    const value = answers[question.id];
    if (Array.isArray(value)) return value.some((item) => item.trim().length > 0);
    return typeof value === "string" && value.trim().length > 0;
  });
}

export function questionAnswerPayload(questions: CurrentActionQuestion[], answers: Record<string, string | string[] | undefined>): Record<string, string[]> {
  return Object.fromEntries(questions.map((question) => {
    const value = answers[question.id];
    return [question.id, Array.isArray(value) ? value : value ? [value] : []];
  }));
}

export function hiddenThreadDeleteStats(plan: HiddenThreadDeletePlan | null, status?: Pick<SystemStatus, "hidden_thread_count" | "app_server_hidden_thread_count" | "app_server_source_counts" | "state_db_integrity">): { hidden: number; visible: number; sourceCounts: string; integrity: string } {
  const hidden = plan?.hidden_threads ?? status?.app_server_hidden_thread_count ?? status?.hidden_thread_count ?? 0;
  return {
    hidden,
    visible: plan?.visible_threads ?? 0,
    sourceCounts: sourceCountsText(plan?.hidden_source_counts ?? status?.app_server_source_counts),
    integrity: plan?.integrity ?? status?.state_db_integrity ?? "未知"
  };
}

export function canStartHiddenThreadDelete(plan: HiddenThreadDeletePlan | null | undefined): boolean {
  return (plan?.hidden_threads ?? 0) > 0;
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

export function modelSupportsServiceTier(models: CodexModel[], modelId: string, tierId: string): boolean {
  const model = models.find((item) => item.id === modelId);
  return Boolean(model?.service_tiers?.some((tier) => tier.id === tierId));
}

function normalizeServiceTier(value?: string | null): string {
  if (value === "fast") return "priority";
  return value?.trim() || "";
}

function nonEmptyString(value: unknown): boolean {
  return typeof value === "string" && value.trim().length > 0;
}

function fieldContainsSubagent(value: unknown): boolean {
  return typeof value === "string" && value.toLowerCase().includes("subagent");
}

function sourceValueContainsSubagent(value: unknown): boolean {
  if (typeof value === "string") return value.toLowerCase().includes("subagent");
  if (Array.isArray(value)) return value.some(sourceValueContainsSubagent);
  if (typeof value === "object" && value) {
    return Object.entries(value).some(([key, item]) => key.toLowerCase().includes("subagent") || sourceValueContainsSubagent(item));
  }
  return false;
}

function isInternalExecThread(thread: Partial<ThreadSummary>): boolean {
  if (normalizeString(thread.source) !== "exec") return false;
  if (!explicitlyNoUserEvent(thread.hasUserEvent ?? thread.has_user_event)) return false;

  const threadSource = normalizeString(thread.threadSource ?? thread.thread_source);
  if (threadSource && threadSource !== "user") return false;

  return [
    thread.title,
    thread.firstUserMessage ?? thread.first_user_message,
    thread.preview,
    thread.latest_message
  ].some((value) => typeof value === "string" && isInternalThreadPromptText(value));
}

function explicitlyNoUserEvent(value: unknown): boolean {
  return value === false || value === 0 || value === "0";
}

function normalizeString(value: unknown): string {
  return typeof value === "string" ? value.trim().toLowerCase() : "";
}

function isInternalThreadPromptText(value: string): boolean {
  const text = value.trim().toLowerCase();
  if (!text) return false;

  const readonlyProbe = text.includes("只读验证")
    || text.includes("只读核查")
    || text.includes("不要修改文件")
    || text.includes("不改文件")
    || text.includes("read-only")
    || text.includes("readonly");
  const agentProbe = text.includes("spawn_agent")
    || text.includes("子代理")
    || text.includes("subagent")
    || text.includes("model_reasoning_effort=xhigh");
  if (readonlyProbe && agentProbe) return true;

  const strongSubagentInstruction = text.includes("你是子代理")
    || text.includes("你是并行子代理")
    || text.includes("you are a subagent");
  const fixedAgentConfig = text.includes("gpt-5.5")
    || text.includes("xhigh")
    || text.includes("model_reasoning_effort=xhigh");
  return strongSubagentInstruction && fixedAgentConfig;
}

function threadDetailFromSlot(threadId: string, slot: ThreadMessageSlot, fallback?: ThreadSummary | null): ThreadDetail {
  return {
    summary: slot.summary ?? fallback ?? {
      id: threadId,
      title: "读取中",
      status: "Recent" as ThreadStatus,
      message_count: 0
    },
    messages: [],
    blocks: slot.blocks,
    raw_event_count: slot.totalBlocks,
    total_blocks: slot.totalBlocks,
    has_more_blocks: slot.hasMoreBlocks,
    before_cursor: slot.beforeCursor
  };
}

type ThreadMessageStoreController = {
  store: ThreadMessageStoreState;
  setActive: (threadId: string | null) => void;
  getSlot: (threadId: string) => ThreadMessageSlot;
  isActive: (threadId: string) => boolean;
  applyDetail: (threadId: string, detail: ThreadDetail) => void;
  applySummary: (threadId: string, summary: ThreadSummary) => void;
  patchSummary: (threadId: string, patch: Partial<ThreadSummary> | ((current: ThreadSummary) => ThreadSummary)) => void;
  applyRealtimeBlocks: (threadId: string, blocks: MessageBlock[]) => void;
  applyBlockPage: (threadId: string, page: Awaited<ReturnType<typeof getThreadBlocks>>, expectedCursor?: string | null) => void;
  setLoadingEarlier: (threadId: string, loading: boolean, error?: string | null) => void;
  setFeedback: (threadId: string, feedback: string | null) => void;
  setLastResult: (threadId: string, result: BridgeActionResult | null) => void;
  setHistoryExpanded: (threadId: string, expanded: boolean) => void;
  setHiddenActionKey: (threadId: string, key: string | null) => void;
  clear: (threadId: string) => void;
};

function fallbackThreadSummary(threadId: string): ThreadSummary {
  return {
    id: threadId,
    title: "读取中",
    status: "Recent",
    message_count: 0
  };
}

function useThreadMessageStoreController(): ThreadMessageStoreController {
  const storeRef = useRef<ThreadMessageStoreState>(createThreadMessageStoreState());
  const [, setRevision] = useState(0);
  const notify = useCallback((threadId: string | null) => {
    if (threadId && storeRef.current.activeThreadId !== threadId) return;
    setRevision((value) => value + 1);
  }, []);
  const setActive = useCallback((nextThreadId: string | null) => {
    setActiveThreadSlot(storeRef.current, nextThreadId);
    notify(nextThreadId);
  }, [notify]);
  const getSlotForThread = useCallback((nextThreadId: string) => getThreadSlot(storeRef.current, nextThreadId), []);
  const isActive = useCallback((nextThreadId: string) => storeRef.current.activeThreadId === nextThreadId, []);
  const applyDetail = useCallback((nextThreadId: string, nextDetail: ThreadDetail) => {
    applyThreadDetailToSlot(storeRef.current, nextThreadId, nextDetail, legacyBlocks);
    notify(nextThreadId);
  }, [notify]);
  const applySummary = useCallback((nextThreadId: string, nextSummary: ThreadSummary) => {
    applyThreadSummaryToSlot(storeRef.current, nextThreadId, nextSummary);
    notify(nextThreadId);
  }, [notify]);
  const patchSummary = useCallback((nextThreadId: string, patch: Partial<ThreadSummary> | ((current: ThreadSummary) => ThreadSummary)) => {
    const slot = getThreadSlot(storeRef.current, nextThreadId);
    const base = slot.summary ?? fallbackThreadSummary(nextThreadId);
    const next = typeof patch === "function" ? patch(base) : { ...base, ...patch };
    applyThreadSummaryToSlot(storeRef.current, nextThreadId, next);
    notify(nextThreadId);
  }, [notify]);
  const applyRealtimeBlocks = useCallback((nextThreadId: string, blocks: MessageBlock[]) => {
    applyRealtimeBlocksToThreadSlot(storeRef.current, nextThreadId, blocks);
    notify(nextThreadId);
  }, [notify]);
  const applyBlockPage = useCallback((nextThreadId: string, page: Awaited<ReturnType<typeof getThreadBlocks>>, expectedCursor?: string | null) => {
    applyThreadBlockPageToSlot(storeRef.current, nextThreadId, page, expectedCursor);
    notify(nextThreadId);
  }, [notify]);
  const setLoadingEarlier = useCallback((nextThreadId: string, loading: boolean, error: string | null = null) => {
    setThreadLoadingEarlier(storeRef.current, nextThreadId, loading, error);
    notify(nextThreadId);
  }, [notify]);
  const setFeedbackForThread = useCallback((nextThreadId: string, feedback: string | null) => {
    setThreadSlotFeedback(storeRef.current, nextThreadId, feedback);
    notify(nextThreadId);
  }, [notify]);
  const setLastResultForThread = useCallback((nextThreadId: string, result: BridgeActionResult | null) => {
    setThreadLastResult(storeRef.current, nextThreadId, result);
    notify(nextThreadId);
  }, [notify]);
  const setHistoryExpanded = useCallback((nextThreadId: string, expanded: boolean) => {
    setThreadHistoryExpanded(storeRef.current, nextThreadId, expanded);
    notify(nextThreadId);
  }, [notify]);
  const setHiddenActionKeyForThread = useCallback((nextThreadId: string, key: string | null) => {
    setThreadHiddenActionKey(storeRef.current, nextThreadId, key);
    notify(nextThreadId);
  }, [notify]);
  const clearThread = useCallback((nextThreadId: string) => {
    const wasActive = storeRef.current.activeThreadId === nextThreadId;
    clearThreadSlot(storeRef.current, nextThreadId);
    notify(wasActive ? null : nextThreadId);
  }, [notify]);

  return useMemo(() => ({
    store: storeRef.current,
    setActive,
    getSlot: getSlotForThread,
    isActive,
    applyDetail,
    applySummary,
    patchSummary,
    applyRealtimeBlocks,
    applyBlockPage,
    setLoadingEarlier,
    setFeedback: setFeedbackForThread,
    setLastResult: setLastResultForThread,
    setHistoryExpanded,
    setHiddenActionKey: setHiddenActionKeyForThread,
    clear: clearThread
  }), [
    setActive,
    getSlotForThread,
    isActive,
    applyDetail,
    applySummary,
    patchSummary,
    applyRealtimeBlocks,
    applyBlockPage,
    setLoadingEarlier,
    setFeedbackForThread,
    setLastResultForThread,
    setHistoryExpanded,
    setHiddenActionKeyForThread,
    clearThread
  ]);
}

function Conversation({ threadId, detail, slot, messageStore, csrfToken, onSelect, onPanelSelect }: {
  threadId: string;
  detail: ThreadDetail;
  slot: ThreadMessageSlot;
  messageStore: ThreadMessageStoreController;
  csrfToken?: string | null;
  onSelect: (id: SelectedThread) => void;
  onPanelSelect: (view: View) => void;
}) {
  const qc = useQueryClient();
  const messageStreamRef = useRef<HTMLDivElement | null>(null);
  const messageEndRef = useRef<HTMLDivElement | null>(null);
  const composerTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const shouldFollowMessagesRef = useRef(true);
  const previousThreadIdRef = useRef(threadId);
  const [explicitBottomFollowRevision, setExplicitBottomFollowRevision] = useState(0);
  const [draft, setDraft] = useState("");
  const runOptions = useCodexRunOptions();
  const pluginsQuery = useQuery({ queryKey: ["plugins"], queryFn: listPlugins, staleTime: 30000 });
  const [runConfig, setRunConfig] = useState<RunConfig>(() => makeRunConfig(undefined, detail.summary));
  const [renameValue, setRenameValue] = useState(detail.summary.title);
  const [renameDirty, setRenameDirty] = useState(false);
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

  useEffect(() => {
    const unsubscribe = subscribeThreadEvents(threadId, {
      onBlocks: (incomingBlocks, eventThreadId) => {
        if (messageStore.isActive(eventThreadId)) {
          updateMessageFollowState();
        }
        messageStore.applyRealtimeBlocks(eventThreadId, incomingBlocks);
      },
      onSummary: (next, eventThreadId) => {
        messageStore.applySummary(eventThreadId, next);
        updateThreadListCaches(qc, next);
        qc.invalidateQueries({ queryKey: ["threads"] });
      },
      onError: (message, eventThreadId) => {
        messageStore.setFeedback(eventThreadId, message);
        qc.invalidateQueries({ queryKey: ["thread", eventThreadId], refetchType: "all" });
        qc.invalidateQueries({ queryKey: ["threads"], refetchType: "all" });
      }
    });
    return unsubscribe;
  }, [messageStore, qc, threadId, updateMessageFollowState]);

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
  const followUps = useQuery({
    queryKey: ["thread-followups", summary.id],
    queryFn: () => listFollowUps(summary.id),
    refetchInterval: running ? 3000 : 8000
  });
  const followUpItems = followUps.data?.items ?? [];
  const payloadRunConfig = useMemo(
    () => runConfigWithSupportedServiceTier(runConfig, runOptions.models),
    [runConfig, runOptions.models]
  );
  const loadEarlierMutation = useMutation({
    mutationFn: async ({ threadId: requestThreadId, cursor }: { threadId: string; cursor: string }) => {
      if (!cursor) throw new Error("没有更早的消息");
      const beforeHeight = messageStreamRef.current?.scrollHeight ?? 0;
      messageStore.setLoadingEarlier(requestThreadId, true);
      const page = await getThreadBlocks(requestThreadId, { limit: 120, before: cursor });
      return { threadId: requestThreadId, cursor, page, beforeHeight };
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
    onError: (err: Error, variables) => {
      const failedThreadId = variables?.threadId ?? summary.id;
      messageStore.setLoadingEarlier(failedThreadId, false, err.message);
      messageStore.setFeedback(failedThreadId, err.message);
    }
  });

  const sendMutation = useMutation({
    mutationFn: async ({ threadId: requestThreadId, message, config, uploads }: { threadId: string; message: string; config: RunConfig; uploads: Pick<UploadRecord, "id">[] }) => {
      const payload = buildPayload(message, config, uploads);
      const result = await sendMessage(requestThreadId, payload, csrfToken);
      return { threadId: requestThreadId, result };
    },
    onSuccess: ({ threadId: resultThreadId, result }) => {
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
      qc.invalidateQueries({ queryKey: ["jobs"] });
      qc.invalidateQueries({ queryKey: ["threads"] });
      qc.invalidateQueries({ queryKey: ["thread", resultThreadId] });
    },
    onError: (err: Error, variables) => messageStore.setFeedback(variables?.threadId ?? summary.id, err.message)
  });

  const stopMutation = useMutation({
    mutationFn: async ({ threadId: requestThreadId, turnId, jobId }: { threadId: string; turnId?: string | null; jobId?: string | null }) => {
      await stopThread(requestThreadId, { turn_id: turnId, job_id: jobId }, csrfToken);
      return { threadId: requestThreadId };
    },
    onSuccess: ({ threadId: stoppedThreadId }) => {
      messageStore.setFeedback(stoppedThreadId, "停止请求已发送");
      qc.invalidateQueries({ queryKey: ["threads"] });
      qc.invalidateQueries({ queryKey: ["thread", stoppedThreadId] });
    },
    onError: (err: Error, variables) => messageStore.setFeedback(variables?.threadId ?? summary.id, err.message)
  });

  const steerMutation = useMutation({
    mutationFn: async ({ threadId: requestThreadId, message, config, uploads }: { threadId: string; message: string; config: RunConfig; uploads: Pick<UploadRecord, "id">[] }) => {
      const payload = buildPayload(message, config, uploads);
      const result = await steerThread(requestThreadId, payload, csrfToken);
      return { threadId: requestThreadId, result };
    },
    onSuccess: ({ threadId: resultThreadId, result }) => {
      messageStore.setLastResult(resultThreadId, result);
      if (messageStore.isActive(resultThreadId)) {
        setDraft("");
        attachments.clearUploads();
        setRunConfig((current) => runConfigAfterSuccessfulSend(current));
      }
      messageStore.setFeedback(resultThreadId, result.fallback ? (result.message ?? "已加入跟进队列") : "已跟进当前 turn");
      qc.invalidateQueries({ queryKey: ["thread-followups", resultThreadId] });
      qc.invalidateQueries({ queryKey: ["threads"] });
      qc.invalidateQueries({ queryKey: ["thread", resultThreadId] });
    },
    onError: (err: Error, variables) => messageStore.setFeedback(variables?.threadId ?? summary.id, err.message)
  });

  const cancelFollowUpMutation = useMutation({
    mutationFn: async ({ threadId: requestThreadId, followUpId }: { threadId: string; followUpId: string }) => {
      await cancelFollowUp(requestThreadId, followUpId, csrfToken);
      return { threadId: requestThreadId };
    },
    onSuccess: ({ threadId: cancelledThreadId }) => {
      messageStore.setFeedback(cancelledThreadId, "跟进已取消");
      qc.invalidateQueries({ queryKey: ["thread-followups", cancelledThreadId] });
    },
    onError: (err: Error, variables) => messageStore.setFeedback(variables?.threadId ?? summary.id, err.message)
  });

  const archiveMutation = useMutation({
    mutationFn: async ({ threadId: requestThreadId, status }: { threadId: string; status: ThreadStatus }) => {
      const wasArchived = status === "Archived";
      if (wasArchived) {
        await restoreThread(requestThreadId, csrfToken);
      } else {
        await archiveThread(requestThreadId, csrfToken);
      }
      return { threadId: requestThreadId, wasArchived };
    },
    onSuccess: ({ threadId: archivedThreadId, wasArchived }) => {
      messageStore.setFeedback(archivedThreadId, wasArchived ? "恢复请求已提交" : "归档请求已提交");
      if (wasArchived) {
        qc.invalidateQueries({ queryKey: ["threads"] });
        qc.invalidateQueries({ queryKey: ["thread", archivedThreadId] });
      } else {
        removeThreadFromListCaches(qc, archivedThreadId);
        qc.removeQueries({ queryKey: ["thread", archivedThreadId] });
        messageStore.clear(archivedThreadId);
        onSelect(null);
        qc.invalidateQueries({ queryKey: ["threads"] });
      }
    },
    onError: (err: Error, variables) => messageStore.setFeedback(variables?.threadId ?? summary.id, err.message)
  });

  const renameMutation = useMutation({
    mutationFn: async ({ threadId: requestThreadId, title: requestedTitle }: { threadId: string; title: string }) => {
      const title = requestedTitle.trim();
      await renameThread(requestThreadId, requestedTitle, csrfToken);
      return { threadId: requestThreadId, title };
    },
    onSuccess: ({ threadId: renamedThreadId, title }) => {
      messageStore.setFeedback(renamedThreadId, "线程名称已更新");
      if (title) {
        setRenameValue(title);
        setRenameDirty(false);
        messageStore.patchSummary(renamedThreadId, { title });
        updateSavedThreadTitleCaches(qc, renamedThreadId, title);
      }
      qc.invalidateQueries({ queryKey: ["threads"] });
      qc.invalidateQueries({ queryKey: ["thread", renamedThreadId] });
    },
    onError: (err: Error, variables) => messageStore.setFeedback(variables?.threadId ?? summary.id, err.message)
  });

  const forkMutation = useMutation({
    mutationFn: async ({ threadId: requestThreadId }: { threadId: string }) => {
      const result = await forkThread(requestThreadId, csrfToken);
      return { threadId: requestThreadId, result };
    },
    onSuccess: ({ threadId: forkedThreadId, result }) => {
      messageStore.setLastResult(forkedThreadId, result);
      messageStore.setFeedback(forkedThreadId, actionMessage(result));
      if (result.thread_id) onSelect(result.thread_id);
      qc.invalidateQueries({ queryKey: ["threads"] });
    },
    onError: (err: Error, variables) => messageStore.setFeedback(variables?.threadId ?? summary.id, err.message)
  });

  const answerMutation = useMutation({
    mutationFn: async ({ threadId: requestThreadId, answers }: { threadId: string; answers: Record<string, string[]> }) => {
      const result = await answerElicitation(requestThreadId, answers, csrfToken);
      return { threadId: requestThreadId, result };
    },
    onSuccess: ({ threadId: answeredThreadId, result }) => {
      messageStore.setLastResult(answeredThreadId, result);
      messageStore.setFeedback(answeredThreadId, actionMessage(result));
      qc.invalidateQueries({ queryKey: ["threads"] });
    },
    onError: (err: Error, variables) => messageStore.setFeedback(variables?.threadId ?? summary.id, err.message)
  });

  const planAcceptMutation = useMutation({
    mutationFn: async ({ threadId: requestThreadId, block }: { threadId: string; block: MessageBlock }) => {
      const result = await acceptPlan(requestThreadId, { turn_id: block.turn_id, item_id: block.item_id }, csrfToken);
      return { threadId: requestThreadId, result };
    },
    onSuccess: ({ threadId: planThreadId, result }) => {
      messageStore.setLastResult(planThreadId, result);
      messageStore.setFeedback(planThreadId, actionMessage(result));
      qc.invalidateQueries({ queryKey: ["threads"] });
      qc.invalidateQueries({ queryKey: ["thread", planThreadId] });
    },
    onError: (err: Error, variables) => messageStore.setFeedback(variables?.threadId ?? summary.id, err.message)
  });

  const planReviseMutation = useMutation({
    mutationFn: async ({ threadId: requestThreadId, block, instructions }: { threadId: string; block: MessageBlock; instructions: string }) => {
      const result = await revisePlan(requestThreadId, { turn_id: block.turn_id, item_id: block.item_id, instructions }, csrfToken);
      return { threadId: requestThreadId, result };
    },
    onSuccess: ({ threadId: planThreadId, result }) => {
      messageStore.setLastResult(planThreadId, result);
      messageStore.setFeedback(planThreadId, actionMessage(result));
      qc.invalidateQueries({ queryKey: ["threads"] });
      qc.invalidateQueries({ queryKey: ["thread", planThreadId] });
    },
    onError: (err: Error, variables) => messageStore.setFeedback(variables?.threadId ?? summary.id, err.message)
  });

  const approvalMutation = useMutation({
    mutationFn: async ({ threadId: requestThreadId, block, decision }: { threadId: string; block: MessageBlock; decision: string }) => {
      const result = await answerApproval(requestThreadId, {
        turn_id: block.turn_id,
        item_id: block.item_id ?? block.call_id,
        decision
      }, csrfToken);
      return { threadId: requestThreadId, result };
    },
    onSuccess: ({ threadId: approvalThreadId, result }) => {
      messageStore.setLastResult(approvalThreadId, result);
      messageStore.setFeedback(approvalThreadId, actionMessage(result));
      qc.invalidateQueries({ queryKey: ["threads"] });
      qc.invalidateQueries({ queryKey: ["thread", approvalThreadId] });
    },
    onError: (err: Error, variables) => messageStore.setFeedback(variables?.threadId ?? summary.id, err.message)
  });
  const executeSlashCommand = useCallback((command: string) => {
    const action = slashCommandAction(command, Boolean(threadId));
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
        archiveMutation.mutate({ threadId: summary.id, status: summary.status });
        break;
      case "fork_thread":
        setDraft("");
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
      case "resume_goal":
        setDraft("");
        resumeGoalMode(summary.id, csrfToken).then((result) => {
          messageStore.setFeedback(threadId, result.available ? "Goal 已恢复" : "Goal API 不可用");
          qc.invalidateQueries({ queryKey: ["codex-goal", threadId] });
        }).catch((err: Error) => messageStore.setFeedback(threadId, err.message));
        break;
      case "clear_goal":
        setDraft("");
        clearGoalMode(summary.id, csrfToken).then((result) => {
          messageStore.setFeedback(threadId, result.available ? "Goal 已清除" : "Goal API 不可用");
          qc.invalidateQueries({ queryKey: ["codex-goal", threadId] });
        }).catch((err: Error) => messageStore.setFeedback(threadId, err.message));
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
  }, [archiveMutation, blocks, csrfToken, forkMutation, lastResult?.job_id, lastResult?.turn_id, messageStore, onPanelSelect, onSelect, qc, runConfig, runOptions.models, stopMutation, summary.id, summary.status, summary.active_job_id, summary.active_turn_id, threadId]);

  const loadEarlierPending = slot.loadingEarlier;
  const sendPending = sendMutation.isPending && sendMutation.variables?.threadId === summary.id;
  const stopPending = stopMutation.isPending && stopMutation.variables?.threadId === summary.id;
  const steerPending = steerMutation.isPending && steerMutation.variables?.threadId === summary.id;
  const cancelFollowUpPending = cancelFollowUpMutation.isPending && cancelFollowUpMutation.variables?.threadId === summary.id;
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
    const exactSlash = slashCommandForComposerSubmit(currentDraft);
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
  }, [actionMode, attachments, draft, executeSlashCommand, followNextMessageUpdate, lastResult?.job_id, lastResult?.turn_id, payloadRunConfig, sendMutation, sendPending, steerMutation, steerPending, stopMutation, stopPending, summary.active_job_id, summary.active_turn_id, summary.id]);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    submitComposer();
  };
  const conversationTitle = conversationTitleText(summary);
  const actionBusy = sendPending || stopPending || steerPending || attachments.uploadInProgress;
  const actionLabel = composerActionLabel(actionMode);
  const actionTitle = composerActionTitle(actionMode);

  return (
    <div className="conversation-shell">
      <div className="conversation-main">
        <header className="conversation-header">
          <div className="conversation-title-copy">
            <span className="eyebrow">{summary.cwd || "/root/.codex"}</span>
            <h2 className="conversation-title" title={conversationTitle}>{conversationTitle}</h2>
          </div>
          <div className="header-actions">
            <StatusChip status={summary.status} />
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

        {summary.status === "ReplyNeeded" && (
          <div className="reply-banner">
            <TriangleAlert size={18} />
            <span>{pending ? "Plan Mode 正在等待选择。" : "Plan Mode 正在等待确认。"}</span>
          </div>
        )}
        {feedback && <div className="feedback-banner">{feedback}</div>}

        {approvalBlock && (
          <div className="action-stack">
            {approvalBlock && (
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
            placeholder={summary.status === "ReplyNeeded" ? "输入选择编号、确认语句或补充要求" : "发送到 root Codex app-server"}
            hasThread
            plugins={pluginsQuery.data ?? []}
            pluginsUnavailable={pluginsQuery.isError}
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
              onCancel={(item) => cancelFollowUpMutation.mutate({ threadId: summary.id, followUpId: item.id })}
              cancelling={cancelFollowUpPending}
            />
          )}
          <div className="composer-actions">
            <span>{lastResult ? actionMessage(lastResult) : `CSRF ${csrfToken ? "已就绪" : "未恢复"}`}</span>
            <button className="primary-button composer-action-button" disabled={actionMode === "disabled" || actionBusy} title={actionTitle}>
              {actionMode === "stop" ? <Square size={17} /> : actionMode === "followup" ? <Goal size={17} /> : <Send size={17} />}
              {actionLabel}
            </button>
          </div>
        </form>
      </div>

      <aside className="conversation-inspector">
        <Panel title="线程设置" icon={<SlidersHorizontal size={18} />}>
          <div className="copy-row">
            <button
              className="secondary-button"
              onClick={() => {
                const command = threadResumeCommand(summary.id);
                if (!command) return;
                navigator.clipboard?.writeText(command);
                setActiveFeedback("已复制恢复命令");
              }}
              disabled={!threadResumeCommand(summary.id)}
            >
              <Copy size={17} />复制 ID
            </button>
            <button
              className="secondary-button"
              onClick={() => {
                if (!summary.rollout_path) return;
                navigator.clipboard?.writeText(summary.rollout_path);
                setActiveFeedback("已复制线程文件绝对路径");
              }}
              disabled={!summary.rollout_path}
            >
              <Copy size={17} />复制路径
            </button>
            <button className="secondary-button" onClick={() => forkMutation.mutate({ threadId: summary.id })} disabled={forkPending}><GitFork size={17} />Fork</button>
          </div>
        </Panel>

        <Panel title="名称与归档" icon={<Edit3 size={18} />}>
          <label className="field-label">线程标题<input value={renameValue} onChange={(event) => {
            setRenameDirty(true);
            setRenameValue(event.target.value);
          }} /></label>
          <div className="button-row">
            <button className="secondary-button" onClick={() => renameMutation.mutate({ threadId: summary.id, title: renameValue })} disabled={!renameValue.trim() || renamePending}><Edit3 size={17} />重命名</button>
            <button className={summary.status === "Archived" ? "secondary-button" : "danger-button soft"} onClick={() => archiveMutation.mutate({ threadId: summary.id, status: summary.status })} disabled={archivePending}>
              {summary.status === "Archived" ? <Undo2 size={17} /> : <Archive size={17} />}
              {summary.status === "Archived" ? "恢复" : "归档"}
            </button>
          </div>
        </Panel>

        <GoalCard threadId={summary.id} csrfToken={csrfToken} />

        <Panel title="运行路径" icon={<HardDrive size={18} />}>
          <Metric label="Workspace" value={runConfig.cwd || defaultCwd} />
          <Metric label="Model" value={runConfig.model || "default"} />
          <Metric label="Reasoning" value={runConfig.reasoning || "default"} />
          <Metric label="Permissions" value={permissionLabel(runConfig.permissionPreset)} />
          <Metric label="Network" value="enabled" tone="success" />
        </Panel>
      </aside>
    </div>
  );
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
        <span className="composer-chip muted"><Goal size={15} />Pursue Goal</span>
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
        <label className="cwd-field">
          <span>CWD</span>
          <input value={config.cwd} onChange={(event) => setConfig({ ...config, cwd: event.target.value })} />
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

function GoalCard({ threadId, csrfToken }: { threadId: string; csrfToken?: string | null }) {
  const qc = useQueryClient();
  const goal = useQuery({ queryKey: ["codex-goal", threadId], queryFn: () => getGoalMode(threadId), refetchInterval: 8000 });
  const goalState = goal.data?.available ? goal.data.data : null;
  const [objective, setObjective] = useState(goalState?.objective ?? "");
  const [tokenBudget, setTokenBudget] = useState(goalState?.token_budget ? String(goalState.token_budget) : "");
  const [feedback, setFeedback] = useState<string | null>(null);

  useEffect(() => {
    if (!goalState) return;
    setObjective(goalState.objective ?? "");
    setTokenBudget(goalState.token_budget ? String(goalState.token_budget) : "");
  }, [goalState?.objective, goalState?.token_budget, goalState]);

  const saveGoal = useMutation({
    mutationFn: () => setGoalMode({
      enabled: Boolean(objective.trim()),
      objective: objective.trim() || null,
      token_budget: tokenBudget.trim() ? Number(tokenBudget) : null
    }, threadId, csrfToken),
    onSuccess: (result) => {
      setFeedback(result.available ? "Goal 已保存" : "Goal API 不可用");
      qc.invalidateQueries({ queryKey: ["codex-goal", threadId] });
    },
    onError: (err: Error) => setFeedback(err.message)
  });
  const clearGoal = useMutation({
    mutationFn: () => clearGoalMode(threadId, csrfToken),
    onSuccess: (result) => {
      setFeedback(result.available ? "Goal 已清除" : "Goal API 不可用");
      setObjective("");
      setTokenBudget("");
      qc.invalidateQueries({ queryKey: ["codex-goal", threadId] });
    },
    onError: (err: Error) => setFeedback(err.message)
  });
  const resumeGoal = useMutation({
    mutationFn: () => resumeGoalMode(threadId, csrfToken),
    onSuccess: (result) => {
      setFeedback(result.available ? "Goal 已恢复" : "Goal API 不可用");
      qc.invalidateQueries({ queryKey: ["codex-goal", threadId] });
    },
    onError: (err: Error) => setFeedback(err.message)
  });

  return (
    <Panel title="Goal" icon={<Goal size={18} />}>
      {goal.data && !goal.data.available ? (
        <div className="config-note">Goal API 不可用</div>
      ) : (
        <>
          <Metric label="状态" value={formatGoalStatus(goalState)} tone={goalState?.enabled ? "success" : undefined} />
          <label className="field-label">目标<textarea value={objective} onChange={(event) => setObjective(event.target.value)} placeholder="本线程要持续追踪的目标" /></label>
          <label className="field-label">Token budget<input type="number" min={1} value={tokenBudget} onChange={(event) => setTokenBudget(event.target.value)} placeholder="可选" /></label>
          {feedback && <div className="config-note">{feedback}</div>}
          <div className="button-row">
            <button className="secondary-button" onClick={() => saveGoal.mutate()} disabled={saveGoal.isPending || (!objective.trim() && !tokenBudget.trim())}><CheckCircle2 size={17} />保存</button>
            <button className="danger-button soft" onClick={() => clearGoal.mutate()} disabled={clearGoal.isPending}><Trash2 size={17} />清除</button>
            <button className="secondary-button" onClick={() => resumeGoal.mutate()} disabled={resumeGoal.isPending || !threadId}><Play size={17} />恢复</button>
          </div>
        </>
      )}
    </Panel>
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

function EmptyConversation({ loading, csrfToken, onCreated }: { loading: boolean; csrfToken?: string | null; onCreated: (id: string) => void }) {
  const qc = useQueryClient();
  const composerTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const [draft, setDraft] = useState("");
  const runOptions = useCodexRunOptions();
  const pluginsQuery = useQuery({ queryKey: ["plugins"], queryFn: listPlugins, staleTime: 30000 });
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
  const mutation = useMutation({
    mutationFn: ({ message }: { message: string }) => createThread(buildPayload(message, payloadRunConfig, attachments.readyUploads), csrfToken),
    onSuccess: (next) => {
      setResult(next);
      setDraft("");
      attachments.clearUploads();
      setRunConfig((current) => runConfigAfterSuccessfulSend(current));
      qc.invalidateQueries({ queryKey: ["threads"] });
      qc.invalidateQueries({ queryKey: ["jobs"] });
      if (next.thread_id) onCreated(next.thread_id);
    },
    onError: (err: Error) => setFeedback(err.message)
  });
  const executeSlashCommand = (command: string) => {
    const action = slashCommandAction(command, false);
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
    const exactSlash = slashCommandForComposerSubmit(currentDraft);
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
      <span>通过受控 app-server bridge 启动，失败时自动降级为 codex exec job。</span>
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
  const options = isPlan ? currentPlanActionOptions() : questions[0]?.options ?? [];
  const selectedPlanRequiresRevision = isPlan && selected === 1;
  const ready = isPlan
    ? Boolean(plan && planActionSubmission(selected, revision))
    : questionAnswersReady(questions, questionAnswers);

  function submitAction() {
    if (busy || !ready) return;
    if (plan) {
      const submission = planActionSubmission(selected, revision);
      if (!submission) return;
      if (submission.action === "accept") {
        onAcceptPlan(plan);
      } else {
        onRevisePlan(plan, submission.instructions);
      }
      return;
    }
    if (pending) onSubmitQuestion(questionAnswerPayload(questions, questionAnswers));
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
  }, [busy, isPlan, onDismiss, options.length, questions, ready, revision, selected, questionAnswers]);

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

function ClaudeWorkspace() {
  const providers = useQuery({ queryKey: ["providers"], queryFn: listProviders, refetchInterval: 30000 });
  const overview = useQuery({ queryKey: ["claude-code-overview"], queryFn: getClaudeCodeOverview, refetchInterval: 30000 });
  const platform = useQuery({ queryKey: ["platform-overview"], queryFn: getPlatformOverview, refetchInterval: 30000 });
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

function ProbeWorkspace({ csrfToken }: { csrfToken?: string | null }) {
  const qc = useQueryClient();
  const status = useQuery({ queryKey: ["probe-status"], queryFn: getProbeStatus, refetchInterval: 15000 });
  const settings = useQuery({ queryKey: ["probe-settings"], queryFn: getProbeSettings, refetchInterval: 30000 });
  const logsDbStatus = useQuery({ queryKey: ["probe-logs-db-status"], queryFn: getProbeLogsDbStatus, refetchInterval: 30000 });
  const events = useQuery({ queryKey: ["probe-events"], queryFn: () => getProbeEvents(10), refetchInterval: 15000 });
  const jobs = useQuery({ queryKey: ["jobs"], queryFn: listJobs, refetchInterval: 5000 });
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
  const barkConfigured = Boolean(currentSettings?.notifications.device_key_configured || draft?.notifications.device_key_configured);
  const probeThreads = probeThreadsByStatus(data);
  const probeEnabled = data?.enabled ?? currentSettings?.probe.enabled ?? false;
  const serviceText = data ? `${data.service_kind}:${data.service_name}` : "未知";
  const statusTone = available && probeEnabled ? "success" : available ? "warning" : "danger";
  const probeJobs = (jobs.data ?? []).filter(isProbeJob).slice(0, 6);
  const probeJobMutation = useMutation({
    mutationFn: (action: ProbeJobAction) => startProbeJob(action, csrfToken),
    onSuccess: (_result, action) => {
      setActionStatus({ tone: "success", message: `${probeJobActionLabel(action)} 已加入 Job History` });
      if (action === "logs-db-dry-run") setLogsDbExecuteArmed(true);
      if (action === "logs-db-execute") setLogsDbExecuteArmed(false);
      qc.invalidateQueries({ queryKey: ["jobs"] });
      qc.invalidateQueries({ queryKey: ["probe-status"] });
      qc.invalidateQueries({ queryKey: ["probe-logs-db-status"] });
      qc.invalidateQueries({ queryKey: ["probe-events"] });
    },
    onError: (err: Error, action) => {
      setActionStatus({ tone: "error", message: `${probeJobActionLabel(action)} 失败: ${err.message}` });
    }
  });
  const pendingProbeAction = probeJobMutation.isPending ? probeJobMutation.variables : null;
  const saveMutation = useMutation({
    mutationFn: async () => {
      if (!draft) throw new Error("探针设置尚未载入");
      const errors = probeSettingsValidation(draft);
      if (errors.length) throw new Error(errors[0]);
      return saveProbeSettings(buildProbeSettingsPayload(draft, currentSettings), csrfToken);
    },
    onSuccess: (saved) => {
      if (!isProbeSettings(saved)) {
        setSaveStatus({ tone: "error", message: "保存响应结构异常，已保留当前输入" });
        return;
      }
      setSaveStatus({ tone: "success", message: "设置已保存" });
      setDraft(buildProbeSettingsDraft(saved));
      qc.invalidateQueries({ queryKey: ["probe-settings"] });
      qc.invalidateQueries({ queryKey: ["probe-status"] });
      qc.invalidateQueries({ queryKey: ["probe-logs-db-status"] });
      qc.invalidateQueries({ queryKey: ["probe-events"] });
    },
    onError: (err: Error) => {
      setSaveStatus({ tone: "error", message: err.message });
    }
  });

  useEffect(() => {
    if (!currentSettings || draft) return;
    setDraft(buildProbeSettingsDraft(currentSettings));
  }, [currentSettings, draft]);

  const overviewSection = (
    <>
      <section className="probe-core-metrics" aria-label="探针核心指标">
        <Metric label="Codex APP" value={probeEnabled ? "运行中" : available ? "停用" : "不可用"} tone={statusTone === "danger" ? "danger" : statusTone} />
        <Metric label="运行中" value={String(data?.running_count ?? 0)} tone={(data?.running_count ?? 0) > 0 ? "success" : undefined} />
        <Metric label="需回复" value={String(data?.reply_needed_count ?? 0)} tone={(data?.reply_needed_count ?? 0) > 0 ? "warning" : undefined} />
        <Metric label="异常数" value={String(data?.recoverable_count ?? 0)} tone={(data?.recoverable_count ?? 0) > 0 ? "danger" : undefined} />
        <Metric label="Bark" value={barkConfigured ? "已配置" : "未配置"} tone={barkConfigured ? "success" : "warning"} />
        <Metric label="Hook 事件" value={String(data?.recent_event_count ?? recentEvents.length)} tone={(data?.recent_event_count ?? recentEvents.length) > 0 ? "success" : undefined} />
        <Metric label="日志库" value={probeStateLabel(logsDbStatusText)} tone={logsDbTone} />
        <Metric label="Codex Home" value={codexHomeStatusValue(data ?? currentSettings?.codex)} />
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
                onSave={() => saveMutation.mutate()}
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
              busy={probeJobMutation.isPending}
              executeArmed={logsDbExecuteArmed}
              onDryRun={() => probeJobMutation.mutate("logs-db-dry-run")}
              onArmExecute={() => setLogsDbExecuteArmed(true)}
              onCancelExecute={() => setLogsDbExecuteArmed(false)}
              onExecute={() => probeJobMutation.mutate("logs-db-execute")}
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
                  onSave={() => saveMutation.mutate()}
                />
              ) : (
                <div className="muted-row">{settings.isLoading ? "正在读取设置" : "设置不可用"}</div>
              )}
            </Panel>
            <Panel title="Probe Job History" icon={<TerminalSquare size={18} />} className="wide-panel">
              <JobList jobs={probeJobs} />
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
          <button className="secondary-button" onClick={() => {
            qc.invalidateQueries({ queryKey: ["probe-status"] });
            qc.invalidateQueries({ queryKey: ["probe-settings"] });
            qc.invalidateQueries({ queryKey: ["probe-logs-db-status"] });
            qc.invalidateQueries({ queryKey: ["probe-events"] });
          }}><RefreshCw size={17} />刷新</button>
          <button className="secondary-button" onClick={() => probeJobMutation.mutate("bark-test")} disabled={!barkConfigured || probeJobMutation.isPending}><Cloud size={17} />测试 Bark</button>
        </div>
      </div>

      <section className={`probe-status-banner tone-${statusTone}`}>
        <div>
          <strong>{available ? probeEnabled ? "Probe 正在接管云机观测" : "Probe 已停用" : "Probe 端点不可用"}</strong>
          <span>{serviceText} · {data?.host_label ?? currentSettings?.codex.host_label ?? "未知主机"}</span>
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

      {!available && (
        <Panel title="端点" icon={<TriangleAlert size={18} />} className="wide-panel">
          <div className="muted-row">探针端点不可用</div>
        </Panel>
      )}
    </div>
  );
}

function OpsWorkspace({ csrfToken }: { csrfToken?: string | null }) {
  const qc = useQueryClient();
  const status = useQuery({ queryKey: ["system-status"], queryFn: getSystemStatus, refetchInterval: 8000 });
  const version = useQuery({ queryKey: ["system-version"], queryFn: getSystemVersion, refetchInterval: 30000 });
  const jobs = useQuery({ queryKey: ["jobs"], queryFn: listJobs, refetchInterval: 5000 });
  const [plan, setPlan] = useState<ArchiveDeletePlan | null>(null);
  const [hiddenPlan, setHiddenPlan] = useState<HiddenThreadDeletePlan | null>(null);
  const [deleteArmed, setDeleteArmed] = useState(false);
  const [hiddenDeleteArmed, setHiddenDeleteArmed] = useState(false);
  const jobMutation = useMutation({ mutationFn: ({ target, action }: { target: "panel" | "codex"; action: "precheck" | "start" | "prune" }) => startUpdateJob(target, action, csrfToken), onSuccess: () => qc.invalidateQueries({ queryKey: ["jobs"] }) });
  const claudeJobMutation = useMutation({ mutationFn: (action: Parameters<typeof startClaudeCodeJob>[0]) => startClaudeCodeJob(action, csrfToken), onSuccess: () => qc.invalidateQueries({ queryKey: ["jobs"] }) });
  const dryRun = useMutation({ mutationFn: () => dryRunArchiveDelete(csrfToken), onSuccess: setPlan });
  const executeDelete = useMutation({ mutationFn: () => startArchiveDelete(csrfToken), onSuccess: () => {
    setDeleteArmed(false);
    qc.invalidateQueries({ queryKey: ["jobs"] });
    qc.invalidateQueries({ queryKey: ["system-status"] });
    qc.invalidateQueries({ queryKey: ["threads"] });
  } });
  const hiddenDryRun = useMutation({ mutationFn: () => dryRunHiddenThreadDelete(csrfToken), onSuccess: setHiddenPlan });
  const executeHiddenDelete = useMutation({ mutationFn: () => startHiddenThreadDelete(csrfToken), onSuccess: (result) => {
    setHiddenDeleteArmed(false);
    setHiddenPlan((current) => current ? { ...current, hidden_threads: result.hidden_threads, hidden_ids: [], hidden_source_counts: {} } : current);
    qc.invalidateQueries({ queryKey: ["jobs"] });
    qc.invalidateQueries({ queryKey: ["system-status"] });
    qc.invalidateQueries({ queryKey: ["threads"] });
  } });
  const publicEndpoint = cleanHostValue(status.data?.public_endpoint);
  const hostname = cleanHostValue(status.data?.hostname) ?? "读取中";
  const hiddenStats = hiddenThreadDeleteStats(hiddenPlan, status.data);

  return (
    <div className="ops-grid">
      <Panel title="系统状态" icon={<HardDrive size={18} />}>
        <Metric label="Hostname" value={hostname} />
        <Metric label="Public endpoint" value={publicEndpoint ?? "未配置"} tone={publicEndpoint ? "success" : "warning"} />
        <Metric label="app-server" value={status.data?.app_server_service.active ? "active/running" : "inactive"} tone={status.data?.app_server_service.active ? "success" : "danger"} />
        <Metric label="state DB" value={status.data?.state_db_integrity ?? "unknown"} tone={status.data?.state_db_integrity === "ok" ? "success" : "warning"} />
        <Metric label="Codex Home" value={codexHomeStatusValue(status.data)} />
        <Metric label="State DB" value={status.data?.state_db ?? "unknown"} />
        <Metric label="Socket" value={status.data?.app_server_socket ?? "unknown"} />
        <Metric label="Hidden threads" value={String(status.data?.hidden_thread_count ?? 0)} tone={(status.data?.hidden_thread_count ?? 0) > 0 ? "warning" : undefined} />
        <Metric label="Sources" value={sourceCountsText(status.data?.thread_source_counts)} />
        <Metric label="app sources" value={sourceCountsText(status.data?.app_server_source_counts)} />
      </Panel>
      <Panel title="面板更新" icon={<RefreshCw size={18} />}>
        <PanelVersionMetrics version={version.data} />
        <div className="button-row">
          <button className="secondary-button" onClick={() => jobMutation.mutate({ target: "panel", action: "precheck" })}><CheckCircle2 size={17} />Precheck</button>
          <button className="primary-button" onClick={() => jobMutation.mutate({ target: "panel", action: "start" })}><Play size={17} />Update</button>
          <button className="danger-button soft" onClick={() => jobMutation.mutate({ target: "panel", action: "prune" })}><Trash2 size={17} />Prune</button>
        </div>
      </Panel>
      <Panel title="Codex 更新" icon={<Bot size={18} />}>
        <CodexVersionMetrics version={version.data} />
        <div className="button-row">
          <button className="secondary-button" onClick={() => jobMutation.mutate({ target: "codex", action: "precheck" })}><CheckCircle2 size={17} />Precheck</button>
          <button className="primary-button" onClick={() => jobMutation.mutate({ target: "codex", action: "start" })}><Play size={17} />一键更新</button>
          <button className="danger-button soft" onClick={() => jobMutation.mutate({ target: "codex", action: "prune" })}><Trash2 size={17} />单独清理旧版本</button>
        </div>
      </Panel>
      <Panel title="Claude Code 维护" icon={<Bot size={18} />}>
        <div className="button-row">
          <button className="secondary-button" disabled={claudeJobMutation.isPending} onClick={() => claudeJobMutation.mutate("version-check")}><CheckCircle2 size={17} />版本检查</button>
          <button className="secondary-button" disabled={claudeJobMutation.isPending} onClick={() => claudeJobMutation.mutate("update-precheck")}><ClipboardCheck size={17} />更新预检</button>
          <button className="primary-button" disabled={claudeJobMutation.isPending} onClick={() => claudeJobMutation.mutate("update-start")}><Play size={17} />开始更新</button>
          <button className="secondary-button" disabled={claudeJobMutation.isPending} onClick={() => claudeJobMutation.mutate("smoke")}><TerminalSquare size={17} />Smoke</button>
          <button className="secondary-button" disabled={claudeJobMutation.isPending} onClick={() => claudeJobMutation.mutate("cache-status")}><Database size={17} />缓存状态</button>
        </div>
      </Panel>
      <Panel title="归档清理" icon={<Archive size={18} />}>
        <div className="button-row">
          <button className="secondary-button" onClick={() => dryRun.mutate()}><Database size={17} />Dry-run</button>
          <button className="secondary-button" onClick={() => hiddenDryRun.mutate()}><Database size={17} />扫描隐藏线程</button>
        </div>
        <div className="archive-cleanup-grid">
          <div className="archive-plan cleanup-section">
            <div className="cleanup-section-title">
              <strong>归档线程</strong>
              <span>删除 archived 线程与 rollout</span>
            </div>
            <Metric label="active" value={String(plan?.active_threads ?? "dry-run 未执行")} />
            <Metric label="archived" value={String(plan?.archived_threads ?? 0)} tone={(plan?.archived_threads ?? 0) > 0 ? "warning" : undefined} />
            <Metric label="integrity" value={plan?.integrity ?? status.data?.state_db_integrity ?? "unknown"} tone={(plan?.integrity ?? status.data?.state_db_integrity) === "ok" ? "success" : "danger"} />
            <div className="button-row">
              {!deleteArmed ? (
                <button className="danger-button soft" disabled={(plan?.archived_threads ?? 0) === 0} onClick={() => setDeleteArmed(true)}><Trash2 size={17} />清理归档</button>
              ) : (
                <>
                  <button className="danger-button" onClick={() => executeDelete.mutate()} disabled={executeDelete.isPending}><Trash2 size={17} />确认清理</button>
                  <button className="secondary-button" onClick={() => setDeleteArmed(false)}>取消</button>
                </>
              )}
            </div>
          </div>
          <div className="archive-plan cleanup-section">
            <div className="cleanup-section-title">
              <strong>隐藏线程</strong>
              <span>删除 non-archived subagent/internal</span>
            </div>
            <Metric label="visible" value={hiddenPlan ? String(hiddenStats.visible) : "dry-run 未执行"} />
            <Metric label="hidden" value={String(hiddenStats.hidden)} tone={hiddenStats.hidden > 0 ? "warning" : undefined} />
            <Metric label="sources" value={hiddenStats.sourceCounts} />
            <Metric label="integrity" value={hiddenStats.integrity} tone={hiddenStats.integrity === "ok" ? "success" : "danger"} />
            <div className="button-row">
              {!hiddenDeleteArmed ? (
                <button className="danger-button soft" disabled={!canStartHiddenThreadDelete(hiddenPlan)} onClick={() => setHiddenDeleteArmed(true)}><Trash2 size={17} />清理隐藏线程</button>
              ) : (
                <>
                  <button className="danger-button" onClick={() => executeHiddenDelete.mutate()} disabled={executeHiddenDelete.isPending}><Trash2 size={17} />确认清理隐藏</button>
                  <button className="secondary-button" onClick={() => setHiddenDeleteArmed(false)}>取消</button>
                </>
              )}
            </div>
          </div>
        </div>
      </Panel>
      <Panel title="Job History" icon={<TerminalSquare size={18} />} className="wide-panel">
        <JobList jobs={jobs.data ?? []} />
      </Panel>
    </div>
  );
}

function PanelVersionMetrics({ version }: { version?: SystemVersion }) {
  return (
    <div className="version-grid">
      <Metric label="Current" value={version?.panel_current ?? "读取中"} />
      <Metric
        label="Latest"
        value={version?.panel_latest ?? "unknown"}
        tone={version?.panel_update_available ? "warning" : "success"}
      />
      <Metric label="Update" value={version?.panel_update_available ? "available" : "current"} tone={version?.panel_update_available ? "warning" : "success"} />
    </div>
  );
}

function CodexVersionMetrics({ version }: { version?: SystemVersion }) {
  const updateState = codexUpdateState(version);
  return (
    <div className="version-grid">
      <Metric label="Current" value={version?.codex_current ?? version?.codex_root ?? "读取中"} />
      <Metric label="Latest" value={version?.codex_latest ?? "unknown"} tone={updateState.tone} />
      <Metric label="Update" value={updateState.label} tone={updateState.tone} />
      <Metric label="root codex" value={version?.codex_root ?? "unknown"} />
      <Metric label="user codex" value={version?.codex_user ?? "unknown"} />
    </div>
  );
}

function SecurityWorkspace({ csrfToken, username }: { csrfToken?: string | null; username: string }) {
  const qc = useQueryClient();
  const security = useQuery({ queryKey: ["security"], queryFn: getSecurity });
  const [draft, setDraft] = useState<Partial<SecuritySettings> & { turnstile_secret_key?: string }>({});
  const [passwordForm, setPasswordForm] = useState({ current: "", next: "", confirm: "" });
  const [passwordFeedback, setPasswordFeedback] = useState<string | null>(null);
  const mutation = useMutation({
    mutationFn: () => saveSecurity(draft, csrfToken),
    onSuccess: () => {
      setDraft({});
      qc.invalidateQueries({ queryKey: ["security"] });
    }
  });
  const passwordMutation = useMutation({
    mutationFn: () => changePassword(passwordForm.current, passwordForm.next, csrfToken),
    onSuccess: () => {
      setPasswordFeedback("密码已更新");
      setPasswordForm({ current: "", next: "", confirm: "" });
    },
    onError: (err: Error) => setPasswordFeedback(err.message)
  });
  const merged = { ...security.data, ...draft } as SecuritySettings & { turnstile_secret_key?: string };
  const ttlDays = secondsToDays(merged.session_ttl_seconds ?? defaultSessionTtlDays * secondsPerDay);
  const expectedHostname = merged.turnstile_expected_hostname || "661313.xyz";
  const expectedAction = normalizeTurnstileAction(merged.turnstile_expected_action);
  const passwordReady = passwordForm.current && passwordForm.next.length >= 12 && passwordForm.next === passwordForm.confirm;
  return (
    <div className="security-layout">
      <Panel title="Turnstile" icon={<ShieldCheck size={18} />}>
        <div className="settings-meta-grid">
          <Metric label="Secret" value={security.data?.turnstile_secret_configured ? "configured" : "not configured"} tone={security.data?.turnstile_secret_configured ? "success" : "warning"} />
          <Metric label="Mode" value={merged.turnstile_required ? "fail-closed" : "enabled"} />
          <Metric label="Expected hostname" value={expectedHostname} />
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
        <label className="field-label">Expected hostname<input value={expectedHostname} onChange={(event) => setDraft({ ...draft, turnstile_expected_hostname: event.target.value })} /></label>
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

function Metric({ label, value, tone }: { label: string; value: string; tone?: "success" | "warning" | "danger" }) {
  return <div className="metric"><span>{label}</span><strong className={tone ? `tone-${tone}` : ""}>{value}</strong></div>;
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
  onSave: () => void;
  onTest: () => void;
}) {
  const setNotifications = (patch: Partial<ProbeSettingsDraft["notifications"]>) => setDraft({ ...draft, notifications: { ...draft.notifications, ...patch } });
  return (
    <div className="probe-card-stack">
      <Metric label="配置状态" value={configuredDeviceKey ? "已配置" : "未配置"} tone={configuredDeviceKey ? "success" : "warning"} />
      <label className="field-label">
        Device Key
        <input
          type="password"
          value={draft.notifications.device_key}
          placeholder={configuredDeviceKey ? "已配置，留空保持不变" : "粘贴 Bark Device Key"}
          onChange={(event) => setNotifications({ device_key: event.target.value })}
        />
      </label>
      <div className="button-row">
        <button className="primary-button" disabled={saving} onClick={onSave}><CheckCircle2 size={17} />保存</button>
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
        <Metric label="Codex Home" value={codexHomeStatusValue(status ?? settings?.codex)} />
        <Metric label="Socket" value={appServerSocketStatusValue(status ?? settings?.codex)} />
        <Metric label="Logs DB Path" value={logsDbPathStatusValue(logsDb ?? settings?.logs_db)} />
        <Metric label="Discovery" value={probeDiscoveryWarningsText(status?.discovery_warnings ?? settings?.codex.discovery_warnings ?? settings?.discovery_warnings ?? logsDb?.discovery_warnings)} />
      </div>
      <div className="form-grid compact-three">
        <label className="field-label">Codex Home<input value={draft.codex.home} placeholder="auto" onChange={(event) => setCodex({ home: event.target.value })} /></label>
        <label className="field-label">App Socket<input value={draft.codex.app_server_socket} placeholder="auto" onChange={(event) => setCodex({ app_server_socket: event.target.value })} /></label>
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
        <label className="toggle-row"><span>安装后重载</span><input type="checkbox" checked={draft.hooks.reload_app_server_after_install} onChange={(event) => setHooks({ reload_app_server_after_install: event.target.checked })} /></label>
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
      <Metric label="数据库路径" value={logsDbPathStatusValue(logsDb)} />
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

type AppServerSocketPathFields = {
  app_server_socket?: string | null;
  configured_app_server_socket?: string | null;
  resolved_app_server_socket?: string | null;
  app_server_socket_source?: string | null;
};

export function codexHomeStatusValue(status?: CodexHomePathFields | null): string {
  return pathWithSource(
    firstStringValue(status, ["resolved_codex_home", "codex_home", "home", "configured_codex_home"]),
    firstStringValue(status, ["codex_home_source"])
  );
}

export function appServerSocketStatusValue(status?: AppServerSocketPathFields | null): string {
  return pathWithSource(
    firstStringValue(status, ["resolved_app_server_socket", "app_server_socket", "configured_app_server_socket"]),
    firstStringValue(status, ["app_server_socket_source"])
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

function JobList({ jobs }: { jobs: JobRecord[] }) {
  return (
    <div className="job-list">
      {jobs.map((job) => (
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
          {job.failure_analysis && (
            <div className="job-analysis">
              <strong>{failureCategoryLabel(job.failure_analysis.category)}</strong>
              <p>{job.failure_analysis.explanation}</p>
              <ul>
                {job.failure_analysis.suggestions.map((suggestion) => <li key={suggestion}>{suggestion}</li>)}
              </ul>
            </div>
          )}
          <pre>{job.output || job.error || "no output"}</pre>
        </details>
      ))}
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

function threadStatusLabel(status?: ThreadStatus | string | null): string {
  if (status === "ReplyNeeded") return "待回复";
  if (status === "Recoverable") return "异常";
  if (status === "Running") return "运行中";
  if (status === "Archived") return "归档";
  return "最近";
}

export function failureCategoryLabel(category: string): string {
  const labels: Record<string, string> = {
    release_missing: "Release 缺失",
    download_sha256_mismatch: "下载或校验失败",
    systemd_failure: "systemd 失败",
    nginx_failure: "Nginx 失败",
    permission_denied_sudo: "权限或 sudo 失败",
    read_only_file_system: "文件系统只读/安装目录不可写",
    codex_auth_failure: "Codex 认证失败",
    sqlite_integrity_failure: "SQLite 完整性失败",
    network_tls_eof: "网络或 TLS 中断",
    app_server_unavailable: "app-server 不可用",
    unknown: "未知失败"
  };
  return labels[category] ?? category;
}

export function buildPayload(message: string, config: RunConfig, attachments: Pick<UploadRecord, "id">[] = []): ThreadSendPayload {
  const attachmentIds = attachments.map((attachment) => attachment.id).filter(Boolean);
  const payload: ThreadSendPayload = {
    message,
    model: config.model.trim() || null,
    service_tier: config.serviceTier.trim() || null,
    reasoning_effort: config.reasoning.trim() || null,
    cwd: config.cwd.trim() || null,
    permission_profile: config.permissionProfile.trim() || null,
    approval_policy: config.approvalPolicy.trim() || null,
    sandbox_mode: config.sandboxMode.trim() || null,
    network_access: config.networkAccess,
    collaboration_mode: config.collaborationMode.trim() || null
  };
  if (attachmentIds.length > 0) {
    payload.attachments = attachmentIds;
  }
  return payload;
}

export function runConfigWithSupportedServiceTier(config: RunConfig, models: CodexModel[]): RunConfig {
  if (!config.serviceTier.trim()) return config;
  if (modelSupportsServiceTier(models, config.model, config.serviceTier.trim())) return config;
  return { ...config, serviceTier: "" };
}

function sourceCountsText(counts?: Record<string, number> | null): string {
  if (!counts || Object.keys(counts).length === 0) return "暂无";
  return Object.entries(counts)
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([key, value]) => `${key}:${value}`)
    .join(" ");
}

function permissionLabel(preset: PermissionPresetId): string {
  return permissionPresets.find((item) => item.id === preset)?.label ?? "完全访问权限";
}

export function codexUpdateState(version?: Pick<SystemVersion, "codex_update_available">): { label: string; tone: "success" | "warning" | "danger" } {
  if (version?.codex_update_available === true) {
    return { label: "可更新", tone: "warning" };
  }
  if (version?.codex_update_available === false) {
    return { label: "已是最新", tone: "success" };
  }
  return { label: "未知", tone: "warning" };
}

function actionMessage(result: BridgeActionResult): string {
  if (result.fallback) return result.message ?? `Fallback job ${result.job_id?.slice(0, 8) ?? ""} 已启动`;
  return `Bridge ${result.turn_id ? `turn ${result.turn_id.slice(0, 8)}` : "请求"} 已提交`;
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
  void status;
  if (!activeTurnId) return null;
  const matches = [...blocks].reverse().filter((block) => block.turn_id === activeTurnId && predicate(block) && !isResolvedActionBlock(block));
  if (!matches.length) return null;
  return matches[0];
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
  return left.turn_id === right.turn_id && leftIds.some((leftId) => rightIds.includes(leftId));
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

export function formatGoalStatus(goal: { enabled?: boolean; status?: string | null } | null | undefined): string {
  const status = goal?.status?.trim().toLowerCase() || (goal?.enabled ? "active" : "idle");
  const labels: Record<string, string> = {
    active: "运行中",
    running: "运行中",
    complete: "已完成",
    completed: "已完成",
    blocked: "已阻塞",
    paused: "已暂停",
    idle: "未启用",
    missing_thread: "未选择线程",
    cleared: "已清除"
  };
  return labels[status] ?? status;
}

function cleanHostValue(value?: string | null): string | null {
  const cleaned = value?.trim();
  const legacyAlias = ["tencent", "wanka"].join("-");
  if (!cleaned || cleaned === legacyAlias) return null;
  return cleaned;
}

function secondsToDays(seconds: number): number {
  return Math.max(1, Math.round(seconds / secondsPerDay));
}

function normalizeTurnstileAction(value?: string | null): string {
  const action = value?.trim();
  return action || "login";
}

export function extractPlanText(value: string): string {
  return value
    .replace(/<\/?proposed_plan>/g, "")
    .trim() || value || "Plan 内容等待 Codex 写入。";
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
