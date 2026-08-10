import { expect, test } from "@playwright/test";
import { onboardedFixtures, type CommandFixtures } from "./fixtures";
import {
  emitMockTauriEvent,
  installMockIpc,
  recordedCommands,
} from "./mock-ipc";

const typescriptRecoveryRecord = {
  capability_id: "language:typescript-language-server",
  module_id: "language_intelligence",
  generation: 1,
  diagnosis_revision: "missing-typescript-language-server",
  observed_at: "2026-08-10T12:00:00Z",
  state: "needs_approval",
  severity: "warning",
  code: "missing_executable",
  title: "TypeScript Language Server is unavailable",
  explanation:
    "The configured language-server executable was not found. Editing and save remain available.",
  affected: ["Diagnostics", "Hover", "Definitions"],
  unaffected: ["Editing", "Scrolling", "Selection", "Save"],
  evidence: [{ label: "Project", value: "RepoDesk" }],
  actions: [
    {
      id: "install-managed-language-server",
      label: "Review repair",
      kind: "confirmable",
      recipe_id: "typescript-language-server",
    },
  ],
  automatic_attempts: 0,
} as const;

test("live recovery event updates the selected IDE Health record without refetching", async ({
  page,
}) => {
  const fixtures: CommandFixtures = {
    ...onboardedFixtures,
    recovery_snapshot: {
      project: "RepoDesk",
      records: [typescriptRecoveryRecord],
      actionable_count: 1,
      warnings: [],
      generated_at: "2026-08-10T12:00:00Z",
    },
  };
  await installMockIpc(page, fixtures);
  await page.goto("/");

  const indicator = page.getByRole("button", {
    name: "IDE health: 1 needs attention",
  });
  await expect(indicator).toBeVisible();
  await indicator.click();

  const panel = page.getByRole("dialog", { name: "IDE Health" });
  await expect(panel).toBeVisible();
  await expect(panel.getByText("TypeScript Language Server is unavailable", { exact: true })).toBeVisible();
  const snapshotCallsBefore = (await recordedCommands(page)).filter(
    (command) => command === "recovery_snapshot",
  ).length;

  await emitMockTauriEvent(page, "recovery-record-changed", {
    ...typescriptRecoveryRecord,
    generation: 2,
    state: "repairing",
    observed_at: "2026-08-10T12:00:01Z",
  });

  await expect(panel.getByText("Repairing", { exact: true })).toBeVisible();
  const snapshotCallsAfter = (await recordedCommands(page)).filter(
    (command) => command === "recovery_snapshot",
  ).length;
  expect(snapshotCallsAfter).toBe(snapshotCallsBefore);
});
