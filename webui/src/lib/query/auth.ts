import { useMutation, useQuery } from "@tanstack/react-query";
import { desktopRuntimeSessionUser, getPublicSettings, login, logout } from "../api";
import type { SessionUser } from "../../types";

export const authQueryKeys = {
  publicSettings: ["public-settings"] as const
};

export function usePublicSettingsQuery() {
  return useQuery({
    queryKey: authQueryKeys.publicSettings,
    queryFn: getPublicSettings
  });
}

export function useLoginMutation(onLogin: (user: SessionUser) => void) {
  return useMutation({
    mutationFn: ({ username, password, turnstileToken }: { username: string; password: string; turnstileToken?: string | null }) =>
      login(username, password, turnstileToken),
    onSuccess: onLogin
  });
}

export async function logoutRuntime(csrfToken?: string | null): Promise<void> {
  await logout(csrfToken);
}

export { desktopRuntimeSessionUser };
