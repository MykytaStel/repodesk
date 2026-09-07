import { test, expect } from "@playwright/test";
import { installMockIpc, recordedCommands, recordedInvocations } from "./mock-ipc";
import { currentOnboardedFixtures } from "./current-fixtures";
import { workControlFixtures } from "../src/features/work/work-control-fixtures";

test.describe("adaptive verification control surface", () => {
  test.beforeEach(async ({ page }) => {
    await installMockIpc(page, {
      ...currentOnboardedFixtures,
      work_verification_advisor: workControlFixtures.ready,
      work_decision_receipt: null,
      work_record_decision: workControlFixtures.deferredReceipt,
    });
    await page.goto("/");
  });

  test("makes the next verification action and its evidence state obvious", async ({ page }) => {
    const surface = page.getByRole("region", { name: "Work Item control" });
    await expect(surface.getByRole("heading", { name: "What should happen next?" })).toBeVisible();
    await expect(surface.getByRole("button", { name: /Run targeted checks/ })).toBeVisible();
    await expect(surface.getByText(/Not measured|Unknown/).first()).toBeVisible();
    await expect(surface.getByText("Deferred", { exact: true })).toBeVisible();
    await expect(surface.getByText("Full integration suite", { exact: true })).toBeVisible();
    await expect(surface.getByText("Unrelated to the changed paths", { exact: false })).toBeVisible();
  });

  test("records a decision without silently running a check", async ({ page }) => {
    const surface = page.getByRole("region", { name: "Work Item control" });
    await surface.getByRole("button", { name: "Defer with debt" }).click();
    await expect.poll(async () => await recordedCommands(page)).toContain("work_record_decision");
    const invocations = await recordedInvocations(page);
    expect(invocations.some(({ cmd }) => cmd === "task_runner_run" || cmd === "work_verify")).toBe(false);
  });

  test("opens the decision receipt inline", async ({ page }) => {
    await installMockIpc(page, {
      ...currentOnboardedFixtures,
      work_verification_advisor: { ...workControlFixtures.ready, latest_receipt: workControlFixtures.deferredReceipt },
      work_decision_receipt: workControlFixtures.deferredReceipt,
    });
    await page.reload();
    const surface = page.getByRole("region", { name: "Work Item control" });
    await surface.getByRole("button", { name: /View decision receipt/ }).click();
    const dialog = page.getByRole("dialog", { name: "Decision Receipt" });
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText("tree-work-control-1", { exact: true })).toBeVisible();
  });
});
