import type {
  SecuritySettings
} from "../../types";
import { callCommand } from "./transport";
import { USE_DEMO } from "./shared";
import { demoSecurity } from "./demo";

export async function getSecurity(): Promise<SecuritySettings> {
  if (USE_DEMO) {
    return demoSecurity();
  }
  return callCommand<SecuritySettings>("security.get");
}

export async function saveSecurity(settings: Partial<SecuritySettings> & { turnstile_secret_key?: string }, csrfToken?: string | null) {
  return callCommand<SecuritySettings>("security.save", { settings, csrfToken });
}

export async function changePassword(current_password: string, new_password: string, csrfToken?: string | null) {
  return callCommand("security.changePassword", { current_password, new_password, csrfToken });
}
