import { expect, test, type Page } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc } from "./mock-ipc";

type SyntaxFixture = {
  path: string;
  language: string;
  content: string;
};

function syntaxFixtures({ path, language, content }: SyntaxFixture) {
  const name = path.split("/").at(-1) ?? path;
  const extension = name.includes(".") ? name.split(".").at(-1) ?? "" : "";
  return {
    ...onboardedFixtures,
    code_workspace_snapshot: {
      project: "RepoDesk",
      source: "git_index",
      truncated: false,
      files: [{
        path,
        name,
        extension,
        language,
        bytes: content.length,
        status: "modified",
        blocked: false,
      }],
    },
    code_workspace_read: {
      path,
      content,
      bytes: content.length,
      line_count: content.split("\n").length,
      language,
      status: "modified",
      fingerprint: `${language}-syntax-fixture`,
    },
    "language_intelligence_snapshot:snapshot": {
      project: "RepoDesk",
      primary_language: language,
      available_count: 0,
      generated_at: "2026-08-10T14:00:00Z",
      servers: [],
    },
  };
}

async function openFixture(page: Page, fixture: SyntaxFixture) {
  await installMockIpc(page, syntaxFixtures(fixture));
  await page.goto("/");
  await page.getByRole("button", { name: /^Code —/ }).click();
  await page.getByRole("treeitem", { name: new RegExp(fixture.path.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")) }).click();
  await expect(page.locator(".semantic-code-editor-host .cm-editor")).toBeVisible();
  await expect(page.locator(".code-editor-status")).toContainText(fixture.language);
}

test.describe("structured syntax highlighting", () => {
  test("highlights TOML tables, keys, and values", async ({ page }) => {
    await openFixture(page, {
      path: "Cargo.toml",
      language: "toml",
      content: "[package]\nname = \"repodesk\"\nversion = \"0.1.0\"",
    });

    await expect(page.locator(".semantic-code-editor-host .cm-line span", { hasText: "[package]" })).toHaveCount(1);
    await expect(page.locator(".semantic-code-editor-host .cm-line span", { hasText: "name" })).toHaveCount(1);
    await expect(page.locator(".semantic-code-editor-host .cm-line span", { hasText: "\"repodesk\"" })).toHaveCount(1);
  });

  test("highlights YAML keys, values, and comments", async ({ page }) => {
    await openFixture(page, {
      path: "compose.yaml",
      language: "yaml",
      content: "name: repodesk\nenabled: true\n# local config",
    });

    await expect(page.locator(".semantic-code-editor-host .cm-line span", { hasText: "name" })).toHaveCount(1);
    await expect(page.locator(".semantic-code-editor-host .cm-line span", { hasText: "true" })).toHaveCount(1);
    await expect(page.locator(".semantic-code-editor-host .cm-line span", { hasText: "# local config" })).toHaveCount(1);
  });

  test("renders HTML as markup instead of plaintext", async ({ page }) => {
    await openFixture(page, {
      path: "index.html",
      language: "html",
      content: "<!DOCTYPE html>\n<main class=\"app\">RepoDesk</main>",
    });

    await expect(page.locator(".semantic-code-editor-host .cm-line span", { hasText: "main" })).toHaveCount(2);
    await expect(page.locator(".semantic-code-editor-host .cm-line span", { hasText: "class" })).toHaveCount(1);
    await expect(page.locator(".semantic-code-editor-host .cm-line span", { hasText: "\"app\"" })).toHaveCount(1);
  });

  test("keeps plaintext files unparsed", async ({ page }) => {
    await openFixture(page, {
      path: "notes.txt",
      language: "plaintext",
      content: "name = \"just text\"",
    });

    await expect(page.locator(".semantic-code-editor-host .cm-line span")).toHaveCount(0);
  });
});
