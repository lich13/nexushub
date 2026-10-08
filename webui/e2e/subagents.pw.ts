import { expect, test, type Page } from "@playwright/test";
import { mockApi, mockCommand, observeCommands } from "./fixtures";
import type { MessageBlock, SubagentActivity, SubagentDetailRequest, SubagentDetailResponse, SystemCapabilitiesResponse, ThreadDetail, ThreadSummary } from "../src/types";

const root: ThreadSummary = { id: "root-fixture", title: "父线程示例", status: "Recent", message_count: 3, cwd: "/fixture" };
const child: SubagentActivity = { agentId: "child-fixture", name: "示例子智能体", status: "running", available: true, delegation: "检查示例文件并报告结果。" };
const nested: SubagentActivity = { agentId: "nested-fixture", name: "嵌套示例", status: "completed", available: true };
const png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aN1cAAAAASUVORK5CYII=";

function message(id: string, text: string, role = "assistant"): MessageBlock {
  return { id, role, kind: "message", text, questions: [] };
}
function activity(agent = child, id = "child-activity"): MessageBlock {
  return { id, role: "tool", kind: "function_call", tool_name: "spawn_agent", questions: [], subagent: agent };
}
function command(id: string): MessageBlock {
  return { id, role: "tool", kind: "function_call_output", tool_name: "exec_command", input: `printf ${id}`, text: "fixture output", status: "completed", questions: [] };
}
function detail(summary: ThreadSummary, blocks: MessageBlock[]): ThreadDetail {
  return { summary, blocks, messages: [], raw_event_count: blocks.length, total_blocks: blocks.length, has_more_blocks: false, before_cursor: null };
}
function childPage(agent = child, blocks = [message("child-reply", "子智能体示例回复")], rootId = root.id): SubagentDetailResponse {
  return {
    rootThreadId: rootId, parentThreadId: agent.agentId === nested.agentId ? child.agentId! : rootId, agent,
    detail: detail({ ...root, id: agent.agentId!, title: agent.name }, blocks)
  };
}
async function installSubagents(page: Page, blocks: MessageBlock[] = [
  message("root-request", "检查示例工作区", "user"), command("before-child"), activity(), command("after-child"), message("root-reply", "父线程示例回复")
], capability: boolean | null = true) {
  await mockApi(page);
  const requests: SubagentDetailRequest[] = [];
  observeCommands(page, (name, args) => { if (name === "threads.subagentDetail") requests.push({ ...args.request }); });
  await mockCommand(page, "system.capabilities", async (_, next) => {
    const response = await next() as SystemCapabilitiesResponse;
    const capabilities = { ...response.capabilities };
    if (capability === null) delete capabilities.thread_subagents;
    else capabilities.thread_subagents = capability;
    return { ...response, capabilities };
  });
  await mockCommand(page, "threads.list", () => [root]);
  await mockCommand(page, "threads.detail", () => detail(root, blocks));
  await mockCommand(page, "threads.subagentDetail", () => childPage());
  return requests;
}
async function openRoot(page: Page) {
  await page.goto("/");
  await page.locator(".thread-item").filter({ hasText: root.title }).click();
  await expect(page.locator(".conversation-title")).toHaveText(root.title);
}
const parentStream = (page: Page) => page.locator(".conversation-main .message-stream");
const panel = (page: Page) => page.getByRole("dialog", { name: "子智能体详情", exact: true });
const trigger = (page: Page) => parentStream(page).getByRole("button", { name: /示例子智能体/ });

