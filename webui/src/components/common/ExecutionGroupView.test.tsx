import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";
import { groupCodexCommandBlocks, groupGrokCommandEvents, groupPiCommandEvents } from "../../lib/domain/executionGroups";
import { ExecutionGroupView } from "./ExecutionGroupView";

test("all instruction tools stay closed even when running or failed", () => {
  const [entry] = groupGrokCommandEvents([
    { kind: "tool_call", callId: "read", text: "Read /workspace/AGENTS.md", status: "in_progress" },
    { kind: "tool_call", callId: "shell", text: "Execute", detail: "cat AGENTS.md", status: "failed" }
  ]);
  if (entry.kind !== "group") throw new Error("expected group");
  const html = renderToStaticMarkup(<ExecutionGroupView group={entry.group} />);
  expect(html).not.toMatch(/<details[^>]*open/);
  expect(html).toContain("1 条失败");
  expect(html).toContain("thread-running-indicator");
  expect(html).not.toContain("/workspace");
});

test("mixed groups keep ordinary failed commands open and pair instruction results", () => {
  const [entry] = groupGrokCommandEvents([
    { kind: "tool_call", callId: "read", text: "Read", status: "in_progress" },
    { kind: "tool_result", callId: "read", detail: "AGENTS.md\nRules", status: "failed" },
    { kind: "tool_call", callId: "shell", text: "Execute pwd", status: "failed" }
  ]);
  if (entry.kind !== "group") throw new Error("expected group");
  expect(entry.group.commands).toHaveLength(2);
  expect(entry.group.commands[0].instructionFile).toBe(true);
  const html = renderToStaticMarkup(<ExecutionGroupView group={entry.group} />);
  expect(html.match(/<details[^>]*open/g)).toHaveLength(2);
});

test("Codex and Pi detect instruction command input without changing grouping", () => {
  const [codex] = groupCodexCommandBlocks([{ id: "shell", kind: "function_call", role: "tool", tool_name: "exec_command", input: "cat /workspace/AGENTS.md", status: "running", questions: [] }]);
  const [pi] = groupPiCommandEvents([{ kind: "tool_call", role: "bash", callId: "shell", detail: '{"command":"cat AGENTS.md"}', status: "in_progress" }]);
  for (const entry of [codex, pi]) {
    if (entry.kind !== "group") throw new Error("expected group");
    expect(entry.group.commands[0].instructionFile).toBe(true);
    expect(renderToStaticMarkup(<ExecutionGroupView group={entry.group} />)).not.toMatch(/<details[^>]*open/);
  }
});
