import { connectionSnapshot, hasNativeRuntime, invokeNative } from "../runtime";
import { RuntimeUnavailableError } from "./transport";

export function canRevealLocalPath(): boolean {
  return connectionSnapshot().target === "local" && hasNativeRuntime();
}

export async function revealLocalPath(path: string): Promise<void> {
  if (!canRevealLocalPath() || !path.startsWith("/") || /[\u0000-\u001f\u007f]/.test(path)) {
    throw new RuntimeUnavailableError("当前路径无法在 Finder 中显示", "revealLocalPath");
  }
  await invokeNative("plugin:opener|reveal_item_in_dir", { paths: [path] });
}
