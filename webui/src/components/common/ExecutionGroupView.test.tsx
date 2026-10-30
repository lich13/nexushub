import { renderToStaticMarkup } from "react-dom/server";
import { expect, test } from "vitest";
import { groupClaudeProcesses } from "../../lib/domain/claudeProcessGroups";
import { groupCodexCommandBlocks, groupGrokCommandEvents } from "../../lib/domain/executionGroups";
import type { ExecutionGroup } from "../../lib/domain/executionGroups";
import { DisclosureScope, rememberScopedDisclosure } from "./ActivityDetails";
import { executionSummary, ExecutionGroupView } from "./ExecutionGroupView";

function summaryGroup(titles: string[], overrides: Partial<ExecutionGroup> = {}): ExecutionGroup {
  return {
    id: "summary-fixture", kind: "tool", provider: "Codex", running: false, failedCount: 0,
    commands: titles.map((title, index) => ({
      id: `command-${index}`, sourceIds: [`source-${index}`], title,
      status: "completed", sections: []
    })),
    ...overrides
  };
}

test.each([
  ["read_file", "已读取文件"], ["Read", "已读取文件"], ["functions.list_files", "已读取文件"],
  ["search_query", "已搜索内容"], ["Grep", "已搜索内容"], ["web.search", "已搜索内容"],
  ["exec_command", "已运行命令"], ["Bash", "已运行命令"], ["functions.js", "已运行命令"]
])("tool %s has a compact Chinese activity summary", (title, expected) => {
  expect(executionSummary(summaryGroup([title])).label).toBe(expected);
});

test("mixed activities list each operation once in a stable order", () => {
  const group = summaryGroup(["exec_command", "search", "Read", "read_file", "bash"]);
  expect(executionSummary(group).label).toBe("已读取文件、搜索内容、运行命令");
  expect(executionSummary({ ...group, running: true }).label).toBe("正在读取文件、搜索内容、运行命令");
});

test("Grok tool titles with arguments retain their operation summaries", () => {
  const [entry] = groupGrokCommandEvents([
    { kind: "tool_call", callId: "read", text: "Read fixture.txt", status: "completed" },
    { kind: "tool_call", callId: "list", text: "List fixture-directory", status: "completed" },
    { kind: "tool_call", callId: "search", text: "Search fixture-query", status: "completed" },
    { kind: "tool_call", callId: "execute", text: "Execute fixture-command", status: "completed" }
  ]);
  if (entry.kind !== "group") throw new Error("expected group");
  expect(executionSummary(entry.group).label).toBe("已读取文件、搜索内容、运行命令");
});

test.each(["Codex", "Claude Code", "Grok"] as const)("unknown %s tools retain their integration identity", provider => {
  const group = summaryGroup(["mcp__fixture__inspect"], { provider, running: true });
  expect(executionSummary(group).label).toContain(`正在使用 ${provider} 集成`);
  expect(executionSummary(group).label).toContain("调用工具");
  expect(executionSummary({ ...group, running: false }).label).toContain(`已使用 ${provider} 集成`);
});

test("tool output text cannot change the operation named in the group summary", () => {
  const group = summaryGroup(["read_file"]);
  group.commands[0].sections = [{ label: "结果", text: "bash search mcp__fixture__inspect" }];
  const html = renderToStaticMarkup(<ExecutionGroupView group={group} />);
  expect(html).toContain("已读取文件");
  expect(html).toContain("1 项工具");
  expect(html).not.toContain("已运行命令");
  expect(html).not.toContain("已搜索内容");
});

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

