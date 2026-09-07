import { expect, test } from "@playwright/test";
import { mockApi } from "./fixtures";

test("selected Grok session shows a new message within three seconds", async ({ page }) => {
  await mockApi(page);
  let text = "Initial native message";
  await page.route("**/grok.detail", route => route.fulfill({ json: {
    summary: { id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", status: "running" },
    events: [{ kind: "agent_message_chunk", text }]
  } }));
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Grok Build", exact: true }).click();
  await expect(page.getByText(text, { exact: true })).toBeVisible();
  const appendedAt = Date.now();
  text = "Fresh native message after opening the conversation";
  await expect(page.getByText(text, { exact: true })).toBeVisible({ timeout: 3000 });
  expect(Date.now() - appendedAt).toBeLessThan(3000);
});

test("Grok follows new messages at the bottom and preserves history reading position", async ({ page }) => {
  await mockApi(page);
  const events = Array.from({ length: 30 }, (_, index) => ({ kind: "agent_message_chunk", text: `Historical message ${index}\n\n${"Readable history. ".repeat(15)}` }));
  await page.route("**/grok.detail", route => route.fulfill({ json: {
    summary: { id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", status: "running" }, events
  } }));
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Grok Build", exact: true }).click();
  const stream = page.locator(".provider-events");
  await expect(stream.locator("article")).toHaveCount(30);
  const bottomGap = () => stream.evaluate(element => element.scrollHeight - element.clientHeight - element.scrollTop);
  await expect.poll(bottomGap).toBeLessThan(3);
  events.push({ kind: "agent_message_chunk", text: "New message at the bottom" });
  await expect(stream.locator("article")).toHaveCount(31, { timeout: 3000 });
  await expect.poll(bottomGap).toBeLessThan(3);
  await stream.evaluate(element => { element.scrollTop = 100; element.dispatchEvent(new Event("scroll", { bubbles: true })); });
  events.push({ kind: "agent_message_chunk", text: "New message while reviewing history" });
  await expect(stream.locator("article")).toHaveCount(32, { timeout: 3000 });
  await expect.poll(() => stream.evaluate(element => element.scrollTop)).toBe(100);
});

test("narrow Grok list stops hidden detail reads and reopening reads fresh messages", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const calls = await mockApi(page);
  await page.goto("/");
  await page.locator(".mobile-tabs").getByRole("button", { name: "Grok Build", exact: true }).click();
  await expect(page.locator(".provider-session")).toBeVisible();
  await page.waitForTimeout(2200);
  expect(calls.filter(call => call === "grok.detail")).toHaveLength(0);
  await page.locator(".provider-session").click();
  await expect(page.getByText("Check the isolated fixture.", { exact: true })).toBeVisible();
  await page.getByTitle("返回任务列表").click();
  const before = calls.filter(call => call === "grok.detail").length;
  await page.waitForTimeout(2200);
  expect(calls.filter(call => call === "grok.detail")).toHaveLength(before);
  await page.locator(".provider-session").click();
  await expect.poll(() => calls.filter(call => call === "grok.detail").length).toBeGreaterThan(before);
});
