import { expect, test } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { installMockIpc, recordedInvocations } from "./mock-ipc";

const projects = [
  {
    name: "RepoDesk",
    path: "/Users/you/code/repodesk",
    project_type: "rust",
    main_language: "rust",
    checks: ["cargo test"],
    context_ignore: [],
  },
  {
    name: "my-api",
    path: "/Users/you/code/my-api",
    project_type: "repository",
    main_language: "typescript",
    checks: [],
    context_ignore: [],
  },
];

async function openProjects(page: import("@playwright/test").Page, fixtures: Record<string, unknown>) {
  await page.addInitScript(() => {
    window.localStorage.setItem("repodesk.activeTab", "projects");
  });
  await installMockIpc(page, fixtures);
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Repository workspaces" })).toBeVisible();
}

test("Projects uses canonical hasProject semantics for an empty workspace", async ({ page }) => {
  await openProjects(page, {
    ...currentOnboardedFixtures,
    desktop_snapshot: {
      project_name: "No active project",
      task_title: "No active task",
    },
    get_active_project_config: null,
    project_list_configs: projects,
  });

  const activeProjectState = page.getByText("No active project", { exact: true });
  await expect(activeProjectState).toHaveAttribute("data-semantic-tone", "neutral");
  await expect(page.locator(".project-registry-card.active")).toHaveCount(0);
});

test("project activation fails closed when project_use returns ok false", async ({ page }) => {
  await openProjects(page, {
    ...currentOnboardedFixtures,
    project_list_configs: projects,
    project_use: {
      ok: false,
      command: "project use my-api",
      stdout: "",
      stderr: "workspace activation refused",
      exit_code: 1,
    },
  });

  const candidate = page.locator(".project-registry-card").filter({ hasText: "my-api" });
  await candidate.getByRole("button", { name: "Open project" }).click();

  await expect(page.getByRole("alert")).toContainText("workspace activation refused");
  await expect(candidate.getByRole("button", { name: "Open project" })).toBeEnabled();
  await expect(page.locator(".project-registry-card.active")).toContainText("RepoDesk");

  const invocations = await recordedInvocations(page);
  const activation = invocations.find((entry) => entry.cmd === "project_use");
  expect(activation?.args).toEqual({ name: "my-api" });
});
