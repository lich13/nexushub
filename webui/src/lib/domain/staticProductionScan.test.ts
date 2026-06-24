import { describe, expect, test } from "vitest";

const productionBusinessSources = import.meta.glob([
  "../../App.tsx",
  "../api/**/*.ts",
  "./**/*.ts",
  "../query/**/*.ts",
  "!../api/**/*.test.*",
  "!../api/**/__tests__/**",
  "!./**/*.test.*",
  "!./**/__tests__/**",
  "!../query/**/*.test.*",
  "!../query/**/__tests__/**"
], {
  eager: true,
  query: "?raw",
  import: "default"
}) as Record<string, string>;

const forbiddenLiteralTokens = [
  "desktopCommand",
  "webCommand",
  "runtimeDispatch",
  "runtimeValue",
  "desktop_api_command",
  "desktopApiRoute",
  "invokeDesktopApi",
  "desktop_api",
  "DesktopApi",
  "desktopBridge",
  "WebRoute",
  "DesktopRoute",
  "getRuntimeKind",
  "currentRuntimeCapabilities",
  "systemCapabilitiesForRuntime",
  "linuxBackupPrune",
  "linuxUpdateLabels"
];

const forbiddenCommandNames = [
  "login",
  "logout",
  "me",
  "publicSettings",
  "desktopApi",
  "uploadFiles",
  "threadEvents"
];

const forbiddenCommandPattern = new RegExp(
  String.raw`\b(?:callCommand|startProbeCommand|runTypedUpdateCommand|runtimeRpc|webJsonRpc|invokeDesktop)(?:<[^>]+>)?\(\s*["'](?:${forbiddenCommandNames.join("|")})["']`
);

describe("production business source scan", () => {
  test("does not reintroduce legacy runtime bridge helpers or command names", () => {
    expect(Object.keys(productionBusinessSources).length).toBeGreaterThan(3);

    for (const [path, source] of Object.entries(productionBusinessSources)) {
      for (const token of forbiddenLiteralTokens) {
        expect(source, `${path} must not contain ${token}`).not.toContain(token);
      }
      expect(source, `${path} must not use legacy non-dot runtime command names`).not.toMatch(forbiddenCommandPattern);
      expect(source, `${path} must not declare route maps in business layers`).not.toMatch(/\bconst\s+ROUTES\b/);
    }
  });
});
