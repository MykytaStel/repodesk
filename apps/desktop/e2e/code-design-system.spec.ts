import { expect, test, type Page } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { installMockIpc } from "./mock-ipc";
import type { CommandFixtures } from "./fixtures";

async function bootCode(page: Page, overrides: CommandFixtures = {}) {
  await page.addInitScript(() => {
    window.localStorage.setItem("repodesk.activeTab", "code");
  });
  await installMockIpc(page, { ...currentOnboardedFixtures, ...overrides });
  await page.goto("/");
  const welcomeDialog = page.locator(".app-dialog[role='dialog']");
  if (await welcomeDialog.isVisible()) {
    await welcomeDialog.getByRole("button", { name: "Close" }).click();
    await welcomeDialog.waitFor({ state: "hidden" });
  }
}

const codeWorkspace = {
  project: "RepoDesk",
  source: "git_index",
  truncated: false,
  files: [
    {
      path: ".gitignore",
      name: ".gitignore",
      extension: null,
      language: "gitignore",
      bytes: 6400,
      status: "modified",
      blocked: false,
    },
  ],
};

test.describe("Code design-system convergence", () => {
  test("no active project uses a surface-scoped semantic empty state", async ({ page }) => {
    await bootCode(page, {
      desktop_snapshot: { project: null, task: null },
      get_active_project_config: null,
    });

    const empty = page.getByText("Connect a project to open the Code workspace.", { exact: true });
    await expect(empty).toBeVisible();
    await expect(empty.locator("xpath=.." )).toHaveClass(/semantic-state--surface/);
  });

  test("workspace loading and authority failure use semantic surface states", async ({ page }) => {
    await bootCode(page, {
      code_workspace_snapshot: { __mock_delay_ms: 700, __mock_value: codeWorkspace },
    });

    const loading = page.getByRole("status").filter({ hasText: "Indexing repository files" });
    await expect(loading).toBeVisible();
    await expect(loading).toHaveClass(/semantic-state--surface/);

    const failedPage = await page.context().newPage();
    await bootCode(failedPage, {
      code_workspace_snapshot: { __mock_error: "fixture Code workspace failure" },
    });
    const failure = failedPage.getByRole("alert").filter({ hasText: "Code workspace unavailable" });
    await expect(failure).toBeVisible();
    await expect(failure).toHaveAttribute("data-semantic-tone", "critical");
    await expect(failure).toContainText("fixture Code workspace failure");
  });

  test("normal workspace uses the canonical shell and typed index status", async ({ page }) => {
    await bootCode(page, {
      code_workspace_snapshot: { ...codeWorkspace, truncated: true },
    });

    await expect(page.locator(".code-workspace")).toBeVisible();
    await expect(page.locator(".code-workspace-v0")).toHaveCount(0);
    await expect(page.getByText("Index capped", { exact: true })).toHaveAttribute("data-semantic-tone", "attention");
  });

  test("Explorer file status uses the typed semantic tone", async ({ page }) => {
    await bootCode(page, { code_workspace_snapshot: codeWorkspace });

    const row = page.getByRole("treeitem", { name: /.gitignore/ });
    await expect(row.getByText("M", { exact: true })).toHaveAttribute("data-semantic-tone", "attention");
  });

  test("active file scope and verification use typed semantic badges", async ({ page }) => {
    await bootCode(page, {
      code_workspace_snapshot: codeWorkspace,
      work_engineering_intelligence: {
        work_item_contract: {
          configured: true,
          contract: { work_item_id: "task-code-cut-f" },
        },
        change_governance: {
          origin: { workers: [{ kind: "human", id: "mykyta", provider: null, model: null }] },
          files: [{ path: ".gitignore", scope_state: "out_of_scope" }],
          review_state: "accepted",
          verification: { state: "passed" },
          gate: { state: "scope_violation", ready: false },
        },
      },
    });

    await page.getByRole("treeitem", { name: /.gitignore/ }).click();
    const editor = page.getByRole("region", { name: "Editor for .gitignore" });
    await expect(editor.getByText("Out of scope", { exact: true })).toHaveAttribute("data-semantic-tone", "critical");
    await expect(editor.getByText("Verified", { exact: true })).toHaveAttribute("data-semantic-tone", "positive");
    await expect(editor.getByText("Accepted", { exact: true })).toHaveAttribute("data-semantic-tone", "positive");
  });
});
