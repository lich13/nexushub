import { Brain, Sparkles, Terminal } from "lucide-react";
import { memo, useContext } from "react";
import { matchesTimelinePosition } from "../../lib/domain/timelineViewModel";
import {
  processGroupSummaryText,
  type ProcessStep,
  type UnifiedProcessGroup,
} from "../../lib/domain/processGroups";
import { ActivityDetails, DisclosureRevealContext } from "./ActivityDetails";
import { ExecutionCommandRow } from "./ExecutionGroupView";
import { ToolOutput } from "./FilePathLink";
import { MarkdownContent } from "./MarkdownContent";
import { RunningIndicator } from "./RunningIndicator";

export const ProcessGroupView = memo(function ProcessGroupView({
  group,
}: {
  group: UnifiedProcessGroup;
}) {
  const positionKey = useContext(DisclosureRevealContext);
  const reveal = Boolean(
    positionKey &&
      (matchesTimelinePosition(positionKey, group.id, group.timelineAliases) ||
        group.steps.some(step =>
          step.kind === "execution"
            ? step.group.commands.some(cmd =>
                matchesTimelinePosition(positionKey, cmd.id, cmd.sourceIds),
              )
            : matchesTimelinePosition(positionKey, step.id, step.sourceIds),
        )),
  );
  const { primary, meta } = processGroupSummaryText(group);
  const Icon = group.thoughtCount > 0 ? (group.toolCount > 0 ? Sparkles : Brain) : Terminal;

  return (
    <ActivityDetails
      timelineId={group.id}
      className={`execution-group process-group${group.running ? " is-running" : ""}`}
      stateKey={group.stateKey}
      stateAliases={group.stateAliases}
      reveal={reveal}
      initiallyOpen={group.failedCount > 0}
      summary={
        <>
          <Icon size={15} aria-hidden="true" />
          <span className="activity-label">{primary}</span>
          {meta && <small>{meta}</small>}
          {group.failedCount > 0 && (
            <span className="process-failure-badge">{group.failedCount} 项失败</span>
          )}
          {group.running && <RunningIndicator />}
        </>
      }
    >
      {() => (
        <div className="process-group-body" data-history-scroll-container="commands">
          {group.steps.map(step => (
            <ProcessStepView
              key={step.kind === "execution" ? step.group.id : step.id}
              step={step}
            />
          ))}
        </div>
      )}
    </ActivityDetails>
  );
});

function ProcessStepView({ step }: { step: ProcessStep }) {
  if (step.kind === "thinking") {
    return (
      <div
        className="process-thought-step"
        data-timeline-id={step.id}
        data-timeline-aliases={step.sourceIds.length ? step.sourceIds.join(" ") : undefined}
      >
        <MarkdownContent
          text={step.text}
          activityId={`${step.id}:thinking`}
          timelineAliases={step.sourceIds}
          foldInstructions={false}
        />
      </div>
    );
  }
  if (step.kind === "execution") {
    return (
      <div className="process-execution-steps">
        {step.group.commands.map(command => (
          <ExecutionCommandRow key={command.id} command={command} />
        ))}
      </div>
    );
  }
  return (
    <ActivityDetails
      timelineId={step.id}
      timelineAliases={step.sourceIds}
      className="execution-command"
      stateKey={step.id}
      initiallyOpen={Boolean(step.running || step.failed)}
      summary={
        <>
          <Terminal size={15} aria-hidden="true" />
          <span className="tool-title">{step.title}</span>
          {step.running && <RunningIndicator />}
          <small>{step.failed ? "失败" : step.running ? "运行中" : "已完成"}</small>
        </>
      }
    >
      {() => (
        <div className="execution-section">
          <ToolOutput text={step.detail} />
        </div>
      )}
    </ActivityDetails>
  );
}
