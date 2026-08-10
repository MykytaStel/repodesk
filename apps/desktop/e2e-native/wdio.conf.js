// WebdriverIO config for the real-backend Tauri E2E smoke.
//
// Drives the actual compiled RepoDesk binary (frontend + IPC + repodesk-core)
// through tauri-driver, which proxies WebDriver to the app's WebView. tauri-driver
// only supports Linux (WebKitWebDriver) and Windows (Edge WebDriver) — NOT macOS —
// so this harness runs in CI on Linux, not on a Mac dev machine. The Playwright
// mock-IPC smoke (../e2e/) is the everywhere-runnable counterpart.
//
// Prereqs (handled by CI / scripts/e2e-native.sh):
//   - `cargo install tauri-driver --locked`
//   - webkit2gtk + WebKitWebDriver on PATH
//   - a release build:  pnpm --dir apps/desktop tauri build
//   - REPODESK_HOME pointed at a throwaway dir (first-run state)

import { mkdirSync, writeFileSync } from "node:fs";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const artifactsDir = resolve(__dirname, "artifacts");

// RepoDesk is a Cargo *workspace*, so `tauri build` emits the binary to the
// workspace-root `target/release`, NOT the crate-local `src-tauri/target`. Search
// the workspace-root dir first (honoring CARGO_TARGET_DIR), then the crate-local
// path as a fallback for non-workspace layouts.
const releaseDirs = [
  process.env.CARGO_TARGET_DIR && resolve(process.env.CARGO_TARGET_DIR, "release"),
  resolve(__dirname, "../../../target/release"), // workspace root
  resolve(__dirname, "../src-tauri/target/release"), // crate-local fallback
].filter(Boolean);

// The cargo bin target is `repodesk-desktop`; the bundler/productName can also
// yield `RepoDesk`. Allow an explicit override and fall back across candidates.
function resolveApplication() {
  const names = [process.env.TAURI_APP_BINARY, "repodesk-desktop", "RepoDesk"].filter(Boolean);
  for (const name of names) {
    if (name.startsWith("/") && existsSync(name)) return name;
    for (const dir of releaseDirs) {
      const full = resolve(dir, name);
      if (existsSync(full)) return full;
    }
  }
  throw new Error(
    `No built Tauri binary found (looked in: ${releaseDirs.join(", ")}; ` +
      `names: ${names.join(", ")}).\nBuild it first: pnpm --dir apps/desktop tauri build`,
  );
}

function artifactName(title) {
  return title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 80) || "native-e2e";
}

let tauriDriver;

export const config = {
  hostname: "127.0.0.1",
  port: 4444,

  specs: ["./specs/**/*.e2e.js"],
  maxInstances: 1,

  capabilities: [
    {
      "tauri:options": {
        application: resolveApplication(),
      },
    },
  ],

  reporters: ["spec"],
  framework: "mocha",
  mochaOpts: {
    ui: "bdd",
    timeout: 120_000,
  },
  logLevel: "warn",
  waitforTimeout: 20_000,
  connectionRetryTimeout: 120_000,

  // tauri-driver must be running before the WebDriver session starts.
  beforeSession() {
    tauriDriver = spawn(process.env.TAURI_DRIVER ?? "tauri-driver", [], {
      stdio: [null, process.stdout, process.stderr],
    });
  },

  // Native failures are otherwise difficult to diagnose from CI logs alone.
  // Capture both the rendered WebView and its DOM without persisting REPODESK_HOME.
  async afterTest(test, _context, { passed }) {
    if (passed) return;
    mkdirSync(artifactsDir, { recursive: true });
    const name = `${Date.now()}-${artifactName(test.title)}`;

    try {
      await browser.saveScreenshot(resolve(artifactsDir, `${name}.png`));
    } catch (error) {
      console.warn(`Could not capture native E2E screenshot: ${error}`);
    }

    try {
      const source = await browser.getPageSource();
      writeFileSync(resolve(artifactsDir, `${name}.html`), source, "utf8");
    } catch (error) {
      console.warn(`Could not capture native E2E DOM: ${error}`);
    }
  },

  afterSession() {
    if (tauriDriver) tauriDriver.kill();
  },
};
