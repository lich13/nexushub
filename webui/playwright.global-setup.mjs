import { chromium, webkit } from "@playwright/test";
import { verifyProjectBrowser } from "./playwright-browser-guard.mjs";

export default async function setup(config) {
  for (const project of config.projects) {
    await verifyProjectBrowser(project, { chromium, webkit });
  }
}
