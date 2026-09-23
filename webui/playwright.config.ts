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
    { name: "chromium", use: { browserName: "chromium", launchOptions: { ignoreDefaultArgs: ["--hide-scrollbars"] } } },
    { name: "webkit", use: { browserName: "webkit" } }
  ],
  webServer: {
    command: "VITE_USE_REAL_API=1 corepack pnpm@11.0.8 dev --host 127.0.0.1 --port 5197 --strictPort",
    url: "http://127.0.0.1:5197",
    reuseExistingServer: false
  }
});
