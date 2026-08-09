import { expect, test } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc } from "./mock-ipc";

test.describe("code editor geometry", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, onboardedFixtures);
    await page.goto("/");
    await page.getByRole("button", { name: /^Code —/ }).click();
    await page.getByRole("treeitem", { name: /.gitignore/ }).click();
    await expect(page.locator(".code-editor-input")).toBeVisible();
  });

  test("the source owns scrolling and the active number is not duplicated", async ({ page }) => {
    const editor = page.locator(".code-editor-input");
    const gutter = page.locator(".code-editor-gutter-shell");
    const gutterTrack = page.locator(".code-editor-gutter-track");
    const activeMarker = page.locator(".code-editor-active-line-number");

    await editor.evaluate((element: HTMLTextAreaElement) => {
      const seventhLineOffset = element.value.split("\n").slice(0, 6).join("\n").length + 1;
      element.focus();
      element.setSelectionRange(seventhLineOffset, seventhLineOffset);
      element.dispatchEvent(new Event("select", { bubbles: true }));
    });

    await expect(activeMarker).toHaveText("");
    await expect(gutter).toHaveCSS("overflow", "hidden");
    await expect(gutterTrack).toHaveCSS("max-height", "none");
    await expect(gutterTrack).toHaveCSS("overflow", "visible");
    expect(await gutterTrack.evaluate((element) => element.scrollHeight === element.clientHeight)).toBe(true);
    expect(await gutterTrack.evaluate((element) => element.clientHeight)).toBeGreaterThan(460);
    expect(await page.locator(".code-editor-line-number").count()).toBe(
      await editor.evaluate((element) => element.value.split("\n").length),
    );
    expect(await editor.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(true);
  });

  test("the gutter follows vertical scroll without moving horizontally", async ({ page }) => {
    const editor = page.locator(".code-editor-input");
    const gutter = page.locator(".code-editor-gutter-shell");

    await editor.evaluate((element: HTMLTextAreaElement) => {
      element.scrollTop = 1300;
      element.scrollLeft = 420;
      element.dispatchEvent(new Event("scroll", { bubbles: true }));
    });

    await expect.poll(() => gutter.evaluate((element) =>
      getComputedStyle(element).getPropertyValue("--editor-scroll-top").trim(),
    )).toBe("1300px");
    expect(await gutter.evaluate((element) => element.scrollLeft)).toBe(0);
  });

  test("the visible scrollbar is on the editor's right edge, outside the gutter", async ({ page }) => {
    const editor = page.locator(".code-editor-input");
    const gutter = page.locator(".code-editor-gutter-shell");
    const scrollbar = page.locator(".code-editor-scrollbar");

    await expect(scrollbar).toBeVisible();
    const [editorBox, gutterBox, scrollbarBox] = await Promise.all([
      editor.boundingBox(),
      gutter.boundingBox(),
      scrollbar.boundingBox(),
    ]);
    expect(editorBox).not.toBeNull();
    expect(gutterBox).not.toBeNull();
    expect(scrollbarBox).not.toBeNull();
    expect(scrollbarBox!.x).toBeGreaterThan(gutterBox!.x + gutterBox!.width);
    expect(scrollbarBox!.x + scrollbarBox!.width).toBeLessThanOrEqual(editorBox!.x + editorBox!.width + 1);
    expect(await editor.evaluate((element) => getComputedStyle(element, "::-webkit-scrollbar").width)).toBe("0px");
  });

  test("clicking a line number moves the caret and fills the gutter highlight", async ({ page }) => {
    const editor = page.locator(".code-editor-input");
    const gutter = page.locator(".code-editor-gutter-shell");
    const activeMarker = page.locator(".code-editor-active-line-number");

    await page.locator('.code-editor-line-number[data-line="13"]').click();

    await expect(editor).toBeFocused();
    expect(await editor.evaluate((element) => element.selectionStart)).toBe(
      await editor.evaluate((element) => element.value.split("\n").slice(0, 12).join("\n").length + 1),
    );
    await expect(page.locator(".code-editor-status")).toContainText("Ln 13, Col 1");

    const [gutterBox, markerBox] = await Promise.all([
      gutter.boundingBox(),
      activeMarker.boundingBox(),
    ]);
    expect(gutterBox).not.toBeNull();
    expect(markerBox).not.toBeNull();
    expect(Math.abs(markerBox!.x - gutterBox!.x)).toBeLessThanOrEqual(1);
    expect(Math.abs(
      markerBox!.x + markerBox!.width - (gutterBox!.x + gutterBox!.width),
    )).toBeLessThanOrEqual(1);
  });
});
