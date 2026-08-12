import { expect, test } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc } from "./mock-ipc";

const path = ".gitignore";
const source = "node_modules/\n.tmp/\n";

const themeFixtures = {
  ...onboardedFixtures,
  code_workspace_snapshot: {
    project: "RepoDesk",
    source: "git_index",
    truncated: false,
    files: [
      {
        path,
        name: ".gitignore",
        extension: null,
        language: "gitignore",
        bytes: source.length,
        status: "modified",
        blocked: false,
      },
    ],
  },
  code_workspace_read: {
    path,
    content: source,
    bytes: source.length,
    line_count: 2,
    language: "gitignore",
    status: "modified",
    fingerprint: "a".repeat(64),
  },
  code_workspace_draft_load: null,
  "language_intelligence_snapshot:snapshot": {
    project: "RepoDesk",
    primary_language: "rust",
    available_count: 0,
    generated_at: "2026-08-13T00:00:00Z",
    servers: [],
  },
};

test("CodeMirror follows live shell theme without recreating the editor", async ({ page }) => {
  await installMockIpc(page, themeFixtures);
  await page.addInitScript(() => {
    window.localStorage.setItem("repodesk.theme", "light");
  });
  await page.goto("/");

  await page.getByRole("button", { name: /^Code —/ }).click();
  await page.getByRole("treeitem", { name: /.gitignore/ }).click();

  const host = page.locator(".semantic-code-editor-host");
  const editor = host.locator(".cm-editor");
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await expect(host).toHaveAttribute("data-editor-theme", "light");
  await expect(editor).toBeVisible();

  await editor.evaluate((element) => element.setAttribute("data-e2e-editor-instance", "preserved"));

  await page.keyboard.press("Control+k");
  const commandSearch = page.getByRole("textbox", { name: "Search commands" });
  await commandSearch.fill("Theme: Dark");
  await page.getByRole("button", { name: /Theme: Dark/ }).click();

  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await expect(host).toHaveAttribute("data-editor-theme", "dark");
  await expect(host.locator(".cm-editor")).toHaveAttribute("data-e2e-editor-instance", "preserved");

  await page.keyboard.press("Control+k");
  await page.getByRole("textbox", { name: "Search commands" }).fill("Theme: Light");
  await page.getByRole("button", { name: /Theme: Light/ }).click();

  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await expect(host).toHaveAttribute("data-editor-theme", "light");
  await expect(host.locator(".cm-editor")).toHaveAttribute("data-e2e-editor-instance", "preserved");
});
