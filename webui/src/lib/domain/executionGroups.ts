import type { MessageBlock, GrokHistoryEvent, PiHistoryEvent } from "../../types";
import { isToolBlock, toolBlockDetailText } from "./conversationViewModel";

export type ExecutionStatus = "running" | "failed" | "completed";
export type ExecutionCommand = {
  id: string;
  title: string;
  status: ExecutionStatus;
  sections: Array<{ label: string; text: string }>;
};
export type ExecutionGroup = {
  id: string;
  commands: ExecutionCommand[];
  running: boolean;
  failedCount: number;
};
export type ExecutionRenderItem<T> =
  | { kind: "item"; item: T; key: string }
  | { kind: "group"; group: ExecutionGroup };

const RUNNING = new Set(["pending", "running", "in_progress", "inprogress", "active", "generating"]);
const FAILED = new Set(["failed", "error", "cancelled", "canceled", "rejected"]);
const COMMAND_NAMES = new Set([
  "exec", "exec_command", "run_terminal_command", "sleep", "js", "javascript",
  "bash", "bash_execution", "bashexecution", "shell", "terminal", "python", "node", "execute", "command_execution", "commandexecution"
]);

export function executionStatus(value?: string | null): ExecutionStatus {
  const status = value?.trim().toLowerCase() ?? "";
  if (RUNNING.has(status)) return "running";
  if (FAILED.has(status)) return "failed";
  return "completed";
}

// Inspect tool names/titles, never arbitrary output (which may mention another tool).
export function isCommandText(value?: string | null): boolean {
  const name = value?.trim().toLowerCase().split(/[\s`(]/, 1)[0].split(/\.|__|\/|:/).pop() ?? "";
  return COMMAND_NAMES.has(name);
}

function fingerprint(value: string): string {
  let hash = 2166136261;
  for (let i = 0; i < value.length; i++) hash = Math.imul(hash ^ value.charCodeAt(i), 16777619);
  return (hash >>> 0).toString(36);
}

function commandTitle(...values: Array<string | null | undefined>): string {
  for (const value of values) {
    const normalized = value?.trim().toLowerCase().split(/[\s`(]/, 1)[0].split(/\.|__|\/|:/).pop() ?? "";
    if (!COMMAND_NAMES.has(normalized)) continue;
    if (normalized === "bashexecution" || normalized === "bash_execution") return "bash";
    if (normalized === "commandexecution" || normalized === "command_execution") return "execute";
    if (normalized === "run_terminal_command") return "exec";
    return normalized;
  }
  return values.find(value => value?.trim())?.trim().split(/\s+/, 1)[0] || "命令";
}

type Activity<T> = {
  item: T;
  key: string;
  callId?: string | null;
  turnId?: string | null;
  tool: boolean;
  command: boolean;
  title: string;
  phase: "call" | "result" | "update";
  status?: string | null;
  sections: ExecutionCommand["sections"];
};

function groupActivities<T>(activities: Activity<T>[]): ExecutionRenderItem<T>[] {
  type Entry = { kind: "item"; item: T; key: string } | { kind: "command"; command: ExecutionCommand };
  const entries: Entry[] = [];
  const pending = new Map<string, ExecutionCommand>();
  const occurrences = new Map<string, number>();
  let turn: string | null | undefined;
  for (const activity of activities) {
    const occurrence = occurrences.get(activity.key) ?? 0;
    occurrences.set(activity.key, occurrence + 1);
    const key = `${activity.key}:${occurrence}`;
    if (!activity.tool || (activity.turnId && turn && activity.turnId !== turn)) pending.clear();
    if (activity.turnId) turn = activity.turnId;
    const previous = activity.callId && activity.phase !== "call" ? pending.get(activity.callId) : undefined;
    if (previous) {
      if (activity.status || activity.phase === "result") previous.status = executionStatus(activity.status);
      for (const section of activity.sections) {
        if (!previous.sections.some(existing => existing.label === section.label && existing.text === section.text)) previous.sections.push(section);
      }
      continue;
    }
    if (!activity.command) {
      entries.push({ kind: "item", item: activity.item, key });
      continue;
    }
    const command: ExecutionCommand = {
      id: key,
      title: activity.title,
      status: activity.status ? executionStatus(activity.status) : activity.phase === "call" ? "running" : "completed",
      sections: [...activity.sections]
    };
    if (activity.callId) pending.set(activity.callId, command);
    entries.push({ kind: "command", command });
  }
  const output: ExecutionRenderItem<T>[] = [];
  for (const entry of entries) {
    if (entry.kind === "item") {
      output.push(entry);
      continue;
    }
    const previous = output[output.length - 1];
    const group = previous?.kind === "group" ? previous.group : undefined;
    if (group) group.commands.push(entry.command);
    else output.push({ kind: "group", group: { id: `group:${entry.command.id}`, commands: [entry.command], running: false, failedCount: 0 } });
  }
  for (const entry of output) {
    if (entry.kind !== "group") continue;
    entry.group.running = entry.group.commands.some(command => command.status === "running");
    entry.group.failedCount = entry.group.commands.filter(command => command.status === "failed").length;
  }
  return output;
}

export function groupCodexCommandBlocks(blocks: MessageBlock[]): ExecutionRenderItem<MessageBlock>[] {
  return groupActivities(blocks.map(block => {
    const result = /output|result/i.test(block.kind);
    return {
      item: block,
      key: `codex:${block.turn_id ?? ""}:${block.call_id ?? block.item_id ?? block.id}`,
      callId: block.call_id,
      turnId: block.turn_id,
      tool: isToolBlock(block),
      command: isToolBlock(block) && (isCommandText(block.tool_name) || isCommandText(block.kind)),
      title: commandTitle(block.tool_name, block.kind),
      phase: result ? "result" : "call",
      status: block.status,
      sections: [{ label: result ? "结果" : "命令详情", text: toolBlockDetailText(block) }]
    };
  }));
}

export function groupGrokCommandEvents(events: GrokHistoryEvent[]): ExecutionRenderItem<GrokHistoryEvent>[] {
  return groupActivities(events.map(event => ({
    item: event,
    key: `grok:${event.callId ?? event.timestamp ?? fingerprint(`${event.kind}\0${event.text ?? ""}`)}`,
    callId: event.callId,
    tool: event.kind.startsWith("tool_"),
    command: event.kind.startsWith("tool_") && (isCommandText(event.method) || isCommandText(event.text)),
    title: commandTitle(event.method, event.text),
    phase: event.kind === "tool_call" ? "call" : event.kind === "tool_result" ? "result" : "update",
    status: event.status,
    sections: event.detail ? [{ label: "命令详情", text: event.detail }] : []
  })));
}

export function groupPiCommandEvents(events: PiHistoryEvent[]): ExecutionRenderItem<PiHistoryEvent>[] {
  return groupActivities(events.map(event => ({
    item: event,
    key: `pi:${event.callId ?? `${event.timestamp ?? ""}:${fingerprint(`${event.kind}\0${event.role ?? ""}\0${event.text ?? ""}`)}`}`,
    callId: event.callId,
    tool: event.kind === "tool_call" || event.kind === "tool_result",
    command: (event.kind === "tool_call" || event.kind === "tool_result") && (isCommandText(event.role) || isCommandText(event.text)),
    title: commandTitle(event.role, event.text),
    phase: event.kind === "tool_call" ? "call" : "result",
    status: event.status,
    sections: event.detail ? [{ label: event.kind === "tool_call" ? "调用参数" : "结果", text: event.detail }] : []
  })));
}
