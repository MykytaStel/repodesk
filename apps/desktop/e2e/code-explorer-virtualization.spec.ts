import { expect, test } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc } from "./mock-ipc";

const targetPath = "file-999.rs";
const targetSource = "pub fn last_file() -> usize { 999 }\n";

const largeFiles = Array.from({ length: 1_000 }, (_, index) => {
  const name = `file-${String(index).padStart(3, "0")}.rs`;
  return {
    path: name,
    name,
    extension: "rs",
    language: "rust",
    bytes: index === 999 ? targetSource.length : 32,
    status: index === 999 ? "modified" : "clean",
    blocked: false,
  };
});

const virtualizationFixtures = {
  ...onboardedFixtures,
  code_workspace_snapshot: {
    project: "RepoDesk",
    source: "git_index",
    truncated: false,
    files: largeFiles,
  },
  code_workspace_quick_open: [
    {
      path: targetPath,
      name: targetPath,
      language: "rust",
      status: "modified",
    },
  ],
  code_workspace_read: {
    path: targetPath,
    content: targetSource,
    bytes: targetSource.length,
    line_count: 1,
    language: "rust",
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

async function openCode(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.getByRole("button", { name: /^Code —/ }).click();
}

test.describe("Large repository Explorer", () => {
  test("windows a 1000-file tree instead of mounting every row", async ({ page }) => {
    await installMockIpc(page, virtualizationFixtures);
    await openCode(page);

    const tree = page.getByRole("tree", { name: "Repository files" });
    await expect(tree).toHaveAttribute("data-virtualized", "true");
    await expect(tree).toHaveAttribute("data-total-rows", "1000");

    const mountedRows = tree.locator(".code-tree-row.file");
    await expect.poll(() => mountedRows.count()).toBeLessThan(100);

    await tree.evaluate((element) => {
      element.scrollTop = element.scrollHeight;
      element.dispatchEvent(new Event("scroll"));
    });

    await expect(page.getByRole("treeitem", { name: /file-999.rs/ })).toBeVisible();
    await expect.poll(() => mountedRows.count()).toBeLessThan(100);
  });

  test("auto-reveals a far-away active file opened through Quick Open", async ({ page }) => {
    await installMockIpc(page, virtualizationFixtures);
    await openCode(page);

    const tree = page.getByRole("tree", { name: "Repository files" });
    await expect(tree).toHaveAttribute("data-virtualized", "true");

    await page.keyboard.press("Control+k");
    const search = page.getByRole("textbox", { name: "Search commands" });
    await search.fill("file-999");
    const result = page.getByRole("button", { name: /Open file: file-999.rs/ });
    await expect(result).toBeVisible();
    await result.click();

    await expect(page.locator(".code-document-location")).toContainText(targetPath);
    await expect(page.locator(".semantic-code-editor-host .cm-content")).toContainText("last_file");

    const activeRow = tree.locator(".code-tree-row.file.active");
    await expect(activeRow).toContainText(targetPath);
    await expect(activeRow).toBeVisible();
    await expect.poll(async () => tree.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  });
});
