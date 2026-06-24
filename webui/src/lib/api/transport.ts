export {
  RuntimeUnavailableError
} from "../runtime";
import {
  createRuntimeThreadEventSource,
  runtimeRpc,
  uploadRuntimeFiles
} from "../runtime";

export function callCommand<T = unknown>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  return runtimeRpc<T>(command, args);
}

export function uploadFilesTransport<T = unknown>(
  files: File[],
  csrfToken?: string | null,
): Promise<T> {
  return uploadRuntimeFiles<T>(files, csrfToken);
}

export function openThreadEventStream(threadId: string) {
  return createRuntimeThreadEventSource(threadId);
}
