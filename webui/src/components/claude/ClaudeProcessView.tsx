import { useContext, type ReactNode } from "react";
import { Brain } from "lucide-react";
import { ActivityDetails, DisclosureRevealContext } from "../common/ActivityDetails";
import { RunningIndicator } from "../common/RunningIndicator";
import type { ClaudeProcessGroup } from "../../lib/domain/claudeProcessGroups";

export function ClaudeProcessView({ group, children }: { group: ClaudeProcessGroup; children: () => ReactNode }) {
  const position = useContext(DisclosureRevealContext);
  const reveal = Boolean(position && (group.sourceIds.includes(position) || group.entries.some(entry => entry.kind === "item"
    ? entry.key === position || entry.sourceIds.includes(position)
    : entry.group.commands.some(command => command.id === position || command.sourceIds.includes(position)))));
  return <ActivityDetails className="execution-group claude-process-group" stateKey={group.stateKey} timelineId={group.id} initiallyOpen={false} reveal={reveal} summary={<>
    <Brain size={17} aria-hidden="true" /><span className="activity-label">过程</span>
    <small>{group.segmentCount} 段 · {group.toolCount} 项工具</small>
    {group.failedCount > 0 && <span className="form-error">{group.failedCount} 项失败</span>}
    {group.hasPlan && <small>含计划</small>}
    {group.needsReply && <small>待回复</small>}
    {group.hasPermission && <small>{group.needsPermission ? "待授权" : "授权记录"}</small>}
    {group.running ? <RunningIndicator /> : <small>{group.needsReply || group.needsPermission ? "展开查看" : group.completed ? "已完成" : "过程记录"}</small>}
  </>}>{() => <div className="claude-process-body">{children()}</div>}</ActivityDetails>;
}
