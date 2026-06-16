import { test, expect } from "@playwright/test";
import { installMockIpc } from "./mock-ipc";
import { firstRunFixtures } from "./fixtures";

// First-run: an empty REPODESK_HOME with no project/task. The workflow surface
// must funnel the user into onboarding (Step 1 · Connect a project) rather than
// showing the daily loop.
test.describe("first run (empty workspace)", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, firstRunFixtures);
    await page.goto("/");
  });

  test("shows onboarding step 1 and gates the next-step CTA", async ({ page }) => {
    await expect(page.getByText("Get started")).toBeVisible();
    await expect(page.getByRole("heading", { name: "Step 1 · Connect a project" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Connect project" })).toBeVisible();
    // No project/task yet — the safe-next-step button is disabled.
    await expect(page.getByRole("button", { name: "Do next safe step" })).toBeDisabled();
  });

  test("header reflects no active project", async ({ page }) => {
    await expect(page.getByRole("heading", { level: 2, name: "No active project" })).toBeVisible();
    await expect(page.getByText("No active task")).toBeVisible();
  });

  test("Connect project button enables once name + path are filled", async ({ page }) => {
    const connect = page.getByRole("button", { name: "Connect project" });
    await expect(connect).toBeDisabled();
    await page.getByPlaceholder("my-app", { exact: true }).fill("demo");
    await page.getByPlaceholder("/Users/you/code/my-app").fill("/tmp/demo");
    await expect(connect).toBeEnabled();
  });
});
