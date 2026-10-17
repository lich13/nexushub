import { isValidElement, type ReactElement, type ReactNode } from "react";
import { describe, expect, test, vi } from "vitest";
import type { DesktopGoal as Goal } from "../../lib/api/desktop";
import { DesktopGoal } from "./DesktopGoal";

type Element = ReactElement<Record<string, any>>;

function elements(node: ReactNode): Element[] {
  if (Array.isArray(node)) return node.flatMap(elements);
  if (!isValidElement<Record<string, any>>(node)) return [];
  return [node, ...elements(node.props.children)];
}

function renderedText(node: ReactNode): string {
  if (Array.isArray(node)) return node.map(renderedText).join("");
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (isValidElement<Record<string, any>>(node)) return renderedText(node.props.children);
  return "";
}

function required(tree: ReactNode, predicate: (element: Element) => boolean): Element {
  const element = elements(tree).find(predicate);
  if (!element) throw new Error("Expected desktop goal fixture element is missing");
  return element;
}

const statusLabels: Array<[string, string]> = [
  ["active", "进行中的目标"],
  ["paused", "目标已暂停"],
  ["blocked", "目标受阻"],
  ["usageLimited", "目标受用量限制"],
  ["budgetLimited", "目标受预算限制"],
  ["complete", "目标已完成"]
];

describe("desktop native goal snapshot", () => {
  test.each(statusLabels)("shows the read-only objective and %s state", (status, label) => {
    const goal: Goal = { objective: "整理桌面协作目录 fixture", status, truncated: true };
    const tree = DesktopGoal({ goal, open: vi.fn(async () => undefined), busy: false });
    const details = required(tree, element => element.type === "details");
    const summary = required(details, element => element.type === "summary");

    expect(details.props.open).toBeUndefined();
    expect(renderedText(summary.props.children)).toContain(label);
    expect(renderedText(summary.props.children)).toContain(goal.objective);
    expect(required(details, element => element.type === "p").props.children).toBe(goal.objective);
    expect(elements(details).some(element => element.type === "small" && element.props.children === "完整目标请在 Codex 中查看")).toBe(true);
  });

  test("delegates management to the provided Codex open action and respects busy state", () => {
    const goal: Goal = { objective: "fixture goal", status: "active", truncated: false };
    const open = vi.fn(async () => undefined);
    const busyTree = DesktopGoal({ goal, open, busy: true });
    const busyButton = required(busyTree, element => element.type === "button" && element.props["aria-label"] === "在 Codex 中管理目标");
    expect(busyButton.props.disabled).toBe(true);

    const availableTree = DesktopGoal({ goal, open, busy: false });
    const openButton = required(availableTree, element => element.type === "button" && element.props["aria-label"] === "在 Codex 中管理目标");
    openButton.props.onClick();
    expect(open).toHaveBeenCalledOnce();
  });
});
