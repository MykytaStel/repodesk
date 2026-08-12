import { expect, test } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc, recordedInvocations } from "./mock-ipc";

const createdSource = "export const created = true;\n";

const fileOpsFixtures = {
  ...onboardedFixtures,
  code_workspace_create_file: {
    path: "src/created.ts",
    previous_path: null,
    kind: "file_created",
    language: "typescript",
  },
  code_workspace_read: {
    path: "src/created.ts",
    content: createdSource,
    bytes: createdSource.length,
    line_count: 1,
    language: "typescript",
    status: "untracked",
    fingerprint: "created-file-fingerprint",
  },
  "language_intelligence_snapshot:snapshot": {
    project: "RepoDesk",
    primary_language: "typescript",
    available_count: 0,
    generated_at: "2026-08-12T20:00:00Z",
    servers: [],
  },
};

test.describe("Code workspace file actions", () => {
  test("creates a repository file through guarded IPC and opens it immediately", async ({ page }) => {
    await installMockIpc(page, fileOpsFixtures);
    await page.goto("/");
    await page.getByRole("button", { name: /^Code —/ }).click();

    page.once("dialog", async (dialog) => {
      expect(dialog.type()).toBe("prompt");
      await dialog.accept("src/created.ts");
    });
    await page.getByRole("button", { name: "New file" }).click();

    await expect(page.getByRole("tab", { name: /created.ts/ })).toBeVisible();
    await expect(page.locator(".code-document-location")).toContainText("src/created.ts");
    await expect(page.locator(".semantic-code-editor-host .cm-content")).toContainText("export const created = true");

    const invocations = await recordedInvocations(page);
    expect(invocations).toContainEqual({
      cmd: "code_workspace_create_file",
      args: { input: { path: "src/created.ts", content: "" } },
    });
  });

  test("disables destructive file actions while the active editor has unsaved changes", async ({ page }) => {
    await installMockIpc(page, onboardedFixtures);
    await page.goto("/");
    await page.getByRole("button", { name: /^Code —/ }).click();
    await page.getByRole("treeitem", { name: /.gitignore/ }).click();

    const rename = page.getByRole("button", { name: "Rename" });
    const remove = page.getByRole("button", { name: "Delete" });
    await expect(rename).toBeEnabled();
    await expect(remove).toBeEnabled();

    await page.locator(".semantic-code-editor-host .cm-content").press("x");

    await expect(rename).toBeDisabled();
    await expect(remove).toBeDisabled();
    await expect(rename).toHaveAttribute("title", "Save or discard unsaved edits before rename/delete");
  });
});
