import { beforeEach, expect, test, vi } from "vitest";
import { isValidElement, type ReactElement, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { DshArchiveCleanup } from "./DshArchiveCleanup";
import { runtimeCapabilitiesFromResponse } from "../../lib/domain/capabilities";
import type { DshArchiveDeletePlan } from "../../types";

const harness = vi.hoisted(() => ({ index: 0, cells: new Map<number, unknown>(), input: null as any, actions: null as any }));
vi.mock("react", async original => ({
  ...await original<typeof import("react")>(),
  useState: (initial: unknown) => {
    const i = harness.index++;
    if (!harness.cells.has(i)) harness.cells.set(i, initial);
    return [harness.cells.get(i), (value: unknown) => harness.cells.set(i, value)];
  }
}));
vi.mock("../../lib/query/ops", () => ({ useDshArchiveActions: (input: unknown) => { harness.input = input; return harness.actions; } }));

function tree(supported = true) { harness.index = 0; return DshArchiveCleanup({ supported }); }
function elements(node: ReactNode): ReactElement<any>[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!isValidElement<{ children?: ReactNode }>(node)) return [];
  return [node, ...elements(node.props.children)];
}
function click(node: ReactElement, label: string) {
  const button = elements(node).find(e => e.type === "button" && renderToStaticMarkup(e).includes(label));
  if (!button || button.props.disabled) throw new Error("button unavailable");
  button.props.onClick();
}
const plan: DshArchiveDeletePlan = { available: true, archivedThreads: 1, candidates: [
  { id: "root", archiveId: "root", parentId: null, reason: null },
  { id: "child", archiveId: "root", parentId: "root", reason: null }
], issues: [] };
beforeEach(() => {
  harness.cells.clear();
  harness.actions = Object.fromEntries(["preview", "execute"].map(key => [key, { isPending: false, error: null, mutate: vi.fn(), reset: vi.fn() }]));
});

test("requires one confirmation and submits only the preview identities", () => {
  tree(); harness.input.onPreview(plan);
  click(tree(), "清理归档");
  expect(harness.actions.execute.mutate).not.toHaveBeenCalled();
  expect(renderToStaticMarkup(tree())).toContain("请先退出当前机器的 DSH");
  click(tree(), "确认清理归档");
  expect(harness.actions.execute.mutate).toHaveBeenCalledWith({ confirmed: true, targets: [
    { id: "root", archiveId: "root" }, { id: "child", archiveId: "root" }
  ] });
  harness.input.onExecute({ deletedThreads: 1, deletedBytes: 10, items: [
    { id: "root", status: "deleted" }, { id: "child", status: "failed", reason: "fixture failure" }
  ] });
  const html = renderToStaticMarkup(tree());
  expect(html).toContain("fixture failure");
  expect(html).not.toContain("确认清理归档");
});

test("fresh machine mount discards the preview and old services are unsupported", () => {
  tree(); harness.input.onPreview(plan); click(tree(), "清理归档");
  harness.cells.clear();
  expect(renderToStaticMarkup(tree())).not.toContain("确认清理归档");
  expect(renderToStaticMarkup(tree(false))).toContain("当前机器不支持 DSH 归档清理");
  expect(runtimeCapabilitiesFromResponse({ capabilities: {} as any }).dshArchiveCleanup).toBe(false);
});

test("missing DSH data displays an empty state", () => {
  tree(); harness.input.onPreview({ available: false, archivedThreads: 0, candidates: [], issues: [] });
  expect(renderToStaticMarkup(tree())).toContain("未发现 DSH 归档数据");
});
