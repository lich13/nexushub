import type {
  PublicSettings,
  SessionUser
} from "../../types";
import { callCommand } from "./transport";
import { currentDemoFixtureKey, USE_DEMO } from "./shared";
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
  return callCommand<PublicSettings>("auth.publicSettings");
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
