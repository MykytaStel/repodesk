import { test, expect } from "@playwright/test";
import { installMockIpc, recordedCommands } from "./mock-ipc";
import { onboardedFixtures } from "./fixtures";

// Drives the daily loop on a fully onboarded workspace. Work is the default home
// and carries the six-phase task flow end to end.
test.describe("daily loop (onboarded)", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, onboardedFixtures);
    await page.goto("/");
  });

  test("shell boots with active workspace + status strip", async ({ page }) => {
    await expect(page.getByText("AI control cockpit")).toBeVisible();
    await expect(page.getByRole("heading", { level: 2, name: "RepoDesk" })).toBeVisible();
    await expect(page.locator("header").getByText("Wire N2 E2E smoke")).toBeVisible();
    await expect(page.getByText("3 changes")).toBeVisible();
    await expect(page.getByText("1/4 working")).toBeVisible();
  });

  test("Work is the home surface with the phase rail", async ({ page }) => {
    const nav = page.locator(".nav-list");
    await expect(nav.getByRole("button", { name: /^Work/ })).toHaveClass(/active/);
    await expect(page.locator(".phase-rail .phase-chip")).toHaveCount(6);
  });

  test("navigates every tab without crashing", async ({ page }) => {
    const nav = page.locator(".nav-list");
    const primaryTabs = ["Work", "Changes", "History", "Settings"];
    const moreTabs = ["Dashboard", "Git", "Code", "Orchestrate", "Memory", "Models", "Tokens", "System Registry", "Debug"];
    for (const tab of primaryTabs) {
      await nav.getByRole("button", { name: new RegExp(`^${tab}`) }).click();
      await expect(page.locator(".app-shell")).toBeVisible();
      await expect(page.getByText("This view crashed")).toHaveCount(0);
      await expect(page.getByText("Something went wrong")).toHaveCount(0);
    }
    await nav.getByRole("button", { name: /^Advanced/ }).click();
    for (const tab of moreTabs) {
      await nav.getByRole("button", { name: new RegExp(`^${tab}`) }).click();
      await expect(page.locator(".app-shell")).toBeVisible();
      await expect(page.getByText("This view crashed")).toHaveCount(0);
      await expect(page.getByText("Something went wrong")).toHaveCount(0);
    }
  });

  test("Models tab guides setup with human status and fixes", async ({ page }) => {
    const nav = page.locator(".nav-list");
    await nav.getByRole("button", { name: /^Advanced/ }).click();
    await nav.getByRole("button", { name: /^Models/ }).click();
    await expect(page.getByRole("heading", { name: /Ready for AI/ })).toBeVisible();
    await expect(page.getByText(/need.* attention/i)).toBeVisible();
    await expect(page.getByText("Ready", { exact: true }).first()).toBeVisible();
    await expect(page.getByText("Needs API key")).toBeVisible();
    await expect(page.getByRole("button", { name: "Add key" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Launch app" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Turn on" })).toBeVisible();
  });

  test("command palette opens with Ctrl-K and navigates", async ({ page }) => {
    await expect(page.locator(".phase-rail")).toBeVisible();
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
    await expect(page.locator(".phase-rail .phase-chip")).toHaveCount(6);
    const commands = await recordedCommands(page);
    expect(commands).toContain("desktop_snapshot");
    expect(commands).toContain("work_phase_state");
    expect(commands).toContain("git_workspace_snapshot");
    expect(commands).toContain("model_health_snapshot");
  });

  test("orchestrate shows reviewable agent changes with proof", async ({ page }) => {
    const nav = page.locator(".nav-list");
    await nav.getByRole("button", { name: /^Advanced/ }).click();
    await nav.getByRole("button", { name: /^Orchestrate/ }).click();
    await expect(page.getByRole("heading", { name: /Conduct sub-agents/ })).toBeVisible();
    await expect(page.getByRole("heading", { name: /agent-changed file/ })).toBeVisible();
    await expect(page.getByText("Checks passed")).toBeVisible();
    await expect(page.getByText("verify passed").first()).toBeVisible();
    await expect(page.getByText("src/app.ts").first()).toBeVisible();
    await expect(page.locator(".diff-add").getByText("+new line")).toBeVisible();
    const commandsBeforeAccept = await recordedCommands(page);
    const checksBeforeAccept = commandsBeforeAccept.filter((command) => command === "orchestrate_check_proof").length;
    await page.getByRole("button", { name: /Accept & run checks \(1\)/ }).click();
    await expect(page.getByText("Accepted src/app.ts (applied and staged)").first()).toBeVisible();
    await expect(page.getByRole("button", { name: "Reject changes" })).toBeVisible();
    const commandsAfterAccept = await recordedCommands(page);
    expect(commandsAfterAccept).toContain("orchestrate_review");
    expect(commandsAfterAccept.filter((command) => command === "orchestrate_check_proof").length).toBeGreaterThan(checksBeforeAccept);
  });
});
