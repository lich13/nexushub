import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { mockApi } from "./fixtures";
import * as demo from "../src/lib/api/demo";

test("Codex Plan copies Markdown and downloads an md named after its title", async ({ page }) => {
  await mockApi(page);
  const markdown = "# Release: Notes/Review?\n\n- **Preserve** formatting\n- [Guide](https://example.com/guide)";
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", { value: { writeText: async (value: string) => { document.documentElement.dataset.copied = value; } } });
  });
  await page.route("**/threads.detail", route => {
    const detail = demo.demoThreadDetail("019e95a0-demo");
    detail.blocks = [{ id: "plan", role: "assistant", kind: "plan", text: `<proposed_plan>\n${markdown}\n</proposed_plan>`, questions: [] }];
    return route.fulfill({ json: detail });
  });
  await page.goto("/");
  await page.locator(".thread-item").filter({ hasText: "Plan Mode 修复" }).click();
  const plan = page.locator(".plan-cell");
  await expect(plan).toContainText("Preserve");
  await plan.getByRole("button", { name: "复制计划" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-copied", markdown);
  await expect(plan.getByRole("button", { name: "已复制计划" })).toBeVisible();
  const downloadPromise = page.waitForEvent("download");
  await plan.getByRole("button", { name: "下载计划 Markdown" }).click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toBe("Release Notes Review.md");
  expect(await readFile(await download.path(), "utf8")).toBe(markdown);
});

test("Grok Plan exposes the same actions and reports clipboard failure nearby", async ({ page }) => {
  await mockApi(page);
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", { value: { writeText: async () => { throw new Error("denied"); } } });
  });
  const markdown = "# Grok Plan\n\nRun **one** check.";
  await page.route("**/grok.detail", route => route.fulfill({ json: {
    summary: { id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", status: "recent" },
    events: [{ kind: "plan", text: markdown }, { kind: "agent_message_chunk", text: "Done" }]
  } }));
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Grok Build", exact: true }).click();
  const plan = page.locator(".provider-event.plan");
  await plan.getByRole("button", { name: "复制计划" }).click();
  await expect(plan.getByRole("status")).toContainText("复制失败");
  const downloadPromise = page.waitForEvent("download");
  await plan.getByRole("button", { name: "下载计划 Markdown" }).click();
  const download = await downloadPromise;
  expect(download.suggestedFilename()).toBe("Grok Plan.md");
  expect(await readFile(await download.path(), "utf8")).toBe(markdown);
});

test("an empty Grok Plan has no copy or download action", async ({ page }) => {
  await mockApi(page);
  await page.route("**/grok.detail", route => route.fulfill({ json: {
    summary: { id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", status: "recent" },
    events: [{ kind: "plan", text: "" }]
  } }));
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Grok Build", exact: true }).click();
  await expect(page.locator(".provider-event.plan button")).toHaveCount(0);
});
