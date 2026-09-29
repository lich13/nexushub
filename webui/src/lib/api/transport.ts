export { RuntimeUnavailableError, runtimeContext } from "../runtime";
import { runtimeContext, runtimeRpc } from "../runtime";
export function currentRuntimeContext() { return runtimeContext(); }
export function callCommand<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T> { return runtimeRpc<T>(command, args); }
