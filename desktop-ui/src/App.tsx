import {
  Activity,
  BellDot,
  Bot,
  CheckCircle2,
  CircleDashed,
  Database,
  FolderCog,
  Gauge,
  GitBranch,
  HardDrive,
  ListChecks,
  RefreshCw,
  ShieldCheck,
  TerminalSquare,
  Trash2,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type {
  DeletePlan,
  DesktopGoal,
  DesktopHome,
  DesktopOverview,
  ThreadSummary,
} from "./tauri";
import {
  clearDesktopGoal,
  loadDesktopHome,
  loadDesktopOverview,
  openConfigDir,
  openLogDir,
  pauseDesktopGoal,
  resumeDesktopGoal,
  saveDesktopGoal,
} from "./tauri";

const statusLabels: Record<string, string> = {
  Recent: "最近",
  Running: "运行中",
  ReplyNeeded: "待回复",
  Recoverable: "需处理",
  Archived: "已归档",
};

export function App() {
  const [home, setHome] = useState<DesktopHome | null>(null);
  const [overview, setOverview] = useState<DesktopOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [goalDraft, setGoalDraft] = useState("");
  const [goalBudget, setGoalBudget] = useState("");
  const [goalAction, setGoalAction] = useState<string | null>(null);

  const refresh = async () => {
    setRefreshing(true);
    setError(null);
    setNotice(null);
    try {
      const [nextOverview, nextHome] = await Promise.all([
        loadDesktopOverview(),
        loadDesktopHome(),
      ]);
      setOverview(nextOverview);
      setHome(nextHome);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setRefreshing(false);
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    const goal = home?.goal;
    setGoalDraft(goal?.objective ?? "");
    setGoalBudget(goal?.tokenBudget ? String(goal.tokenBudget) : "");
  }, [home?.goal.threadId, home?.goal.objective, home?.goal.tokenBudget]);

  const effectiveOverview = home?.overview ?? overview;
  const readiness = useMemo(() => {
    if (!effectiveOverview) return 0;
    return [
      effectiveOverview.appSupportDirReady,
      effectiveOverview.logDirReady,
      effectiveOverview.configFileExists,
      effectiveOverview.databaseFileExists,
    ].filter(Boolean).length;
  }, [effectiveOverview]);

  const currentThreadId = home?.goal.threadId ?? home?.threads[0]?.id ?? null;
  const updateGoal = (goal: DesktopGoal) => {
    setHome((previous) => (previous ? { ...previous, goal } : previous));
  };
  const runGoalAction = async (
    label: string,
    action: (threadId: string) => Promise<DesktopGoal>,
  ) => {
    if (!currentThreadId) {
      setError("没有可绑定 Goal 的线程");
      return;
    }
    setGoalAction(label);
    setError(null);
    setNotice(null);
    try {
      const goal = await action(currentThreadId);
      updateGoal(goal);
      setNotice(`${label}已完成`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setGoalAction(null);
    }
  };

  return (
    <main className="app-shell">
      <TopStatus overview={effectiveOverview} refreshing={refreshing} onRefresh={refresh} />
      <section className="console-grid">
        <aside className="left-rail" aria-label="NexusHub 桌面导航">
          <div className="brand-block">
            <div className="brand-mark">NH</div>
            <div>
              <h1>NexusHub</h1>
              <p>macOS Tauri</p>
            </div>
          </div>

          <nav className="nav-stack">
            {[
              ["线程", GitBranch],
              ["Probe", BellDot],
              ["Goal", ListChecks],
              ["清理", Trash2],
              ["设置", TerminalSquare],
            ].map(([label, Icon], index) => (
              <button
                className={index === 0 ? "nav-item active" : "nav-item"}
                key={label as string}
                type="button"
              >
                <Icon size={17} />
                <span>{label as string}</span>
              </button>
            ))}
          </nav>

          <div className="rail-note">
            <ShieldCheck size={16} />
            <span>macOS 使用本机 App 入口，公网控制台仅限 Linux 部署。</span>
          </div>
        </aside>

        <section className="workspace">
          <div className="workspace-header">
            <div>
              <p className="eyebrow">本机控制台</p>
              <h2>运行概览</h2>
            </div>
            <div className="readiness-meter">
              <Gauge size={16} />
              <span>{readiness}/4 初始化项</span>
            </div>
          </div>

          {error ? (
            <div className="error-row" role="alert">
              <CircleDashed size={18} />
              <span>{error}</span>
            </div>
          ) : null}
          {notice ? (
            <div className="success-row" role="status">
              <CheckCircle2 size={18} />
              <span>{notice}</span>
            </div>
          ) : null}

          <MetricGrid home={home} overview={effectiveOverview} />

          <section className="detail-grid">
            <article className="status-panel thread-panel">
              <PanelTitle icon={GitBranch} title="线程" />
              <ThreadList threads={home?.threads ?? []} />
            </article>

            <article className="status-panel">
              <PanelTitle icon={BellDot} title="Probe" />
              <ProbeSummary home={home} />
            </article>

            <article className="status-panel">
              <PanelTitle icon={ListChecks} title="Goal" />
              <GoalSummary
                home={home}
                draft={goalDraft}
                budget={goalBudget}
                pendingAction={goalAction}
                onDraftChange={setGoalDraft}
                onBudgetChange={setGoalBudget}
                onSave={() =>
                  runGoalAction("保存", (threadId) => {
                    const parsedBudget = goalBudget.trim() ? Number(goalBudget.trim()) : null;
                    const nextBudget =
                      parsedBudget === null || Number.isFinite(parsedBudget) ? parsedBudget : null;
                    return saveDesktopGoal(threadId, goalDraft, nextBudget);
                  })
                }
                onPause={() => runGoalAction("暂停", pauseDesktopGoal)}
                onResume={() => runGoalAction("恢复", resumeDesktopGoal)}
                onClear={() => runGoalAction("清除", clearDesktopGoal)}
              />
            </article>

            <article className="status-panel">
              <PanelTitle icon={Trash2} title="清理 dry-run" />
              <CleanupSummary archive={home?.archivePlan} hidden={home?.hiddenPlan} />
            </article>

            <article className="status-panel wide">
              <PanelTitle icon={FolderCog} title="路径与配置" />
              <PathRows
                overview={effectiveOverview}
                onOpenConfig={openConfigDir}
                onOpenLogs={openLogDir}
              />
            </article>

            <article className="status-panel wide">
              <PanelTitle icon={Bot} title="模型与权限" />
              <ModelRows home={home} />
            </article>
          </section>
        </section>
      </section>
    </main>
  );
}

function TopStatus({
  overview,
  refreshing,
  onRefresh,
}: {
  overview: DesktopOverview | null;
  refreshing: boolean;
  onRefresh: () => void;
}) {
  return (
    <header className="top-status">
      <div className="status-left">
        <Activity size={18} />
        <span>本机 App</span>
        <strong>{overview ? `${overview.os}/${overview.arch}` : "检测中"}</strong>
      </div>
      <div className="status-right">
        <span>{overview?.identifier ?? "com.lich13.nexushub"}</span>
        <span>v{overview?.version ?? "0.1.98"}</span>
        <button type="button" onClick={onRefresh} disabled={refreshing}>
          <RefreshCw size={15} className={refreshing ? "spin" : undefined} />
          刷新
        </button>
      </div>
    </header>
  );
}

function MetricGrid({
  home,
  overview,
}: {
  home: DesktopHome | null;
  overview: DesktopOverview | null;
}) {
  const cards = [
    {
      title: "线程",
      value: home ? String(home.threads.length) : "-",
      detail: "本机 Codex 状态",
      icon: GitBranch,
    },
    {
      title: "Probe",
      value: home?.probe?.logs_db_status ?? "-",
      detail: home?.probe?.lifecycle_status ?? "等待读取",
      icon: BellDot,
    },
    {
      title: "Codex Home",
      value: overview?.codexHomeSource ?? "-",
      detail: overview?.codexHome ?? "检测中",
      icon: Database,
    },
    {
      title: "入口",
      value: "本机 App",
      detail: "配置与日志保存在用户目录",
      icon: ShieldCheck,
    },
  ];

  return (
    <div className="module-grid">
      {cards.map((entry) => {
        const Icon = entry.icon;
        return (
          <article className="module-card" key={entry.title}>
            <div className="module-icon">
              <Icon size={20} />
            </div>
            <div>
              <div className="module-title-row">
                <h3>{entry.title}</h3>
                <span>{entry.value}</span>
              </div>
              <p>{entry.detail}</p>
            </div>
          </article>
        );
      })}
    </div>
  );
}

function ThreadList({ threads }: { threads: ThreadSummary[] }) {
  if (threads.length === 0) {
    return <p className="empty-state">未读取到线程。</p>;
  }

  return (
    <div className="thread-list">
      {threads.slice(0, 12).map((thread) => (
        <div className="thread-row" key={thread.id}>
          <div>
            <strong>{thread.title || thread.id}</strong>
            <p>{thread.latest_message ?? thread.id}</p>
          </div>
          <span>{statusLabels[thread.status] ?? thread.status}</span>
        </div>
      ))}
    </div>
  );
}

function ProbeSummary({ home }: { home: DesktopHome | null }) {
  const probe = home?.probe;
  const rows: Array<[string, string | undefined]> = [
    ["Hook", probe?.hook_status],
    ["Bark", probe?.bark_status],
    ["logs_2", probe?.logs_db_status],
    ["Host", probe?.host_label],
  ];
  return <KeyRows rows={rows} />;
}

function GoalSummary({
  home,
  draft,
  budget,
  pendingAction,
  onDraftChange,
  onBudgetChange,
  onSave,
  onPause,
  onResume,
  onClear,
}: {
  home: DesktopHome | null;
  draft: string;
  budget: string;
  pendingAction: string | null;
  onDraftChange: (value: string) => void;
  onBudgetChange: (value: string) => void;
  onSave: () => void;
  onPause: () => void;
  onResume: () => void;
  onClear: () => void;
}) {
  const goal = home?.goal;
  const rows: Array<[string, string | undefined]> = [
    ["状态", goal?.status ?? "idle"],
    ["线程", goal?.threadId ?? home?.threads[0]?.id ?? "未读取"],
    ["可用", goal?.available ? "是" : "否"],
  ];
  const disabled = !!pendingAction || !goal?.threadId;
  return (
    <div className="goal-panel">
      <KeyRows rows={rows} />
      <label>
        <span>目标</span>
        <textarea
          value={draft}
          onChange={(event) => onDraftChange(event.target.value)}
          rows={3}
        />
      </label>
      <label>
        <span>Token budget</span>
        <input
          inputMode="numeric"
          min="0"
          value={budget}
          onChange={(event) => onBudgetChange(event.target.value)}
          placeholder="可选"
        />
      </label>
      <div className="action-row">
        <button type="button" onClick={onSave} disabled={disabled}>
          保存
        </button>
        <button type="button" onClick={onPause} disabled={disabled || !goal?.enabled}>
          暂停
        </button>
        <button type="button" onClick={onResume} disabled={disabled || !goal?.threadId}>
          恢复
        </button>
        <button type="button" onClick={onClear} disabled={disabled || !goal?.threadId}>
          清除
        </button>
      </div>
    </div>
  );
}

function CleanupSummary({
  archive,
  hidden,
}: {
  archive?: DeletePlan | null;
  hidden?: DeletePlan | null;
}) {
  const rows: Array<[string, string]> = [
    ["归档线程", archive?.archived_threads ?? 0],
    ["隐藏线程", hidden?.hidden_threads ?? 0],
    ["rollout 文件", (archive?.rollout_files ?? 0) + (hidden?.rollout_files ?? 0)],
    ["完整性", archive?.integrity ?? hidden?.integrity ?? "未读取"],
  ].map(([key, value]) => [String(key), String(value)]);
  return <KeyRows rows={rows} />;
}

function PathRows({
  overview,
  onOpenConfig,
  onOpenLogs,
}: {
  overview: DesktopOverview | null;
  onOpenConfig: () => Promise<void>;
  onOpenLogs: () => Promise<void>;
}) {
  const rows = [
    ["配置目录", overview?.paths.appSupportDir, overview?.appSupportDirReady],
    ["配置文件", overview?.paths.configFile, overview?.configFileExists],
    ["数据库", overview?.paths.databaseFile, overview?.databaseFileExists],
    ["日志目录", overview?.paths.logDir, overview?.logDirReady],
    ["App 日志", overview?.paths.appLogFile, undefined],
  ] as const;

  return (
    <>
      <div className="path-actions">
        <button type="button" onClick={() => void onOpenConfig()}>
          打开配置目录
        </button>
        <button type="button" onClick={() => void onOpenLogs()}>
          打开日志目录
        </button>
      </div>
      <div className="path-table">
        {rows.map(([label, value, ready]) => (
          <div className="path-row" key={label}>
            <span className="path-label">{label}</span>
            <code>{value ?? "加载中"}</code>
            {ready === undefined ? (
              <HardDrive size={15} />
            ) : ready ? (
              <CheckCircle2 size={15} className="ok" />
            ) : (
              <CircleDashed size={15} className="muted" />
            )}
          </div>
        ))}
      </div>
    </>
  );
}

function ModelRows({ home }: { home: DesktopHome | null }) {
  const model = home?.models.find((item) => item.default) ?? home?.models[0];
  const profile =
    home?.permissionProfiles.find((item) => item.default) ?? home?.permissionProfiles[0];
  return (
    <KeyRows
      rows={[
        ["模型", model?.label ?? "未读取"],
        ["权限", profile?.label ?? "未读取"],
        ["cwd", home?.codexConfig.cwd ?? "未读取"],
        ["插件", home ? String(home.plugins.length) : "0"],
      ]}
    />
  );
}

function KeyRows({ rows }: { rows: Array<[string, string | undefined]> }) {
  return (
    <div className="key-table">
      {rows.map(([label, value]) => (
        <div className="key-row" key={label}>
          <span>{label}</span>
          <strong>{value ?? "未读取"}</strong>
        </div>
      ))}
    </div>
  );
}

function PanelTitle({ icon: Icon, title }: { icon: typeof GitBranch; title: string }) {
  return (
    <div className="panel-title">
      <Icon size={18} />
      <h3>{title}</h3>
    </div>
  );
}
