import type { ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, test, vi } from "vitest";
import { buildProbeSettingsDraft } from "../../lib/probeUi";
import { ProbeGotifyCard } from "./ProbeWorkspace";

type CardProps = ComponentProps<typeof ProbeGotifyCard>;

function fixtureDraft() {
  return buildProbeSettingsDraft({
    codex: { host_label: "fixture-host" },
    probe: {},
    notifications: { enabled: false, server_url: "https://example.invalid/bark/" },
    gotify: { enabled: false, server_url: "", priority: 5, token_configured: false }
  });
}

function render(props: Partial<CardProps> = {}) {
  return renderToStaticMarkup(<ProbeGotifyCard
    draft={fixtureDraft()}
    setDraft={() => {}}
    configured={false}
    enabled={false}
    saving={false}
    busy={false}
    saveStatus={null}
    onSave={() => {}}
    onTest={() => {}}
    {...props}
  />);
}

function testButton(html: string) {
  const button = html.match(/<button\b[^>]*>[\s\S]*?<\/button>/g)?.find(value => value.includes("测试推送"));
  expect(button).toBeDefined();
  return button ?? "";
}

describe("Gotify settings card", () => {
  test("starts disabled with priority 5 and an empty password input", () => {
    const html = render();
    expect(html).toContain("启用 Gotify");
    expect(html).not.toContain('checked=""');
    expect(html).toMatch(/<input[^>]*type="number"[^>]*min="0"[^>]*max="10"[^>]*value="5"/);
    expect(html).toMatch(/<input[^>]*type="password"[^>]*autoComplete="new-password"[^>]*value=""/);
    expect(html).toContain("粘贴 Gotify Application Token");
    expect(html).not.toContain("保存时移除 Token");
    expect(testButton(html)).toContain('disabled=""');
  });

  test("a configured Token stays blank and exposes an explicit removal control", () => {
    const draft = fixtureDraft();
    draft.gotify.token_configured = true;
    const html = render({ draft, configured: true });
    expect(html).toContain("已配置，留空保持不变");
    expect(html).toContain("保存时移除 Token");
    expect(html).toMatch(/<input[^>]*type="password"[^>]*value=""/);
  });

  test.each([
    { configured: false, enabled: false, canTest: false },
    { configured: false, enabled: true, canTest: false },
    { configured: true, enabled: false, canTest: false },
    { configured: true, enabled: true, canTest: true }
  ])("test availability uses saved Token and enablement state: %j", ({ configured, enabled, canTest }) => {
    const html = render({ configured, enabled });
    expect(testButton(html).includes('disabled=""')).toBe(!canTest);
    expect(html).not.toMatch(/^<fieldset[^>]*disabled/);
  });

  test("an unsaved Token or enable toggle does not make a test available", () => {
    const draft = fixtureDraft();
    draft.gotify.enabled = true;
    draft.gotify.token = "fixture-unsaved-token";
    expect(testButton(render({ draft }))).toContain('disabled=""');
    expect(testButton(render({ draft, configured: true, enabled: false }))).toContain('disabled=""');
  });

  test("Token removal disables the password and test while keeping save available", () => {
    const draft = fixtureDraft();
    draft.gotify.clear_token = true;
    const html = render({ draft, configured: true, enabled: true });
    expect(html).toMatch(/<input[^>]*type="password"[^>]*disabled=""/);
    expect(testButton(html)).toContain('disabled=""');
    expect(html).not.toMatch(/^<fieldset[^>]*disabled/);
    expect(html).toMatch(/<button class="primary-button"[^>]*>/);
  });

  test.each([
    { saving: true, busy: false },
    { saving: false, busy: true },
    { saving: true, busy: true }
  ])("saving and running jobs disable the whole form: %j", state => {
    const onSave = vi.fn();
    const onTest = vi.fn();
    const html = render({ ...state, configured: true, enabled: true, onSave, onTest });
    expect(html).toMatch(/^<fieldset[^>]*disabled=""/);
    expect(html).toContain("保存");
    expect(html).toContain("测试推送");
    expect(onSave).not.toHaveBeenCalled();
    expect(onTest).not.toHaveBeenCalled();
  });

  test("save feedback distinguishes success from errors without claiming Android receipt", () => {
    const saved = render({ saveStatus: { tone: "success", message: "设置已保存" } });
    expect(saved).toContain('role="status"');
    expect(saved).toContain("设置已保存");
    expect(saved).not.toMatch(/安卓.*(?:已收到|已接收|已送达)/);
    const failed = render({ saveStatus: { tone: "error", message: "示例保存失败" } });
    expect(failed).toContain('role="alert"');
    expect(failed).toContain("示例保存失败");
    expect(failed).not.toContain("设置已保存");
  });
});
