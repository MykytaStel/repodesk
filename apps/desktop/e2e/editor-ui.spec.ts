import { expect, test } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc } from "./mock-ipc";

test.describe("semantic code editor geometry", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, onboardedFixtures);
    await page.goto("/");
    await page.getByRole("button", { name: /^Code —/ }).click();
    await page.getByRole("treeitem", { name: /.gitignore/ }).click();
    await expect(page.locator(".semantic-code-editor-host .cm-editor")).toBeVisible();
  });

  test("CodeMirror owns scrolling and renders one active line number", async ({ page }) => {
    const scroller = page.locator(".semantic-code-editor-host .cm-scroller");
    const lineNumbers = page.locator(".semantic-code-editor-host .cm-lineNumbers");

    await expect(lineNumbers.locator(".cm-activeLineGutter")).toHaveCount(1);
    expect(await lineNumbers.locator(".cm-gutterElement").count()).toBeGreaterThan(20);
    expect(await scroller.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(true);
  });

  test("the CodeMirror gutter stays fixed during two-axis scrolling", async ({ page }) => {
    const scroller = page.locator(".semantic-code-editor-host .cm-scroller");
    const gutters = page.locator(".semantic-code-editor-host .cm-gutters");
    const before = await gutters.boundingBox();

    await scroller.evaluate((element) => {
      element.scrollTop = 600;
      element.scrollLeft = 420;
      element.dispatchEvent(new Event("scroll", { bubbles: true }));
    });

    await expect.poll(() => scroller.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
    const after = await gutters.boundingBox();
    expect(before).not.toBeNull();
    expect(after).not.toBeNull();
    expect(Math.abs(after!.x - before!.x)).toBeLessThanOrEqual(1);
  });

  test("the only editor scrollbar belongs to the right-side scroller", async ({ page }) => {
    const editor = page.locator(".semantic-code-editor-host .cm-editor");
    const scroller = page.locator(".semantic-code-editor-host .cm-scroller");
    const gutters = page.locator(".semantic-code-editor-host .cm-gutters");
    const [editorBox, gutterBox] = await Promise.all([editor.boundingBox(), gutters.boundingBox()]);

    expect(editorBox).not.toBeNull();
    expect(gutterBox).not.toBeNull();
    expect(gutterBox!.x + gutterBox!.width).toBeLessThan(editorBox!.x + editorBox!.width);
    expect(await scroller.evaluate((element) =>
      getComputedStyle(element, "::-webkit-scrollbar").width,
    )).toBe("10px");
    expect(await gutters.evaluate((element) => ({
      vertical: element.scrollHeight === element.clientHeight,
      horizontal: element.scrollWidth === element.clientWidth,
    }))).toEqual({ vertical: true, horizontal: true });
  });

  test("clicking a CodeMirror line number moves the caret and fills its gutter", async ({ page }) => {
    const content = page.locator(".semantic-code-editor-host .cm-content");
    const lineNumbers = page.locator(".semantic-code-editor-host .cm-lineNumbers");
    const line13 = lineNumbers.locator(".cm-gutterElement", { hasText: /^13$/ });

    await line13.click();

    await expect(content).toBeFocused();
    await expect(page.locator(".code-editor-status")).toContainText("Ln 13, Col 1");
    const active = lineNumbers.locator(".cm-activeLineGutter");
    await expect(active).toHaveText("13");
    expect(await active.evaluate((element) => getComputedStyle(element).backgroundColor)).not.toBe("rgba(0, 0, 0, 0)");

    const [gutterBox, activeBox] = await Promise.all([lineNumbers.boundingBox(), active.boundingBox()]);
    expect(gutterBox).not.toBeNull();
    expect(activeBox).not.toBeNull();
    expect(Math.abs(activeBox!.x - gutterBox!.x)).toBeLessThanOrEqual(1);
    expect(Math.abs(
      activeBox!.x + activeBox!.width - (gutterBox!.x + gutterBox!.width),
    )).toBeLessThanOrEqual(1);
  });
});
