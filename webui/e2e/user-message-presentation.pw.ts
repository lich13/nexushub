import { expect, test } from "@playwright/test";
import { assertNoOverflow, mockApi } from "./fixtures";
import * as demo from "../src/lib/api/demo";

const providers = ["codex", "grok", "pi"] as const;
type Provider = typeof providers[number];

const questionOne = "Which fictional branch label should I inspect?\nInclude the requested status line.";
const answerOne = "Use branch-demo only.\nKeep **this answer** as ordinary text.";
const questionTwo = "Should the sample note be changed?";
const answerTwo = "No. Preserve the fictional note.";
const replies = [
  { questionItemId: "question-demo-1", question: questionOne, answer: answerOne },
  { questionItemId: "question-demo-2", question: questionTwo, answer: answerTwo }
];
const replyEnvelope = `<send_user_message_question_reply>${JSON.stringify(replies)}</send_user_message_question_reply>`;
const instructionBlock = [
  "# AGENTS.md instructions for /isolated/fictional-project",
  "",
  "<INSTRUCTIONS>",
  "Use the fictional branch label only.",
  "Keep this exact line break.",
  "</INSTRUCTIONS>"
].join("\n");
const longInstructionBlock = [
  "# AGENTS.md instructions for /isolated/fictional-project",
  "",
  "<INSTRUCTIONS>",
  "Use only this fictional workspace.",
  "# Internal same-level section",
  ...Array.from({ length: 150 }, (_, index) => `Instruction line ${index + 1}: preserve sample-${index + 1}.`),
  "</INSTRUCTIONS>"
].join("\n");
const trailingRequest = "Please inspect sample.txt and keep this request visible.";
const formattedText = `${replyEnvelope}\n\n${instructionBlock}\n\n${trailingRequest}`;
const longFormattedText = `${longInstructionBlock}\n\n${trailingRequest}`;

async function installLegacyRoute(page: Parameters<typeof mockApi>[0], provider: Provider, getTexts: () => string[]) {
  const endpoint = provider === "codex" ? "threads" : provider;
  await page.route(`**/${endpoint}.detail`, route => {
    const texts = getTexts();
    if (provider === "codex") {
      const detail = demo.demoThreadDetail("019e95a0-demo");
      detail.blocks = texts.map((text, index) => ({
        id: `legacy-user-${index}`,
        role: "user",
        kind: "message",
        questions: [],
        text
      }));
      return route.fulfill({ json: detail });
    }
    const events = texts.map(text => provider === "grok"
      ? { kind: "user_message_chunk", text }
      : { kind: "user_message", role: "user", text });
    return route.fulfill({ json: {
      summary: provider === "grok"
        ? { id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace" }
        : { sessionKey: "project/session-fixture.jsonl", title: "Pi fixture", cwd: "/isolated/pi-workspace" },
      events
    } });
  });
}

async function openProvider(page: Parameters<typeof mockApi>[0], provider: Provider, mobile: boolean, getTexts: () => string[]) {
  if (mobile) await page.setViewportSize({ width: 390, height: 844 });
  else await page.setViewportSize({ width: 1440, height: 980 });
  await page.emulateMedia({ colorScheme: mobile ? "dark" : "light", reducedMotion: "reduce" });
  await mockApi(page);
  await installLegacyRoute(page, provider, getTexts);
  await page.goto("/");
  if (provider === "codex") {
    await page.locator(".thread-item").filter({ hasText: "Plan Mode 修复" }).click();
  } else {
    const navigation = page.locator(mobile ? ".mobile-tabs" : ".side-nav");
    await navigation.getByRole("button", { name: provider === "grok" ? "Grok Build" : "Pi", exact: true }).click();
    if (mobile) await page.locator(".provider-session").click();
  }
}

for (const provider of providers) {
  for (const mobile of [false, true]) {
    test(`${provider} legacy user presentation works in ${mobile ? "dark mobile" : "light desktop"}`, async ({ page }) => {
      await page.addInitScript(() => {
        Object.defineProperty(navigator, "clipboard", {
          configurable: true,
          value: { writeText: async (value: string) => { document.documentElement.dataset.copiedAnswer = value; } }
        });
      });
      await openProvider(page, provider, mobile, () => [formattedText]);

      const article = page.locator(".user-message");
      await expect(article).toHaveCount(1);
      const answerNodes = article.locator(".user-answer");
      await expect(answerNodes).toHaveCount(2);
      expect(await answerNodes.nth(0).textContent()).toBe(answerOne);
      expect(await answerNodes.nth(1).textContent()).toBe(answerTwo);
      await expect(answerNodes.nth(0).locator("strong, code, pre, p")).toHaveCount(0);
      const rendered = await article.textContent();
      expect(rendered).not.toContain("<send_user_message_question_reply>");
      expect(rendered).not.toContain("questionItemId");
      expect(rendered).not.toContain(replyEnvelope);
      await expect(page.getByText(trailingRequest, { exact: true })).toBeVisible();

      const firstReply = article.locator(".user-question-reply").first();
      const question = firstReply.locator("details.user-question-context");
      await expect(question).not.toHaveAttribute("open");
      await expect(question.locator(".user-question-full")).toHaveCount(0);
      const questionSummary = question.locator(":scope > summary");
      const questionLabel = questionSummary.locator(":scope > span");
      await expect(questionLabel).toHaveCSS("white-space", "nowrap");
      await expect(questionLabel).toHaveCSS("overflow", "hidden");
      await expect(questionLabel).toHaveCSS("text-overflow", "ellipsis");
      await questionSummary.focus();
      await page.keyboard.press("Enter");
      await expect(question).toHaveAttribute("open", "");
      await expect(question.locator(".user-question-full")).toHaveText(questionOne);

      const copy = firstReply.getByRole("button", { name: "复制回答", exact: true });
      await copy.click();
      await expect(page.locator("html")).toHaveAttribute("data-copied-answer", answerOne);
      await expect(firstReply.getByRole("button", { name: "已复制回答", exact: true })).toBeVisible();

      const instructions = article.locator("details.user-instructions");
      await expect(instructions).toHaveCount(1);
      await expect(instructions).not.toHaveAttribute("open");
      const instructionSummary = instructions.locator(":scope > summary");
      const byteCount = new TextEncoder().encode(instructionBlock).length;
      await expect(instructionSummary).toContainText(`6 行 · ${byteCount} 字节`);
      await expect(instructions.locator(".user-instructions-body")).toHaveCount(0);
      await instructionSummary.focus();
      await page.keyboard.press("Enter");
      await expect(instructions).toHaveAttribute("open", "");
      const instructionBody = instructions.locator(".user-instructions-body");
      await expect(instructionBody).toHaveText(instructionBlock);
      await expect(instructionBody).toHaveCSS("white-space", "pre-wrap");
      await assertNoOverflow(page);
    });
  }
}

test("a failed answer copy shows an accessible error without copying the question", async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: async () => { throw new Error("clipboard denied by fixture"); } }
    });
  });
  await openProvider(page, "pi", false, () => [replyEnvelope]);
  const reply = page.locator(".user-question-reply").first();
  await reply.getByRole("button", { name: "复制回答", exact: true }).click();
  const error = reply.locator('[aria-live="polite"]');
  await expect(error).toBeVisible();
  await expect(error).toContainText("复制失败");
  await expect(reply.getByRole("button", { name: "复制回答", exact: true })).toBeVisible();
});

