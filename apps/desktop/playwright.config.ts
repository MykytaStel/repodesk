import { defineConfig, devices } from "@playwright/test";

// Frontend smoke for the daily loop. Runs the real React UI in Chromium against
// the Vite dev server with a mocked Tauri IPC layer (see e2e/mock-ipc.ts), so it
// runs everywhere — locally on macOS and headless in CI. The real-backend native
// E2E lives under e2e-native/ (tauri-driver, Linux/CI only).
const PORT = 5177;

export default defineConfig({
  testDir: "./e2e",
  testMatch: "**/*.spec.ts",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI
    ? [["github"], ["list"], ["html", { open: "never" }]]
    : "list",
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    trace: "on-first-retry",
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
  ],
  webServer: {
    command: "pnpm dev",
    url: `http://127.0.0.1:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
