import { expect, test, type Page } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { installMockIpc } from "./mock-ipc";

async function boot(page: Page, fixtures: Record<string, unknown> = {}) {
  await installMockIpc(page, {
    ...currentOnboardedFixtures,
    memory_list: [],
    ...fixtures,
  });
  await page.goto("/");
}

async function openFromPalette(page: Page, title: string) {
  await page.getByRole("button", { name: "Command palette" }).click();
  const input = page.getByRole("textbox", { name: "Search commands" });
  await input.fill(title);
  await page.keyboard.press("Enter");
}

async function openKnowledgeSurface(page: Page) {
  await page.getByRole("button", { name: /^Projects —/ }).click();
  await page.getByRole("tab", { name: "Knowledge" }).click();
}

async function openKnowledge(page: Page) {
  await openKnowledgeSurface(page);
  await expect(page.getByRole("heading", { name: "Engineering knowledge" })).toBeVisible();
}

test("Settings is global-only while Projects Knowledge owns repository inputs", async ({ page }) => {
  await boot(page);

  await openFromPalette(page, "Go to Settings");
  await expect(page.getByRole("heading", { name: "API keys, providers, and preferences." })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Import context from other AI tools" })).toHaveCount(0);
  await expect(page.getByText("Project Memory & Guidelines", { exact: true })).toHaveCount(0);

  await openKnowledge(page);
  await expect(page.getByRole("heading", { name: "Import context from other AI tools" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Compatibility instructions" })).toBeVisible();
  await expect(page.locator(".project-guidelines-panel")).toContainText("not reviewed Engineering Knowledge");
});

test("legacy project guideline retrieval fails visibly instead of looking empty", async ({ page }) => {
  await boot(page, {
    memory_list: { __mock_error: "legacy guideline store unavailable" },
  });

  await openKnowledge(page);
  const alert = page.getByRole("alert");
  await expect(alert).toContainText("Could not load project guidelines");
  await expect(alert).toContainText("legacy guideline store unavailable");
  await expect(page.getByText("No compatibility guidelines saved yet.")).toHaveCount(0);
});

test("project input tools stay unavailable until a Project is active", async ({ page }) => {
  await boot(page, {
    desktop_snapshot: {
      project_name: "No active project",
      task_title: "No active task",
    },
    get_active_project_config: null,
  });

  await openKnowledgeSurface(page);
  await expect(page.getByText("Connect a project to use Project Knowledge.")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Import context from other AI tools" })).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Compatibility instructions" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Scan project" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Add project guideline" })).toHaveCount(0);
});
