import type {
  DesktopWebUiPasswordReset,
  DesktopWebUiSettings,
  DesktopWebUiSettingsPatch,
  DesktopWebUiStatus
} from "../../types";
import { callCommand } from "./transport";
import { objectValue, USE_DEMO } from "./shared";

const demoSettings: DesktopWebUiSettings = {
  enabled: false,
  listen: "0.0.0.0:15743",
  username: "admin",
  sessionTtlSeconds: 86400,
  cookieSecure: false,
  publicBaseUrl: null,
  turnstileEnabled: false,
  passwordConfigured: false
};

const demoStatus: DesktopWebUiStatus = {
  configured: false,
  enabled: false,
  running: false,
  pid: null,
  listen: "0.0.0.0:15743",
  url: "http://127.0.0.1:15743/nexushub/",
  message: "stopped"
};

export async function getDesktopWebUiSettings(): Promise<DesktopWebUiSettings> {
  if (USE_DEMO) return demoSettings;
  return normalizeDesktopWebUiSettings(await callCommand<unknown>("desktopWebUi.settings.get"));
}

export async function saveDesktopWebUiSettings(
  settings: DesktopWebUiSettingsPatch,
): Promise<DesktopWebUiSettings> {
  if (USE_DEMO) return normalizeDesktopWebUiSettings({ ...demoSettings, ...settings });
  return normalizeDesktopWebUiSettings(await callCommand<unknown>("desktopWebUi.settings.save", { settings }));
}

export async function getDesktopWebUiStatus(): Promise<DesktopWebUiStatus> {
  if (USE_DEMO) return demoStatus;
  return normalizeDesktopWebUiStatus(await callCommand<unknown>("desktopWebUi.status"));
}

export async function startDesktopWebUi(): Promise<DesktopWebUiStatus> {
  if (USE_DEMO) return { ...demoStatus, enabled: true, configured: true, running: true, message: "running" };
  return normalizeDesktopWebUiStatus(await callCommand<unknown>("desktopWebUi.start"));
}

export async function stopDesktopWebUi(): Promise<DesktopWebUiStatus> {
  if (USE_DEMO) return demoStatus;
  return normalizeDesktopWebUiStatus(await callCommand<unknown>("desktopWebUi.stop"));
}

export async function resetDesktopWebUiPassword(
  request: DesktopWebUiPasswordReset,
): Promise<DesktopWebUiSettings> {
  if (USE_DEMO) {
    return normalizeDesktopWebUiSettings({
      ...demoSettings,
      username: request.username,
      passwordConfigured: true
    });
  }
  return normalizeDesktopWebUiSettings(await callCommand<unknown>("desktopWebUi.password.reset", { request }));
}

function boolValue(value: unknown): boolean {
  return value === true || value === "true";
}

function numberValue(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

function stringValue(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function optionalStringValue(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}

export function normalizeDesktopWebUiSettings(payload: unknown): DesktopWebUiSettings {
  const raw = objectValue(payload);
  return {
    enabled: boolValue(raw.enabled),
    listen: stringValue(raw.listen, demoSettings.listen),
    username: stringValue(raw.username, demoSettings.username),
    sessionTtlSeconds: numberValue(raw.sessionTtlSeconds ?? raw.session_ttl_seconds, demoSettings.sessionTtlSeconds),
    cookieSecure: boolValue(raw.cookieSecure ?? raw.cookie_secure),
    publicBaseUrl: optionalStringValue(raw.publicBaseUrl ?? raw.public_base_url),
    turnstileEnabled: boolValue(raw.turnstileEnabled ?? raw.turnstile_enabled),
    passwordConfigured: boolValue(raw.passwordConfigured ?? raw.password_configured)
  };
}

export function normalizeDesktopWebUiStatus(payload: unknown): DesktopWebUiStatus {
  const raw = objectValue(payload);
  return {
    configured: boolValue(raw.configured),
    enabled: boolValue(raw.enabled),
    running: boolValue(raw.running),
    pid: typeof raw.pid === "number" ? raw.pid : null,
    listen: stringValue(raw.listen, demoStatus.listen),
    url: stringValue(raw.url, demoStatus.url),
    message: optionalStringValue(raw.message)
  };
}
