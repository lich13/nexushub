import { configureDemoFixtureKey } from "./api/shared";
import { runtimeContext } from "./runtime";

configureDemoFixtureKey(runtimeContext().kind === "desktop" ? "macos-tauri" : "linux-web");

export {
  runtimeCapabilities,
  runtimeCapabilitiesForRuntime,
  runtimeCapabilitiesFromSystemStatus,
  type RuntimeCapabilityMatrix
} from "./domain/capabilities";

export { ApiError } from "./api/shared";
export { desktopRuntimeSessionUser, getPublicSettings, login, logout, me } from "./api/auth";
export { getSecurity, saveSecurity, changePassword } from "./api/settings";
export {
  getSystemStatus,
  getSystemVersion,
  listProviders,
  getPlatformOverview,
} from "./api/system";
export {
  getProbeStatus,
  getProbeSettings,
  saveProbeSettings,
  getProbeLogsDbStatus,
  getProbeEvents,
  runProbeBarkTest,
  runProbeHooksInstall,
  runProbeLogsDbDryRun,
  runProbeLogsDbExecute
} from "./api/probe";
export { getUpdateStatus, updates } from "./api/updates";
export { listGrokSessions, getGrokSession, renameGrokSession, previewGrokSessionDelete, deleteGrokSession } from "./api/grok";
export type { UnifiedUpdateAction, UpdateActionResult } from "./api/updates";
export {
  listThreads,
  getThread,
  getThreadBlocks,
  archiveThread,
  restoreThread,
  renameThread,
  subscribeThreadEvents
} from "./api/threads";
export type { ThreadDetailOptions } from "./api/threads";
export {
  dryRunArchiveDelete,
  startArchiveDelete,
  dryRunHiddenThreadDelete,
  startHiddenThreadDelete,
  listJobs,
  getJob
} from "./api/jobs";
