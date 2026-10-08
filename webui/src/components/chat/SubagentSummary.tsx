import { Bot, ChevronRight } from "lucide-react";
import type { SubagentCollection } from "../../types";
import { RunningIndicator } from "../common/RunningIndicator";

export function SubagentSummary({ collection, onOpen }: {
  collection?: SubagentCollection | null;
  onOpen: (trigger: HTMLButtonElement) => void;
}) {
  if (!collection) return <div className="subagent-summary-note" role="status">更新当前机器服务后可查看子智能体汇总。</div>;
  if (!collection.agents.length && collection.complete) return null;
  const counts = collection.counts;
  return <div className="subagent-summary">
    <button type="button" aria-label="查看直属子智能体" onClick={event => onOpen(event.currentTarget)}>
      <Bot size={16} aria-hidden="true" /><span>子智能体</span>
      {counts.running > 0 && <span className="subagent-summary-running"><RunningIndicator />{counts.running} 个运行中</span>}
      {counts.completed > 0 && <span>{counts.completed} 已完成</span>}
      {counts.creating > 0 && <span>{counts.creating} 正在创建</span>}
      {counts.failed > 0 && <span className="subagent-summary-error">{counts.failed} 失败</span>}
      {counts.interrupted > 0 && <span>{counts.interrupted} 已中断</span>}
      {counts.unknown > 0 && <span>{counts.unknown} 状态未知</span>}
      <ChevronRight size={14} aria-hidden="true" />
    </button>
    {!collection.complete && <div className="subagent-summary-note" role="status">{collection.warning || "部分记录无法确认，统计可能不完整"}</div>}
  </div>;
}
