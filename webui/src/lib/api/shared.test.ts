import { describe, expect, test } from "vitest";
import type { ProbeSettings, SystemCapabilities } from "../../types";
import { runtimeCapabilitiesFromResponse } from "../domain/capabilities";
import { buildProbeSettingsDraft, buildProbeSettingsPayload } from "../probeUi";
import { normalizeProbeRuntimePayload } from "./shared";

describe("Gotify runtime settings normalization", () => {
  test("normalizes desktop camelCase settings while preserving shared Provider filters", () => {
    const raw = {
      codex: { hostLabel: "fixture-host" },
      probe: { notifications: { enabled: false, notifyCodex: false, notifyGrok: true, notifyClaude: false } },
      notifications: {
        enabled: false, serverUrl: "https://example.invalid/bark/",
        notifyCodex: false, notifyGrok: true, notifyClaude: false
      },
      gotify: {
        enabled: true, serverUrl: "https://example.invalid/gotify/", priority: 5, tokenConfigured: true
      }
    };
    const normalized = normalizeProbeRuntimePayload(raw) as ProbeSettings;
    expect(normalized.gotify).toEqual({
      enabled: true, server_url: "https://example.invalid/gotify/", priority: 5, token_configured: true
    });
    expect(normalized.probe.notifications).toEqual({
      enabled: false, notify_codex: false, notify_grok: true, notify_claude: false
    });
    const draft = buildProbeSettingsDraft(normalized);
    expect(draft.gotify).toMatchObject({ supported: true, enabled: true, token_configured: true, token: "" });
    const saved = buildProbeSettingsPayload(draft, normalized);
    expect(saved.probe.notifications).toMatchObject({
      enabled: false, notify_codex: false, notify_grok: true, notify_claude: false
    });
    expect(saved.gotify).toEqual({ enabled: true, server_url: "https://example.invalid/gotify/", priority: 5 });
    expect(raw.gotify).toHaveProperty("serverUrl", "https://example.invalid/gotify/");
    expect(raw.gotify).not.toHaveProperty("server_url");
  });

  test("preserves snake_case settings, disabled state and priority zero from the remote service", () => {
    const normalized = normalizeProbeRuntimePayload({
      codex: { host_label: "fixture-host" },
      probe: {},
      notifications: { enabled: true, server_url: "https://example.invalid/bark/" },
      gotify: { enabled: false, server_url: "", priority: 0, token_configured: false }
    }) as ProbeSettings;
    expect(normalized.gotify).toEqual({ enabled: false, server_url: "", priority: 0, token_configured: false });
    const draft = buildProbeSettingsDraft(normalized);
    expect(draft.gotify).toMatchObject({ enabled: false, priority: 0, token_configured: false });
    expect(buildProbeSettingsPayload(draft).probe.notifications?.enabled).toBe(true);
  });

  test("does not invent Gotify support when an old service omits the settings", () => {
    const normalized = normalizeProbeRuntimePayload({
      codex: { hostLabel: "fixture-host" },
      probe: {},
      notifications: { enabled: true, serverUrl: "https://example.invalid/bark/" }
    }) as ProbeSettings;
    expect(normalized).not.toHaveProperty("gotify");
    const draft = buildProbeSettingsDraft(normalized);
    expect(draft.gotify).toMatchObject({ supported: false, enabled: false, priority: 5 });
    expect(buildProbeSettingsPayload(draft, normalized)).not.toHaveProperty("gotify");
  });

  test("Gotify capability requires an explicit true response from the selected machine", () => {
    const legacy = {} as SystemCapabilities;
    expect(runtimeCapabilitiesFromResponse({ capabilities: legacy }).gotify).toBe(false);
    expect(runtimeCapabilitiesFromResponse({ capabilities: { ...legacy, gotify: false } }).gotify).toBe(false);
    expect(runtimeCapabilitiesFromResponse({ capabilities: { ...legacy, gotify: true } }).gotify).toBe(true);
    expect(runtimeCapabilitiesFromResponse(undefined).gotify).not.toBe(true);
    expect(runtimeCapabilitiesFromResponse(null).gotify).not.toBe(true);
  });
});
