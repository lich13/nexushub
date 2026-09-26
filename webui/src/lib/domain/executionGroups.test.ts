import { describe, expect, test } from "vitest";
import { groupCodexCommandBlocks, groupGrokCommandEvents, groupPiCommandEvents } from "./executionGroups";

describe("execution groups", () => {
  test("groups adjacent Codex command blocks and leaves other tools alone", () => {
    const result = groupCodexCommandBlocks([
      { id: "a", role: "tool", kind: "function_call", tool_name: "exec_command", status: "completed", questions: [] },
      { id: "b", role: "tool", kind: "function_call_output", tool_name: "exec_command", status: "failed", questions: [] },
      { id: "c", role: "tool", kind: "function_call", tool_name: "read_file", status: "completed", questions: [] }
    ]);
    expect(result).toHaveLength(2);
    expect(result[0].kind).toBe("group");
    if (result[0].kind === "group") {
      expect(result[0].group.commands).toHaveLength(2);
      expect(result[0].group.failedCount).toBe(1);
    }
    expect(result[1].kind).toBe("item");
  });

  test("keeps active Grok call and result in one open group", () => {
    const result = groupGrokCommandEvents([
      { kind: "tool_call", callId: "c1", method: "exec_command", status: "in_progress", text: "pwd" },
      { kind: "tool_result", callId: "c1", method: "exec_command", status: "in_progress", detail: "running" }
    ]);
    expect(result).toHaveLength(1);
    expect(result[0].kind).toBe("group");
    if (result[0].kind === "group") expect(result[0].group.running).toBe(true);
  });

  test("recognizes Grok's native Execute title after tool updates", () => {
    const result = groupGrokCommandEvents([
      { kind: "tool_call", callId: "c1", method: "session/update", status: "completed", text: "Execute `pwd`" },
      { kind: "tool_call", callId: "c2", method: "session/update", status: "completed", text: "Execute `ls`" }
    ]);
    expect(result).toHaveLength(1);
    expect(result[0].kind).toBe("group");
    if (result[0].kind === "group") expect(result[0].group.commands).toHaveLength(2);
  });

  test("folds mixed native Grok tools and keeps paired results inside one row", () => {
    const result = groupGrokCommandEvents([
      { kind: "tool_call", callId: "read-1", method: "session/update", text: "Read `guide.md`", status: "in_progress", detail: "guide.md" },
      { kind: "tool_result", callId: "read-1", method: "session/update", text: "Read `guide.md`", status: "completed", detail: "contents" },
      { kind: "tool_call", callId: "list-1", method: "session/update", text: "List `docs`", status: "completed" },
      { kind: "tool_call", callId: "search-1", method: "session/update", text: "Search examples", status: "failed" },
      { kind: "tool_call", callId: "exec-1", method: "session/update", text: "Execute `pwd`", status: "completed" },
      { kind: "plan", text: "# Next steps" },
      { kind: "tool_call", callId: "read-2", method: "session/update", text: "Read `next.md`", status: "in_progress" }
    ]);
    expect(result).toHaveLength(3);
    if (result[0].kind !== "group" || result[2].kind !== "group") throw new Error("expected tool groups");
    expect(result[0].group.kind).toBe("tool");
    expect(result[0].group.commands.map(command => command.title)).toEqual(["Read `guide.md`", "List `docs`", "Search examples", "execute"]);
    expect(result[0].group.commands[0].sections.map(section => section.text)).toEqual(["guide.md", "contents"]);
    expect(result[0].group.failedCount).toBe(1);
    expect(result[0].group.running).toBe(false);
    expect(result[2].group.running).toBe(true);
    expect(result[1].kind).toBe("item");
  });

  test("uses stable Grok group identities when another tool is appended", () => {
    const events = [
      { kind: "tool_call", callId: "a", text: "Read `a`", status: "completed" },
      { kind: "tool_call", callId: "b", text: "List `b`", status: "completed" }
    ];
    const before = groupGrokCommandEvents(events);
    const after = groupGrokCommandEvents([...events, { kind: "tool_call", callId: "c", text: "Search c", status: "completed" }]);
    if (before[0].kind !== "group" || after[0].kind !== "group") throw new Error("expected tool groups");
    expect(after[0].group.id).toBe(before[0].group.id);
    expect(after[0].group.commands.slice(0, 2).map(command => command.id)).toEqual(before[0].group.commands.map(command => command.id));
  });

  test("recognizes Pi bashExecution without a call id", () => {
    const result = groupPiCommandEvents([
      { kind: "tool_result", role: "bashExecution", status: "completed", detail: "echo ok" },
      { kind: "assistant_message", text: "done" }
    ]);
    expect(result[0].kind).toBe("group");
    expect(result[1].kind).toBe("item");
  });

  test("recognizes Pi tool calls when the message role is assistant", () => {
    const result = groupPiCommandEvents([
      { kind: "tool_call", role: "assistant", text: "bash", callId: "c1", status: "completed" },
      { kind: "tool_result", role: "toolResult", text: "bash", callId: "c1", status: "completed", detail: "ok" }
    ]);
    expect(result).toHaveLength(1);
    expect(result[0].kind).toBe("group");
  });

  test("combines different native command names into one outer group", () => {
    const result = groupGrokCommandEvents([
      { kind: "tool_call", method: "sleep", text: "sleep", callId: "sleep-1", status: "completed" },
      { kind: "tool_call", method: "exec", text: "exec", callId: "exec-1", status: "completed" },
      { kind: "tool_call", method: "js", text: "js", callId: "js-1", status: "completed" }
    ]);
    expect(result).toHaveLength(1);
    if (result[0].kind === "group") {
      expect(result[0].group.commands.map(command => command.title)).toEqual(["sleep", "exec", "js"]);
      expect(result[0].group.commands).toHaveLength(3);
    }
  });

  test("pairs a call and result as one command row", () => {
    const result = groupPiCommandEvents([
      { kind: "tool_call", role: "assistant", text: "bash", callId: "c1", status: "in_progress", detail: "pwd" },
      { kind: "tool_result", role: "toolResult", text: "bash", callId: "c1", status: "completed", detail: "/workspace" }
    ]);
    expect(result).toHaveLength(1);
    if (result[0].kind === "group") {
      expect(result[0].group.commands).toHaveLength(1);
      expect(result[0].group.commands[0].status).toBe("completed");
      expect(result[0].group.commands[0].sections.map(section => section.text)).toEqual(["pwd", "/workspace"]);
    }
  });
});
