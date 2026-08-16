import { expect, test } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { installMockIpc } from "./mock-ipc";

async function gridColumnCount(page: import("@playwright/test").Page, selector: string) {
  return page.locator(selector).evaluate((element) =>
    getComputedStyle(element).gridTemplateColumns.split(" ").filter(Boolean).length,
  );
}

test.describe("Work canonical visual ownership", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, currentOnboardedFixtures);
    await page.goto("/");
  });

  test("Work uses one canonical non-versioned workbench shell", async ({ page }) => {
    const workbench = page.locator(".work-workbench");
    await expect(workbench).toBeVisible();
    await expect(page.locator(".work-workbench-v3")).toHaveCount(0);
    expect(await workbench.evaluate((element) => getComputedStyle(element).display)).toBe("grid");
    expect(await gridColumnCount(page, ".work-workbench")).toBe(2);
  });

  test("canonical Work shell preserves narrow block layout", async ({ page }) => {
    const workbench = page.locator(".work-workbench");
    await page.setViewportSize({ width: 680, height: 720 });
    await expect(workbench).toBeVisible();
    expect(await workbench.evaluate((element) => getComputedStyle(element).display)).toBe("block");
  });
});
