import { test, expect } from "@playwright/test";
import { installMockIpc, recordedCommands } from "./mock-ipc";
import { onboardedFixtures } from "./fixtures";

// Drives the daily loop on a fully onboarded, commit-ready workspace:
// shell boots → workflow home → dashboard → git → code, asserting the key UI
// state at each surface. IPC is mocked, so this asserts the frontend wiring
// (hooks → queries → render), not the Rust backend.
test.describe("daily loop (onboarded)", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, onboardedFixtures);
    await page.goto("/");
  });

  test("shell boots with active workspace + status strip", async ({ page }) => {
    await expect(page.getByText("AI control cockpit")).toBeVisible();
    // Header shows the active project name from get_active_project_config.
    await expect(page.getByRole("heading", { level: 2, name: "RepoDesk" })).toBeVisible();
    // Status strip reflects the mocked snapshot.
    await expect(page.getByText("Wire N2 E2E smoke")).toBeVisible();
    await expect(page.getByText("3 changes")).toBeVisible();
    await expect(page.getByText("1/2 working")).toBeVisible();
  });

  test("workflow home shows a commit-ready next step and route", async ({ page }) => {
    // Workflow is the default home surface.
    await expect(page.getByRole("heading", { level: 1, name: "Run safety checks" })).toBeVisible();
    // Commit readiness panel reflects the "ready" status.
    await expect(page.getByText("All checks passed — safe to commit")).toBeVisible();
    await expect(page.getByText("Ready to commit")).toBeVisible();
    // Best route panel rendered from routing_snapshot.
    await expect(page.getByRole("heading", { name: "ollama / llama3" })).toBeVisible();
    // The "do next safe step" CTA is enabled (not blocked by onboarding).
    await expect(page.getByRole("button", { name: "Build bounded context" })).toBeEnabled();
  });

  test("navigates the Work tabs without crashing", async ({ page }) => {
    for (const tab of ["Dashboard", "Git", "Code"]) {
      await page.getByRole("button", { name: new RegExp(`^${tab}`) }).click();
      // Each tab keeps the shell intact (no error boundary).
      await expect(page.locator(".app-shell")).toBeVisible();
      await expect(page.getByText("Something went wrong")).toHaveCount(0);
    }
  });

  test("frontend actually issued the daily-loop commands through IPC", async ({ page }) => {
    // Give the React Query hooks a beat to fire.
    await expect(page.getByText("All checks passed — safe to commit")).toBeVisible();
    const commands = await recordedCommands(page);
    expect(commands).toContain("desktop_snapshot");
    expect(commands).toContain("product_workflow_state");
    expect(commands).toContain("git_workspace_snapshot");
    expect(commands).toContain("model_health_snapshot");
  });
});
