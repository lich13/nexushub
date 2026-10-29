import { visibleMarkdown } from "./visibleMarkdown";
import type { ExecutionGroup } from "./executionGroups";

export type ProcessStep =
  | {
      kind: "thinking";
      id: string;
      sourceIds: string[];
      text: string;
      running?: boolean;
      turnId?: string | null;
    }
  | {
      kind: "execution";
      group: ExecutionGroup;
      turnId?: string | null;
    }
  | {
      kind: "detail";
      id: string;
      sourceIds: string[];
      title: string;
      detail: string;
      running?: boolean;
      failed?: boolean;
      turnId?: string | null;
    };

export type UnifiedProcessGroup = {
  id: string;
  stateKey: string;
  stateAliases: string[];
  timelineAliases: string[];
  sourceIds: string[];
  steps: ProcessStep[];
  thoughtCount: number;
  toolCount: number;
  failedCount: number;
  running: boolean;
};

export type ProcessFoldItem<T> =
  | { kind: "passthrough"; item: T }
  | { kind: "execution"; group: ExecutionGroup }
  | { kind: "process"; process: UnifiedProcessGroup };

function uniqueStrings(values: readonly string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const value of values) {
    if (value && !seen.has(value)) {
      seen.add(value);
      out.push(value);
    }
  }
  return out;
}

export function buildUnifiedProcessGroup(prefix: string, steps: ProcessStep[]): UnifiedProcessGroup {
  let thoughtCount = 0;
  let toolCount = 0;
  let failedCount = 0;
  let running = false;
  const rawSourceIds: string[] = [];
  const rawTimelineAliases: string[] = [];
  const rawStateAliases: string[] = [];

  for (const step of steps) {
    if (step.kind === "thinking") {
      thoughtCount += 1;
      if (step.running) running = true;
      rawSourceIds.push(step.id, ...step.sourceIds);
      rawTimelineAliases.push(step.id, ...step.sourceIds);
      rawStateAliases.push(step.id);
    } else if (step.kind === "execution") {
      toolCount += step.group.commands.length;
      failedCount += step.group.failedCount;
      if (step.group.running) running = true;
      rawStateAliases.push(step.group.id, ...step.group.commands.map(cmd => `group:${cmd.id}`));
      rawTimelineAliases.push(step.group.id, ...step.group.commands.map(cmd => `group:${cmd.id}`));
      for (const command of step.group.commands) {
        rawSourceIds.push(command.id, ...command.sourceIds);
        rawTimelineAliases.push(command.id, ...command.sourceIds);
      }
    } else {
      toolCount += 1;
      if (step.failed) failedCount += 1;
      if (step.running) running = true;
      rawSourceIds.push(step.id, ...step.sourceIds);
      rawTimelineAliases.push(step.id, ...step.sourceIds);
      rawStateAliases.push(step.id);
    }
  }

  const firstStep = steps[0];
  const anchorId = firstStep.kind === "execution" ? firstStep.group.id : firstStep.id;
  const id = `${prefix}:process:${anchorId}`;
  return {
    id,
    stateKey: id,
    stateAliases: uniqueStrings(rawStateAliases),
    timelineAliases: uniqueStrings(rawTimelineAliases),
    sourceIds: uniqueStrings(rawSourceIds),
    steps,
    thoughtCount,
    toolCount,
    failedCount,
    running,
  };
}

/**
 * Folds consecutive intermediate process steps (thoughts, tool groups, tool detail rows)
 * between user prompts and visible assistant prose into a single ChatGPT-App-style disclosure.
 *
 * Preserves standalone single `ExecutionGroup` items unwrapped so existing single-group
 * DOM & performance benchmarks keep exact `details.execution-group` node counts.
 */
export function foldProcessSequence<T>(
  prefix: string,
  items: readonly T[],
  classify: (item: T) => ProcessStep | "skip" | null,
): ProcessFoldItem<T>[] {
  const output: ProcessFoldItem<T>[] = [];
  let pending: ProcessStep[] = [];
  let activeTurnId: string | null | undefined;

  const flush = () => {
    if (!pending.length) return;
    if (pending.length === 1 && pending[0].kind === "execution") {
      output.push({ kind: "execution", group: pending[0].group });
    } else {
      output.push({ kind: "process", process: buildUnifiedProcessGroup(prefix, pending) });
    }
    pending = [];
    activeTurnId = undefined;
  };

  for (const item of items) {
    const step = classify(item);
    if (step === "skip") {
      continue;
    }
    if (step) {
      const stepTurnId = step.kind === "execution" ? (step.turnId ?? step.group.turnId) : step.turnId;
      if (stepTurnId && activeTurnId && stepTurnId !== activeTurnId) {
        flush();
      }
      if (stepTurnId) activeTurnId = stepTurnId;
      pending.push(step);
    } else {
      flush();
      output.push({ kind: "passthrough", item });
    }
  }
  flush();
  return output;
}

export function thinkingStepFromText(
  id: string,
  sourceIds: readonly string[],
  rawText: string | null | undefined,
  running = false,
  turnId?: string | null,
): ProcessStep | "skip" {
  const cleaned = visibleMarkdown(rawText ?? "").trim();
  if (!cleaned) return "skip";
  return {
    kind: "thinking",
    id,
    sourceIds: uniqueStrings([id, ...sourceIds]),
    text: rawText ?? "",
    running,
    turnId,
  };
}

export function processGroupSummaryText(group: UnifiedProcessGroup): {
  primary: string;
  meta: string | null;
} {
  if (group.running) {
    if (group.thoughtCount > 0 && group.toolCount > 0) {
      return { primary: "正在思考与执行", meta: `${group.toolCount} 项操作` };
    }
    if (group.thoughtCount > 0) {
      return { primary: "正在思考…", meta: group.thoughtCount > 1 ? `${group.thoughtCount} 段推理` : null };
    }
    return { primary: "正在执行", meta: `${group.toolCount} 项操作` };
  }
  if (group.thoughtCount > 0 && group.toolCount > 0) {
    return {
      primary: "已思考",
      meta: `${group.toolCount} 项操作`,
    };
  }
  if (group.thoughtCount > 0) {
    return {
      primary: "已思考",
      meta: group.thoughtCount > 1 ? `${group.thoughtCount} 段推理` : null,
    };
  }
  return {
    primary: "已执行操作",
    meta: `${group.toolCount} 项操作`,
  };
}
