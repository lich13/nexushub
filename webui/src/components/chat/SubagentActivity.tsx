import { Bot, ChevronRight } from "lucide-react";
import type { MessageBlock, SubagentActivity as Agent } from "../../types";
import { RunningIndicator } from "../common/RunningIndicator";
import { visibleMarkdown } from "../../lib/domain/visibleMarkdown";

export const subagentStatusLabel: Record<Agent["status"], string> = {
  running: "运行中", completed: "已完成", failed: "失败", interrupted: "已中断", unknown: "状态未知"
};
export type OpenSubagent = (agent: Agent, trigger: HTMLButtonElement) => void;

export function SubagentActivityRow({ block, onOpen, supported = true }: { block: MessageBlock; onOpen?: OpenSubagent; supported?: boolean }) {
  const agent = block.subagent!;
  const available = supported && agent.available && !!agent.agentId && !!onOpen;
  const reason = supported ? agent.unavailableReason : "请更新当前机器服务以查看子智能体";
  return <div className="subagent-activity" data-timeline-id={block.id}>
    <button type="button" className="subagent-activity-button" aria-disabled={!available} title={reason || undefined}
      onClick={event => { if (available) onOpen?.(agent, event.currentTarget); }}>
      <Bot size={17} aria-hidden="true" />
      <span className="subagent-name">{visibleMarkdown(agent.name)}</span>
      {agent.status === "running" && <RunningIndicator />}
      <span className={`subagent-state ${agent.status}`}>{subagentStatusLabel[agent.status]}</span>
      {available && <ChevronRight size={15} aria-hidden="true" />}
    </button>
    {!available && reason && <small className="subagent-unavailable">{reason}</small>}
  </div>;
}
