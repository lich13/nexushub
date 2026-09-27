import { Blocks, Terminal } from "lucide-react";
import { ToolOutput } from "./FilePathLink";
import { ActivityDetails } from "./ActivityDetails";
import { visibleMarkdown } from "../../lib/domain/visibleMarkdown";
import { activityFingerprint, type ExecutionCommand, type ExecutionGroup } from "../../lib/domain/executionGroups";
import { RunningIndicator } from "./RunningIndicator";

function CommandView({ command }: { command: ExecutionCommand }) {
  return <ActivityDetails className="execution-command" stateKey={command.id} initiallyOpen={!command.instructionFile && command.status !== "completed"} summary={<>
    <Terminal size={16} aria-hidden="true" /><span className="tool-title" title={command.instructionFile ? "AGENTS.md" : visibleMarkdown(`${command.title} ${command.preview ?? ""}`)}>{command.instructionFile ? "AGENTS.md" : <>{visibleMarkdown(command.title)}{command.preview && <span className="command-preview"> {visibleMarkdown(command.preview)}</span>}</>}</span>
    {command.status === "running" && <RunningIndicator />}
    <small>{command.status}</small>
  </>}>
    {command.sections.length ? command.sections.map(section => <div className="execution-section" key={`${section.label}:${activityFingerprint(section.text)}`}>
      <span>{section.label}</span><ToolOutput text={section.text} />
    </div>) : <p className="muted-text">暂无输出</p>}
  </ActivityDetails>;
}

export function ExecutionGroupView({ group }: { group: ExecutionGroup }) {
  return <ActivityDetails className="execution-group" stateKey={group.id} stateAliases={group.commands.map(command => `group:${command.id}`)} initiallyOpen={!group.commands.every(command => command.instructionFile) && (group.running || group.failedCount > 0)} summary={<>
    {group.kind === "tool" ? <Blocks size={17} aria-hidden="true" /> : <Terminal size={17} aria-hidden="true" />}
    <span className="activity-label">{group.running ? "正在使用" : "已使用"} {group.provider} {group.kind === "tool" ? "运行工具" : "运行命令"}</span>
    <small>{group.commands.length} {group.kind === "tool" ? "项工具" : "条命令"}{group.failedCount ? ` · ${group.failedCount} 条失败` : ""}</small>
    {group.running && <RunningIndicator />}
  </>}>
    {() => <div className="execution-rows">{group.commands.map(command => <CommandView key={command.id} command={command} />)}</div>}
  </ActivityDetails>;
}