for (const docked of [true, false]) {
  test(`child activities stay independent and keyboard detail closes with focus returned (${docked ? "docked" : "overlay"})`, async ({ page }) => {
    await page.setViewportSize(docked ? { width: 1700, height: 950 } : { width: 390, height: 844 });
    await page.emulateMedia({ reducedMotion: "reduce" });
    const requests = await installSubagents(page);
    await openRoot(page);
    const stream = parentStream(page);
    await expect(stream.locator(".execution-group")).toHaveCount(2);
    await expect(stream.locator(".subagent-activity")).toHaveCount(1);
    await expect(stream.locator(".execution-group .subagent-activity")).toHaveCount(0);
    await expect(stream.locator(".execution-group > summary").first()).toContainText("已运行命令");
    const order = await stream.locator(":scope > article, :scope > details, :scope > .subagent-activity").evaluateAll(nodes => nodes.map(node =>
      node.matches(".subagent-activity") ? "child" : node.matches("details") ? "tools" : node.matches(".user-message") ? "user" : "assistant"));
    expect(order).toEqual(["user", "tools", "child", "tools", "assistant"]);
    await trigger(page).focus();
    await page.keyboard.press("Enter");
    const view = panel(page);
    await expect(view).toBeVisible();
    await expect(view).toHaveAttribute("aria-modal", String(!docked));
    await expect(view.getByRole("button", { name: "关闭子智能体详情", exact: true })).toBeFocused();
    await expect(view).toContainText("子智能体示例回复");
    expect(requests[0]).toEqual({ rootThreadId: root.id, agentId: child.agentId, before: null, limit: 120 });
    if (!docked) {
      const last = view.getByRole("button", { name: "复制回复", exact: true });
      await last.focus();
      await page.keyboard.press("Tab");
      await expect(view.getByRole("button", { name: "关闭子智能体详情", exact: true })).toBeFocused();
      await page.keyboard.press("Shift+Tab");
      await expect(last).toBeFocused();
    }
    await page.keyboard.press("Escape");
    await expect(view).toHaveCount(0);
    await expect(trigger(page)).toBeFocused();
  });
}

test("nested navigation retains the original root and returns to the parent child", async ({ page }) => {
  await page.setViewportSize({ width: 1700, height: 950 });
  const requests = await installSubagents(page);
  await mockCommand(page, "threads.subagentDetail", args => args.request.agentId === nested.agentId
    ? childPage(nested, [message("nested-reply", "嵌套示例回复"), activity(child, "cycle-activity")])
    : childPage(child, [message("child-reply", "子智能体示例回复"), activity(nested, "nested-activity")]));
  await openRoot(page);
  await trigger(page).click();
  const view = panel(page);
  const delegation = view.locator("details.subagent-delegation");
  await expect(delegation).not.toHaveAttribute("open");
  await delegation.locator("summary").click();
  await expect(delegation).toContainText(child.delegation!);
  await view.getByRole("button", { name: /嵌套示例/ }).click();
  await expect(view.locator(".subagent-panel-header strong")).toHaveText(nested.name);
  await expect(view).toContainText("嵌套示例回复");
  expect(requests.some(request => request.agentId === nested.agentId && request.rootThreadId === root.id)).toBe(true);
  await view.getByRole("button", { name: /示例子智能体/ }).click();
  await expect(view.locator(".subagent-panel-header strong")).toHaveText(nested.name);
  await view.getByRole("button", { name: "返回上一级子智能体", exact: true }).click();
  await expect(view.locator(".subagent-panel-header strong")).toHaveText(child.name);
  await expect(view).toContainText("子智能体示例回复");
  await expect(view.getByRole("button", { name: "返回上一级子智能体", exact: true })).toHaveCount(0);
  await expect(delegation).toHaveAttribute("open", "");
  expect(requests.every(request => request.rootThreadId === root.id)).toBe(true);
});

test("a failed detail read announces loading and supports explicit retry", async ({ page }) => {
  const requests = await installSubagents(page);
  let release!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  let reads = 0;
  await mockCommand(page, "threads.subagentDetail", async () => {
    if (++reads === 1) { await gate; throw new Error("示例记录暂时不可读"); }
    return childPage();
  });
  await openRoot(page);
  await trigger(page).click();
  const view = panel(page);
  await expect(view.getByRole("status")).toContainText("正在读取子智能体");
  release();
  await expect(view.getByRole("alert")).toContainText("示例记录暂时不可读");
  await view.getByRole("button", { name: "重试", exact: true }).click();
  await expect(view).toContainText("子智能体示例回复");
  await expect(view.getByRole("alert")).toHaveCount(0);
  expect(requests.slice(0, 2)).toEqual([
    { rootThreadId: root.id, agentId: child.agentId, before: null, limit: 120 },
    { rootThreadId: root.id, agentId: child.agentId, before: null, limit: 120 }
  ]);
});

