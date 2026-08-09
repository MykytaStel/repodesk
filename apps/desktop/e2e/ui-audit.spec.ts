import { expect, test } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc } from "./mock-ipc";

const primaryTabs = ["Work", "Code", "Changes", "Runs", "Projects"];

for (const viewport of [
  { name: "desktop", width: 1280, height: 800 },
  { name: "narrow", width: 760, height: 720 },
]) {
  test(`primary surfaces fit the ${viewport.name} workspace`, async ({ page }) => {
    const errors: string[] = [];
    page.on("console", (message) => {
      if (message.type() === "error") errors.push(message.text());
    });
    await page.setViewportSize(viewport);
    await installMockIpc(page, onboardedFixtures);
    await page.goto("/");

    for (const tab of primaryTabs) {
      await page.getByRole("button", { name: new RegExp(`^${tab} —`) }).click();
      await expect(page.locator("main")).toBeVisible();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth)).toBe(true);
      expect(await page.locator("main").evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
    }

    expect(errors).toEqual([]);
  });
}
