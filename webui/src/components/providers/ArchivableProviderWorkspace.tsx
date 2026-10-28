import {
  Archive,
  ArchiveRestore,
  Brain,
  Check,
  ChevronLeft,
  Copy,
  Layers,
  Pencil,
  RefreshCw,
  Search,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { groupProviderEvents, type ExecutionRenderItem } from "../../lib/domain/executionGroups";
import { userTimelineEntries } from "../../lib/domain/timelineViewModel";
import { visibleMarkdown } from "../../lib/domain/visibleMarkdown";
import { machineScope } from "../../lib/query/connection";
import {
  readArchivableProviderSession,
  useProviderActions,
  useProviderDetail,
  useProviderSessions,
  type ArchivableProviderId,
} from "../../lib/query/providerSessions";
import { useSessionSelection } from "../../lib/query/sessions";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";
import { useHistoryScroll } from "../../lib/useHistoryScroll";
import { providerTimelinePage, useTimelineWindow } from "../../lib/useTimelineWindow";
import type {
  ProviderHistoryEvent,
  ProviderSessionSummary,
  SessionSearchResult,
} from "../../types";
import {
  ActivityDetails,
  DisclosureRevealContext,
  DisclosureScope,
} from "../common/ActivityDetails";
import { CopyReplyButton } from "../common/CopyReplyButton";
import { ExecutionGroupView } from "../common/ExecutionGroupView";
import { MarkdownPathScope, ToolOutput } from "../common/FilePathLink";
import { HistoryLoading } from "../common/HistoryLoading";
import { MarkdownContent } from "../common/MarkdownContent";
import { PlanActions } from "../common/PlanActions";
import { RenameableSession } from "../common/RenameableSession";
import { RunningIndicator } from "../common/RunningIndicator";
import { SessionBatchControls, SessionCheckbox } from "../common/SessionBatchControls";
import { locateTimelineTarget, useSearchWorkspace } from "../common/SessionSearch";
import { SessionSize } from "../common/SessionSize";
import { TaskMenu, ThreadMenuItems, type ThreadMenuAction } from "../common/TaskMenu";
import { TimelineHistoryStatus } from "../common/TimelineHistoryStatus";
import { TimelineRail } from "../common/TimelineRail";
import { UserMessage, UserMessageScope } from "../common/UserMessage";

export function AntigravityWorkspace({
  capabilities,
}: {
  capabilities: RuntimeCapabilityMatrix;
}) {
  return <ArchivableProviderWorkspace provider="antigravity" capabilities={capabilities} />;
}

export function DshWorkspace({
  capabilities,
}: {
  capabilities: RuntimeCapabilityMatrix;
}) {
  return <ArchivableProviderWorkspace provider="dsh" capabilities={capabilities} />;
}

export function ArchivableProviderWorkspace({
  provider,
  capabilities,
}: {
  provider: ArchivableProviderId;
  capabilities: RuntimeCapabilityMatrix;
}) {
  const label = provider === "antigravity" ? "Antigravity" : "DeepSeek Harness";
  const EmptyIcon = provider === "antigravity" ? Sparkles : Layers;
  const supported =
    provider === "antigravity"
      ? capabilities.antigravitySessions
      : capabilities.dshSessions;
  const queryEnabled = supported === true;

  const menuTrigger = useRef<HTMLElement>(null);
  const stream = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState("");
  const [archived, setArchived] = useState(false);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [narrow, setNarrow] = useState(
    () => typeof window !== "undefined" && window.matchMedia("(max-width: 767px)").matches,
  );
  const [renaming, setRenaming] = useState(false);
  const [title, setTitle] = useState("");
  const [feedback, setFeedback] = useState("");
  const [pendingSearch, setPendingSearch] = useState<SessionSearchResult | null>(null);

  const sessions = useProviderSessions(provider, { q: query, archived }, queryEnabled);
  const filterKey = `${archived ? "archived" : "active"}:${query}`;
  const batch = useSessionSelection(
    provider,
    filterKey,
    (sessions.data ?? []).map(item => item.sessionKey),
    keys => {
      if (selectedKey && keys.includes(selectedKey)) setSelectedKey(null);
      setRenaming(false);
    },
  );

  const listed =
    sessions.data?.find(item => item.sessionKey === selectedKey) ?? sessions.data?.[0];
  const detailTargetKey =
    pendingSearch && selectedKey ? selectedKey : listed?.sessionKey ?? null;
  const detailVisible = !narrow || Boolean(selectedKey);
  const detail = useProviderDetail(
    provider,
    detailVisible ? detailTargetKey : null,
    queryEnabled,
  );
  const selected = detail.data?.summary ?? listed;

  const loadedEvents = useMemo(() => {
    const latest = new Map<string, ProviderHistoryEvent>();
    for (const page of [...(detail.data ? [detail.data] : [])]) {
      for (const event of page.events) latest.set(event.id, event);
    }
    return [...latest.values()];
  }, [detail.data]);

  const navigation = useTimelineWindow({
    scope: `${machineScope()}:${provider}:${selected?.sessionKey ?? ""}`,
    streamRef: stream,
    history: detail.data?.history,
    latestId: loadedEvents[loadedEvents.length - 1]?.id,
    id: (event: ProviderHistoryEvent) => event.id ?? "",
    read: async cursor =>
      providerTimelinePage(
        await readArchivableProviderSession(provider, {
          sessionKey: selected!.sessionKey,
          limit: 160,
          before: cursor,
        }),
      ),
    onNavigate: () => {
      history.follow.current = false;
      setPendingSearch(null);
    },
    onLocated: follow => {
      history.follow.current = Boolean(follow);
      history.capture();
    },
  });

  const events = navigation.items ?? loadedEvents;
  const groupedEvents = useMemo(() => groupProviderEvents(events, label), [events, label]);
  const timelineEntries = useMemo(
    () =>
      detail.data?.timeline ??
      userTimelineEntries(
        provider,
        groupedEvents.flatMap(entry =>
          entry.kind === "item"
            ? [
                {
                  id: entry.key,
                  kind: entry.item.kind,
                  text: entry.item.text,
                  userMessage: entry.item.userMessage,
                },
              ]
            : [],
        ),
      ),
    [detail.data?.timeline, groupedEvents, provider],
  );

  const latestReplyText = useMemo(() => {
    for (let index = events.length - 1; index >= 0; index -= 1) {
      const candidate = events[index];
      if (!candidate.kind.startsWith("assistant_message")) continue;
      const cleaned = visibleMarkdown(candidate.text ?? "").trim();
      if (cleaned) return cleaned;
    }
    return "";
  }, [events]);

  const actions = useProviderActions(provider);
  const error =
    actions.rename.error ??
    actions.archive.error ??
    actions.restore.error ??
    (detail.isFetchNextPageError ? null : detail.error) ??
    sessions.error;

  const selectSearchResult = useCallback((result: SessionSearchResult) => {
    setSelectedKey(result.sessionKey);
    setPendingSearch(result);
    setRenaming(false);
    setFeedback("");
  }, []);

  useSearchWorkspace(
    useMemo(
      () => ({
        provider,
        sessionKey: selected?.sessionKey ?? null,
        selectResult: selectSearchResult,
      }),
      [provider, selected?.sessionKey, selectSearchResult],
    ),
  );

  useEffect(() => {
    const media = window.matchMedia("(max-width: 767px)");
    const update = () => setNarrow(media.matches);
    media.addEventListener("change", update);
    update();
    return () => media.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    if (
      pendingSearch &&
      detail.data?.summary.sessionKey === pendingSearch.sessionKey &&
      detail.data.summary.archived !== archived
    ) {
      setArchived(detail.data.summary.archived);
    }
  }, [archived, detail.data?.summary.archived, detail.data?.summary.sessionKey, pendingSearch]);

  const history = useHistoryScroll({
    scopeKey: `${machineScope()}:${provider}:${selected?.sessionKey ?? ""}:${navigation.generation}`,
    streamRef: stream,
    cursor: navigation.active ? navigation.before : detail.data?.beforeCursor,
    hasMore: navigation.active ? navigation.hasMore : Boolean(detail.hasNextPage),
    ready: detailVisible && Boolean(detail.data),
    initialPosition: navigation.active ? { top: 0, follow: false } : undefined,
    busy: navigation.busy || (!navigation.active && detail.isFetching),
    searchActive: Boolean(pendingSearch),
    loadPage: async (cursor, isCurrent) => {
      if (navigation.active) return navigation.loadOlder(cursor, isCurrent);
      const result = await detail.fetchNextPage({ cancelRefetch: false });
      if (result.isError) throw result.error;
      const pages = result.data?.pages;
      const page = pages?.[pages.length - 1];
      if (!page) throw new Error("历史读取失败");
      return { cursor: page.beforeCursor, hasMore: page.hasMore };
    },
  });

  useEffect(() => {
    if (
      !pendingSearch ||
      pendingSearch.sessionKey !== selected?.sessionKey ||
      detail.isLoading ||
      detail.isFetching ||
      history.loading
    ) {
      return;
    }
    if (navigation.active) {
      navigation.latest();
      return;
    }
    history.follow.current = false;
    if (
      locateTimelineTarget(
        pendingSearch.positionKey,
        stream.current?.closest(".provider-detail") ?? undefined,
        "auto",
      )
    ) {
      history.capture();
      setFeedback("");
      setPendingSearch(null);
    } else if (detail.hasNextPage) {
      if (!history.error) void history.loadOlder("search");
    } else {
      setFeedback("结果所在历史未加载，请刷新后重试");
      setPendingSearch(null);
    }
  }, [
    detail.hasNextPage,
    detail.isFetching,
    detail.isLoading,
    events,
    history.capture,
    history.error,
    history.follow,
    history.loadOlder,
    history.loading,
    navigation.active,
    navigation.latest,
    pendingSearch,
    selected?.sessionKey,
  ]);

  const copy = async (value: string, successText = "已复制") => {
    try {
      await navigator.clipboard.writeText(value);
      setFeedback(successText);
    } catch {
      setFeedback("复制失败：剪贴板不可用");
    }
  };

  const handleArchiveToggle = (item: ProviderSessionSummary) => {
    actions.archive.reset();
    actions.restore.reset();
    const mutation = item.archived ? actions.restore : actions.archive;
    mutation.mutate(
      { sessionKey: item.sessionKey },
      {
        onSuccess: () => {
          setFeedback(item.archived ? "已恢复线程" : "已归档线程");
          if (selectedKey === item.sessionKey) {
            const list = sessions.data ?? [];
            const index = list.findIndex(entry => entry.sessionKey === item.sessionKey);
            const next =
              (index >= 0 ? list[index + 1] ?? list[index - 1] : undefined)?.sessionKey ?? null;
            setSelectedKey(next);
            setRenaming(false);
          }
        },
      },
    );
  };

  const rowMenuActions = (item: ProviderSessionSummary): ThreadMenuAction[] => [
    {
      id: item.archived ? "restore" : "archive",
      label: item.archived ? "恢复" : "归档",
      icon: item.archived ? <ArchiveRestore size={15} /> : <Archive size={15} />,
      disabled:
        !item.canArchive ||
        batch.busy ||
        actions.archive.isPending ||
        actions.restore.isPending,
      reason: !item.canArchive ? item.archiveBlockReason : null,
      run: () => handleArchiveToggle(item),
    },
    {
      id: "copy-id",
      label: "复制 ID",
      icon: <Copy size={15} />,
      run: () => {
        void copy(item.id, "已复制 ID");
      },
    },
    {
      id: "copy-path",
      label: "复制路径",
      icon: <Copy size={15} />,
      run: () => {
        void copy(item.path, "已复制路径");
      },
    },
    {
      id: "delete",
      label: "永久删除",
      icon: <Trash2 size={15} />,
      danger: true,
      disabled: !item.canDelete || batch.busy,
      reason: !item.canDelete ? item.deleteBlockReason : null,
      run: () => {
        batch.prepareSingle(item.sessionKey, "delete");
      },
    },
  ];

  const switchArchiveTab = (nextArchived: boolean) => {
    if (nextArchived === archived) return;
    setArchived(nextArchived);
    setSelectedKey(null);
    setRenaming(false);
    setFeedback("");
    setPendingSearch(null);
  };

  return (
    <div className={`provider-workspace ${selectedKey ? "has-selection" : ""}`}>
      <aside className="provider-list">
        <header className="workspace-heading">
          <h1>{label}</h1>
          <button
            className="icon-button"
            disabled={!queryEnabled}
            onClick={() => {
              void sessions.refetch();
              if (selected && detailVisible) void detail.refetch();
            }}
            title="刷新任务"
          >
            <RefreshCw size={16} />
          </button>
        </header>
        <label className="search-box">
          <Search size={16} />
          <input
            value={query}
            disabled={supported === false}
            onChange={event => setQuery(event.target.value)}
            placeholder="搜索标题、ID 或工作目录"
          />
        </label>
        <div className="segmented provider-filter-tabs" role="tablist" aria-label={`${label} 线程筛选`}>
          <button
            role="tab"
            type="button"
            className={!archived ? "active" : ""}
            aria-selected={!archived}
            disabled={supported === false}
            onClick={() => switchArchiveTab(false)}
          >
            全部
          </button>
          <button
            role="tab"
            type="button"
            className={archived ? "active" : ""}
            aria-selected={archived}
            disabled={supported === false}
            onClick={() => switchArchiveTab(true)}
          >
            已归档
          </button>
        </div>
        <div className="provider-list-scroll">
          {supported === false ? (
            <div className="muted-row" role="status">
              当前连接的远程服务版本较旧，尚未支持 {label} 会话列表。请先在「设置 → 更新与维护」升级远程服务。
            </div>
          ) : (
            <>
              <SessionBatchControls
                batch={batch}
                operations={archived ? ["restore", "delete"] : ["archive", "delete"]}
              />
              {sessions.error && (
                <div className="form-error" role="alert">
                  {sessions.error.message}
                </div>
              )}
              {sessions.data?.map(item => (
                <div key={item.sessionKey} className="selectable-session">
                  {batch.selecting && (
                    <SessionCheckbox
                      batch={batch}
                      sessionKey={item.sessionKey}
                      title={providerSessionLabel(item)}
                    />
                  )}
                  <RenameableSession
                    title={providerSessionLabel(item)}
                    className={`provider-session ${
                      item.sessionKey === selected?.sessionKey ? "selected" : ""
                    }`}
                    disabled={batch.busy}
                    selecting={batch.selecting}
                    selected={item.sessionKey === selected?.sessionKey}
                    renameBlockReason={
                      !item.canRename ? item.renameBlockReason || "当前线程不可改名" : undefined
                    }
                    onSelect={event =>
                      batch.select(item.sessionKey, event, () => {
                        setSelectedKey(item.sessionKey);
                        setRenaming(false);
                        setFeedback("");
                      })
                    }
                    onRename={next =>
                      actions.rename.mutateAsync({ sessionKey: item.sessionKey, title: next })
                    }
                    menuActions={rowMenuActions(item)}
                  >
                    <strong>{providerSessionLabel(item)}</strong>
                    {item.cwd && <span>{item.cwd}</span>}
                    <span>{item.id}</span>
                    <span className="session-status-row">
                      {item.status === "running" ? (
                        <RunningIndicator />
                      ) : (
                        <small>{providerStatusLabel(item)}</small>
                      )}
                      <SessionSize size={item.storageSize} />
                    </span>
                  </RenameableSession>
                </div>
              ))}
              {(supported === undefined || sessions.isLoading) && (
                <div className="muted-row">正在读取任务...</div>
              )}
              {queryEnabled && !sessions.isLoading && !sessions.data?.length && (
                <div className="muted-row">
                  {archived ? `暂无已归档的 ${label} 会话` : `未发现 ${label} 会话`}
                </div>
              )}
            </>
          )}
        </div>
      </aside>
      <main className="provider-detail">
        {!selected && (
          <div className="empty-state">
            <EmptyIcon size={28} />
            <strong>
              {supported === false
                ? `升级远程服务后即可浏览 ${label} 会话`
                : `选择一个 ${label} 任务`}
            </strong>
          </div>
        )}
        {selected && (
          <>
            <header className="conversation-header" data-timeline-id="session">
              <button
                className="icon-button mobile-back"
                title="返回任务列表"
                onClick={() => setSelectedKey(null)}
              >
                <ChevronLeft size={18} />
              </button>
              <div className="conversation-title-copy">
                <h1 className="conversation-title">{providerSessionLabel(selected)}</h1>
              </div>
              {selected.archived ? (
                <span className="status-chip tone-neutral">已归档</span>
              ) : selected.status === "running" ? (
                <span className="status-chip tone-info">运行中</span>
              ) : null}
              <TaskMenu label={`${label} 任务操作`} triggerRef={menuTrigger}>
                <button
                  disabled={!selected.canRename || batch.busy}
                  title={!selected.canRename ? selected.renameBlockReason ?? undefined : undefined}
                  onClick={() => {
                    setTitle(selected.title);
                    setRenaming(true);
                    actions.rename.reset();
                  }}
                >
                  <Pencil size={15} />
                  改名
                </button>
                <button
                  disabled={
                    !selected.canArchive ||
                    batch.busy ||
                    actions.archive.isPending ||
                    actions.restore.isPending
                  }
                  title={
                    !selected.canArchive ? selected.archiveBlockReason ?? undefined : undefined
                  }
                  onClick={() => handleArchiveToggle(selected)}
                >
                  {selected.archived ? (
                    <>
                      <ArchiveRestore size={15} />
                      取消归档
                    </>
                  ) : (
                    <>
                      <Archive size={15} />
                      归档
                    </>
                  )}
                </button>
                <button
                  disabled={!latestReplyText}
                  title={!latestReplyText ? "当前会话暂无可复制的助手答复" : undefined}
                  onClick={() => {
                    void copy(latestReplyText, "已复制答复");
                  }}
                >
                  <Copy size={15} />
                  复制答复
                </button>
                <ThreadMenuItems
                  actions={[
                    {
                      id: "copy-id",
                      label: "复制 ID",
                      icon: <Copy size={15} />,
                      run: () => {
                        void copy(selected.id, "已复制 ID");
                      },
                    },
                    {
                      id: "copy-path",
                      label: "复制路径",
                      icon: <Copy size={15} />,
                      run: () => {
                        void copy(selected.path, "已复制路径");
                      },
                    },
                    {
                      id: "delete",
                      label: "永久删除",
                      icon: <Trash2 size={15} />,
                      danger: true,
                      disabled: !selected.canDelete || batch.busy,
                      reason: !selected.canDelete ? selected.deleteBlockReason : null,
                      run: () => {
                        batch.prepareSingle(selected.sessionKey, "delete", menuTrigger.current);
                      },
                    },
                  ]}
                />
              </TaskMenu>
            </header>
            {feedback && (
              <div role="status" className="task-feedback">
                {feedback}
              </div>
            )}
            {renaming && (
              <form
                className="inline-rename"
                onSubmit={event => {
                  event.preventDefault();
                  actions.rename.mutate(
                    { sessionKey: selected.sessionKey, title },
                    { onSuccess: () => setRenaming(false) },
                  );
                }}
              >
                <input
                  aria-label={`${label} 任务名称`}
                  maxLength={200}
                  value={title}
                  onChange={event => setTitle(event.target.value)}
                  autoFocus
                />
                <button
                  className="icon-button"
                  title="保存名称"
                  disabled={actions.rename.isPending || !title.trim()}
                >
                  <Check size={17} />
                </button>
                <button
                  className="icon-button"
                  type="button"
                  title="取消改名"
                  onClick={() => setRenaming(false)}
                >
                  <X size={17} />
                </button>
              </form>
            )}
            {selected.readError && (
              <div className="form-error" role="alert">
                {selected.readError}
              </div>
            )}
            {selected.readWarning && (
              <DisclosureScope.Provider value={`${provider}:${selected.sessionKey}`}>
                <ActivityDetails
                  className="task-feedback"
                  summary="读取提示"
                  initiallyOpen={false}
                  stateKey="read-warning"
                >
                  {() => <div>{selected.readWarning}</div>}
                </ActivityDetails>
              </DisclosureScope.Provider>
            )}
            {error && (
              <div className="form-error" role="alert">
                {error.message}
              </div>
            )}
            <div className="timeline-reading-shell">
              <TimelineRail
                entries={timelineEntries}
                streamRef={stream}
                onSelect={detail.data?.timeline ? navigation.select : undefined}
                loadOlder={
                  !detail.data?.timeline && detail.hasNextPage
                    ? () => {
                        void history.loadOlder("search");
                      }
                    : undefined
                }
                loading={history.loading}
              />
              <div
                ref={stream}
                className="provider-events"
                onScroll={history.onScroll}
                onScrollCapture={history.onScrollCapture}
                onWheel={history.onWheel}
                onTouchStart={history.onTouchStart}
                onTouchMove={history.onTouchMove}
                onKeyDown={history.onKeyDown}
                tabIndex={0}
              >
                <HistoryLoading
                  loading={history.loading}
                  error={history.error}
                  retry={history.retry}
                />
                <UserMessageScope.Provider
                  value={{ provider, sessionKey: selected.sessionKey }}
                >
                  <MarkdownPathScope.Provider value={selected.cwd}>
                    <DisclosureScope.Provider value={`${provider}:${selected.sessionKey}`}>
                      <DisclosureRevealContext.Provider
                        value={
                          pendingSearch?.sessionKey === selected.sessionKey
                            ? pendingSearch.positionKey
                            : null
                        }
                      >
                        {renderProviderEvents(groupedEvents, selected.title, label)}
                      </DisclosureRevealContext.Provider>
                    </DisclosureScope.Provider>
                  </MarkdownPathScope.Provider>
                </UserMessageScope.Provider>
                <TimelineHistoryStatus
                  navigation={{ ...navigation, busy: navigation.busy || history.loading }}
                />
                {detail.isLoading && <div className="muted-row">正在读取消息...</div>}
                {!detail.isLoading && !events.length && (
                  <div className="muted-row">暂无对话</div>
                )}
              </div>
            </div>
          </>
        )}
      </main>
    </div>
  );
}

export function renderProviderEvents(
  entries: ExecutionRenderItem<ProviderHistoryEvent>[],
  title: string,
  providerLabel: string,
): ReactNode {
  return entries.map(entry =>
    entry.kind === "group" ? (
      <ExecutionGroupView key={entry.group.id} group={entry.group} />
    ) : (
      <ProviderEventView
        key={entry.key}
        activityId={entry.key}
        event={entry.item}
        title={title}
        providerLabel={providerLabel}
        timelineAliases={entry.sourceIds}
      />
    ),
  );
}

function ProviderEventView({
  event,
  activityId,
  title,
  providerLabel,
  timelineAliases = [],
}: {
  event: ProviderHistoryEvent;
  activityId: string;
  title: string;
  providerLabel: string;
  timelineAliases?: string[];
}) {
  const timelineProps = {
    "data-timeline-id": activityId,
    "data-timeline-aliases": timelineAliases.length ? timelineAliases.join(" ") : undefined,
  };
  if (event.kind === "user_message") {
    return (
      <UserMessage
        message={event.userMessage}
        text={event.text ?? ""}
        activityId={activityId}
        timelineId={activityId}
        timelineAliases={timelineAliases}
      />
    );
  }
  if (event.kind === "plan") {
    return (
      <article className="provider-event plan" {...timelineProps}>
        <header className="plan-heading">
          <strong>计划</strong>
          <PlanActions markdown={event.text ?? ""} fallbackTitle={title} />
        </header>
        <MarkdownContent
          text={event.text ?? ""}
          activityId={activityId}
          foldInstructions={false}
        />
      </article>
    );
  }
  if (event.kind === "thinking") {
    if (!visibleMarkdown(event.text ?? "").trim()) return null;
    return (
      <ActivityDetails
        timelineId={activityId}
        timelineAliases={timelineAliases}
        className="execution-group provider-thinking"
        stateKey={activityId}
        initiallyOpen={false}
        summary={
          <>
            <Brain size={16} aria-hidden="true" />
            <span className="activity-label">已思考</span>
          </>
        }
      >
        {() => (
          <div className="thinking-body">
            <MarkdownContent
              text={event.text ?? ""}
              activityId={`${activityId}:thinking`}
              foldInstructions={false}
            />
          </div>
        )}
      </ActivityDetails>
    );
  }
  if (event.detail) {
    return (
      <ActivityDetails
        timelineId={activityId}
        timelineAliases={timelineAliases}
        className="grok-tool"
        stateKey={activityId}
        initiallyOpen={false}
        summary={<span className="tool-title">{event.text ?? "活动记录"}</span>}
      >
        {() => <ToolOutput text={event.detail!} />}
      </ActivityDetails>
    );
  }
  if (!visibleMarkdown(event.text ?? "").trim()) return null;
  const isAssistant = event.kind.startsWith("assistant_message");
  return (
    <article
      className={`provider-event ${event.kind}${isAssistant ? " assistant-reply" : ""}`}
      {...timelineProps}
    >
      {!isAssistant && (
        <div className="chat-meta">{providerEventLabel(event, providerLabel)}</div>
      )}
      <MarkdownContent
        text={event.text ?? ""}
        activityId={activityId}
        timelineAliases={timelineAliases}
        foldInstructions={event.kind !== "compaction" && event.kind !== "branch_summary"}
      />
      {isAssistant && <CopyReplyButton text={event.text ?? ""} />}
    </article>
  );
}

function providerEventLabel(event: ProviderHistoryEvent, providerLabel: string): string {
  if (event.kind === "user_message") return "你";
  if (event.kind === "assistant_message") return providerLabel;
  if (event.kind === "thinking") return "思考";
  if (event.kind === "failure") return "失败";
  if (event.kind === "compaction") return "压缩摘要";
  if (event.kind === "branch_summary") return "分支摘要";
  return event.role || providerLabel;
}

function providerStatusLabel(session: ProviderSessionSummary): string {
  if (session.status === "running") return "运行中";
  if (session.archived) return "已归档";
  if (session.status === "unknown") return "状态未知";
  return session.updatedAt ? new Date(session.updatedAt).toLocaleString() : "最近";
}

export function providerSessionLabel(session: Pick<ProviderSessionSummary, "id" | "title">): string {
  return session.title.trim() || session.id;
}
