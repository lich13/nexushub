import { describe, expect, test } from "vitest";
import { fileLinkSpans, fileLinkTarget } from "./fileLinks";

describe("file links use source paths", () => {
  test.each([
    ["/workspace/report.md:12:4", undefined, "/workspace/report.md"],
    ["/workspace/report.md#L12-C4", undefined, "/workspace/report.md"],
    ["file:///workspace/%E6%96%87%E4%BB%B6%20A.md#L12-L15", undefined, "/workspace/文件 A.md"],
    ["file://localhost/workspace/result.md", undefined, "/workspace/result.md"],
    ["./docs/../report.md#L4C2", "/workspace/project", "/workspace/project/report.md"],
    ["README.md", "/workspace/project", "/workspace/project/README.md"],
    ["../result.md", "/workspace/project", "/workspace/result.md"],
    ["/workspace/100%25.md", undefined, "/workspace/100%.md"],
    ["/workspace/literal%2520.md", undefined, "/workspace/literal%20.md"],
    ["/workspace/literal%23L4", undefined, "/workspace/literal#L4"],
  ])("resolves %s", (href, cwd, expected) => {
    expect(fileLinkTarget(href!, cwd)).toEqual({ path: expected, resolved: true });
  });
  test.each(["https://example.com/a.md", "mailto:a@example.com", "#section", "//example.com/a.md", "javascript:alert(1)", "data:text/html,x", "file://remote/workspace/a.md", "/workspace/a%00.md"])("does not treat %s as a local path", href => {
    expect(fileLinkTarget(href)).toBeNull();
  });
  test("keeps unresolved relative paths without inventing a browser origin", () => {
    expect(fileLinkTarget("docs/notes.md")).toEqual({ path: "docs/notes.md", resolved: false });
  });
  test("enhances tool links but preserves literal inline and fenced examples", () => {
    const text = "[report](/workspace/report.md) ` [literal](/workspace/code.md) `\n\n```md\n[fenced](/workspace/fenced.md)\n```\n\n[reference][file]\n\n[file]: /workspace/reference.md";
    expect(fileLinkSpans(text).map(({ href, label }) => ({ href, label }))).toEqual([
      { href: "/workspace/report.md", label: "report" },
      { href: "/workspace/reference.md", label: "reference" }
    ]);
  });
});
