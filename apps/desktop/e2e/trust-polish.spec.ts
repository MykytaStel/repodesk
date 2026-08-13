import { expect, test } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { installMockIpc, recordedCommands } from "./mock-ipc";

async function openFromPalette(page: import("@playwright/test").Page, title: string) {
  await page.getByRole("button", { name: "Command palette" }).click();
  const input = page.getByRole("textbox", { name: "Search commands" });
  await input.fill(title);
  await page.keyboard.press("Enter");
}

test("About presents the engineering-workspace identity and restores focus", async ({ page }) => {
  await installMockIpc(page, currentOnboardedFixtures);
  await page.goto("/");

  const opener = page.getByRole("button", { name: "About RepoDesk" });
  await opener.click();

  const dialog = page.getByRole("dialog", { name: "Your local-first engineering workspace" });
  await expect(dialog).toHaveAttribute("aria-modal", "true");
  await expect(dialog.getByText("AI operations cockpit")).toHaveCount(0);
  await expect(dialog.getByText("Code", { exact: true })).toBeVisible();
  await expect(dialog.getByText("Runs", { exact: true })).toBeVisible();
  await expect(dialog.getByText("Projects", { exact: true })).toBeVisible();

  await expect(dialog.getByRole("button", { name: "Get started" })).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(dialog.getByRole("button", { name: "Close" })).toBeFocused();

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(opener).toBeFocused();
});

test("project switcher distinguishes registry failure from an empty registry", async ({ page }) => {
  await installMockIpc(page, {
    ...currentOnboardedFixtures,
    project_list_configs: { __mock_error: "registry unavailable" },
  });
  await page.goto("/");

  await page.getByRole("button", { name: "Show workspace sidebar" }).click();
  await page.locator(".workspace-sidebar").getByRole("button", { name: "RepoDesk", exact: true }).click();

  await expect(page.getByRole("alert")).toContainText("Could not load projects");
  await expect(page.getByRole("alert")).toContainText("registry unavailable");
  await expect(page.getByRole("button", { name: "Retry loading projects" })).toBeVisible();
  await expect(page.getByText("No matching projects.")).toHaveCount(0);
});

test("Orchestrate cleanup uses a RepoDesk decision instead of a browser dialog", async ({ page }) => {
  await installMockIpc(page, {
    ...currentOnboardedFixtures,
    orchestrate_worktrees: [
      {
        workspace_id: "wt-trust-polish",
        run_id: "run-trust-polish",
        step_id: "implement",
        path: "/tmp/repodesk/worktrees/wt-trust-polish",
        base_commit: "abc123",
        created_at: "2026-08-13T10:00:00Z",
        metadata_path: null,
        git_tracked: true,
        exists: true,
        dirty: true,
        changed_files: ["src/app.ts"],
        removable: true,
        warnings: [],
      },
    ],
    orchestrate_cleanup_worktree: {
      workspace_id: "wt-trust-polish",
      path: "/tmp/repodesk/worktrees/wt-trust-polish",
      removed: true,
      metadata_removed: true,
      warnings: [],
    },
  });
  let nativeDialogOpened = false;
  page.on("dialog", async (dialog) => {
    nativeDialogOpened = true;
    await dialog.dismiss();
  });
  await page.goto("/");
  await openFromPalette(page, "Orchestrate");

  await page.getByRole("button", { name: "Cleanup worktree" }).click();
  const dialog = page.getByRole("dialog", { name: "Remove managed worktree?" });
  await expect(dialog).toContainText("1 changed file");
  expect(nativeDialogOpened).toBe(false);

  await dialog.getByRole("button", { name: "Remove worktree" }).click();
  await expect.poll(async () => (await recordedCommands(page)).filter((command) => command === "orchestrate_cleanup_worktree").length).toBe(1);
});
