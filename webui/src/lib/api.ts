import { configureDemoFixtureKey } from "./api/shared";
import { runtimeContext } from "./runtime";

configureDemoFixtureKey(runtimeContext().kind === "desktop" ? "macos-tauri" : "linux-api");

export {
  runtimeCapabilities,
  runtimeCapabilitiesForRuntime,
  runtimeCapabilitiesFromResponse,
  type RuntimeCapabilityMatrix
} from "./domain/capabilities";

export { ApiError } from "./api/shared";
export {
  getSystemCapabilities,
  getSystemVersion,
  listProviders,
  getPlatformOverview,
} from "./api/system";
export {
  getProbeStatus,
  getProbeSettings,
  saveProbeSettings,
  getProbeEvents,
  runProbeBarkTest,
  runProbeGotifyTest,
  runProbeHooksInstall,
} from "./api/probe";
export { getUpdateStatus, updates } from "./api/updates";
export { listGrokSessions, getGrokSession, renameGrokSession, previewGrokSessionDelete, deleteGrokSession } from "./api/grok";
export type { UnifiedUpdateAction, UpdateActionResult } from "./api/updates";
export {
  listThreads,
  getThread,
  getSubagentDetail,
  getThreadBlocks,
  archiveThread,
  restoreThread,
  renameThread,
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

export { listClaudeSessions, getClaudeSession, renameClaudeSession, previewClaudeSessionDelete, deleteClaudeSession } from "./api/claude";
