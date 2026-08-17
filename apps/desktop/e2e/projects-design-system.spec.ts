import { expect, test, type Page } from "@playwright/test";
import { installMockIpc } from "./mock-ipc";
import { currentOnboardedFixtures } from "./current-fixtures";
import type { CommandFixtures } from "./fixtures";

function tabButton(page: Page, name: string) {
  return page.getByRole("button", { name: new RegExp(`^${name} —`) }).first();
}

async function bootProjects(page: Page, overrides: CommandFixtures = {}) {
  await installMockIpc(page, { ...currentOnboardedFixtures, ...overrides });
  await page.goto("/");
  const welcomeDialog = page.locator(".app-dialog[role='dialog']");
  if (await welcomeDialog.isVisible()) {
    await welcomeDialog.getByRole("button", { name: "Close" }).click();
    await welcomeDialog.waitFor({ state: "hidden" });
  }
  await tabButton(page, "Projects").click();
}

function projectCard(page: Page, name: string) {
  return page.locator(".project-registry-card").filter({ has: page.getByRole("heading", { name }) });
}

test.describe("Projects design-system convergence", () => {
  test("active project and attribution policy use typed semantic state", async ({ page }) => {
    await bootProjects(page, {
      project_list_configs: [
        {
          name: "RepoDesk",
          path: "/Users/you/code/repodesk",
          project_type: "rust",
          main_language: "rust",
          checks: ["cargo test"],
          context_ignore: [],
          require_exact_change_attribution: true,
        },
        {
          name: "my-api",
          path: "/Users/you/code/my-api",
          project_type: "node",
          main_language: "typescript",
          checks: [],
          context_ignore: [],
          require_exact_change_attribution: false,
        },
      ],
    });

    await expect(page.getByText("Active · RepoDesk", { exact: true })).toHaveAttribute("data-semantic-tone", "positive");

    const active = projectCard(page, "RepoDesk");
    await expect(active.getByText("Exact required", { exact: true })).toHaveAttribute("data-semantic-tone", "info");
    await expect(active.locator(".semantic-action-bar__primary")).toHaveCount(0);

    const inactive = projectCard(page, "my-api");
    await expect(inactive.getByText("Informational", { exact: true })).toHaveAttribute("data-semantic-tone", "neutral");
    await expect(inactive.locator(".semantic-action-bar__primary").getByRole("button", { name: "Open project" })).toBeVisible();
  });

  test("no active project remains a neutral workspace fact", async ({ page }) => {
    await bootProjects(page, {
      desktop_snapshot: { project: null, task: null },
      get_active_project_config: null,
    });

    await expect(page.getByText("No active project", { exact: true })).toHaveAttribute("data-semantic-tone", "neutral");
  });

  test("registry loading, failure and empty states use shared accessible vocabulary", async ({ page }) => {
    await bootProjects(page, {
      project_list_configs: { __mock_delay_ms: 700, __mock_value: [] },
    });
    await expect(page.getByRole("status").filter({ hasText: "Loading projects" })).toBeVisible();

    const failedPage = await page.context().newPage();
    await bootProjects(failedPage, { project_list_configs: { __mock_error: "fixture registry failure" } });
    await expect(failedPage.getByRole("alert")).toContainText("Project registry unavailable");

    const emptyPage = await page.context().newPage();
    await bootProjects(emptyPage, { project_list_configs: [] });
    await expect(emptyPage.getByText("No projects registered.", { exact: true })).toBeVisible();
  });

  test("project setup validation failure is explicit critical mutation feedback", async ({ page }) => {
    await bootProjects(page);
    await page.getByRole("button", { name: "Add project" }).click();
    await page.getByRole("button", { name: "Add and activate project" }).click();

    const failure = page.getByRole("alert").filter({ hasText: "Project name and path are required." });
    await expect(failure).toBeVisible();
    await expect(failure).toHaveAttribute("data-semantic-tone", "critical");
  });

  test("attribution-policy mutation failure remains local and critical", async ({ page }) => {
    await bootProjects(page, {
      project_set_exact_attribution_required: { __mock_error: "fixture policy failure" },
    });

    await projectCard(page, "RepoDesk").getByRole("button", { name: "Require exact attribution" }).click();
    const failure = page.getByRole("alert").filter({ hasText: "Could not update project trust policy" });
    await expect(failure).toBeVisible();
    await expect(failure).toHaveAttribute("data-semantic-tone", "critical");
  });
});
