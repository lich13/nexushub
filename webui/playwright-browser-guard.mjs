import assert from "node:assert/strict";
import { realpathSync } from "node:fs";
import { basename, dirname, isAbsolute, relative, resolve, sep } from "node:path";

const revisions = /^(?:chromium|chromium_headless_shell|webkit)-\d+$/;

function cacheRoot(executable) {
  for (let path = dirname(resolve(executable)); dirname(path) !== path; path = dirname(path)) {
    if (revisions.test(basename(path))) return dirname(path);
  }
  throw new Error("Browser executable is not in a Playwright browser installation");
}

export function assertManagedExecutable(actual, expected) {
  const root = realpathSync(cacheRoot(expected));
  const path = relative(root, realpathSync(actual));
  assert(path && !isAbsolute(path) && path !== ".." && !path.startsWith(`..${sep}`),
    "Browser executable escaped the Playwright browser cache");
  assert(revisions.test(path.split(sep)[0]), "Browser executable is not in a Playwright revision directory");
  return path;
}

export function launchOptionsForProject(use, env = process.env) {
  assert(["chromium", "webkit"].includes(use.browserName), "Only bundled Chromium and WebKit are supported");
  const options = use.launchOptions ?? {};
  assert(use.channel === undefined && options.channel === undefined, "Browser channel overrides are prohibited");
  assert(options.executablePath === undefined, "Browser executablePath overrides are prohibited");
  assert(use.connectOptions === undefined && !env.PW_TEST_CONNECT_WS_ENDPOINT,
    "Browser connection overrides bypass the managed browser check");
  return { ...options, headless: use.headless ?? options.headless ?? true, host: "127.0.0.1" };
}

export async function verifyProjectBrowser(project, browsers, env = process.env) {
  const options = launchOptionsForProject(project.use, env);
  const browserType = browsers[project.use.browserName];
  const expected = browserType.executablePath();
  // Reject a redirected installation before it could launch a system browser.
  assertManagedExecutable(expected, expected);
  const server = await browserType.launchServer(options);
  try {
    return assertManagedExecutable(server.process().spawnfile, expected);
  } finally {
    await server.close();
  }
}
