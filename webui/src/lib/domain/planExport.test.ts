import { expect, test } from "vitest";
import { planFilename } from "./planExport";

test("plan download names preserve the visible title and cannot escape the download directory", () => {
  expect(planFilename("# 修复 Plan: API/CLI?\n\nBody", "Fallback")).toBe("修复 Plan API CLI.md");
  expect(planFilename("## [Guide](https://example.com) **v2**\nBody", "Fallback")).toBe("Guide v2.md");
  expect(planFilename("Body without a heading", "Fallback")).toBe("Body without a heading.md");
  expect(planFilename("", "Fallback/Title")).toBe("Fallback Title.md");
  expect(planFilename("# CON", "Fallback")).toBe("_CON.md");
  expect(planFilename("# Plan.md", "Fallback")).toBe("Plan.md");
  expect(new TextEncoder().encode(planFilename(`# ${"计划".repeat(120)}`, "Fallback")).length).toBeLessThanOrEqual(203);
});
