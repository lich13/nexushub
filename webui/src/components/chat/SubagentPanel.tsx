import { ArrowLeft, Bot, ChevronRight, X } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { mergeSubagentUpdates } from "../../lib/threadMessageStore";
import { useSubagentDetail } from "../../lib/query/subagents";
import { groupCodexCommandBlocks } from "../../lib/domain/executionGroups";
import { useHistoryScroll } from "../../lib/useHistoryScroll";
import { HistoryLoading } from "../common/HistoryLoading";
import { machineScope } from "../../lib/query/connection";
import { visibleMarkdown } from "../../lib/domain/visibleMarkdown";
import type { MessageBlock, SubagentActivity, SubagentCollection } from "../../types";
import { ExecutionGroupView } from "../common/ExecutionGroupView";
import { DisclosureScope, ActivityDetails } from "../common/ActivityDetails";
import { UserMessageScope } from "../common/UserMessage";
import { MarkdownPathScope } from "../common/FilePathLink";
import { RunningIndicator } from "../common/RunningIndicator";
import { MessageBlockView } from "./MessageStream";
import { subagentStatusLabel } from "./SubagentActivity";
import { SubagentSummary } from "./SubagentSummary";

type Frame = { kind: "detail"; agent: SubagentActivity } | { kind: "list"; agent?: SubagentActivity };
const priority: Record<SubagentActivity["status"], number> = { running: 0, creating: 1, failed: 2, interrupted: 3, unknown: 4, completed: 5 };

