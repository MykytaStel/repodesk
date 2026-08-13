import { expect, test } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc } from "./mock-ipc";

test.describe("Code IDE chrome accessibility", () => {
  test("icon toolbar keeps accessible names and context menu closes with Escape", async ({ page }) => {
    await installMockIpc(page, onboardedFixtures);
    await page.goto("/");
    await page.getByRole("button", { name: /^Code —/ }).click();

    await expect(page.getByRole("toolbar", { name: "Explorer actions" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Search project" })).toBeVisible();
    await expect(page.getByRole("button", { name: "New file" })).toBeVisible();
    await expect(page.getByRole("button", { name: "New folder" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Refresh Explorer" })).toBeVisible();

    const row = page.getByRole("treeitem", { name: /.gitignore/ });
    await row.click({ button: "right" });
    const menu = page.getByRole("menu", { name: /Explorer actions for .gitignore/ });
    await expect(menu).toBeVisible();
    await expect(menu.getByRole("menuitem", { name: "Rename…" })).toBeVisible();
    await expect(menu.getByRole("menuitem", { name: "Copy Relative Path" })).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(menu).toBeHidden();
  });
});