test("Codex and Grok detect instruction command input without changing grouping", () => {
  const [codex] = groupCodexCommandBlocks([{ id: "shell", kind: "function_call", role: "tool", tool_name: "exec_command", input: "cat /workspace/AGENTS.md", status: "running", questions: [] }]);
  const [grok] = groupGrokCommandEvents([{ kind: "tool_call", role: "bash", callId: "shell", detail: '{"command":"cat AGENTS.md"}', status: "in_progress" }]);
  for (const entry of [codex, grok]) {
    if (entry.kind !== "group") throw new Error("expected group");
    expect(entry.group.commands[0].instructionFile).toBe(true);
    expect(renderToStaticMarkup(<ExecutionGroupView group={entry.group} />)).not.toMatch(/<details[^>]*open/);
  }
});

test("an open activity group does not mount the large output of completed commands", () => {
  const group = summaryGroup(["exec_command", "read_file"], { running: true });
  group.commands[0].sections = [{ label: "结果", text: "Completed output payload fixture.\n".repeat(4096) }];
  group.commands[1].status = "running";
  group.commands[1].sections = [{ label: "结果", text: "Visible running output fixture." }];

  const html = renderToStaticMarkup(<ExecutionGroupView group={group} />);
  expect(html.match(/<details[^>]*open/g)).toHaveLength(2);
  expect(html).toContain("已完成");
  expect(html).toContain("Visible running output fixture.");
  expect(html.includes("Completed output payload fixture."), "closed completed commands should not mount output").toBe(false);
});

test("an open failed group keeps instruction output unmounted and ordinary failure output visible", () => {
  const group = summaryGroup(["read_file", "exec_command"], { failedCount: 2 });
  group.commands[0].instructionFile = true;
  group.commands[0].status = "failed";
  group.commands[0].sections = [{ label: "结果", text: "Instruction output payload fixture.\n".repeat(4096) }];
  group.commands[1].status = "failed";
  group.commands[1].sections = [{ label: "结果", text: "Visible failed output fixture." }];

  const html = renderToStaticMarkup(<ExecutionGroupView group={group} />);
  expect(html.match(/<details[^>]*open/g)).toHaveLength(2);
  expect(html).toContain("AGENTS.md");
  expect(html).toContain("Visible failed output fixture.");
  expect(html.includes("Instruction output payload fixture."), "closed instruction commands should not mount output").toBe(false);
});

test("merged Claude execution groups preserve an absorbed group's expanded disclosure state until toggled together", () => {
  const scope = "claude-merge-disclosure-fixture";
  rememberScopedDisclosure(scope, "group:claude:bash-1:0", false);
  rememberScopedDisclosure(scope, "group:claude:bash-2:0", true);

  const items = groupClaudeProcesses([
    { id: "bash-1", turnId: "t1", kind: "tool_call", callId: "b1", role: "Bash", status: "completed", detail: "{\"command\":\"pwd\"}" },
    { id: "ctx-1", turnId: "t1", kind: "system", role: "native_context", detail: "ctx" },
    { id: "bash-2", turnId: "t1", kind: "tool_call", callId: "b2", role: "Bash", status: "completed", detail: "{\"command\":\"ls\"}" },
  ]);
  const proc = items.find(entry => entry.kind === "process");
  if (!proc || proc.kind !== "process") throw new Error("expected process");
  const merged = proc.process.entries.find(entry => entry.kind === "group");
  if (!merged || merged.kind !== "group") throw new Error("expected merged group");

  const openHtml = renderToStaticMarkup(
    <DisclosureScope.Provider value={scope}>
      <ExecutionGroupView group={merged.group} />
    </DisclosureScope.Provider>
  );
  expect(openHtml).toMatch(/<details class="execution-group"[^>]*open/);

  const aliases = [...(merged.group.mergedIds ?? []), ...merged.group.commands.map(cmd => `group:${cmd.id}`)];
  rememberScopedDisclosure(scope, merged.group.id, false, aliases);
  const closedHtml = renderToStaticMarkup(
    <DisclosureScope.Provider value={scope}>
      <ExecutionGroupView group={merged.group} />
    </DisclosureScope.Provider>
  );
  expect(closedHtml).not.toMatch(/<details class="execution-group"[^>]*open/);
});
