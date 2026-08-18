import { expect, test, type Page } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { installMockIpc } from "./mock-ipc";

async function bootWorkbench(page: Page) {
  await installMockIpc(page, currentOnboardedFixtures);
  await page.goto("/");
}

function modifierShortcut(key: string): string {
  return process.platform === "darwin" ? `Meta+${key}` : `Control+${key}`;
}

test.describe("Workbench interaction contract", () => {
  test("uses Navigator terminology and Cmd/Ctrl+B toggles the structural left pane", async ({ page }) => {
    await bootWorkbench(page);

    const toggle = page.getByRole("button", { name: /Show Navigator.*Ctrl\+B/i });
    await expect(toggle).toBeVisible();
    await toggle.click();

    await expect(page.getByRole("complementary", { name: "Workspace navigator" })).toBeVisible();

    await page.keyboard.press(modifierShortcut("B"));
    await expect(page.getByRole("complementary", { name: "Workspace navigator" })).toHaveCount(0);
  });

  test("Inspector exposes local close and Escape restores focus to its opener", async ({ page }) => {
    await bootWorkbench(page);

    const opener = page.getByRole("button", { name: "Show inspector" });
    await opener.focus();
    await opener.click();

    const inspector = page.getByRole("complementary", { name: "Engineering evidence inspector" });
    await expect(inspector).toBeVisible();
    await expect(inspector.getByRole("button", { name: "Close inspector" })).toBeVisible();

    await page.keyboard.press("Escape");

    await expect(inspector).toHaveCount(0);
    await expect(opener).toBeFocused();
  });

  test("Escape does not close the Bottom Panel", async ({ page }) => {
    await bootWorkbench(page);

    const toggle = page.getByRole("button", { name: /Show bottom panel.*Ctrl\+J/i });
    await toggle.click();

    const panel = page.getByRole("region", { name: "Workbench bottom panel" });
    await expect(panel).toBeVisible();

    await page.keyboard.press("Escape");
    await expect(panel).toBeVisible();
  });
});
