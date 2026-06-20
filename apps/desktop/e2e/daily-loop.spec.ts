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
    await expect(page.getByText("1/4 working")).toBeVisible();
  });

  test("workflow home shows a commit-ready next step and route", async ({ page }) => {
    // Workflow is the default home surface.
    await expect(page.getByRole("heading", { level: 1, name: "Build smart context" })).toBeVisible();
    // Commit readiness panel reflects the "ready" status.
    await expect(page.getByText("All checks passed — safe to commit")).toBeVisible();
    await expect(page.getByText("Ready to commit")).toBeVisible();
    // Best route panel rendered from routing_snapshot.
    await expect(page.getByRole("heading", { name: "ollama / llama3" })).toBeVisible();
    // The "do next safe step" CTA is enabled (not blocked by onboarding).
    await expect(page.getByRole("button", { name: "Build bounded context" })).toBeEnabled();
  });

  test("journey stepper makes the 8-step path legible", async ({ page }) => {
    // The stepper is the spine of the home surface.
    await expect(page.getByRole("heading", { name: "One task, eight steps" })).toBeVisible();
    // All 8 steps render, including blocked ones downstream.
    const track = page.locator(".journey-track");
    await expect(track.locator(".journey-node")).toHaveCount(8);
    // The current step (smart_context) is highlighted.
    await expect(track.locator(".journey-node.current")).toContainText("Smart Context");
    // Auto-vs-manual is explicit on every step via compact tags.
    await expect(track.locator(".journey-tag")).toHaveCount(8);
    await expect(track.getByText("Auto", { exact: true }).first()).toBeVisible();
    await expect(track.getByText("You", { exact: true }).first()).toBeVisible();
    // The focused step spells out who performs it in full.
    await expect(page.locator(".journey-detail").getByText("RepoDesk does this")).toBeVisible();
    // The primary CTA previews what it will run, before clicking.
    await expect(page.locator(".cta-preview")).toContainText("This runs");
    await expect(page.locator(".cta-preview")).toContainText("Smart Context");
  });

  test("navigates every tab without crashing", async ({ page }) => {
    // Visit all surfaces — partial mock data must render an empty state, never
    // the error boundary ("This view crashed" / "Something went wrong").
    // Primary tabs are always shown; depth tabs live under a collapsible "More".
    const nav = page.locator(".nav-list");
    const primaryTabs = ["Git", "Code", "Memory", "Orchestrate", "Settings"];
    const moreTabs = ["Dashboard", "Models", "Tokens", "System Registry", "Debug"];
    for (const tab of primaryTabs) {
      await nav.getByRole("button", { name: new RegExp(`^${tab}`) }).click();
      await expect(page.locator(".app-shell")).toBeVisible();
      await expect(page.getByText("This view crashed")).toHaveCount(0);
      await expect(page.getByText("Something went wrong")).toHaveCount(0);
    }
    // Expand the "More" section to reach depth & diagnostics surfaces.
    await nav.getByRole("button", { name: /^More/ }).click();
    for (const tab of moreTabs) {
      await nav.getByRole("button", { name: new RegExp(`^${tab}`) }).click();
      await expect(page.locator(".app-shell")).toBeVisible();
      await expect(page.getByText("This view crashed")).toHaveCount(0);
      await expect(page.getByText("Something went wrong")).toHaveCount(0);
    }
  });

  test("a journey step can be run from its card with a result", async ({ page }) => {
    // The focused (current) step card offers a run action…
    const detail = page.locator(".journey-detail");
    const runBtn = detail.getByRole("button", { name: /^Run Smart Context/ });
    await expect(runBtn).toBeVisible();
    await runBtn.click();
    // …and reports a human-readable result inline.
    await expect(detail.getByText(/Smart Context done\./)).toBeVisible();
  });

  test("Models tab guides setup with human status and fixes", async ({ page }) => {
    const nav = page.locator(".nav-list");
    await nav.getByRole("button", { name: /^More/ }).click();
    await nav.getByRole("button", { name: /^Models/ }).click();
    // Readiness-focused headline + attention banner (1 working, 2 need attention).
    await expect(page.getByRole("heading", { name: /Ready for AI/ })).toBeVisible();
    await expect(page.getByText(/need.* attention/i)).toBeVisible();
    // Human statuses replace raw reachability strings.
    await expect(page.getByText("Ready", { exact: true }).first()).toBeVisible();
    await expect(page.getByText("Needs API key")).toBeVisible();
    // Concrete one-click fixes are offered.
    await expect(page.getByRole("button", { name: "Add key" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Launch app" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Turn on" })).toBeVisible();
  });

  test("command palette opens with Ctrl-K and navigates", async ({ page }) => {
    // Wait for the app to mount (so the global keydown listener is attached).
    await expect(page.getByRole("heading", { level: 1, name: "Build smart context" })).toBeVisible();
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
    await expect(page.getByRole("button", { name: /Open from folder/ })).toBeVisible();
    await expect(page.getByRole("button", { name: /Connect with details/ })).toBeVisible();
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
