import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test } from "vitest";
import { MarkdownContent } from "./MarkdownContent";

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
    expect(html).toContain("Run checks.");
    expect(html).toContain("Ordinary result.");
  });
});
