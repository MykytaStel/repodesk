import { expect, test } from "@playwright/test";
import { onboardedFixtures } from "./fixtures";
import { installMockIpc, recordedInvocations } from "./mock-ipc";

const path = "src/important/session_manager.rs";
const source = [
  "pub fn noop() {}",
  "pub fn restore_session() {",
  "    let session_id = 42;",
  "}",
  "",
].join("\n");

const projectSearchFixtures = {
  ...onboardedFixtures,
  code_workspace_snapshot: {
    project: "RepoDesk",
    source: "git_index",
    truncated: false,
    files: [
      {
        path,
        name: "session_manager.rs",
        extension: "rs",
        language: "rust",
        bytes: source.length,
        status: "modified",
        blocked: false,
      },
    ],
  },
  code_workspace_project_search: {
    query: "session_id",
    case_sensitive: false,
    matches: [
      {
        path,
        line: 3,
        column: 9,
        end_column: 19,
        preview: "    let session_id = 42;",
      },
    ],
    scanned_files: 412,
    scanned_bytes: 5_242_880,
    skipped_files: 2,
    truncated: false,
    workspace_truncated: false,
  },
  code_workspace_read: {
    path,
    content: source,
    bytes: source.length,
    line_count: 4,
    language: "rust",
    status: "modified",
    fingerprint: "project-search-session-manager",
  },
  "language_intelligence_snapshot:snapshot": {
    project: "RepoDesk",
    primary_language: "rust",
    available_count: 0,
    generated_at: "2026-08-13T00:00:00Z",
    servers: [],
  },
};

test.describe("Project-wide Code search", () => {
  test("searches bounded repository text and opens a result in the editor", async ({ page }) => {
    await installMockIpc(page, projectSearchFixtures);
    await page.goto("/");
    await page.getByRole("button", { name: /^Code —/ }).click();

    await page.getByRole("button", { name: "Search project" }).click();
    await expect(page.getByRole("complementary", { name: "Project search" })).toBeVisible();

    await page.getByRole("textbox", { name: "Search project text" }).fill("session_id");
    await page.getByRole("button", { name: /^Search$/ }).click();

    await expect(page.getByRole("button", { name: /session_manager.rs/ })).toBeVisible();
    await expect(page.getByText(/1 match · 412 files · 5.0 MiB/)).toBeVisible();
    await expect(page.getByText("2 files skipped by text/safety policy")).toBeVisible();

    const invocations = await recordedInvocations(page);
    expect(invocations).toContainEqual({
      cmd: "code_workspace_project_search",
      args: {
        input: {
          query: "session_id",
          case_sensitive: false,
          limit: 200,
        },
      },
    });

    await page.getByRole("button", { name: /session_manager.rs/ }).click();

    await expect(page.locator(".code-document-location")).toContainText(path);
    await expect(page.locator(".semantic-code-editor-host .cm-content")).toContainText("session_id = 42");
    await expect(page.getByRole("complementary", { name: "Repository explorer" })).toBeVisible();

    const afterOpen = await recordedInvocations(page);
    expect(afterOpen).toContainEqual({
      cmd: "code_workspace_read",
      args: { relativePath: path },
    });
  });
});