for (const capability of [false, null]) {
  test(`unsupported detail remains readable without dispatching a child request (${capability === null ? "missing" : "false"})`, async ({ page }) => {
    const requests = await installSubagents(page, [activity()], capability);
    await openRoot(page);
    await expect(trigger(page)).toHaveAttribute("aria-disabled", "true");
    await expect(parentStream(page)).toContainText("请更新当前机器服务以查看子智能体");
    await trigger(page).focus();
    await page.keyboard.press("Enter");
    await expect(panel(page)).toHaveCount(0);
    expect(requests).toEqual([]);
  });
}

test("an unavailable child shows its native reason and cannot be opened", async ({ page }) => {
  const requests = await installSubagents(page, [activity({ ...child, status: "unknown", available: false, unavailableReason: "示例子智能体记录已移除" })]);
  await openRoot(page);
  await expect(trigger(page)).toContainText("状态未知");
  await expect(trigger(page)).toHaveAttribute("aria-disabled", "true");
  await expect(parentStream(page)).toContainText("示例子智能体记录已移除");
  await trigger(page).focus();
  await page.keyboard.press("Space");
  await expect(panel(page)).toHaveCount(0);
  expect(requests).toEqual([]);
});

test("older child pages retain the visible anchor and expanded command", async ({ page }) => {
  await page.setViewportSize({ width: 1700, height: 950 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  const requests = await installSubagents(page);
  const history = Array.from({ length: 20 }, (_, index) => message(`history-${index}`, `较早示例 ${index}\n\n${"示例历史正文。".repeat(18)}`));
  const current = [command("retained-command"), message("anchor", "当前页锚点"), ...Array.from({ length: 20 }, (_, index) => message(`current-${index}`, `当前示例 ${index}\n\n${"示例当前正文。".repeat(18)}`))];
  let release!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  await mockCommand(page, "threads.subagentDetail", async args => {
    const older = Boolean(args.request.before);
    if (older) await gate;
    const response = childPage(child, older ? history : current);
    response.detail = { ...response.detail, total_blocks: history.length + current.length, has_more_blocks: !older, before_cursor: older ? null : "block:20" };
    return response;
  });
  await openRoot(page);
  await trigger(page).click();
  const view = panel(page);
  const stream = view.locator(".subagent-stream");
  await expect(stream.locator("article")).toHaveCount(21);
  await stream.evaluate(node => { node.scrollTop = 0; node.dispatchEvent(new Event("scroll", { bubbles: true })); });
  const group = stream.locator("details.execution-group");
  await group.locator(":scope > summary").click();
  const retained = group.locator("details.execution-command");
  await retained.locator("summary").click();
  await expect(retained).toHaveAttribute("open", "");
  await stream.evaluate(node => { node.scrollTop = 0; node.dispatchEvent(new Event("scroll", { bubbles: true })); });
  const anchor = view.getByText("当前页锚点", { exact: true });
  const offset = await anchor.evaluate(node => node.getBoundingClientRect().top);
  const older = view.getByRole("button", { name: "较早消息", exact: true });
  await older.click();
  await expect(older).toBeDisabled();
  release();
  await expect(stream.locator("article")).toHaveCount(41);
  await expect(older).toHaveCount(0);
  await expect(retained).toHaveAttribute("open", "");
  await expect(group).toHaveAttribute("open", "");
  await expect.poll(async () => Math.abs(await anchor.evaluate(node => node.getBoundingClientRect().top) - offset)).toBeLessThanOrEqual(2);
  expect(requests).toContainEqual({ rootThreadId: root.id, agentId: child.agentId, before: "block:20", limit: 120 });
});

test("nested attachment reads carry the original root and Escape first closes the image", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await installSubagents(page);
  const attachmentRequests: unknown[] = [];
  await mockCommand(page, "sessions.attachmentRead", args => {
    attachmentRequests.push(args.request);
    return { mimeType: "image/png", base64: png };
  });
  await mockCommand(page, "threads.subagentDetail", args => args.request.agentId === nested.agentId
    ? childPage(nested, [{
      ...message("image-message", "示例附件", "user"),
      user_message: { id: "image-message", text: "示例附件", attachments: [{ id: "image-fixture", name: "sample.png", kind: "image" }] }
    }]) : childPage(child, [activity(nested, "nested-activity")]));
  await openRoot(page);
  await trigger(page).click();
  const view = panel(page);
  await view.getByRole("button", { name: /嵌套示例/ }).click();
  const image = view.locator(".attachment-thumbnail img");
  await expect.poll(() => image.evaluate(node => (node as HTMLImageElement).naturalWidth)).toBe(1);
  const thumbnail = view.getByRole("button", { name: "预览图片：sample.png", exact: true });
  await thumbnail.click();
  const preview = page.getByRole("dialog", { name: "图片预览：sample.png", exact: true });
  await expect(preview).toBeVisible();
  expect(attachmentRequests).toEqual([{ provider: "codex", rootThreadId: root.id, sessionKey: nested.agentId, messageId: "image-message", attachmentId: "image-fixture" }]);
  await page.keyboard.press("Escape");
  await expect(preview).not.toBeVisible();
  await expect(view).toBeVisible();
  await expect(thumbnail).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(view).toHaveCount(0);
  await expect(trigger(page)).toBeFocused();
});

test("opening and closing a docked child preserves the parent reading anchor", async ({ page }) => {
  await page.setViewportSize({ width: 1700, height: 950 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  const before = Array.from({ length: 14 }, (_, index) => message(`before-${index}`, `父线程较早示例 ${index}\n\n${"用于观察宽度变化后的正文。".repeat(25)}`));
  const after = Array.from({ length: 14 }, (_, index) => message(`after-${index}`, `父线程后续示例 ${index}\n\n${"用于观察宽度变化后的正文。".repeat(25)}`));
  await installSubagents(page, [...before, activity(), ...after]);
  await openRoot(page);
  const stream = parentStream(page);
  await trigger(page).scrollIntoViewIfNeeded();
  await stream.evaluate(node => { node.dispatchEvent(new Event("scroll", { bubbles: true })); });
  const anchor = await stream.evaluate(node => {
    const top = node.getBoundingClientRect().top;
    const item = Array.from(node.querySelectorAll<HTMLElement>("[data-timeline-id]")).find(element => element.getBoundingClientRect().bottom > top)!;
    return { id: item.dataset.timelineId!, offset: item.getBoundingClientRect().top - top };
  });
  const offset = () => stream.locator(`[data-timeline-id="${anchor.id}"]`).evaluate(node => node.getBoundingClientRect().top - node.closest(".message-stream")!.getBoundingClientRect().top);
  await trigger(page).click();
  await expect(panel(page)).toHaveClass(/docked/);
  await expect.poll(async () => Math.abs(await offset() - anchor.offset)).toBeLessThanOrEqual(2);
  await panel(page).getByRole("button", { name: "关闭子智能体详情", exact: true }).click();
  await expect(panel(page)).toHaveCount(0);
  await expect.poll(async () => Math.abs(await offset() - anchor.offset)).toBeLessThanOrEqual(2);
  await expect(trigger(page)).toBeFocused();
});

test("switching the parent thread discards an in-flight child response", async ({ page }) => {
  await page.setViewportSize({ width: 1700, height: 950 });
  const requests = await installSubagents(page);
  const otherRoot = { ...root, id: "other-root-fixture", title: "另一父线程示例" };
  await mockCommand(page, "threads.list", () => [root, otherRoot]);
  await mockCommand(page, "threads.detail", args => detail(args.id === root.id ? root : otherRoot, [activity()]));
  let release!: () => void;
  let finished = false;
  const gate = new Promise<void>(resolve => { release = resolve; });
  await mockCommand(page, "threads.subagentDetail", async args => {
    if (args.request.rootThreadId === root.id) {
      await gate;
      finished = true;
      return childPage(child, [message("late", "旧父线程迟到内容")]);
    }
    return childPage(child, [message("fresh", "新父线程子智能体内容")], otherRoot.id);
  });
  await openRoot(page);
  await trigger(page).click();
  await expect(panel(page).getByRole("status")).toContainText("正在读取子智能体");
  await page.locator(".thread-item").filter({ hasText: otherRoot.title }).click();
  await expect(page.locator(".conversation-title")).toHaveText(otherRoot.title);
  await expect(panel(page)).toHaveCount(0);
  release();
  await expect.poll(() => finished).toBe(true);
  await trigger(page).click();
  await expect(panel(page)).toContainText("新父线程子智能体内容");
  await expect(panel(page)).not.toContainText("旧父线程迟到内容");
  expect(requests).toContainEqual({ rootThreadId: otherRoot.id, agentId: child.agentId, before: null, limit: 120 });
});
