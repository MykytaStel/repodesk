import { expect, test } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { installMockIpc } from "./mock-ipc";

test.describe("balanced Knowledge layout", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, currentOnboardedFixtures);
    await page.goto("/");
    await page.getByRole("button", { name: /^Projects —/ }).click();
    await page.getByRole("tab", { name: "Knowledge" }).click();
    await expect(page.getByRole("heading", { name: "Engineering knowledge" })).toBeVisible();
  });

  test("keeps tabs and primary copy readable", async ({ page }) => {
    const review = page.locator(".knowledge-summary-strip button").first();
    const label = review.locator("span");
    expect(Number.parseFloat(await label.evaluate((element) => getComputedStyle(element).fontSize))).toBeGreaterThanOrEqual(12);
    await expect(review).toHaveClass(/active/);
    await expect(page.locator(".knowledge-empty-state")).toBeVisible();
  });

  test("bounds and reflows the proposal form", async ({ page }) => {
    await page.getByRole("button", { name: "Add knowledge" }).first().click();
    const form = page.locator(".knowledge-proposal-form");
    expect(await form.evaluate((element) => element.getBoundingClientRect().width)).toBeLessThanOrEqual(760);

    await page.setViewportSize({ width: 760, height: 720 });
    const columns = await page.locator(".knowledge-proposal-grid").evaluate((element) => getComputedStyle(element).gridTemplateColumns.split(" "));
    expect(columns).toHaveLength(1);
  });
});
