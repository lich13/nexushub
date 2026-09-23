import type {
  PublicSettings,
  SessionUser
} from "../../types";
import { callCommand } from "./transport";
import { currentDemoFixtureKey, objectValue, USE_DEMO } from "./shared";
import { demoPublicSettings, demoSessionUser } from "./demo";

function desktopSessionUser(): SessionUser {
  return {
    id: "desktop",
    username: "desktop",
    csrf_token: null,
    session_id: "desktop"
  };
}

export function desktopRuntimeSessionUser(): SessionUser {
  return desktopSessionUser();
}

export async function getPublicSettings(): Promise<PublicSettings> {
  if (USE_DEMO) {
    return demoPublicSettings();
  }
  return normalizePublicSettings(await callCommand<unknown>("auth.publicSettings"));
}

export async function login(username: string, password: string, turnstileToken?: string | null): Promise<SessionUser> {
  if (USE_DEMO) {
    return demoSessionUser(username, currentDemoFixtureKey());
  }
  return callCommand<SessionUser>("auth.login", { username, password, turnstile_token: turnstileToken ?? null });
}

export async function logout(csrfToken?: string | null): Promise<void> {
  if (USE_DEMO) return;
  await callCommand<void>("auth.logout", { csrfToken });
}

export async function me(): Promise<SessionUser> {
  if (USE_DEMO) {
    return demoSessionUser("admin", currentDemoFixtureKey());
  }
  return callCommand<SessionUser>("auth.me");
}

function boolValue(value: unknown): boolean {
  return value === true || value === "true";
}

function stringValue(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function normalizePublicSettings(payload: unknown): PublicSettings {
  const raw = objectValue(payload);
  const nested = objectValue(raw.public);
  const source = Object.keys(nested).length ? nested : raw;
  return {
    site_name: stringValue(source.site_name ?? source.siteName, "NexusHub"),
    turnstile_enabled: boolValue(source.turnstile_enabled ?? source.turnstileEnabled),
    turnstile_required: boolValue(source.turnstile_required ?? source.turnstileRequired),
    turnstile_site_key: stringValue(source.turnstile_site_key ?? source.turnstileSiteKey),
    turnstile_action: stringValue(source.turnstile_action ?? source.turnstileAction, "login"),
    admin_configured: boolValue(source.admin_configured ?? source.adminConfigured),
    base_url: typeof (source.base_url ?? source.baseUrl) === "string"
      ? stringValue(source.base_url ?? source.baseUrl)
      : null
  };
}
