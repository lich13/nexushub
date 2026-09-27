import { describe, expect, test } from "vitest";
import { instructionSegments, isInstructionFileActivity, visibleMarkdown } from "./visibleMarkdown";
import { planFilename } from "./planExport";

describe("visible Markdown", () => {
  test.each([
    "<oai-mem-citation><citation_entries>internal</citation_entries><rollout_ids>id</rollout_ids></oai-mem-citation>",
    "<citation_entries>internal\nsecond</citation_entries><rollout_ids>id</rollout_ids>",
    "&lt;oai-mem-citation&gt;internal&lt;/oai-mem-citation&gt;"
  ])("removes inline, multiline and adjacent metadata without losing prose", metadata => {
    expect(visibleMarkdown(`Before ${metadata} after.\n\n${metadata}`)).toBe("Before  after.");
  });

  test("keeps literal code and ordinary memory references", () => {
    const markdown = "Read MEMORY.md for memory guidance.\n`<citation_entries>inline</citation_entries>`\n\n````xml\n```\n<oai-mem-citation>example</oai-mem-citation>\n````\n\n~~~json\n{\"memory_citation\": \"example\"}\n~~~";
    expect(visibleMarkdown(markdown)).toBe(markdown);
    expect(visibleMarkdown("    <rollout_ids>indented example</rollout_ids>")).toContain("indented example");
    expect(visibleMarkdown("    memory_citation: example")).toBe("    memory_citation: example");
  });

  test("removes standalone fields and incomplete streamed metadata", () => {
    expect(visibleMarkdown('Visible.\nmemory_citation: {\n "entries": ["internal"]\n}\nMore.')).toBe("Visible.\n\nMore.");
    expect(visibleMarkdown('{"memory_citation": {"entries": []}}')).toBe("");
    expect(visibleMarkdown('Visible.\n"memory_citation": "internal"')).toBe("Visible.");
    expect(visibleMarkdown('Visible.<oai-mem-citation>\n<citation_entries>unfinished')).toBe("Visible.");
    expect(visibleMarkdown("The memory_citation: field is documented here.")).toBe("The memory_citation: field is documented here.");
  });

  test("removes tagged metadata with attributes and adjacent blocks", () => {
    expect(visibleMarkdown('Before <oai-mem-citation source="internal"><citation_entries id="x">secret</citation_entries></oai-mem-citation><rollout_ids source="internal">id</rollout_ids> after.')).toBe("Before  after.");
  });

  test("uses only the cleaned title for plan filenames", () => {
    expect(planFilename("<citation_entries>\n# Internal\n</citation_entries>\n# Visible: /Plan?\nBody", "Fallback")).toBe("Visible Plan.md");
    expect(planFilename("<rollout_ids>id</rollout_ids>", "Thread title")).toBe("Thread title.md");
  });
});

describe("instruction files", () => {
  test("recognizes explicit file references in names, input and result", () => {
    for (const value of ["AGENTS.md", "Read /workspace/AGENTS.md", '{"path":"/workspace/AGENTS.md"}', "Output:\n./AGENTS.md\nGuidance"])
      expect(isInstructionFileActivity(value)).toBe(true);
    expect(isInstructionFileActivity("myAGENTS.md", "AGENTS.md.backup")).toBe(false);
  });

  test("folds only the file section through the same or higher heading", () => {
    const text = "Intro\n\n## AGENTS.md\n规则\n### Checks\nRun tests.\n## Result\nDone.";
    const sections = instructionSegments(text);
    expect(sections.map(s => s.kind)).toEqual(["markdown", "instructions", "markdown"]);
    expect(sections[1].text).toBe("## AGENTS.md\n规则\n### Checks\nRun tests.\n");
    expect(sections[1].lines).toBe(4);
    expect(sections[1].bytes).toBe(new TextEncoder().encode(sections[1].text).length);
    expect(instructionSegments(text + "\nMore result.")[1].id).toBe(sections[1].id);
  });

  test("folds standalone paths and native instruction envelopes", () => {
    expect(instructionSegments("`/workspace/AGENTS.md`\n# Rules\nGuidance")[0].kind).toBe("instructions");
    const sections = instructionSegments("# AGENTS.md instructions for /workspace\n\n<INSTRUCTIONS>\n# Rules\nGuidance\n</INSTRUCTIONS>\n\nUser request.");
    expect(sections.map(s => s.kind)).toEqual(["instructions", "markdown"]);
    expect(sections[1].text.trim()).toBe("User request.");
  });

  test("does not fold prose, code examples or a similarly named file", () => {
    for (const text of ["Please read /workspace/AGENTS.md", "Mention AGENTS.md in prose.", "```md\n# AGENTS.md\nExample\n```", "# /workspace/myAGENTS.md\nExample"])
      expect(instructionSegments(text).map(s => s.kind)).toEqual(["markdown"]);
  });
});
