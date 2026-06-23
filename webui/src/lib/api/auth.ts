import type {
  PublicSettings,
  SessionUser
} from "../../types";
import { runtimeRpc } from "./transport";
import { USE_DEMO } from "./shared";
import { demoSessionUser } from "./demo";

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
    return { site_name: "NexusHub", turnstile_enabled: false, turnstile_required: false, turnstile_site_key: "", turnstile_action: "login", admin_configured: true };
  }
  return runtimeRpc<PublicSettings>("auth.publicSettings");
}

export async function login(username: string, password: string, turnstileToken?: string | null): Promise<SessionUser> {
  if (USE_DEMO) {
    return demoSessionUser(username);
  }
  return runtimeRpc<SessionUser>("auth.login", { username, password, turnstile_token: turnstileToken ?? null });
}

export async function logout(csrfToken?: string | null): Promise<void> {
  if (USE_DEMO) return;
  await runtimeRpc<void>("auth.logout", { csrfToken });
}

export async function me(): Promise<SessionUser> {
  if (USE_DEMO) {
    return demoSessionUser();
  }
  return runtimeRpc<SessionUser>("auth.me");
}
