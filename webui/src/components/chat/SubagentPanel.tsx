import { ArrowLeft, X } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { mergeSubagentUpdates } from "../../lib/threadMessageStore";
import { useSubagentDetail } from "../../lib/query/subagents";
import { groupCodexCommandBlocks } from "../../lib/domain/executionGroups";
import { shouldAutoFollowMessageStream } from "../../lib/domain/conversationViewModel";
import { visibleMarkdown } from "../../lib/domain/visibleMarkdown";
import type { MessageBlock, SubagentActivity } from "../../types";
import { ExecutionGroupView } from "../common/ExecutionGroupView";
import { DisclosureScope, ActivityDetails } from "../common/ActivityDetails";
import { UserMessageScope } from "../common/UserMessage";
import { MarkdownPathScope } from "../common/FilePathLink";
import { RunningIndicator } from "../common/RunningIndicator";
import { MessageBlockView } from "./MessageStream";
import { subagentStatusLabel } from "./SubagentActivity";

export function SubagentPanel({ rootThreadId, initialAgent, onClose, docked }: {
  rootThreadId: string; initialAgent: SubagentActivity; onClose: () => void; docked: boolean;
}) {
  const [stack, setStack] = useState([initialAgent]);
  const agent = stack[stack.length - 1];
  const panel = useRef<HTMLElement>(null);
  const close = useRef<HTMLButtonElement>(null);
  const stream = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  const prepend = useRef<number | null>(null);
  const positions = useRef(new Map<string, { top: number; follow: boolean; returnTo?: string }>());
  const restore = useRef<{ top: number; returnTo?: string } | null>(null);
  const query = useSubagentDetail(rootThreadId, agent.agentId);
  const detail = query.data?.pages[0]?.detail;
  const current = query.data?.pages[0]?.agent ?? agent;
  const blocks = useMemo(() => {
    const map = new Map<string, MessageBlock>();
    for (const page of [...(query.data?.pages ?? [])].reverse()) for (const block of page.detail.blocks) map.set(block.id, block);
    return mergeSubagentUpdates([...map.values()], query.data?.pages[0]?.detail.subagent_updates);
  }, [query.data]);
  const items = groupCodexCommandBlocks(blocks);
  useLayoutEffect(() => {
    const saved = agent.agentId ? positions.current.get(agent.agentId) : undefined;
    follow.current = saved?.follow ?? true;
    restore.current = saved ?? null;
    prepend.current = null;
    close.current?.focus({ preventScroll: true });
  }, [agent.agentId]);
  useLayoutEffect(() => {
    const element = stream.current;
    if (!element) return;
    if (restore.current && detail) {
      element.scrollTop = restore.current.top;
      const target = Array.from(element.querySelectorAll<HTMLElement>("[data-timeline-id]"))
        .find(item => item.dataset.timelineId === restore.current?.returnTo);
      target?.querySelector<HTMLButtonElement>("button")?.focus({ preventScroll: true });
      restore.current = null;
    } else if (prepend.current !== null && !query.isFetchingNextPage) {
      element.scrollTop = element.scrollHeight - prepend.current;
      prepend.current = null;
    } else if (follow.current) element.scrollTop = element.scrollHeight;
  }, [blocks, detail, query.isFetchingNextPage]);
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
        {stack.length > 1 && <button type="button" className="icon-button" aria-label="返回上一级子智能体" onClick={() => setStack(items => items.slice(0, -1))}><ArrowLeft size={17} /></button>}
        <div><strong title={visibleMarkdown(current.name)}>{visibleMarkdown(current.name)}</strong><span>{current.status === "running" && <RunningIndicator />}{subagentStatusLabel[current.status]}</span></div>
        <button ref={close} type="button" className="icon-button" aria-label="关闭子智能体详情" onClick={onClose}><X size={18} /></button>
      </header>
      <div ref={stream} className="subagent-stream message-stream" onScroll={event => { follow.current = shouldAutoFollowMessageStream(event.currentTarget); }}>
        {query.isLoading && <div role="status" className="muted-row">正在读取子智能体…</div>}
        {query.error && <div className="form-error" role="alert">{query.error.message}<button className="file-path-label" onClick={() => void query.refetch()}>重试</button></div>}
        {query.hasNextPage && <button type="button" className="secondary-button" disabled={query.isFetchingNextPage} onClick={() => {
          if (stream.current) prepend.current = stream.current.scrollHeight - stream.current.scrollTop;
          follow.current = false;
          void query.fetchNextPage();
        }}>较早消息</button>}
        <UserMessageScope.Provider value={{ provider: "codex", sessionKey: agent.agentId!, rootThreadId }}>
          <MarkdownPathScope.Provider value={detail?.summary.cwd}><DisclosureScope.Provider value={`codex:${rootThreadId}:agent:${agent.agentId}`}>
            {agent.delegation && <ActivityDetails stateKey="delegation" className="subagent-delegation" initiallyOpen={false} summary={<span>委派内容</span>}>
              {() => <div className="user-instructions-body">{visibleMarkdown(agent.delegation!)}</div>}
            </ActivityDetails>}
            {items.map(entry => entry.kind === "group" ? <ExecutionGroupView key={entry.group.id} group={entry.group} />
              : <MessageBlockView key={entry.item.id} block={entry.item} planFallbackTitle={detail?.summary.title ?? agent.name}
                onOpenSubagent={(next, trigger) => {
                  if (!next.agentId || stack.some(item => item.agentId === next.agentId)) return;
                  if (agent.agentId && stream.current) positions.current.set(agent.agentId, {
                    top: stream.current.scrollTop, follow: follow.current,
                    returnTo: trigger.closest<HTMLElement>("[data-timeline-id]")?.dataset.timelineId
                  });
                  setStack(items => [...items, next]);
                }} />)}
          </DisclosureScope.Provider></MarkdownPathScope.Provider>
        </UserMessageScope.Provider>
        {detail && !blocks.length && <div className="muted-row">暂无子智能体消息</div>}
      </div>
    </aside>
  </>;
}
