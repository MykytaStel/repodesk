import { test, expect, type Page } from "@playwright/test";
import { installMockIpc } from "./mock-ipc";
import { firstRunFixtures } from "./fixtures";

async function openWorkflow(page: Page) {
  const nav = page.locator(".nav-list");
  await nav.getByRole("button", { name: /^Advanced/ }).click();
  await nav.getByRole("button", { name: /^Workflow/ }).click();
}

// First-run: Work is the default home and onboards from Scope; the classic
// Workflow onboarding remains available under Advanced.
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

  test("classic Workflow onboarding still works", async ({ page }) => {
    await openWorkflow(page);
    await expect(page.getByText("Get started")).toBeVisible();
    await expect(page.getByRole("heading", { name: "Step 1 · Connect a project" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Connect project" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Do next safe step" })).toBeDisabled();
  });

  test("classic Workflow previews the whole 8-step journey", async ({ page }) => {
    await openWorkflow(page);
    await expect(page.getByRole("heading", { name: "One task, eight steps" })).toBeVisible();
    const track = page.locator(".journey-preview .journey-track");
    await expect(track.locator(".journey-node")).toHaveCount(8);
    await expect(page.locator(".journey-preview").getByText("Preview")).toBeVisible();
  });

  test("classic Workflow connect button enables once name + path are filled", async ({ page }) => {
    await openWorkflow(page);
    const connect = page.getByRole("button", { name: "Connect project" });
    await expect(connect).toBeDisabled();
    await page.getByPlaceholder("my-app", { exact: true }).fill("demo");
    await page.getByPlaceholder("/Users/you/code/my-app").fill("/tmp/demo");
    await expect(connect).toBeEnabled();
  });
});
