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
    // Status strip reflects the mocked snapshot. Scope to the header: the task
    // title also appears in the workflow TaskSwitcher panel.
    await expect(page.locator("header").getByText("Wire N2 E2E smoke")).toBeVisible();
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

  test("navigates every tab without crashing", async ({ page }) => {
    // Visit all surfaces — partial mock data must render an empty state, never
    // the error boundary ("This view crashed" / "Something went wrong").
    const tabs = ["Dashboard", "Git", "Code", "Models", "Tokens", "Memory", "Orchestrate", "Settings", "System Registry", "Debug"];
    for (const tab of tabs) {
      await page.getByRole("button", { name: new RegExp(`^${tab}`) }).click();
      await expect(page.locator(".app-shell")).toBeVisible();
      await expect(page.getByText("This view crashed")).toHaveCount(0);
      await expect(page.getByText("Something went wrong")).toHaveCount(0);
    }
  });

  test("command palette opens with Ctrl-K and navigates", async ({ page }) => {
    // Wait for the app to mount (so the global keydown listener is attached).
    await expect(page.getByRole("heading", { level: 1, name: "Run safety checks" })).toBeVisible();
    await page.locator("body").click();
    await page.keyboard.press("ControlOrMeta+k");
    const input = page.getByPlaceholder("Search tabs and actions…");
    await expect(input).toBeVisible();
    await input.fill("Git");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("heading", { level: 1, name: "feat/n2-e2e" })).toBeVisible();
  });

  test("project switcher lists connected projects", async ({ page }) => {
    await page.getByRole("button", { name: /RepoDesk/ }).first().click();
    await expect(page.getByRole("button", { name: /my-api/ })).toBeVisible();
    await expect(page.getByRole("button", { name: /Connect project/ })).toBeVisible();
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
