import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  getDesktopWebUiSettings,
  getDesktopWebUiStatus,
  resetDesktopWebUiPassword,
  saveDesktopWebUiSettings,
  startDesktopWebUi,
  stopDesktopWebUi
} from "../api";
import type {
  DesktopWebUiPasswordReset,
  DesktopWebUiSettingsPatch
} from "../../types";
import { preservePreviousQueryData } from "./shared";

export const desktopWebUiQueryKeys = {
  settings: ["desktop-webui", "settings"] as const,
  status: ["desktop-webui", "status"] as const
};

export function useDesktopWebUiQueries(enabled: boolean) {
  return {
    settings: useQuery({
      queryKey: desktopWebUiQueryKeys.settings,
      queryFn: getDesktopWebUiSettings,
      enabled,
      placeholderData: preservePreviousQueryData
    }),
    status: useQuery({
      queryKey: desktopWebUiQueryKeys.status,
      queryFn: getDesktopWebUiStatus,
      enabled,
      refetchInterval: enabled ? 5000 : false,
      placeholderData: preservePreviousQueryData
    })
  };
}

export function useDesktopWebUiActions() {
  const qc = useQueryClient();
  const invalidate = () => {
    qc.invalidateQueries({ queryKey: desktopWebUiQueryKeys.settings });
    qc.invalidateQueries({ queryKey: desktopWebUiQueryKeys.status });
  };

  return {
    saveSettings: useMutation({
      mutationFn: (settings: DesktopWebUiSettingsPatch) => saveDesktopWebUiSettings(settings),
      onSuccess: (settings) => {
        qc.setQueryData(desktopWebUiQueryKeys.settings, settings);
        invalidate();
      }
    }),
    resetPassword: useMutation({
      mutationFn: (request: DesktopWebUiPasswordReset) => resetDesktopWebUiPassword(request),
      onSuccess: (settings) => {
        qc.setQueryData(desktopWebUiQueryKeys.settings, settings);
        invalidate();
      }
    }),
    start: useMutation({
      mutationFn: startDesktopWebUi,
      onSuccess: (status) => {
        qc.setQueryData(desktopWebUiQueryKeys.status, status);
        invalidate();
      }
    }),
    stop: useMutation({
      mutationFn: stopDesktopWebUi,
      onSuccess: (status) => {
        qc.setQueryData(desktopWebUiQueryKeys.status, status);
        invalidate();
      }
    })
  };
}
