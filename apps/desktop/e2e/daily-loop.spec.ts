import { test, expect, type Page } from "@playwright/test";
import { installMockIpc, recordedCommands } from "./mock-ipc";
import { onboardedFixtures } from "./fixtures";

// Tab entries are `.nav-item`; deep tabs live under collapsible group toggles
// (Work / AI / System). Helpers scope to those so a group name (e.g. "Work")
// never collides with a same-named tab.
function tabButton(page: Page, name: string) {
  // Match the nav-item whose title (the `strong`) is exactly `name`, so a name
  // like "Memory" never matches another tab's subtitle ("Runs, memory & audit")
  // and the leading icon doesn't defeat a start anchor.
  return page.locator(".nav-item").filter({ has: page.getByText(name, { exact: true }) });
}
async function openGroup(page: Page, group: string) {
  await page.locator(".nav-group-toggle").filter({ hasText: group }).click();
}

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
    await expect(tabButton(page, "Work")).toHaveClass(/active/);
    await expect(page.locator(".phase-rail .phase-chip")).toHaveCount(6);
    await expect(page.getByText("Agent handoff")).toBeVisible();
    await expect(page.getByText("Sent to agent")).toBeVisible();
    await expect(page.getByText("Comes back")).toBeVisible();
  });

  test("navigates every tab without crashing", async ({ page }) => {
    const primaryTabs = ["Work", "Changes", "History", "Models & Cost", "Settings"];
    const moreTabs = ["Dashboard", "Orchestrate", "Playbooks", "System Registry", "Debug"];
    for (const tab of primaryTabs) {
      await tabButton(page, tab).click();
      await expect(page.locator(".app-shell")).toBeVisible();
      await expect(page.getByText("This view crashed")).toHaveCount(0);
      await expect(page.getByText("Something went wrong")).toHaveCount(0);
    }
    // Expand all three Advanced groups, then visit each deep tab.
    for (const group of ["Work", "AI", "System"]) await openGroup(page, group);
    for (const tab of moreTabs) {
      await tabButton(page, tab).click();
      await expect(page.locator(".app-shell")).toBeVisible();
      await expect(page.getByText("This view crashed")).toHaveCount(0);
      await expect(page.getByText("Something went wrong")).toHaveCount(0);
    }
  });

  test("burger collapses the sidebar to an icon rail", async ({ page }) => {
    const sidebar = page.locator(".sidebar");
    await expect(sidebar).not.toHaveClass(/sidebar--collapsed/);
    // Labels are visible while expanded.
    await expect(tabButton(page, "Work").locator(".nav-text")).toBeVisible();

    await page.getByRole("button", { name: /Collapse sidebar/ }).click();
    await expect(sidebar).toHaveClass(/sidebar--collapsed/);
    // Collapsed: the icon stays, the label is hidden (icons-only rail).
    await expect(page.locator(".nav-item").first().locator(".nav-icon")).toBeVisible();
    await expect(page.locator(".nav-item").first().locator(".nav-text")).toBeHidden();

    // Expanding restores the labels.
    await page.getByRole("button", { name: /Expand sidebar/ }).click();
    await expect(sidebar).not.toHaveClass(/sidebar--collapsed/);
  });

  test("Models tab guides setup with human status and fixes", async ({ page }) => {
    // Models is now the default "Runtime health" view inside the Models & Cost surface.
    await tabButton(page, "Models & Cost").click();
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

  test("playbook shortcuts navigate to real work surfaces with feedback", async ({ page }) => {
    await openGroup(page, "AI");
    await tabButton(page, "Playbooks").click();
    await expect(page.getByRole("heading", { name: "Workflow shortcuts" })).toBeVisible();
    // Playbooks are now authorable: a New-playbook control + per-card edit/delete.
    await expect(page.getByRole("button", { name: "New playbook" })).toBeVisible();
    await expect(page.getByText("No hidden run").first()).toBeVisible();
    await expect(page.getByText("Visible result").first()).toBeVisible();

    await page.getByRole("button", { name: "Open Changes" }).click();

    await expect(tabButton(page, "Changes")).toHaveClass(/active/);
    await expect(page.getByText("Active view")).toBeVisible();
    await expect(page.getByText("Security Hotspot Review: opened Changes.")).toBeVisible();
  });

  test("Memory shows what becomes agent context", async ({ page }) => {
    // Memory is now a subnav view inside the History surface.
    await tabButton(page, "History").click();
    await page.getByRole("tab", { name: "Memory" }).click();
    await expect(page.getByRole("heading", { name: "What becomes agent context" })).toBeVisible();
    await expect(page.getByText("Memory pipeline")).toBeVisible();
    await expect(page.getByText("Agent slice")).toBeVisible();
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
    await openGroup(page, "AI");
    await tabButton(page, "Orchestrate").click();
    await expect(page.getByRole("heading", { name: /Run sub-agents/ })).toBeVisible();
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
