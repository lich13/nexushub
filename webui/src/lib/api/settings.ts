import type {
  SecuritySettings
} from "../../types";
import { callCommand } from "./transport";
import { currentDemoFixtureKey, objectValue, USE_DEMO } from "./shared";
import { demoSecurity } from "./demo";

export async function getSecurity(): Promise<SecuritySettings> {
  if (USE_DEMO) {
    return demoSecurity(currentDemoFixtureKey());
  }
  return normalizeSecuritySettings(await callCommand<unknown>("security.get"));
}

export async function saveSecurity(settings: Partial<SecuritySettings> & { turnstile_secret_key?: string }, csrfToken?: string | null) {
  return normalizeSecuritySettings(await callCommand<unknown>("security.save", { settings, csrfToken }));
}

export async function changePassword(current_password: string, new_password: string, csrfToken?: string | null) {
  return callCommand("security.changePassword", { current_password, new_password, csrfToken });
}

function boolValue(value: unknown): boolean {
  return value === true || value === "true";
}

function stringValue(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function numberValue(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function optionalStringValue(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function normalizeSecuritySettings(payload: unknown): SecuritySettings {
  const raw = objectValue(payload);
  return {
    turnstile_enabled: boolValue(raw.turnstile_enabled ?? raw.turnstileEnabled),
    turnstile_required: boolValue(raw.turnstile_required ?? raw.turnstileRequired),
    turnstile_site_key: stringValue(raw.turnstile_site_key ?? raw.turnstileSiteKey),
    turnstile_secret_configured: boolValue(raw.turnstile_secret_configured ?? raw.turnstileSecretConfigured),
    session_ttl_seconds: numberValue(raw.session_ttl_seconds ?? raw.sessionTtlSeconds, 31536000),
    turnstile_expected_hostname: optionalStringValue(raw.turnstile_expected_hostname ?? raw.turnstileExpectedHostname),
    turnstile_expected_action: optionalStringValue(raw.turnstile_expected_action ?? raw.turnstileExpectedAction)
  };
}
