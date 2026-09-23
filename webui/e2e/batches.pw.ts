import { expect, test } from "@playwright/test";
import { assertNoOverflow, mockApi } from "./fixtures";

test("batch archive confirms explicit keys, retains failed selections, and requires another preview", async ({ page }) => {
  await mockApi(page);
  let previews = 0;
  const executed: any[] = [];
  await page.route("**/sessions.bulkPreview", route => {
    previews++;
    const request = route.request().postDataJSON().request;
    return route.fulfill({ json: { ...request, items: request.sessionKeys.map((key: string) => ({ sessionKey: key, id: key, title: "Selected fixture", paths: [], bytes: 0, allowed: true, reason: null, fingerprint: `${previews}:${key}` })) } });
  });
  await page.route("**/sessions.bulkExecute", route => {
    const request = route.request().postDataJSON().request;
    executed.push(request);
    return route.fulfill({ json: { items: request.items.map((item: any, index: number) => ({ sessionKey: item.sessionKey, status: index === 0 ? "succeeded" : "blocked", message: index === 0 ? null : "Fixture changed; preview again" })) } });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "多选线程", exact: true }).click();
  const boxes = page.locator(".session-checkbox");
  await boxes.nth(0).check();
  await boxes.nth(1).check();
  await expect(page.getByText("已选 2 / 100", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "归档所选线程", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  expect(executed).toHaveLength(0);
  await dialog.getByRole("button", { name: "取消", exact: true }).click();
  await expect(page.getByRole("button", { name: "归档所选线程", exact: true })).toBeFocused();
  await page.getByRole("button", { name: "归档所选线程", exact: true }).click();
  await dialog.getByRole("button", { name: "确认归档 2 项", exact: true }).click();
  await expect(page.getByText("已完成 1 项，未完成 1 项", { exact: true })).toBeVisible();
  await expect(page.getByText("已选 1 / 100", { exact: true })).toBeVisible();
  expect(executed).toHaveLength(1);
  expect(executed[0].provider).toBe("codex");
  expect(executed[0].items).toHaveLength(2);
  expect(executed[0].items.every((item: any) => typeof item.sessionKey === "string" && !item.path)).toBe(true);
  await page.getByRole("button", { name: "归档所选线程", exact: true }).click();
  await expect(dialog.getByRole("button", { name: "确认归档 1 项", exact: true })).toBeEnabled();
  expect(previews).toBe(3);
});

test("filter changes invalidate an in-flight batch preview and never select newly loaded rows", async ({ page }) => {
  await mockApi(page);
  let release!: () => void;
  let started = false;
  const gate = new Promise<void>(resolve => { release = resolve; });
  await page.route("**/sessions.bulkPreview", async route => {
    started = true;
    await gate;
    const request = route.request().postDataJSON().request;
    await route.fulfill({ json: { ...request, items: request.sessionKeys.map((key: string) => ({ sessionKey: key, id: key, title: "Old preview", paths: [], bytes: 0, allowed: true, fingerprint: key })) } });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "多选线程", exact: true }).click();
  await page.locator(".session-checkbox").first().check();
  await page.getByRole("button", { name: "归档所选线程", exact: true }).click();
  await expect.poll(() => started).toBe(true);
  await page.locator(".search-box input").first().fill("no matching fixture");
  release();
  await expect(page.getByRole("button", { name: "多选线程", exact: true })).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await page.locator(".search-box input").first().fill("");
  await page.getByRole("button", { name: "多选线程", exact: true }).click();
  await expect(page.getByText("已选 0 / 100", { exact: true })).toBeVisible();
});

for (const provider of ["grok", "pi"] as const) {
  test(`${provider} batch deletion uses distinct keys and excludes protected files on mobile`, async ({ page }) => {
    await mockApi(page);
    await page.setViewportSize({ width: 390, height: 844 });
    const keys = provider === "pi" ? ["project/a.jsonl", "project/b.jsonl"] : ["grok-one", "grok-two"];
    await page.route(`**/${provider}.list`, route => route.fulfill({ json: keys.map((key, index) => ({ id: provider === "pi" ? "duplicate-native-id" : key, sessionKey: key, title: `Fixture ${index}`, cwd: "/isolated/workspace", path: `/isolated/sessions/${key}`, status: "recent", messageCount: 1, canRename: true, canDelete: index === 0 })) }));
    await page.route("**/sessions.bulkPreview", route => route.fulfill({ json: { provider, operation: "delete", items: keys.map((key, index) => ({ sessionKey: key, id: provider === "pi" ? "duplicate-native-id" : key, title: `Fixture ${index}`, paths: [`/isolated/sessions/${key}`], bytes: 64, allowed: index === 0, reason: index === 0 ? null : "活动状态不明", fingerprint: index === 0 ? "fixture" : null })) } }));
    let executed: any;
    await page.route("**/sessions.bulkExecute", route => {
      executed = route.request().postDataJSON().request;
      return route.fulfill({ json: { items: [{ sessionKey: keys[0], status: "succeeded" }] } });
    });
    await page.goto("/");
    await page.locator(".mobile-tabs").getByRole("button", { name: provider === "pi" ? "Pi" : "Grok Build", exact: true }).click();
    await page.getByRole("button", { name: "多选线程", exact: true }).click();
    await page.getByRole("button", { name: "全选当前结果", exact: true }).click();
    await expect(page.getByText("已选 2 / 100", { exact: true })).toBeVisible();
    await page.getByRole("button", { name: "删除所选线程", exact: true }).click();
    await expect(page.getByRole("dialog")).toContainText("活动状态不明");
    await assertNoOverflow(page);
    await page.getByRole("button", { name: "确认删除 1 项", exact: true }).click();
    await expect(page.getByText("已选 1 / 100", { exact: true })).toBeVisible();
    expect(executed.items).toEqual([{ sessionKey: keys[0], fingerprint: "fixture" }]);
  });
}
