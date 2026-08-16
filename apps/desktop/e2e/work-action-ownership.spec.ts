import { expect, test, type Page } from "@playwright/test";
import { onboardedFixtures, reviewFixtures, type CommandFixtures } from "./fixtures";
import { installMockIpc } from "./mock-ipc";

const scopePhases = [
  { phase: "scope", status: "available", title: "Scope", summary: "Create an active task" },
  { phase: "prepare", status: "locked", title: "Prepare", summary: "Build bounded context for this task" },
  { phase: "execute", status: "locked", title: "Execute", summary: "Launch the coding agent in an isolated worktree" },
  { phase: "review", status: "locked", title: "Review", summary: "No changes to review yet" },
  { phase: "verify", status: "locked", title: "Verify", summary: "Run final project checks and verification" },
  { phase: "finish", status: "locked", title: "Finish", summary: "Stage, commit, and close the task" },
] as const;

const projectWithoutTask: CommandFixtures = {
  ...onboardedFixtures,
  desktop_snapshot: { project: { name: "RepoDesk" } },
  task_list: [],
  work_phase_state: {
    current: "scope",
    complete: false,
    execution_mode: "agent_run",
    cta: { phase: "scope", label: "Create a task", action_id: null },
    phases: scopePhases,
  },
};

const noProject: CommandFixtures = {
  ...onboardedFixtures,
  desktop_snapshot: {},
  get_active_project_config: null,
  project_list_configs: [],
  task_list: [],
  work_phase_state: {
    current: "scope",
    complete: false,
    execution_mode: "agent_run",
    cta: { phase: "scope", label: "Add or select a project", action_id: null },
    phases: scopePhases,
  },
};

async function boot(page: Page, fixtures: CommandFixtures) {
  await installMockIpc(page, fixtures);
  await page.goto("/");
  const about = page.getByRole("dialog", { name: "Your local-first engineering workspace" });
  if (await about.isVisible()) {
    await about.getByRole("button", { name: "Close" }).click();
  }
}

test("Scope with a project but no task exposes one task-creation action", async ({ page }) => {
  await boot(page, projectWithoutTask);

  await expect(page.locator(".semantic-panel-header__description")).toHaveText("Define the Work Item for this project");
  await expect(page.getByRole("button", { name: "Create task", exact: true })).toHaveCount(1);
  await expect(page.getByRole("button", { name: "Create a task", exact: true })).toHaveCount(0);
  await expect(page.locator(".semantic-action-bar__primary .primary-cta")).toHaveCount(0);
});

test("Scope without a project has one project action and routes it to Projects", async ({ page }) => {
  await boot(page, noProject);

  await expect(page.locator(".semantic-panel-header__description")).toHaveText("Choose the repository for this Work Item");
  await expect(page.getByRole("button", { name: "Connect a project", exact: true })).toHaveCount(1);
  await expect(page.locator(".semantic-action-bar__primary .primary-cta")).toHaveCount(0);

  await page.getByRole("button", { name: "Connect a project", exact: true }).click();
  await expect(page.getByRole("button", { name: /^Projects —/ })).toHaveAttribute("aria-pressed", "true");
});

test("Review owns exactly one primary decision without a second generic CTA", async ({ page }) => {
  await boot(page, reviewFixtures);

  await expect(page.getByRole("button", { name: /Accept .* Verify/ })).toBeVisible();
  await expect(page.getByRole("button", { name: /Reject .* re-run/ })).toBeVisible();
  const primary = page.locator(".semantic-action-bar__primary .primary-cta");
  await expect(primary).toHaveCount(1);
  await expect(primary).toHaveText(/Accept .* Verify/);
});
