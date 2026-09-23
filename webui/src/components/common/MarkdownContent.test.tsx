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
});
