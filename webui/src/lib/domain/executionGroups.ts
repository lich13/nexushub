import type { MessageBlock, GrokHistoryEvent, PiHistoryEvent } from "../../types";

export type ExecutionGroup<T> = {
  id: string;
  items: T[];
  running: boolean;
  failed: boolean;
};

export type ExecutionRenderItem<T> =
  | { kind: "item"; item: T }
  | { kind: "group"; group: ExecutionGroup<T> };

const RUNNING = new Set(["pending", "running", "in_progress", "inprogress", "active", "generating"]);
const FAILED = new Set(["failed", "error", "cancelled", "canceled", "rejected"]);

export function executionStatus(value?: string | null): "running" | "failed" | "done" {
  const status = value?.trim().toLowerCase() ?? "";
  if (RUNNING.has(status)) return "running";
  if (FAILED.has(status)) return "failed";
  return "done";
}

export function isCommandText(value?: string | null): boolean {
  const text = value?.trim().toLowerCase() ?? "";
  return /(?:^|[._:\s-])(exec_command|run_terminal_command|bash_execution|bashexecution|shell|terminal|bash|execute)(?:$|[._:\s-])/.test(text)
    || text === "exec_command";
}

export function groupAdjacentCommands<T>(
  items: T[],
  isCommand: (item: T) => boolean,
  status: (item: T) => string | null | undefined,
  identity: (item: T, index: number) => string | null | undefined
): ExecutionRenderItem<T>[] {
  const output: ExecutionRenderItem<T>[] = [];
  let group: T[] = [];
  let groupKey = "";
  const flush = () => {
    if (!group.length) return;
    output.push({
      kind: "group",
      group: {
        id: groupKey || `execution-${output.length}`,
        items: group,
        running: group.some(item => executionStatus(status(item)) === "running"),
        failed: group.some(item => executionStatus(status(item)) === "failed")
      }
    });
    group = [];
    groupKey = "";
  };
  items.forEach((item, index) => {
    if (!isCommand(item)) {
      flush();
      output.push({ kind: "item", item });
      return;
    }
    const key = identity(item, index);
    if (group.length && key && groupKey && key !== groupKey && group.some(existing => identity(existing, index) === key)) {
      group.push(item);
      return;
    }
    if (!group.length) groupKey = key || `execution-${index}`;
    group.push(item);
  });
  flush();
  return output;
}

export function groupCodexCommandBlocks(blocks: MessageBlock[]): ExecutionRenderItem<MessageBlock>[] {
  return groupAdjacentCommands(
    blocks,
    block => isCommandText(block.tool_name) || block.kind.toLowerCase() === "commandexecution",
    block => block.status,
    block => block.call_id ?? block.id
  );
}

export function groupGrokCommandEvents(events: GrokHistoryEvent[]): ExecutionRenderItem<GrokHistoryEvent>[] {
  return groupAdjacentCommands(
    events,
    event => event.kind.startsWith("tool_") && isCommandText(event.method ?? event.text ?? event.detail),
    event => event.status,
    event => event.callId
  );
}

export function groupPiCommandEvents(events: PiHistoryEvent[]): ExecutionRenderItem<PiHistoryEvent>[] {
  return groupAdjacentCommands(
    events,
    event => (event.kind === "tool_call" || event.kind === "tool_result")
      && isCommandText(event.role ?? event.text ?? event.detail),
    event => event.status,
    event => event.callId
  );
}
