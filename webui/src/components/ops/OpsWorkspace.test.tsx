import { beforeEach, describe, expect, test, vi } from "vitest";
import { isValidElement, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { HiddenThreadDeletePlan, HiddenThreadDeleteResult } from "../../types";
import type { RuntimeCapabilityMatrix } from "../../lib/query/system";
import { CleanupPagination, OpsWorkspace } from "./OpsWorkspace";

const harness = vi.hoisted(() => ({
  index: 0,
  cells: new Map<number, unknown>(),
  opsInput: null as any,
  opsActions: null as any
}));

vi.mock("./DshArchiveCleanup", () => ({ DshArchiveCleanup: () => null }));

vi.mock("react", async importOriginal => {
  const actual = await importOriginal<typeof import("react")>();
  return {
    ...actual,
    useState: (initial: unknown) => {
      const index = harness.index++;
      if (!harness.cells.has(index)) {
        harness.cells.set(index, typeof initial === "function" ? (initial as () => unknown)() : initial);
      }
      return [harness.cells.get(index), (value: unknown) => {
        const current = harness.cells.get(index);
        harness.cells.set(index, typeof value === "function"
          ? (value as (previous: unknown) => unknown)(current)
          : value);
      }];
    }
  };
});

vi.mock("../../lib/query/ops", () => ({
  useOpsQueries: () => ({
    update: { data: null, isPending: false, error: null },
    jobs: { data: [], error: null }
  }),
  useOpsActions: (input: unknown) => {
    harness.opsInput = input;
    return harness.opsActions;
  }
}));

vi.mock("../../lib/query/jobs", async importOriginal => {
  const actual = await importOriginal<typeof import("../../lib/query/jobs")>();
  return { ...actual, useStartedJob: () => ({ data: undefined, error: null }) };
});

const capabilities: RuntimeCapabilityMatrix = {
  runtimeKind: "desktop",
  hostSurface: "desktop_embedded_tauri",
  codexStatePaths: false,
  hiddenCleanupScan: true,
  updatePrune: false,
  threadCleanup: true,
  threadArchiveActions: true,
  updateServiceLabels: false
};

function mutation() {
  return {
    data: undefined,
    error: null,
    isPending: false,
    isSuccess: false,
    variables: undefined,
    mutate: vi.fn(),
    reset: vi.fn()
  };
}

function hiddenPlan(count: number): HiddenThreadDeletePlan {
  const candidates = Array.from({ length: count }, (_, index) => {
    const id = "candidate-" + String(index).padStart(3, "0");
    return { id, title: id, fingerprint: "fixture-fingerprint", allowed: true, reason: null };
  });
  return {
    total_threads: count + 1,
    visible_threads: 1,
    hidden_threads: count,
    archived_threads: 0,
    session_index_lines: count + 1,
    rollout_files: count + 1,
    hidden_ids: candidates.map(item => item.id),
    candidates,
    hidden_source_counts: { subagent: count },
    integrity: "ok"
  };
}

function failedResult(plan: HiddenThreadDeletePlan): HiddenThreadDeleteResult {
  const items = plan.candidates.map(item => ({ id: item.id, status: "failed" as const, reason: "fixture failure" }));
  return {
    skipped_threads: 0,
    failed_threads: items.length,
    items,
    before: plan,
    deleted_threads: 0,
    after_total_threads: plan.total_threads,
    after_visible_threads: plan.visible_threads,
    after_hidden_threads: plan.hidden_threads,
    after_archived_threads: plan.archived_threads,
    after_integrity: "ok",
    visible_threads: plan.visible_threads,
    hidden_threads: plan.hidden_threads,
    integrity: "ok",
    deleted_rollout_files: 0
  };
}

type Element = ReactElement<Record<string, any>>;

function elements(node: ReactNode): Element[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!isValidElement<Record<string, any>>(node)) return [];
  return [node, ...elements(node.props.children)];
}

function renderTree(): ReactElement {
  harness.index = 0;
  return OpsWorkspace({ capabilities }) as unknown as ReactElement;
}

function markup(tree: ReactElement): string {
  return renderToStaticMarkup(tree);
}

function pagination(tree: ReactElement, label: string): Element {
  const found = elements(tree).find(element => element.type === CleanupPagination && element.props.label === label);
  if (!found) throw new Error("Expected " + label + " pagination");
  return found;
}

function page(paginationElement: Element, index: number) {
  (paginationElement.props.onChange as (page: number) => void)(index);
}

function createActions() {
  return {
    updateJob: mutation(),
    archiveDryRun: mutation(),
    archiveExecute: mutation(),
    hiddenDryRun: mutation(),
    hiddenExecute: mutation(),
    stopHiddenScan: vi.fn()
  };
}

beforeEach(() => {
  harness.index = 0;
  harness.cells.clear();
  harness.opsInput = null;
  harness.opsActions = createActions();
});

describe("OpsWorkspace cleanup pagination", () => {
  test("renders no more than 100 hidden candidates on each page", () => {
    const plan = hiddenPlan(205);
    renderTree();
    harness.opsInput.onHiddenDryRun(plan);

    let tree = renderTree();
    let html = markup(tree);
    expect((html.match(/<li\b/g) ?? [])).toHaveLength(100);
    expect(html).toContain("candidate-000");
    expect(html).toContain("candidate-099");
    expect(html).not.toContain("candidate-100");

    page(pagination(tree, "候选"), 1);
    tree = renderTree();
    html = markup(tree);
    expect((html.match(/<li\b/g) ?? [])).toHaveLength(100);
    expect(html).toContain("candidate-100");
    expect(html).toContain("candidate-199");
    expect(html).not.toContain("candidate-200");

    page(pagination(tree, "候选"), 2);
    html = markup(renderTree());
    expect((html.match(/<li\b/g) ?? [])).toHaveLength(5);
    expect(html).toContain("candidate-204");
  });

  test("renders no more than 100 unsuccessful delete results on each page", () => {
    const plan = hiddenPlan(205);
    renderTree();
    harness.opsInput.onHiddenDryRun(plan);
    renderTree();
    harness.opsInput.onHiddenExecute(failedResult(plan));

    let tree = renderTree();
    let html = markup(tree);
    expect((html.match(/<p>candidate-/g) ?? [])).toHaveLength(100);
    expect(html).toContain("candidate-099：fixture failure");
    expect(html).not.toContain("candidate-100：fixture failure");

    page(pagination(tree, "清理结果"), 1);
    tree = renderTree();
    html = markup(tree);
    expect((html.match(/<p>candidate-/g) ?? [])).toHaveLength(100);
    expect(html).toContain("candidate-100：fixture failure");
    expect(html).toContain("candidate-199：fixture failure");
    expect(html).not.toContain("candidate-200：fixture failure");

    page(pagination(tree, "清理结果"), 2);
    html = markup(renderTree());
    expect((html.match(/<p>candidate-/g) ?? [])).toHaveLength(5);
    expect(html).toContain("candidate-204：fixture failure");
  });
});
