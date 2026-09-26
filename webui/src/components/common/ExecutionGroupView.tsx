import { ChevronRight } from "lucide-react";
import { useState, type ReactNode } from "react";
import type { ExecutionCommand, ExecutionGroup } from "../../lib/domain/executionGroups";
import { RunningIndicator } from "./RunningIndicator";

function ActivityDetails({ className, initiallyOpen, summary, children }: {
  className: string; initiallyOpen: boolean; summary: ReactNode; children: ReactNode;
}) {
  // A user choice wins over subsequent status/stream updates for this identity.
  const [choice, setChoice] = useState<boolean>();
  const open = choice ?? initiallyOpen;
  return <details className={className} open={open} onToggle={event => {
    if (event.target !== event.currentTarget) return;
    if (event.currentTarget.open !== open) setChoice(event.currentTarget.open);
  }}>
    <summary>{summary}<ChevronRight className="execution-chevron" size={16} /></summary>
    {children}
  </details>;
}

function CommandView({ command }: { command: ExecutionCommand }) {
  return <ActivityDetails className="execution-command" initiallyOpen={command.status !== "completed"} summary={<>
    <span className="tool-title">{command.title}</span>
    {command.status === "running" && <RunningIndicator />}
    <small>{command.status}</small>
  </>}>
    {command.sections.length ? command.sections.map((section, index) => <div className="execution-section" key={`${section.label}-${index}`}>
      <span>{section.label}</span><pre>{section.text}</pre>
    </div>) : <p className="muted-text">暂无输出</p>}
  </ActivityDetails>;
}

export function ExecutionGroupView({ group }: { group: ExecutionGroup }) {
  return <ActivityDetails className="execution-group" initiallyOpen={group.running || group.failedCount > 0} summary={<>
    <span>命令执行组</span>
    <small>{group.commands.length} 条命令{group.failedCount ? ` · ${group.failedCount} 条失败` : ""}</small>
    {group.running && <RunningIndicator />}
  </>}>
    {group.commands.map(command => <CommandView key={command.id} command={command} />)}
  </ActivityDetails>;
}
