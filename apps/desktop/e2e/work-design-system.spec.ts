import { expect, test, type Page } from "@playwright/test";
import { currentOnboardedFixtures } from "./current-fixtures";
import { incompleteReviewFixtures, type CommandFixtures } from "./fixtures";
import { installMockIpc } from "./mock-ipc";

async function boot(page: Page, fixtures: CommandFixtures) {
  await installMockIpc(page, fixtures);
  await page.goto("/");
}

test.describe("Work semantic design-system migration", () => {
  test("current Work phase and completed phases expose typed semantic state", async ({ page }) => {
    await boot(page, currentOnboardedFixtures);

    await expect(page.getByRole("status", { name: "Current phase: Execute" })).toHaveAttribute(
      "data-semantic-tone",
      "info",
    );
    await expect(page.getByRole("status", { name: "Phase Scope: Done" })).toHaveAttribute(
      "data-semantic-tone",
      "positive",
    );
  });

  test("prepared execution context is positive while missing approvals stay attention", async ({ page }) => {
    await boot(page, currentOnboardedFixtures);

    const packet = page.getByRole("region", { name: "Execution packet preview" });
    await expect(packet.getByRole("status", { name: "Execution context: Prepared" })).toHaveAttribute(
      "data-semantic-tone",
      "positive",
    );
    await expect(page.getByRole("status", { name: "Launch approvals: Action required" })).toHaveAttribute(
      "data-semantic-tone",
      "attention",
    );
  });

  test("Work route uses the shared surface loading state", async ({ page }) => {
    const delayed = {
      ...currentOnboardedFixtures,
      work_phase_state: {
        __mock_delay_ms: 1_000,
        __mock_value: currentOnboardedFixtures.work_phase_state,
      },
    } as CommandFixtures;

    await boot(page, delayed);
    await expect(page.getByRole("status").filter({ hasText: "Loading Work Item flow" })).toBeVisible();
  });

  test("Work route uses the shared surface error state with recovery actions", async ({ page }) => {
    const failed = {
      ...currentOnboardedFixtures,
      work_phase_state: { __mock_error: "fixture work failure" },
    } as CommandFixtures;

    await boot(page, failed);
    const alert = page.getByRole("alert").filter({ hasText: "RepoDesk stopped instead of guessing" });
    await expect(alert).toBeVisible();
    await expect(alert).toHaveAttribute("data-semantic-tone", "critical");
    await expect(alert.getByRole("button", { name: "Retry" })).toBeVisible();
    await expect(alert.getByRole("button", { name: "Open Runs" })).toBeVisible();
  });

  test("incomplete review evidence is a critical semantic blocker", async ({ page }) => {
    await boot(page, incompleteReviewFixtures);

    const alert = page.getByRole("alert").filter({
      hasText: "cannot prove which tracked paths changed",
    });
    await expect(alert).toBeVisible();
    await expect(alert).toHaveAttribute("data-semantic-tone", "critical");
  });
});
