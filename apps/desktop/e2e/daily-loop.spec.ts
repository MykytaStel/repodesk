import { test, expect, type Page } from "@playwright/test";
import { installMockIpc, recordedCommands } from "./mock-ipc";
import { onboardedFixtures } from "./fixtures";

// Tab entries are `.nav-item`; deep tabs live under collapsible group toggles
// (Work / AI / System). Helpers scope to those so a group name (e.g. "Work")
// never collides with a same-named tab.
function tabButton(page: Page, name: string) {
  const currentTitle = name === "History" ? "Runs" : name;
  const primary = page.getByRole("button", { name: new RegExp(`^${currentTitle} —`) });
  const drawer = page.locator(".workspace-side-link").filter({ has: page.getByText(currentTitle, { exact: true }) });
  return primary.or(drawer).first();
}
async function openGroup(page: Page, _group: string) {
  if (!(await page.locator(".workspace-sidebar").isVisible())) {
    await page.getByRole("button", { name: "Show workspace sidebar" }).click();
  }
}
async function openFromPalette(page: Page, title: string) {
  await page.getByRole("button", { name: "Command palette" }).click();
  const input = page.getByPlaceholder("Search tabs and actions…");
  await input.fill(title);
  await page.keyboard.press("Enter");
}

// Drives the daily loop on a fully onboarded workspace. Work is the default home
// and carries the six-phase task flow end to end.
test.describe("daily loop (onboarded)", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, onboardedFixtures);
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

  test("navigates every tab without crashing", async ({ page }) => {
    const primaryTabs = ["Work", "Code", "Changes", "History", "Projects"];
    const moreTabs = ["Dashboard", "Orchestrate", "Playbooks", "System Registry", "Debug"];
    for (const tab of primaryTabs) {
      await tabButton(page, tab).click();
      await expect(page.locator(".ide-shell")).toBeVisible();
      await expect(page.getByText("This view crashed")).toHaveCount(0);
      await expect(page.getByText("Something went wrong")).toHaveCount(0);
    }
    for (const tab of moreTabs) {
      await openFromPalette(page, tab);
      await expect(page.locator(".ide-shell")).toBeVisible();
      await expect(page.getByText("This view crashed")).toHaveCount(0);
      await expect(page.getByText("Something went wrong")).toHaveCount(0);
    }
  });

  test("burger collapses the sidebar to an icon rail", async ({ page }) => {
    const sidebar = page.locator(".workspace-sidebar");
    await expect(sidebar).toBeHidden();
    await page.getByRole("button", { name: "Show workspace sidebar" }).click();
    await expect(sidebar).toBeVisible();
    await page.getByRole("button", { name: "Hide workspace sidebar" }).click();
    await expect(sidebar).toBeHidden();
  });

  test("Models tab guides setup with human status and fixes", async ({ page }) => {
    // Models is now the default "Runtime health" view inside the Models & Cost surface.
    await openFromPalette(page, "Models & Cost");
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
    await input.fill("Repository explorer");
    await page.keyboard.press("Enter");
    await expect(page.getByRole("button", { name: /^Code —/ })).toHaveAttribute("aria-pressed", "true");
  });

  test("project switcher lists connected projects", async ({ page }) => {
    await openGroup(page, "Work");
    await page.locator(".workspace-sidebar").getByRole("button", { name: "RepoDesk", exact: true }).click();
    await expect(page.getByRole("button", { name: /my-api/ })).toBeVisible();
    await expect(page.getByRole("button", { name: /RepoDesk/ }).first()).toBeVisible();
  });

  test("playbook shortcuts navigate to real work surfaces with feedback", async ({ page }) => {
    await openFromPalette(page, "Playbooks");
    await expect(page.getByRole("heading", { name: "Workflow shortcuts" })).toBeVisible();
    // Playbooks are now authorable: a New-playbook control + per-card edit/delete.
    await expect(page.getByRole("button", { name: "New playbook" })).toBeVisible();
    await expect(page.getByText("No hidden run").first()).toBeVisible();
    await expect(page.getByText("Visible result").first()).toBeVisible();

    await page.getByRole("button", { name: "Open Changes" }).click();

    await expect(tabButton(page, "Changes")).toHaveAttribute("aria-pressed", "true");
    await expect(page.getByRole("region", { name: "Changed files" })).toBeVisible();
  });

  test("Knowledge shows reviewed engineering memory", async ({ page }) => {
    await openFromPalette(page, "Knowledge");
    await expect(page.getByRole("heading", { name: "Engineering knowledge" })).toBeVisible();
    await expect(page.getByText("Reviewed rules, decisions and commands RepoDesk can reuse in future work.")).toBeVisible();
  });

  test("frontend actually issued the daily-loop commands through IPC", async ({ page }) => {
    await expect(page.locator(".phase-rail .phase-chip")).toHaveCount(6);
    const commands = await recordedCommands(page);
    expect(commands).toContain("desktop_snapshot");
    expect(commands).toContain("work_phase_state");
    expect(commands).toContain("git_workspace_snapshot");
    expect(commands).toContain("orchestrate_status");
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
