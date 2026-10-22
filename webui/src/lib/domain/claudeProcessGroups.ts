import type { ClaudeHistoryEvent } from "../../types";
import { executionStatus, groupClaudeEvents, type ExecutionRenderItem } from "./executionGroups";

export type ClaudeProcessGroup = {
  id: string;
  stateKey: string;
  turnId: string;
  sourceIds: string[];
  entries: ExecutionRenderItem<ClaudeHistoryEvent>[];
  segmentCount: number;
  toolCount: number;
  failedCount: number;
  running: boolean;
  completed: boolean;
  needsReply: boolean;
  needsPermission: boolean;
  hasPermission: boolean;
  hasPlan: boolean;
};
export type ClaudeRenderItem = ExecutionRenderItem<ClaudeHistoryEvent> | { kind: "process"; process: ClaudeProcessGroup };

const PROCESS_KINDS = new Set(["thinking", "tool_call", "tool_result", "tool_reference", "plan", "failure", "permission_request", "permission_response", "file-history-snapshot", "file-history-delta", "queue-operation", "cost-state", "atis-latch", "attachment"]);
function isProcess(event: ClaudeHistoryEvent) {
  return PROCESS_KINDS.has(event.kind) || (event.kind === "assistant_message" && event.assistantPhase === "intermediate")
    || (event.kind === "system" && ["turn_duration", "away_summary", "stop_hook_summary", "api_error"].includes(event.role ?? ""));
}

function groupTurn(events: ClaudeHistoryEvent[], turnId: string, context: string, ended: boolean): ClaudeRenderItem[] {
  const first = events.findIndex(isProcess);
  const final = events.findIndex((event, index) => index > first && event.assistantPhase === "final");
  // A native final answer closes the process. Later bookkeeping stays in its
  // original position rather than pulling the final answer into the disclosure.
  let last = final >= 0 ? final - 1 : events.length - 1;
  while (last >= 0 && !isProcess(events[last])) last--;
  // Unknown-phase prose stays at its native position. If it interrupts the
  // process, retain the existing presentation instead of reordering or hiding it.
  if (first < 0 || events.slice(first, last + 1).some(event => !isProcess(event))) return groupClaudeEvents(events);
  const processEvents = events.slice(first, last + 1);
  const entries = groupClaudeEvents(processEvents);
  const commands = entries.flatMap(entry => entry.kind === "group" ? entry.group.commands : []);
  const failedCount = commands.filter(command => command.status === "failed").length
    + processEvents.filter(event => event.kind !== "tool_call" && event.kind !== "tool_result" && executionStatus(event.status) === "failed").length;
  const completed = events.slice(last + 1).some(event => event.assistantPhase === "final");
  const closed = ended || completed || processEvents.some(event => event.kind === "failure");
  const runningToolIds = new Set(commands.filter(command => command.status === "running").flatMap(command => command.sourceIds));
  const pending = (event: ClaudeHistoryEvent) => !closed && (["tool_call", "tool_result"].includes(event.kind)
    ? runningToolIds.has(event.id) : executionStatus(event.status) === "running");
  const needsReply = processEvents.some(event => event.kind === "tool_call" && event.role === "AskUserQuestion" && pending(event));
  const needsPermission = processEvents.some(event => event.kind === "permission_request" && pending(event));
  const stateKey = `claude-process:${turnId}`;
  return [
    ...groupClaudeEvents(events.slice(0, first)),
    { kind: "process", process: {
      id: `claude-process[${turnId}][${context}]`, stateKey, turnId,
      sourceIds: processEvents.map(event => event.id), entries,
      segmentCount: processEvents.filter(event => !["tool_call", "tool_result"].includes(event.kind)).length,
      toolCount: commands.length + processEvents.filter(event => event.kind === "plan" && event.callId).length,
      failedCount, completed, needsReply, needsPermission,
      running: !closed && !needsReply && !needsPermission && (commands.some(command => command.status === "running") || processEvents.some(pending)),
      hasPermission: processEvents.some(event => event.kind === "permission_request"),
      hasPlan: processEvents.some(event => event.kind === "plan")
    } },
    ...groupClaudeEvents(events.slice(last + 1))
  ];
}

/** Display-only grouping. Native event IDs and inner tool/result aliases survive. */
export function groupClaudeProcesses(events: ClaudeHistoryEvent[]): ClaudeRenderItem[] {
  const output: ClaudeRenderItem[] = [];
  let turn: string | null | undefined;
  let context = "";
  let pending: ClaudeHistoryEvent[] = [];
  const flush = (ended: boolean) => {
    if (pending.length) output.push(...(turn ? groupTurn(pending, turn, context, ended) : groupClaudeEvents(pending)));
    pending = [];
  };
  for (const event of events) {
    if (event.turnId !== turn) { flush(true); turn = event.turnId; context = ""; }
    if (["user_message", "compaction", "branch_summary"].includes(event.kind)) {
      flush(true);
      output.push(...groupClaudeEvents([event]));
      context = event.kind === "user_message" ? "" : event.id;
    } else pending.push(event);
  }
  flush(false);
  return output;
}
