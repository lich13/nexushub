import { expect, test, type Page } from "@playwright/test";
import { assertContrast, assertNoOverflow, mockApi, mockCommand } from "./fixtures";
import type { ClaudeSessionSummary, GrokSessionSummary, MessageBlock, SessionStorageSize, ThreadDetail, ThreadSummary } from "../src/types";

const sizeFixtures: Array<{ key: string; label: string; tone: "success" | "warning" | "danger" | "muted"; size?: SessionStorageSize }> = [
  { key: "zero", label: "0K", tone: "success", size: { bytes: 0, scope: "file", status: "complete" } },
  { key: "kilobytes", label: "2K", tone: "success", size: { bytes: 2048, scope: "file", status: "complete" } },
  { key: "megabytes", label: "1M", tone: "warning", size: { bytes: 1024 ** 2, scope: "directory", status: "complete" } },
  { key: "gigabytes", label: "1G", tone: "danger", size: { bytes: 1024 ** 3, scope: "directory", status: "complete" } },
  { key: "unknown", label: "—", tone: "muted", size: { bytes: null, scope: "file", status: "unavailable" } },
  { key: "partial", label: "≥1M", tone: "warning", size: { bytes: 1024 ** 2, scope: "directory", status: "partial" } }
];

const codexRows: ThreadSummary[] = sizeFixtures.map((fixture, index) => ({
  id: `size-codex-${index}`,
  title: `Codex ${fixture.key}`,
  status: "Recent",
  message_count: 1,
  cwd: "/isolated/codex-workspace",
  storageSize: fixture.size
}));
const claudeRows: ClaudeSessionSummary[] = sizeFixtures.map((fixture, index) => ({
  id: `size-claude-${index}`,
  sessionKey: `claude:size-${index}`,
  title: `Claude ${fixture.key}`,
  cwd: "/isolated/claude-workspace",
  path: `/isolated/claude-sessions/size-${index}.jsonl`,
  status: "recent",
  formatVersion: "2.1.284",
  messageCount: 1,
  canRename: true,
  canDelete: true,
  storageSize: fixture.size
}));
const grokRows: GrokSessionSummary[] = sizeFixtures.map((fixture, index) => ({
  id: `size-grok-${index}`,
  title: `Grok ${fixture.key}`,
  cwd: "/isolated/grok-workspace",
  path: `/isolated/grok-sessions/size-${index}`,
  messageCount: 1,
  status: "recent",
  storageSize: fixture.size
}));

function message(id: string, text: string): MessageBlock {
  return { id, role: "assistant", kind: "message", text, questions: [] };
}

function codexDetail(summary: ThreadSummary): ThreadDetail {
  return { summary, blocks: [message(`${summary.id}-message`, "隔离尺寸 fixture")], messages: [], raw_event_count: 1, total_blocks: 1, has_more_blocks: false, before_cursor: null };
}

async function openProvider(page: Page, provider: "codex" | "claude" | "grok") {
  await page.goto("/");
  if (provider === "claude" || provider === "grok") {
    await page.locator(".mobile-tabs").getByRole("button", { name: provider === "claude" ? "Claude Code" : "Grok Build", exact: true }).click();
  }
}

async function assertCards(page: Page, provider: "codex" | "claude" | "grok") {
  const selector = provider === "codex" ? ".thread-item" : ".provider-session";
  const prefix = provider === "codex" ? "Codex" : provider === "claude" ? "Claude" : "Grok";
  const rows = page.locator(selector);
  await expect(rows).toHaveCount(sizeFixtures.length);
  const colors = new Map<string, string>();
  for (const fixture of sizeFixtures) {
    const row = rows.filter({ hasText: `${prefix} ${fixture.key}` }).first();
    const size = row.locator(".session-size");
    await expect(size).toHaveText(fixture.label);
    await expect(size).toHaveClass(new RegExp(`\\b${fixture.tone}\\b`));
    await expect(size).toHaveAttribute("aria-label", fixture.size?.status === "partial" ? /统计不完整/ : fixture.size?.status === "unavailable" ? "会话大小暂不可用" : /会话/);
    colors.set(fixture.tone, await size.evaluate(element => getComputedStyle(element).color));
  }
  expect(colors.get("success")).toBeTruthy();
  expect(colors.get("warning")).not.toBe(colors.get("success"));
  expect(colors.get("danger")).not.toBe(colors.get("warning"));
  expect(colors.get("muted")).not.toBe(colors.get("success"));
  await assertContrast(page, ".session-size");
  await assertNoOverflow(page);
  const first = rows.first();
  await first.focus();
  await expect(first).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.getByTitle("返回任务列表")).toBeVisible();
  await expect(page.locator(".conversation-title")).toHaveText(`${prefix} zero`);
}

for (const colorScheme of ["light", "dark"] as const) {
  for (const provider of ["codex", "claude", "grok"] as const) {
    test(`${provider} size cards expose K/M/G/0, unknown and partial tones at narrow ${colorScheme} keyboard view`, async ({ page }) => {
      await page.setViewportSize({ width: 390, height: 844 });
      await page.emulateMedia({ colorScheme, reducedMotion: "reduce" });
      await mockApi(page);
      if (provider === "codex") {
        await mockCommand(page, "threads.list", () => codexRows);
        await mockCommand(page, "threads.detail", args => codexDetail(codexRows.find(row => row.id === args.id) ?? codexRows[0]));
      } else if (provider === "claude") {
        await mockCommand(page, "claude.list", () => claudeRows);
        await mockCommand(page, "claude.detail", args => ({ summary: claudeRows.find(row => row.sessionKey === args.sessionKey) ?? claudeRows[0], events: [], hasMore: false, totalEvents: 0 }));
      } else {
        await mockCommand(page, "grok.list", () => grokRows);
        await mockCommand(page, "grok.detail", args => ({ summary: grokRows.find(row => row.id === args.id) ?? grokRows[0], events: [] }));
      }
      await openProvider(page, provider);
      await assertCards(page, provider);
    });
  }
}
