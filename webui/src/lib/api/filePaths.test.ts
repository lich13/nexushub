import { afterEach, describe, expect, test, vi } from "vitest";

describe("file path runtime adapter", () => {
  afterEach(() => {
    delete globalThis.__NEXUSHUB_DESKTOP_RUNTIME__;
    delete globalThis.__NEXUSHUB_TEST_INVOKE__;
    vi.resetModules();
  });

  test("desktop reveals a validated path through the existing opener plugin", async () => {
    globalThis.__NEXUSHUB_DESKTOP_RUNTIME__ = true;
    const invoke = vi.fn(async (command: string, args: unknown) => ({ command, args }));
    globalThis.__NEXUSHUB_TEST_INVOKE__ = invoke;
    const { revealLocalPath } = await import("./filePaths");
    await revealLocalPath("/workspace/中文 文件.md");
    expect(invoke).toHaveBeenCalledWith("plugin:opener|reveal_item_in_dir", { paths: ["/workspace/中文 文件.md"] });
  });

  test("web refuses Finder reveal and keeps the path action copy-only", async () => {
    const { canRevealLocalPath, revealLocalPath } = await import("./filePaths");
    expect(canRevealLocalPath()).toBe(false);
    await expect(revealLocalPath("/workspace/file.md")).rejects.toThrow("无法在 Finder 中显示");
  });
});