export function SubagentPanel({ rootThreadId, initialAgent, rootSubagents, onClose, docked }: {
  rootThreadId: string; initialAgent?: SubagentActivity; rootSubagents?: SubagentCollection | null;
  onClose: () => void; docked: boolean;
}) {
  const [stack, setStack] = useState<Frame[]>([initialAgent ? { kind: "detail", agent: initialAgent } : { kind: "list" }]);
  const frame = stack[stack.length - 1];
  const agent = frame.agent;
  const frameKey = `${frame.kind}:${agent?.agentId ?? rootThreadId}`;
  const panel = useRef<HTMLElement>(null);
  const close = useRef<HTMLButtonElement>(null);
  const stream = useRef<HTMLDivElement>(null);
  const positions = useRef(new Map<string, { top: number; follow: boolean; returnTo?: string }>());
  const restore = useRef<{ top: number; returnTo?: string } | null>(null);
  const query = useSubagentDetail(rootThreadId, agent?.agentId);
  const detail = agent ? query.data?.pages[0]?.detail : undefined;
  const current = (agent ? query.data?.pages[0]?.agent : undefined) ?? agent;
  const collection = agent ? detail?.subagents : rootSubagents;
  const blocks = useMemo(() => {
    const map = new Map<string, MessageBlock>();
    for (const page of [...(query.data?.pages ?? [])].reverse()) for (const block of page.detail.blocks) map.set(block.id, block);
    return mergeSubagentUpdates([...map.values()], query.data?.pages[0]?.detail.subagent_updates);
  }, [query.data]);
  const items = groupCodexCommandBlocks(blocks);
  const oldestPage = query.data?.pages[query.data.pages.length - 1];
  const history = useHistoryScroll({
    scopeKey: `${machineScope()}:subagent:${rootThreadId}:${frameKey}`, streamRef: stream,
    cursor: oldestPage?.detail.before_cursor, hasMore: frame.kind === "detail" && Boolean(query.hasNextPage),
    ready: frame.kind === "list" || Boolean(detail), busy: query.isFetching,
    initialPosition: positions.current.get(frameKey) ?? (frame.kind === "list" ? { top: 0, follow: false } : undefined),
    loadPage: async () => {
      const result = await query.fetchNextPage({ cancelRefetch: false });
      if (result.isError) throw result.error;
      const page = result.data?.pages[result.data.pages.length - 1]?.detail;
      if (!page) throw new Error("历史读取失败");
      return { cursor: page.before_cursor, hasMore: Boolean(page.has_more_blocks) };
    }
  });
  const savePosition = (trigger?: HTMLButtonElement) => {
    if (stream.current) positions.current.set(frameKey, {
      top: stream.current.scrollTop, follow: history.follow.current,
      returnTo: trigger?.closest<HTMLElement>("[data-timeline-id], [data-agent-key]")?.getAttribute("data-timeline-id")
        ?? trigger?.getAttribute("data-agent-key") ?? (trigger ? "summary" : undefined)
    });
  };
  const openAgent = (next: SubagentActivity, trigger: HTMLButtonElement) => {
    if (!next.available || !next.agentId || stack.some(item => item.kind === "detail" && item.agent.agentId === next.agentId)) return;
    savePosition(trigger);
    setStack(items => [...items, { kind: "detail", agent: next }]);
  };
  useLayoutEffect(() => {
    const saved = positions.current.get(frameKey);
    restore.current = saved ?? null;
    close.current?.focus({ preventScroll: true });
  }, [frameKey, frame.kind]);
  useLayoutEffect(() => {
    const element = stream.current;
    if (!element) return;
    if (restore.current && (detail || frame.kind === "list")) {
      const id = restore.current.returnTo;
      const target = Array.from(panel.current?.querySelectorAll<HTMLElement>("[data-timeline-id], [data-agent-key]") ?? [])
        .find(item => item.dataset.timelineId === id || item.dataset.agentKey === id);
      const button = id === "summary" ? panel.current?.querySelector<HTMLButtonElement>('[aria-label="查看直属子智能体"]')
        : target?.matches("button") ? target as HTMLButtonElement : target?.querySelector<HTMLButtonElement>("button");
      button?.focus({ preventScroll: true });
      restore.current = null;
    }
    history.capture();
  }, [blocks, detail, frame.kind, query.isFetchingNextPage, history.capture]);
  useEffect(() => {
    const keydown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !document.querySelector("dialog[open]")) { event.preventDefault(); onClose(); }
      if (docked || event.key !== "Tab" || !panel.current) return;
      const controls = Array.from(panel.current.querySelectorAll<HTMLElement>('button:not(:disabled), [href], input, summary, [tabindex="0"]'))
        .filter(element => element.getClientRects().length > 0);
      const first = controls[0], last = controls[controls.length - 1];
      if (event.shiftKey && (document.activeElement === first || !panel.current.contains(document.activeElement))) { event.preventDefault(); last?.focus(); }
      else if (!event.shiftKey && (document.activeElement === last || !panel.current.contains(document.activeElement))) { event.preventDefault(); first?.focus(); }
    };
    document.addEventListener("keydown", keydown);
    return () => document.removeEventListener("keydown", keydown);
  }, [docked, onClose]);
  return <>
    {!docked && <button type="button" className="subagent-backdrop" aria-label="关闭子智能体详情" tabIndex={-1} onClick={onClose} />}
    <aside ref={panel} className={`subagent-panel ${docked ? "docked" : "overlay"}`} role="dialog" aria-modal={!docked} aria-label="子智能体详情">
      <header className="subagent-panel-header">
        {stack.length > 1 && <button type="button" className="icon-button" aria-label={stack[stack.length - 2].kind === "list" ? "返回子智能体列表" : "返回上一级子智能体"} onClick={() => { savePosition(); setStack(items => items.slice(0, -1)); }}><ArrowLeft size={17} /></button>}
        <div><strong title={frame.kind === "list" ? "直属子智能体" : visibleMarkdown(current?.name ?? "子智能体")}>{frame.kind === "list" ? "直属子智能体" : visibleMarkdown(current?.name ?? "子智能体")}</strong>
          {frame.kind === "detail" && current && <span>{current.status === "running" && <RunningIndicator />}{subagentStatusLabel[current.status]}</span>}
        </div>
        <button ref={close} type="button" className="icon-button" aria-label="关闭子智能体详情" onClick={onClose}><X size={18} /></button>
      </header>
      {frame.kind === "detail" && detail && <SubagentSummary collection={collection} onOpen={trigger => {
        savePosition(trigger); setStack(items => [...items, { kind: "list", agent: current }]);
      }} />}
      <div ref={stream} className="subagent-stream message-stream" onScroll={history.onScroll} onWheel={history.onWheel} onTouchStart={history.onTouchStart} onTouchMove={history.onTouchMove} onKeyDown={history.onKeyDown} tabIndex={0}>
        {agent && query.isLoading && <div role="status" className="muted-row">正在读取子智能体…</div>}
        {agent && query.error && !query.isFetchNextPageError && <div className="form-error" role="alert">{query.error.message}<button className="file-path-label" onClick={() => void query.refetch()}>重试</button></div>}
        {frame.kind === "list" ? <>
          {!collection && !query.isLoading && <div className="muted-row">更新当前机器服务后可查看子智能体汇总。</div>}
          {[...(collection?.agents ?? [])].sort((a, b) => priority[a.status] - priority[b.status]).map(child => <button
            type="button" className="subagent-list-item" key={child.agentId ?? child.eventId} data-agent-key={child.agentId ?? child.eventId}
            aria-disabled={!child.available} title={child.unavailableReason ?? undefined} onClick={event => openAgent(child, event.currentTarget)}>
            <Bot size={16} aria-hidden="true" /><span>{visibleMarkdown(child.name)}</span>
            {child.status === "running" && <RunningIndicator />}<small>{subagentStatusLabel[child.status]}</small>
            {child.available && <ChevronRight size={15} aria-hidden="true" />}
          </button>)}
          {collection?.complete && !collection.agents.length && <div className="muted-row">暂无直属子智能体</div>}
        </> : agent && <>
          <HistoryLoading loading={history.loading} error={history.error} retry={history.retry} />
          <UserMessageScope.Provider value={{ provider: "codex", sessionKey: agent.agentId!, rootThreadId }}>
            <MarkdownPathScope.Provider value={detail?.summary.cwd}><DisclosureScope.Provider value={`codex:${rootThreadId}:agent:${agent.agentId}`}>
              {agent.delegation && <ActivityDetails stateKey="delegation" className="subagent-delegation" initiallyOpen={false} summary={<span>委派内容</span>}>
                {() => <div className="user-instructions-body">{visibleMarkdown(agent.delegation!)}</div>}
              </ActivityDetails>}
              {items.map(entry => entry.kind === "group" ? <ExecutionGroupView key={entry.group.id} group={entry.group} />
                : <MessageBlockView key={entry.item.id} block={entry.item} planFallbackTitle={detail?.summary.title ?? agent.name} onOpenSubagent={openAgent} />)}
            </DisclosureScope.Provider></MarkdownPathScope.Provider>
          </UserMessageScope.Provider>
          {detail && !blocks.length && <div className="muted-row">暂无子智能体消息</div>}
        </>}
      </div>
    </aside>
  </>;
}
