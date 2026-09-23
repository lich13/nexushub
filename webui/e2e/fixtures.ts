import { expect, type Page } from "@playwright/test";
import * as demo from "../src/lib/api/demo";

export async function mockApi(page: Page, signedIn = true) {
  const calls: string[] = [];
  const titles = new Map<string, string>();
  const archived = new Set<string>();
  let probeSettings = demo.demoProbeSettings();
  probeSettings.notifications = { ...probeSettings.notifications, enabled: true, device_key_configured: true };
  let jobReads = 0;
  let grok = [{ id: "grok-fixture", title: "Grok fixture", cwd: "/isolated/workspace", path: "/isolated/sessions/grok-fixture", messageCount: 2, status: "recent" }];
  let pi = [{
    id: "pi-native-fixture",
    sessionKey: "project/session-fixture.jsonl",
    title: "Pi fixture",
    cwd: "/isolated/pi-workspace",
    path: "/isolated/pi-sessions/project/session-fixture.jsonl",
    updatedAt: "2026-09-22T08:00:00Z",
    messageCount: 3,
    lastMessage: "Current Pi branch",
    status: "recent",
    formatVersion: 3,
    canRename: true,
    renameBlockReason: null,
    canDelete: true,
    deleteBlockReason: null,
    readError: null
  }];
  if (signedIn) await page.addInitScript((session) => localStorage.setItem("nexushub-session", JSON.stringify(session)), demo.demoSessionUser());
  await page.route("**/api/rpc/**", async (route) => {
    const name = decodeURIComponent(new URL(route.request().url()).pathname.split("/api/rpc/")[1]);
    if (name.startsWith("threadEvents/")) return route.fulfill({ contentType: "text/event-stream", body: ": ready\n\n" });
    calls.push(name);
    if (name === "grok.detail" && !grok.length) return route.fulfill({ status: 500, json: { error: "Grok session not found" } });
    if (name === "pi.detail" && !pi.length) return route.fulfill({ status: 500, json: { error: "Pi session not found" } });
    const args = route.request().postDataJSON() ?? {};
    const responses: Record<string, () => unknown> = {
      "auth.publicSettings": demo.demoPublicSettings,
      "auth.login": demo.demoSessionUser,
      "auth.me": demo.demoSessionUser,
      "system.status": demo.demoSystemStatus,
      "system.version": demo.demoSystemVersion,
      "system.platform": demo.demoPlatformOverview,
      "threads.list": () => demo.demoThreads("all", "").map((thread) => ({ ...thread, title: titles.get(thread.id) ?? thread.title, status: archived.has(thread.id) ? "Archived" : thread.status })).filter((thread) => {
        const status = args.status ?? "all";
        return (status === "archived" ? thread.status === "Archived" : thread.status !== "Archived" && (status === "all" || thread.status === ({ running: "Running", "reply-needed": "ReplyNeeded", recoverable: "Recoverable" } as Record<string, string>)[status])) && thread.title.toLowerCase().includes((args.q ?? "").toLowerCase());
      }),
      "threads.detail": () => { const detail = demo.demoThreadDetail(args.id); return { ...detail, summary: { ...detail.summary, title: titles.get(args.id) ?? detail.summary.title, status: archived.has(args.id) ? "Archived" : detail.summary.status } }; },
      "threads.rename": () => { titles.set(args.threadId, args.name); return { ok: true }; },
      "threads.archive": () => { archived.add(args.threadId); return { ok: true }; },
      "threads.restore": () => { archived.delete(args.threadId); return { ok: true }; },
      "threads.blocks": () => demo.demoThreadBlockPage(args.id),
      "updates.status": demo.demoUpdateStatus,
      "jobs.list": demo.demoJobs,
      "jobs.detail": () => ({ ...demo.demoJob(args.id), status: ++jobReads < 3 ? "running" : "succeeded" }),
      "updates.check": () => ({ job_id: "fixture-job" }),
      "probe.status": () => ({ available: true, data: demo.demoProbeStatus() }),
      "probe.settings.get": () => ({ available: true, data: probeSettings }),
      "probe.settings.save": () => {
        const input = args.settings;
        probeSettings = {
          ...probeSettings,
          codex: { ...probeSettings.codex, ...input.codex },
          probe: { ...probeSettings.probe, ...input.probe },
          notifications: { ...probeSettings.notifications, ...input.probe.notifications }
        };
        return probeSettings;
      },
      "probe.barkTest": () => ({ job_id: "fixture-job" }),
      "cleanup.archiveDryRun": demo.demoArchiveDeletePlan,
      "cleanup.archiveExecute": demo.demoArchiveDeleteResult,
      "cleanup.hiddenDryRun": demo.demoHiddenThreadDeletePlan,
      "cleanup.hiddenExecute": demo.demoHiddenThreadDeleteResult,
      "probe.events": demo.demoProbeEvents,
      "security.get": demo.demoSecurity,
      "grok.list": () => grok.filter((item) => item.title.includes(args.q ?? "")),
      "grok.detail": () => ({ summary: grok[0], events: [{ kind: "user_message_chunk", text: "Check the isolated fixture." }, { kind: "assistant_message_chunk", text: "## Result\n\nReadable response with a [link](https://example.com).\n\n```sh\nprintf test\n```" }] }),
      "grok.rename": () => { grok = grok.map((item) => ({ ...item, title: args.title })); return grok[0]; },
      "grok.deletePreview": () => ({ ...grok[0], fingerprint: "fixture-fingerprint", fileCount: 2, bytes: 128 }),
      "grok.deleteExecute": () => { if (!args.confirmed || args.fingerprint !== "fixture-fingerprint") throw new Error("missing confirmation"); grok = []; return { id: args.id, deleted: true, bytes: 128 }; },
      "pi.list": () => pi.filter((item) => [item.title, item.id, item.cwd].some((value) => value.toLowerCase().includes((args.q ?? "").toLowerCase()))),
      "pi.detail": () => ({ summary: pi.find((item) => item.sessionKey === args.sessionKey) ?? pi[0], events: [
        { kind: "user_message", role: "user", text: "Inspect the current branch." },
        { kind: "tool_call", text: "read", callId: "fixture-call", status: "in_progress", detail: "{\"path\":\"README.md\"}" },
        { kind: "tool_result", text: "read", callId: "fixture-call", status: "completed", detail: "NexusHub" },
        { kind: "compaction", text: "Earlier work was compacted." },
        { kind: "branch_summary", text: "Current Pi branch" },
        { kind: "assistant_message", role: "assistant", text: "Pi fixture result" }
      ] }),
      "pi.rename": () => { pi = pi.map((item) => item.sessionKey === args.sessionKey ? { ...item, title: args.title } : item); return pi[0]; },
      "pi.deletePreview": () => ({ ...pi.find((item) => item.sessionKey === args.sessionKey), fingerprint: "pi-fixture-fingerprint", fileCount: 1, bytes: 256 }),
      "pi.deleteExecute": () => { if (!args.confirmed || args.fingerprint !== "pi-fixture-fingerprint") throw new Error("missing confirmation"); pi = pi.filter((item) => item.sessionKey !== args.sessionKey); return { sessionKey: args.sessionKey, deleted: true, bytes: 256 }; }
    };
    const response = responses[name];
    await route.fulfill({ status: response ? 200 : 404, json: response ? response() : { error: `Unmocked command: ${name}` } });
  });
  return calls;
}