test("AGENTS disclosure remains open when refreshed history appends a legacy message", async ({ page }) => {
  let texts = [formattedText];
  await openProvider(page, "grok", false, () => texts);
  const question = page.locator("details.user-question-context").first();
  const instructions = page.locator("details.user-instructions");
  await expect(question).not.toHaveAttribute("open");
  await expect(instructions).not.toHaveAttribute("open");
  await question.locator(":scope > summary").click();
  await instructions.locator(":scope > summary").click();
  await expect(question).toHaveAttribute("open", "");
  await expect(instructions).toHaveAttribute("open", "");

  texts = [...texts, "First follow-up for the fictional branch."];
  await page.getByTitle("刷新任务").click();
  await expect(page.getByText("First follow-up for the fictional branch.", { exact: true })).toBeVisible();
  await expect(question).toHaveAttribute("open", "");
  await expect(instructions).toHaveAttribute("open", "");

  await question.locator(":scope > summary").click();
  await instructions.locator(":scope > summary").click();
  await expect(question).not.toHaveAttribute("open");
  await expect(instructions).not.toHaveAttribute("open");
  texts = [...texts, "Second follow-up for the fictional branch."];
  await page.getByTitle("刷新任务").click();
  await expect(page.getByText("Second follow-up for the fictional branch.", { exact: true })).toBeVisible();
  await expect(question).not.toHaveAttribute("open");
  await expect(instructions).not.toHaveAttribute("open");
  await assertNoOverflow(page);
});

test("long user AGENTS body scrolls internally and keeps its choice across mobile navigation and theme changes", async ({ page }) => {
  await openProvider(page, "grok", true, () => [longFormattedText]);
  const instructions = page.locator("details.user-instructions");
  await expect(instructions).not.toHaveAttribute("open");
  const summary = instructions.locator(":scope > summary");
  const lineCount = longInstructionBlock.split("\n").length;
  const byteCount = new TextEncoder().encode(longInstructionBlock).length;
  await expect(summary).toContainText(`${lineCount} 行 · ${byteCount} 字节`);
  await expect(instructions.locator(".user-instructions-body")).toHaveCount(0);

  await summary.click();
  await expect(instructions).toHaveAttribute("open", "");
  const body = instructions.locator(".user-instructions-body");
  await expect(body).toHaveText(longInstructionBlock);
  await expect(body).toContainText("# Internal same-level section");
  await expect(page.getByText(trailingRequest, { exact: true })).toBeVisible();
  await expect(body).toHaveCSS("max-height", "460px");
  const scroll = await body.evaluate(element => ({
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
    overflowY: getComputedStyle(element).overflowY
  }));
  expect(scroll.scrollHeight).toBeGreaterThan(scroll.clientHeight);
  expect(["auto", "scroll"]).toContain(scroll.overflowY);
  await assertNoOverflow(page);

  await page.getByTitle("返回任务列表").click();
  await expect(page.locator(".provider-session")).toBeVisible();
  await page.locator(".provider-session").click();
  await expect(instructions).toHaveAttribute("open", "");
  await page.emulateMedia({ colorScheme: "light" });
  await expect(instructions).toHaveAttribute("open", "");
  await expect(body).toBeVisible();
  await assertNoOverflow(page);
});

const invalidExamples = [
  `Code example:\n\`\`\`xml\n${replyEnvelope}\n\`\`\``,
  `> ${replyEnvelope}`,
  '<send_user_message_question_reply>[{"questionItemId":"question-incomplete","question":"unfinished question"}'
];

for (const provider of providers) {
  test(`${provider} preserves code, quoted and incomplete question envelopes verbatim`, async ({ page }) => {
    await openProvider(page, provider, false, () => invalidExamples);
    await expect(page.locator(".user-question-reply")).toHaveCount(0);
    const bubbles = page.locator(".user-message-bubble");
    await expect(bubbles).toHaveCount(invalidExamples.length);
    for (let index = 0; index < invalidExamples.length; index++)
      expect(await bubbles.nth(index).textContent()).toBe(invalidExamples[index]);
  });
}
