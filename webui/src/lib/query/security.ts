import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { changePassword, getSecurity, saveSecurity } from "../api";
import type { SecuritySettings } from "../../types";

export const securityQueryKeys = {
  security: ["security"] as const
};

export function useSecurityQuery() {
  return useQuery({
    queryKey: securityQueryKeys.security,
    queryFn: getSecurity
  });
}

export function useSecurityActions(input: {
  csrfToken?: string | null;
  draft: Partial<SecuritySettings> & { turnstile_secret_key?: string };
  passwordForm: { current: string; next: string; confirm: string };
  onSaveSuccess: () => void;
  onPasswordSuccess: () => void;
  onPasswordError: (error: Error) => void;
}) {
  const qc = useQueryClient();

  return {
    save: useMutation({
      mutationFn: () => saveSecurity(input.draft, input.csrfToken),
      onSuccess: () => {
        input.onSaveSuccess();
        qc.invalidateQueries({ queryKey: securityQueryKeys.security });
      }
    }),
    password: useMutation({
      mutationFn: () => changePassword(input.passwordForm.current, input.passwordForm.next, input.csrfToken),
      onSuccess: input.onPasswordSuccess,
      onError: (error: Error) => input.onPasswordError(error)
    })
  };
}
