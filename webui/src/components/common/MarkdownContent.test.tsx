import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";
import { MarkdownContent } from "./MarkdownContent";
import { MarkdownPathScope } from "./FilePathLink";

describe("user-visible Markdown", () => {
  test("does not display trailing Codex memory citation metadata", () => {
    const html = renderToStaticMarkup(<MarkdownContent text={'Finished.\n\n<oai-mem-citation>\n<citation_entries>\nMEMORY.md:1-2|note=[internal]\n</citation_entries>\n<rollout_ids>\nprivate-id\n</rollout_ids>\n</oai-mem-citation>'} />);
    expect(html).toContain("Finished.");
    expect(html).not.toContain("private-id");
    expect(html).not.toContain("MEMORY.md:1-2");
  });

  test("keeps literal metadata examples inside code", () => {
    const html = renderToStaticMarkup(<MarkdownContent text={'Example:\n```xml\n<oai-mem-citation>literal</oai-mem-citation>\n```'} />);
    expect(html).toContain("literal");
    expect(html).toContain("&lt;oai-mem-citation&gt;");
  });

  test("removes inline metadata and folds instructions with a path-free summary", () => {
    const html = renderToStaticMarkup(<MarkdownContent foldInstructions text={'Done.<oai-mem-citation>internal-entry</oai-mem-citation>\n\n## /workspace/AGENTS.md\nRun checks.\n## Result\nOrdinary result.'} />);
    expect(html).not.toContain("internal-entry");
    expect(html).toMatch(/<details[^>]*instruction-file[^>]*>/);
    expect(html).not.toMatch(/<details[^>]*open/);
    const summary = html.match(/<summary>(.*?)<\/summary>/)?.[1];
    expect(summary).toContain("AGENTS.md");
    expect(summary).toContain("字节");
    expect(summary).not.toContain("/workspace");
    expect(html).not.toContain("Run checks.");
    expect(html).toContain("Ordinary result.");
  });

  test("does not mount a long instruction body while its disclosure is closed", () => {
    const body = "Instruction payload fixture with **formatting** and `code`.\n\n".repeat(2048);
    const html = renderToStaticMarkup(<MarkdownContent activityId="long-instructions" foldInstructions text={`## AGENTS.md\n${body}\n## Result\nVisible result fixture.`} />);
    expect(html).toMatch(/<details[^>]*instruction-file[^>]*>/);
    expect(html).not.toMatch(/<details[^>]*open/);
    expect(html).toContain("AGENTS.md");
    expect(html).toContain("字节");
    expect(html.includes("instruction-file-body"), "closed instructions should not mount their body").toBe(false);
    expect(html.includes("Instruction payload fixture"), "closed instructions should not render Markdown").toBe(false);
    expect(html).toContain("Visible result fixture.");
  });

  test("renders instruction-like Markdown normally when folding is disabled", () => {
    const html = renderToStaticMarkup(<MarkdownContent text={"## AGENTS.md\nOrdinary instruction example."} />);
    expect(html).not.toContain("<details");
    expect(html).toContain("Ordinary instruction example.");
  });

  test("reuses GFM parsing without leaking path scope or a transformed tree across server renders", () => {
    const text = [
      "[Guide](./guide.md)",
      "",
      "| Format | Value |",
      "| --- | --- |",
      "| GFM | retained |",
      "",
      "- [x] Completed task",
      "",
      "A note[^footnote].",
      "",
      "[^footnote]: Footnote body.",
      "",
      "[Unsafe](javascript:alert%281%29)",
      "",
      "```ts",
      "const fixture = 1;",
      "```"
    ].join("\n");
    const renderAt = (cwd: string) => renderToStaticMarkup(
      <MarkdownPathScope.Provider value={cwd}><MarkdownContent text={text} /></MarkdownPathScope.Provider>
    );

    const one = renderAt("/fixture/one");
    const two = renderAt("/fixture/two");
    const oneAgain = renderAt("/fixture/one");
    const withoutPath = (html: string) => html.replace(/\/fixture\/(?:one|two)\/guide\.md/g, "/fixture/<cwd>/guide.md");

    expect(one).toContain("：/fixture/one/guide.md");
    expect(one).not.toContain("/fixture/two/guide.md");
    expect(two).toContain("：/fixture/two/guide.md");
    expect(two).not.toContain("/fixture/one/guide.md");
    expect(oneAgain).toContain("：/fixture/one/guide.md");
    for (const html of [one, two, oneAgain]) {
      expect(html).toContain("<table>");
      expect(html).toContain("GFM");
      expect(html).toContain("type=\"checkbox\"");
      expect(html).toContain("checked");
      expect(html).toContain("Footnote body.");
      expect(html).toContain("class=\"markdown-code\"");
      expect(html).toContain("const fixture = 1;");
      expect(html).not.toMatch(/href=[\"']javascript:/i);
    }
    expect(withoutPath(two)).toBe(withoutPath(one));
    expect(withoutPath(oneAgain)).toBe(withoutPath(one));
  });

  test("renders updated text under the same path scope without retaining old content", () => {
    const render = (text: string) => renderToStaticMarkup(
      <MarkdownPathScope.Provider value="/fixture/one"><MarkdownContent text={text} /></MarkdownPathScope.Provider>
    );
    const before = render("Previous body.\n\n[Guide](./old.md)");
    const after = render("Updated body.\n\n[Guide](./new.md)");

    expect(after).toContain("Updated body.");
    expect(after).not.toContain("Previous body.");
    expect(after).toContain("/fixture/one/new.md");
    expect(after).not.toContain("/fixture/one/old.md");
    expect(before).toContain("/fixture/one/old.md");
  });
});
