import type { ClaudeHistoryEvent } from "../../types";
import { executionStatus, groupClaudeEvents, type ExecutionRenderItem } from "./executionGroups";

export type ClaudeProcessGroup = {
  id: string;
  stateKey: string;
  turnId: string;
  sourceIds: string[];
  entries: ClaudeEventRenderItem[];
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
export type ClaudeMetadataGroup = { id: string; sourceIds: string[]; label: string; entries: ExecutionRenderItem<ClaudeHistoryEvent>[] };
export type ClaudeEventRenderItem = ExecutionRenderItem<ClaudeHistoryEvent> | { kind: "metadata"; group: ClaudeMetadataGroup };
export type ClaudeRenderItem = ClaudeEventRenderItem | { kind: "process"; process: ClaudeProcessGroup };

const METADATA_KINDS = new Set(["file-history-snapshot", "file-history-delta", "queue-operation", "cost-state", "atis-latch", "attachment", "compaction", "branch_summary"]);
const AUTOMATIC_ROLES = new Set(["native_context", "local_command", "task_notification", "turn_duration", "away_summary", "stop_hook_summary"]);
function isAutomatic(event: ClaudeHistoryEvent) {
  return METADATA_KINDS.has(event.kind) || (event.kind === "system" && AUTOMATIC_ROLES.has(event.role ?? ""));
}
function foldAutomatic(entries: ExecutionRenderItem<ClaudeHistoryEvent>[]): ClaudeEventRenderItem[] {
  const result: ClaudeEventRenderItem[] = [];
  let pending: Extract<ExecutionRenderItem<ClaudeHistoryEvent>, { kind: "item" }>[] = [];
  const flush = () => {
    if (!pending.length) return;
    const sourceIds = pending.flatMap(entry => entry.sourceIds);
    const label = pending.every(entry => ["compaction", "branch_summary"].includes(entry.item.kind)) ? "压缩摘要"
      : pending.every(entry => entry.item.role === "native_context") ? "上下文" : "系统活动";
    result.push({ kind: "metadata", group: { id: `claude-metadata:${sourceIds[0]}`, label, sourceIds, entries: pending } });
    pending = [];
  };
  for (const entry of entries) {
    if (entry.kind === "item" && isAutomatic(entry.item)) {
      pending.push(entry);
      continue;
    }
    if (
      entry.kind === "group" &&
      pending.length > 0 &&
      pending.every(item => !["compaction", "branch_summary"].includes(item.item.kind))
    ) {
      const prev = result[result.length - 1];
      if (
        prev?.kind === "group" &&
        (!prev.group.turnId || !entry.group.turnId || prev.group.turnId === entry.group.turnId)
      ) {
        const commands = [...prev.group.commands, ...entry.group.commands];
        const mergedIds = [
          ...(prev.group.mergedIds ?? []),
          entry.group.id,
          ...(entry.group.mergedIds ?? [])
        ];
        result[result.length - 1] = {
          kind: "group",
          group: {
            ...prev.group,
            mergedIds,
            commands,
            kind: entry.group.kind === "tool" ? "tool" : prev.group.kind,
            turnId: prev.group.turnId || entry.group.turnId,
            running: commands.some(command => command.status === "running"),
            failedCount: commands.filter(command => command.status === "failed").length
          }
        };
        continue;
      }
    }
    flush();
    result.push(entry);
  }
  flush();
  return result;
}

const PROCESS_KINDS = new Set(["thinking", "tool_call", "tool_result", "tool_reference", "plan", "failure", "permission_request", "permission_response", "file-history-snapshot", "file-history-delta", "queue-operation", "cost-state", "atis-latch", "attachment"]);
function isProcess(event: ClaudeHistoryEvent) {
  return PROCESS_KINDS.has(event.kind) || (event.kind === "assistant_message" && event.assistantPhase === "intermediate")
    || (event.kind === "system" && (AUTOMATIC_ROLES.has(event.role ?? "") || event.role === "api_error"));
}

function groupTurn(events: ClaudeHistoryEvent[], turnId: string, context: string, ended: boolean): ClaudeRenderItem[] {
  if (events.every(isAutomatic)) return foldAutomatic(groupClaudeEvents(events));
  const first = events.findIndex(isProcess);
  const final = events.findIndex((event, index) => index > first && event.assistantPhase === "final");
  // A native final answer (or a terminal non-process reply/API error followed
  // only by automatic turn-duration/cost-state bookkeeping) closes the process.
  const closing = final >= 0
    ? final
    : events.findIndex(
        (event, index) =>
          index > first &&
          !isProcess(event) &&
          events.slice(index + 1).every(later => !isProcess(later) || isAutomatic(later)),
      );
  let last = closing >= 0 ? closing - 1 : events.length - 1;
  while (last >= 0 && !isProcess(events[last])) last--;
  // Unknown-phase prose stays at its native position. If it interrupts the
  // process, retain the existing presentation instead of reordering or hiding it.
  if (first < 0 || events.slice(first, last + 1).some(event => !isProcess(event))) return foldAutomatic(groupClaudeEvents(events));
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
    ...foldAutomatic(groupClaudeEvents(events.slice(0, first))),
    { kind: "process", process: {
      id: `claude-process[${turnId}][${context}]`, stateKey, turnId,
      sourceIds: processEvents.map(event => event.id), entries: foldAutomatic(entries),
      segmentCount: processEvents.filter(event => !["tool_call", "tool_result"].includes(event.kind)).length,
      toolCount: commands.length + processEvents.filter(event => event.kind === "plan" && event.callId).length,
      failedCount, completed, needsReply, needsPermission,
      running: !closed && !needsReply && !needsPermission && (commands.some(command => command.status === "running") || processEvents.some(pending)),
      hasPermission: processEvents.some(event => event.kind === "permission_request"),
      hasPlan: processEvents.some(event => event.kind === "plan")
    } },
    ...foldAutomatic(groupClaudeEvents(events.slice(last + 1)))
  ];
}

/** Display-only grouping. Native event IDs and inner tool/result aliases survive. */
export function groupClaudeProcesses(events: ClaudeHistoryEvent[]): ClaudeRenderItem[] {
  const output: ClaudeRenderItem[] = [];
  let turn: string | null | undefined;
  let context = "";
  let pending: ClaudeHistoryEvent[] = [];
  const flush = (ended: boolean) => {
    if (pending.length) output.push(...(turn ? groupTurn(pending, turn, context, ended) : foldAutomatic(groupClaudeEvents(pending))));
    pending = [];
  };
  for (const event of events) {
    if (event.turnId !== turn) { flush(true); turn = event.turnId; context = ""; }
    if (["user_message", "compaction", "branch_summary"].includes(event.kind)) {
      flush(true);
      output.push(...foldAutomatic(groupClaudeEvents([event])));
      context = event.kind === "user_message" ? "" : event.id;
    } else pending.push(event);
  }
  flush(false);
  return output;
}
