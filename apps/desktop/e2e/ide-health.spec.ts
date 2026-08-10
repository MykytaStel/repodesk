import { expect, test } from "@playwright/test";
import { onboardedFixtures, type CommandFixtures } from "./fixtures";
import {
  emitMockTauriEvent,
  installMockIpc,
  recordedCommands,
  recordedInvocations,
} from "./mock-ipc";

const recoveryConfirmation = ["recovery", "fixture", "proof"].join("_");

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
  evidence: [
    { label: "Project", value: "RepoDesk" },
    { label: "Install error", value: "Executable was not found" },
  ],
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

const healthyTypescriptRecord = {
  ...typescriptRecoveryRecord,
  generation: 4,
  state: "healthy",
  severity: "info",
  code: null,
  title: "TypeScript Language Server is ready",
  explanation: "Language intelligence completed protocol initialization.",
  actions: [],
  observed_at: "2026-08-10T12:00:04Z",
} as const;

const recoveryPreview = {
  capability_id: typescriptRecoveryRecord.capability_id,
  diagnosis_revision: typescriptRecoveryRecord.diagnosis_revision,
  action_id: "install-managed-language-server",
  title: "Install TypeScript Language Server",
  summary: "Install an allowlisted managed language tool without changing the repository.",
  risk: "moderate",
  recipe_id: "typescript-language-server",
  recipe_revision: "typescript-language-server:5.3.0:typescript:6.0.3",
  changes: [
    "Install typescript-language-server@5.3.0",
    "Write RepoDesk-managed files under ~/.repodesk/tools/language-servers",
  ],
  network_required: true,
  verification: "Verify the package, then initialize TypeScript Language Server through LSP",
  confirmation_token: recoveryConfirmation,
  expires_at: "2026-08-10T12:05:00Z",
} as const;

function recoveryFixtures(extra: CommandFixtures = {}): CommandFixtures {
  return {
    ...onboardedFixtures,
    recovery_snapshot: {
      project: "RepoDesk",
      records: [typescriptRecoveryRecord],
      actionable_count: 1,
      warnings: [],
      generated_at: "2026-08-10T12:00:00Z",
    },
    recovery_history: [
      {
        id: "attempt-1",
        capability_id: typescriptRecoveryRecord.capability_id,
        diagnosis_revision: "previous-diagnosis",
        action_id: "restart-language-session",
        started_at: "2026-08-10T11:59:00Z",
        finished_at: "2026-08-10T11:59:01Z",
        result: "failed",
        verification_summary: "Language server restart did not become ready",
      },
    ],
    recovery_repair_preview: recoveryPreview,
    recovery_repair_confirm: healthyTypescriptRecord,
    recovery_repair_cancel: true,
    ...extra,
  };
}

async function openIDEHealth(page: Parameters<typeof installMockIpc>[0]) {
  const indicator = page.getByRole("button", {
    name: "IDE health: 1 needs attention",
  });
  await expect(indicator).toBeVisible();
  await indicator.click();
  const panel = page.getByRole("dialog", { name: "IDE Health" });
  await expect(panel).toBeVisible();
  return panel;
}

test("live recovery event updates the selected IDE Health record without refetching", async ({
  page,
}) => {
  await installMockIpc(page, recoveryFixtures());
  await page.goto("/");

  const panel = await openIDEHealth(page);
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

test("IDE Health explains impact and requires review before a managed repair", async ({ page }) => {
  await installMockIpc(page, recoveryFixtures());
  await page.goto("/");

  const panel = await openIDEHealth(page);
  await expect(panel.getByText("Executable was not found", { exact: true })).toBeVisible();
  await expect(panel.getByText("Diagnostics", { exact: true })).toBeVisible();
  await expect(panel.getByText("Editing", { exact: true })).toBeVisible();
  await expect(panel.getByText("Language server restart did not become ready")).toBeVisible();

  await panel.getByRole("button", { name: "Review repair" }).click();
  await expect(panel.getByRole("heading", { name: "Install TypeScript Language Server" })).toBeVisible();
  await expect(panel.getByText("Moderate risk")).toBeVisible();
  await expect(panel.getByText("Install typescript-language-server@5.3.0")).toBeVisible();
  await expect(panel.getByText("Required", { exact: true })).toBeVisible();
  await expect(panel).not.toContainText(recoveryConfirmation);

  const previewInvocation = (await recordedInvocations(page)).find(
    (invocation) => invocation.cmd === "recovery_repair_preview",
  );
  expect(previewInvocation?.args).toEqual({
    capabilityId: typescriptRecoveryRecord.capability_id,
    actionId: "install-managed-language-server",
  });
});

test("approved repair sends only the recovery confirmation and supports cancellation", async ({ page }) => {
  await installMockIpc(page, recoveryFixtures({
    recovery_repair_confirm: {
      __mock_delay_ms: 250,
      __mock_value: healthyTypescriptRecord,
    },
  }));
  await page.goto("/");

  const panel = await openIDEHealth(page);
  await panel.getByRole("button", { name: "Review repair" }).click();
  await panel.getByRole("button", { name: "Approve repair" }).click();

  await expect(panel.getByText("Repairing", { exact: true })).toBeVisible();
  const cancelButton = panel.getByRole("button", { name: "Cancel repair" });
  await expect(cancelButton).toBeVisible();
  await cancelButton.click();

  const invocations = await recordedInvocations(page);
  const confirmInvocation = invocations.find(
    (invocation) => invocation.cmd === "recovery_repair_confirm",
  );
  expect(confirmInvocation?.args).toEqual({ confirmationToken: recoveryConfirmation });
  const cancelInvocation = invocations.find(
    (invocation) => invocation.cmd === "recovery_repair_cancel",
  );
  expect(cancelInvocation?.args).toEqual({ recipeId: "typescript-language-server" });
});
