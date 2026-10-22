import { defineConfig } from "@playwright/test";
import { join } from "node:path";
import { tmpdir } from "node:os";

export default defineConfig({
  testDir: "./e2e",
  testMatch: "**/*.pw.ts",
  timeout: 30000,
  fullyParallel: true,
  workers: 2,
  reporter: "list",
  outputDir: process.env.NEXUSHUB_TEST_OUTPUT ?? join(tmpdir(), `codex-${process.env.CODEX_THREAD_ID ?? "nexushub-ci"}`, "playwright"),
  use: { baseURL: "http://127.0.0.1:5197", screenshot: "only-on-failure", trace: "retain-on-failure" },
  projects: [
    // Use Chromium's full browser renderer, including in headless CI.
    { name: "chromium", use: { browserName: "chromium", channel: "chromium", launchOptions: { ignoreDefaultArgs: ["--hide-scrollbars"] } } },
    { name: "webkit", use: { browserName: "webkit" } }
  ],
  webServer: {
    command: process.env.NEXUSHUB_BROWSER_PREVIEW === "1"
      ? "pnpm exec vite preview --host 127.0.0.1 --port 5197 --strictPort"
      : "VITE_USE_REAL_API=1 pnpm dev --host 127.0.0.1 --port 5197 --strictPort",
    url: "http://127.0.0.1:5197",
    reuseExistingServer: false
  }
});
