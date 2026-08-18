import { test, expect, type Page } from "@playwright/test";
import { installMockIpc, recordedCommands } from "./mock-ipc";
import { currentOnboardedFixtures } from "./current-fixtures";

function tabButton(page: Page, name: string) {
  const currentTitle = name === "History" ? "Runs" : name;
  return page.getByRole("button", { name: new RegExp(`^${currentTitle} —`) }).first();
}

async function openFromPalette(page: Page, title: string) {
  await page.getByRole("button", { name: "Command palette" }).click();
  const input = page.getByRole("textbox", { name: "Search commands" });
  await input.fill(title);
  await page.keyboard.press("Enter");
}

async function openProjectsView(page: Page, name: "Knowledge" | "Work templates") {
  await tabButton(page, "Projects").click();
  await page.getByRole("tab", { name }).click();
}

test.describe("daily loop (onboarded)", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, currentOnboardedFixtures);
    await page.goto("/");
  });

  test("shell boots with active workspace + status strip", async ({ page }) => {
    await expect(page.locator(".activity-brand")).toBeVisible();
    await expect(page.locator("header").getByText("Wire N2 E2E smoke")).toBeVisible();
    await expect(page.getByText("3 changes")).toBeVisible();
    await expect(page.getByText("Run agent", { exact: true }).first()).toBeVisible();
  });

  test("Work is the home surface with the phase rail", async ({ page }) => {
    await expect(tabButton(page, "Work")).toHaveAttribute("aria-pressed", "true");
    await expect(page.locator(".phase-rail .phase-chip")).toHaveCount(6);
    await expect(page.getByRole("group", { name: "Execution mode" })).toBeVisible();
    await expect(page.getByText("Review decides whether the exact returned ChangeSet is accepted.")).toBeVisible();
  });

  test("navigates every canonical surface and utility without crashing", async ({ page }) => {
    for (const tab of ["Work", "Code", "Changes", "Runs", "Projects"]) {
      await tabButton(page, tab).click();
      await expect(page.locator(".ide-shell")).toBeVisible();
      await expect(page.getByText("This view crashed")).toHaveCount(0);
      await expect(page.getByText("Something went wrong")).toHaveCount(0);
    }

    for (const command of ["Go to Settings", "Go to Debug"]) {
      await openFromPalette(page, command);
      await expect(page.locator(".ide-shell")).toBeVisible();
      await expect(page.getByText("This view crashed")).toHaveCount(0);
      await expect(page.getByText("Something went wrong")).toHaveCount(0);
    }
  });

  test("burger collapses the sidebar to an icon rail", async ({ page }) => {
    const sidebar = page.locator(".workspace-sidebar");
    await expect(sidebar).toBeHidden();
    await page.getByRole("button", { name: "Show Navigator" }).click();
    await expect(sidebar).toBeVisible();
    await page.getByRole("button", { name: "Hide Navigator" }).click();
    await expect(sidebar).toBeHidden();
  });

  test("Settings owns provider keys and runtime configuration", async ({ page }) => {
    await openFromPalette(page, "Go to Settings");
    await expect(page.getByRole("heading", { name: "API keys, providers, and preferences." })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Credentials", exact: true })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Runtime configuration" })).toBeVisible();
    await expect(page.getByText("Codex CLI route enabled")).toBeVisible();
    await expect(page.getByText("Ollama enabled")).toBeVisible();
  });

  test("command palette opens with Ctrl-K and navigates", async ({ page }) => {
    await expect(page.locator(".phase-rail")).toBeVisible();
    await page.locator("body").click();
    await page.keyboard.press("ControlOrMeta+k");
    const palette = page.getByRole("dialog", { name: "RepoDesk command palette" });
    const input = page.getByRole("textbox", { name: "Search commands" });
    await expect(palette).toHaveAttribute("aria-modal", "true");
    await expect(input).toBeVisible();
    await input.fill("Repository explorer");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("button", { name: /^Code —/ })).toHaveAttribute("aria-pressed", "true");
  });

  test("project switcher lists connected projects", async ({ page }) => {
    await page.getByRole("button", { name: "Show Navigator" }).click();
    await page.locator(".workspace-sidebar").getByRole("button", { name: "RepoDesk", exact: true }).click();
    await expect(page.getByRole("button", { name: /my-api/ })).toBeVisible();
    await expect(page.getByRole("button", { name: /RepoDesk/ }).first()).toBeVisible();
  });

  test("Work Templates live inside Projects and open canonical surfaces", async ({ page }) => {
    await openProjectsView(page, "Work templates");
    await expect(page.getByRole("heading", { name: "Reusable workflow entry points" })).toBeVisible();
    await expect(page.getByRole("button", { name: "New work template" })).toBeVisible();
    await expect(page.getByText("No hidden run").first()).toBeVisible();
    await expect(page.getByText("Visible result").first()).toBeVisible();

    await page.getByRole("button", { name: "Open Changes" }).click();
    await expect(tabButton(page, "Changes")).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("region", { name: "Changed files" })).toBeVisible();
  });

  test("reviewed Engineering Knowledge lives inside Projects", async ({ page }) => {
    await openProjectsView(page, "Knowledge");
    await expect(page.getByRole("heading", { name: "Engineering knowledge" })).toBeVisible();
    await expect(page.getByText(/Reviewed rules, decisions and commands RepoDesk can reuse only while their review lifecycle remains valid/)).toBeVisible();
  });

  test("frontend actually issued the daily-loop commands through IPC", async ({ page }) => {
    await expect(page.locator(".phase-rail .phase-chip")).toHaveCount(6);
    const commands = await recordedCommands(page);
    expect(commands).toContain("desktop_snapshot");
    expect(commands).toContain("work_phase_state");
    expect(commands).toContain("git_workspace_snapshot");
    expect(commands).toContain("orchestrate_status");
  });

  test("Changes and Runs own agent change and execution evidence", async ({ page }) => {
    await tabButton(page, "Changes").click();
    await expect(page.getByRole("region", { name: "Changed files" })).toBeVisible();
    await expect(page.getByText("src/app.ts").first()).toBeVisible();

    await tabButton(page, "Runs").click();
    await expect(page.getByRole("tablist", { name: "Runs views" })).toBeVisible();
    await expect(page.getByRole("tab", { name: "Run evidence" })).toHaveAttribute("aria-selected", "true");
    await expect(page.getByText("This view crashed")).toHaveCount(0);
  });
});
