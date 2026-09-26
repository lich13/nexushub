import { ActivityDetails } from "./ActivityDetails";
import { visibleMarkdown } from "../../lib/domain/visibleMarkdown";
import type { ExecutionCommand, ExecutionGroup } from "../../lib/domain/executionGroups";
import { RunningIndicator } from "./RunningIndicator";

function CommandView({ command }: { command: ExecutionCommand }) {
  return <ActivityDetails className="execution-command" stateKey={command.id} initiallyOpen={!command.instructionFile && command.status !== "completed"} summary={<>
    <span className="tool-title">{command.instructionFile ? "AGENTS.md" : visibleMarkdown(command.title)}</span>
    {command.status === "running" && <RunningIndicator />}
    <small>{command.status}</small>
  </>}>
    {command.sections.length ? command.sections.map((section, index) => <div className="execution-section" key={`${section.label}-${index}`}>
      <span>{section.label}</span><pre>{visibleMarkdown(section.text)}</pre>
    </div>) : <p className="muted-text">暂无输出</p>}
  </ActivityDetails>;
}

export function ExecutionGroupView({ group }: { group: ExecutionGroup }) {
  return <ActivityDetails className="execution-group" stateKey={group.id} initiallyOpen={!group.commands.every(command => command.instructionFile) && (group.running || group.failedCount > 0)} summary={<>
    <span>{group.kind === "tool" ? "工具活动组" : "命令执行组"}</span>
    <small>{group.commands.length} {group.kind === "tool" ? "项工具" : "条命令"}{group.failedCount ? ` · ${group.failedCount} 条失败` : ""}</small>
    {group.running && <RunningIndicator />}
  </>}>
    {group.commands.map(command => <CommandView key={command.id} command={command} />)}
  </ActivityDetails>;
}
