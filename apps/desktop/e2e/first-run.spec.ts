import { test, expect } from "@playwright/test";
import { installMockIpc } from "./mock-ipc";
import { firstRunFixtures } from "./fixtures";

// First-run: Work is the default home and onboards from its Scope phase.
test.describe("first run (empty workspace)", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, firstRunFixtures);
    await page.goto("/");
  });

  test("header reflects no active project", async ({ page }) => {
    await expect(page.getByRole("heading", { level: 2, name: "No active project" })).toBeVisible();
    await expect(page.getByText("No active task")).toBeVisible();
  });

  test("Work Scope phase funnels into onboarding", async ({ page }) => {
    const rail = page.locator(".phase-rail");
    await expect(rail.locator(".phase-current")).toContainText("Scope");
    await expect(page.getByRole("button", { name: "Connect a project" })).toBeVisible();
    await expect(page.locator(".work-cta-row .primary-cta")).toHaveText("Add or select a project");
  });
});
