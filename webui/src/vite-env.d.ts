/// <reference types="vite/client" />

declare var __NEXUSHUB_DESKTOP_RUNTIME__: boolean | undefined;
declare var __NEXUSHUB_TEST_INVOKE__:
  | ((command: string, args?: Record<string, unknown>) => unknown)
  | undefined;

interface Window {
  __NEXUSHUB_DESKTOP_RUNTIME__?: boolean;
}
