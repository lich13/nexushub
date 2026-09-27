import { expect, test } from "@playwright/test";
import { mockApi } from "./fixtures";
import * as demo from "../src/lib/api/demo";

const png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aN1cAAAAASUVORK5CYII=";
const text = "# User heading\n  Keep spaces\n- plain list\n```text\n<image>example</image>\n```";

for (const provider of ["codex", "grok", "pi"] as const) {
  for (const mobile of [false, true]) {
    test(`${provider} literal user bubble and image preview ${mobile ? "mobile" : "desktop"}`, async ({ page }) => {
      await page.setViewportSize(mobile ? { width: 390, height: 844 } : { width: 1440, height: 980 });
      await page.emulateMedia({ colorScheme: mobile ? "dark" : "light", reducedMotion: "reduce" });
      await mockApi(page);
      let reads = 0;
      await page.route("**/sessions.attachmentRead", route => {
        const request = route.request().postDataJSON().request;
        expect(request.provider).toBe(provider);
        expect(request.messageId).toBe("user-1");
        expect(request.attachmentId).toBe("attachment-1");
        expect(request).not.toHaveProperty("path");
        reads++;
        return route.fulfill({ json: { mimeType: "image/png", base64: png } });
      });
      const user = { id: "user-1", text, attachments: [{ id: "attachment-1", kind: "image", name: "sample.png" }] };
      await page.route(`**/${provider === "codex" ? "threads" : provider}.detail`, route => provider === "codex"
        ? route.fulfill({ json: { ...demo.demoThreadDetail("019e95a0-demo"), blocks: [{ id: "user-1", role: "user", kind: "message", questions: [], text: "attachment envelope", user_message: user }] } })
        : route.fulfill({ json: { summary: {}, events: [{ kind: provider === "grok" ? "user_message_chunk" : "user_message", userMessage: user, text: "attachment envelope" }] } }));
      await page.goto("/");
      if (provider !== "codex") await page.locator(mobile ? ".mobile-tabs" : ".side-nav").getByRole("button", { name: provider === "grok" ? "Grok Build" : "Pi", exact: true }).click();
      if (provider === "codex") await page.locator(".thread-item").filter({ hasText: "Plan Mode 修复" }).click();
      else if (mobile) await page.locator(".provider-session").click();
      const bubble = page.locator(".user-message-bubble");
      await expect(bubble).toHaveText(text);
      expect(await bubble.textContent()).toBe(text);
      await expect(bubble.locator("h1, h2, pre, ul")).toHaveCount(0);
      await expect(page.getByText("attachment envelope", { exact: true })).toHaveCount(0);
      const image = page.locator(".attachment-thumbnail img");
      await expect(image).toBeVisible();
      await expect.poll(() => image.evaluate(el => (el as HTMLImageElement).naturalWidth)).toBe(1);
      const imageBox = await image.boundingBox(); const bubbleBox = await bubble.boundingBox();
      expect(imageBox!.y + imageBox!.height).toBeLessThan(bubbleBox!.y);
      const button = page.getByRole("button", { name: "预览图片：sample.png" });
      await button.focus(); await page.keyboard.press("Enter");
      await expect(page.getByRole("dialog")).toBeVisible();
      await page.keyboard.press("Escape");
      await expect(page.getByRole("dialog")).not.toBeVisible();
      await expect(button).toBeFocused();
      expect(reads).toBe(1);
      await expect.poll(() => page.evaluate(() => document.documentElement.scrollHeight - innerHeight)).toBeLessThanOrEqual(1);
    });
  }
}

test("missing image and long user messages keep the document within the viewport", async ({ page }) => {
  await mockApi(page);
  const user = { id: "missing", text: "Long request\n".repeat(400), attachments: [{ id: "missing-file", kind: "image", name: "expired.png", reason: "原附件已不存在或无法读取" }] };
  await page.route("**/grok.detail", route => route.fulfill({ json: { summary: {}, events: [{ kind: "user_message_chunk", userMessage: user }] } }));
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Grok Build", exact: true }).click();
  const stream = page.locator(".provider-events");
  await expect(page.locator(".user-message-bubble")).toContainText("Long request");
  await stream.evaluate(el => { el.scrollTop = 0; });
  await expect(page.getByText("图片不可用", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "预览图片：expired.png" })).toBeDisabled();
  expect(await page.evaluate(() => document.documentElement.scrollHeight - innerHeight)).toBeLessThanOrEqual(1);
});

test("a failed attachment read reports a local error and can be retried", async ({ page }) => {
  await mockApi(page);
  let reads = 0;
  await page.route("**/sessions.attachmentRead", route => ++reads === 1
    ? route.fulfill({ status: 409, json: { error: "附件文件已变化" } })
    : route.fulfill({ json: { mimeType: "image/png", base64: png } }));
  await page.route("**/grok.detail", route => route.fulfill({ json: { summary: {}, events: [{ kind: "user_message_chunk", userMessage: { id: "retry", text: "Request", attachments: [{ id: "image", kind: "image", name: "retry.png" }] } }] } }));
  await page.goto("/");
  await page.locator(".side-nav").getByRole("button", { name: "Grok Build", exact: true }).click();
  await expect(page.getByText("附件文件已变化", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "重试预览" }).click();
  await expect(page.locator(".attachment-thumbnail img")).toBeVisible();
  expect(reads).toBe(2);
});
