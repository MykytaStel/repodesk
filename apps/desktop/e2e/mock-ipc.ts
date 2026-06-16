import type { Page } from "@playwright/test";
import type { CommandFixtures } from "./fixtures";

// Installs a fake Tauri IPC layer before any app script runs.
//
// Tauri v2's `invoke(cmd, args)` (from @tauri-apps/api/core) dispatches through
// `window.__TAURI_INTERNALS__.invoke(cmd, args, options)`. By defining that object
// in an init script, we intercept every command the frontend issues — no app
// changes, no Rust backend. Commands present in `fixtures` resolve to their value;
// anything else resolves to `null` (which optionalCommand and the shell hooks tolerate).
//
// `__repodeskMockCalls` records the command sequence so specs can assert that the
// frontend actually drove the daily loop through the IPC seam.
export async function installMockIpc(page: Page, fixtures: CommandFixtures): Promise<void> {
  await page.addInitScript((data: CommandFixtures) => {
    const calls: string[] = [];
    (window as unknown as { __repodeskMockCalls: string[] }).__repodeskMockCalls = calls;

    (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
      invoke(cmd: string) {
        calls.push(cmd);
        const value = Object.prototype.hasOwnProperty.call(data, cmd) ? data[cmd] : null;
        return Promise.resolve(value);
      },
      // Stubs for the event/callback machinery so `@tauri-apps/api/event` and
      // friends don't throw if a component subscribes on mount.
      transformCallback(callback: unknown) {
        void callback;
        return Math.floor(Math.random() * 1_000_000);
      },
      unregisterCallback() {},
      convertFileSrc(path: string) {
        return path;
      },
      metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
    };
  }, fixtures);
}

/** Reads the ordered list of commands the frontend invoked through the mock. */
export async function recordedCommands(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { __repodeskMockCalls?: string[] }).__repodeskMockCalls ?? []);
}
