import { useMutation, useQuery } from "@tanstack/react-query";
import { desktopRuntimeSessionUser, getPublicSettings, login, logout } from "../api";
import type { SessionUser } from "../../types";
import { bootstrapRuntimeCapabilities } from "./system";

export const authQueryKeys = {
  publicSettings: ["public-settings"] as const
};

export function usePublicSettingsQuery() {
  return useQuery({
    queryKey: authQueryKeys.publicSettings,
    queryFn: () => {
      if (!bootstrapRuntimeCapabilities().webAuth) {
        return {
          site_name: "NexusHub",
          turnstile_enabled: false,
          turnstile_required: false,
          turnstile_site_key: "",
          turnstile_action: "login",
          admin_configured: true
        };
      }
      return getPublicSettings();
    }
  });
}

export function useLoginMutation(onLogin: (user: SessionUser) => void) {
  return useMutation({
    mutationFn: async ({ username, password, turnstileToken }: { username: string; password: string; turnstileToken?: string | null }) => {
      if (!bootstrapRuntimeCapabilities().webAuth) return desktopRuntimeSessionUser();
      return login(username, password, turnstileToken);
    },
    onSuccess: onLogin
  });
}

export async function logoutRuntime(csrfToken?: string | null): Promise<void> {
  if (!bootstrapRuntimeCapabilities().logout) return;
  await logout(csrfToken);
}

export { desktopRuntimeSessionUser };
