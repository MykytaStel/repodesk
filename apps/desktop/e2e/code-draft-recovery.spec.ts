import { expect, test } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc, recordedInvocations } from "./mock-ipc";

const path = ".gitignore";
const diskFingerprint = "a".repeat(64);
const draftFingerprint = "b".repeat(64);
const savedFingerprint = "c".repeat(64);
const diskContent = "# disk version\nnode_modules/\n";
const recoveredContent = "# recovered unsaved draft\nnode_modules/\n.tmp/\n";

function workspaceFixtures(state: "safe" | "conflict") {
  return {
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
          bytes: diskContent.length,
          status: "modified",
          blocked: false,
        },
      ],
    },
    code_workspace_read: {
      path,
      content: diskContent,
      bytes: diskContent.length,
      line_count: 2,
      language: "gitignore",
      status: "modified",
      fingerprint: diskFingerprint,
    },
    code_workspace_draft_load: {
      draft: {
        path,
        content: recoveredContent,
        base_fingerprint: state === "safe" ? diskFingerprint : "d".repeat(64),
        content_fingerprint: draftFingerprint,
        updated_at: "2026-08-13T00:00:00Z",
      },
      state,
    },
    code_workspace_draft_save: {
      path,
      content: recoveredContent,
      base_fingerprint: diskFingerprint,
      content_fingerprint: draftFingerprint,
      updated_at: "2026-08-13T00:00:01Z",
    },
    code_workspace_draft_delete: true,
    code_workspace_save: {
      document: {
        path,
        content: recoveredContent,
        bytes: recoveredContent.length,
        line_count: 3,
        language: "gitignore",
        status: "modified",
        fingerprint: savedFingerprint,
      },
      previous_fingerprint: diskFingerprint,
      changed: true,
    },
    "language_intelligence_snapshot:snapshot": {
      project: "RepoDesk",
      primary_language: "rust",
      available_count: 0,
      generated_at: "2026-08-13T00:00:00Z",
      servers: [],
    },
  };
}

async function openCodeFile(page: import("@playwright/test").Page) {
  await page.goto("/");
  await page.getByRole("button", { name: /^Code —/ }).click();
  await page.getByRole("treeitem", { name: /.gitignore/ }).click();
}

test.describe("Code editor draft recovery", () => {
  test("auto-restores a safe draft, persists it, then clears recovery after Save", async ({ page }) => {
    await installMockIpc(page, workspaceFixtures("safe"));
    await openCodeFile(page);

    await expect(page.locator(".semantic-code-editor-host .cm-content")).toContainText("recovered unsaved draft");
    await expect(page.locator(".code-draft-badge")).toHaveText("recovered");
    await expect(page.locator(".code-document-location")).toContainText("Recovered draft · not saved");

    await expect.poll(async () => {
      const invocations = await recordedInvocations(page);
      return invocations.some(({ cmd }) => cmd === "code_workspace_draft_save");
    }).toBe(true);

    const beforeSave = await recordedInvocations(page);
    expect(beforeSave).toContainEqual({
      cmd: "code_workspace_draft_load",
      args: {
        input: {
          path,
          current_fingerprint: diskFingerprint,
        },
      },
    });
    expect(beforeSave).toContainEqual({
      cmd: "code_workspace_draft_save",
      args: {
        input: {
          path,
          content: recoveredContent,
          base_fingerprint: diskFingerprint,
        },
      },
    });

    await page.getByRole("button", { name: /^Save$/ }).click();
    await expect(page.locator(".code-draft-badge")).toHaveCount(0);
    await expect(page.locator(".code-document-location")).not.toContainText("Recovered draft · not saved");

    const afterSave = await recordedInvocations(page);
    expect(afterSave).toContainEqual({
      cmd: "code_workspace_save",
      args: {
        input: {
          path,
          content: recoveredContent,
          expected_fingerprint: diskFingerprint,
        },
      },
    });
    expect(afterSave).toContainEqual({
      cmd: "code_workspace_draft_delete",
      args: { relativePath: path },
    });
  });

  test("does not silently restore a conflicting draft when user keeps the disk version", async ({ page }) => {
    await installMockIpc(page, workspaceFixtures("conflict"));

    page.once("dialog", async (dialog) => {
      expect(dialog.type()).toBe("confirm");
      expect(dialog.message()).toContain("changed on disk after the draft was created");
      await dialog.dismiss();
    });

    await openCodeFile(page);

    await expect(page.locator(".semantic-code-editor-host .cm-content")).toContainText("disk version");
    await expect(page.locator(".semantic-code-editor-host .cm-content")).not.toContainText("recovered unsaved draft");
    await expect(page.locator(".code-draft-badge")).toHaveCount(0);

    const invocations = await recordedInvocations(page);
    expect(invocations).toContainEqual({
      cmd: "code_workspace_draft_load",
      args: {
        input: {
          path,
          current_fingerprint: diskFingerprint,
        },
      },
    });
    expect(invocations).toContainEqual({
      cmd: "code_workspace_draft_delete",
      args: { relativePath: path },
    });
    expect(invocations.some(({ cmd }) => cmd === "code_workspace_draft_save")).toBe(false);
  });
});
