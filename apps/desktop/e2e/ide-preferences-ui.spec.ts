import { expect, test, type Page } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc } from "./mock-ipc";

async function openFromPalette(page: Page, title: string) {
  await page.getByRole("button", { name: "Command palette" }).click();
  const input = page.getByRole("textbox", { name: "Search commands" });
  await input.fill(title);
  await page.keyboard.press("Enter");
}

test.describe("IDE preferences", () => {
  test("settings update CodeMirror and Explorer presentation", async ({ page }) => {
    await installMockIpc(page, onboardedFixtures);
    await page.goto("/");

    await openFromPalette(page, "Settings");
    await expect(page.getByRole("heading", { name: "Code workspace" })).toBeVisible();

    await page.getByRole("combobox", { name: "Editor font size" }).selectOption("16");
    await page.getByRole("combobox", { name: "Editor tab size" }).selectOption("4");
    await page.getByRole("combobox", { name: "Explorer density" }).selectOption("comfortable");
    await page.getByRole("checkbox", { name: "Word wrap" }).check();

    const stored = await page.evaluate(() => JSON.parse(localStorage.getItem("repodesk.ide-preferences.v1") || "{}"));
    expect(stored).toMatchObject({
      editorFontSize: 16,
      tabSize: 4,
      wordWrap: true,
      explorerDensity: "comfortable",
    });

    await page.getByRole("button", { name: /^Code —/ }).click();
    const row = page.getByRole("treeitem", { name: /.gitignore/ });
    await row.click();

    const editor = page.locator(".semantic-code-editor-host .cm-editor");
    const content = page.locator(".semantic-code-editor-host .cm-content");
    await expect(editor).toBeVisible();
    await expect(content).toHaveClass(/cm-lineWrapping/);

    const fontSize = await editor.evaluate((element) => getComputedStyle(element).fontSize);
    const tabSize = await content.evaluate((element) => getComputedStyle(element).tabSize);
    const rowBox = await row.boundingBox();

    expect(fontSize).toBe("16px");
    expect(tabSize).toBe("4");
    expect(Math.round(rowBox?.height ?? 0)).toBe(27);
  });
});
