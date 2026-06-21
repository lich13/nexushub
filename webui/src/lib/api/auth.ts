import type {
  PublicSettings,
  SessionUser
} from "../../types";
import {
  desktopSessionUser,
  runtimeRpc,
  runtimeValue
} from "./transport";
import { USE_DEMO } from "./shared";

export function desktopRuntimeSessionUser(): SessionUser {
  return desktopSessionUser();
}

export async function getPublicSettings(): Promise<PublicSettings> {
  if (USE_DEMO) {
    return { site_name: "NexusHub", turnstile_enabled: false, turnstile_required: false, turnstile_site_key: "", turnstile_action: "login", admin_configured: true };
  }
  return runtimeRpc<PublicSettings>("getPublicSettings");
}

export async function login(username: string, password: string, turnstileToken?: string | null): Promise<SessionUser> {
  if (USE_DEMO) {
    return runtimeValue<SessionUser>({
      web: { id: "dev", username, csrf_token: "dev-csrf" },
      desktop: () => desktopSessionUser()
    });
  }
  return runtimeRpc<SessionUser>("login", { username, password, turnstile_token: turnstileToken ?? null });
}

export async function logout(csrfToken?: string | null): Promise<void> {
  if (USE_DEMO) return;
  await runtimeRpc<void>("logout", { csrfToken });
}

export async function me(): Promise<SessionUser> {
  if (USE_DEMO) {
    return runtimeValue<SessionUser>({
      web: { id: "dev", username: "admin", csrf_token: "dev-csrf" },
      desktop: () => desktopSessionUser()
    });
  }
  return runtimeRpc<SessionUser>("me");
}
