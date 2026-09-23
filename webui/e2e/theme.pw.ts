import { expect, test } from "@playwright/test";
import { assertContrast, assertNoOverflow, mockApi } from "./fixtures";

for (const colorScheme of ["light", "dark"] as const) {
  test.describe(colorScheme, () => {
    test.use({ colorScheme, viewport: { width: 1280, height: 820 } });
    test("login text, borders and focus remain readable across desktop and mobile", async ({ page }, info) => {
      await mockApi(page, false);
      await page.goto("/");
      await expect(page.locator(".login-panel")).toBeVisible();
      for (const width of [1440, 1280, 390, 320]) {
        await page.setViewportSize({ width, height: width > 767 ? 900 : 844 });
        await assertContrast(page, ".login-panel h1, .login-panel p, .login-panel input, .login-panel label > span, .login-panel button");
        await assertContrast(page, ".login-panel input", 3, "border-top-color");
        await page.getByRole("textbox", { name: "管理员", exact: true }).focus();
        await assertContrast(page, ".login-panel input:focus-visible", 3, "outline-color");
        await assertNoOverflow(page);
        await page.screenshot({ path: info.outputPath(`login-${width}.png`) });
      }
    });
    test("conversation colors resolve and jobs remain readable", async ({ page }) => {
      await mockApi(page);
      await page.goto("/");
      await page.locator(".thread-item").first().click();
      await expect(page.locator(".conversation-title")).toBeVisible();
      expect(await page.locator(".conversation-shell").evaluate((el) => getComputedStyle(el).getPropertyValue("--text").trim())).not.toBe("");
      await assertContrast(page, ".conversation-title, .chat-meta span, .tool-title");
      await page.locator(".side-nav").getByRole("button", { name: "设置", exact: true }).click();
      await page.locator(".execution-history > summary").click();
      await expect(page.locator(".job-item").first()).toBeVisible();
      await assertContrast(page, ".job-item summary, .tone-success, .tone-warning, .tone-danger");
      await assertNoOverflow(page);
    });
  });
}
