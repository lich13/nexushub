export {
  RuntimeUnavailableError,
  runtimeContext
} from "../runtime";
import type { RuntimeContext } from "../runtime";
import {
  createRuntimeThreadEventSource,
  runtimeContext,
  runtimeRpc
} from "../runtime";

export function currentRuntimeContext(): RuntimeContext {
  return runtimeContext();
}

export function callCommand<T = unknown>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  return runtimeRpc<T>(command, args);
}

export function openThreadEventStream(threadId: string) {
  return createRuntimeThreadEventSource(threadId);
}