export async function assertContrast(page: Page, selector: string, minimum = 4.5, property = "color") {
  const samples = await page.locator(selector).evaluateAll((elements, property) => {
    const rgb = (value: string) => (value.match(/[\d.]+/g) ?? []).map(Number);
    const luminance = (color: number[]) => color.slice(0, 3).reduce((sum, channel, i) => {
      const c = channel / 255;
      return sum + (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4) * [0.2126, 0.7152, 0.0722][i];
    }, 0);
    return elements.filter((element) => element.getClientRects().length).map((element) => {
      const chain: Element[] = [];
      for (let parent: Element | null = element; parent; parent = parent.parentElement) chain.unshift(parent);
      let background = [255, 255, 255];
      for (const parent of chain) {
        const color = rgb(getComputedStyle(parent).backgroundColor);
        const alpha = color[3] ?? 1;
        background = background.map((channel, i) => (color[i] ?? channel) * alpha + channel * (1 - alpha));
      }
      const color = rgb(getComputedStyle(element).getPropertyValue(property));
      const foreground = background.map((channel, i) => color[i] * (color[3] ?? 1) + channel * (1 - (color[3] ?? 1)));
      const a = luminance(foreground), b = luminance(background);
      return { text: element.textContent?.slice(0, 60) || element.tagName, ratio: (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05) };
    });
  }, property);
  expect(samples.length, `visible samples for ${selector}`).toBeGreaterThan(0);
  for (const sample of samples) expect(sample.ratio, `${sample.text}: contrast ${sample.ratio.toFixed(2)}`).toBeGreaterThanOrEqual(minimum);
}

export async function assertNoOverflow(page: Page) {
  const widths = await page.evaluate(() => ({ scroll: document.documentElement.scrollWidth, available: document.documentElement.clientWidth }));
  expect(widths.scroll, `document width ${widths.scroll}, available width ${widths.available}`).toBeLessThanOrEqual(widths.available);
}
